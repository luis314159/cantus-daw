//! The session's state and everything but its sync (`ara_sync.rs`). See `ara.rs`.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use cantus_ara::callbacks::PlaybackRequest;
use cantus_ara::host::HostDocument;
use cantus_core::ara::{AraArchive, AraClip, AraPlugin};
use cantus_core::model::{ClipId, Project, TrackId};
use cantus_core::{Command, PluginState, SampleData};

use super::AraStatus;
use crate::ara_worker::{ClipRender, SharedAudio, Worker};
use crate::backend::{HostError, MainThread, PluginInfo};

#[path = "ara_sync.rs"]
mod sync;

/// A source as the window can give it now.
pub enum SourceState {
    Ready(Arc<SampleData>),
    /// Still being read (the sample cache's loader): asked again next frame.
    Loading,
    Missing,
}

/// One bound clip's instance: its main half (here, on the document thread), its audio half
/// (shared with the render thread), its render generation and progress, its editor window.
pub(super) struct Bound {
    pub main: Box<dyn MainThread>,
    pub audio: SharedAudio,
    pub generation: Arc<AtomicU64>,
    pub progress: Arc<AtomicU32>,
    pub rendering: bool,
    /// A changed region waits until then to be rendered (a drag resting).
    pub due: Option<Instant>,
    /// The part the plug-in said changed, waiting for its render (frames from the region's
    /// start; `None` inside is the whole region).
    pub range: Option<Option<(i64, i64)>>,
    /// What the render on its way covers (`None` inside: the whole region): a newer job that
    /// cancels it covers that too.
    pub inflight: Option<Option<(i64, i64)>>,
    pub ext: crate::vst3::ara::Extension,
    pub window: Option<crate::window::PluginWindow>,
}

impl Drop for Bound {
    fn drop(&mut self) {
        // A render of it stops at its next block; the instance is torn down here, on the
        // document thread, once the render thread has let go of it.
        self.generation.fetch_add(1, Ordering::SeqCst);
        let start = Instant::now();
        while Arc::strong_count(&self.audio) > 1 && start.elapsed() < Duration::from_secs(10) {
            std::thread::sleep(Duration::from_millis(1));
        }
        if self.window.take().is_some() {
            self.main.close_editor();
        }
    }
}

pub(super) struct PluginDoc {
    pub info: PluginInfo,
    pub instances: BTreeMap<ClipId, Bound>,
    pub doc: Option<HostDocument>,
    pub _module: crate::vst3::ara::AraModule,
}

impl Drop for PluginDoc {
    fn drop(&mut self) {
        // Instances before their document, the document before the module.
        self.instances.clear();
        self.doc = None;
    }
}

/// What the project and the plug-in agree a clip's state is: the project's bytes and the
/// plug-in's own serialisation of the same state (a plug-in may write it differently).
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Agreed {
    pub project: Option<PluginState>,
    pub live: PluginState,
}

/// The plug-ins, documents, instances and renders of one open project.
pub struct AraSession {
    pub(super) docs: BTreeMap<String, PluginDoc>,
    /// Plug-ins that could not run, and why (tried again on *Retry*, or when the project
    /// stops naming them and names them anew).
    pub(super) failed: HashMap<String, AraStatus>,
    pub(super) renders: HashMap<ClipId, ClipRender>,
    pub(super) status: HashMap<ClipId, AraStatus>,
    pub(super) revision: u64,
    pub(super) renders_made: u64,
    /// Per clip: its binding number and the plug-in's archive when its document closed.
    pub(super) kept: HashMap<ClipId, (u64, AraArchive)>,
    /// A clip let go of its plug-in: its binding as it was, to bind again with its edits.
    pub(super) previous: HashMap<ClipId, AraClip>,
    pub(super) baseline: HashMap<ClipId, AraArchive>,
    pub(super) agreed: HashMap<ClipId, Agreed>,
    pub(super) edits: Vec<(Command, (ClipId, u64))>,
    pub(super) gestures: HashMap<ClipId, (u64, Instant)>,
    pub(super) next_gesture: u64,
    pub(super) requests: Vec<PlaybackRequest>,
    pub(super) epoch: Option<u64>,
    pub(super) worker: Option<Worker>,
    /// Plug-ins stopped for answering no more, and their render threads: never freed.
    pub(super) abandoned: Vec<PluginDoc>,
    pub(super) hang_limit: Duration,
    pub(super) debounce: Duration,
    pub(super) enabled: bool,
    /// Where the "a plug-in is running" marker is written (`None`: none, tests).
    pub(super) marker: Option<PathBuf>,
    /// What the marker names now: plug-in id → what it is doing.
    pub(super) in_flight: BTreeMap<String, &'static str>,
    pub(super) last_render: Option<Duration>,
}

impl Default for AraSession {
    fn default() -> Self {
        AraSession {
            docs: BTreeMap::new(),
            failed: HashMap::new(),
            renders: HashMap::new(),
            status: HashMap::new(),
            revision: 0,
            renders_made: 0,
            kept: HashMap::new(),
            previous: HashMap::new(),
            baseline: HashMap::new(),
            agreed: HashMap::new(),
            edits: Vec::new(),
            gestures: HashMap::new(),
            next_gesture: 0,
            requests: Vec::new(),
            epoch: None,
            worker: None,
            abandoned: Vec::new(),
            hang_limit: Duration::from_secs(30),
            debounce: super::DEBOUNCE,
            enabled: true,
            marker: None,
            in_flight: BTreeMap::new(),
            last_render: None,
        }
    }
}

pub(super) fn binding_of(project: &Project, clip: ClipId) -> Option<(TrackId, &AraClip)> {
    project.tracks.iter().find_map(|t| {
        t.clips
            .iter()
            .find(|c| c.id == clip)
            .and_then(|c| c.audio.as_ref()?.ara.as_deref())
            .map(|a| (t.id, a))
    })
}

/// The marker's file name in its folder.
pub const MARKER_FILE: &str = "ara-running.txt";

impl AraSession {
    pub fn new() -> Self {
        Self::default()
    }

    /// A session that keeps its crash marker in `dir`. A marker left there by a Cantus that
    /// did not close (a plug-in took it down) stops the plug-ins it names until *Retry*.
    pub fn with_marker(dir: PathBuf) -> Self {
        let mut s = Self::new();
        let path = dir.join(MARKER_FILE);
        if let Ok(text) = std::fs::read_to_string(&path) {
            for line in text.lines() {
                if let Some((id, what)) = line.split_once('\t') {
                    s.failed.insert(
                        id.to_ascii_uppercase(),
                        AraStatus::Stopped(format!("Cantus closed while the plug-in was {what}")),
                    );
                }
            }
            let _ = std::fs::remove_file(&path);
        }
        s.marker = Some(path);
        s
    }

    pub(super) fn mark(&mut self, plugin: &str, what: &'static str) {
        self.in_flight.insert(plugin.to_ascii_uppercase(), what);
        self.write_marker();
    }

    pub(super) fn unmark(&mut self, plugin: &str) {
        if self
            .in_flight
            .remove(&plugin.to_ascii_uppercase())
            .is_some()
        {
            self.write_marker();
        }
    }

    fn write_marker(&self) {
        let Some(path) = &self.marker else { return };
        if self.in_flight.is_empty() {
            let _ = std::fs::remove_file(path);
            return;
        }
        let text: String = self
            .in_flight
            .iter()
            .map(|(id, what)| format!("{id}\t{what}\n"))
            .collect();
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(path, text);
    }

    /// Follow the window's project: when `epoch` is another project's, everything of the
    /// last one goes. Whether it had any.
    pub fn follow(&mut self, epoch: u64) -> bool {
        if self.epoch == Some(epoch) {
            return false;
        }
        let had = !self.docs.is_empty() || !self.renders.is_empty();
        self.docs.clear();
        self.epoch = Some(epoch);
        self.renders.clear();
        self.status.clear();
        self.kept.clear();
        self.previous.clear();
        self.baseline.clear();
        self.agreed.clear();
        self.edits.clear();
        self.gestures.clear();
        self.requests.clear();
        self.revision += u64::from(had);
        had
    }

    /// Run ARA plug-ins, or not (*Play sources only*): off, every bound clip plays its
    /// source and no plug-in is loaded.
    pub fn set_enabled(&mut self, on: bool) {
        if self.enabled != on {
            self.enabled = on;
            if !on {
                self.docs.clear();
            }
            self.revision += 1;
        }
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    /// Run a stopped or failed plug-in again (the clip menu's *Retry*).
    pub fn retry(&mut self, plugin_id: &str) {
        self.failed.remove(&plugin_id.to_ascii_uppercase());
    }

    /// How long a render may run before the plug-in is stopped for the session.
    pub fn set_hang_limit(&mut self, limit: Duration) {
        self.hang_limit = limit;
    }

    /// How long a changed region rests before it is rendered again.
    pub fn set_debounce(&mut self, d: Duration) {
        self.debounce = d;
    }

    /// Wait (at most `patience`) until no render is queued, due or running, and take them in.
    /// Returns whether the renders changed. For tests and offline work.
    pub fn wait(&mut self, patience: Duration) -> bool {
        let before = self.revision;
        let start = Instant::now();
        while self.worker.as_ref().is_some_and(|w| w.pending() > 0) && start.elapsed() < patience {
            std::thread::sleep(Duration::from_millis(1));
        }
        self.take_renders();
        self.revision != before
    }

    /// Renders queued or running.
    pub fn pending(&self) -> usize {
        self.worker.as_ref().map_or(0, Worker::pending)
    }

    /// Whether every bound clip's render is the one its plug-in's state calls for: none queued,
    /// running or waiting for a drag to rest, none made and not yet taken in. What an offline
    /// render (export, freeze, bounce, a listen) waits for, so it never bakes a render older than
    /// the state the project holds. Takes in what is ready.
    pub fn settled(&mut self) -> bool {
        let before = self.revision;
        self.take_renders();
        before == self.revision
            && self.pending() == 0
            && self.edits.is_empty()
            && self.docs.values().all(|pd| {
                pd.instances
                    .values()
                    .all(|b| !b.rendering && b.due.is_none())
            })
    }

    /// How long the last render took.
    pub fn last_render(&self) -> Option<Duration> {
        self.last_render
    }

    /// A clip's render on its way: how far (0..=1).
    pub fn progress(&self, clip: ClipId) -> Option<f32> {
        let b = self.docs.values().find_map(|pd| pd.instances.get(&clip))?;
        (b.rendering || b.due.is_some()).then(|| b.progress.load(Ordering::Relaxed) as f32 / 1000.0)
    }

    /// The plug-in's edits since the last call, each a `SetClipAraState` with the new state
    /// and a merge key (the clip and its gesture): the window runs them as undo steps.
    pub fn take_edits(&mut self) -> Vec<(Command, (ClipId, u64))> {
        std::mem::take(&mut self.edits)
    }

    /// What the plug-ins asked of the transport since the last call (start, stop, move).
    pub fn take_playback_requests(&mut self) -> Vec<PlaybackRequest> {
        std::mem::take(&mut self.requests)
    }

    /// A clip let go of its plug-in: its binding as it was, with the plug-in's edits, to bind
    /// it again with them (*Restore previous edits*).
    pub fn previous_binding(&self, clip: ClipId) -> Option<&AraClip> {
        self.previous.get(&clip)
    }

    /// Open the plug-in's own editor for a bound clip, in a window owned by `owner`, showing
    /// the clip's region.
    pub fn open_editor(
        &mut self,
        clip: ClipId,
        title: &str,
        owner: Option<raw_window_handle::RawWindowHandle>,
    ) -> Result<(), HostError> {
        let pd = self
            .docs
            .values_mut()
            .find(|pd| pd.instances.contains_key(&clip))
            .ok_or(HostError::Unsupported(
                "the clip's ARA plug-in is not running",
            ))?;
        let doc = pd
            .doc
            .as_ref()
            .ok_or(HostError::Unsupported("no ARA document"))?;
        let b = pd.instances.get_mut(&clip).expect("found");
        // SAFETY: the instance's own editor view, bound to this document.
        unsafe { doc.select(clip, b.ext.editor_view, b.ext.editor_view_iface) };
        if let Some(w) = b.window.as_ref() {
            if !b.main.focus_editor() {
                w.focus();
            }
            return Ok(());
        }
        if !b.main.has_editor() {
            return Err(HostError::Unsupported("the plug-in has no editor"));
        }
        let window = crate::window::PluginWindow::open(title, owner, b.main.as_mut())?;
        b.window = Some(window);
        Ok(())
    }

    /// Whether a bound clip's plug-in editor is open.
    pub fn editor_open(&self, clip: ClipId) -> bool {
        self.docs
            .values()
            .any(|pd| pd.instances.get(&clip).is_some_and(|b| b.window.is_some()))
    }

    /// What each live bound clip plays in place of its source.
    pub fn renders(&self) -> &HashMap<ClipId, ClipRender> {
        &self.renders
    }

    /// The bytes the renders hold (memory the session's renders take).
    pub fn render_bytes(&self) -> usize {
        self.renders
            .values()
            .map(|r| {
                let at_engine = r.data.frames.len() * 4;
                let at_source = if Arc::ptr_eq(&r.data.frames, &r.at_source_rate) {
                    0
                } else {
                    r.at_source_rate.len() * 4
                };
                at_engine + at_source
            })
            .sum()
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn renders_made(&self) -> u64 {
        self.renders_made
    }

    pub fn status(&self, clip: ClipId) -> Option<&AraStatus> {
        self.status.get(&clip)
    }

    /// Whether an ARA plug-in runs inside Cantus now.
    pub fn active(&self) -> bool {
        !self.docs.is_empty()
    }

    /// `project` with every live bound clip's archive as the plug-in gives it now.
    pub fn archive_into(&self, project: &mut Project) {
        for t in &mut project.tracks {
            for c in &mut t.clips {
                let Some(ara) = c.audio.as_mut().and_then(|a| a.ara.as_mut()) else {
                    continue;
                };
                let Some(pd) = self.docs.get(&ara.plugin.id.to_ascii_uppercase()) else {
                    continue;
                };
                if let Some(a) = pd.doc.as_ref().and_then(|d| d.store(c.id)) {
                    ara.archive = Some(a);
                }
            }
        }
    }
}

impl Drop for AraSession {
    fn drop(&mut self) {
        // Instances (which wait for the render thread to let go of them) before the thread;
        // what a hung plug-in holds is never freed.
        self.docs.clear();
        for pd in std::mem::take(&mut self.abandoned) {
            std::mem::forget(pd);
        }
        self.worker = None;
        self.in_flight.clear();
        self.write_marker();
    }
}

pub(super) fn open_doc(
    plugin: &AraPlugin,
    find: &dyn Fn(&AraPlugin) -> Option<PluginInfo>,
    name: &str,
) -> Result<PluginDoc, AraStatus> {
    let info = find(plugin).ok_or(AraStatus::NotInstalled)?;
    let module = crate::vst3::ara::open(&info.path, Some(&info.name))
        .map_err(|e: HostError| AraStatus::Failed(e.to_string()))?;
    let doc = HostDocument::new(module.factory, name).map_err(AraStatus::Failed)?;
    Ok(PluginDoc {
        info,
        instances: BTreeMap::new(),
        doc: Some(doc),
        _module: module,
    })
}

pub(super) fn new_bound(loaded: crate::backend::Loaded, ext: crate::vst3::ara::Extension) -> Bound {
    Bound {
        main: loaded.main,
        audio: Arc::new(Mutex::new(loaded.audio)),
        generation: Arc::new(AtomicU64::new(0)),
        progress: Arc::new(AtomicU32::new(0)),
        rendering: false,
        due: None,
        range: None,
        inflight: None,
        ext,
        window: None,
    }
}

pub(super) type Live = BTreeSet<ClipId>;

//! The sample browser's state (ADR 0030): the content manager, the roots it lists
//! (installed packs, then the user's folders), which folders are open, the search index,
//! the audition voice's decoder, and a sample being dragged out of it. The view is
//! `views::samples_browser`; the window that installs packs is `views::samples_content`;
//! where a dragged sample lands is `samples_drop`.

use cantus_content::library::{self, Entry, Index, Indexer};
use cantus_content::{Manager, Manifest};
use cantus_core::SampleData;
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[path = "samples_drop.rs"]
pub mod drop;

/// What a root of the tree is.
#[derive(Clone, Debug, PartialEq)]
pub enum RootKind {
    /// An installed pack, by id.
    Pack(String),
    /// A folder the user added.
    User,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Root {
    pub label: String,
    pub path: PathBuf,
    pub kind: RootKind,
}

/// How loud an audition plays (about -3 dB): a reference, not a mix.
pub const AUDITION_GAIN: f32 = 0.7;

/// How many decoded auditions are kept, so clicking back and forth is instant.
const RECENT: usize = 24;

type Decoded = (PathBuf, Result<Arc<SampleData>, String>);

/// Decodes auditions on its own thread and remembers the last few.
pub struct Previewer {
    jobs: Sender<PathBuf>,
    done: Receiver<Decoded>,
    recent: VecDeque<(PathBuf, Arc<SampleData>)>,
    /// What was asked for and is still decoding.
    pub pending: Option<PathBuf>,
    /// What is playing: the file, when it started and how long it lasts.
    pub playing: Option<(PathBuf, Instant, Duration)>,
    /// The last decode error, for the status line.
    pub error: Option<String>,
}

impl Default for Previewer {
    fn default() -> Self {
        let (jobs, rx) = channel::<PathBuf>();
        let (tx, done) = channel::<Decoded>();
        let _ = std::thread::Builder::new()
            .name("cantus-audition".into())
            .spawn(move || {
                while let Ok(mut path) = rx.recv() {
                    // Only the newest request matters when clicks come quickly.
                    while let Ok(newer) = rx.try_recv() {
                        path = newer;
                    }
                    let r = cantus_engine::SampleCache::new().load(&path);
                    if tx.send((path, r)).is_err() {
                        break;
                    }
                }
            });
        Previewer {
            jobs,
            done,
            recent: VecDeque::new(),
            pending: None,
            playing: None,
            error: None,
        }
    }
}

impl Previewer {
    /// The decoded sample, if it is among the recent ones.
    pub fn cached(&self, path: &Path) -> Option<Arc<SampleData>> {
        self.recent
            .iter()
            .find(|(p, _)| p == path)
            .map(|(_, s)| s.clone())
    }

    /// Ask for `path` to be decoded; [`Previewer::poll`] returns it when it is.
    pub fn request(&mut self, path: &Path) {
        self.pending = Some(path.to_path_buf());
        self.error = None;
        let _ = self.jobs.send(path.to_path_buf());
    }

    /// A decoded sample that was asked for and is still wanted.
    pub fn poll(&mut self) -> Option<(PathBuf, Arc<SampleData>)> {
        let mut ready = None;
        while let Ok((path, r)) = self.done.try_recv() {
            match r {
                Ok(s) => {
                    self.remember(&path, s.clone());
                    if self.pending.as_deref() == Some(path.as_path()) {
                        self.pending = None;
                        ready = Some((path, s));
                    }
                }
                Err(e) => {
                    if self.pending.as_deref() == Some(path.as_path()) {
                        self.pending = None;
                        self.error = Some(e);
                    }
                }
            }
        }
        ready
    }

    fn remember(&mut self, path: &Path, s: Arc<SampleData>) {
        self.recent.retain(|(p, _)| p != path);
        self.recent.push_front((path.to_path_buf(), s));
        self.recent.truncate(RECENT);
    }

    pub fn is_playing(&self, path: &Path) -> bool {
        self.playing
            .as_ref()
            .is_some_and(|(p, at, len)| p == path && at.elapsed() < *len)
    }
}

/// A sample on its way out of the browser.
#[derive(Clone, Debug, PartialEq)]
pub struct Drag {
    pub path: PathBuf,
    /// Where the press began (a drag hit-tests its origin, never the pointer).
    pub from: egui::Pos2,
}

/// The content window's own state.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ContentWindow {
    pub open: bool,
    /// The pack whose details and licence are shown.
    pub selected: Option<String>,
    /// A removal waiting for its confirmation.
    pub confirm_remove: Option<String>,
}

/// One drawn row of the browser, for tests and the drag code.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub rect: egui::Rect,
    pub path: PathBuf,
    pub dir: bool,
    pub depth: usize,
}

/// What the browser drew last frame.
#[derive(Clone, Debug, PartialEq)]
pub struct BrowserGeometry {
    pub rect: egui::Rect,
    pub search: egui::Rect,
    pub rows: Vec<Row>,
    /// The one-click starter offer, when shown.
    pub starter: Option<egui::Rect>,
    /// Its row of buttons (*Download* first).
    pub starter_actions: Option<egui::Rect>,
    /// The *Content…* key and *Add folder* key in the header.
    pub content_key: egui::Rect,
    pub add_folder_key: egui::Rect,
}

impl BrowserGeometry {
    pub fn row_of(&self, path: &Path) -> Option<&Row> {
        self.rows.iter().find(|r| r.path == path)
    }
}

#[derive(Default)]
pub struct Samples {
    content: Option<Manager>,
    /// Where packs install: `<data dir>/Content` unless a test points elsewhere.
    pub content_root: Option<PathBuf>,
    /// A manifest to use instead of the bundled one (tests).
    pub manifest: Option<Manifest>,
    pub open: HashSet<PathBuf>,
    listings: HashMap<PathBuf, Vec<Entry>>,
    pub query: String,
    index: Option<Index>,
    indexer: Option<Indexer>,
    indexed_roots: Vec<PathBuf>,
    pub selected: Option<PathBuf>,
    pub preview: Previewer,
    pub drag: Option<Drag>,
    /// A sample released over the window, resolved against last frame's targets.
    pub released: Option<(PathBuf, egui::Pos2)>,
    pub geometry: Option<BrowserGeometry>,
    pub window: ContentWindow,
    /// A message for the browser's foot (installed, failed, dropped onto…).
    pub notice: Option<String>,
    /// Scroll offset of the tree, in points.
    pub scroll: f32,
    /// Sound packs the open project needs and does not find (round 26).
    pub missing: crate::views::samples_content::missing::State,
}

impl Samples {
    /// The content manager, made on first use.
    pub fn content(&mut self) -> &mut Manager {
        if self.content.is_none() {
            let root = self
                .content_root
                .clone()
                .unwrap_or_else(|| crate::app::data_dir().join("Content"));
            let manifest = self
                .manifest
                .clone()
                .unwrap_or_else(|| local_manifest(&root));
            self.content = Some(Manager::new(root, manifest));
        }
        self.content.as_mut().expect("just made")
    }

    /// Use this manager (tests: a local transport).
    pub fn set_content(&mut self, m: Manager) {
        self.content = Some(m);
        self.refresh();
    }

    /// The tree's roots: installed packs by name, then the user's folders.
    pub fn roots(&mut self, user: &[PathBuf]) -> Vec<Root> {
        let lang = crate::i18n::locale().code();
        let c = self.content();
        let mut out: Vec<Root> = c
            .installed()
            .iter()
            .map(|r| Root {
                label: r.name.get(lang).to_string(),
                path: c.pack_dir(&r.id),
                kind: RootKind::Pack(r.id.clone()),
            })
            .collect();
        out.sort_by(|a, b| library::natural(&a.label, &b.label));
        out.extend(user.iter().map(|p| {
            Root {
                label: p
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| p.to_string_lossy().into_owned()),
                path: p.clone(),
                kind: RootKind::User,
            }
        }));
        out
    }

    /// A folder's entries, read once and kept until [`Samples::refresh`].
    pub fn listing(&mut self, dir: &Path) -> &[Entry] {
        self.listings
            .entry(dir.to_path_buf())
            .or_insert_with(|| library::list(dir))
    }

    /// Forget listings and the index (a pack installed or removed, a folder added).
    pub fn refresh(&mut self) {
        self.listings.clear();
        self.index = None;
        self.indexer = None;
        self.indexed_roots.clear();
    }

    /// Search results for the current query, building the index on a worker thread the
    /// first time. `None` while it is being built.
    pub fn search(&mut self, roots: &[Root], max: usize) -> Option<Vec<PathBuf>> {
        let paths: Vec<PathBuf> = roots.iter().map(|r| r.path.clone()).collect();
        if paths != self.indexed_roots {
            self.indexed_roots = paths.clone();
            self.index = None;
            self.indexer = Some(Indexer::spawn(paths));
        }
        if let Some(i) = self.indexer.as_ref().and_then(|i| i.poll()) {
            self.index = Some(i);
            self.indexer = None;
        }
        let idx = self.index.as_ref()?;
        Some(
            idx.search(&self.query, max)
                .into_iter()
                .map(|h| h.path.clone())
                .collect(),
        )
    }

    /// Whether the index is still being built (the view asks for another frame).
    pub fn indexing(&self) -> bool {
        self.indexer.is_some()
    }
}

/// The manifest: a newer one saved beside the content (`manifest.json`, when an update
/// of the list was fetched or copied there) wins over the one built in.
pub fn local_manifest(root: &Path) -> Manifest {
    let bundled = Manifest::bundled();
    std::fs::read_to_string(root.join("manifest.json"))
        .ok()
        .and_then(|t| Manifest::parse(&t).ok())
        .filter(|m| m.updated > bundled.updated)
        .unwrap_or(bundled)
}

impl crate::app::CantusApp {
    /// Play `path` through the audition voice (decoding it first if it is not recent).
    pub fn audition_sample(&mut self, path: &Path) {
        if let Some(s) = self.samples.preview.cached(path) {
            self.start_audition(path, s);
        } else {
            self.samples.preview.request(path);
        }
    }

    fn start_audition(&mut self, path: &Path, s: Arc<SampleData>) {
        let len = Duration::from_secs_f64(s.len_frames() as f64 / s.rate.max(1) as f64);
        self.samples.preview.playing = Some((path.to_path_buf(), Instant::now(), len));
        self.engine.preview(Some((s, AUDITION_GAIN)));
    }

    /// Stop the audition (a fade, in the engine).
    pub fn stop_audition(&mut self) {
        self.samples.preview.playing = None;
        self.samples.preview.pending = None;
        self.engine.preview(None);
    }

    /// Once a frame: start auditions that finished decoding, take what installs
    /// reported, and resolve a sample dropped last frame.
    pub fn poll_samples(&mut self, ctx: &egui::Context) {
        if let Some((path, s)) = self.samples.preview.poll() {
            self.start_audition(&path, s);
        }
        if self.samples.preview.pending.is_some() || self.samples.indexing() {
            ctx.request_repaint_after(Duration::from_millis(30));
        }
        if self.samples.content.is_some() {
            let finished = self.samples.content().poll();
            if self.samples.content().busy() {
                ctx.request_repaint_after(Duration::from_millis(100));
            }
            for f in finished {
                let name = self
                    .samples
                    .content()
                    .manifest()
                    .pack(&f.id)
                    .map(|p| p.name.get(crate::i18n::locale().code()).to_string())
                    .unwrap_or(f.id.clone());
                self.samples.notice = Some(match &f.result {
                    Ok(_) => crate::i18n::tf("samples.notice.installed", &[("pack", &name)]),
                    // Not enough room: how much it needs and how much there is, in words.
                    Err(cantus_content::install::InstallError::NoSpace { need, free }) => {
                        crate::i18n::tf(
                            "samples.notice.no_space",
                            &[
                                ("pack", &name),
                                ("need", &crate::views::samples_content::size(*need)),
                                ("free", &crate::views::samples_content::size(*free)),
                            ],
                        )
                    }
                    Err(e) => {
                        crate::i18n::tf("samples.notice.failed", &[("pack", &name), ("why", &e)])
                    }
                });
                self.samples.refresh();
            }
        }
        drop::resolve(self, ctx);
    }

    /// Add a folder of the user's to the browser (and the settings).
    pub fn add_sample_folder(&mut self, dir: PathBuf) {
        if !self.settings.sample_folders.contains(&dir) {
            self.settings.sample_folders.push(dir.clone());
            self.store_settings();
        }
        self.samples.open.insert(dir);
        self.samples.refresh();
    }

    pub fn remove_sample_folder(&mut self, dir: &Path) {
        self.settings.sample_folders.retain(|d| d != dir);
        self.store_settings();
        self.samples.refresh();
    }
}

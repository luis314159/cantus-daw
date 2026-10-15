//! The hosted plug-ins of an open project, on the main thread (ADR 0019).
//!
//! [`PluginHost`] is what the application holds. Each frame it is given the project and
//! the engine handle, and it:
//! - loads the plug-in of every hosted slot that has none yet, loads the project's state
//!   into it, and delivers its audio half to the engine (`EngineHandle::add_unit`) before
//!   the snapshot that holds the slot — a slot whose plug-in is not on this machine gets
//!   nothing, and the engine runs it as a wire;
//! - unloads the plug-ins whose slot left the project, keeping their state for an undo
//!   that brings the slot back;
//! - loads a state the project changed (undo, a preset, A/B) into the plug-in;
//! - services the plug-ins' requests (`MainThread::idle`), restarts them when they ask
//!   (their latency changed), and turns what their own windows did into commands — one
//!   undo step per gesture;
//! - gives the project to save with every plug-in's current state in it.

use crate::backend::{
    AudioSetup, AudioThread, HostError, MainThread, Notice, ParamChange, ParamEvent, ParamInfo,
    ParamOut, PluginInfo,
};
use crate::db::PluginDb;
use crate::scan::{ScanReport, Scanner};
use crate::unit::HostedUnit;
use crate::window::PluginWindow;
use crate::{backend_for, from_project_format, to_project_format};
use cantus_core::{
    Command, CommandBus, EffectId, EffectKind, EffectSlot, EffectSpec, ExternalPlugin, MergeKey,
    Origin, PluginState, Project, StripId,
};
use cantus_engine::{EffectUnit, EngineHandle, FxParamEvent, Wire, MAX_BLOCK_FRAMES};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{mpsc, Arc};

#[path = "session_inst.rs"]
mod inst;
#[path = "session_inst_idle.rs"]
mod inst_idle;

pub use inst::HostedInst;

/// Something the application should know about.
#[derive(Clone, Debug, PartialEq)]
pub enum HostNotice {
    /// A slot's plug-in could not be loaded; the slot is a wire.
    LoadFailed {
        effect: EffectId,
        name: String,
        error: String,
    },
    /// A plug-in could not restart; its slot is a wire until the project is reopened.
    RestartFailed { effect: EffectId, error: String },
    /// A scan finished.
    ScanDone(ScanReport),
    /// The user closed a plug-in's window.
    WindowClosed(EffectId),
    /// A track's hosted instrument could not be loaded (ADR 0028); the track plays
    /// silence until it is installed and the project rescanned or reopened.
    InstrumentLoadFailed {
        track: cantus_core::TrackId,
        name: String,
        error: String,
    },
    /// A track's hosted instrument could not restart; it is silent until reopened.
    InstrumentRestartFailed {
        track: cantus_core::TrackId,
        error: String,
    },
    /// The user closed a hosted instrument's window.
    InstrumentWindowClosed(cantus_core::TrackId),
}

/// One loaded plug-in in a slot.
pub struct Hosted {
    pub strip: StripId,
    pub info: PluginInfo,
    pub params: Vec<ParamInfo>,
    pub(crate) main: Box<dyn MainThread>,
    /// The audio half, until it is delivered to the engine.
    audio: Option<Box<dyn AudioThread>>,
    /// The project's state this plug-in was last given (or reported): a different one in
    /// the project is loaded into it.
    synced_state: PluginState,
    /// The project's values last delivered on the main thread.
    flushed: BTreeMap<u32, f64>,
    generation: Arc<AtomicU64>,
    restarting: bool,
    /// Parameters the plug-in's own gesture is moving.
    gestures: Vec<u32>,
    pub(crate) window: Option<PluginWindow>,
}

impl Hosted {
    /// The plug-in's own text for a value.
    pub fn param_text(&self, id: u32, value: f64) -> Option<String> {
        self.main.param_text(id, value)
    }

    /// The value the plug-in holds now.
    pub fn param_value(&self, id: u32) -> Option<f64> {
        self.main.param_value(id)
    }

    pub fn latency(&self) -> usize {
        self.main.latency()
    }

    pub fn has_editor(&self) -> bool {
        self.main.has_editor()
    }

    pub fn window_open(&self) -> bool {
        self.window.is_some()
    }

    /// Whether its window belongs to a plug-in that floats its own (round 27, ADR 0051).
    pub fn window_floating(&self) -> bool {
        self.window.as_ref().is_some_and(|w| w.is_floating())
    }

    /// Whether its window is open and in front, nothing over it (round 27, ADR 0051).
    pub fn window_frontmost(&self) -> bool {
        self.window.as_ref().is_some_and(|w| w.is_frontmost())
    }

    /// The plug-in window's native handle, when one is open.
    pub fn window_handle(&self) -> Option<isize> {
        self.window.as_ref().and_then(|w| w.handle())
    }
}

pub struct PluginHost {
    pub db: PluginDb,
    /// Where the database is kept; `None` keeps it in memory (tests).
    pub db_path: Option<PathBuf>,
    pub scanner: Scanner,
    setup: AudioSetup,
    slots: HashMap<EffectId, Hosted>,
    /// State of plug-ins unloaded because their slot left the project.
    parked: HashMap<EffectId, PluginState>,
    /// Hosted slots whose plug-in could not be loaded, and why.
    missing: HashMap<EffectId, String>,
    scan_job: Option<mpsc::Receiver<(PluginDb, ScanReport)>>,
    /// Instances loaded for an offline render, kept until it has finished.
    render_mains: Vec<Box<dyn MainThread>>,
    /// The application's main window: plug-in windows are owned by it.
    pub owner_window: Option<raw_window_handle::RawWindowHandle>,
    /// Hosted instruments (ADR 0028), by track.
    instruments: HashMap<cantus_core::TrackId, HostedInst>,
    /// State of instruments unloaded because their track left them, by track and plug-in.
    inst_parked: HashMap<(cantus_core::TrackId, u64), PluginState>,
    /// Tracks whose hosted instrument could not be loaded: the plug-in's key, and why.
    inst_missing: HashMap<cantus_core::TrackId, (u64, String)>,
    /// Every parameter command a plug-in's own window ran since the application last took
    /// them ([`PluginHost::take_tweaks`]): what it records as automation and calls *last
    /// tweaked* (ADR 0057) — all of them, not only a frame's last (review 1).
    tweaks: Vec<Command>,
}

/// The most window tweaks kept between two takes (an application that never takes them
/// does not grow them for ever).
const TWEAKS: usize = 4096;

impl std::fmt::Debug for PluginHost {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PluginHost")
            .field("loaded", &self.slots.len())
            .field("missing", &self.missing.len())
            .field("instruments", &self.instruments.len())
            .finish()
    }
}

fn changes(params: &BTreeMap<u32, f64>) -> Vec<ParamChange> {
    params
        .iter()
        .map(|(id, v)| ParamChange {
            offset: 0,
            id: *id,
            value: *v,
        })
        .collect()
}

fn to_fx(e: ParamEvent) -> FxParamEvent {
    match e {
        ParamEvent::Begin(id) => FxParamEvent::Begin(id),
        ParamEvent::Value(id, v) => FxParamEvent::Value(id, v),
        ParamEvent::End(id) => FxParamEvent::End(id),
    }
}

/// A plug-in just loaded that asked for a restart while it was set up (a state with
/// another latency, say) is restarted at once, before its audio half goes anywhere: the
/// half is dropped unused, and the instance reactivated — or, when the backend cannot do
/// that, replaced by a new one with the same state and values. `None` if neither works.
fn settle_new(
    main: &mut Box<dyn MainThread>,
    audio: Box<dyn AudioThread>,
    info: &PluginInfo,
    setup: AudioSetup,
    values: &BTreeMap<u32, f64>,
) -> Option<Box<dyn AudioThread>> {
    let wants = main
        .idle()
        .iter()
        .any(|n| matches!(n, Notice::RestartNeeded | Notice::LatencyChanged(_)));
    if !wants {
        return Some(audio);
    }
    drop(audio);
    match main.restart() {
        Ok(Some(audio)) => Some(audio),
        _ => {
            let state = main.save_state().ok();
            let loaded = backend_for(info.format)?.load(info, setup).ok()?;
            let mut fresh = loaded.main;
            if let Some(s) = &state {
                let _ = fresh.load_state(s);
            }
            let mut out = ParamOut::new();
            fresh.flush(&changes(values), &mut out);
            *main = fresh;
            Some(loaded.audio)
        }
    }
}

/// The project's slot for an effect id, with its strip.
fn find_slot(project: &Project, effect: EffectId) -> Option<(StripId, &EffectSlot)> {
    // The strips' inserts and the master's (ADR 0026).
    project.all_effects().find(|(_, e)| e.id == effect)
}

impl PluginHost {
    /// A host whose database is read from (and written to) `db_path`.
    pub fn new(db_path: Option<PathBuf>, sample_rate: f64) -> PluginHost {
        let db = db_path.as_deref().map(PluginDb::load).unwrap_or_default();
        PluginHost {
            db,
            db_path,
            scanner: Scanner::default(),
            setup: AudioSetup {
                sample_rate,
                max_block: MAX_BLOCK_FRAMES,
            },
            slots: HashMap::new(),
            parked: HashMap::new(),
            missing: HashMap::new(),
            scan_job: None,
            render_mains: Vec::new(),
            owner_window: None,
            instruments: HashMap::new(),
            inst_parked: HashMap::new(),
            inst_missing: HashMap::new(),
            tweaks: Vec::new(),
        }
    }

    pub fn setup(&self) -> AudioSetup {
        self.setup
    }

    /// Plug-ins load at this rate from now on (the audio device changed).
    pub fn set_sample_rate(&mut self, sample_rate: f64) {
        self.setup.sample_rate = sample_rate;
    }

    pub fn hosted(&self, effect: EffectId) -> Option<&Hosted> {
        self.slots.get(&effect)
    }

    /// Why a hosted slot has no plug-in, if it has none.
    pub fn missing(&self, effect: EffectId) -> Option<&str> {
        self.missing.get(&effect).map(String::as_str)
    }

    /// Loaded plug-ins: inserts and instruments.
    pub fn loaded_count(&self) -> usize {
        self.slots.len() + self.instruments.len()
    }

    /// Keep what a loaded plug-in says it is, over what its scan guessed.
    fn learn(&mut self, main: &dyn MainThread) {
        if self.db.learn(main.info()) {
            self.save_db();
        }
    }

    fn save_db(&self) {
        if let Some(path) = &self.db_path {
            if let Err(e) = self.db.save(path) {
                log::warn!("plug-in database: {e}");
            }
        }
    }

    // ---- scanning ------------------------------------------------------------------

    /// Scan now, on this thread; for tests and tools. The application uses
    /// [`Self::start_scan`].
    pub fn scan_now(&mut self, force: bool) -> ScanReport {
        let backends = crate::backends();
        let report = crate::scan::scan_all(&mut self.db, &backends, &self.scanner, force);
        self.save_db();
        self.missing.clear();
        self.inst_missing.clear();
        report
    }

    /// Scan in the background; [`Self::idle`] picks the result up.
    pub fn start_scan(&mut self, force: bool) {
        if self.scan_job.is_some() {
            return;
        }
        let mut db = self.db.clone();
        let scanner = self.scanner.clone();
        let (tx, rx) = mpsc::channel();
        let spawned = std::thread::Builder::new()
            .name("cantus-plugin-scan".into())
            .spawn(move || {
                let backends = crate::backends();
                let report = crate::scan::scan_all(&mut db, &backends, &scanner, force);
                let _ = tx.send((db, report));
            });
        if spawned.is_ok() {
            self.scan_job = Some(rx);
        }
    }

    pub fn scanning(&self) -> bool {
        self.scan_job.is_some()
    }

    /// Forget a file (blocked or not) and scan again, so it is read afresh.
    pub fn rescan_file(&mut self, path: &std::path::Path) {
        self.db.forget(path);
        self.save_db();
        self.start_scan(false);
    }

    pub fn add_folder(&mut self, folder: PathBuf) {
        if !self.db.folders.contains(&folder) {
            self.db.folders.push(folder);
            self.save_db();
        }
    }

    pub fn remove_folder(&mut self, folder: &std::path::Path) {
        self.db.folders.retain(|f| f != folder);
        self.save_db();
    }
}

impl PluginHost {
    // ---- slots ---------------------------------------------------------------------

    /// A new insert slot holding `info`, ready for `Command::InsertEffect`. The plug-in is
    /// loaded once, here, to read its parameters and its state as it starts; that instance
    /// then serves the slot.
    pub fn make_slot(
        &mut self,
        bus: &mut CommandBus,
        strip: StripId,
        info: &PluginInfo,
    ) -> Result<EffectSlot, HostError> {
        let backend = backend_for(info.format).ok_or(HostError::Unsupported("format"))?;
        let loaded = backend.load(info, self.setup)?;
        let main = loaded.main;
        self.learn(main.as_ref());
        let params = main.params();
        let values: BTreeMap<u32, f64> = params
            .iter()
            .filter(|p| !p.read_only)
            .filter_map(|p| main.param_value(p.id).map(|v| (p.id, v)))
            .collect();
        let mut main = main;
        let state = main.save_state().map(PluginState::new).unwrap_or_default();
        let mut plugin = ExternalPlugin::new(
            to_project_format(info.format),
            info.id.clone(),
            info.name.clone(),
        );
        plugin.vendor = info.vendor.clone();
        plugin.sidechain = info.sidechain;
        plugin.state = state.clone();
        plugin.params = values.clone();
        let mut slot = bus.new_effect(EffectKind::External);
        slot.spec = EffectSpec::external(plugin);
        self.slots.insert(
            slot.id,
            Hosted {
                strip,
                info: info.clone(),
                params,
                main,
                audio: Some(loaded.audio),
                synced_state: state,
                flushed: values,
                generation: Arc::new(AtomicU64::new(0)),
                restarting: false,
                gestures: Vec::new(),
                window: None,
            },
        );
        Ok(slot)
    }

    /// Load the plug-in of a slot the host has not seen: from the database by format and
    /// id, with the slot's state (or the one it had when its slot was last removed) and
    /// the project's values.
    fn load_for(
        &mut self,
        strip: StripId,
        slot: &EffectSlot,
        plugin: &ExternalPlugin,
    ) -> Result<Hosted, String> {
        let format = from_project_format(plugin.format);
        let info = self
            .db
            .find(format, &plugin.id)
            .cloned()
            .ok_or_else(|| "not installed on this computer".to_string())?;
        let backend = backend_for(format).ok_or_else(|| "format not hosted".to_string())?;
        let loaded = backend.load(&info, self.setup).map_err(|e| e.to_string())?;
        let mut main = loaded.main;
        let state = self
            .parked
            .remove(&slot.id)
            .unwrap_or_else(|| plugin.state.clone());
        if !state.is_empty() {
            if let Err(e) = main.load_state(state.bytes()) {
                log::warn!("{}: {e}", info.name);
            }
        }
        let mut out = ParamOut::new();
        main.flush(&changes(&plugin.params), &mut out);
        let audio = settle_new(&mut main, loaded.audio, &info, self.setup, &plugin.params)
            .ok_or_else(|| "it could not restart after its state was loaded".to_string())?;
        self.learn(main.as_ref());
        let params = main.params();
        Ok(Hosted {
            strip,
            info,
            params,
            main,
            audio: Some(audio),
            synced_state: plugin.state.clone(),
            flushed: plugin.params.clone(),
            generation: Arc::new(AtomicU64::new(0)),
            restarting: false,
            gestures: Vec::new(),
            window: None,
        })
    }

    fn make_unit(&self, h: &Hosted, audio: Box<dyn AudioThread>, slot: &EffectSlot) -> EffectUnit {
        let unit = HostedUnit::new(
            audio,
            &h.params,
            |id| h.main.param_value(id),
            h.main.latency(),
            self.setup.sample_rate,
            h.generation.clone(),
        );
        EffectUnit::from_fx(slot, Box::new(unit))
    }

    /// Before each snapshot: load what is new, deliver it, unload what left, load changed
    /// states. See the module documentation.
    pub fn sync(&mut self, project: &Project, engine: &mut EngineHandle) -> Vec<HostNotice> {
        let mut notices = Vec::new();
        let mut seen = HashSet::new();
        // The strips' inserts and the master's (ADR 0026).
        for (host, slot) in project.all_effects() {
            let Some(plugin) = slot.spec.external.as_deref() else {
                continue;
            };
            seen.insert(slot.id);
            if self.missing.contains_key(&slot.id) {
                continue;
            }
            if !self.slots.contains_key(&slot.id) {
                match self.load_for(host, slot, plugin) {
                    Ok(h) => {
                        self.slots.insert(slot.id, h);
                    }
                    Err(error) => {
                        log::warn!("plug-in {}: {error}", plugin.name);
                        self.missing.insert(slot.id, error.clone());
                        notices.push(HostNotice::LoadFailed {
                            effect: slot.id,
                            name: plugin.name.clone(),
                            error,
                        });
                        continue;
                    }
                }
            }
            let Some(mut h) = self.slots.remove(&slot.id) else {
                continue;
            };
            h.strip = host;
            // A state the project changed: into the plug-in.
            if plugin.state != h.synced_state {
                if !plugin.state.is_empty() {
                    if let Err(e) = h.main.load_state(plugin.state.bytes()) {
                        log::warn!("{}: {e}", h.info.name);
                    }
                }
                h.synced_state = plugin.state.clone();
                h.generation.fetch_add(1, Ordering::AcqRel);
            }
            // Values the project changed, delivered here while nothing processes (no
            // audio device, or not started yet); processing, they go with the next
            // block instead.
            if plugin.params != h.flushed {
                let delta: BTreeMap<u32, f64> = plugin
                    .params
                    .iter()
                    .filter(|(id, v)| h.flushed.get(id) != Some(v))
                    .map(|(id, v)| (*id, *v))
                    .collect();
                let mut out = ParamOut::new();
                h.main.flush(&changes(&delta), &mut out);
                h.flushed = plugin.params.clone();
            }
            if let Some(audio) = h.audio.take() {
                let unit = self.make_unit(&h, audio, slot);
                if engine.knows_effect(slot.id) {
                    engine.replace_unit(unit);
                } else {
                    engine.add_unit(unit);
                }
            }
            self.slots.insert(slot.id, h);
        }
        let gone: Vec<EffectId> = self
            .slots
            .keys()
            .filter(|id| !seen.contains(id))
            .copied()
            .collect();
        for id in gone {
            self.unload(id);
        }
        self.missing.retain(|id, _| seen.contains(id));
        self.sync_instruments(project, engine, &mut notices);
        notices
    }

    /// Unload a plug-in, keeping its state for a slot that comes back.
    fn unload(&mut self, id: EffectId) {
        if let Some(mut h) = self.slots.remove(&id) {
            if let Some(w) = h.window.take() {
                h.main.close_editor();
                // The window goes after the editor closed (on Windows it destroys the
                // native window; elsewhere it is a placeholder with nothing to drop).
                #[cfg_attr(not(windows), allow(clippy::drop_non_drop))]
                drop(w);
            }
            if let Ok(s) = h.main.save_state() {
                self.parked.insert(id, PluginState::new(s));
            }
        }
    }

    /// The engine was replaced (another audio device, another rate): every plug-in is
    /// loaded again at `sample_rate`, with its state, at the next [`Self::sync`].
    pub fn reload_all(&mut self, sample_rate: f64) {
        self.setup.sample_rate = sample_rate;
        let ids: Vec<EffectId> = self.slots.keys().copied().collect();
        for id in ids {
            self.unload(id);
        }
        self.missing.clear();
        let tracks: Vec<cantus_core::TrackId> = self.instruments.keys().copied().collect();
        for track in tracks {
            self.unload_instrument(track);
        }
        self.inst_missing.clear();
    }
}

impl PluginHost {
    // ---- every frame ---------------------------------------------------------------

    /// Once per UI frame while anything is hosted: scans finishing, the plug-ins' requests,
    /// restarts, and what their windows did (from the engine, and from a flush) recorded
    /// as commands.
    pub fn idle(&mut self, bus: &mut CommandBus, engine: &mut EngineHandle) -> Vec<HostNotice> {
        crate::clap::collect_garbage();
        let mut notices = Vec::new();
        if let Some(rx) = &self.scan_job {
            match rx.try_recv() {
                Ok((db, report)) => {
                    // Folders the user changed while the scan ran are kept.
                    let folders = std::mem::take(&mut self.db.folders);
                    self.db = db;
                    self.db.folders = folders;
                    self.save_db();
                    self.missing.clear();
                    self.inst_missing.clear();
                    self.scan_job = None;
                    notices.push(HostNotice::ScanDone(report));
                }
                Err(mpsc::TryRecvError::Disconnected) => self.scan_job = None,
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        for (effect, event) in engine.take_fx_params() {
            self.record(bus, effect, event);
        }
        let ids: Vec<EffectId> = self.slots.keys().copied().collect();
        for id in ids {
            // What the plug-in reported outside processing (it asked for a flush).
            let mut out = ParamOut::new();
            if let Some(h) = self.slots.get_mut(&id) {
                h.main.flush(&[], &mut out);
            }
            for e in out.events() {
                self.record(bus, id, to_fx(*e));
            }
            let asked = match self.slots.get_mut(&id) {
                Some(h) => h.main.idle(),
                None => continue,
            };
            for notice in asked {
                match notice {
                    Notice::RestartNeeded | Notice::LatencyChanged(_) => {
                        self.begin_restart(bus.project(), engine, id)
                    }
                    Notice::ParamsChanged => {
                        if let Some(h) = self.slots.get_mut(&id) {
                            h.params = h.main.params();
                        }
                        self.capture_changes(bus, id);
                    }
                    Notice::StateDirty => self.capture_changes(bus, id),
                    Notice::EditorResized(size) => {
                        if let Some(h) = self.slots.get_mut(&id) {
                            if let Some(w) = h.window.as_mut() {
                                w.fit(size);
                            }
                        }
                    }
                    Notice::EditorClosed => {
                        if self.close_window(id) {
                            notices.push(HostNotice::WindowClosed(id));
                        }
                    }
                }
            }
            if let Some(n) = self.continue_restart(bus.project(), engine, id) {
                notices.push(n);
            }
            // The user closed the window with its own close box.
            let closed = self
                .slots
                .get(&id)
                .and_then(|h| h.window.as_ref())
                .is_some_and(|w| w.closed());
            if closed && self.close_window(id) {
                notices.push(HostNotice::WindowClosed(id));
            }
            // The user resized the window: offer the size to the editor.
            if let Some(h) = self.slots.get_mut(&id) {
                if let Some(size) = h.window.as_mut().and_then(|w| w.take_resize()) {
                    if let Some(taken) = h.main.resize_editor(size) {
                        if let Some(w) = h.window.as_mut() {
                            w.fit(taken);
                        }
                    }
                }
            }
        }
        self.idle_instruments(bus, engine, &mut notices);
        notices
    }

    /// One thing a plug-in's window did, as a command: a value inside a gesture merges
    /// into one undo step with the rest of the gesture.
    /// The parameter commands plug-in windows ran since the last take, oldest first.
    pub fn take_tweaks(&mut self) -> Vec<Command> {
        std::mem::take(&mut self.tweaks)
    }

    pub(crate) fn tweaked(&mut self, cmd: Command) {
        if self.tweaks.len() < TWEAKS {
            self.tweaks.push(cmd);
        }
    }

    pub fn record(&mut self, bus: &mut CommandBus, effect: EffectId, event: FxParamEvent) {
        let Some(h) = self.slots.get_mut(&effect) else {
            return;
        };
        let strip = h.strip;
        match event {
            FxParamEvent::Begin(id) => {
                if !h.gestures.contains(&id) {
                    h.gestures.push(id);
                }
            }
            FxParamEvent::Value(id, v) => {
                // The plug-in holds it already: nothing to flush back.
                h.flushed.insert(id, v);
                let in_gesture = h.gestures.contains(&id);
                let current = find_slot(bus.project(), effect)
                    .and_then(|(_, s)| s.spec.external.as_deref())
                    .and_then(|p| p.params.get(&id).copied());
                if current.map(f64::to_bits) == Some(v.to_bits()) {
                    return; // no change, no step
                }
                let cmd = Command::SetPluginParam {
                    strip,
                    effect,
                    id,
                    value: Some(v),
                };
                let result = if in_gesture {
                    bus.execute_merging(
                        cmd.clone(),
                        Origin::User,
                        MergeKey::of(&("plugin", effect.0, id)),
                    )
                } else {
                    bus.execute(cmd.clone(), Origin::User)
                };
                match result {
                    Ok(_) => self.tweaked(cmd),
                    Err(e) => log::warn!("plug-in parameter {id}: {e}"),
                }
            }
            FxParamEvent::End(id) => {
                let was = h.gestures.len();
                h.gestures.retain(|g| *g != id);
                if h.gestures.len() != was {
                    bus.end_gesture();
                }
            }
        }
    }

    /// The plug-in's state and values, read now.
    pub fn capture(&mut self, effect: EffectId) -> Option<(PluginState, BTreeMap<u32, f64>)> {
        let h = self.slots.get_mut(&effect)?;
        let state = h
            .main
            .save_state()
            .map(PluginState::new)
            .unwrap_or_default();
        let params = h
            .params
            .iter()
            .filter(|p| !p.read_only)
            .filter_map(|p| h.main.param_value(p.id).map(|v| (p.id, v)))
            .collect();
        Some((state, params))
    }

    /// The plug-in changed its values by itself (a preset from its own browser): record
    /// its state and values as one step. A state change that moved no value (a sample it
    /// loaded) is kept for the save, which asks the plug-in for its state.
    fn capture_changes(&mut self, bus: &mut CommandBus, effect: EffectId) {
        let Some((strip, same)) = find_slot(bus.project(), effect).map(|(s, slot)| {
            let now = slot.spec.external.as_deref().map(|p| p.params.clone());
            (s, now)
        }) else {
            return;
        };
        let Some((state, params)) = self.capture(effect) else {
            return;
        };
        if same.as_ref() == Some(&params) {
            return;
        }
        if let Some(h) = self.slots.get_mut(&effect) {
            h.synced_state = state.clone();
            h.flushed = params.clone();
        }
        let cmd = Command::SetPluginState {
            strip,
            effect,
            state,
            params,
        };
        if let Err(e) = bus.execute(cmd, Origin::User) {
            log::warn!("plug-in state: {e}");
        }
    }

    // ---- restarts ------------------------------------------------------------------

    /// Take the plug-in's unit out of the engine (a wire stands in) so it can be
    /// deactivated and activated again.
    fn begin_restart(&mut self, project: &Project, engine: &mut EngineHandle, effect: EffectId) {
        let Some(h) = self.slots.get_mut(&effect) else {
            return;
        };
        if h.restarting {
            return;
        }
        h.restarting = true;
        if h.audio.take().is_some() {
            return; // never delivered: dropped now, so nothing waits on it
        }
        if let Some((_, slot)) = find_slot(project, effect) {
            engine.replace_unit(EffectUnit::from_fx(slot, Box::new(Wire)));
        }
    }

    fn continue_restart(
        &mut self,
        project: &Project,
        engine: &mut EngineHandle,
        effect: EffectId,
    ) -> Option<HostNotice> {
        let (_, slot) = find_slot(project, effect)?;
        let setup = self.setup;
        let h = self.slots.get_mut(&effect)?;
        if !h.restarting {
            return None;
        }
        let audio = match h.main.restart() {
            Ok(None) => return None, // its audio half is still on its way back
            Ok(Some(audio)) => audio,
            Err(HostError::Unsupported(_)) => {
                // The backend cannot restart in place: a new instance, with the state.
                let state = h.main.save_state().ok();
                let info = h.info.clone();
                let loaded = backend_for(info.format)
                    .ok_or(HostError::Unsupported("format"))
                    .and_then(|b| b.load(&info, setup));
                match loaded {
                    Ok(l) => {
                        let mut main = l.main;
                        if let Some(s) = &state {
                            let _ = main.load_state(s);
                        }
                        let mut out = ParamOut::new();
                        main.flush(&changes(&h.flushed), &mut out);
                        h.params = main.params();
                        h.main = main;
                        l.audio
                    }
                    Err(e) => {
                        h.restarting = false;
                        return Some(HostNotice::RestartFailed {
                            effect,
                            error: e.to_string(),
                        });
                    }
                }
            }
            Err(e) => {
                h.restarting = false;
                return Some(HostNotice::RestartFailed {
                    effect,
                    error: e.to_string(),
                });
            }
        };
        h.restarting = false;
        h.generation.fetch_add(1, Ordering::AcqRel);
        let h = self.slots.get(&effect)?;
        let unit = self.make_unit(h, audio, slot);
        if engine.knows_effect(effect) {
            engine.replace_unit(unit);
        } else {
            engine.add_unit(unit);
        }
        None
    }

    /// Whether a plug-in is between being taken out of the engine and put back.
    pub fn restarting(&self, effect: EffectId) -> bool {
        self.slots.get(&effect).is_some_and(|h| h.restarting)
    }

    // ---- saving and rendering ------------------------------------------------------

    /// `project` as it should be saved: every loaded plug-in's state as it is now. A slot
    /// whose plug-in is missing keeps what the project holds, untouched.
    pub fn project_for_save(&mut self, project: &Project) -> Project {
        let mut out = project.clone();
        for slot in out.all_effects_mut() {
            let Some(h) = self.slots.get_mut(&slot.id) else {
                continue;
            };
            let Some(plugin) = slot.spec.external.as_deref_mut() else {
                continue;
            };
            match h.main.save_state() {
                Ok(bytes) => plugin.state = PluginState::new(bytes),
                Err(e) => log::warn!("{}: {e}", h.info.name),
            }
        }
        self.instruments_for_save(&mut out);
        out
    }

    /// Fresh instances of every loaded plug-in in `project` for an offline render at
    /// `sample_rate`, each with the live one's state: the units to hand to
    /// `cantus_engine::render_with_units`. Their main halves stay here until
    /// [`Self::finish_render`].
    pub fn render_units(&mut self, project: &Project, sample_rate: f64) -> Vec<EffectUnit> {
        let setup = AudioSetup {
            sample_rate,
            max_block: MAX_BLOCK_FRAMES,
        };
        let mut units = Vec::new();
        for (_, slot) in project.all_effects() {
            let Some(plugin) = slot.spec.external.as_deref() else {
                continue;
            };
            let Some(h) = self.slots.get_mut(&slot.id) else {
                continue;
            };
            let state = h.main.save_state().ok();
            let info = h.info.clone();
            let Some(Ok(loaded)) = backend_for(info.format).map(|b| b.load(&info, setup)) else {
                continue;
            };
            let mut main = loaded.main;
            if let Some(s) = &state {
                let _ = main.load_state(s);
            }
            let mut out = ParamOut::new();
            main.flush(&changes(&plugin.params), &mut out);
            let Some(audio) = settle_new(&mut main, loaded.audio, &info, setup, &plugin.params)
            else {
                continue;
            };
            let params = main.params();
            let unit = HostedUnit::new(
                audio,
                &params,
                |id| main.param_value(id),
                main.latency(),
                sample_rate,
                Arc::new(AtomicU64::new(0)),
            );
            units.push(EffectUnit::from_fx(slot, Box::new(unit)));
            self.render_mains.push(main);
        }
        units
    }

    /// The render that [`Self::render_units`] served has finished: its instances go.
    pub fn finish_render(&mut self) {
        self.render_mains.clear();
    }

    // ---- windows -------------------------------------------------------------------

    /// Open the plug-in's own editor in a window of its own, owned by `owner` (the main
    /// window) so it stays above it.
    pub fn open_window(
        &mut self,
        effect: EffectId,
        title: &str,
        owner: Option<raw_window_handle::RawWindowHandle>,
    ) -> Result<(), HostError> {
        let h = self
            .slots
            .get_mut(&effect)
            .ok_or(HostError::Unsupported("no plug-in in this slot"))?;
        if let Some(w) = h.window.as_ref() {
            w.focus();
            return Ok(());
        }
        let window = PluginWindow::open(title, owner, h.main.as_mut())?;
        h.window = Some(window);
        Ok(())
    }

    /// Close the plug-in's window; `true` if one was open.
    pub fn close_window(&mut self, effect: EffectId) -> bool {
        let Some(h) = self.slots.get_mut(&effect) else {
            return false;
        };
        match h.window.take() {
            Some(w) => {
                h.main.close_editor();
                // The window goes after the editor closed (on Windows it destroys the
                // native window; elsewhere it is a placeholder with nothing to drop).
                #[cfg_attr(not(windows), allow(clippy::drop_non_drop))]
                drop(w);
                true
            }
            None => false,
        }
    }
}

impl Drop for PluginHost {
    fn drop(&mut self) {
        let ids: Vec<EffectId> = self.slots.keys().copied().collect();
        for id in ids {
            self.close_window(id);
        }
        let tracks: Vec<cantus_core::TrackId> = self.instruments.keys().copied().collect();
        for track in tracks {
            self.close_instrument_window(track);
        }
    }
}

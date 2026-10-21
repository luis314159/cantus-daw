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
    AudioSetup, AudioThread, Backend, Blame, HostError, MainThread, Notice, ParamChange,
    ParamEvent, ParamInfo, ParamOut, PluginInfo, StatePoll,
};
use crate::db::PluginDb;
use crate::scan::{ScanReport, Scanner};
use crate::unit::HostedUnit;
use crate::window::PluginWindow;
use crate::{from_project_format, to_project_format};
use cantus_core::{
    Command, CommandBus, EffectId, EffectKind, EffectSlot, EffectSpec, ExternalPlugin, MergeKey,
    Origin, PluginIsolation, PluginState, Project, StripId,
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
#[path = "session_iso.rs"]
mod iso;
#[path = "session_render.rs"]
mod render;

pub use inst::HostedInst;
pub use iso::{CpuFigure, RenderFailure, RenderLoss};

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
    /// A slot's plug-in stopped working (round 29, ADR 0063): its helper process crashed,
    /// or it hung and was ended. The slot passes the sound through until it is reloaded
    /// ([`PluginHost::reload`]); its last-good state is kept and saved.
    PluginCrashed {
        effect: EffectId,
        name: String,
        reason: String,
    },
    /// A track's hosted instrument stopped working; the track is silent until it is
    /// reloaded ([`PluginHost::reload_instrument`]).
    InstrumentCrashed {
        track: cantus_core::TrackId,
        name: String,
        reason: String,
    },
    /// Plug-ins were set to run isolated, but the helper (`cantus-sandbox`) is not
    /// installed: they run within Cantus. Said once.
    IsolationUnavailable,
    /// A change made in a plug-in's own window was refused by a freeze (round 29, ADR
    /// 0064, review 2): the plug-in is given back the project's values; say which.
    Frozen(cantus_core::freeze::FreezeTarget),
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
    /// Edits made in its own window or its own browser (values, presets, a state of its
    /// own), counted: a freeze rendering its region compares them, never its state's
    /// bytes, which automation and modulators move (review 3).
    pub(crate) edits: u64,
    pub(crate) window: Option<PluginWindow>,
    /// Where it runs, as loaded (round 29, ADR 0063).
    pub(crate) isolation: PluginIsolation,
    /// Why it stopped working, while it has.
    pub(crate) crashed: Option<String>,
    /// Whose doing its helper's end was, while it has stopped.
    pub(crate) blame: Option<Blame>,
    pub(crate) cpu: iso::CpuWatch,
    /// Just inserted, its load answered later (an isolated plug-in): its values and state
    /// go into the project when it loads, in the insert's undo step.
    fresh: bool,
    /// Where it runs changed: its state as it is now is asked for, and it is moved once
    /// that is in (round 29, review isolation-5).
    moving: bool,
}

impl Hosted {
    fn new(
        strip: StripId,
        info: PluginInfo,
        main: Box<dyn MainThread>,
        audio: Box<dyn AudioThread>,
        isolation: PluginIsolation,
    ) -> Hosted {
        Hosted {
            strip,
            params: main.params(),
            info,
            main,
            audio: Some(audio),
            synced_state: PluginState::default(),
            flushed: BTreeMap::new(),
            generation: Arc::new(AtomicU64::new(0)),
            restarting: false,
            gestures: Vec::new(),
            edits: 0,
            window: None,
            isolation,
            crashed: None,
            blame: None,
            cpu: iso::CpuWatch::default(),
            fresh: false,
            moving: false,
        }
    }

    /// Closes its window, if one is open, now.
    pub(crate) fn close_window_now(&mut self) {
        if let Some(w) = self.window.take() {
            self.main.close_editor();
            // The window goes after the editor closed (on Windows it destroys the native
            // window; elsewhere it is a placeholder with nothing to drop).
            #[cfg_attr(not(windows), allow(clippy::drop_non_drop))]
            drop(w);
        }
    }

    /// Why it stopped working, while it has (round 29).
    pub fn crashed(&self) -> Option<&str> {
        self.crashed.as_deref()
    }

    /// Where it runs (round 29).
    pub fn isolation(&self) -> PluginIsolation {
        self.isolation
    }

    /// What it costs, measured over the last half second (round 29).
    pub fn cpu(&self) -> Option<CpuFigure> {
        self.cpu.figure
    }

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
    /// Hosted slots whose plug-in could not be loaded: why, where it was tried, whether it
    /// crashed doing it.
    missing: HashMap<EffectId, iso::Missing>,
    scan_job: Option<mpsc::Receiver<(PluginDb, ScanReport)>>,
    /// Instances loaded for an offline render, kept until it has finished.
    render_mains: Vec<Box<dyn MainThread>>,
    /// Plug-ins the running render could not load (review #6, #47).
    render_lost: Vec<iso::RenderFailure>,
    /// The render being made ready ([`Self::render_ready`]).
    render_prep: Option<render::Prep>,
    /// The plug-ins the last [`Self::project_for_save`] took as they were last said, and
    /// why (review isolation-5 rule 2).
    saved_stale: Vec<(String, String)>,
    /// The application's main window: plug-in windows are owned by it.
    pub owner_window: Option<raw_window_handle::RawWindowHandle>,
    /// Hosted instruments (ADR 0028), by track.
    instruments: HashMap<cantus_core::TrackId, HostedInst>,
    /// State of instruments unloaded because their track left them, by track and plug-in.
    inst_parked: HashMap<(cantus_core::TrackId, u64), PluginState>,
    /// Tracks whose hosted instrument could not be loaded: the plug-in's key, why, where
    /// it was tried, whether it crashed doing it.
    inst_missing: HashMap<cantus_core::TrackId, iso::Missing>,
    /// Every parameter command a plug-in's own window ran since the application last took
    /// them ([`PluginHost::take_tweaks`]): what it records as automation and calls *last
    /// tweaked* (ADR 0057) — all of them, not only a frame's last (review 1).
    tweaks: Vec<Command>,
    /// Edits from plug-ins' windows a freeze refused since the last `idle` (review 2).
    pub(crate) frozen_refusals: Vec<cantus_core::freeze::FreezeTarget>,
    /// Plug-ins a freeze refused an edit of, to be given back what the project holds once
    /// per frame — at the gesture's end while one is open (review 3: a drag reloaded the
    /// whole state for every value) — and the instruments the same way.
    pub(crate) give_back: Vec<EffectId>,
    pub(crate) inst_give_back: Vec<cantus_core::TrackId>,
    /// How many give-backs were made (tests).
    pub(crate) give_backs: u64,
    /// The helper processes isolated plug-ins run in (round 29, ADR 0063).
    sandboxes: Option<Arc<crate::bridge::Sandboxes>>,
    /// Where a hosted plug-in runs when its project does not say: the machine's choice
    /// (Settings' plug-in manager). Within Cantus until the application says otherwise.
    pub default_isolation: PluginIsolation,
    /// Crashes this session, per plug-in (format, id).
    crash_counts: HashMap<(crate::backend::Format, String), u32>,
    /// Helper deaths already counted for a plug-in: (helper's serial, format, id).
    counted: HashSet<(u64, crate::backend::Format, String)>,
    /// The running render's stop ([`Self::cancel_render`]).
    render_cancel: Arc<std::sync::atomic::AtomicBool>,
    /// For the interface's tests: how the next reload fails, and slots said to wait on a
    /// window of their own.
    pub(crate) test_hooks: iso::TestHooks,
    /// Isolation was asked and the helper is missing; told once.
    isolation_missing: bool,
    told_missing: bool,
    /// Main halves of instances just replaced (a move, a reload): kept a moment, until the
    /// engine has surely swapped in their successors, so their helper never unloads an
    /// instance the engine is still calling.
    retired: Vec<(std::time::Instant, Box<dyn MainThread>)>,
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

pub(crate) fn changes(params: &BTreeMap<u32, f64>) -> Vec<ParamChange> {
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
    backend: &dyn Backend,
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
            let loaded = backend.load(info, setup).ok()?;
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

/// The latency each loaded hosted plug-in reports: inserts by id, instruments by track
/// (`PluginHost::latencies`).
pub type Latencies = (Vec<(EffectId, usize)>, Vec<(cantus_core::TrackId, usize)>);

/// Whether a freeze holds the plug-in of insert `effect` on `strip`: an edit of it would be
/// refused (review 3).
pub(crate) fn slot_locked(project: &Project, strip: StripId, effect: EffectId) -> bool {
    cantus_core::freeze::refuses(
        project,
        &Command::SetPluginParam {
            strip,
            effect,
            id: 0,
            value: None,
        },
    )
    .is_some()
}

/// Whether a freeze holds `track`'s hosted instrument (review 3).
pub(crate) fn instrument_locked(project: &Project, track: cantus_core::TrackId) -> bool {
    cantus_core::freeze::refuses(
        project,
        &Command::SetInstrumentPluginParam {
            track,
            id: 0,
            value: None,
        },
    )
    .is_some()
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
            render_lost: Vec::new(),
            render_prep: None,
            saved_stale: Vec::new(),
            owner_window: None,
            instruments: HashMap::new(),
            inst_parked: HashMap::new(),
            inst_missing: HashMap::new(),
            tweaks: Vec::new(),
            frozen_refusals: Vec::new(),
            give_back: Vec::new(),
            inst_give_back: Vec::new(),
            give_backs: 0,
            sandboxes: Some(crate::bridge::Sandboxes::new(
                crate::bridge::Sandboxes::default_exe(),
            )),
            default_isolation: PluginIsolation::Within,
            crash_counts: HashMap::new(),
            counted: HashSet::new(),
            render_cancel: Arc::default(),
            test_hooks: Default::default(),
            isolation_missing: false,
            told_missing: false,
            retired: Vec::new(),
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
        self.missing.get(&effect).map(|m| m.error.as_str())
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

    /// Writes the database now (the plug-in manager changed it).
    pub fn save_db_now(&self) {
        self.save_db();
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
    /// then serves the slot. It runs where the machine's default says.
    pub fn make_slot(
        &mut self,
        bus: &mut CommandBus,
        strip: StripId,
        info: &PluginInfo,
    ) -> Result<EffectSlot, HostError> {
        self.make_slot_isolated(bus, strip, info, None)
    }

    /// [`Self::make_slot`], run where `isolation` says (`None`: the default), which the
    /// slot's record keeps (round 29, ADR 0063).
    pub fn make_slot_isolated(
        &mut self,
        bus: &mut CommandBus,
        strip: StripId,
        info: &PluginInfo,
        isolation: Option<PluginIsolation>,
    ) -> Result<EffectSlot, HostError> {
        let (backend, runs) = self
            .backend_for_plugin(info, isolation, false)
            .ok_or(HostError::Unsupported("format"))?;
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
        plugin.isolation = isolation;
        let mut slot = bus.new_effect(EffectKind::External);
        slot.spec = EffectSpec::external(plugin);
        let fresh = !main.loaded();
        let mut h = Hosted::new(strip, info.clone(), main, loaded.audio, runs);
        h.params = params;
        h.synced_state = state;
        h.flushed = values;
        h.fresh = fresh;
        self.slots.insert(slot.id, h);
        Ok(slot)
    }

    /// The undo key a new insert's command is run under (`execute_merging`): an isolated
    /// plug-in's values, known once it loads, join the insert's step (round 29, review
    /// isolation-5).
    pub fn load_key(effect: EffectId) -> MergeKey {
        MergeKey::of(&("plugin-load", effect.0))
    }

    /// Load the plug-in of a slot the host has not seen: from the database by format and
    /// id, with the slot's state (or the one it had when its slot was last removed) and
    /// the project's values.
    fn load_for(
        &mut self,
        strip: StripId,
        slot: &EffectSlot,
        plugin: &ExternalPlugin,
        locked: bool,
    ) -> Result<Hosted, iso::Failed> {
        let format = from_project_format(plugin.format);
        let info = self
            .db
            .find(format, &plugin.id)
            .cloned()
            .ok_or_else(|| iso::Failed::from("not installed on this computer"))?;
        // A slot a freeze holds comes back as the project holds it — what its render was
        // made with — never with a state parked since (review 3).
        let parked = self.parked.remove(&slot.id);
        let state = match parked {
            Some(s) if !locked => s,
            _ => plugin.state.clone(),
        };
        let setup = self.setup;
        let inst = self.instantiate(
            &info,
            plugin.isolation,
            &state,
            &plugin.params,
            setup,
            false,
        )?;
        self.learn(inst.main.as_ref());
        let mut h = Hosted::new(strip, info, inst.main, inst.audio, inst.isolation);
        h.synced_state = plugin.state.clone();
        h.flushed = plugin.params.clone();
        Ok(h)
    }

    fn make_unit(
        &self,
        h: &mut Hosted,
        audio: Box<dyn AudioThread>,
        slot: &EffectSlot,
    ) -> EffectUnit {
        let meter = h.cpu.renew();
        let unit = HostedUnit::new(
            audio,
            &h.params,
            |id| h.main.param_value(id),
            h.main.latency(),
            self.setup.sample_rate,
            h.generation.clone(),
        )
        .metered(meter);
        EffectUnit::from_fx(slot, Box::new(unit))
    }

    /// Before each snapshot: load what is new, deliver it, unload what left, load changed
    /// states. See the module documentation.
    pub fn sync(&mut self, project: &Project, engine: &mut EngineHandle) -> Vec<HostNotice> {
        let mut notices = Vec::new();
        let mut seen = HashSet::new();
        // Frozen (round 29, ADR 0064): the plug-ins of a chain that plays its render, or
        // that only feeds one, are not run — unloaded (their state parked) until it thaws.
        let dormant = engine
            .frozen
            .dormant(project, engine.sample_rate.round() as u32);
        // The strips' inserts and the master's (ADR 0026).
        for (host, slot) in project.all_effects() {
            let Some(plugin) = slot.spec.external.as_deref() else {
                continue;
            };
            if dormant.effect_rests(host) {
                continue;
            }
            seen.insert(slot.id);
            let runs = self.effective_isolation(plugin.isolation);
            // A plug-in that failed where it was tried waits — until where it runs changes
            // (then it is tried there, review #15) or it is reloaded from its menu: never
            // tried again by itself (review isolation-5 rule 4).
            match self.missing.get(&slot.id) {
                Some(m) if m.isolation == runs => continue,
                Some(_) => {
                    self.missing.remove(&slot.id);
                }
                None => {}
            }
            if !self.slots.contains_key(&slot.id) {
                let locked = slot_locked(project, host, slot.id);
                match self.load_for(host, slot, plugin, locked) {
                    Ok(h) => {
                        self.slots.insert(slot.id, h);
                    }
                    Err(failed) => {
                        notices.extend(self.load_failed(slot.id, plugin, runs, failed));
                        continue;
                    }
                }
            }
            // Where it runs changed (its choice, or the machine's default): its state as
            // it is now is asked for, and `idle` moves it there once that is in.
            if let Some(h) = self.slots.get_mut(&slot.id) {
                if h.isolation != runs && !h.restarting && !h.moving {
                    h.moving = true;
                    h.main.ask_state_now();
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
                let unit = self.make_unit(&mut h, audio, slot);
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
        self.tell_unavailable(&mut notices);
        notices
    }

    /// A slot's plug-in could not be loaded where it runs: kept as missing until it is
    /// reloaded or where it runs changes; told as a crash (counted, reloadable) when its
    /// helper died doing it, else as a failure.
    fn load_failed(
        &mut self,
        effect: EffectId,
        plugin: &ExternalPlugin,
        runs: PluginIsolation,
        failed: iso::Failed,
    ) -> Option<HostNotice> {
        log::warn!("plug-in {}: {}", plugin.name, failed.why);
        self.missing.insert(
            effect,
            iso::Missing {
                key: 0,
                error: failed.why.clone(),
                isolation: runs,
                crashed: failed.crashed,
                found: self
                    .db
                    .find(from_project_format(plugin.format), &plugin.id)
                    .is_some(),
            },
        );
        Some(if failed.crashed {
            let format = from_project_format(plugin.format);
            if let Some(info) = self.db.find(format, &plugin.id).cloned() {
                self.count_crash(&info, failed.blame, None);
            }
            HostNotice::PluginCrashed {
                effect,
                name: plugin.name.clone(),
                reason: failed.why,
            }
        } else {
            HostNotice::LoadFailed {
                effect,
                name: plugin.name.clone(),
                error: failed.why,
            }
        })
    }

    /// A slot whose plug-in now runs elsewhere, its state asked for: moved once that is in
    /// — or, not to be had in time, from what it said last (said in the log). Polled by
    /// `idle`, never waited for (round 29, review isolation-5).
    fn continue_move(
        &mut self,
        project: &Project,
        engine: &mut EngineHandle,
        effect: EffectId,
    ) -> Option<HostNotice> {
        let (_, slot) = find_slot(project, effect)?;
        let plugin = slot.spec.external.as_deref()?;
        let runs = self.effective_isolation(plugin.isolation);
        let h = self.slots.get_mut(&effect).filter(|h| h.moving)?;
        if h.isolation == runs {
            h.moving = false;
            return None;
        }
        let state = match h.main.poll_state() {
            StatePoll::Waiting => return None,
            StatePoll::Fresh(s) => PluginState::new(s),
            StatePoll::Stale(s, why) => {
                log::warn!("{}: moved with the state it said last: {why}", h.info.name);
                PluginState::new(s)
            }
            StatePoll::Failed(_) => plugin.state.clone(),
        };
        h.moving = false;
        match self.move_plugin(slot, plugin, engine, state) {
            Ok(()) => None,
            Err(failed) => self.load_failed(effect, plugin, runs, failed),
        }
    }

    /// Loads a slot's plug-in where it now runs (round 29) with `state` and the project's
    /// values; the new unit replaces the old.
    fn move_plugin(
        &mut self,
        slot: &EffectSlot,
        plugin: &ExternalPlugin,
        engine: &mut EngineHandle,
        state: PluginState,
    ) -> Result<(), iso::Failed> {
        let mut h = self.slots.remove(&slot.id).ok_or("not loaded")?;
        h.close_window_now();
        let setup = self.setup;
        let info = h.info.clone();
        let values = h.flushed.clone();
        // The old instance goes first: a helper it leaves empty ends.
        h.audio = None;
        let inst = match self.instantiate(&info, plugin.isolation, &state, &values, setup, false) {
            Ok(i) => i,
            Err(e) => {
                self.parked.insert(slot.id, state);
                return Err(e);
            }
        };
        let old = std::mem::replace(&mut h.main, inst.main);
        self.retire(old);
        h.isolation = inst.isolation;
        h.crashed = None;
        h.blame = None;
        h.params = h.main.params();
        h.generation.fetch_add(1, Ordering::AcqRel);
        let unit = self.make_unit(&mut h, inst.audio, slot);
        if engine.knows_effect(slot.id) {
            engine.replace_unit(unit);
        } else {
            engine.add_unit(unit);
        }
        self.slots.insert(slot.id, h);
        Ok(())
    }

    /// Unload a plug-in, keeping its state for a slot that comes back.
    fn unload(&mut self, id: EffectId) {
        if let Some(mut h) = self.slots.remove(&id) {
            h.close_window_now();
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
        self.retired
            .retain(|(at, _)| at.elapsed() < std::time::Duration::from_secs(1));
        self.forget_render_prep();
        // A plug-in that crashed its helper as it left (its destructor) is counted.
        self.count_departed();
        let mut notices = Vec::new();
        if let Some(rx) = &self.scan_job {
            match rx.try_recv() {
                Ok((db, report)) => {
                    // Folders the user changed while the scan ran are kept, and the crash
                    // records made (or forgotten) meanwhile (review #18, #52).
                    let folders = std::mem::take(&mut self.db.folders);
                    let crashes = std::mem::take(&mut self.db.crashes);
                    self.db = db;
                    self.db.folders = folders;
                    self.db.crashes = crashes;
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
                    Notice::Loaded => self.on_loaded(bus, engine, id),
                    Notice::LoadFailed(why) => {
                        notices.extend(self.on_load_failed(bus.project(), engine, id, why))
                    }
                    Notice::Crashed { reason, blame } => {
                        if let Some(n) = self.on_crash(id, reason, blame) {
                            notices.push(n);
                        }
                    }
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
                    // Its window opened after all (its helper was busy when asked): kept
                    // as open (review isolation-4 #39).
                    Notice::EditorOpened => {
                        if let Some(h) = self.slots.get_mut(&id) {
                            if h.window.is_none() {
                                h.window = crate::window::PluginWindow::floating(&h.info.name);
                            }
                        }
                    }
                }
            }
            // Stopped without a notice reaching here (it died while loading a state, say).
            let stopped = self
                .slots
                .get(&id)
                .filter(|h| h.crashed.is_none())
                .and_then(|h| h.main.stopped());
            if let Some((reason, blame)) = stopped {
                if let Some(n) = self.on_crash(id, reason, blame) {
                    notices.push(n);
                }
            }
            if let Some(n) = self.continue_restart(bus.project(), engine, id) {
                notices.push(n);
            }
            if let Some(n) = self.continue_move(bus.project(), engine, id) {
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
            let rate = self.setup.sample_rate;
            if let Some(h) = self.slots.get_mut(&id) {
                if let Some(size) = h.window.as_mut().and_then(|w| w.take_resize()) {
                    if let Some(taken) = h.main.resize_editor(size) {
                        if let Some(w) = h.window.as_mut() {
                            w.fit(taken);
                        }
                    }
                }
                h.cpu.update(h.main.as_ref(), rate);
            }
        }
        self.idle_instruments(bus, engine, &mut notices);
        self.give_back_now(bus.project());
        self.tell_unavailable(&mut notices);
        for t in std::mem::take(&mut self.frozen_refusals) {
            if !notices.contains(&HostNotice::Frozen(t)) {
                notices.push(HostNotice::Frozen(t));
            }
        }
        notices
    }

    /// A window's change a freeze refused: the plug-in is given back what the project
    /// holds (its state, then its values) — at the end of the frame, or of the gesture
    /// while one is open (review 3) — and the refusal is said (review 2: the plug-in kept
    /// the new value, a refresh rendered it under the old key, a save wrote it).
    pub(crate) fn refused_by_freeze(
        &mut self,
        _project: &Project,
        effect: EffectId,
        error: &cantus_core::CommandError,
    ) -> bool {
        let cantus_core::CommandError::Frozen(target) = error else {
            return false;
        };
        if !self.give_back.contains(&effect) {
            self.give_back.push(effect);
        }
        self.frozen_refusals.push(*target);
        true
    }

    /// The give-backs due: each refused plug-in once, its gesture over (review 3).
    fn give_back_now(&mut self, project: &Project) {
        let due: Vec<EffectId> = self
            .give_back
            .iter()
            .copied()
            .filter(|id| self.slots.get(id).is_none_or(|h| h.gestures.is_empty()))
            .collect();
        self.give_back.retain(|id| !due.contains(id));
        for effect in due {
            let (Some(h), Some((_, slot))) =
                (self.slots.get_mut(&effect), find_slot(project, effect))
            else {
                continue;
            };
            let Some(plugin) = slot.spec.external.as_deref() else {
                continue;
            };
            if !plugin.state.is_empty() {
                if let Err(e) = h.main.load_state(plugin.state.bytes()) {
                    log::warn!("{}: {e}", h.info.name);
                }
            }
            let mut out = ParamOut::new();
            h.main.flush(&changes(&plugin.params), &mut out);
            h.flushed = plugin.params.clone();
            h.synced_state = plugin.state.clone();
            h.generation.fetch_add(1, Ordering::AcqRel);
            self.give_backs += 1;
        }
        self.instrument_give_back_now(project);
    }

    /// The latency each loaded hosted plug-in reports now — inserts by id, instruments by
    /// track: what a plan made without instantiating them counts
    /// (`cantus_engine::freeze::render::strip_arrival`, review 3).
    pub fn latencies(&self) -> Latencies {
        let fx = self
            .slots
            .iter()
            .map(|(id, h)| (*id, h.latency()))
            .collect();
        let inst = self
            .instruments
            .iter()
            .map(|(t, h)| (*t, h.latency()))
            .collect();
        (fx, inst)
    }

    /// Whether the plug-in `format`/`id` is installed (scanned) on this computer — a
    /// render made with a stand-in for it is made again once it is (review 3).
    pub fn installed(&self, format: cantus_core::PluginFormat, id: &str) -> bool {
        self.db.find(from_project_format(format), id).is_some()
    }

    /// How many plug-ins were given back what the project holds (tests).
    pub fn give_backs(&self) -> u64 {
        self.give_backs
    }

    /// Edits made in the windows or browsers of the plug-ins of `effects` and the
    /// instruments of `tracks` — values, presets, states of their own (review 3: a freeze
    /// compares them, not the states' bytes automation and modulators move).
    pub fn edits_of(&self, effects: &[EffectId], tracks: &[cantus_core::TrackId]) -> u64 {
        let fx: u64 = effects
            .iter()
            .filter_map(|id| self.slots.get(id))
            .map(|h| h.edits)
            .sum();
        let inst: u64 = tracks
            .iter()
            .filter_map(|t| self.instruments.get(t))
            .map(|h| h.edits)
            .sum();
        fx + inst
    }

    /// A state loaded into a plug-in without telling the host — a change the host hears of
    /// only through its state, if at all (tests: a preset from its own browser).
    #[doc(hidden)]
    pub fn load_state_for_test(&mut self, effect: EffectId, state: &[u8]) {
        if let Some(h) = self.slots.get_mut(&effect) {
            let _ = h.main.load_state(state);
        }
    }

    /// A state loaded into a plug-in by itself (its own browser), then its `StateDirty`
    /// (tests).
    #[doc(hidden)]
    pub fn state_dirty_for_test(&mut self, bus: &mut CommandBus, effect: EffectId, state: &[u8]) {
        if let Some(h) = self.slots.get_mut(&effect) {
            let _ = h.main.load_state(state);
        }
        self.capture_changes(bus, effect);
    }

    /// The states the instances served to the last render hold now, in order (tests: a
    /// freeze's render is made with what the project holds, never the live instance's).
    #[doc(hidden)]
    pub fn render_states_for_test(&mut self) -> Vec<Vec<u8>> {
        self.render_mains
            .iter_mut()
            .filter_map(|m| m.save_state().ok())
            .collect()
    }

    /// A slot's plug-in stopped working: marked, counted as its blame says, its window
    /// gone, told once.
    /// For the interface's tests: the loaded plug-in in `effect`'s slot crashed (kept, silent,
    /// until it is reloaded). The application never calls it.
    #[doc(hidden)]
    pub fn mark_loaded_crashed_for_tests(&mut self, effect: EffectId, reason: &str) {
        if let Some(h) = self.slots.get_mut(&effect) {
            h.crashed = Some(reason.to_string());
        }
    }

    fn on_crash(&mut self, effect: EffectId, reason: String, blame: Blame) -> Option<HostNotice> {
        let h = self.slots.get_mut(&effect)?;
        if h.crashed.is_some() {
            return None;
        }
        h.crashed = Some(reason.clone());
        h.blame = Some(blame);
        h.restarting = false;
        // Its helper made the editor's window, and it is gone with it.
        if let Some(w) = h.window.take() {
            #[cfg_attr(not(windows), allow(clippy::drop_non_drop))]
            drop(w);
        }
        let info = h.info.clone();
        let helper = h.main.helper_id();
        self.count_crash(&info, blame, helper);
        Some(HostNotice::PluginCrashed {
            effect,
            name: info.name,
            reason,
        })
    }

    /// An isolated plug-in loaded (its load is answered later): its parameters read, its
    /// unit made again with its audio half and latency — and, one just inserted, its
    /// values and state recorded in the project in the insert's undo step, as an insert
    /// within Cantus records them as it loads (round 29, review isolation-5).
    fn on_loaded(&mut self, bus: &mut CommandBus, engine: &mut EngineHandle, effect: EffectId) {
        let Some(mut h) = self.slots.remove(&effect) else {
            return;
        };
        self.learn(h.main.as_ref());
        h.params = h.main.params();
        if let Some(audio) = h.main.take_audio() {
            if h.audio.is_some() {
                // Not delivered yet: this one goes instead.
                h.audio = Some(audio);
            } else if let Some((_, slot)) = find_slot(bus.project(), effect) {
                let unit = self.make_unit(&mut h, audio, slot);
                if engine.knows_effect(effect) {
                    engine.replace_unit(unit);
                } else {
                    engine.add_unit(unit);
                }
            }
        }
        let (fresh, strip) = (std::mem::take(&mut h.fresh), h.strip);
        self.slots.insert(effect, h);
        if !fresh {
            return;
        }
        let Some((state, mut params)) = self.capture(effect) else {
            return;
        };
        // A value set since it was inserted stands (it was sent to it, after its load).
        let set = find_slot(bus.project(), effect)
            .and_then(|(_, s)| s.spec.external.as_deref())
            .map(|p| p.params.clone())
            .unwrap_or_default();
        params.extend(set);
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
        if let Err(e) = bus.execute_merging(cmd, Origin::User, Self::load_key(effect)) {
            log::warn!("plug-in state: {e}");
        }
    }

    /// An isolated plug-in's load failed, its helper alive (it refused, or did not load
    /// within its limit): kept as a plug-in that could not be loaded, with Reload — never
    /// tried again by itself (round 29, review isolation-5 rule 4). Its slot is a wire.
    fn on_load_failed(
        &mut self,
        project: &Project,
        engine: &mut EngineHandle,
        effect: EffectId,
        why: String,
    ) -> Option<HostNotice> {
        let mut h = self.slots.remove(&effect)?;
        h.close_window_now();
        // What it was given stays for its reload.
        if let Ok(s) = h.main.save_state() {
            self.parked.insert(effect, PluginState::new(s));
        }
        let (_, slot) = find_slot(project, effect)?;
        if engine.knows_effect(effect) {
            engine.replace_unit(EffectUnit::from_fx(slot, Box::new(Wire)));
        }
        let plugin = slot.spec.external.as_deref()?;
        let failed = iso::Failed::from(why);
        self.load_failed(effect, plugin, h.isolation, failed)
    }

    /// Keeps a replaced instance's main half a moment (see `retired`).
    pub(crate) fn retire(&mut self, main: Box<dyn MainThread>) {
        self.retired.push((std::time::Instant::now(), main));
    }

    /// The parameter commands plug-in windows ran since the last take, oldest first.
    pub fn take_tweaks(&mut self) -> Vec<Command> {
        std::mem::take(&mut self.tweaks)
    }

    pub(crate) fn tweaked(&mut self, cmd: Command) {
        if self.tweaks.len() < TWEAKS {
            self.tweaks.push(cmd);
        }
    }

    /// One thing a plug-in's window did, as a command: a value inside a gesture merges
    /// into one undo step with the rest of the gesture.
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
                    Ok(_) => {
                        if let Some(h) = self.slots.get_mut(&effect) {
                            h.edits += 1;
                        }
                        self.tweaked(cmd)
                    }
                    Err(e) => {
                        if !self.refused_by_freeze(bus.project(), effect, &e) {
                            log::warn!("plug-in parameter {id}: {e}");
                        }
                    }
                }
            }
            FxParamEvent::End(id) => {
                let was = h.gestures.len();
                h.gestures.retain(|g| *g != id);
                if h.gestures.len() != was {
                    bus.end_gesture();
                }
                // A gesture ended: a good moment for an isolated plug-in's last-good state.
                h.main.checkpoint();
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
            // A state of its own that moved no value (a sample it loaded): kept for the
            // save — unless a freeze holds it: then given back and said, as an edit of a
            // value is (review 3: the change went unheard, and an unload kept it).
            let probe = Command::SetPluginParam {
                strip,
                effect,
                id: 0,
                value: None,
            };
            if let Some(target) = cantus_core::freeze::refuses(bus.project(), &probe) {
                let e = cantus_core::CommandError::Frozen(target);
                self.refused_by_freeze(bus.project(), effect, &e);
            } else if let Some(h) = self.slots.get_mut(&effect) {
                h.edits += 1;
            }
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
        match bus.execute(cmd, Origin::User) {
            Ok(_) => {
                if let Some(h) = self.slots.get_mut(&effect) {
                    h.edits += 1;
                }
            }
            Err(e) => {
                if !self.refused_by_freeze(bus.project(), effect, &e) {
                    log::warn!("plug-in state: {e}");
                }
            }
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
        let mut h = self.slots.remove(&effect)?;
        if !h.restarting {
            self.slots.insert(effect, h);
            return None;
        }
        let audio = match h.main.restart() {
            Ok(None) => {
                // Its audio half is still on its way back.
                self.slots.insert(effect, h);
                return None;
            }
            Ok(Some(audio)) => audio,
            Err(HostError::Unsupported(_)) => {
                // The backend cannot restart in place: a new instance where it runs, with
                // the state.
                let state = h
                    .main
                    .save_state()
                    .map(PluginState::new)
                    .unwrap_or_default();
                let info = h.info.clone();
                let values = h.flushed.clone();
                match self.instantiate(&info, Some(h.isolation), &state, &values, setup, false) {
                    Ok(inst) => {
                        h.main = inst.main;
                        h.params = h.main.params();
                        inst.audio
                    }
                    Err(error) => {
                        h.restarting = false;
                        self.slots.insert(effect, h);
                        return Some(HostNotice::RestartFailed {
                            effect,
                            error: error.why,
                        });
                    }
                }
            }
            // It stopped working: the crash is told by `idle`, its slot stays a wire.
            Err(HostError::Crashed(_)) => {
                h.restarting = false;
                self.slots.insert(effect, h);
                return None;
            }
            Err(e) => {
                h.restarting = false;
                self.slots.insert(effect, h);
                return Some(HostNotice::RestartFailed {
                    effect,
                    error: e.to_string(),
                });
            }
        };
        h.restarting = false;
        h.generation.fetch_add(1, Ordering::AcqRel);
        let unit = self.make_unit(&mut h, audio, slot);
        if engine.knows_effect(effect) {
            engine.replace_unit(unit);
        } else {
            engine.add_unit(unit);
        }
        self.slots.insert(effect, h);
        None
    }

    /// Whether a plug-in is between being taken out of the engine and put back.
    pub fn restarting(&self, effect: EffectId) -> bool {
        self.slots.get(&effect).is_some_and(|h| h.restarting)
    }

    // ---- saving and rendering ------------------------------------------------------

    /// Asks every loaded plug-in for its state as it is now — a save's first step; an
    /// isolated plug-in answers later ([`Self::states_in`]). Never waits (round 29, review
    /// isolation-5 rule 2).
    pub fn ask_states(&mut self) {
        for h in self.slots.values_mut() {
            h.main.ask_state_now();
        }
        for h in self.instruments.values_mut() {
            h.main.ask_state_now();
        }
    }

    /// Whether the states asked for are all in — or not to be had in time, each by its own
    /// limit from when its helper started it. A save asks each frame.
    pub fn states_in(&mut self) -> bool {
        let mains = self.slots.values_mut().map(|h| &mut h.main);
        let insts = self.instruments.values_mut().map(|h| &mut h.main);
        let mut waiting = false;
        for m in mains.chain(insts) {
            waiting |= m.state_waiting();
        }
        !waiting
    }

    /// The plug-ins the last [`Self::project_for_save`] saved as they were last said (not
    /// to be had as they are now), and why: the save says so (never silently).
    pub fn saved_stale(&self) -> &[(String, String)] {
        &self.saved_stale
    }

    /// One plug-in's state for a save: as it is now, or — said in `saved_stale` — as it
    /// was last said.
    fn state_for_save(
        main: &mut Box<dyn MainThread>,
        name: &str,
        stale: &mut Vec<(String, String)>,
    ) -> Option<PluginState> {
        match main.poll_state() {
            StatePoll::Fresh(bytes) => Some(PluginState::new(bytes)),
            StatePoll::Stale(bytes, why) => {
                stale.push((name.to_string(), why));
                Some(PluginState::new(bytes))
            }
            StatePoll::Waiting => {
                stale.push((name.to_string(), "it was still answering".into()));
                main.save_state().ok().map(PluginState::new)
            }
            StatePoll::Failed(e) => {
                log::warn!("{name}: {e}");
                None
            }
        }
    }

    /// `project` as it should be saved: every loaded plug-in's state — asked for first
    /// ([`Self::ask_states`], [`Self::states_in`]); one not to be had as it is now is saved
    /// as it was last said, and [`Self::saved_stale`] names it. A slot whose plug-in is
    /// missing keeps what the project holds, untouched.
    pub fn project_for_save(&mut self, project: &Project) -> Project {
        let mut out = project.clone();
        let mut stale = Vec::new();
        // A plug-in in a frozen region keeps the state its render holds (review 2).
        let locks = project
            .any_frozen()
            .then(|| cantus_core::freeze::Locks::of(project));
        for slot in out.all_effects_mut() {
            let Some(h) = self.slots.get_mut(&slot.id) else {
                continue;
            };
            if locks.as_ref().is_some_and(|l| l.chain.contains(&h.strip)) {
                continue;
            }
            let Some(plugin) = slot.spec.external.as_deref_mut() else {
                continue;
            };
            if let Some(state) = Self::state_for_save(&mut h.main, &h.info.name, &mut stale) {
                plugin.state = state;
            }
        }
        self.instruments_for_save(&mut out, locks.as_ref(), &mut stale);
        stale.extend(self.test_hooks.stale_saves.iter().cloned());
        self.saved_stale = stale;
        out
    }

    /// The render that [`Self::render_units`] served has finished: its instances go (ask
    /// [`Self::render_failures`] first).
    pub fn finish_render(&mut self) {
        self.render_mains.clear();
        self.render_lost.clear();
        self.render_prep = None;
        self.fresh_render_cancel();
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
            if !h.main.focus_editor() {
                w.focus();
            }
            return Ok(());
        }
        if h.crashed.is_some() {
            return Err(HostError::Crashed(h.crashed.clone().unwrap_or_default()));
        }
        let window = PluginWindow::open(title, owner, h.main.as_mut())?;
        // An isolated plug-in's window is its helper's: brought to the front there.
        h.main.focus_editor();
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

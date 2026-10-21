//! Where the hosted plug-ins of a project run, and what happens when one stops working
//! (round 29, ADR 0063). A child of `session`, beside the instrument halves.
//!
//! - **Choosing.** Each plug-in runs where its project record says
//!   (`ExternalPlugin::isolation`), or else where [`PluginHost::default_isolation`] says —
//!   the machine's choice, from Settings. *Within Cantus* loads it with the format's own
//!   backend; the others with a [`BridgedBackend`] into a helper of its group. When the
//!   helper is not installed, plug-ins run within Cantus, and the application is told once
//!   ([`HostNotice::IsolationUnavailable`]).
//! - **Moving.** A plug-in whose choice changed is moved at the next `sync`: its state is
//!   taken, it is loaded where it now runs, the state and the project's values go in, and
//!   the new unit replaces the old one in the engine.
//! - **Crashing.** A crash ([`Notice::Crashed`]) marks the plug-in crashed, counts it (this
//!   session, and in the plug-in database for good) and tells the application once per
//!   crash. Its unit keeps passing the sound through (an effect) or silence (an instrument).
//! - **Reloading.** [`PluginHost::reload`] loads it again where it runs, with its last-good
//!   state and the project's values — what the user had — and replaces the dead unit.
//! - **CPU.** Every hosted unit measures its calls ([`CpuMeter`]); [`PluginHost::idle`]
//!   turns that into a figure per plug-in twice a second.

use super::{changes, settle_new, HostNotice, PluginHost};
use crate::backend::{
    AudioSetup, AudioThread, Backend, Blame, Format, HostError, MainThread, ParamOut, PluginInfo,
};
use crate::bridge::{group_key, BridgedBackend, Sandboxes};
use crate::unit::CpuMeter;
use cantus_core::{EffectId, PluginIsolation, PluginState, Project, TrackId};
use cantus_engine::EngineHandle;
use std::collections::BTreeMap;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// A plug-in after loading: its halves and where it runs.
pub(crate) struct Instance {
    pub main: Box<dyn MainThread>,
    pub audio: Box<dyn AudioThread>,
    pub isolation: PluginIsolation,
}

/// Why a plug-in could not be loaded, and whether its helper died doing it (a crash, not
/// a refusal: counted, told as a crash, and reloadable — review #15).
#[derive(Clone, Debug)]
pub(crate) struct Failed {
    pub why: String,
    pub crashed: bool,
    /// Whose doing a helper's end was (a neighbour's crash is not this plug-in's; review
    /// #7, #40).
    pub blame: Blame,
}

impl From<String> for Failed {
    fn from(why: String) -> Self {
        Failed {
            why,
            crashed: false,
            blame: Blame::Unknown,
        }
    }
}

impl From<&str> for Failed {
    fn from(why: &str) -> Self {
        Failed::from(why.to_string())
    }
}

impl From<Failed> for String {
    fn from(f: Failed) -> Self {
        f.why
    }
}

/// A plug-in that could not be loaded: why, where it was tried, whether that was a crash.
#[derive(Clone, Debug)]
pub(crate) struct Missing {
    /// The instrument's key ([`cantus_engine::hosted_key`]); 0 for an insert.
    pub key: u64,
    pub error: String,
    /// Where it was tried: a choice that changes tries again (review #15).
    pub isolation: PluginIsolation,
    pub crashed: bool,
    /// The plug-in is on this computer (a load was tried): one not installed is not offered
    /// a reload.
    pub found: bool,
}

/// How a plug-in failed an offline render (review isolation-4 #41).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenderLoss {
    /// It stopped working during the render: the result lacks it from that point on.
    Stopped,
    /// It could not be made for the render at all: the result lacks it throughout.
    NeverLoaded,
    /// Its state as it was then could not be had when the render began (its helper held
    /// by a window of a plug-in, or too slow to answer): the render used the state it had
    /// last said — a warning, not a failure (review isolation-4 #10).
    Stale,
}

/// A plug-in that failed an offline render (review #6, #47; isolation-4 #41).
#[derive(Clone, Debug, PartialEq)]
pub struct RenderFailure {
    pub name: String,
    pub reason: String,
    pub kind: RenderLoss,
}

/// The plug-ins waiting on a window of their own: inserts and instruments, with their
/// names ([`PluginHost::waiting`]).
pub type Waiting = (Vec<(EffectId, String)>, Vec<(TrackId, String)>);

/// For the interface's tests only: what the host is made to say (never set by the
/// application).
#[derive(Debug, Default)]
pub(crate) struct TestHooks {
    /// The next reload fails so: why, and whether it crashed.
    pub reload_fails: Option<(String, bool)>,
    /// Inserts said to wait on a window of their own.
    pub waiting: Vec<EffectId>,
    /// Plug-ins every save says it kept as they were last said: name, why.
    pub stale_saves: Vec<(String, String)>,
}

/// What a plug-in costs, for its slot's tooltip (BACKLOG §3's per-plug-in meter).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CpuFigure {
    /// The share of real time Cantus's audio threads spend in it (0.012 is 1.2 %): its
    /// processing, and for an isolated plug-in the round trip to its helper too.
    pub share: f32,
    /// Of which the plug-in's own processing, measured in its helper (isolated only).
    pub own: Option<f32>,
    /// Times a job of it missed its deadline (isolated only); the blocks that then passed
    /// without it are not counted one by one.
    pub late: u64,
}

/// The meter of one loaded plug-in, read twice a second.
#[derive(Debug)]
pub(crate) struct CpuWatch {
    pub meter: Arc<CpuMeter>,
    last: (u64, u64),
    last_own: Option<(u64, u64)>,
    at: Instant,
    pub figure: Option<CpuFigure>,
}

impl Default for CpuWatch {
    fn default() -> Self {
        CpuWatch {
            meter: Arc::new(CpuMeter::default()),
            last: (0, 0),
            last_own: None,
            at: Instant::now(),
            figure: None,
        }
    }
}

const CPU_EVERY: Duration = Duration::from_millis(500);

impl CpuWatch {
    /// A fresh meter (a new instance), its figure kept until the next reading.
    pub fn renew(&mut self) -> Arc<CpuMeter> {
        self.meter = Arc::new(CpuMeter::default());
        self.last = (0, 0);
        self.last_own = None;
        self.meter.clone()
    }

    pub fn update(&mut self, main: &dyn MainThread, sample_rate: f64) {
        if self.at.elapsed() < CPU_EVERY {
            return;
        }
        self.at = Instant::now();
        let share = |now: (u64, u64), last: (u64, u64)| {
            let ns = now.0.saturating_sub(last.0) as f64;
            let frames = now.1.saturating_sub(last.1) as f64;
            (frames > 0.0).then(|| (ns / (frames / sample_rate * 1e9)) as f32)
        };
        let now = self.meter.read();
        let s = share(now, self.last);
        self.last = now;
        let own_now = main.own_time();
        let own = match (own_now, self.last_own) {
            (Some(n), Some(l)) => share(n, l),
            _ => None,
        };
        self.last_own = own_now;
        if let Some(share) = s {
            self.figure = Some(CpuFigure {
                share,
                own,
                late: main.late_blocks(),
            });
        }
    }
}

impl PluginHost {
    /// The helpers isolated plug-ins run in (tests point at their own build of it).
    pub fn set_sandboxes(&mut self, pool: Arc<Sandboxes>) {
        self.sandboxes = Some(pool);
    }

    pub fn sandboxes(&self) -> Option<&Arc<Sandboxes>> {
        self.sandboxes.as_ref()
    }

    /// Whether isolated plug-ins can run here (the helper is installed).
    pub fn isolation_available(&self) -> bool {
        self.sandboxes.as_ref().is_some_and(|s| s.available())
    }

    /// Where a plug-in whose project says `chosen` runs on this machine.
    pub fn effective_isolation(&self, chosen: Option<PluginIsolation>) -> PluginIsolation {
        let wanted = chosen.unwrap_or(self.default_isolation);
        if wanted.isolated() && !self.isolation_available() {
            PluginIsolation::Within
        } else {
            wanted
        }
    }

    /// The backend that loads `info` where `chosen` says, and where that is.
    pub(crate) fn backend_for_plugin(
        &mut self,
        info: &PluginInfo,
        chosen: Option<PluginIsolation>,
        offline: bool,
    ) -> Option<(Box<dyn Backend>, PluginIsolation)> {
        let wanted = chosen.unwrap_or(self.default_isolation);
        let isolation = self.effective_isolation(chosen);
        if wanted != isolation {
            self.isolation_missing = true;
        }
        match (group_key(isolation, info), self.sandboxes.clone()) {
            (Some(group), Some(pool)) => Some((
                Box::new(BridgedBackend {
                    format: info.format,
                    // A render's instances run in helpers of their own: a crash or a hang
                    // there never takes the live plug-ins down with it (review #47).
                    group: if offline {
                        format!("render-{group}")
                    } else {
                        group
                    },
                    pool,
                    offline,
                    cancel: offline.then(|| self.render_cancel.clone()),
                }),
                isolation,
            )),
            _ => crate::backend_for(info.format).map(|b| (b, PluginIsolation::Within)),
        }
    }

    /// Loads `info` where `chosen` says, with `state` and the project's `values`, settled
    /// (restarted at once if the state asked for it). An offline instance that could not
    /// be made — not loaded, its state refused, not settled — is a render that lost it
    /// (review #6, #47; isolation-4 #10, #24, #34, #63).
    pub(crate) fn instantiate(
        &mut self,
        info: &PluginInfo,
        chosen: Option<PluginIsolation>,
        state: &PluginState,
        values: &BTreeMap<u32, f64>,
        setup: AudioSetup,
        offline: bool,
    ) -> Result<Instance, Failed> {
        let made = self.make_instance(info, chosen, state, values, setup, offline);
        if offline {
            if let Err(f) = &made {
                self.render_lost.push(RenderFailure {
                    name: info.name.clone(),
                    reason: f.why.clone(),
                    kind: RenderLoss::NeverLoaded,
                });
            }
        }
        made
    }

    fn make_instance(
        &mut self,
        info: &PluginInfo,
        chosen: Option<PluginIsolation>,
        state: &PluginState,
        values: &BTreeMap<u32, f64>,
        setup: AudioSetup,
        offline: bool,
    ) -> Result<Instance, Failed> {
        let (backend, isolation) = self
            .backend_for_plugin(info, chosen, offline)
            .ok_or_else(|| Failed::from("format not hosted"))?;
        let failed = |e: HostError| match e {
            HostError::Ended { why, blame } => Failed {
                why,
                crashed: true,
                blame,
            },
            e => Failed {
                crashed: matches!(e, HostError::Crashed(_)),
                why: e.to_string(),
                blame: Blame::This,
            },
        };
        // An isolated plug-in: one request with its state and values, answered later —
        // its helper loads, gives the state and the values, and settles it (round 29,
        // review isolation-5). Nothing here waits.
        let pairs: Vec<(u32, f64)> = values.iter().map(|(k, v)| (*k, *v)).collect();
        if let Some(made) = backend.load_with(info, setup, state.bytes(), &pairs) {
            let loaded = made.map_err(failed)?;
            return Ok(Instance {
                main: loaded.main,
                audio: loaded.audio,
                isolation,
            });
        }
        let loaded = backend.load(info, setup).map_err(failed)?;
        let mut main = loaded.main;
        if !state.is_empty() {
            if let Err(e) = main.load_state(state.bytes()) {
                if offline {
                    // A render never runs a plug-in with another state than its own
                    // (review isolation-4 #10, #34).
                    return Err(Failed {
                        crashed: main.stopped().is_some(),
                        blame: main.stopped().map_or(Blame::Unknown, |(_, b)| b),
                        why: e.to_string(),
                    });
                }
                log::warn!("{}: {e}", info.name);
            }
        }
        let mut out = ParamOut::new();
        main.flush(&changes(values), &mut out);
        let audio = settle_new(
            &mut main,
            loaded.audio,
            info,
            setup,
            values,
            backend.as_ref(),
        )
        .ok_or_else(|| Failed::from("it could not restart after its state was loaded"))?;
        Ok(Instance {
            main,
            audio,
            isolation,
        })
    }

    /// The application learns once that isolation was asked and the helper is missing.
    pub(crate) fn tell_unavailable(&mut self, notices: &mut Vec<HostNotice>) {
        if self.isolation_missing && !self.told_missing {
            self.told_missing = true;
            notices.push(HostNotice::IsolationUnavailable);
        }
    }

    /// A plug-in stopped working: counted this session and remembered in the database when
    /// it was its own doing, once per helper death (`helper`: the helper's serial, never
    /// reused as a process id is) however many of its instances it took down (review #0,
    /// #20; isolation-4 #35, #53, #59); a crash while loading (no helper of its own yet)
    /// every time. An end nothing lays on anyone, and a neighbour's, are neither — so
    /// *Run individually* is never offered for a plug-in that never crashed (review
    /// isolation-4 #58).
    pub(crate) fn count_crash(&mut self, info: &PluginInfo, blame: Blame, helper: Option<u64>) {
        if blame != Blame::This {
            return;
        }
        let first = match helper {
            Some(h) => self.counted.insert((h, info.format, info.id.clone())),
            None => true,
        };
        if first {
            self.db.remember_crash(info);
            self.save_db();
            *self
                .crash_counts
                .entry((info.format, info.id.clone()))
                .or_default() += 1;
        }
    }

    /// Plug-ins that crashed their helper while they left it (their destructor), counted
    /// as their own crashes (review isolation-4 #45).
    pub(crate) fn count_departed(&mut self) {
        let Some(pool) = self.sandboxes.clone() else {
            return;
        };
        for d in pool.take_departed_crashes() {
            self.count_crash(&d.info, Blame::This, Some(d.helper));
        }
    }

    /// Whether a crashed slot's plug-in is the one its helper's end is laid on: the one
    /// *Run individually* is offered for — never one that merely shared its process, nor
    /// one of several nothing chose between (review #49; isolation-4 #58). A plug-in that
    /// crashed while it loaded is its own end.
    pub fn crash_culprit(&self, effect: EffectId) -> bool {
        match self.slots.get(&effect) {
            Some(h) => h.blame == Some(Blame::This),
            None => true,
        }
    }

    /// [`Self::crash_culprit`] for a track's hosted instrument.
    pub fn instrument_crash_culprit(&self, track: TrackId) -> bool {
        match self.instruments.get(&track) {
            Some(h) => h.blame == Some(Blame::This),
            None => true,
        }
    }

    /// How many times `info`'s plug-in crashed in this session.
    pub fn crashes_this_session(&self, format: Format, id: &str) -> u32 {
        self.crash_counts
            .get(&(format, id.to_string()))
            .copied()
            .unwrap_or(0)
    }

    /// Whether a slot's plug-in has stopped working (or crashed while it loaded), and why.
    pub fn crashed(&self, effect: EffectId) -> Option<&str> {
        match self.slots.get(&effect) {
            Some(h) => h.crashed.as_deref(),
            None => self
                .missing
                .get(&effect)
                .filter(|m| m.crashed)
                .map(|m| m.error.as_str()),
        }
    }

    /// Whether a track's hosted instrument has stopped working, and why.
    pub fn instrument_crashed(&self, track: TrackId) -> Option<&str> {
        match self.instruments.get(&track) {
            Some(h) => h.crashed.as_deref(),
            None => self
                .inst_missing
                .get(&track)
                .filter(|m| m.crashed)
                .map(|m| m.error.as_str()),
        }
    }

    /// Every crashed slot and instrument.
    pub fn crashed_all(&self) -> (Vec<EffectId>, Vec<TrackId>) {
        let mut e: Vec<EffectId> = self
            .slots
            .iter()
            .filter(|(_, h)| h.crashed.is_some())
            .map(|(id, _)| *id)
            .collect();
        let mut t: Vec<TrackId> = self
            .instruments
            .iter()
            .filter(|(_, h)| h.crashed.is_some())
            .map(|(id, _)| *id)
            .collect();
        e.sort();
        t.sort();
        (e, t)
    }

    /// Loads a crashed (or any) slot's plug-in again where it runs, with its last-good
    /// state and the project's values, and puts it back in the engine.
    pub fn reload(
        &mut self,
        project: &Project,
        engine: &mut EngineHandle,
        effect: EffectId,
    ) -> Result<(), String> {
        if let Some((why, crashed)) = self.test_hooks.reload_fails.take() {
            if let Some(m) = self.missing.get_mut(&effect) {
                m.error = why.clone();
                m.crashed = crashed;
            }
            return Err(why);
        }
        let (strip, slot) = super::find_slot(project, effect).ok_or("no such slot")?;
        let plugin = slot.spec.external.as_deref().ok_or("not a hosted slot")?;
        if !self.slots.contains_key(&effect) && self.missing.remove(&effect).is_some() {
            // It never loaded (it crashed loading): loaded now, as `sync` would.
            return self.load_missing(strip, slot, plugin, engine);
        }
        let mut old = self.slots.remove(&effect).ok_or("not loaded")?;
        old.close_window_now();
        let state = old
            .main
            .save_state()
            .map(PluginState::new)
            .unwrap_or_else(|_| plugin.state.clone());
        let values = old.flushed.clone();
        let info = old.info.clone();
        let setup = self.setup;
        match self.instantiate(&info, plugin.isolation, &state, &values, setup, false) {
            Ok(inst) => {
                let gone = std::mem::replace(&mut old.main, inst.main);
                self.retire(gone);
                old.isolation = inst.isolation;
                old.crashed = None;
                old.blame = None;
                old.restarting = false;
                old.params = old.main.params();
                old.synced_state = plugin.state.clone();
                old.generation
                    .fetch_add(1, std::sync::atomic::Ordering::AcqRel);
                let unit = self.make_unit(&mut old, inst.audio, slot);
                engine.replace_unit(unit);
                self.slots.insert(effect, old);
                Ok(())
            }
            Err(e) => {
                // It crashed again while it loaded: counted, so a plug-in that keeps
                // crashing is offered a process of its own (review #15). Its card says why
                // it is off now, and whose doing this end was (review isolation-4 #64).
                if e.crashed {
                    self.count_crash(&info, e.blame, None);
                    old.blame = Some(e.blame);
                }
                old.crashed = Some(e.why.clone());
                self.slots.insert(effect, old);
                Err(e.why)
            }
        }
    }

    /// A slot whose plug-in never loaded, loaded now and put in the engine.
    fn load_missing(
        &mut self,
        strip: cantus_core::StripId,
        slot: &cantus_core::EffectSlot,
        plugin: &cantus_core::ExternalPlugin,
        engine: &mut EngineHandle,
    ) -> Result<(), String> {
        match self.load_for(strip, slot, plugin) {
            Ok(mut h) => {
                if let Some(audio) = h.audio.take() {
                    let unit = self.make_unit(&mut h, audio, slot);
                    if engine.knows_effect(slot.id) {
                        engine.replace_unit(unit);
                    } else {
                        engine.add_unit(unit);
                    }
                }
                self.slots.insert(slot.id, h);
                Ok(())
            }
            Err(f) => {
                self.count_load_crash(plugin, &f);
                self.missing.insert(
                    slot.id,
                    Missing {
                        key: 0,
                        error: f.why.clone(),
                        isolation: self.effective_isolation(plugin.isolation),
                        crashed: f.crashed,
                        found: self
                            .db
                            .find(crate::from_project_format(plugin.format), &plugin.id)
                            .is_some(),
                    },
                );
                Err(f.why)
            }
        }
    }

    /// Loads a track's crashed (or any) hosted instrument again, as [`Self::reload`].
    pub fn reload_instrument(
        &mut self,
        project: &Project,
        engine: &mut EngineHandle,
        track: TrackId,
    ) -> Result<(), String> {
        if let Some((why, crashed)) = self.test_hooks.reload_fails.take() {
            if let Some(m) = self.inst_missing.get_mut(&track) {
                m.error = why.clone();
                m.crashed = crashed;
            }
            return Err(why);
        }
        let plugin = project
            .track(track)
            .and_then(|t| t.instrument.external())
            .ok_or("not a hosted instrument")?;
        if !self.instruments.contains_key(&track) && self.inst_missing.remove(&track).is_some() {
            // It never loaded (it crashed loading): loaded now, as `sync` would.
            let key = cantus_engine::hosted_key(plugin);
            return match self.load_instrument(track, plugin, key) {
                Ok(mut h) => {
                    if let Some(audio) = h.audio.take() {
                        let rate = self.setup.sample_rate;
                        let unit = super::inst::make_unit(&mut h, track, audio, rate);
                        super::inst::deliver(engine, unit);
                    }
                    self.instruments.insert(track, h);
                    Ok(())
                }
                Err(f) => {
                    self.count_load_crash(plugin, &f);
                    self.inst_missing.insert(
                        track,
                        Missing {
                            key,
                            error: f.why.clone(),
                            isolation: self.effective_isolation(plugin.isolation),
                            crashed: f.crashed,
                            found: self
                                .db
                                .find(crate::from_project_format(plugin.format), &plugin.id)
                                .is_some(),
                        },
                    );
                    Err(f.why)
                }
            };
        }
        let mut old = self.instruments.remove(&track).ok_or("not loaded")?;
        old.close_window_now();
        let state = old
            .main
            .save_state()
            .map(PluginState::new)
            .unwrap_or_else(|_| plugin.state.clone());
        let values = old.flushed.clone();
        let info = old.info.clone();
        let setup = self.setup;
        match self.instantiate(&info, plugin.isolation, &state, &values, setup, false) {
            Ok(inst) => {
                let gone = std::mem::replace(&mut old.main, inst.main);
                self.retire(gone);
                old.isolation = inst.isolation;
                old.crashed = None;
                old.blame = None;
                old.restarting = false;
                old.params = old.main.params();
                old.synced_state = plugin.state.clone();
                old.generation
                    .fetch_add(1, std::sync::atomic::Ordering::AcqRel);
                let unit = super::inst::make_unit(&mut old, track, inst.audio, setup.sample_rate);
                super::inst::deliver(engine, unit);
                self.instruments.insert(track, old);
                Ok(())
            }
            Err(e) => {
                if e.crashed {
                    self.count_crash(&info, e.blame, None);
                    old.blame = Some(e.blame);
                }
                old.crashed = Some(e.why.clone());
                self.instruments.insert(track, old);
                Err(e.why)
            }
        }
    }

    /// Reloads every crashed plug-in; returns the names that could not be.
    pub fn reload_all_crashed(
        &mut self,
        project: &Project,
        engine: &mut EngineHandle,
    ) -> Vec<String> {
        let (effects, tracks) = self.crashed_all();
        let mut failed = Vec::new();
        for e in effects {
            let name = self.slots.get(&e).map(|h| h.info.name.clone());
            if let Err(why) = self.reload(project, engine, e) {
                failed.push(format!("{}: {why}", name.unwrap_or_default()));
            }
        }
        for t in tracks {
            let name = self.instruments.get(&t).map(|h| h.info.name.clone());
            if let Err(why) = self.reload_instrument(project, engine, t) {
                failed.push(format!("{}: {why}", name.unwrap_or_default()));
            }
        }
        failed
    }

    /// Why a slot's plug-in could not be loaded — not a crash: it was refused, or it did
    /// not load within its limit — while it is missing: it can be loaded again from its
    /// menu (review isolation-4 #57; isolation-5).
    pub fn load_error(&self, effect: EffectId) -> Option<&str> {
        if self.slots.contains_key(&effect) {
            return None;
        }
        self.missing
            .get(&effect)
            .filter(|m| !m.crashed && m.found)
            .map(|m| m.error.as_str())
    }

    /// [`Self::load_error`] for a track's hosted instrument.
    pub fn instrument_load_error(&self, track: TrackId) -> Option<&str> {
        if self.instruments.contains_key(&track) {
            return None;
        }
        self.inst_missing
            .get(&track)
            .filter(|m| !m.crashed && m.found)
            .map(|m| m.error.as_str())
    }

    /// Whether a plug-in is still loading — an isolated one, whose load is answered later
    /// (its slot passes the sound through, its track is silent, meanwhile) — or waits for
    /// its state to move where it now runs.
    pub fn loading(&self) -> bool {
        let busy = |crashed: &Option<String>, main: &dyn MainThread| {
            crashed.is_none() && !main.loaded() && main.stopped().is_none()
        };
        self.slots
            .values()
            .any(|h| h.moving || busy(&h.crashed, h.main.as_ref()))
            || self
                .instruments
                .values()
                .any(|h| h.moving || busy(&h.crashed, h.main.as_ref()))
    }

    /// What a slot's plug-in costs, measured over the last half second.
    pub fn cpu(&self, effect: EffectId) -> Option<CpuFigure> {
        self.slots.get(&effect).and_then(|h| h.cpu.figure)
    }

    /// What a track's hosted instrument costs.
    pub fn instrument_cpu(&self, track: TrackId) -> Option<CpuFigure> {
        self.instruments.get(&track).and_then(|h| h.cpu.figure)
    }

    /// Where a slot's plug-in runs now.
    pub fn isolation(&self, effect: EffectId) -> Option<PluginIsolation> {
        self.slots.get(&effect).map(|h| h.isolation)
    }

    /// Where a track's hosted instrument runs now.
    pub fn instrument_isolation(&self, track: TrackId) -> Option<PluginIsolation> {
        self.instruments.get(&track).map(|h| h.isolation)
    }

    /// For the interface's tests: marks a hosted slot (`Some(effect)`) or a track's hosted
    /// instrument (with its [`cantus_engine::hosted_key`]) as stopped — as a plug-in that crashed while it loaded — and counts
    /// `times` crashes of the plug-in (`format`, `id`) this session. The application never
    /// calls it.
    #[doc(hidden)]
    pub fn mark_crashed_for_tests(
        &mut self,
        target: (Option<EffectId>, Option<(TrackId, u64)>),
        (format, id): (Format, &str),
        reason: &str,
        times: u32,
    ) {
        let missing = Missing {
            key: 0,
            error: reason.to_string(),
            isolation: self.default_isolation,
            crashed: true,
            found: true,
        };
        if let Some(e) = target.0 {
            self.slots.remove(&e);
            self.missing.insert(e, missing.clone());
        }
        if let Some((t, key)) = target.1 {
            self.instruments.remove(&t);
            self.inst_missing.insert(t, Missing { key, ..missing });
        }
        *self
            .crash_counts
            .entry((format, id.to_string()))
            .or_default() += times;
    }

    /// For the interface's tests: the next reload fails with `why` (a crash when
    /// `crashed`), as one whose plug-in crashed again, or could not be loaded, does. The
    /// application never calls it.
    #[doc(hidden)]
    pub fn fail_next_reload_for_tests(&mut self, why: &str, crashed: bool) {
        self.test_hooks.reload_fails = Some((why.to_string(), crashed));
    }

    /// For the interface's tests: a hosted slot that could not be loaded — not a crash
    /// (refused, or not loaded within its limit). The application never calls it.
    #[doc(hidden)]
    pub fn mark_missing_for_tests(&mut self, effect: EffectId, why: &str) {
        self.slots.remove(&effect);
        self.missing.insert(
            effect,
            Missing {
                key: 0,
                error: why.to_string(),
                isolation: self.default_isolation,
                crashed: false,
                found: true,
            },
        );
    }

    /// For the interface's tests: a track's hosted instrument (with its
    /// [`cantus_engine::hosted_key`]) that could not be loaded — not a crash. The
    /// application never calls it.
    #[doc(hidden)]
    pub fn mark_instrument_missing_for_tests(&mut self, track: TrackId, key: u64, why: &str) {
        self.instruments.remove(&track);
        self.inst_missing.insert(
            track,
            Missing {
                key,
                error: why.to_string(),
                isolation: self.default_isolation,
                crashed: false,
                found: true,
            },
        );
    }

    /// For the interface's tests: every save from now on says it kept `name` as it was
    /// last said, for `why`. The application never calls it.
    #[doc(hidden)]
    pub fn stale_in_saves_for_tests(&mut self, name: &str, why: &str) {
        self.test_hooks
            .stale_saves
            .push((name.to_string(), why.to_string()));
    }

    /// For the interface's tests: a hosted slot said to wait on a window of its own (or no
    /// longer). The application never calls it.
    #[doc(hidden)]
    pub fn set_waiting_for_tests(&mut self, effect: EffectId, waiting: bool) {
        self.test_hooks.waiting.retain(|e| *e != effect);
        if waiting {
            self.test_hooks.waiting.push(effect);
        }
    }

    /// The loaded plug-ins waiting on a window of their own — a call of theirs served past
    /// its limit while their helper lives: a dialog, perhaps behind Cantus (review
    /// isolation-4 #54) — inserts and instruments, with their names.
    pub fn waiting(&self) -> Waiting {
        let mut e: Vec<(EffectId, String)> = self
            .slots
            .iter()
            .filter(|(id, h)| {
                h.crashed.is_none() && (h.main.waiting() || self.test_hooks.waiting.contains(id))
            })
            .map(|(id, h)| (*id, h.info.name.clone()))
            .collect();
        for id in &self.test_hooks.waiting {
            if !self.slots.contains_key(id) {
                e.push((*id, String::new()));
            }
        }
        let mut t: Vec<(TrackId, String)> = self
            .instruments
            .iter()
            .filter(|(_, h)| h.crashed.is_none() && h.main.waiting())
            .map(|(id, h)| (*id, h.info.name.clone()))
            .collect();
        e.sort_by_key(|(id, _)| *id);
        t.sort_by_key(|(id, _)| *id);
        (e, t)
    }

    /// How many other loaded plug-ins share the process of a slot's (`Some(effect)`) or a
    /// track's plug-in: stopping it stops them too (review isolation-5 #44).
    pub fn neighbours(&self, effect: Option<EffectId>, track: Option<TrackId>) -> usize {
        let helper = effect
            .and_then(|e| self.slots.get(&e))
            .map(|h| h.main.helper_id())
            .or_else(|| {
                track
                    .and_then(|t| self.instruments.get(&t))
                    .map(|h| h.main.helper_id())
            })
            .flatten();
        let Some(helper) = helper else {
            return 0;
        };
        let same = |m: &dyn MainThread| m.helper_id() == Some(helper);
        let n = self
            .slots
            .values()
            .filter(|h| same(h.main.as_ref()))
            .count()
            + self
                .instruments
                .values()
                .filter(|h| same(h.main.as_ref()))
                .count();
        n.saturating_sub(1)
    }

    /// Brings the windows a waiting insert's plug-in waits on to the front.
    pub fn surface(&mut self, effect: EffectId) {
        if let Some(h) = self.slots.get_mut(&effect) {
            h.main.surface();
        }
    }

    /// [`Self::surface`] for a track's hosted instrument.
    pub fn surface_instrument(&mut self, track: TrackId) {
        if let Some(h) = self.instruments.get_mut(&track) {
            h.main.surface();
        }
    }

    /// Ends a waiting insert's plug-in — its helper — at the user's word: told as its own
    /// crash, reloadable (review isolation-4 #54).
    pub fn stop(&mut self, effect: EffectId) {
        self.test_hooks.waiting.retain(|e| *e != effect);
        if let Some(h) = self.slots.get_mut(&effect) {
            h.main.stop();
        }
    }

    /// [`Self::stop`] for a track's hosted instrument.
    pub fn stop_instrument(&mut self, track: TrackId) {
        if let Some(h) = self.instruments.get_mut(&track) {
            h.main.stop();
        }
    }

    /// For the interface's tests: the running render lost `name` as `kind` says (as a
    /// render instance that stopped, could not be loaded, or began with a stale state
    /// does). The application never calls it.
    #[doc(hidden)]
    pub fn lose_in_render_for_tests(&mut self, name: &str, reason: &str, kind: RenderLoss) {
        self.render_lost.push(RenderFailure {
            name: name.to_string(),
            reason: reason.to_string(),
            kind,
        });
    }

    /// The plug-ins of the running render (export, stems, bake, freeze) that failed it —
    /// stopped during it, never loaded for it, or began with a stale state: a render that
    /// lost one must not be reported done, and one with a stale state says so (review #6,
    /// #47; isolation-4 #10, #41). Asked before [`Self::finish_render`].
    pub fn render_failures(&self) -> Vec<RenderFailure> {
        let mut out = self.render_lost.clone();
        out.extend(self.render_mains.iter().filter_map(|m| {
            m.stopped().map(|(reason, _)| RenderFailure {
                name: m.info().name.clone(),
                reason,
                kind: if m.loaded() {
                    RenderLoss::Stopped
                } else {
                    RenderLoss::NeverLoaded
                },
            })
        }));
        out
    }

    /// A plug-in whose helper died while it loaded again: counted as its blame says.
    fn count_load_crash(&mut self, plugin: &cantus_core::ExternalPlugin, f: &Failed) {
        if !f.crashed {
            return;
        }
        let format = crate::from_project_format(plugin.format);
        if let Some(info) = self.db.find(format, &plugin.id).cloned() {
            self.count_crash(&info, f.blame, None);
        }
    }

    /// Stops the running render's isolated plug-ins from being waited for (the export's
    /// Stop): a hung one ends with its helper instead of holding the render (review #1).
    pub fn cancel_render(&self) {
        self.render_cancel
            .store(true, std::sync::atomic::Ordering::Release);
    }

    pub(crate) fn fresh_render_cancel(&mut self) {
        if self.render_mains.is_empty() {
            self.render_cancel = Arc::new(AtomicBool::new(false));
        }
    }

    /// Jobs of isolated plug-ins that missed their deadline, since loaded: shown beside the
    /// engine's own drop counts.
    pub fn late_blocks(&self) -> u64 {
        self.slots
            .values()
            .map(|h| h.main.late_blocks())
            .chain(self.instruments.values().map(|h| h.main.late_blocks()))
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::{PluginInfo, PluginKind};
    use crate::scan::ScanReport;

    /// A neighbour's notice that comes first does not use up the helper death's one count
    /// (review isolation-3: seen on Linux, where the notices came the other way round).
    #[test]
    fn a_neighbours_notice_first_still_counts_the_plugin_that_did_it() {
        let mut host = PluginHost::new(None, 48_000.0);
        let info = PluginInfo {
            format: Format::Clap,
            id: "com.example.glacier".into(),
            name: "Glacier".into(),
            vendor: "Example".into(),
            version: String::new(),
            path: std::path::PathBuf::new(),
            kind: crate::backend::PluginKind::Effect,
            main_inputs: 2,
            main_outputs: 2,
            sidechain: false,
            editor: false,
            features: Vec::new(),
        };
        host.count_crash(&info, Blame::Neighbour, Some(7));
        host.count_crash(&info, Blame::This, Some(7));
        host.count_crash(&info, Blame::This, Some(7));
        assert_eq!(host.crashes_this_session(Format::Clap, &info.id), 1);
    }

    /// A helper's death is counted once for the plug-in it is laid on, by the helper's
    /// serial (never reused, as a process id may be), and never for one nothing lays it on
    /// (review isolation-4 #35, #53, #58, #59).
    #[test]
    fn deaths_are_counted_once_per_helper_and_never_for_nobody() {
        let mut host = PluginHost::new(None, 48_000.0);
        let info = PluginInfo {
            format: Format::Clap,
            id: "com.example.glacier".into(),
            name: "Glacier".into(),
            vendor: "Example".into(),
            version: String::new(),
            path: std::path::PathBuf::new(),
            kind: PluginKind::Effect,
            main_inputs: 2,
            main_outputs: 2,
            sidechain: false,
            editor: false,
            features: Vec::new(),
        };
        host.count_crash(&info, Blame::Unknown, Some(1));
        assert_eq!(host.crashes_this_session(Format::Clap, &info.id), 0);
        assert!(host.db.crash_record(Format::Clap, &info.id).is_none());
        host.count_crash(&info, Blame::This, Some(1));
        host.count_crash(&info, Blame::This, Some(1));
        assert_eq!(host.crashes_this_session(Format::Clap, &info.id), 1);
        host.count_crash(&info, Blame::This, Some(2));
        assert_eq!(host.crashes_this_session(Format::Clap, &info.id), 2);
    }

    /// A crash recorded (or forgotten) while a background scan ran survives the scan's
    /// result being taken (review #18, #52).
    #[test]
    fn a_scan_keeps_the_crash_records_made_while_it_ran() {
        let mut host = PluginHost::new(None, 48_000.0);
        let before_scan = host.db.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        host.scan_job = Some(rx);
        let info = PluginInfo {
            format: Format::Clap,
            id: "com.example.glacier".into(),
            name: "Glacier".into(),
            vendor: "Example".into(),
            version: String::new(),
            path: std::path::PathBuf::new(),
            kind: PluginKind::Effect,
            main_inputs: 2,
            main_outputs: 2,
            sidechain: false,
            editor: false,
            features: Vec::new(),
        };
        host.count_crash(&info, Blame::This, None);
        tx.send((before_scan, ScanReport::default())).unwrap();
        let mut bus = cantus_core::CommandBus::new(Project::new("scan"));
        let mut engine = EngineHandle::dummy();
        let _ = host.idle(&mut bus, &mut engine);
        assert!(host.scan_job.is_none(), "the scan's result was taken");
        assert!(host.db.crash_record(Format::Clap, &info.id).is_some());
    }
}

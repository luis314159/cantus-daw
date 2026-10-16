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
    AudioSetup, AudioThread, Backend, Format, MainThread, ParamOut, PluginInfo,
};
use crate::bridge::{group_key, BridgedBackend, Sandboxes};
use crate::unit::CpuMeter;
use cantus_core::{EffectId, PluginIsolation, PluginState, Project, TrackId};
use cantus_engine::EngineHandle;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// A plug-in after loading: its halves and where it runs.
pub(crate) struct Instance {
    pub main: Box<dyn MainThread>,
    pub audio: Box<dyn AudioThread>,
    pub isolation: PluginIsolation,
}

/// What a plug-in costs, for its slot's tooltip (BACKLOG §3's per-plug-in meter).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CpuFigure {
    /// The share of real time Cantus's audio threads spend in it (0.012 is 1.2 %): its
    /// processing, and for an isolated plug-in the round trip to its helper too.
    pub share: f32,
    /// Of which the plug-in's own processing, measured in its helper (isolated only).
    pub own: Option<f32>,
    /// Blocks that passed without it because it was late (isolated only).
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
                    group,
                    pool,
                    offline,
                }),
                isolation,
            )),
            _ => crate::backend_for(info.format).map(|b| (b, PluginIsolation::Within)),
        }
    }

    /// Loads `info` where `chosen` says, with `state` and the project's `values`, settled
    /// (restarted at once if the state asked for it).
    pub(crate) fn instantiate(
        &mut self,
        info: &PluginInfo,
        chosen: Option<PluginIsolation>,
        state: &PluginState,
        values: &BTreeMap<u32, f64>,
        setup: AudioSetup,
        offline: bool,
    ) -> Result<Instance, String> {
        let (backend, isolation) = self
            .backend_for_plugin(info, chosen, offline)
            .ok_or_else(|| "format not hosted".to_string())?;
        let loaded = backend.load(info, setup).map_err(|e| e.to_string())?;
        let mut main = loaded.main;
        if !state.is_empty() {
            if let Err(e) = main.load_state(state.bytes()) {
                log::warn!("{}: {e}", info.name);
            }
        }
        let mut out = ParamOut::new();
        main.flush(&changes(values), &mut out);
        let audio = settle_new(&mut main, loaded.audio, info, setup, values, backend.as_ref())
            .ok_or_else(|| "it could not restart after its state was loaded".to_string())?;
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

    /// A plug-in stopped working: counted this session and in the database.
    pub(crate) fn count_crash(&mut self, info: &PluginInfo) {
        *self
            .crash_counts
            .entry((info.format, info.id.clone()))
            .or_default() += 1;
        self.db.remember_crash(info);
        self.save_db();
    }

    /// How many times `info`'s plug-in crashed in this session.
    pub fn crashes_this_session(&self, format: Format, id: &str) -> u32 {
        self.crash_counts
            .get(&(format, id.to_string()))
            .copied()
            .unwrap_or(0)
    }

    /// Whether a slot's plug-in has stopped working, and why.
    pub fn crashed(&self, effect: EffectId) -> Option<&str> {
        self.slots.get(&effect).and_then(|h| h.crashed.as_deref())
    }

    /// Whether a track's hosted instrument has stopped working, and why.
    pub fn instrument_crashed(&self, track: TrackId) -> Option<&str> {
        self.instruments
            .get(&track)
            .and_then(|h| h.crashed.as_deref())
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
        let (_, slot) = super::find_slot(project, effect).ok_or("no such slot")?;
        let plugin = slot.spec.external.as_deref().ok_or("not a hosted slot")?;
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
                self.slots.insert(effect, old);
                Err(e)
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
        let plugin = project
            .track(track)
            .and_then(|t| t.instrument.external())
            .ok_or("not a hosted instrument")?;
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
                self.instruments.insert(track, old);
                Err(e)
            }
        }
    }

    /// Reloads every crashed plug-in; returns the names that could not be.
    pub fn reload_all_crashed(&mut self, project: &Project, engine: &mut EngineHandle) -> Vec<String> {
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

    /// Blocks every isolated plug-in passed without it because it was late, since loaded:
    /// shown beside the engine's own drop counts.
    pub fn late_blocks(&self) -> u64 {
        self.slots
            .values()
            .map(|h| h.main.late_blocks())
            .chain(self.instruments.values().map(|h| h.main.late_blocks()))
            .sum()
    }
}

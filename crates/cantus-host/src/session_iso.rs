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
}

/// A plug-in that stopped working during an offline render (review #6, #47).
#[derive(Clone, Debug, PartialEq)]
pub struct RenderFailure {
    pub name: String,
    pub reason: String,
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
    /// (restarted at once if the state asked for it).
    pub(crate) fn instantiate(
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
        let loaded = backend.load(info, setup).map_err(|e| match e {
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
        });
        let loaded = match loaded {
            Ok(l) => l,
            Err(f) => {
                if offline {
                    // A render that lost a plug-in before its first block says so (#6, #47).
                    self.render_lost.push(RenderFailure {
                        name: info.name.clone(),
                        reason: f.why.clone(),
                    });
                }
                return Err(f);
            }
        };
        let mut main = loaded.main;
        if !state.is_empty() {
            if let Err(e) = main.load_state(state.bytes()) {
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
    /// it was its own doing; when nothing says whose it was, counted once per helper death
    /// (`pid`) this session, not remembered; a neighbour of the one that did is neither
    /// (review #25, #49).
    pub(crate) fn count_crash(&mut self, info: &PluginInfo, blame: Blame, pid: Option<u32>) {
        // Once per helper death and plug-in, however many of its instances it took down
        // (review #0, #20); a crash while loading (no helper of its own yet) every time.
        let first = match pid {
            Some(pid) => self
                .unknown_counted
                .insert((pid, info.format, info.id.clone())),
            None => true,
        };
        let count = match blame {
            Blame::This if first => {
                self.db.remember_crash(info);
                self.save_db();
                true
            }
            Blame::Unknown => first,
            _ => false,
        };
        if count {
            *self
                .crash_counts
                .entry((info.format, info.id.clone()))
                .or_default() += 1;
        }
    }

    /// Whether a crashed slot's plug-in is the one its helper's end is laid on (or nothing
    /// says otherwise): the one *Run individually* is offered for (review #49).
    pub fn crash_culprit(&self, effect: EffectId) -> bool {
        match self.slots.get(&effect) {
            Some(h) => h.blame != Some(Blame::Neighbour),
            None => true,
        }
    }

    /// [`Self::crash_culprit`] for a track's hosted instrument.
    pub fn instrument_crash_culprit(&self, track: TrackId) -> bool {
        match self.instruments.get(&track) {
            Some(h) => h.blame != Some(Blame::Neighbour),
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
                // crashing is offered a process of its own (review #15).
                if e.crashed {
                    self.count_crash(&info, e.blame, None);
                }
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
                }
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

    /// The plug-ins of the running render (export, stems, bake) that stopped working
    /// during it: a render that lost one must not be reported done (review #6, #47).
    /// Asked before [`Self::finish_render`].
    pub fn render_failures(&self) -> Vec<RenderFailure> {
        let mut out = self.render_lost.clone();
        out.extend(self.render_mains.iter().filter_map(|m| {
            m.stopped().map(|(reason, _)| RenderFailure {
                name: m.info().name.clone(),
                reason,
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

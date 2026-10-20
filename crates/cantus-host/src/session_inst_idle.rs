//! The instrument half of the host's frame (ADR 0028): requests, restarts, gestures as
//! commands, saving, rendering, windows. A child of `session` beside `session_inst`.

use super::inst::HostedInst;
use super::{to_fx, PluginHost};
use crate::backend::{AudioSetup, HostError, Notice, ParamOut};
use crate::inst_unit::{HostedInstrument, Silent};
use crate::session::HostNotice;
use crate::window::PluginWindow;
use cantus_core::{Command, CommandBus, MergeKey, Origin, PluginState, Project, TrackId};
use cantus_engine::{EngineHandle, FxParamEvent, NativeUnit, UnitKind, MAX_BLOCK_FRAMES};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

impl PluginHost {
    /// The instrument half of [`Self::idle`].
    pub(crate) fn idle_instruments(
        &mut self,
        bus: &mut CommandBus,
        engine: &mut EngineHandle,
        notices: &mut Vec<HostNotice>,
    ) {
        for (track, event) in engine.take_inst_params() {
            self.record_instrument(bus, track, event);
        }
        let tracks: Vec<TrackId> = self.instruments.keys().copied().collect();
        for track in tracks {
            let mut out = ParamOut::new();
            if let Some(h) = self.instruments.get_mut(&track) {
                h.main.flush(&[], &mut out);
            }
            for e in out.events() {
                self.record_instrument(bus, track, to_fx(*e));
            }
            let asked = match self.instruments.get_mut(&track) {
                Some(h) => h.main.idle(),
                None => continue,
            };
            for notice in asked {
                match notice {
                    Notice::RestartNeeded | Notice::LatencyChanged(_) => {
                        self.begin_instrument_restart(engine, track)
                    }
                    Notice::ParamsChanged => {
                        if let Some(h) = self.instruments.get_mut(&track) {
                            h.params = h.main.params();
                        }
                        self.capture_instrument_changes(bus, track);
                    }
                    Notice::StateDirty => self.capture_instrument_changes(bus, track),
                    Notice::Crashed { reason, blame } => {
                        if let Some(n) = self.on_instrument_crash(track, reason, blame) {
                            notices.push(n);
                        }
                    }
                    Notice::EditorResized(size) => {
                        if let Some(w) = self
                            .instruments
                            .get_mut(&track)
                            .and_then(|h| h.window.as_mut())
                        {
                            w.fit(size);
                        }
                    }
                    Notice::EditorClosed => {
                        if self.close_instrument_window(track) {
                            notices.push(HostNotice::InstrumentWindowClosed(track));
                        }
                    }
                }
            }
            // Stopped without a notice reaching here (it died while loading a state, say).
            let stopped = self
                .instruments
                .get(&track)
                .filter(|h| h.crashed.is_none())
                .and_then(|h| h.main.stopped());
            if let Some((reason, blame)) = stopped {
                if let Some(n) = self.on_instrument_crash(track, reason, blame) {
                    notices.push(n);
                }
            }
            if let Some(n) = self.continue_instrument_restart(engine, track) {
                notices.push(n);
            }
            let closed = self
                .instruments
                .get(&track)
                .and_then(|h| h.window.as_ref())
                .is_some_and(|w| w.closed());
            if closed && self.close_instrument_window(track) {
                notices.push(HostNotice::InstrumentWindowClosed(track));
            }
            let rate = self.setup.sample_rate;
            if let Some(h) = self.instruments.get_mut(&track) {
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
    }

    /// A track's instrument stopped working: marked, counted as its blame says, its window
    /// gone, told once.
    fn on_instrument_crash(
        &mut self,
        track: TrackId,
        reason: String,
        blame: crate::backend::Blame,
    ) -> Option<HostNotice> {
        let h = self.instruments.get_mut(&track)?;
        if h.crashed.is_some() {
            return None;
        }
        h.crashed = Some(reason.clone());
        h.blame = Some(blame);
        h.restarting = false;
        if let Some(w) = h.window.take() {
            #[cfg_attr(not(windows), allow(clippy::drop_non_drop))]
            drop(w);
        }
        let info = h.info.clone();
        let pid = h.main.isolated_in();
        self.count_crash(&info, blame, pid);
        Some(HostNotice::InstrumentCrashed {
            track,
            name: info.name,
            reason,
        })
    }

    /// One thing an instrument's window did, as a command: a value inside a gesture merges
    /// into one undo step with the rest of the gesture.
    pub fn record_instrument(&mut self, bus: &mut CommandBus, track: TrackId, event: FxParamEvent) {
        let Some(h) = self.instruments.get_mut(&track) else {
            return;
        };
        match event {
            FxParamEvent::Begin(id) => {
                if !h.gestures.contains(&id) {
                    h.gestures.push(id);
                }
            }
            FxParamEvent::Value(id, v) => {
                h.flushed.insert(id, v);
                let in_gesture = h.gestures.contains(&id);
                let current = bus
                    .project()
                    .track(track)
                    .and_then(|t| t.instrument.external())
                    .and_then(|p| p.params.get(&id).copied());
                if current.map(f64::to_bits) == Some(v.to_bits()) {
                    return;
                }
                let cmd = Command::SetInstrumentPluginParam {
                    track,
                    id,
                    value: Some(v),
                };
                let result = if in_gesture {
                    bus.execute_merging(
                        cmd.clone(),
                        Origin::User,
                        MergeKey::of(&("instrument-plugin", track.0, id)),
                    )
                } else {
                    bus.execute(cmd.clone(), Origin::User)
                };
                match result {
                    Ok(_) => self.tweaked(cmd),
                    Err(e) => {
                        if !self.instrument_refused_by_freeze(bus.project(), track, &e) {
                            log::warn!("instrument parameter {id}: {e}");
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

    /// A track's instrument's state and values, read now.
    pub fn capture_instrument(
        &mut self,
        track: TrackId,
    ) -> Option<(PluginState, BTreeMap<u32, f64>)> {
        let h = self.instruments.get_mut(&track)?;
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

    /// The plug-in changed its values by itself (its own preset browser): its state and
    /// values become one step.
    fn capture_instrument_changes(&mut self, bus: &mut CommandBus, track: TrackId) {
        let Some(same) = bus
            .project()
            .track(track)
            .and_then(|t| t.instrument.external())
            .map(|p| p.params.clone())
        else {
            return;
        };
        let Some((state, params)) = self.capture_instrument(track) else {
            return;
        };
        if same == params {
            return;
        }
        if let Some(h) = self.instruments.get_mut(&track) {
            h.synced_state = state.clone();
            h.flushed = params.clone();
        }
        let cmd = Command::SetInstrumentPluginState {
            track,
            state,
            params,
        };
        if let Err(e) = bus.execute(cmd, Origin::User) {
            if !self.instrument_refused_by_freeze(bus.project(), track, &e) {
                log::warn!("instrument state: {e}");
            }
        }
    }

    /// A window's change to a track's instrument a freeze refused: given back what the
    /// project holds, and said (review 2).
    fn instrument_refused_by_freeze(
        &mut self,
        project: &Project,
        track: TrackId,
        error: &cantus_core::CommandError,
    ) -> bool {
        let cantus_core::CommandError::Frozen(target) = error else {
            return false;
        };
        let plugin = project.track(track).and_then(|t| t.instrument.external());
        if let (Some(h), Some(plugin)) = (self.instruments.get_mut(&track), plugin) {
            if !plugin.state.is_empty() {
                if let Err(e) = h.main.load_state(plugin.state.bytes()) {
                    log::warn!("{}: {e}", h.info.name);
                }
            }
            let mut out = ParamOut::new();
            h.main
                .flush(&crate::session::changes(&plugin.params), &mut out);
            h.flushed = plugin.params.clone();
            h.synced_state = plugin.state.clone();
        }
        self.frozen_refusals.push(*target);
        true
    }

    /// Take the plug-in's unit out of the engine (a silent unit stands in) so it can be
    /// deactivated and activated again.
    fn begin_instrument_restart(&mut self, engine: &mut EngineHandle, track: TrackId) {
        let Some(h) = self.instruments.get_mut(&track) else {
            return;
        };
        if h.restarting {
            return;
        }
        h.restarting = true;
        if h.audio.take().is_some() {
            return; // never delivered: dropped now, so nothing waits on it
        }
        let kind = UnitKind::Hosted(h.key);
        if engine.knows_instrument(track, kind) {
            engine.replace_instrument(NativeUnit::hosted(track, h.key, Box::new(Silent)));
        }
    }

    fn continue_instrument_restart(
        &mut self,
        engine: &mut EngineHandle,
        track: TrackId,
    ) -> Option<HostNotice> {
        let setup = self.setup;
        let mut h = self.instruments.remove(&track)?;
        if !h.restarting {
            self.instruments.insert(track, h);
            return None;
        }
        let audio = match h.main.restart() {
            Ok(None) => {
                // Its audio half is still on its way back.
                self.instruments.insert(track, h);
                return None;
            }
            Ok(Some(audio)) => audio,
            Err(HostError::Unsupported(_)) => {
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
                        self.instruments.insert(track, h);
                        return Some(HostNotice::InstrumentRestartFailed {
                            track,
                            error: error.why,
                        });
                    }
                }
            }
            // It stopped working: the crash is told by `idle`; the track stays silent.
            Err(HostError::Crashed(_)) => {
                h.restarting = false;
                self.instruments.insert(track, h);
                return None;
            }
            Err(e) => {
                h.restarting = false;
                self.instruments.insert(track, h);
                return Some(HostNotice::InstrumentRestartFailed {
                    track,
                    error: e.to_string(),
                });
            }
        };
        h.restarting = false;
        h.generation.fetch_add(1, Ordering::AcqRel);
        let unit = super::inst::make_unit(&mut h, track, audio, setup.sample_rate);
        super::inst::deliver(engine, unit);
        self.instruments.insert(track, h);
        None
    }

    /// Whether a track's instrument is between being taken out of the engine and put back.
    pub fn instrument_restarting(&self, track: TrackId) -> bool {
        self.instruments.get(&track).is_some_and(|h| h.restarting)
    }

    /// Every loaded instrument's state as it is now, into `project` (for saving).
    pub(crate) fn instruments_for_save(
        &mut self,
        project: &mut Project,
        locks: Option<&cantus_core::freeze::Locks>,
    ) {
        for t in &mut project.tracks {
            let Some(h) = self.instruments.get_mut(&t.id) else {
                continue;
            };
            // In a frozen region: the state its render holds (review 2).
            if locks.is_some_and(|l| l.source.contains(&t.id)) {
                continue;
            }
            let Some(plugin) = t.instrument.external_mut() else {
                continue;
            };
            match h.main.save_state() {
                Ok(bytes) => plugin.state = PluginState::new(bytes),
                Err(e) => log::warn!("{}: {e}", h.info.name),
            }
        }
    }

    /// Fresh instances of every loaded instrument in `project` for an offline render at
    /// `sample_rate`, each with the live one's state: the units to hand to
    /// `cantus_engine::render_with_instruments`. Their main halves stay here until
    /// [`Self::finish_render`].
    pub fn render_instruments(&mut self, project: &Project, sample_rate: f64) -> Vec<NativeUnit> {
        self.render_instruments_as(project, sample_rate, false)
    }

    /// [`Self::render_instruments`] with the states `project` holds (a freeze's render).
    pub fn render_instruments_stored(
        &mut self,
        project: &Project,
        sample_rate: f64,
    ) -> Vec<NativeUnit> {
        self.render_instruments_as(project, sample_rate, true)
    }

    fn render_instruments_as(
        &mut self,
        project: &Project,
        sample_rate: f64,
        stored: bool,
    ) -> Vec<NativeUnit> {
        self.fresh_render_cancel();
        let setup = AudioSetup {
            sample_rate,
            max_block: MAX_BLOCK_FRAMES,
        };
        let mut units = Vec::new();
        for t in &project.tracks {
            let Some(plugin) = t.instrument.external() else {
                continue;
            };
            let Some(h) = self.instruments.get_mut(&t.id) else {
                continue;
            };
            let state = if stored {
                plugin.state.clone()
            } else {
                h.main
                    .save_state()
                    .map(PluginState::new)
                    .unwrap_or_default()
            };
            let info = h.info.clone();
            let key = h.key;
            // Where it runs live; isolated, every block is waited for (ADR 0063).
            let isolation = Some(h.isolation);
            let Ok(inst) = self.instantiate(&info, isolation, &state, &plugin.params, setup, true)
            else {
                continue;
            };
            let (main, audio) = (inst.main, inst.audio);
            let params = main.params();
            let unit = HostedInstrument::new(
                audio,
                &params,
                |id| main.param_value(id),
                main.latency(),
                sample_rate,
                Arc::new(AtomicU64::new(0)),
            );
            units.push(NativeUnit::hosted(t.id, key, Box::new(unit)));
            self.render_mains.push(main);
        }
        units
    }

    /// Open a track's instrument's own editor in a window of its own, owned by `owner`.
    pub fn open_instrument_window(
        &mut self,
        track: TrackId,
        title: &str,
        owner: Option<raw_window_handle::RawWindowHandle>,
    ) -> Result<(), HostError> {
        let h = self
            .instruments
            .get_mut(&track)
            .ok_or(HostError::Unsupported("no plug-in on this track"))?;
        if let Some(w) = h.window.as_ref() {
            if !h.main.focus_editor() {
                w.focus();
            }
            return Ok(());
        }
        if let Some(why) = h.crashed.clone() {
            return Err(HostError::Crashed(why));
        }
        let window = PluginWindow::open(title, owner, h.main.as_mut())?;
        // An isolated plug-in's window is its helper's: brought to the front there.
        h.main.focus_editor();
        h.window = Some(window);
        Ok(())
    }

    /// Close a track's instrument window; `true` if one was open.
    pub fn close_instrument_window(&mut self, track: TrackId) -> bool {
        let Some(h) = self.instruments.get_mut(&track) else {
            return false;
        };
        let open = h.window.is_some();
        h.close_window_now();
        open
    }

    /// Whether `track`'s instrument window is open.
    pub fn instrument_window_open(&self, track: TrackId) -> bool {
        self.instruments
            .get(&track)
            .is_some_and(HostedInst::window_open)
    }

    /// Whether `track`'s instrument window is open but minimised to the taskbar: F8 and
    /// the rack's channel button bring it back rather than close it (round 27, ADR 0051).
    pub fn instrument_window_minimised(&self, track: TrackId) -> bool {
        self.instruments
            .get(&track)
            .is_some_and(HostedInst::window_minimised)
    }

    /// Whether `track`'s instrument window is open and in front: F8 and the rack's button
    /// close only that one, and bring any other back (round 27, ADR 0051).
    pub fn instrument_window_frontmost(&self, track: TrackId) -> bool {
        self.instruments
            .get(&track)
            .is_some_and(HostedInst::window_frontmost)
    }
}

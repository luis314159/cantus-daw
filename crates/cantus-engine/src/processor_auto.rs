//! Automation and modulation on the audio thread (ADR 0027, ADR 0057;
//! `crate::automation`, `crate::modulator`): when a control point is due, what it writes,
//! the ramps the mix loops read between two of them, and the timed changes a hosted
//! plug-in hears inside a pass.
//!
//! **The null rule.** Without a runtime nothing here runs: no segment is split, no ramp
//! exists, the master's values are the snapshot's. A gain or pan whose curve holds still
//! has no ramp either, and takes the old constant path.
//!
//! **Playing**, control points sit on the 16-sample grid counted from the transport's
//! origin; a pass is cut at the next one while a native destination moves (or a modulator
//! follows audio, whose windows must be whole), never for a hosted plug-in, which hears
//! every grid point of the pass at its offset. **Stopped**, the lanes hold where the
//! playhead is and the modulators keep running for what is played live: a control point
//! at every pass, its gains ramped over the pass.

use super::*;
use crate::automation::{At, Dest, Ramp, Runtime, GRID};
use crate::fx::HostedValue;
use crate::modulator::{Ear, ModRt};
use cantus_core::db_to_linear;
use cantus_core::modulator::eval::ModTime;
use cantus_core::ModulatorId;
use cantus_core::TempoMap;

/// The processor's automation state. Fixed size; the runtime arrives built.
pub(super) struct AutoState {
    pub(super) rt: Option<Box<Runtime>>,
    /// A runtime that arrived ahead of its snapshot.
    pending: Option<Box<Runtime>>,
    /// The transport sample at which the next control point is due (`u64::MAX`: none).
    next: u64,
    /// The transport sample of the last control point: ramps count from it.
    pub(super) at: u64,
    /// Track and strip slots that may hold a ramp (the tracks' level and balance, the
    /// strips' fader and pan live in their nodes' state): cleared at the next install.
    ramped_tracks: [bool; MAX_TRACKS],
    ramped_strips: [bool; MAX_STRIPS],
    pub(super) master_gain: Option<Ramp>,
    pub(super) master_pan: Option<Ramp>,
    /// The master's level (linear), balance and width while automation drives them.
    pub(super) master: [Option<f32>; 3],
    /// The next grid point (transport samples) whose hosted values are not sent yet.
    hosted_next: u64,
    /// Playing at the last pass: a change is a start or a stop.
    was_playing: bool,
    /// The engine clock where playback last started, and where it last stopped.
    play_clock: u64,
    stop_clock: u64,
    /// The song position (beats) where playback stopped: stopped, the modulators run on
    /// from here.
    stop_beats: f64,
    /// The engine clock of the last control point (what stateful modulators stepped to).
    mods_clock: u64,
    /// The master's output peak since the last control point (a follower may hear it).
    pub(super) follow_master: f32,
    /// Each modulator's listened peak at a control point (scratch, fixed size).
    peaks: Vec<f32>,
    /// The block the modulators' live values were last reported in.
    told: u64,
    /// An offline trace of one modulator's output × amount at every control point, for
    /// *Bake to automation* (ADR 0057): room made before the render, never grown.
    pub(super) trace: Option<(ModulatorId, Vec<(f64, f32)>)>,
}

/// Modulators a snapshot may play at once with their audio heard (more are heard as
/// silence): the peaks scratch is fixed.
const MAX_EARS: usize = 256;

impl AutoState {
    pub(super) fn new() -> Self {
        AutoState {
            rt: None,
            pending: None,
            next: u64::MAX,
            at: 0,
            ramped_tracks: [false; MAX_TRACKS],
            ramped_strips: [false; MAX_STRIPS],
            master_gain: None,
            master_pan: None,
            master: [None; 3],
            hosted_next: u64::MAX,
            was_playing: false,
            play_clock: 0,
            stop_clock: 0,
            stop_beats: 0.0,
            mods_clock: 0,
            follow_master: 0.0,
            peaks: vec![0.0; MAX_EARS],
            told: u64::MAX,
            trace: None,
        }
    }

    /// Every ramp goes: the nodes' (in their state) and the master's.
    fn clear_ramps(&mut self, nodes: &exec::Nodes) {
        for (slot, r) in self.ramped_tracks.iter_mut().enumerate() {
            if std::mem::take(r) {
                if let Some(mut s) = nodes.track(slot) {
                    s.gain_ramp = None;
                    s.pan_ramp = None;
                }
            }
        }
        for (slot, r) in self.ramped_strips.iter_mut().enumerate() {
            if std::mem::take(r) {
                if let Some(mut s) = nodes.strip(slot) {
                    s.gain_ramp = None;
                    s.pan_ramp = None;
                }
            }
        }
        self.master_gain = None;
        self.master_pan = None;
    }

    /// Samples since the last control point, for a segment starting at transport sample
    /// `now` — `None` when nothing ramps (no runtime).
    #[inline]
    pub(super) fn k0(&self, now: u64) -> Option<u64> {
        self.rt.as_ref().map(|_| now.saturating_sub(self.at))
    }

    /// The last control point, when a runtime is installed (the passes' `auto_at`).
    #[inline]
    pub(super) fn base(&self) -> Option<u64> {
        self.rt.as_ref().map(|_| self.at)
    }
}

/// Multiply a track's segment by its automated level and balance, sample by sample.
/// Returns `false` (touching nothing) when neither ramps.
#[inline]
pub(crate) fn ramp_track(s: &TrackSlot, k0: u64, l: &mut [f32], r: &mut [f32]) -> bool {
    let (g, p) = (s.gain_ramp, s.pan_ramp);
    if g.is_none() && p.is_none() {
        return false;
    }
    for (i, (x, y)) in l.iter_mut().zip(r.iter_mut()).enumerate() {
        let k = k0 + i as u64;
        let gain = if s.audible {
            g.map_or(s.gain, |g| g.at(k))
        } else {
            0.0
        };
        let (bl, br) = balance_gains(p.map_or(s.pan, |p| p.at(k)));
        *x *= gain * bl;
        *y *= gain * br;
    }
    true
}

/// Multiply a strip's segment by its automated fader and pan (through the pan law).
/// Returns `false` (touching nothing) when neither ramps.
#[inline]
pub(crate) fn ramp_strip(
    st: &StripSlot,
    law: PanLaw,
    k0: u64,
    l: &mut [f32],
    r: &mut [f32],
) -> bool {
    let (g, p) = (st.gain_ramp, st.pan_ramp);
    if g.is_none() && p.is_none() {
        return false;
    }
    for (i, (x, y)) in l.iter_mut().zip(r.iter_mut()).enumerate() {
        let k = k0 + i as u64;
        let gain = g.map_or(st.gain, |g| g.at(k));
        let (pl, pr) = law.gains(p.map_or(st.pan, |p| p.at(k)));
        *x *= gain * pl;
        *y *= gain * pr;
    }
    true
}

impl Processor {
    /// A runtime for the next snapshot: it waits until that snapshot is installed.
    pub(super) fn auto_arrived(&mut self, rt: Option<Box<Runtime>>) {
        if let Some(old) = std::mem::replace(&mut self.auto.pending, rt) {
            self.auto_dispose(old);
        }
    }

    fn auto_dispose(&mut self, rt: Box<Runtime>) {
        if self.tx.push(EngineEvent::GarbageAutomation(rt)).is_err() {
            self.stats.dropped_events += 1;
        }
    }

    /// Pair the snapshot being installed with its runtime (the snapshot's own values have
    /// just been applied everywhere, so every ramp and override goes), find each automated
    /// effect's unit, carry the modulators' state over from the last runtime, and tell
    /// every hosted unit that its automation was planned again. The first control point
    /// is due at once (`reseek_events`).
    pub(super) fn auto_install(&mut self, snapshot: &Arc<Snapshot>) {
        let rt = match self.auto.pending.take() {
            Some(rt) if rt.is_for(snapshot) => Some(rt),
            Some(stale) => {
                self.auto_dispose(stale);
                None
            }
            None => None,
        };
        if rt.is_none() && !(snapshot.automation.is_empty() && snapshot.modulators.is_empty()) {
            // Sent without its runtime: it plays, unautomated.
            self.stats.dropped_events += 1;
        }
        let old = std::mem::replace(&mut self.auto.rt, rt);
        let nodes = self.nodes.clone();
        // Every hosted unit, automated or not any more: what it is not sent from the next
        // control point on is the project's again.
        for si in 0..MAX_STRIPS {
            if self.strip_ids[si].is_none() {
                continue;
            }
            if let Some(mut s) = nodes.strip(si) {
                s.follow_peak = 0.0;
                for e in s.effects.iter_mut() {
                    e.automation_replanned();
                }
            }
        }
        for e in self.master.effects.iter_mut() {
            e.automation_replanned();
        }
        for slot in 0..MAX_TRACKS {
            if self.track_ids[slot].is_none() {
                continue;
            }
            if let Some(mut s) = nodes.track(slot) {
                if let Some(u) = s.native.as_mut() {
                    u.automation.clear();
                    u.unit.automation_replanned();
                }
            }
        }
        self.auto.follow_master = 0.0;
        if let Some(mut rt) = old {
            if let Some(new) = self.auto.rt.as_mut() {
                for m in new.mods.iter_mut() {
                    if let Some(o) = rt.mods.iter().find(|o| o.id == m.id) {
                        m.carry(o);
                    }
                }
            }
            rt.mods.iter_mut().for_each(|m| m.notes_slot = None);
            self.auto_dispose(rt);
        }
        self.auto.clear_ramps(&nodes);
        self.auto.master = [None; 3];
        self.auto.next = u64::MAX;
        self.auto.hosted_next = u64::MAX;
        if let Some(rt) = self.auto.rt.as_mut() {
            let find = |effect: EffectId| {
                (0..MAX_STRIPS)
                    .filter(|si| self.strip_ids[*si].is_some())
                    .find_map(|si| {
                        let s = nodes.strip(si)?;
                        s.effects
                            .iter()
                            .position(|e| e.id == effect)
                            .map(|i| (si, i))
                    })
                    .or_else(|| {
                        self.master
                            .effects
                            .iter()
                            .position(|e| e.id == effect)
                            .map(|i| (usize::MAX, i))
                    })
            };
            for c in rt.effects.iter_mut() {
                c.at = find(c.effect);
            }
            for h in rt.hosted.iter_mut() {
                h.at = find(h.effect);
            }
            for (m, plan) in rt.mods.iter_mut().zip(&snapshot.modulators) {
                m.notes_slot = plan
                    .notes
                    .and_then(|ti| self.track_to_slot.get(ti).copied());
                m.ear = plan.listen.and_then(|l| Ear::of(l, &self.strip_to_slot));
            }
        }
    }

    /// Trace modulator `id`'s output × amount at every control point of the coming
    /// (offline) render, room for `capacity` points; `None` stops. Allocates: call it before
    /// the render, never on the audio thread.
    pub fn trace_modulator(&mut self, id: Option<ModulatorId>, capacity: usize) {
        self.auto.trace = id.map(|id| (id, Vec::with_capacity(capacity)));
    }

    /// The trace so far, taken.
    pub fn take_modulator_trace(&mut self) -> Vec<(f64, f32)> {
        self.auto
            .trace
            .as_mut()
            .map(|(_, t)| std::mem::take(t))
            .unwrap_or_default()
    }

    /// The playhead jumped (a start, seek, loop wrap, a new snapshot): a control point is
    /// due now.
    pub(super) fn auto_reseek(&mut self) {
        if self.auto.rt.is_some() {
            self.auto.next = self.transport.samples();
            self.auto.hosted_next = self.transport.samples();
        }
    }

    /// The song position in beats now, as the modulators read it (stopped: where playback
    /// stopped, moving on at the tempo from there).
    fn auto_beat(&self, clock: u64) -> f64 {
        if self.transport.is_playing() {
            self.transport.position_f64() / cantus_core::PPQ as f64
        } else {
            self.auto_time_stopped(clock, self.transport.map()).beats
        }
    }

    fn spb(&self) -> f64 {
        60.0 / self.transport.bpm().max(1e-9)
    }

    /// Time as the modulators read it at transport sample `s`, `clock` the engine's clock
    /// there, while playing — through `map` when the tempo changes along the song (ADR
    /// 0060: the tempo at that sample, beats to seconds through the map).
    fn auto_time<'m>(&self, s: u64, clock: u64, map: &'m TempoMap) -> ModTime<'m> {
        let tick = self.transport.position_at(s);
        let moving = !map.is_constant();
        ModTime {
            beats: tick / cantus_core::PPQ as f64,
            since_play: clock.saturating_sub(self.auto.play_clock) as f64 / self.sample_rate,
            spb: if moving {
                60.0 / map.bpm_at(tick).max(1e-9)
            } else {
                self.spb()
            },
            playing: true,
            map: moving.then_some(map),
        }
    }

    /// Time as the modulators read it at engine clock `clock`, stopped: the song moves on
    /// from where it stopped at the tempo there (through `map` when it changes).
    fn auto_time_stopped<'m>(&self, clock: u64, map: &'m TempoMap) -> ModTime<'m> {
        let secs = clock.saturating_sub(self.auto.stop_clock) as f64 / self.sample_rate;
        if !map.is_constant() {
            let ppq = cantus_core::PPQ as f64;
            let tick = map.tick_after_seconds(self.auto.stop_beats * ppq, secs);
            return ModTime {
                beats: tick / ppq,
                since_play: secs,
                spb: 60.0 / map.bpm_at(tick).max(1e-9),
                playing: false,
                map: Some(map),
            };
        }
        let spb = self.spb();
        ModTime {
            beats: self.auto.stop_beats + secs / spb,
            since_play: secs,
            spb,
            playing: false,
            map: None,
        }
    }

    /// A live note on track slot `slot` (the keyboard, MIDI): the modulators hearing that
    /// track's notes open their envelopes and retrigger their LFOs.
    pub(super) fn auto_live_note(&mut self, slot: usize, on: bool) {
        let beat = self.auto_beat(self.clock);
        let (Some(rt), Some(snap)) = (self.auto.rt.as_mut(), self.snapshot.as_ref()) else {
            return;
        };
        for (m, plan) in rt.mods.iter_mut().zip(&snap.modulators) {
            if m.notes_slot == Some(slot) {
                m.live_note(plan, on, beat);
            }
        }
    }

    /// Every note was let go (a stop, a seek, a wrap, a panic).
    pub(super) fn auto_live_release(&mut self) {
        let (Some(rt), Some(snap)) = (self.auto.rt.as_mut(), self.snapshot.as_ref()) else {
            return;
        };
        for (m, plan) in rt.mods.iter_mut().zip(&snap.modulators) {
            m.live_release(plan);
        }
    }

    /// At the start of a pass `[done, …)` of the block: notice a start or a stop, empty the
    /// hosted units' queues, run the control point if one is due. Then cap the pass at the
    /// next control point, so it lands on its sample whatever the block size.
    pub(super) fn auto_point(&mut self, done: usize, seg_end: usize) -> usize {
        if self.auto.rt.is_none() {
            return seg_end;
        }
        let clock = self.clock + done as u64;
        let playing = self.transport.is_playing();
        if playing != self.auto.was_playing {
            self.auto.was_playing = playing;
            if playing {
                // Playback starts: free modulators from here, state from rest.
                self.auto.play_clock = clock;
                if let Some(rt) = self.auto.rt.as_mut() {
                    rt.mods.iter_mut().for_each(ModRt::restart);
                }
            } else {
                self.auto.stop_clock = clock;
                self.auto.stop_beats = self.transport.position_f64() / cantus_core::PPQ as f64;
            }
            self.auto.mods_clock = clock;
            self.auto.next = self.transport.samples();
        }
        self.auto_clear_queues();
        let now = self.transport.samples();
        let has_mods = self.auto.rt.as_ref().is_some_and(|rt| !rt.mods.is_empty());
        if !playing {
            if has_mods {
                // Stopped, the modulators run on: a control point every pass, its gains
                // ramped over the pass.
                let end = seg_end.min(done + crate::graph::PASS_FRAMES);
                self.auto_evaluate(now, clock, (end - done) as u64);
            } else if now >= self.auto.next {
                self.auto_evaluate(now, clock, GRID);
            }
            return seg_end;
        }
        if now >= self.auto.next {
            self.auto_evaluate(now, clock, (now / GRID + 1) * GRID - now);
        }
        if self.auto.next == u64::MAX {
            return seg_end;
        }
        let until = self.auto.next.saturating_sub(now);
        seg_end.min(done + until.min(MAX_BLOCK_FRAMES as u64) as usize)
    }

    /// Every hosted unit's queue emptied for the pass about to run.
    fn auto_clear_queues(&mut self) {
        let Some(rt) = self.auto.rt.as_ref().filter(|rt| rt.any_hosted) else {
            return;
        };
        let nodes = self.nodes.clone();
        for h in &rt.hosted {
            match h.at {
                Some((usize::MAX, i)) => {
                    if let Some(u) = self.master.effects.get_mut(i) {
                        u.automation.clear();
                        // What found no room on the way to the plug-in is never silent.
                        self.stats.dropped_events += u.take_dropped();
                    }
                }
                Some((s, i)) => {
                    if let Some(mut st) = nodes.strip(s) {
                        if let Some(u) = st.effects.get_mut(i) {
                            u.automation.clear();
                            self.stats.dropped_events += u.take_dropped();
                        }
                    }
                }
                None => {}
            }
        }
        for l in &rt.lanes {
            if let Dest::HostedInst(ti, _) = l.dest {
                if let Some(mut s) = self.track_to_slot.get(ti).and_then(|&sl| nodes.track(sl)) {
                    if let Some(u) = s.native.as_mut() {
                        u.automation.clear();
                        self.stats.dropped_events +=
                            u.automation.take_dropped() + u.unit.take_dropped();
                    }
                }
            }
        }
    }

    /// Push one hosted value `offset` samples into the pass; `at` is where a hosted
    /// insert runs (`HostedFx::at`). Whether its unit's queue took it.
    fn auto_push_hosted(
        &mut self,
        dest: &Dest,
        at: Option<(usize, usize)>,
        offset: u32,
        v: HostedValue,
    ) -> bool {
        let nodes = self.nodes.clone();
        match *dest {
            Dest::Hosted(_, id) => match at {
                Some((usize::MAX, i)) => self
                    .master
                    .effects
                    .get_mut(i)
                    .is_some_and(|u| u.automation.push(offset, id, v)),
                Some((s, i)) => nodes.strip(s).is_some_and(|mut st| {
                    st.effects
                        .get_mut(i)
                        .is_some_and(|u| u.automation.push(offset, id, v))
                }),
                None => false,
            },
            Dest::HostedInst(ti, id) => self
                .track_to_slot
                .get(ti)
                .and_then(|&sl| nodes.track(sl))
                .is_some_and(|mut s| {
                    s.native
                        .as_mut()
                        .is_some_and(|u| u.automation.push(offset, id, v))
                }),
            _ => false,
        }
    }

    /// A hosted unit was swapped in place (`replace_effect` / `replace_instrument`: a
    /// plug-in's restart). The fresh unit holds none of the values queued for the old one,
    /// so every hosted lane is queued again at the next grid point; stopped, a control
    /// point is due now (stopped there are no inner grid points). Review 2.
    pub(super) fn auto_unit_swapped(&mut self) {
        if let Some(rt) = self.auto.rt.as_mut() {
            rt.forget_sent();
            if !self.transport.is_playing() {
                self.auto_reseek();
            }
        }
    }

    /// The hosted values at every grid point inside the pass `[done, done + n)` not sent
    /// yet, at their offsets (playing only: stopped, the control point at each pass sends
    /// them).
    pub(super) fn auto_hosted(&mut self, done: usize, n: usize) {
        if !self.transport.is_playing() {
            return;
        }
        let Some(mut rt) = self.auto.rt.take() else {
            return;
        };
        if let (true, Some(snap)) = (rt.any_hosted, self.snapshot.clone()) {
            let now = self.transport.samples();
            let end = now + n as u64;
            while self.auto.hosted_next < end {
                let g = self.auto.hosted_next;
                self.auto.hosted_next = (g / GRID + 1) * GRID;
                if g < now {
                    continue;
                }
                let tick = self.transport.position_at(g);
                let clock = self.clock + done as u64 + (g - now);
                self.auto_step_live(&mut rt, &snap, clock);
                let t = self.auto_time(g, clock, &snap.tempo);
                let offset = (g - now) as u32;
                rt.hosted_at(&snap, tick, &t, |dest, at, v| {
                    self.auto_push_hosted(dest, at, offset, v)
                });
            }
        }
        self.auto.rt = Some(rt);
    }

    /// Step the envelopes live notes open to engine clock `clock`, at a hosted inner grid
    /// point: a key played now is heard on a plug-in's parameter at the grid's pace. (What
    /// hears audio makes every grid point a control point: no inner points then.)
    fn auto_step_live(&mut self, rt: &mut Runtime, snap: &Snapshot, clock: u64) {
        if rt.audio_mods {
            return;
        }
        let dt = clock.saturating_sub(self.auto.mods_clock) as f64 / self.sample_rate;
        if dt <= 0.0 {
            return;
        }
        self.auto.mods_clock = clock;
        for (m, plan) in rt.mods.iter_mut().zip(&snap.modulators) {
            if let cantus_core::ModSource::Envelope(e) = &plan.source {
                if matches!(e.trigger, cantus_core::modulator::EnvTrigger::Notes { .. }) {
                    m.env.step(e, dt);
                }
            }
        }
    }

    /// Step the stateful modulators to engine clock `clock`, with what their ears heard
    /// since the last control point.
    fn auto_step_mods(&mut self, rt: &mut Runtime, snap: &Snapshot, clock: u64) {
        let dt = clock.saturating_sub(self.auto.mods_clock) as f64 / self.sample_rate;
        self.auto.mods_clock = clock;
        if rt.mods.is_empty() {
            return;
        }
        let nodes = self.nodes.clone();
        // Read every ear first, then empty them: two modulators may hear one strip.
        for (k, m) in rt.mods.iter().enumerate().take(MAX_EARS) {
            self.auto.peaks[k] = match m.ear {
                Some(Ear::Strip(slot)) => nodes.strip(slot).map_or(0.0, |s| s.follow_peak),
                Some(Ear::Master) => self.auto.follow_master,
                None => 0.0,
            };
        }
        for m in rt.mods.iter() {
            match m.ear {
                Some(Ear::Strip(slot)) => {
                    if let Some(mut s) = nodes.strip(slot) {
                        s.follow_peak = 0.0;
                    }
                }
                Some(Ear::Master) => self.auto.follow_master = 0.0,
                None => {}
            }
        }
        if dt <= 0.0 {
            return;
        }
        for (k, (m, plan)) in rt.mods.iter_mut().zip(&snap.modulators).enumerate() {
            let peak = self.auto.peaks.get(k).copied().unwrap_or(0.0);
            m.step(plan, dt, peak);
        }
    }

    /// A control point at transport sample `now` (engine clock `clock`): every lane and
    /// modulator evaluated there and `len` samples on (the next control point), the values
    /// written where they go.
    fn auto_evaluate(&mut self, now: u64, clock: u64, len: u64) {
        let (Some(mut rt), Some(snap)) = (self.auto.rt.take(), self.snapshot.clone()) else {
            return;
        };
        let playing = self.transport.is_playing();
        self.auto_step_mods(&mut rt, &snap, clock);
        let tick = self.transport.position_at(now);
        let grid = now + len;
        let at = if playing {
            At {
                tick,
                next_tick: self.transport.position_at(grid),
                t: self.auto_time(now, clock, &snap.tempo),
                t_next: self.auto_time(grid, clock + len, &snap.tempo),
            }
        } else {
            // Stopped, the lanes hold at the playhead; the modulators run on.
            At {
                tick,
                next_tick: tick,
                t: self.auto_time_stopped(clock, &snap.tempo),
                t_next: self.auto_time_stopped(clock + len, &snap.tempo),
            }
        };
        rt.evaluate(&snap, &at);
        self.auto.at = now;
        // The trace (an offline bake) and the live values (once a block).
        if let Some((id, trace)) = self.auto.trace.as_mut() {
            if let Some(m) = rt.mods.iter().find(|m| m.id == *id) {
                if trace.len() < trace.capacity() {
                    trace.push((tick, m.now * m.amount_now));
                }
            }
        }
        if self.auto.told != self.block && !rt.mods.is_empty() {
            self.auto.told = self.block;
            let mut t = crate::modulator::ModTelemetry {
                count: rt.mods.len().min(crate::modulator::MOD_TELEMETRY),
                ..Default::default()
            };
            for (k, m) in rt
                .mods
                .iter()
                .take(crate::modulator::MOD_TELEMETRY)
                .enumerate()
            {
                t.ids[k] = m.id.0;
                t.values[k] = m.now * m.amount_now;
            }
            if self.tx.push(EngineEvent::Modulation(t)).is_err() {
                self.stats.dropped_events += 1;
            }
        }
        let nodes = self.nodes.clone();
        let mut moving = false;
        for li in 0..rt.lanes.len() {
            let lane = &rt.lanes[li];
            let (a, b) = (lane.now, lane.next);
            if !lane.dest.is_hosted() {
                moving |= a != b;
            }
            match lane.dest {
                Dest::TrackGain(ti) => {
                    if let Some(&slot) = self.track_to_slot.get(ti) {
                        let (ga, gb) = (db_to_linear(a), db_to_linear(b));
                        if let Some(mut s) = nodes.track(slot) {
                            s.gain = ga;
                            s.gain_ramp = Ramp::of(ga, gb, len);
                            self.auto.ramped_tracks[slot] = true;
                        }
                    }
                }
                Dest::TrackPan(ti) => {
                    if let Some(&slot) = self.track_to_slot.get(ti) {
                        if let Some(mut s) = nodes.track(slot) {
                            s.pan = a;
                            s.pan_ramp = Ramp::of(a, b, len);
                            self.auto.ramped_tracks[slot] = true;
                        }
                    }
                }
                Dest::StripGain(si) => {
                    if let Some(&slot) = self.strip_to_slot.get(si) {
                        let (ga, gb) = (db_to_linear(a), db_to_linear(b));
                        if let Some(mut s) = nodes.strip(slot) {
                            s.gain = ga;
                            s.gain_ramp = Ramp::of(ga, gb, len);
                            self.auto.ramped_strips[slot] = true;
                        }
                    }
                }
                Dest::StripPan(si) => {
                    if let Some(&slot) = self.strip_to_slot.get(si) {
                        if let Some(mut s) = nodes.strip(slot) {
                            s.pan = a;
                            s.pan_ramp = Ramp::of(a, b, len);
                            self.auto.ramped_strips[slot] = true;
                        }
                    }
                }
                Dest::StripWidth(si) => {
                    if let Some(&slot) = self.strip_to_slot.get(si) {
                        if let Some(mut s) = nodes.strip(slot) {
                            s.width = a;
                        }
                    }
                }
                Dest::Send { strip, to } => {
                    if let (Some(&s), Some(&d)) =
                        (self.strip_to_slot.get(strip), self.strip_to_slot.get(to))
                    {
                        if let Some(mut st) = nodes.strip(s) {
                            for k in 0..st.send_count {
                                if st.sends[k].0 == d {
                                    st.sends[k].1 = a;
                                }
                            }
                        }
                    }
                }
                Dest::MasterGain => {
                    let (ga, gb) = (db_to_linear(a), db_to_linear(b));
                    self.auto.master[0] = Some(ga);
                    self.auto.master_gain = Ramp::of(ga, gb, len);
                }
                Dest::MasterPan => {
                    self.auto.master[1] = Some(a);
                    self.auto.master_pan = Ramp::of(a, b, len);
                }
                Dest::MasterWidth => self.auto.master[2] = Some(a),
                Dest::Hosted(h, _) | Dest::HostedInst(h, _) => {
                    let dest = lane.dest.clone();
                    let at = match dest {
                        Dest::Hosted(..) => rt.hosted.get(h).and_then(|x| x.at),
                        _ => None,
                    };
                    let v = HostedValue {
                        pos: lane.now,
                        modulation: lane.mod_now,
                    };
                    // Marked as sent only when it found room (review 2).
                    rt.lanes[li].sent = self
                        .auto_push_hosted(&dest, at, 0, v)
                        .then(|| crate::automation::hosted_bits(v));
                }
                Dest::Effect(..)
                | Dest::Native(..)
                | Dest::Firmus(..)
                | Dest::ModAmount(..)
                | Dest::Nothing => {}
            }
        }
        if rt.any_hosted && playing {
            self.auto.hosted_next = (now / GRID + 1) * GRID;
        }
        // Parameter sets that moved: their units configured from the copies.
        for c in rt.effects.iter_mut().filter(|c| c.dirty) {
            c.dirty = false;
            match c.at {
                Some((usize::MAX, i)) => {
                    if let Some(u) = self.master.effects.get_mut(i).filter(|u| u.id == c.effect) {
                        u.configure(&c.spec);
                    }
                }
                Some((s, i)) => {
                    if let Some(mut st) = nodes.strip(s) {
                        if let Some(u) = st.effects.get_mut(i).filter(|u| u.id == c.effect) {
                            u.configure(&c.spec);
                        }
                    }
                }
                None => {}
            }
        }
        for c in rt.natives.iter_mut().filter(|c| c.dirty) {
            c.dirty = false;
            if let (Some(&slot), Some(t)) =
                (self.track_to_slot.get(c.track), snap.tracks.get(c.track))
            {
                if let Some(mut s) = nodes.track(slot) {
                    if let Some(u) = s.native.as_mut() {
                        u.unit.configure(&c.spec, &t.assets);
                    }
                }
            }
        }
        for c in rt.firmus.iter_mut().filter(|c| c.dirty) {
            c.dirty = false;
            if let Some(&slot) = self.track_to_slot.get(c.track) {
                if let Some(mut s) = nodes.track(slot) {
                    s.synth.set_settings(&to_settings(&c.params));
                }
            }
        }
        // When the next control point is due: at the next grid point while anything
        // native moves (or a modulator must hear every window); otherwise at the grid
        // point just before the next piece of any lane begins (nothing changes until
        // then); stopped, the next pass decides.
        let next = if !playing {
            u64::MAX
        } else if moving || rt.every_grid_point() || rt.moving(&snap, tick) {
            grid
        } else {
            match rt.next_piece(&snap, tick) {
                Some(t) => {
                    let at = now + self.transport.samples_until(t);
                    grid.max((at.saturating_sub(1) / GRID) * GRID)
                }
                None => u64::MAX,
            }
        };
        // While modulators play, a control point at least once a pass (on the grid, so
        // the same at every block size): what they carry is stepped there and the
        // interface hears where they are, even when they drive only hosted parameters.
        self.auto.next = if playing && !rt.mods.is_empty() {
            next.min(grid.max((now + crate::graph::PASS_FRAMES as u64) / GRID * GRID))
        } else {
            next
        };
        self.auto.rt = Some(rt);
    }
}

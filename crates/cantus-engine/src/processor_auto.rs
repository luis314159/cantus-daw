//! Automation on the audio thread (ADR 0027, `crate::automation`): when a control point
//! is due, what it writes, and the ramps the mix loops read between two of them.
//!
//! **The null rule.** Without a runtime nothing here runs: no segment is split, no ramp
//! exists, the master's values are the snapshot's. A gain or pan whose curve holds still
//! has no ramp either, and takes the old constant path.

use super::*;
use crate::automation::{Dest, Ramp, Runtime, GRID};
use cantus_core::db_to_linear;

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
}

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
pub(crate) fn ramp_strip(st: &StripSlot, law: PanLaw, k0: u64, l: &mut [f32], r: &mut [f32]) -> bool {
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
    /// just been applied everywhere, so every ramp and override goes), and find each
    /// automated effect's unit. The first control point is due at once (`reseek_events`).
    pub(super) fn auto_install(&mut self, snapshot: &Arc<Snapshot>) {
        let rt = match self.auto.pending.take() {
            Some(rt) if rt.is_for(snapshot) => Some(rt),
            Some(stale) => {
                self.auto_dispose(stale);
                None
            }
            None => None,
        };
        if rt.is_none() && !snapshot.automation.is_empty() {
            // Sent without its runtime: it plays, unautomated.
            self.stats.dropped_events += 1;
        }
        if let Some(old) = std::mem::replace(&mut self.auto.rt, rt) {
            self.auto_dispose(old);
        }
        let nodes = self.nodes.clone();
        self.auto.clear_ramps(&nodes);
        self.auto.master = [None; 3];
        self.auto.next = u64::MAX;
        if let Some(rt) = self.auto.rt.as_mut() {
            for c in rt.effects.iter_mut() {
                c.at = (0..MAX_STRIPS)
                    .filter(|si| self.strip_ids[*si].is_some())
                    .find_map(|si| {
                        let s = nodes.strip(si)?;
                        s.effects
                            .iter()
                            .position(|e| e.id == c.effect)
                            .map(|i| (si, i))
                    })
                    .or_else(|| {
                        self.master
                            .effects
                            .iter()
                            .position(|e| e.id == c.effect)
                            .map(|i| (usize::MAX, i))
                    });
            }
        }
    }

    /// The playhead jumped (a start, seek, loop wrap, a new snapshot): a control point is
    /// due now.
    pub(super) fn auto_reseek(&mut self) {
        if self.auto.rt.is_some() {
            self.auto.next = self.transport.samples();
        }
    }

    /// At the start of a segment: run the control point if one is due. Then cap the
    /// segment at the next one, so it lands on its sample whatever the block size.
    pub(super) fn auto_point(&mut self, done: usize, seg_end: usize) -> usize {
        if self.auto.rt.is_none() {
            return seg_end;
        }
        let now = self.transport.samples();
        if now >= self.auto.next {
            self.auto_evaluate(now);
        }
        if !self.transport.is_playing() || self.auto.next == u64::MAX {
            return seg_end;
        }
        let until = self.auto.next.saturating_sub(now);
        seg_end.min(done + until.min(MAX_BLOCK_FRAMES as u64) as usize)
    }

    /// Evaluate every lane at transport sample `now` and the next grid point, and write
    /// the values where they go.
    fn auto_evaluate(&mut self, now: u64) {
        let (Some(mut rt), Some(snap)) = (self.auto.rt.take(), self.snapshot.clone()) else {
            return;
        };
        let playing = self.transport.is_playing();
        let tick = self.transport.position_at(now);
        let grid = (now / GRID + 1) * GRID;
        let next_tick = if playing {
            self.transport.position_at(grid)
        } else {
            tick
        };
        let len = grid - now;
        rt.evaluate(&snap, tick, next_tick);
        self.auto.at = now;
        let nodes = self.nodes.clone();
        let mut moving = false;
        for lane in &rt.lanes {
            let (a, b) = (lane.now, lane.next);
            moving |= a != b;
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
                Dest::Effect(..) | Dest::Native(..) | Dest::Firmus(..) | Dest::Nothing => {}
            }
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
        // moves; otherwise at the grid point just before the next piece of any lane
        // begins (nothing changes until then); never, stopped.
        self.auto.next = if !playing {
            u64::MAX
        } else if moving || rt.moving(&snap, tick) {
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
        self.auto.rt = Some(rt);
    }
}

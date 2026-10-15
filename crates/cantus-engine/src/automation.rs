//! Automation and modulation on the audio thread (ADR 0027, ADR 0057): what a snapshot's
//! lanes drive, resolved once, the copies of the parameter sets they write into, and the
//! modulators that offset them.
//!
//! A lane's value lands on the **16-sample control grid** every native DSP already
//! updates on (ADR 0020 §4), counted from the transport's origin (the last start, seek,
//! loop wrap or tempo change), so it is the same for every block size and offline. At a
//! grid point the engine evaluates every lane: effects and instruments are configured
//! with the new values (their own glides smooth them, as they smooth a knob), and gains
//! and pans **ramp** sample by sample from this grid point's value to the next one's, so
//! a fader ride has no steps at all.
//!
//! **Modulation** (ADR 0057) offsets a lane's position — its automation's, or the
//! project's value where no clip plays — in the normalised domain:
//! `clamp(position + Σ depth × amount × output, 0, 1)`; a lane with no route reads exactly
//! as before. A **hosted plug-in's** parameter is not configured between passes: its
//! values at every grid point of a pass go to the unit with their sample offsets
//! (`AutoQueue`), so a pass is never cut for a plug-in.
//!
//! An effect or an instrument is configured from a whole parameter set, and the audio
//! thread cannot copy one (a map allocates). So the UI thread builds a [`Runtime`] with a
//! copy of each automated set, every automated key already in it, and sends it with the
//! snapshot ([`EngineMessage::Automation`](crate::EngineMessage::Automation)); the audio
//! thread only overwrites numbers in place. The runtime comes back to the UI thread to be
//! freed, like a snapshot.

use crate::fx::HostedValue;
use crate::modulator::ModRt;
use cantus_core::modulator::eval::ModTime;
use cantus_core::{
    AutoTarget, EffectId, EffectSpec, FirmusParam, FirmusParams, NativeSpec, Snapshot,
};
use std::sync::Arc;

/// Samples between two control points: the DSPs' update grid (ADR 0020 §4).
pub const GRID: u64 = 16;

/// Where a lane's value goes.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Dest {
    /// Indices into the snapshot's tracks / strips.
    TrackGain(usize),
    TrackPan(usize),
    StripGain(usize),
    StripPan(usize),
    StripWidth(usize),
    Send {
        strip: usize,
        to: usize,
    },
    MasterGain,
    MasterPan,
    MasterWidth,
    /// Index into [`Runtime::effects`] and the key in its copy.
    Effect(usize, String),
    /// Index into [`Runtime::natives`].
    Native(usize, String),
    /// Index into [`Runtime::firmus`].
    Firmus(usize, FirmusParam),
    /// A hosted insert's parameter: index into [`Runtime::hosted`], the plug-in's id.
    Hosted(usize, u32),
    /// A hosted instrument's parameter: the snapshot track index, the plug-in's id.
    HostedInst(usize, u32),
    /// A modulator's amount: index into [`Runtime::mods`].
    ModAmount(usize),
    /// A target the snapshot does not hold.
    Nothing,
}

impl Dest {
    /// A hosted plug-in's parameter (timed changes, never a cut pass).
    pub fn is_hosted(&self) -> bool {
        matches!(self, Dest::Hosted(..) | Dest::HostedInst(..))
    }
}

/// An automated effect's parameter copy.
#[derive(Debug)]
pub(crate) struct FxCopy {
    pub effect: EffectId,
    pub spec: EffectSpec,
    pub dirty: bool,
    /// Where its unit runs, found when the snapshot is installed: a strip slot and the
    /// index in its chain, or `usize::MAX` and the index in the master's.
    pub at: Option<(usize, usize)>,
}

/// A hosted insert some lane drives, and where its unit runs (as [`FxCopy::at`]).
#[derive(Debug)]
pub(crate) struct HostedFx {
    pub effect: EffectId,
    pub at: Option<(usize, usize)>,
}

/// An automated native instrument's copy, by snapshot track index.
#[derive(Debug)]
pub(crate) struct NativeCopy {
    pub track: usize,
    pub spec: NativeSpec,
    pub dirty: bool,
}

/// An automated Firmus's copy, by snapshot track index.
#[derive(Debug)]
pub(crate) struct FirmusCopy {
    pub track: usize,
    pub params: FirmusParams,
    pub dirty: bool,
}

/// One lane's destination and its values at the last control point and the next.
#[derive(Debug)]
pub(crate) struct Lane {
    pub dest: Dest,
    /// The value (a hosted lane: the automated position, NaN where none) now and at the
    /// next grid point.
    pub now: f32,
    pub next: f32,
    /// A hosted lane: the modulators' sum now.
    pub mod_now: f32,
    /// Never evaluated yet: the first evaluation always applies.
    pub fresh: bool,
    /// The modulators driving it: (index into [`Runtime::mods`], depth).
    pub routes_in: Vec<(usize, f32)>,
    /// The project's value as a position (where no clip plays), for modulation.
    pub base_pos: f32,
}

/// When a control point is.
#[derive(Copy, Clone, Debug)]
pub(crate) struct At {
    /// The song position now and at the next grid point, in ticks.
    pub tick: f64,
    pub next_tick: f64,
    /// The same, as the modulators read time.
    pub t: ModTime,
    pub t_next: ModTime,
}

/// Everything the audio thread needs to play a snapshot's automation and modulators.
/// Built on the UI thread by [`Runtime::build`]; never allocates once on the audio thread.
#[derive(Debug)]
pub struct Runtime {
    /// The snapshot it was built for (its address), so a runtime never meets another's.
    pub(crate) snapshot: usize,
    pub(crate) lanes: Vec<Lane>,
    pub(crate) effects: Vec<FxCopy>,
    pub(crate) natives: Vec<NativeCopy>,
    pub(crate) firmus: Vec<FirmusCopy>,
    pub(crate) hosted: Vec<HostedFx>,
    /// One per `Snapshot::modulators`, in order.
    pub(crate) mods: Vec<ModRt>,
    /// A modulator follows audio: every grid point is a control point (its window must be
    /// whole), whatever it drives.
    pub(crate) audio_mods: bool,
    /// A native (non-hosted) lane is modulated: every grid point is a control point while
    /// playing.
    pub(crate) native_modulated: bool,
    /// Some lane is a hosted plug-in's.
    pub(crate) any_hosted: bool,
}

impl Runtime {
    /// The runtime for `snapshot`'s lanes and modulators, or `None` when it has neither
    /// (the engine then does nothing it did not do before). Allocates: never on the audio
    /// thread.
    pub fn build(snapshot: &Arc<Snapshot>) -> Option<Box<Runtime>> {
        if snapshot.automation.is_empty() && snapshot.modulators.is_empty() {
            return None;
        }
        let mut rt = Runtime {
            snapshot: Arc::as_ptr(snapshot) as usize,
            lanes: Vec::with_capacity(snapshot.automation.len()),
            effects: Vec::new(),
            natives: Vec::new(),
            firmus: Vec::new(),
            hosted: Vec::new(),
            mods: snapshot.modulators.iter().map(ModRt::new).collect(),
            audio_mods: snapshot.modulators.iter().any(|m| m.source.follows_audio()),
            native_modulated: false,
            any_hosted: false,
        };
        for lane in &snapshot.automation {
            let dest = rt.dest_of(snapshot, &lane.target).unwrap_or(Dest::Nothing);
            rt.any_hosted |= dest.is_hosted();
            rt.lanes.push(Lane {
                // A lane whose target is not in the snapshot drives nothing.
                now: lane.base,
                next: lane.base,
                mod_now: 0.0,
                fresh: true,
                routes_in: Vec::new(),
                base_pos: if lane.base.is_finite() {
                    lane.range.pos(lane.base)
                } else {
                    f32::NAN
                },
                dest,
            });
        }
        for (mi, plan) in snapshot.modulators.iter().enumerate() {
            for &(li, depth) in &plan.routes {
                if let Some(l) = rt.lanes.get_mut(li) {
                    l.routes_in.push((mi, depth));
                    if !l.dest.is_hosted() && !matches!(l.dest, Dest::ModAmount(_) | Dest::Nothing)
                    {
                        rt.native_modulated = true;
                    }
                }
            }
        }
        Some(Box::new(rt))
    }

    /// Where a target's value goes in `snapshot`, its copies made as needed.
    fn dest_of(&mut self, snapshot: &Snapshot, target: &AutoTarget) -> Option<Dest> {
        let track = |id| snapshot.tracks.iter().position(|t| t.id == id);
        let strip = |id| snapshot.strips.iter().position(|s| s.id == id);
        match target {
            AutoTarget::TrackGain(t) => track(*t).map(Dest::TrackGain),
            AutoTarget::TrackPan(t) => track(*t).map(Dest::TrackPan),
            AutoTarget::StripGain(s) => strip(*s).map(Dest::StripGain),
            AutoTarget::StripPan(s) => strip(*s).map(Dest::StripPan),
            AutoTarget::StripWidth(s) => strip(*s).map(Dest::StripWidth),
            AutoTarget::Send { strip: s, to } => strip(*s)
                .zip(strip(*to))
                .map(|(strip, to)| Dest::Send { strip, to }),
            AutoTarget::MasterGain => Some(Dest::MasterGain),
            AutoTarget::MasterPan => Some(Dest::MasterPan),
            AutoTarget::MasterWidth => Some(Dest::MasterWidth),
            AutoTarget::Effect { effect, key } => {
                let slot = snapshot.effect_slots().find(|e| e.id == *effect)?;
                let i = match self.effects.iter().position(|c| c.effect == *effect) {
                    Some(i) => i,
                    None => {
                        self.effects.push(FxCopy {
                            effect: *effect,
                            spec: slot.spec.clone(),
                            dirty: false,
                            at: None,
                        });
                        self.effects.len() - 1
                    }
                };
                // The key is in the copy from the start, so writing it later is a write and
                // never an insertion (which would allocate).
                let spec = &mut self.effects[i].spec;
                let v = spec.get(key);
                spec.params.insert(key.clone(), v);
                Some(Dest::Effect(i, key.clone()))
            }
            AutoTarget::Instrument { track: t, key } => {
                let ti = track(*t)?;
                let native = snapshot.tracks[ti].instrument.native()?;
                let i = match self.natives.iter().position(|c| c.track == ti) {
                    Some(i) => i,
                    None => {
                        self.natives.push(NativeCopy {
                            track: ti,
                            spec: native.clone(),
                            dirty: false,
                        });
                        self.natives.len() - 1
                    }
                };
                let spec = &mut self.natives[i].spec;
                let v = spec.get(key);
                spec.params.insert(key.clone(), v);
                Some(Dest::Native(i, key.clone()))
            }
            AutoTarget::Firmus { track: t, param } => {
                let ti = track(*t)?;
                let f = snapshot.tracks[ti].instrument.firmus()?;
                let i = match self.firmus.iter().position(|c| c.track == ti) {
                    Some(i) => i,
                    None => {
                        self.firmus.push(FirmusCopy {
                            track: ti,
                            params: f.clone(),
                            dirty: false,
                        });
                        self.firmus.len() - 1
                    }
                };
                Some(Dest::Firmus(i, *param))
            }
            AutoTarget::Plugin { effect, id } => {
                snapshot
                    .effect_slots()
                    .find(|e| e.id == *effect && e.spec.external.is_some())?;
                let i = match self.hosted.iter().position(|h| h.effect == *effect) {
                    Some(i) => i,
                    None => {
                        self.hosted.push(HostedFx {
                            effect: *effect,
                            at: None,
                        });
                        self.hosted.len() - 1
                    }
                };
                Some(Dest::Hosted(i, *id))
            }
            AutoTarget::PluginInstrument { track: t, id } => track(*t)
                .filter(|ti| snapshot.tracks[*ti].instrument.external().is_some())
                .map(|ti| Dest::HostedInst(ti, *id)),
            AutoTarget::ModAmount(m) => snapshot
                .modulators
                .iter()
                .position(|p| p.id == *m)
                .map(Dest::ModAmount),
        }
    }

    /// Whether this runtime was built for `snapshot`.
    pub(crate) fn is_for(&self, snapshot: &Arc<Snapshot>) -> bool {
        self.snapshot == Arc::as_ptr(snapshot) as usize
    }

    /// Every grid point must be a control point while playing.
    pub(crate) fn every_grid_point(&self) -> bool {
        self.audio_mods || self.native_modulated
    }

    /// Each modulator's output and amount now and at the next grid point.
    fn modulate(&mut self, snapshot: &Snapshot, at: &At) {
        for (m, plan) in self.mods.iter_mut().zip(&snapshot.modulators) {
            if ModRt::stateful(plan) {
                // One control point behind: ramp from the last point's state to this one's.
                let cur = m.state_value(plan);
                m.now = m.held_value;
                m.next = cur;
                m.held_value = cur;
            } else {
                m.now = m.value_at(plan, &at.t);
                m.next = m.value_at(plan, &at.t_next);
            }
        }
        // Amounts: automated, and offset by other modulators (one level: a route into an
        // amount reads its own modulator at its unmodulated amount).
        for i in 0..self.mods.len() {
            let plan = &snapshot.modulators[i];
            let (mut a, mut b) = (plan.amount, plan.amount);
            if let Some(li) = plan.amount_lane {
                let lane = &snapshot.automation[li];
                a = lane.pos_at(at.tick).unwrap_or(plan.amount);
                b = lane.pos_at(at.next_tick).unwrap_or(plan.amount);
                if let Some(l) = self.lanes.get(li) {
                    for &(j, depth) in &l.routes_in {
                        a += depth * self.mods[j].now * base_amount(snapshot, j, at.tick);
                        b += depth * self.mods[j].next * base_amount(snapshot, j, at.next_tick);
                    }
                }
            }
            self.mods[i].amount_now = a.clamp(0.0, 1.0);
            self.mods[i].amount_next = b.clamp(0.0, 1.0);
        }
    }

    /// The modulators' sum into lane `li` now and next.
    #[inline]
    fn mod_sum(&self, li: usize) -> (f32, f32) {
        let (mut a, mut b) = (0.0f32, 0.0f32);
        for &(j, depth) in &self.lanes[li].routes_in {
            let m = &self.mods[j];
            a += depth * m.now * m.amount_now;
            b += depth * m.next * m.amount_next;
        }
        (a, b)
    }

    /// Evaluate every lane at a control point, writing the values into the parameter
    /// copies; returns whether any value changed. No allocation: numbers written in place.
    pub(crate) fn evaluate(&mut self, snapshot: &Snapshot, at: &At) -> bool {
        if !self.mods.is_empty() {
            self.modulate(snapshot, at);
        }
        let mut changed = false;
        for li in 0..self.lanes.len() {
            let lane = &snapshot.automation[li];
            let (ma, mb) = self.mod_sum(li);
            let l = &mut self.lanes[li];
            if matches!(l.dest, Dest::ModAmount(_)) {
                continue;
            }
            if l.dest.is_hosted() {
                // The position (NaN: the project's) and the modulation, as the unit wants.
                l.fresh = false;
                l.now = lane.pos_at(at.tick).unwrap_or(f32::NAN);
                l.mod_now = ma;
                continue;
            }
            let base_pos = l.base_pos;
            let value = |tick: f64, m: f32| {
                if m == 0.0 {
                    lane.value_at(tick)
                } else {
                    let p = lane.pos_at(tick).unwrap_or(base_pos);
                    lane.range.value((p + m).clamp(0.0, 1.0))
                }
            };
            let v = value(at.tick, ma);
            let w = value(at.next_tick, mb);
            if v == l.now && w == l.next && !l.fresh {
                continue;
            }
            l.fresh = false;
            changed = true;
            l.now = v;
            l.next = w;
            match &l.dest {
                Dest::Effect(i, key) => {
                    if let Some(c) = self.effects.get_mut(*i) {
                        if let Some(x) = c.spec.params.get_mut(key.as_str()) {
                            *x = v;
                            c.dirty = true;
                        }
                    }
                }
                Dest::Native(i, key) => {
                    if let Some(c) = self.natives.get_mut(*i) {
                        if let Some(x) = c.spec.params.get_mut(key.as_str()) {
                            *x = v;
                            c.dirty = true;
                        }
                    }
                }
                Dest::Firmus(i, param) => {
                    if let Some(c) = self.firmus.get_mut(*i) {
                        param.set(&mut c.params, v);
                        c.dirty = true;
                    }
                }
                _ => {}
            }
        }
        changed
    }

    /// The hosted lanes at an inner grid point of a pass (`t` its time), each handed to
    /// `f` with its destination and value. Modulators that are functions of time are read
    /// there; what holds state keeps its control point's value.
    pub(crate) fn hosted_at(
        &mut self,
        snapshot: &Snapshot,
        tick: f64,
        t: &ModTime,
        mut f: impl FnMut(&Dest, Option<(usize, usize)>, HostedValue),
    ) {
        for (m, plan) in self.mods.iter_mut().zip(&snapshot.modulators) {
            m.inner = if ModRt::stateful(plan) {
                m.held_value
            } else {
                m.value_at(plan, t)
            };
            m.inner_amount = plan.amount;
        }
        for i in 0..self.mods.len() {
            let plan = &snapshot.modulators[i];
            let Some(li) = plan.amount_lane else { continue };
            let mut a = snapshot.automation[li].pos_at(tick).unwrap_or(plan.amount);
            for &(j, depth) in &self.lanes[li].routes_in {
                a += depth * self.mods[j].inner * base_amount(snapshot, j, tick);
            }
            self.mods[i].inner_amount = a.clamp(0.0, 1.0);
        }
        for (li, l) in self.lanes.iter().enumerate() {
            if !l.dest.is_hosted() {
                continue;
            }
            let mut m = 0.0f32;
            for &(j, depth) in &l.routes_in {
                m += depth * self.mods[j].inner * self.mods[j].inner_amount;
            }
            let pos = snapshot.automation[li].pos_at(tick).unwrap_or(f32::NAN);
            let at = match l.dest {
                Dest::Hosted(h, _) => self.hosted.get(h).and_then(|x| x.at),
                _ => None,
            };
            f(&l.dest, at, HostedValue { pos, modulation: m });
        }
    }

    /// Whether any non-hosted lane is moving at `tick` (the next control point is needed
    /// at once).
    pub(crate) fn moving(&self, snapshot: &Snapshot, tick: f64) -> bool {
        snapshot
            .automation
            .iter()
            .zip(&self.lanes)
            .any(|(a, l)| !l.dest.is_hosted() && a.moving_at(tick))
    }

    /// The earliest tick after `tick` where any non-hosted lane's next piece starts.
    pub(crate) fn next_piece(&self, snapshot: &Snapshot, tick: f64) -> Option<f64> {
        snapshot
            .automation
            .iter()
            .zip(&self.lanes)
            .filter(|(_, l)| !l.dest.is_hosted())
            .filter_map(|(a, _)| a.next_after(tick))
            .map(|t| t.0 as f64)
            .min_by(f64::total_cmp)
    }
}

/// Modulator `j`'s amount without modulation: its automation's, else its own.
#[inline]
fn base_amount(snapshot: &Snapshot, j: usize, tick: f64) -> f32 {
    let plan = &snapshot.modulators[j];
    plan.amount_lane
        .and_then(|li| snapshot.automation[li].pos_at(tick))
        .unwrap_or(plan.amount)
        .clamp(0.0, 1.0)
}

/// A value travelling linearly from `a` (at the control point) to `b` (at the next one)
/// over `len` samples. `at(k)` is the value `k` samples after the control point: computed
/// from the control point, never accumulated, so every block size gives the same number.
#[derive(Copy, Clone, Debug, PartialEq)]
pub(crate) struct Ramp {
    pub a: f32,
    pub b: f32,
    pub inv_len: f32,
}

impl Ramp {
    #[inline]
    pub fn at(&self, k: u64) -> f32 {
        self.a + (self.b - self.a) * (k as f32 * self.inv_len)
    }

    /// `Some` only when it moves: a flat ramp is a constant and takes the old path.
    #[inline]
    pub fn of(a: f32, b: f32, len: u64) -> Option<Ramp> {
        (a != b).then(|| Ramp {
            a,
            b,
            inv_len: 1.0 / len.max(1) as f32,
        })
    }
}

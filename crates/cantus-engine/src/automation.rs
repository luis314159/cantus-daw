//! Automation on the audio thread (ADR 0027): what a snapshot's lanes drive, resolved
//! once, and the copies of the parameter sets they write into.
//!
//! A lane's value lands on the **16-sample control grid** every native DSP already
//! updates on (ADR 0020 §4), counted from the transport's origin (the last start, seek,
//! loop wrap or tempo change), so it is the same for every block size and offline. At a
//! grid point the engine evaluates every lane: effects and instruments are configured
//! with the new values (their own glides smooth them, as they smooth a knob), and gains
//! and pans **ramp** sample by sample from this grid point's value to the next one's, so
//! a fader ride has no steps at all.
//!
//! An effect or an instrument is configured from a whole parameter set, and the audio
//! thread cannot copy one (a map allocates). So the UI thread builds a [`Runtime`] with a
//! copy of each automated set, every automated key already in it, and sends it with the
//! snapshot ([`EngineMessage::Automation`](crate::EngineMessage::Automation)); the audio
//! thread only overwrites numbers in place. The runtime comes back to the UI thread to be
//! freed, like a snapshot.

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
    /// A target the snapshot does not hold.
    Nothing,
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
    pub now: f32,
    pub next: f32,
    /// Never evaluated yet: the first evaluation always applies.
    pub fresh: bool,
}

/// Everything the audio thread needs to play a snapshot's automation. Built on the UI
/// thread by [`Runtime::build`]; never allocates once on the audio thread.
#[derive(Debug)]
pub struct Runtime {
    /// The snapshot it was built for (its address), so a runtime never meets another's.
    pub(crate) snapshot: usize,
    pub(crate) lanes: Vec<Lane>,
    pub(crate) effects: Vec<FxCopy>,
    pub(crate) natives: Vec<NativeCopy>,
    pub(crate) firmus: Vec<FirmusCopy>,
}

impl Runtime {
    /// The runtime for `snapshot`'s lanes, or `None` when it has none (the engine then
    /// does nothing it did not do before). Allocates: never on the audio thread.
    pub fn build(snapshot: &Arc<Snapshot>) -> Option<Box<Runtime>> {
        if snapshot.automation.is_empty() {
            return None;
        }
        let mut rt = Runtime {
            snapshot: Arc::as_ptr(snapshot) as usize,
            lanes: Vec::with_capacity(snapshot.automation.len()),
            effects: Vec::new(),
            natives: Vec::new(),
            firmus: Vec::new(),
        };
        let track = |id| snapshot.tracks.iter().position(|t| t.id == id);
        let strip = |id| snapshot.strips.iter().position(|s| s.id == id);
        for lane in &snapshot.automation {
            let dest = match &lane.target {
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
                    snapshot
                        .effect_slots()
                        .find(|e| e.id == *effect)
                        .map(|slot| {
                            let i = match rt.effects.iter().position(|c| c.effect == *effect) {
                                Some(i) => i,
                                None => {
                                    rt.effects.push(FxCopy {
                                        effect: *effect,
                                        spec: slot.spec.clone(),
                                        dirty: false,
                                        at: None,
                                    });
                                    rt.effects.len() - 1
                                }
                            };
                            // The key is in the copy from the start, so writing it later is
                            // a write and never an insertion (which would allocate).
                            let spec = &mut rt.effects[i].spec;
                            let v = spec.get(key);
                            spec.params.insert(key.clone(), v);
                            Dest::Effect(i, key.clone())
                        })
                }
                AutoTarget::Instrument { track: t, key } => track(*t).and_then(|ti| {
                    let native = snapshot.tracks[ti].instrument.native()?;
                    // A key that is set, never automated (Chorda's tuning): the clip is inert.
                    if !native.kind.automatable(key) {
                        return None;
                    }
                    let i = match rt.natives.iter().position(|c| c.track == ti) {
                        Some(i) => i,
                        None => {
                            rt.natives.push(NativeCopy {
                                track: ti,
                                spec: native.clone(),
                                dirty: false,
                            });
                            rt.natives.len() - 1
                        }
                    };
                    let spec = &mut rt.natives[i].spec;
                    let v = spec.get(key);
                    spec.params.insert(key.clone(), v);
                    Some(Dest::Native(i, key.clone()))
                }),
                AutoTarget::Firmus { track: t, param } => track(*t).and_then(|ti| {
                    let f = snapshot.tracks[ti].instrument.firmus()?;
                    let i = match rt.firmus.iter().position(|c| c.track == ti) {
                        Some(i) => i,
                        None => {
                            rt.firmus.push(FirmusCopy {
                                track: ti,
                                params: f.clone(),
                                dirty: false,
                            });
                            rt.firmus.len() - 1
                        }
                    };
                    Some(Dest::Firmus(i, *param))
                }),
            };
            rt.lanes.push(Lane {
                // A lane whose target is not in the snapshot drives nothing.
                dest: dest.unwrap_or(Dest::Nothing),
                now: lane.base,
                next: lane.base,
                fresh: true,
            });
        }
        Some(Box::new(rt))
    }

    /// Whether this runtime was built for `snapshot`.
    pub(crate) fn is_for(&self, snapshot: &Arc<Snapshot>) -> bool {
        self.snapshot == Arc::as_ptr(snapshot) as usize
    }

    /// Evaluate every lane at `tick` (now) and `next` (the next control point), writing
    /// the values into the parameter copies; returns whether any value changed. No
    /// allocation: numbers written in place.
    pub(crate) fn evaluate(&mut self, snapshot: &Snapshot, tick: f64, next: f64) -> bool {
        let mut changed = false;
        for (lane, l) in snapshot.automation.iter().zip(self.lanes.iter_mut()) {
            let v = lane.value_at(tick);
            let w = lane.value_at(next);
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

    /// Whether any lane is moving at `tick` (the next control point is needed at once).
    pub(crate) fn moving(&self, snapshot: &Snapshot, tick: f64) -> bool {
        snapshot.automation.iter().any(|l| l.moving_at(tick))
    }

    /// The earliest tick after `tick` where any lane's next piece starts.
    pub(crate) fn next_piece(&self, snapshot: &Snapshot, tick: f64) -> Option<f64> {
        snapshot
            .automation
            .iter()
            .filter_map(|l| l.next_after(tick))
            .map(|t| t.0 as f64)
            .min_by(f64::total_cmp)
    }
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

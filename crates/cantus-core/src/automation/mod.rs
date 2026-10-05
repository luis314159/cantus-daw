//! Automation clips (ADR 0027): FL Studio's automation clips, in lanes under a track.
//!
//! An **automation clip is a [`Clip`](crate::Clip)** whose content is a curve instead of
//! notes (`Clip::automation`). Everything a clip already is comes with it unchanged: a
//! window onto its content (move, split, trim from either side, slip, loop), mute, colour,
//! copy and paste, undo. Its content is a list of [`AutoPoint`]s in the clip's *content*
//! ticks, exactly as notes are, so a split or a looping window needs no special case.
//!
//! The curve is **normalised** (0…1, FL's convention): a point's value is a position on
//! the target's control, mapped to the parameter's own units through [`AutoRange`] — a
//! fader's taper for gains, a log taper for frequencies. Flip, scale and the LFO shapes
//! are then the same arithmetic for every target, and a curve can be pointed at another
//! parameter and still mean "from the bottom of the knob to the top".
//!
//! What a target is, how a clip is evaluated and how the engine is handed one function of
//! time per target are in the sub-modules.

use crate::instrument::FirmusParam;
use crate::model::{EffectId, StripId, TrackId};
use crate::time::Ticks;
use serde::{Deserialize, Serialize};

pub mod compile;
pub mod edit;
pub mod range;

pub use compile::{AutoLane, AutoSeg};
pub use range::{AutoRange, Taper};

/// What an automation clip drives. Stable across saves: ids and parameter keys, never
/// indices (a key is forever once shipped, ADR 0018/0023).
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum AutoTarget {
    /// A track's own level (dB, on the fader's taper) and balance.
    TrackGain(TrackId),
    TrackPan(TrackId),
    /// A mixer strip's fader, pan and stereo width.
    StripGain(StripId),
    StripPan(StripId),
    StripWidth(StripId),
    /// A strip's post-fader send into `to`.
    Send { strip: StripId, to: StripId },
    MasterGain,
    MasterPan,
    MasterWidth,
    /// One parameter of an insert, on a strip or on the master (the effect's host is
    /// found from its id: ids are unique in a project).
    Effect { effect: EffectId, key: String },
    /// One number of a track's native instrument (`NativeSpec` key).
    Instrument { track: TrackId, key: String },
    /// One parameter of Firmus, the built-in synth.
    Firmus { track: TrackId, param: FirmusParam },
}

impl AutoTarget {
    /// Whether the value is a level the engine multiplies by (dB on the fader's taper).
    pub fn is_gain(&self) -> bool {
        matches!(
            self,
            AutoTarget::TrackGain(_) | AutoTarget::StripGain(_) | AutoTarget::MasterGain
        )
    }

    /// The parameter a command sets, when it sets one that can be automated: what FL
    /// calls the *last tweaked* parameter. `None` for everything else.
    pub fn of_command(cmd: &crate::Command) -> Option<AutoTarget> {
        use crate::Command as C;
        Some(match cmd {
            C::SetTrackGain { id, .. } => AutoTarget::TrackGain(*id),
            C::SetTrackPan { id, .. } => AutoTarget::TrackPan(*id),
            C::SetStripGain { id, .. } => AutoTarget::StripGain(*id),
            C::SetStripPan { id, .. } => AutoTarget::StripPan(*id),
            C::SetStripWidth { id, .. } => AutoTarget::StripWidth(*id),
            C::SetStripSend { id, send } => AutoTarget::Send {
                strip: *id,
                to: send.to,
            },
            C::SetMasterGain { .. } => AutoTarget::MasterGain,
            C::SetMasterPan { .. } => AutoTarget::MasterPan,
            C::SetMasterWidth { .. } => AutoTarget::MasterWidth,
            C::SetEffectParam { effect, key, .. } | C::ClearEffectParam { effect, key, .. } => {
                AutoTarget::Effect {
                    effect: *effect,
                    key: key.clone(),
                }
            }
            C::SetInstrumentValue { track, key, .. } | C::ClearInstrumentValue { track, key } => {
                AutoTarget::Instrument {
                    track: *track,
                    key: key.clone(),
                }
            }
            C::SetInstrumentParam {
                track,
                param: crate::InstrumentParam::Firmus(p),
                ..
            } => AutoTarget::Firmus {
                track: *track,
                param: *p,
            },
            C::Batch { commands, .. } => return commands.iter().rev().find_map(Self::of_command),
            _ => return None,
        })
    }
}

/// How the curve travels from a point to the next one — FL's point modes.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Shape {
    /// One bend: tension pushes it towards the start or the end (FL's *single curve*).
    #[default]
    Curve,
    /// An S: slow out, fast through the middle, slow in (FL's *double curve*).
    SCurve,
    /// Stay at this point's value and jump at the next one: a step.
    Hold,
    /// Climb in equal steps; tension sets how many.
    Stairs,
    /// Alternate between the two values; tension sets how often.
    Pulse,
    /// A sine between the two values; tension sets how many cycles.
    Wave,
}

impl Shape {
    pub const ALL: [Shape; 6] = [
        Shape::Curve,
        Shape::SCurve,
        Shape::Hold,
        Shape::Stairs,
        Shape::Pulse,
        Shape::Wave,
    ];

    /// The i18n key of its name.
    pub fn label_key(self) -> &'static str {
        match self {
            Shape::Curve => "auto.shape.curve",
            Shape::SCurve => "auto.shape.scurve",
            Shape::Hold => "auto.shape.hold",
            Shape::Stairs => "auto.shape.stairs",
            Shape::Pulse => "auto.shape.pulse",
            Shape::Wave => "auto.shape.wave",
        }
    }

    /// Whether tension means something for it.
    pub fn has_tension(self) -> bool {
        self != Shape::Hold
    }

    /// How far (0…1) from the start value towards the end value the curve is at `u`
    /// (0…1) of the way between the two points. Pure, cheap (one `powf` at most), and
    /// the one place a shape is defined: the editor draws it and the engine plays it.
    pub fn at(self, tension: f32, u: f32) -> f32 {
        let u = u.clamp(0.0, 1.0);
        let t = if tension.is_finite() {
            tension.clamp(-1.0, 1.0)
        } else {
            0.0
        };
        match self {
            Shape::Curve => bend(u, t),
            Shape::SCurve => {
                if u < 0.5 {
                    0.5 * bend(2.0 * u, t.abs())
                } else {
                    1.0 - 0.5 * bend(2.0 - 2.0 * u, t.abs())
                }
            }
            Shape::Hold => 0.0,
            Shape::Stairs => {
                let n = steps(t) as f32;
                ((u * n).floor() / (n - 1.0)).min(1.0)
            }
            Shape::Pulse => {
                let c = steps(t) as f32 * 0.5;
                if (u * c).fract() < 0.5 {
                    0.0
                } else {
                    1.0
                }
            }
            Shape::Wave => {
                let c = (steps(t) / 2) as f32;
                0.5 - 0.5 * (std::f32::consts::TAU * u * c).cos()
            }
        }
    }
}

/// The single curve: `u^p` for a positive tension, its mirror for a negative one, a line
/// at zero. `p` runs from 1 to 8 — steep enough for a filter sweep that waits and then
/// opens, which is what the far end of FL's tension is for.
#[inline]
fn bend(u: f32, t: f32) -> f32 {
    if t == 0.0 {
        return u;
    }
    let p = 1.0 + 7.0 * t.abs();
    if t > 0.0 {
        u.powf(p)
    } else {
        1.0 - (1.0 - u).powf(p)
    }
}

/// Stairs, pulses and waves: 2…16 by tension (even, so a pulse or a wave ends where it
/// began a cycle).
#[inline]
fn steps(t: f32) -> u32 {
    2 * (1 + (t.abs() * 7.0).round() as u32)
}

/// One point of an automation clip, in the clip's content ticks.
#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AutoPoint {
    pub tick: Ticks,
    /// 0…1: the bottom of the target's control to its top.
    pub value: f32,
    /// −1…1: the bend of the curve **from** this point to the next.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub tension: f32,
    /// How the curve travels from this point to the next.
    #[serde(default, skip_serializing_if = "is_curve")]
    pub shape: Shape,
}

fn is_zero(x: &f32) -> bool {
    *x == 0.0
}

fn is_curve(s: &Shape) -> bool {
    *s == Shape::Curve
}

impl AutoPoint {
    pub fn new(tick: Ticks, value: f32) -> Self {
        AutoPoint {
            tick,
            value,
            tension: 0.0,
            shape: Shape::Curve,
        }
    }
}

/// The content of an automation clip: what it drives and the curve.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Automation {
    pub target: AutoTarget,
    /// Sorted by tick (ties keep their order: two points at one tick are a jump).
    pub points: Vec<AutoPoint>,
}

impl Automation {
    pub fn new(target: AutoTarget, points: Vec<AutoPoint>) -> Self {
        let mut a = Automation { target, points };
        a.sort();
        a
    }

    /// Sort the points by tick, keeping the order of points at one tick.
    pub fn sort(&mut self) {
        self.points.sort_by_key(|p| p.tick);
    }

    /// Whether every point is finite, in range and in order — what a command accepts.
    pub fn is_valid(&self) -> bool {
        self.points.windows(2).all(|w| w[0].tick <= w[1].tick)
            && self.points.iter().all(|p| {
                p.value.is_finite()
                    && (0.0..=1.0).contains(&p.value)
                    && p.tension.is_finite()
                    && (-1.0..=1.0).contains(&p.tension)
            })
    }

    /// The curve's value (0…1) at a content tick; `None` for a clip with no points.
    /// Before the first point it holds the first value, after the last the last one.
    pub fn value_at(&self, content: f64) -> Option<f32> {
        let pts = &self.points;
        let first = pts.first()?;
        let i = pts.partition_point(|p| p.tick.0 as f64 <= content);
        if i == 0 {
            return Some(first.value);
        }
        let p = pts[i - 1];
        let Some(q) = pts.get(i) else {
            return Some(p.value);
        };
        Some(segment(p, *q, content))
    }
}

/// The value between two points at content tick `c` (`p.tick <= c < q.tick`).
#[inline]
pub fn segment(p: AutoPoint, q: AutoPoint, c: f64) -> f32 {
    let span = (q.tick.0 - p.tick.0) as f64;
    if span <= 0.0 {
        return q.value;
    }
    let u = ((c - p.tick.0 as f64) / span) as f32;
    p.value + (q.value - p.value) * p.shape.at(p.tension, u)
}

#[cfg(test)]
mod tests;

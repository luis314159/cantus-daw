//! Clip gain envelopes (schema 44, ADR 0070): a gain line drawn on an audio clip, on top
//! of its `gain_db` and its fades.
//!
//! The points are in the clip's **content time** — the ticks of `Clip::offset`, where
//! content tick 0 is the source's first frame — so the line belongs to the audio it
//! shapes: a slip carries it, a trim hides points (and a trim back shows them again), a
//! split gives both halves the same line (each hears its own stretch of it), a reversed
//! clip plays it mirrored with its audio. Between two points the value travels on
//! automation's shapes (`crate::automation::Shape`, the same tension), in dB.

use crate::automation::Shape;
use crate::time::Ticks;
use cantus_dsp::math;
use serde::{Deserialize, Serialize};

/// The line's floor, dB: a point here is silence (drawn as −∞).
pub const ENV_MIN_DB: f32 = -60.0;
/// The line's ceiling, dB (Pro Tools' clip gain goes to +36; +12 is where a clip line is
/// still a level, not a distortion).
pub const ENV_MAX_DB: f32 = 12.0;
/// The most points one envelope holds (a command refuses more).
pub const MAX_ENV_POINTS: usize = 4096;

/// One point of a clip's gain envelope.
#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GainPoint {
    /// In the clip's content ticks.
    pub tick: Ticks,
    /// [`ENV_MIN_DB`]…[`ENV_MAX_DB`].
    pub db: f32,
    /// −1…1: the bend of the line **from** this point to the next.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub tension: f32,
    /// How the line travels from this point to the next.
    #[serde(default, skip_serializing_if = "is_curve")]
    pub shape: Shape,
}

fn is_zero(x: &f32) -> bool {
    *x == 0.0
}

fn is_curve(s: &Shape) -> bool {
    *s == Shape::Curve
}

impl GainPoint {
    pub fn new(tick: Ticks, db: f32) -> Self {
        GainPoint {
            tick,
            db,
            tension: 0.0,
            shape: Shape::Curve,
        }
    }
}

/// A clip's gain envelope: its points, sorted by tick (two at one tick are a jump).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct GainEnvelope {
    pub points: Vec<GainPoint>,
}

impl GainEnvelope {
    pub fn new(mut points: Vec<GainPoint>) -> Self {
        points.sort_by_key(|p| p.tick);
        GainEnvelope { points }
    }

    /// Whether every point is finite, in range and in order, and there are not too many —
    /// what a command accepts.
    pub fn is_valid(&self) -> bool {
        self.points.len() <= MAX_ENV_POINTS
            && self.points.windows(2).all(|w| w[0].tick <= w[1].tick)
            && self.points.iter().all(|p| {
                p.db.is_finite()
                    && (ENV_MIN_DB..=ENV_MAX_DB).contains(&p.db)
                    && p.tension.is_finite()
                    && (-1.0..=1.0).contains(&p.tension)
            })
    }

    /// Whether the envelope changes nothing: no points, or every one at 0 dB (any shape
    /// between two zeros stays at zero). The snapshot drops such an envelope, so the
    /// engine plays the clip as it did without one, to the bit.
    pub fn is_flat(&self) -> bool {
        self.points.iter().all(|p| p.db == 0.0)
    }

    /// The line's value, dB, at a content tick: the first point's before it, the last's
    /// after it.
    pub fn db_at(&self, content: f64) -> f32 {
        let pts = &self.points;
        let Some(first) = pts.first() else {
            return 0.0;
        };
        let i = pts.partition_point(|p| p.tick.0 as f64 <= content);
        if i == 0 {
            return first.db;
        }
        let p = pts[i - 1];
        let Some(q) = pts.get(i) else {
            return p.db;
        };
        let span = (q.tick.0 - p.tick.0) as f64;
        if span <= 0.0 {
            return q.db;
        }
        segment(&p, q, ((content - p.tick.0 as f64) / span) as f32)
    }

    /// The pieces of the line inside `[from, to)` of content time, as points: those in it,
    /// with a point on each edge holding the line's value there — what one window of a
    /// glued pair contributes ([`GainEnvelope::glued`]).
    pub fn within(&self, from: Ticks, to: Ticks) -> Vec<GainPoint> {
        let mut out = vec![GainPoint {
            tick: from,
            db: self.db_at(from.0 as f64),
            ..self.shape_at(from)
        }];
        out.extend(
            self.points
                .iter()
                .copied()
                .filter(|p| p.tick > from && p.tick < to),
        );
        out.push(GainPoint::new(to, self.db_at(to.0 as f64)));
        out
    }

    /// The shape and tension of the segment that `t` lies in (a point put there keeps the
    /// line's travel to the next one as close as one point can).
    fn shape_at(&self, t: Ticks) -> GainPoint {
        let i = self.points.partition_point(|p| p.tick <= t);
        match i.checked_sub(1).and_then(|k| self.points.get(k)) {
            Some(p) if i < self.points.len() => GainPoint {
                shape: p.shape,
                tension: p.tension,
                ..GainPoint::new(t, 0.0)
            },
            _ => GainPoint::new(t, 0.0),
        }
    }

    /// The envelope of a glued clip (ADR 0070 §4): each window's own line over the content
    /// it showed, joined. `pieces` are `(envelope, content from, content to)` in order;
    /// `None` when no piece had a line. Identical lines on every piece (a split's halves)
    /// stay the line they were.
    pub fn glued(pieces: &[(Option<&GainEnvelope>, Ticks, Ticks)]) -> Option<GainEnvelope> {
        if pieces.iter().all(|p| p.0.is_none_or(|e| e.is_flat())) {
            return None;
        }
        if let Some(first) = pieces[0].0 {
            if pieces.iter().all(|p| p.0 == Some(first)) {
                return Some(first.clone());
            }
        }
        let flat = GainEnvelope::default();
        let mut points: Vec<GainPoint> = Vec::new();
        for (e, from, to) in pieces {
            let (lo, hi) = if from <= to {
                (*from, *to)
            } else {
                (*to, *from)
            };
            points.extend(e.unwrap_or(&flat).within(lo, hi));
        }
        points.sort_by_key(|p| p.tick);
        // Two equal points at one tick (the joins) are one.
        points.dedup_by(|b, a| a.tick == b.tick && a.db == b.db);
        points.truncate(MAX_ENV_POINTS);
        Some(GainEnvelope { points })
    }
}

/// Make an envelope one a command would take (a hand-edited or damaged file — review 1:
/// an infinite or unsorted point reached the audio thread): points that are not numbers
/// dropped, values and tensions clamped into range, the points sorted, at most
/// [`MAX_ENV_POINTS`] kept. Whether anything changed.
pub fn repair(e: &mut GainEnvelope) -> bool {
    let before = e.clone();
    e.points
        .retain(|p| p.db.is_finite() || p.db == f32::NEG_INFINITY);
    for p in &mut e.points {
        p.db = if p.db.is_finite() {
            p.db.clamp(ENV_MIN_DB, ENV_MAX_DB)
        } else {
            ENV_MIN_DB
        };
        p.tension = if p.tension.is_finite() {
            p.tension.clamp(-1.0, 1.0)
        } else {
            0.0
        };
    }
    e.points.sort_by_key(|p| p.tick);
    e.points.truncate(MAX_ENV_POINTS);
    *e != before
}

/// [`repair`] every envelope of a project (the clips and the windows alternate comps
/// keep); returns how many it changed. An envelope left with no points goes.
pub fn repair_project(project: &mut crate::Project) -> usize {
    let mut n = 0;
    for t in &mut project.tracks {
        let windows = t.comps.iter_mut().flat_map(|c| c.windows.iter_mut());
        for c in t.clips.iter_mut().chain(windows) {
            let Some(a) = c.audio.as_mut() else {
                continue;
            };
            if let Some(e) = a.gain_env.as_mut() {
                if repair(e) {
                    n += 1;
                }
                if e.points.is_empty() {
                    a.gain_env = None;
                }
            }
        }
    }
    n
}

/// The value between two points, `u` (0…1) of the way from `p` to `q`, dB.
#[inline]
pub fn segment(p: &GainPoint, q: &GainPoint, u: f32) -> f32 {
    if p.db == q.db {
        return p.db;
    }
    p.db + (q.db - p.db) * p.shape.at(p.tension, u)
}

/// The linear gain of a line's value: silence at the floor.
#[inline]
pub fn gain_of(db: f32) -> f32 {
    if db <= ENV_MIN_DB {
        0.0
    } else if db == 0.0 {
        1.0
    } else {
        math::db_to_gain(db)
    }
}

#[cfg(test)]
#[path = "clip_gain_tests.rs"]
mod tests;

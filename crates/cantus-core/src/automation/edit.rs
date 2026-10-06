//! Editing a curve: FL's articulators and the editor's point operations, as pure
//! functions over a point list, so the arrangement's clip, the large editor and a test
//! all reach the same code. Every one returns a sorted, in-range list; the caller wraps
//! it in one `SetClipAutomation` (one undo step).

use super::{AutoPoint, Shape};
use crate::time::Ticks;
use cantus_dsp::math;

/// Clamp and sort: what every edit returns.
pub fn tidy(points: &mut [AutoPoint]) {
    for p in points.iter_mut() {
        p.value = if p.value.is_finite() {
            p.value.clamp(0.0, 1.0)
        } else {
            0.5
        };
        p.tension = if p.tension.is_finite() {
            p.tension.clamp(-1.0, 1.0)
        } else {
            0.0
        };
        p.tick = p.tick.max(Ticks::ZERO);
    }
    points.sort_by_key(|p| p.tick);
}

/// Insert a point where it belongs; returns its index.
pub fn insert(points: &mut Vec<AutoPoint>, p: AutoPoint) -> usize {
    let i = points.partition_point(|q| q.tick <= p.tick);
    points.insert(i, p);
    i
}

/// Upside down: every value becomes `1 − value` (over the selection, or all).
pub fn flip_vertical(points: &mut [AutoPoint], sel: &[usize]) {
    for (i, p) in points.iter_mut().enumerate() {
        if sel.is_empty() || sel.contains(&i) {
            p.value = 1.0 - p.value;
        }
    }
}

/// Back to front over `[from, to]`: a point at `t` moves to `from + to − t`. A curve's
/// bend belongs to the segment *after* a point, so it moves to the point that now starts
/// that segment, and changes sign (the same bend, travelled the other way).
pub fn flip_horizontal(points: &mut [AutoPoint], from: Ticks, to: Ticks) {
    let n = points.len();
    if n == 0 {
        return;
    }
    let old = points.to_vec();
    for (i, p) in points.iter_mut().enumerate() {
        let src = &old[n - 1 - i];
        p.tick = from + to - src.tick;
        p.value = src.value;
        // The segment now starting at i was old segment (n-2-i).
        match n.checked_sub(2 + i).map(|k| &old[k]) {
            Some(seg) => {
                p.shape = seg.shape;
                p.tension = -seg.tension;
            }
            None => {
                p.shape = Shape::Curve;
                p.tension = 0.0;
            }
        }
    }
    tidy(points);
}

/// Scale values about `pivot` (0…1) by `factor`; 0 flattens to the pivot, 1 leaves it.
pub fn scale_values(points: &mut [AutoPoint], sel: &[usize], pivot: f32, factor: f32) {
    for (i, p) in points.iter_mut().enumerate() {
        if sel.is_empty() || sel.contains(&i) {
            p.value = (pivot + (p.value - pivot) * factor).clamp(0.0, 1.0);
        }
    }
}

/// Stretch time about `anchor` by `factor` (2 = twice as long).
pub fn scale_time(points: &mut [AutoPoint], anchor: Ticks, factor: f64) {
    if !(factor.is_finite() && factor > 0.0) {
        return;
    }
    for p in points.iter_mut() {
        let d = (p.tick - anchor).0 as f64 * factor;
        p.tick = anchor + Ticks(d.round() as i64);
    }
    tidy(points);
}

/// Normalise: stretch the values so the lowest is 0 and the highest 1.
pub fn normalize(points: &mut [AutoPoint]) {
    let (lo, hi) = points.iter().fold((f32::MAX, f32::MIN), |(lo, hi), p| {
        (lo.min(p.value), hi.max(p.value))
    });
    if hi - lo > 1e-6 {
        for p in points.iter_mut() {
            p.value = (p.value - lo) / (hi - lo);
        }
    }
}

/// Smooth: each value moves towards the mean of its neighbours by `amount` (0…1); the
/// ends stay. Twice the same call is smoother still, as a brush stroke is.
pub fn smooth(points: &mut [AutoPoint], amount: f32) {
    let n = points.len();
    if n < 3 {
        return;
    }
    let a = amount.clamp(0.0, 1.0);
    let old: Vec<f32> = points.iter().map(|p| p.value).collect();
    for i in 1..n - 1 {
        let mean = (old[i - 1] + old[i + 1]) * 0.5;
        points[i].value = old[i] + (mean - old[i]) * a;
    }
}

/// Randomise a little: every value moves by up to `amount` (0…1 of the range), from a
/// seed, so the same click gives the same result and undo/redo are exact.
pub fn randomize(points: &mut [AutoPoint], sel: &[usize], amount: f32, seed: u64) {
    let mut s = seed ^ 0x9E37_79B9_7F4A_7C15;
    for (i, p) in points.iter_mut().enumerate() {
        // splitmix64: one well-mixed number per point.
        s = s.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = s;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^= z >> 31;
        if sel.is_empty() || sel.contains(&i) {
            let r = (z >> 11) as f32 / (1u64 << 53) as f32 * 2.0 - 1.0;
            p.value = (p.value + r * amount).clamp(0.0, 1.0);
        }
    }
}

/// The LFO shapes FL's *create sequence* draws.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Lfo {
    Sine,
    Triangle,
    Saw,
    Square,
    /// Sample-and-hold: a seeded random step per cycle.
    Random,
}

impl Lfo {
    pub const ALL: [Lfo; 5] = [Lfo::Sine, Lfo::Triangle, Lfo::Saw, Lfo::Square, Lfo::Random];

    pub fn label_key(self) -> &'static str {
        match self {
            Lfo::Sine => "auto.lfo.sine",
            Lfo::Triangle => "auto.lfo.triangle",
            Lfo::Saw => "auto.lfo.saw",
            Lfo::Square => "auto.lfo.square",
            Lfo::Random => "auto.lfo.random",
        }
    }
}

/// Replace the curve over `[from, to)` with `cycles` cycles of an LFO between `lo` and
/// `hi`, with as few points as draw it exactly: a sine is a chain of S-curves between its
/// peaks, a triangle straight lines, a saw a ramp and a jump, a square holds.
pub fn lfo(
    shape: Lfo,
    from: Ticks,
    to: Ticks,
    cycles: u32,
    lo: f32,
    hi: f32,
    seed: u64,
) -> Vec<AutoPoint> {
    let cycles = cycles.max(1) as i64;
    let len = (to - from).0.max(cycles);
    let at = |num: i64, den: i64| from + Ticks(len * num / den);
    let pt = |tick, value: f32, shape| AutoPoint {
        tick,
        value: value.clamp(0.0, 1.0),
        tension: 0.0,
        shape,
    };
    let mut out = Vec::new();
    let mut held = lo;
    for c in 0..cycles {
        match shape {
            Lfo::Sine => {
                out.push(pt(at(2 * c, 2 * cycles), lo, Shape::SCurve));
                out.push(pt(at(2 * c + 1, 2 * cycles), hi, Shape::SCurve));
            }
            Lfo::Triangle => {
                out.push(pt(at(2 * c, 2 * cycles), lo, Shape::Curve));
                out.push(pt(at(2 * c + 1, 2 * cycles), hi, Shape::Curve));
            }
            Lfo::Saw => {
                out.push(pt(at(c, cycles), lo, Shape::Curve));
                out.push(pt(at(c + 1, cycles) - Ticks(1), hi, Shape::Hold));
            }
            Lfo::Square => {
                out.push(pt(at(2 * c, 2 * cycles), hi, Shape::Hold));
                out.push(pt(at(2 * c + 1, 2 * cycles), lo, Shape::Hold));
            }
            Lfo::Random => {
                let mut p = [pt(at(c, cycles), 0.5, Shape::Hold)];
                randomize(&mut p, &[], 0.5, seed.wrapping_add(c as u64));
                held = lo + (hi - lo) * p[0].value;
                out.push(pt(at(c, cycles), held, Shape::Hold));
            }
        }
    }
    // Close the last cycle where it began, so the next clip or the hold starts clean.
    let last = match shape {
        Lfo::Sine | Lfo::Triangle => lo,
        Lfo::Saw => hi,
        Lfo::Square => lo,
        Lfo::Random => held,
    };
    out.push(pt(to.max(from + Ticks(1)), last, Shape::Hold));
    out
}

/// Replace the points of `points` inside `[from, to)` with `new` (for an LFO drawn over
/// part of a clip).
pub fn splice(points: &mut Vec<AutoPoint>, from: Ticks, to: Ticks, new: Vec<AutoPoint>) {
    points.retain(|p| p.tick < from || p.tick >= to);
    points.extend(new);
    tidy(points);
}

/// Set the bend of the segment starting at `i` from where its midpoint was dragged to:
/// the curve passes through `(0.5, mid)` (0…1 between its two values). Inverse of the
/// single curve at u = ½, so the handle stays under the pointer.
pub fn tension_for_mid(shape: Shape, mid: f32) -> f32 {
    let m = mid.clamp(0.002, 0.998);
    match shape {
        Shape::Curve => {
            // bend(0.5) = 0.5^p for t > 0 (below the line), 1 − 0.5^p for t < 0.
            let (below, y) = if m <= 0.5 {
                (true, m)
            } else {
                (false, 1.0 - m)
            };
            let p = math::ln(y) / math::ln(0.5f32); // 0.5^p = y
            let t = ((p - 1.0) / 7.0).clamp(0.0, 1.0);
            if below {
                t
            } else {
                -t
            }
        }
        // The others read tension as a count or a steepness: the drag's height, up = more.
        _ => (m * 2.0 - 1.0).clamp(-1.0, 1.0),
    }
}

//! The match-EQ suggestion: a curve that moves the mix's tonal balance toward the
//! reference's by a chosen amount — a magnitude curve (Hz, dB), which the engine hands to
//! Aequalis's fit ([`crate::aequalis::fit::fit`]) to become a few editable bands, in the
//! phase mode the user picks there (minimum phase: no latency; linear phase: no phase
//! change). Comparatio never filters the mix itself.
//!
//! How the curve is made, band by band from a [`Difference`]:
//!
//! 1. **The wish**: `reference − mix`, weighed by the band's confidence (a band the
//!    comparison is unsure of asks for less), and nothing outside `lo_hz … hi_hz` (faded
//!    over a third of an octave at each end).
//! 2. **Smoothed** over log frequency by a Gaussian `smooth_oct` octaves wide (its
//!    standard deviation), so the curve follows the balance, not each band's wobble; the
//!    weights are the confidences, so a trusted band speaks for its untrusted neighbours.
//! 3. **Scaled** by `amount` (0 … 1) and **limited** softly to ±`max_db`
//!    ([`limit`]: a move up to half the limit passes exactly, a larger one bends smoothly
//!    toward the limit and never reaches it).
//! 4. **Level-free**: the loudness the curve would add is computed from the mix's own
//!    K-weighted power in each band (`Curve::share`) — `10·log₁₀ Σ share·10^(g/10)` — and
//!    returned as [`MatchCurve::makeup_db`], its negative, for the EQ's output gain: the
//!    suggestion changes the balance, not the loudness.

use super::tonal::{Difference, MAX_BANDS};
use crate::math;

/// How the suggestion is made.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct MatchOptions {
    /// How far toward the reference, 0 … 1.
    pub amount: f32,
    /// The most it boosts or cuts, dB.
    pub max_db: f32,
    /// The smoothing's width (standard deviation), octaves.
    pub smooth_oct: f32,
    pub lo_hz: f32,
    pub hi_hz: f32,
}

impl Default for MatchOptions {
    fn default() -> Self {
        MatchOptions {
            amount: 0.5,
            max_db: 6.0,
            smooth_oct: 0.33,
            lo_hz: 30.0,
            hi_hz: 16_000.0,
        }
    }
}

/// The suggestion: `count` points (Hz, dB).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct MatchCurve {
    pub count: usize,
    pub points: [(f32, f32); MAX_BANDS],
    /// The output gain that keeps the mix's loudness where it was, dB.
    pub makeup_db: f32,
}

impl MatchCurve {
    pub fn points(&self) -> &[(f32, f32)] {
        &self.points[..self.count]
    }
}

/// The edge fade at `hz` for the range `lo … hi`: 1 inside, to 0 a third of an octave out.
fn inside(hz: f32, lo: f32, hi: f32) -> f32 {
    let third = 1.0 / 3.0;
    let below = math::log2(lo / hz) / third;
    let above = math::log2(hz / hi) / third;
    (1.0 - below.max(above).max(0.0)).clamp(0.0, 1.0)
}

/// The soft limit: `x` itself up to `max/2`; beyond, `max/2 + (max/2)·tanh((|x| − max/2) /
/// (max/2))` with its sign — the same slope at the knee, and under `max` always.
pub fn limit(x: f32, max: f32) -> f32 {
    let k = 0.5 * max;
    if x.abs() <= k {
        x
    } else {
        x.signum() * (k + k * math::tanh((x.abs() - k) / k))
    }
}

/// The curve that moves the mix toward the reference (see the module notes).
pub fn suggest(diff: &Difference, opts: &MatchOptions) -> MatchCurve {
    let bands = &diff.bands;
    let n = bands.count;
    let mut wish = [0f32; MAX_BANDS];
    let mut weight = [0f32; MAX_BANDS];
    for b in 0..n {
        let edge = inside(bands.centre[b], opts.lo_hz, opts.hi_hz);
        let c = diff.confidence[b];
        if diff.db[b].is_finite() && c > 0.0 {
            wish[b] = -diff.db[b] * edge;
            weight[b] = c;
        }
    }
    // Smoothed over log frequency, the confidences as weights.
    let sigma = opts.smooth_oct.max(0.01);
    let mut smooth = [0f32; MAX_BANDS];
    for b in 0..n {
        let (mut num, mut den) = (0f32, 0f32);
        for j in 0..n {
            if weight[j] <= 0.0 {
                continue;
            }
            let d = math::log2(bands.centre[j] / bands.centre[b]) / sigma;
            let g = math::exp(-0.5 * d * d) * weight[j];
            num += g * wish[j];
            den += g;
        }
        let edge = inside(bands.centre[b], opts.lo_hz, opts.hi_hz);
        smooth[b] = if den > 1e-6 { num / den * edge } else { 0.0 };
    }
    let max = opts.max_db.max(0.1);
    let amount = opts.amount.clamp(0.0, 1.0);
    let mut out = MatchCurve {
        count: n,
        points: [(0.0, 0.0); MAX_BANDS],
        makeup_db: 0.0,
    };
    let mut loud = 0f64;
    let mut shares = 0f64;
    for b in 0..n {
        let g = limit(smooth[b] * amount, max);
        out.points[b] = (bands.centre[b], g);
        let share = diff.mix_share[b] as f64;
        loud += share * math::powf(10f64, g as f64 / 10.0);
        shares += share;
    }
    if shares > 0.0 && loud > 0.0 {
        out.makeup_db = -(10.0 * math::log10(loud / shares)) as f32;
    }
    out
}

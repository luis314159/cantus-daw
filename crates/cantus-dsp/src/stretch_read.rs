//! Reading a source **faster** than it was recorded without folding what lies above the
//! output's band back into it (round 29's review): an octave mip level (Exemplar's
//! half-band chain) brings the local speed into [1, 2), and a windowed sinc whose cut-off
//! is scaled by that speed removes what would alias — so the band follows `fs / 2k`
//! continuously instead of switching a whole octave at a time. Re-pitch reads through it
//! above speed 1; Rhythm's slices and Complex's percussive grains when transposed up.
//! At speed 1 or below the callers keep their own readers (a whole frame is the sample).

use crate::exemplar::resample::{mip_chain, MAX_TAPS};
use crate::math;
use std::sync::OnceLock;

/// Taps of every kernel (the most the resampler's tables hold).
const TAPS: usize = MAX_TAPS;
/// The Kaiser window's β (≈ 90 dB of stop band).
const BETA: f64 = 9.0;
/// Kernels per octave of local speed: speed `1 + i/STEPS` reads through kernel `i`.
const STEPS: usize = 16;
/// Fractional positions per kernel table.
const PHASES: usize = 128;
/// The pass band kept, as a share of the band the speed leaves (the window's transition
/// lies above it).
const KEEP: f64 = 0.95;

/// Kernel `i`'s table: `(PHASES + 1) × TAPS` coefficients, each row summing to 1.
fn tables() -> &'static Vec<Vec<f32>> {
    static T: OnceLock<Vec<Vec<f32>>> = OnceLock::new();
    T.get_or_init(|| {
        (0..=STEPS)
            .map(|i| {
                let cut = KEEP / (1.0 + i as f64 / STEPS as f64);
                let half = TAPS as f64 / 2.0;
                let mut t = vec![0.0f32; (PHASES + 1) * TAPS];
                for p in 0..=PHASES {
                    let frac = p as f64 / PHASES as f64;
                    let mut row = [0.0f64; TAPS];
                    let mut sum = 0.0;
                    for (j, v) in row.iter_mut().enumerate() {
                        let x = j as f64 - (half - 1.0) - frac;
                        *v = cut * sinc(cut * x) * kaiser(x / half);
                        sum += *v;
                    }
                    for (o, v) in t[p * TAPS..(p + 1) * TAPS].iter_mut().zip(row) {
                        *o = (v / sum) as f32;
                    }
                }
                t
            })
            .collect()
    })
}

fn sinc(x: f64) -> f64 {
    if x.abs() < 1e-12 {
        1.0
    } else {
        let a = std::f64::consts::PI * x;
        math::sin(a) / a
    }
}

/// The Kaiser window at `x` ∈ [−1, 1].
fn kaiser(x: f64) -> f64 {
    if x.abs() >= 1.0 {
        return 0.0;
    }
    i0(BETA * (1.0 - x * x).sqrt()) / i0(BETA)
}

/// The modified Bessel function of the first kind, order 0 (its series).
fn i0(x: f64) -> f64 {
    let (mut sum, mut term, q) = (1.0, 1.0, x * x / 4.0);
    for k in 1..64 {
        term *= q / (k * k) as f64;
        sum += term;
        if term < sum * 1e-17 {
            break;
        }
    }
    sum
}

/// One channel's span `[origin, origin + level 0's length)` and its mip levels, for reads
/// up to a top speed.
pub(super) struct Fast {
    origin: usize,
    /// Level 0 is the span itself; level `L` is it halved `L` times.
    levels: Vec<Vec<f32>>,
}

impl Fast {
    /// The levels of `x[from..to]` (clamped) for reads at speeds up to `top`: `from` is
    /// moved down to a multiple of the deepest level's step (so every level's samples sit
    /// on the source's own frames) and the span padded by the kernel's reach.
    pub(super) fn new(x: &[f32], from: f64, to: f64, top: f64) -> Fast {
        let deepest = if top > 1.0 {
            (math::log2(top).floor().max(0.0) as usize).min(7)
        } else {
            0
        };
        let step = 1usize << deepest;
        let pad = ((TAPS + 64) << deepest) as f64;
        let lo = ((from - pad).max(0.0) as usize / step) * step;
        let hi = ((to + pad).max(0.0).ceil() as usize)
            .min(x.len())
            .max(lo.min(x.len()));
        let lo = lo.min(hi);
        let span = x[lo..hi].to_vec();
        let mut levels = vec![span];
        if deepest > 0 {
            levels.extend(mip_chain(&levels[0], 1, deepest, false));
        }
        Fast { origin: lo, levels }
    }

    /// The source at `pos` (its own frames) read at `speed` (> 1) source frames per output
    /// frame, band-limited to what that speed leaves.
    pub(super) fn read(&self, pos: f64, speed: f64) -> f32 {
        let level =
            (math::log2(speed.max(1.0)).floor().max(0.0) as usize).min(self.levels.len() - 1);
        let local = speed / (1u64 << level) as f64;
        let i = (((local - 1.0) * STEPS as f64).ceil().max(0.0) as usize).min(STEPS);
        let data = &self.levels[level];
        let p = (pos - self.origin as f64) / (1u64 << level) as f64;
        let base = p.floor();
        let frac = p - base;
        let x = frac * PHASES as f64;
        let ph = (x as usize).min(PHASES - 1);
        let a = (x - ph as f64) as f32;
        let t = &tables()[i];
        let (r0, r1) = (
            &t[ph * TAPS..(ph + 1) * TAPS],
            &t[(ph + 1) * TAPS..(ph + 2) * TAPS],
        );
        let first = base as i64 - (TAPS as i64 / 2 - 1);
        let mut acc = 0.0f32;
        for j in 0..TAPS {
            let at = first + j as i64;
            if at < 0 || at as usize >= data.len() {
                continue;
            }
            let c = r0[j] + a * (r1[j] - r0[j]);
            acc += c * data[at as usize];
        }
        acc
    }
}

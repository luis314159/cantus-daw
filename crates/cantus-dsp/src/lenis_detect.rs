//! Lenis's detector: what in a frame's spectrum is a resonance, and how much to take off.
//!
//! ```text
//! |X|² ─ average (DET_MS) ─ dB ─┬─ fine:      [¼ ½ ¼] across three bins ───────────┐
//!                               └─ envelope:  box over W octaves, clipped twice,   │
//!                                             a second box (a triangle) ─────────── − ─► excess
//! excess − threshold ─ curve (soft 2:1 knee, or hard ∞:1) ─ × focus weight ─ ≤ depth
//!        ─ spread (max of three bins, then [¼ ½ ¼]) ─ attack / release ─► reduction, dB
//! ```
//!
//! - **The envelope** is the spectrum's own smooth outline, in dB, over a window of `W`
//!   octaves around each bin (log-symmetric, so it is as wide in pitch everywhere; never
//!   narrower than [`MIN_HALF`] bins a side, wider than the window's main lobe). A plain
//!   mean is pulled up by the very peaks it should stand under; so the spectrum is
//!   clipped at the first estimate plus [`CLIP_DB`] and smoothed again, twice — a robust
//!   mean in the spirit of a median (as harmonic–percussive separation smooths across
//!   frequency, Fitzgerald 2010), at a fraction of a median's cost: every box is a
//!   difference of two prefix sums, O(1) a bin. Sharpness sets `W`: a peak much narrower
//!   than `W` stands out of the envelope, a broad bump is followed by it.
//! - **The excess** is the fine spectrum over the envelope. Only what exceeds the
//!   threshold (Selectivity) is a resonance; the curve turns the excess into a reduction.
//! - **Spread and time**: a single bin is never cut alone (a needle in frequency is a long
//!   ring in time, and an isolated bin switching on and off is the "musical noise" of
//!   spectral subtraction — Berouti et al. 1979, Cappé 1994); the reduction is widened by
//!   one bin and smoothed, then follows the attack and release in dB, so it never jumps.

use super::{Channels, LenisSettings, Mode, FOCUS_BANDS};
use crate::math;

/// The detector's averaging of each bin's power, ms.
pub const DET_MS: f32 = 15.0;
/// The envelope's box is never narrower than this many bins on each side.
pub const MIN_HALF: usize = 4;
/// The envelope's robust passes clip the spectrum this far above the previous estimate, dB.
pub const CLIP_DB: f32 = 3.0;
/// The soft curve's knee, dB (centred on the threshold) and its ratio.
pub const SOFT_KNEE_DB: f32 = 6.0;
pub const SOFT_RATIO: f32 = 2.0;
/// A bin's level floor, dBFS: nothing under it is ever a resonance.
pub const FLOOR_DB: f32 = -140.0;
/// The focus limits fade over this many octaves outside them.
pub const LIMIT_FADE_OCT: f32 = 1.0 / 3.0;

/// The envelope's width, octaves, at a sharpness of 0 … 1: two octaves (broad bumps count
/// too) to a quarter of one (only narrow peaks).
pub fn envelope_octaves(sharpness: f32) -> f32 {
    2.0 * math::powf(0.125, sharpness.clamp(0.0, 1.0))
}

/// How far over the envelope a peak must stand before it is reduced, dB, at a selectivity
/// of 0 … 1.
pub fn threshold_db(selectivity: f32) -> f32 {
    2.0 + 10.0 * selectivity.clamp(0.0, 1.0)
}

/// The reduction for an `excess` over the threshold (dB, may be negative), before the
/// focus weight, capped at `depth` dB.
pub fn curve_db(excess: f32, depth: f32, mode: Mode) -> f32 {
    if depth <= 0.0 {
        return 0.0;
    }
    match mode {
        Mode::Hard => excess.clamp(0.0, depth),
        Mode::Soft => {
            let slope = 1.0 - 1.0 / SOFT_RATIO;
            let h = 0.5 * SOFT_KNEE_DB;
            let r = if excess <= -h {
                return 0.0;
            } else if excess < h {
                slope * (excess + h) * (excess + h) / (2.0 * SOFT_KNEE_DB)
            } else {
                slope * excess
            };
            depth * math::tanh(r / depth)
        }
    }
}

/// The focus curve at `hz`: 0 (left alone) … 2 (twice the reduction); 1 where no band
/// acts, inside the limits.
pub fn focus_weight(s: &LenisSettings, hz: f32) -> f32 {
    if hz <= 0.0 {
        return 0.0;
    }
    let o = math::log2(hz);
    let edge = |d: f32| {
        if d <= 0.0 {
            1.0
        } else if d >= LIMIT_FADE_OCT {
            0.0
        } else {
            0.5 + 0.5 * math::cos(std::f32::consts::PI * d / LIMIT_FADE_OCT)
        }
    };
    let lim = edge(math::log2(s.low_hz.max(1.0)) - o) * edge(o - math::log2(s.high_hz.max(1.0)));
    if lim == 0.0 {
        return 0.0;
    }
    let mut w = 1.0f32;
    for b in s.bands.iter().take(FOCUS_BANDS).filter(|b| b.on) {
        // A raised cosine in octaves, half height at ±width/2, zero at ±width.
        let half = b.width_oct.max(0.01);
        let d = (o - math::log2(b.hz.max(1.0))).abs();
        if d < half {
            w += b.amount.clamp(-1.0, 1.0)
                * (0.5 + 0.5 * math::cos(std::f32::consts::PI * d / half));
        }
    }
    lim * w.clamp(0.0, 2.0)
}

/// What the settings make of each bin: the focus weight and the envelope's box, and the
/// curve's numbers. Rebuilt only when the settings or the geometry change.
pub struct Design {
    pub weight: Vec<f32>,
    pub lo: Vec<u32>,
    pub hi: Vec<u32>,
    pub bins: usize,
    pub threshold: f32,
    pub depth: f32,
    pub mode: Mode,
    pub a_det: f32,
    pub a_att: f32,
    pub a_rel: f32,
    /// Raw power × `norm2` is power in dBFS terms (a full-scale sine at 0 dB).
    pub norm2: f32,
    /// Raw power under this is flushed to zero (−160 dBFS).
    pub flush: f32,
    pub linked: bool,
}

impl Design {
    pub fn new(max_bins: usize) -> Design {
        Design {
            weight: vec![0.0; max_bins],
            lo: vec![0; max_bins],
            hi: vec![0; max_bins],
            bins: 0,
            threshold: 0.0,
            depth: 0.0,
            mode: Mode::Soft,
            a_det: 1.0,
            a_att: 1.0,
            a_rel: 1.0,
            norm2: 1.0,
            flush: 0.0,
            linked: true,
        }
    }

    /// Build for `s` at `bins` bins of `sr / n` Hz, a hop of `hop` samples and an analysis
    /// window summing to `wsum`.
    pub fn build(&mut self, s: &LenisSettings, sr: f32, n: usize, hop: usize, wsum: f64) {
        let bins = n / 2 + 1;
        self.bins = bins;
        let half = 0.5 * envelope_octaves(s.sharpness);
        let (down, up) = (math::exp2(-half), math::exp2(half));
        let bin_hz = sr / n as f32;
        for k in 0..bins {
            self.weight[k] = focus_weight(s, k as f32 * bin_hz);
            let lo = ((k as f32 * down).floor() as usize).min(k.saturating_sub(MIN_HALF));
            let hi = ((k as f32 * up).ceil() as usize).max(k + MIN_HALF);
            self.lo[k] = lo as u32;
            self.hi[k] = hi.min(bins - 1) as u32;
        }
        self.threshold = threshold_db(s.selectivity);
        self.depth = s.depth_db.clamp(0.0, MAX_DEPTH_DB);
        self.mode = s.mode;
        let per = |ms: f32| {
            let frames = ms.max(0.0) * 1e-3 * sr / hop as f32;
            if frames <= 0.0 {
                1.0
            } else {
                1.0 - math::exp(-1.0 / frames)
            }
        };
        self.a_det = per(DET_MS);
        self.a_att = per(s.attack_ms.clamp(MIN_ATTACK_MS, MAX_ATTACK_MS));
        self.a_rel = per(s.release_ms.clamp(MIN_RELEASE_MS, MAX_RELEASE_MS));
        let norm = 2.0 / wsum;
        self.norm2 = (norm * norm) as f32;
        self.flush = (1e-16 / (norm * norm)) as f32;
        self.linked = s.channels == Channels::Linked;
    }
}

pub use super::{MAX_ATTACK_MS, MAX_DEPTH_DB, MAX_RELEASE_MS, MIN_ATTACK_MS, MIN_RELEASE_MS};

/// Working arrays one detector pass uses, shared by the detectors.
pub struct Scratch {
    /// The averaged spectrum, dBFS (the meter reads it after each pass).
    pub level: Vec<f32>,
    fine: Vec<f32>,
    env: Vec<f32>,
    clip: Vec<f32>,
    raw: Vec<f32>,
    prefix: Vec<f64>,
}

impl Scratch {
    pub fn new(max_bins: usize) -> Scratch {
        Scratch {
            level: vec![FLOOR_DB; max_bins],
            fine: vec![0.0; max_bins],
            env: vec![0.0; max_bins],
            clip: vec![0.0; max_bins],
            raw: vec![0.0; max_bins],
            prefix: vec![0.0; max_bins + 1],
        }
    }
}

/// `out[k]` = the mean of `x[lo[k]..=hi[k]]`.
fn boxed(x: &[f32], lo: &[u32], hi: &[u32], prefix: &mut [f64], out: &mut [f32]) {
    let n = x.len();
    prefix[0] = 0.0;
    for i in 0..n {
        prefix[i + 1] = prefix[i] + x[i] as f64;
    }
    for k in 0..n {
        let (a, b) = (lo[k] as usize, hi[k] as usize);
        out[k] = ((prefix[b + 1] - prefix[a]) / (b - a + 1) as f64) as f32;
    }
}

/// `[¼ ½ ¼]` across bins, the edges repeated.
fn three(x: &[f32], out: &mut [f32]) {
    let n = x.len();
    for k in 0..n {
        let a = x[k.saturating_sub(1)];
        let c = x[(k + 1).min(n - 1)];
        out[k] = 0.25 * a + 0.5 * x[k] + 0.25 * c;
    }
}

/// One detector: a channel's (or the linked pair's, or mid's, or side's) averaged power and
/// reduction per bin.
pub struct Detector {
    ps: Vec<f32>,
    /// The reduction now, dB ≥ 0, per bin.
    pub reduction: Vec<f32>,
    /// The gain now, per bin (exactly 1 where the reduction is 0).
    pub gain: Vec<f32>,
    /// Every averaged power and every reduction is +0.0.
    quiet: bool,
}

impl Detector {
    pub fn new(max_bins: usize) -> Detector {
        Detector {
            ps: vec![0.0; max_bins],
            reduction: vec![0.0; max_bins],
            gain: vec![1.0; max_bins],
            quiet: true,
        }
    }

    pub fn reset(&mut self) {
        self.ps.fill(0.0);
        self.reduction.fill(0.0);
        self.gain.fill(1.0);
        self.quiet = true;
    }

    /// Every average and reduction is zero: silence leaves it so.
    pub fn quiet(&self) -> bool {
        self.quiet
    }

    /// One frame. `p` is the frame's raw power per bin (`None`: an all-zero frame). Returns
    /// whether any bin's gain is not exactly 1.
    pub fn frame(&mut self, p: Option<&[f32]>, d: &Design, sc: &mut Scratch) -> bool {
        let k_n = d.bins;
        if p.is_none() && self.quiet {
            sc.level[..k_n].fill(FLOOR_DB);
            return false;
        }
        // The average, flushed to +0.0 under −160 dBFS.
        let mut any_power = false;
        for k in 0..k_n {
            let x = p.map_or(0.0, |p| p[k]);
            let mut v = self.ps[k] + d.a_det * (x - self.ps[k]);
            if v < d.flush {
                v = 0.0;
            }
            self.ps[k] = v;
            any_power |= v != 0.0;
            sc.level[k] = (10.0 * math::log10(v * d.norm2 + 1e-14)).max(FLOOR_DB);
        }
        let (level, fine, env, clip, raw) = (
            &sc.level[..k_n],
            &mut sc.fine[..k_n],
            &mut sc.env[..k_n],
            &mut sc.clip[..k_n],
            &mut sc.raw[..k_n],
        );
        let (lo, hi, prefix) = (&d.lo[..k_n], &d.hi[..k_n], &mut sc.prefix[..k_n + 1]);
        if d.depth > 0.0 && any_power {
            three(level, fine);
            boxed(level, lo, hi, prefix, env);
            for _ in 0..2 {
                for k in 0..k_n {
                    clip[k] = level[k].min(env[k] + CLIP_DB);
                }
                boxed(clip, lo, hi, prefix, env);
            }
            boxed(env, lo, hi, prefix, clip);
            for k in 0..k_n {
                let w = d.weight[k];
                fine[k] = if w > 0.0 && level[k] > FLOOR_DB {
                    w * curve_db(fine[k] - clip[k] - d.threshold, d.depth, d.mode)
                } else {
                    0.0
                };
            }
            // Spread: the most of three neighbours, then [¼ ½ ¼].
            for k in 0..k_n {
                let a = fine[k.saturating_sub(1)];
                let c = fine[(k + 1).min(k_n - 1)];
                clip[k] = a.max(fine[k]).max(c);
            }
            three(clip, raw);
        } else {
            raw.fill(0.0);
        }
        let mut touched = false;
        for k in 0..k_n {
            let t = raw[k];
            let r = self.reduction[k];
            let a = if t > r { d.a_att } else { d.a_rel };
            let mut v = r + (t - r) * a;
            if t == 0.0 && v < 1e-3 {
                v = 0.0;
            }
            self.reduction[k] = v;
            self.gain[k] = if v == 0.0 {
                1.0
            } else {
                touched = true;
                math::db_to_gain(-v)
            };
        }
        self.quiet = !any_power && !touched;
        touched
    }
}

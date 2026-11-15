//! Lenis's detector: what in a frame's spectrum is a resonance, and how much to take off.
//!
//! ```text
//! |X|² ─ average (DET_MS) ─ dB ─┬─ fine:      [¼ ½ ¼] across three bins ───────────┐
//!                               └─ envelope:  box over W octaves, clipped twice,   │
//!                                             a second box (a triangle) ─────────── − ─► excess
//! excess − threshold ─ curve (hard, or soft: a knee) × 2·depth·focus ─ ≤ the excess
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

/// The detector's averaging of each bin's power, ms (measured: `diagnose_the_excess`).
pub const DET_MS: f32 = 30.0;
/// The fast detector's averaging, ms, and how much further a peak must stand out on it:
/// a resonance that starts suddenly is caught at once, while noise's quick flickers (wider
/// on a short average) stay under the line (measured: `diagnose_the_excess`).
pub const FAST_MS: f32 = 5.0;
pub const FAST_MARGIN_DB: f32 = 4.0;
/// And the slow one never stands more than this over the fast one: when a resonance stops,
/// the fast average lets go at once and the reduction with it.
pub const FAST_HOLD_DB: f32 = 3.0;
/// The fine spectrum bridges the gaps between partials this far on each side, Hz: a sung
/// or played note's harmonics (up to about 240 Hz apart) are one smooth upper envelope, so
/// a resonance is a bump in it, never a harmonic itself.
pub const BRIDGE_HZ: f32 = 120.0;
/// Under 720 Hz the bridge is this fraction of the frequency instead (a sixth: low
/// resonances stay resolved; low partials are notes there, which the focus decides on).
pub const BRIDGE_FRACTION: f32 = 1.0 / 6.0;
/// The envelope's box is never narrower than this many bins on each side.
pub const MIN_HALF: usize = 4;
/// The envelope's robust passes clip the spectrum this far above the previous estimate, dB.
pub const CLIP_DB: f32 = 1.0;
/// How many times the envelope is clipped and smoothed again.
pub const PASSES: usize = 3;
/// At the edge of a band (a roll-off's knee) the envelope stays within this of the higher
/// side's mean, so the edge is never taken for a peak.
pub const EDGE_DB: f32 = 1.5;
/// The side means start `GUARD_MUL · bridge + 1` bins away from the bin.
pub const GUARD_MUL: usize = 2;
/// The soft curve's knee, dB: `e²/(e + K)` — no slope at the threshold, `e − K` far over it.
pub const SOFT_KNEE_DB: f32 = 4.0;
/// No bin is ever turned down further than this, dB.
pub const MAX_CUT_DB: f32 = 30.0;
/// The reduction is `DEPTH_SCALE · depth` times the excess over the threshold (Hard): at a
/// quarter of the depth a resonance comes down to the line.
pub const DEPTH_SCALE: f32 = 4.0;
/// A bin's level floor, dBFS: nothing under it is ever a resonance.
pub const FLOOR_DB: f32 = -140.0;
/// The focus limits fade over this many octaves outside them.
pub const LIMIT_FADE_OCT: f32 = 1.0 / 3.0;

/// The envelope's width, octaves, at a sharpness of 0 … 1: three octaves (broad bumps count
/// too) to three eighths of one (only narrow peaks).
pub fn envelope_octaves(sharpness: f32) -> f32 {
    3.0 * math::powf(0.125, sharpness.clamp(0.0, 1.0))
}

/// How far over the envelope a peak must stand before it is reduced, dB, at a selectivity
/// of 0 … 1.
pub fn threshold_db(selectivity: f32) -> f32 {
    1.5 + 5.0 * selectivity.clamp(0.0, 1.0)
}

/// The reduction, dB, for a bin standing `excess` dB over the envelope, at a threshold,
/// an amount `depth` (0 … 1, already weighted by the focus: up to 2) and a mode.
///
/// - **Hard**: the excess over the threshold, × 4·depth — at 25 % a resonance is brought
///   down to the line (the envelope + the threshold), beyond it toward the envelope.
/// - **Soft**: the same through `e²/(e + K)`, a knee 4 dB wide: a peak just over the
///   threshold is barely touched, a big one is treated almost as Hard treats it.
/// - **Never below the envelope**: no bin is turned down further than it stands out, nor
///   further than [`MAX_CUT_DB`].
pub fn reduction_db(excess: f32, threshold: f32, depth: f32, mode: Mode) -> f32 {
    let e = excess - threshold;
    if depth <= 0.0 || e <= 0.0 {
        return 0.0;
    }
    let c = match mode {
        Mode::Hard => e,
        Mode::Soft => e * e / (e + SOFT_KNEE_DB),
    };
    (DEPTH_SCALE * depth * c).min(excess).min(MAX_CUT_DB)
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
    /// Each bin's octave (`log2` of the bin number; bin 0 at half a bin), and the prefix
    /// sums of `u` and `u²` the fit reads.
    pub u: Vec<f64>,
    pub pu: Vec<f64>,
    pub puu: Vec<f64>,
    pub bins: usize,
    pub threshold: f32,
    pub depth: f32,
    pub mode: Mode,
    pub a_det: f32,
    pub a_fast: f32,
    pub a_att: f32,
    pub a_rel: f32,
    /// Raw power × `norm2` is power in dBFS terms (a full-scale sine at 0 dB).
    pub norm2: f32,
    /// Raw power under this is flushed to zero (−160 dBFS).
    pub flush: f32,
    /// The bridge across partials, bins on each side (the upper envelope and its triangle).
    pub bridge: Vec<u32>,
    /// The envelope's robust passes and their clipping, dB.
    pub passes: usize,
    pub clip_db: f32,
    /// How far under the higher side's mean the envelope may sit (a band's edge), dB.
    pub edge_db: f32,
    pub guard_mul: usize,
    pub linked: bool,
}

impl Design {
    pub fn new(max_bins: usize) -> Design {
        Design {
            weight: vec![0.0; max_bins],
            lo: vec![0; max_bins],
            hi: vec![0; max_bins],
            u: vec![0.0; max_bins],
            pu: vec![0.0; max_bins + 1],
            puu: vec![0.0; max_bins + 1],
            bins: 0,
            threshold: 0.0,
            depth: 0.0,
            mode: Mode::Soft,
            a_det: 1.0,
            a_fast: 1.0,
            a_att: 1.0,
            a_rel: 1.0,
            norm2: 1.0,
            flush: 0.0,
            bridge: vec![0; max_bins],
            passes: PASSES,
            clip_db: CLIP_DB,
            edge_db: EDGE_DB,
            guard_mul: GUARD_MUL,
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
        let bridge_at =
            |k: usize| (BRIDGE_HZ.min(k as f32 * bin_hz * BRIDGE_FRACTION) / bin_hz).round() as usize;
        for k in 0..bins {
            self.weight[k] = focus_weight(s, k as f32 * bin_hz);
            let br = bridge_at(k);
            self.bridge[k] = br as u32;
            let min_half = MIN_HALF + GUARD_MUL * br + 1;
            let lo = ((k as f32 * down).floor() as usize).min(k.saturating_sub(min_half));
            let hi = ((k as f32 * up).ceil() as usize).max(k + min_half);
            self.lo[k] = lo as u32;
            self.hi[k] = hi.min(bins - 1) as u32;
            let u = math::log2((k as f64).max(0.5));
            self.u[k] = u;
            self.pu[k + 1] = self.pu[k] + u;
            self.puu[k + 1] = self.puu[k] + u * u;
        }
        self.threshold = threshold_db(s.selectivity);
        self.depth = s.depth.clamp(0.0, 1.0);
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
        self.a_fast = per(FAST_MS);
        self.a_att = per(s.attack_ms.clamp(MIN_ATTACK_MS, MAX_ATTACK_MS));
        self.a_rel = per(s.release_ms.clamp(MIN_RELEASE_MS, MAX_RELEASE_MS));
        let norm = 2.0 / wsum;
        self.norm2 = (norm * norm) as f32;
        self.flush = (1e-16 / (norm * norm)) as f32;
        self.linked = s.channels == Channels::Linked;
    }
}

pub use super::{MAX_ATTACK_MS, MAX_RELEASE_MS, MIN_ATTACK_MS, MIN_RELEASE_MS};


/// Working arrays one detector pass uses, shared by the detectors.
pub struct Scratch {
    /// The slow average's spectrum, dBFS (the meter reads it after each pass).
    pub level: Vec<f32>,
    /// The excess the curve read (the slow one, raised to the fast one less
    /// [`FAST_MARGIN_DB`] and held under it plus [`FAST_HOLD_DB`]), dB, after each pass.
    pub excess: Vec<f32>,
    pub fast_level: Vec<f32>,
    pub fine: Vec<f32>,
    pub env: Vec<f32>,
    pub clip: Vec<f32>,
    raw: Vec<f32>,
    prefix: Vec<f64>,
    prefix_u: Vec<f64>,
}

impl Scratch {
    pub fn new(max_bins: usize) -> Scratch {
        Scratch {
            level: vec![FLOOR_DB; max_bins],
            excess: vec![0.0; max_bins],
            fast_level: vec![FLOOR_DB; max_bins],
            fine: vec![0.0; max_bins],
            env: vec![0.0; max_bins],
            clip: vec![0.0; max_bins],
            raw: vec![0.0; max_bins],
            prefix: vec![0.0; max_bins + 1],
            prefix_u: vec![0.0; max_bins + 1],
        }
    }
}

/// dBFS of a raw power.
#[inline]
fn db(p: f32, d: &Design) -> f32 {
    (10.0 * math::log10(p * d.norm2 + 1e-14)).max(FLOOR_DB)
}

/// One time scale's excess: the fine spectrum of the averaged power `ps` against the robust
/// envelope of its level `level` (dB), written into `out`.
fn excess_of(level: &[f32], d: &Design, sc: &mut ScratchView, out: &mut [f32]) {
    let k_n = level.len();

    // The upper envelope over the partials: the loudest within the bridge on each side…
    for k in 0..k_n {
        let b = d.bridge[k] as isize;
        let a = (k as isize - b).max(0) as usize;
        let z = ((k as isize + b) as usize).min(k_n - 1);
        sc.clip[k] = level[a..=z].iter().fold(FLOOR_DB, |m, v| m.max(*v));
    }
    // …smoothed by a triangle as wide.
    for k in 0..k_n {
        let (mut acc, mut wsum) = (0.0f32, 0.0f32);
        let b = d.bridge[k] as isize;
        for j in -b..=b {
            let i = (k as isize + j).clamp(0, k_n as isize - 1) as usize;
            let w = (b + 1 - j.abs()) as f32;
            acc += w * sc.clip[i];
            wsum += w;
        }
        sc.fine[k] = acc / wsum;
    }
    sided(sc.fine, d, sc.prefix, sc.prefix_u, sc.env);
    for _ in 0..d.passes {
        for k in 0..k_n {
            sc.clip[k] = sc.fine[k].min(sc.env[k] + d.clip_db);
        }
        sided(sc.clip, d, sc.prefix, sc.prefix_u, sc.env);
    }
    sided(sc.env, d, sc.prefix, sc.prefix_u, sc.clip);
    for k in 0..k_n {
        out[k] = sc.fine[k] - sc.clip[k];
    }
}

/// The scratch arrays one pass works in, cut to the bins in use.
struct ScratchView<'a> {
    fine: &'a mut [f32],
    env: &'a mut [f32],
    clip: &'a mut [f32],
    prefix: &'a mut [f64],
    prefix_u: &'a mut [f64],
}

/// One detector: a channel's (or the linked pair's, or mid's, or side's) averaged power on
/// two time scales, and its reduction per bin.
pub struct Detector {
    slow: Vec<f32>,
    fast: Vec<f32>,
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
            slow: vec![0.0; max_bins],
            fast: vec![0.0; max_bins],
            reduction: vec![0.0; max_bins],
            gain: vec![1.0; max_bins],
            quiet: true,
        }
    }

    pub fn reset(&mut self) {
        self.slow.fill(0.0);
        self.fast.fill(0.0);
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
            sc.excess[..k_n].fill(0.0);
            return false;
        }
        // The averages, flushed to +0.0 under −160 dBFS.
        let mut any_power = false;
        for k in 0..k_n {
            let x = p.map_or(0.0, |p| p[k]);
            let mut s = self.slow[k] + d.a_det * (x - self.slow[k]);
            let mut f = self.fast[k] + d.a_fast * (x - self.fast[k]);
            if s < d.flush {
                s = 0.0;
            }
            if f < d.flush {
                f = 0.0;
            }
            self.slow[k] = s;
            self.fast[k] = f;
            any_power |= s != 0.0 || f != 0.0;
            sc.level[k] = db(s, d);
            sc.fast_level[k] = db(f, d);
        }
        let mut view = ScratchView {
            fine: &mut sc.fine[..k_n],
            env: &mut sc.env[..k_n],
            clip: &mut sc.clip[..k_n],
            prefix: &mut sc.prefix[..k_n + 1],
            prefix_u: &mut sc.prefix_u[..k_n + 1],
        };
        let raw = &mut sc.raw[..k_n];
        let excess = &mut sc.excess[..k_n];
        if d.depth > 0.0 && any_power {
            // The slow excess into `excess`, the fast one into `raw`; the larger of the
            // slow and the fast less its margin is what the curve reads.
            excess_of(&sc.level[..k_n], d, &mut view, excess);
            excess_of(&sc.fast_level[..k_n], d, &mut view, raw);
            let level = &sc.level[..k_n];
            for k in 0..k_n {
                let e = excess[k]
                    .max(raw[k] - FAST_MARGIN_DB)
                    .min(raw[k] + FAST_HOLD_DB);
                excess[k] = e;
                let w = d.weight[k];
                view.fine[k] = if w > 0.0 && level[k] > FLOOR_DB {
                    reduction_db(e, d.threshold, w * d.depth, d.mode)
                } else {
                    0.0
                };
            }
            // Spread: the most of three neighbours, then [¼ ½ ¼].
            for k in 0..k_n {
                let a = view.fine[k.saturating_sub(1)];
                let c = view.fine[(k + 1).min(k_n - 1)];
                view.clip[k] = a.max(view.fine[k]).max(c);
            }
            three(view.clip, raw);
        } else {
            raw.fill(0.0);
            excess.fill(0.0);
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

/// `[¼ ½ ¼]` across bins, the edges repeated.
fn three(x: &[f32], out: &mut [f32]) {
    let n = x.len();
    for k in 0..n {
        let a = x[k.saturating_sub(1)];
        let c = x[(k + 1).min(n - 1)];
        out[k] = 0.25 * a + 0.5 * x[k] + 0.25 * c;
    }
}

/// The straight line fitted to `x[a..b]` against the bins' octave, read at bin `at`; `None`
/// with fewer than three bins.
#[inline]
fn line(d: &Design, px: &[f64], pux: &[f64], a: usize, b: usize, at: usize) -> Option<f64> {
    if b < a + 3 {
        return None;
    }
    let m = (b - a) as f64;
    let su = d.pu[b] - d.pu[a];
    let suu = d.puu[b] - d.puu[a];
    let sx = px[b] - px[a];
    let sux = pux[b] - pux[a];
    let den = m * suu - su * su;
    let mean = sx / m;
    Some(if den > 1e-12 {
        mean + (m * sux - su * sx) / den * (d.u[at] - su / m)
    } else {
        mean
    })
}

/// The envelope at each bin: the greatest of three estimates over the envelope's box — the
/// straight line (least squares, against the bins' octave) through the whole box, read at
/// `k`; the mean of the bins under `k`; the mean of the bins over it (both beyond a guard
/// of `3·bridge + 1` bins, clear of a peak's own bridged plateau). The two side means are
/// the "greatest of" cell-averaging detector of radar (GO-CFAR: Hansen & Sawyers, 1980),
/// here in dB across frequency: a peak stands over both sides, but the knee of a roll-off
/// or the edge of a band stands over one side only, so it is not taken for a peak. The line
/// follows a slope in octaves (pink noise, the low bins where the box is lopsided), where
/// a mean would sit under the bins it should stand on. Every sum is a difference of
/// prefix sums, in `f64`: O(1) a bin.
fn sided(x: &[f32], d: &Design, px: &mut [f64], pux: &mut [f64], out: &mut [f32]) {
    let n = x.len();
    px[0] = 0.0;
    pux[0] = 0.0;
    for i in 0..n {
        px[i + 1] = px[i] + x[i] as f64;
        pux[i + 1] = pux[i] + d.u[i] * x[i] as f64;
    }

    let mean = |a: usize, b: usize| (b >= a + 3).then(|| (px[b] - px[a]) / (b - a) as f64);
    for k in 0..n {
        let (lo, hi) = (d.lo[k] as usize, d.hi[k] as usize + 1);
        let both = line(d, px, pux, lo, hi, k).unwrap_or(x[k] as f64);
        let g = d.guard_mul * d.bridge[k] as usize + 1;
        let left = if k + 1 >= g + lo { mean(lo, k + 1 - g) } else { None };
        let right = mean((k + g).min(hi), hi);
        out[k] = match (left, right) {
            (Some(a), Some(b)) => both.max(0.5 * (a + b)).max(a.max(b) - d.edge_db as f64),
            (Some(a), None) | (None, Some(a)) => both.max(a - d.edge_db as f64),
            (None, None) => both,
        } as f32;
    }
}

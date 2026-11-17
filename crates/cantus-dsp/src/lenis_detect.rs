//! Lenis's detector: what in a frame's spectrum is a resonance, and how much to take off.
//!
//! ```text
//! |X|² ─┬─ slow average (30 ms) ─ dB ─ excess ─┐
//!       └─ fast average (5 ms)  ─ dB ─ excess ─┴─ the slow says whether, the fast how much (an onset)
//! a bin starting: the newest half frame's level over the slow upper envelope lifts the excess
//!   excess = upper envelope − envelope:
//!     upper envelope: the line through the prominent peaks (any partial spacing); under 800 Hz
//!                     a note's partials judged against their harmonic partners
//!     envelope:       the greatest of {line through the box, the sides' mean, the higher side − 3…6 dB},
//!                     the spectrum clipped at it + 1 dB and estimated again, three times
//! depth · gate(excess − threshold) · focus · excess ─ ≤ the excess
//!        ─ spread (the most of three bins, then [¼ ½ ¼]) ─ attack / release ─► reduction, dB
//! ```
//!
//! - **The upper envelope over the partials.** A note's harmonics are narrow peaks too; a
//!   detector that compared each bin with its neighbours would flatten every voice. So the
//!   envelope is first drawn through the spectrum's prominent peaks (`hull`): a note
//!   becomes the outline over its partials, however far apart, and a resonance is a peak
//!   over its neighbouring peaks. Under 800 Hz, where a note's first partials stand alone,
//!   a peak with harmonic partners is judged against them (`series_caps`).
//! - **The envelope** is that outline's own smooth shape over a box of `W` octaves around
//!   each bin (Sharpness sets `W`: 3 octaves to 3/8 of one; never narrower than
//!   seven bins a side). It is the greatest of three estimates:
//!   the least-squares line through the box against octaves (follows any slope, where a
//!   mean would sit under the bins at a lopsided box); the mean of the two sides beyond a
//!   guard (clear of a peak's own bridged plateau), each moved along the box's slope to the
//!   bin; and the higher side less 3 dB at a cliff, 6 on a slope — the "greatest of" detector
//!   of radar (GO-CFAR: Hansen & Sawyers, 1980), which keeps the knee of a roll-off or the
//!   edge of a band from being taken for a peak. The peaks are clipped out (at the estimate
//!   plus [`CLIP_DB`]) and the estimate made again, three times: a robust estimate in the
//!   spirit of a median (as harmonic–percussive separation smooths across frequency,
//!   Fitzgerald 2010), at the cost of prefix sums — O(1) a bin, the constants precomputed.
//! - **Two time scales.** The slow average (30 ms) is what tells a resonance from noise's
//!   flicker (measured: noise's 99.9th percentile excess +4.0 dB, a 12 dB resonance +10.6);
//!   once it says a resonance is there, the fast one (5 ms) says how much — an onset is
//!   measured at the fast pace — and, since the slow one may never stand more than 3 dB
//!   over it, it lets go as soon as the ringing stops.
//! - **The onset's lift** (third pass). The long frame is what resolves a resonance, but a
//!   sound that starts takes the whole frame (21 ms at Standard) to fill it: its peak
//!   reads low until then, by the window's shape, not by anything the averages could
//!   hurry. So a bin that crosses the threshold after [`CALM_MS`] under it, while its fast
//!   excess stands well over the threshold too (at least [`LIFT_ONSET_DB`]: noise crossing
//!   it does not), is *starting*: for as long as the frame and two fast time constants take
//!   to fill, its level is also read on a frame half as long — the newest samples only,
//!   through the same window shape, on the same bins (zero-padded) — and the slow excess is
//!   raised by how far that reading stands over the slow upper envelope (less
//!   [`LIFT_ALLOW_DB`], the shorter window's smaller scalloping between bins). A sine reads
//!   the same on both frames once filled, so the lift ends by itself as the long frame
//!   fills. 90 % of the cut comes 8 ms after a tone starts instead of 21; on noise nothing
//!   changes. Under [`SERIES_HZ`], where only the long frame can tell a note's start from a
//!   resonance, there is no lift. The short frame is transformed only in frames where a bin
//!   is starting.
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
/// Where the fast average stands this far over the slow one, the slow one catches up at the
/// fast rate — an onset is followed at once, the slow average's calm kept for the rest —
pub const CATCH_UP_DB: f32 = 1.0;
/// …and only where the fast excess already stands this far over the threshold.
pub const ONSET_DB: f32 = 3.0;
/// The onset's lift (measured: `the_time_response_follows_attack_and_release`,
/// `the_onset_lift_leaves_noise_alone`, `a_note_s_start_is_not_a_resonance`): a bin that
/// crosses the threshold after standing under it this long, ms, may be starting…
pub const CALM_MS: f32 = 30.0;
/// …if its fast excess stands this far over the envelope too (and [`ONSET_DB`] over the
/// threshold): noise crossing the threshold does not (6 dB let pink noise's 99.9th
/// percentile cut grow 0.6 dB at selectivity 0).
pub const LIFT_ONSET_DB: f32 = 8.0;
/// Then, for as long as the analysis frame and two fast time constants take to fill, its
/// level is also read on a frame this many times shorter (the newest samples only; a
/// sine reads the same on both once filled, noise on average 1 dB more on the short one)…
pub const LIFT_DIV: usize = 2;
/// …less this (between two bins the short window's scalloping is 1.6 dB smaller: 1 dB keeps
/// a tone between bins from overshooting by more than 0.8 dB, 1.5 dB lost Fast's gain)…
pub const LIFT_ALLOW_DB: f32 = 1.0;
/// …and the excess raised by at most this.
pub const MAX_LIFT_DB: f32 = 24.0;
/// The fine spectrum bridges the gaps between partials this far on each side, Hz: a sung
/// or played note's harmonics (up to about 240 Hz apart) are one smooth upper envelope, so
/// a resonance is a bump in it, never a harmonic itself.
pub const BRIDGE_HZ: f32 = 120.0;
/// Under 720 Hz the bridge is this fraction of the frequency instead (a sixth: low
/// resonances stay resolved; low partials are notes there, which the focus decides on).
pub const BRIDGE_FRACTION: f32 = 1.0 / 6.0;
/// The upper envelope drops a candidate peak this far under the line between its
/// neighbours (a valley's ripple), in this many passes; and bridges a gap of up to the lower
/// peak's own bin number (its frequency) plus this many bins.
pub const HULL_DROP_DB: f32 = 6.0;
pub const HULL_PASSES: usize = 3;
pub const HULL_MIN_GAP: usize = 4;
/// Under this frequency a peak with harmonic partners is judged against them, and a partner
/// counts no further than this under it.
pub const SERIES_HZ: f32 = 800.0;
pub const SERIES_RANGE_DB: f32 = 30.0;
/// The envelope's box is never narrower than this many bins on each side.
pub const MIN_HALF: usize = 4;
/// The envelope's robust passes clip the spectrum this far above the previous estimate, dB.
pub const CLIP_DB: f32 = 1.0;
/// How many times the envelope is clipped and smoothed again.
pub const PASSES: usize = 3;
/// At the edge of a band (a roll-off's knee) the envelope stays within this of the higher
/// side's mean, so the edge is never taken for a peak.
pub const EDGE_DB: f32 = 3.0;
/// On a straight slope the allowance is this instead; between, it follows how much the two
/// sides' means differ (from `KNEE_FROM_DB`, over `KNEE_SPAN_DB`).
pub const EDGE_SLOPE_DB: f32 = 6.0;
const KNEE_FROM_DB: f64 = 12.0;
const KNEE_SPAN_DB: f64 = 12.0;
/// The side means start `GUARD_MUL · bridge + 1` bins away from the bin.
pub const GUARD_MUL: usize = 2;
/// The soft curve's knee, dB: the gate `e/(e + K)`.
pub const SOFT_KNEE_DB: f32 = 4.0;
/// The hard curve's gate opens over this much excess over the threshold, dB.
pub const HARD_RAMP_DB: f32 = 2.0;
/// No bin is ever turned down further than this, dB.
pub const MAX_CUT_DB: f32 = 30.0;
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
/// an amount `depth` (0 … 1, already weighted by the focus: up to 2) and a mode: the
/// **share `depth` of the bump**, gated as it crosses the threshold — so at 100 % a
/// resonance is brought down to the envelope itself, at 50 % half way (in dB), and the
/// knob acts over its whole travel.
///
/// - **Hard**: the gate opens over the first [`HARD_RAMP_DB`] over the threshold.
/// - **Soft**: the gate is `e/(e + K)`, `K` = [`SOFT_KNEE_DB`]: a peak just over the
///   threshold is barely touched, a big one nearly as in Hard.
/// - **Never below the envelope** (not even with the focus at +100 %), nor further than
///   [`MAX_CUT_DB`].
pub fn reduction_db(excess: f32, threshold: f32, depth: f32, mode: Mode) -> f32 {
    let e = excess - threshold;
    if depth <= 0.0 || e <= 0.0 {
        return 0.0;
    }
    let gate = match mode {
        Mode::Hard => (e / HARD_RAMP_DB).min(1.0),
        Mode::Soft => e / (e + SOFT_KNEE_DB),
    };
    (depth * gate * excess).min(excess).min(MAX_CUT_DB)
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
#[derive(Clone)]
pub struct Design {
    pub weight: Vec<f32>,
    pub lo: Vec<u32>,
    pub hi: Vec<u32>,
    /// Each bin's octave (`log2` of the bin number; bin 0 at half a bin), and the prefix
    /// sums of `u` and `u²` the fit reads.
    pub u: Vec<f64>,
    pub pu: Vec<f64>,
    pub puu: Vec<f64>,
    /// Each bin's envelope estimate, precomputed.
    pub fits: Vec<Fit>,
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
    /// The allowance on a straight slope, dB.
    pub edge_slope_db: f32,
    /// [`FAST_MARGIN_DB`], adjustable for the measurements.
    pub fast_margin_db: f32,
    /// The power ratio of the fast average over the slow one past which the slow one catches
    /// up at the fast rate ([`CATCH_UP_DB`]).
    pub catch_up: f32,
    /// The side means moved along the box's slope to the bin.
    pub slope_sides: bool,
    /// The upper envelope through the prominent peaks (true), or the fixed bridge (false).
    pub hull: bool,
    /// [`SERIES_HZ`] in bins (0: no harmonic awareness).
    pub series_bins: usize,
    /// The onset's lift is on (the measurements turn it off to compare).
    pub lift: bool,
    /// `(Σwa / Σwd)²`: a short frame's power × this reads a sine as the long frame does
    /// (both in the same dBFS terms).
    pub lift_gain: f32,
    /// [`CALM_MS`], and the onset's window (the frame and two fast time constants), frames.
    pub calm_frames: u16,
    pub rise_frames: u16,
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
            fits: vec![Fit::default(); max_bins],
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
            edge_slope_db: EDGE_SLOPE_DB,
            fast_margin_db: FAST_MARGIN_DB,
            catch_up: 0.0,
            slope_sides: true,
            hull: true,
            series_bins: 0,
            lift: true,
            lift_gain: 1.0,
            calm_frames: 1,
            rise_frames: 0,
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
        let hull = self.hull;
        self.series_bins = if hull {
            (SERIES_HZ / bin_hz).round() as usize
        } else {
            0
        };
        let bridge_at = |k: usize| {
            if hull {
                1
            } else {
                (BRIDGE_HZ.min(k as f32 * bin_hz * BRIDGE_FRACTION) / bin_hz).round() as usize
            }
        };
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
        for k in 0..bins {
            let (a, b) = (self.lo[k] as usize, self.hi[k] as usize + 1);
            let m = (b - a) as f64;
            let su = self.pu[b] - self.pu[a];
            let suu = self.puu[b] - self.puu[a];
            let den = m * suu - su * su;
            let c = if den > 1e-12 {
                (self.u[k] - su / m) / den
            } else {
                0.0
            };
            let g = GUARD_MUL * self.bridge[k] as usize + 1;
            let (la, lb) = (a, (k + 1).saturating_sub(g).max(a));
            let (ra, rb) = ((k + g).min(b), b);
            let inv = |x: usize, y: usize| {
                if y >= x + 3 {
                    1.0 / (y - x) as f64
                } else {
                    0.0
                }
            };
            self.fits[k] = Fit {
                a: a as u32,
                b: b as u32,
                m,
                inv_m: 1.0 / m,
                su,
                c,
                la: la as u32,
                lb: lb as u32,
                inv_l: inv(la, lb),
                ra: ra as u32,
                rb: rb as u32,
                inv_r: inv(ra, rb),
                inv_den: if den > 1e-12 { 1.0 / den } else { 0.0 },
                du_l: if lb > la {
                    self.u[k] - (self.pu[lb] - self.pu[la]) / (lb - la) as f64
                } else {
                    0.0
                },
                du_r: if rb > ra {
                    self.u[k] - (self.pu[rb] - self.pu[ra]) / (rb - ra) as f64
                } else {
                    0.0
                },
            };
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
        let g = math::db_to_gain(CATCH_UP_DB);
        self.catch_up = g * g;
        self.a_att = per(s.attack_ms.clamp(MIN_ATTACK_MS, MAX_ATTACK_MS));
        self.a_rel = per(s.release_ms.clamp(MIN_RELEASE_MS, MAX_RELEASE_MS));
        let frames = |samples: f32| (samples / hop as f32).ceil().clamp(1.0, 4_000.0) as u16;
        self.calm_frames = frames(CALM_MS * 1e-3 * sr);
        self.rise_frames = frames(n as f32 + 2.0 * FAST_MS * 1e-3 * sr);
        let norm = 2.0 / wsum;
        self.norm2 = (norm * norm) as f32;
        self.flush = (1e-16 / (norm * norm)) as f32;
        self.linked = s.channels == Channels::Linked;
    }
}

pub use super::{MAX_ATTACK_MS, MAX_RELEASE_MS, MIN_ATTACK_MS, MIN_RELEASE_MS};

/// Working arrays one detector pass uses, shared by the detectors.
#[derive(Clone)]
pub struct Scratch {
    /// The slow average's spectrum, dBFS (the meter reads it after each pass).
    pub level: Vec<f32>,
    /// The excess the curve read (the slow one, raised to the fast one less
    /// [`FAST_MARGIN_DB`] and held under it plus [`FAST_HOLD_DB`]), dB, after each pass.
    pub excess: Vec<f32>,
    fast_level: Vec<f32>,
    fine: Vec<f32>,
    env: Vec<f32>,
    clip: Vec<f32>,
    raw: Vec<f32>,
    /// The slow excess's upper envelope (the onset's lift reads it).
    upper: Vec<f32>,
    /// The bins starting this frame (the onset's lift), with their slow excess.
    start_bins: Vec<u32>,
    start_slow: Vec<f32>,
    starting: usize,
    prefix: Vec<f64>,
    prefix_u: Vec<f64>,
    peaks: Vec<u32>,
    cands: Vec<u32>,
    /// The slow and the fast excess before they are combined (the measurements read them).
    #[cfg(test)]
    pub parts: Vec<(f32, f32, f32)>,
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
            upper: vec![0.0; max_bins],
            start_bins: vec![0; max_bins],
            start_slow: vec![0.0; max_bins],
            starting: 0,
            prefix: vec![0.0; max_bins + 1],
            prefix_u: vec![0.0; max_bins + 1],
            #[cfg(test)]
            parts: vec![(0.0, 0.0, 0.0); max_bins],
            peaks: vec![0; max_bins],
            cands: vec![0; max_bins],
        }
    }
}

/// dBFS of a raw power.
#[inline]
fn db(p: f32, d: &Design) -> f32 {
    (10.0 * fast_log10(p * d.norm2 + 1e-14)).max(FLOOR_DB)
}

/// `log10(x)` for a finite positive `f32`, to about 1e-6 (2e-5 dB on a power), in plain
/// arithmetic (the same bits on every machine, as [`crate::math`] promises): the exponent
/// from the bits, the mantissa moved into √½ … √2, and `ln m = 2·atanh t`, `t = (m − 1)/(m + 1)`
/// (|t| ≤ 0.172), to the seventh power. A third of the cost of the library call, which the
/// detector made twice a bin a frame.
#[inline]
fn fast_log10(x: f32) -> f32 {
    fast_log10_impl(x)
}

#[cfg(test)]
pub fn fast_log10_for_tests(x: f32) -> f32 {
    fast_log10_impl(x)
}

#[inline]
fn fast_log10_impl(x: f32) -> f32 {
    let bits = x.to_bits();
    let mut e = ((bits >> 23) & 0xff) as i32 - 127;
    let mut m = f32::from_bits((bits & 0x007f_ffff) | 0x3f80_0000);
    if m > std::f32::consts::SQRT_2 {
        m *= 0.5;
        e += 1;
    }
    let t = (m - 1.0) / (m + 1.0);
    let t2 = t * t;
    let ln_m = 2.0 * t * (1.0 + t2 * (1.0 / 3.0 + t2 * (0.2 + t2 * (1.0 / 7.0))));
    (e as f32 * std::f32::consts::LN_2 + ln_m) * std::f32::consts::LOG10_E
}

/// One time scale's excess: the fine spectrum of the averaged power `ps` against the robust
/// envelope of its level `level` (dB), written into `out`.
fn excess_of(level: &[f32], d: &Design, sc: &mut ScratchView, out: &mut [f32]) {
    let k_n = level.len();
    let mut nc = 0usize;

    if d.hull {
        nc = hull(level, sc.peaks, sc.cands, sc.clip);
    } else {
        // The upper envelope over the partials: the loudest within the bridge on each side…
        for k in 0..k_n {
            let b = d.bridge[k] as isize;
            let a = (k as isize - b).max(0) as usize;
            let z = ((k as isize + b) as usize).min(k_n - 1);
            sc.clip[k] = level[a..=z].iter().fold(FLOOR_DB, |m, v| m.max(*v));
        }
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
    for ((o, f), c) in out.iter_mut().zip(sc.fine.iter()).zip(sc.clip.iter()) {
        *o = f - c;
    }
    if nc > 0 && d.series_bins > 0 {
        series_caps(level, &sc.cands[..nc], d.series_bins, out);
    }
}

/// The scratch arrays one pass works in, cut to the bins in use.
struct ScratchView<'a> {
    fine: &'a mut [f32],
    env: &'a mut [f32],
    clip: &'a mut [f32],
    prefix: &'a mut [f64],
    prefix_u: &'a mut [f64],
    peaks: &'a mut [u32],
    cands: &'a mut [u32],
}

/// One detector: a channel's (or the linked pair's, or mid's, or side's) averaged power on
/// two time scales, and its reduction per bin.
#[derive(Clone)]
pub struct Detector {
    slow: Vec<f32>,
    fast: Vec<f32>,
    /// The last frame's fast excess (an onset is a fast excess well over the threshold).
    onset: Vec<f32>,
    /// The reduction now, dB ≥ 0, per bin.
    pub reduction: Vec<f32>,
    /// The gain now, per bin (exactly 1 where the reduction is 0).
    pub gain: Vec<f32>,
    /// Frames since the slow excess last stood over the threshold (`u16::MAX`: calm for
    /// [`CALM_MS`] or more — the state silence leaves).
    since: Vec<u16>,
    /// Frames of the onset's lift left.
    rise: Vec<u16>,
    /// Every averaged power and every reduction is +0.0.
    quiet: bool,
    /// This frame: all-zero on a quiet detector (nothing to do), and any power averaged.
    idle: bool,
    power: bool,
}

impl Detector {
    pub fn new(max_bins: usize) -> Detector {
        Detector {
            slow: vec![0.0; max_bins],
            fast: vec![0.0; max_bins],
            onset: vec![0.0; max_bins],
            reduction: vec![0.0; max_bins],
            gain: vec![1.0; max_bins],
            since: vec![u16::MAX; max_bins],
            rise: vec![0; max_bins],
            quiet: true,
            idle: true,
            power: false,
        }
    }

    pub fn reset(&mut self) {
        self.slow.fill(0.0);
        self.fast.fill(0.0);
        self.onset.fill(0.0);
        self.since.fill(u16::MAX);
        self.rise.fill(0);
        self.reduction.fill(0.0);
        self.gain.fill(1.0);
        self.quiet = true;
        self.idle = true;
        self.power = false;
    }

    /// Bin `k`'s slow and fast averaged power, raw (the measurements read them).
    #[cfg(test)]
    pub fn averages(&self, k: usize) -> (f32, f32) {
        (self.slow[k], self.fast[k])
    }

    /// Every average and reduction is zero: silence leaves it so.
    pub fn quiet(&self) -> bool {
        self.quiet
    }

    /// One frame, first half: the averages, the two excesses and the onsets. `p` is the
    /// frame's raw power per bin (`None`: an all-zero frame). Returns whether a bin is
    /// starting (the onset's lift): then [`Detector::finish`] wants the short frame's power.
    #[allow(clippy::needless_range_loop)] // several arrays a bin, side by side
    pub fn analyse(&mut self, p: Option<&[f32]>, d: &Design, sc: &mut Scratch) -> bool {
        let k_n = d.bins;
        sc.starting = 0;
        if p.is_none() && self.quiet {
            sc.level[..k_n].fill(FLOOR_DB);
            sc.excess[..k_n].fill(0.0);
            self.idle = true;
            return false;
        }
        self.idle = false;
        // The averages, flushed to +0.0 under −160 dBFS.
        let mut any_power = false;
        for k in 0..k_n {
            let x = p.map_or(0.0, |p| p[k]);
            let mut f = self.fast[k] + d.a_fast * (x - self.fast[k]);
            // The slow average catches up at the fast rate while the fast one stands far
            // over it (an onset), and keeps its own pace otherwise.
            let a = if f > d.catch_up * self.slow[k] && self.onset[k] > d.threshold + ONSET_DB {
                d.a_fast
            } else {
                d.a_det
            };
            let mut s = self.slow[k] + a * (x - self.slow[k]);
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
        self.power = any_power;
        let mut view = ScratchView {
            fine: &mut sc.fine[..k_n],
            env: &mut sc.env[..k_n],
            clip: &mut sc.clip[..k_n],
            prefix: &mut sc.prefix[..k_n + 1],
            prefix_u: &mut sc.prefix_u[..k_n + 1],
            peaks: &mut sc.peaks[..k_n],
            cands: &mut sc.cands[..k_n],
        };
        let raw = &mut sc.raw[..k_n];
        let excess = &mut sc.excess[..k_n];
        if !(d.depth > 0.0 && any_power) {
            raw.fill(0.0);
            self.onset[..k_n].fill(0.0);
            self.since[..k_n].fill(u16::MAX);
            self.rise[..k_n].fill(0);
            excess.fill(0.0);
            return false;
        }
        // The slow excess into `excess`, the fast one into `raw`.
        excess_of(&sc.level[..k_n], d, &mut view, excess);
        sc.upper[..k_n].copy_from_slice(view.fine);
        excess_of(&sc.fast_level[..k_n], d, &mut view, raw);
        self.onset[..k_n].copy_from_slice(raw);
        let mut starting = 0usize;
        for k in 0..k_n {
            // The slow excess is what says a resonance is there; once it says so (from
            // the threshold, fully 2 dB over it), the fast one says how much — the onset
            // measured at the fast pace, the noise judged at the slow one.
            let (slow, fast) = (excess[k], raw[k]);
            #[cfg(test)]
            {
                sc.parts[k] = (slow, fast, 0.0);
            }
            let confirmed = ((slow - d.threshold) / 2.0).clamp(0.0, 1.0);
            excess[k] = slow
                .max(fast - d.fast_margin_db)
                .max(slow + (fast - slow).max(0.0) * confirmed)
                .min(fast + FAST_HOLD_DB);
            // The onset's lift: a bin crossing the threshold after a calm is starting…
            if slow > d.threshold {
                if self.since[k] == u16::MAX {
                    self.rise[k] = d.rise_frames;
                }
                self.since[k] = 0;
            } else if self.since[k] != u16::MAX {
                let v = self.since[k] + 1;
                self.since[k] = if v >= d.calm_frames { u16::MAX } else { v };
            }
            if self.rise[k] > 0 {
                self.rise[k] -= 1;
                // …while the fast excess stands well over the threshold too, as a sound
                // starting does and noise crossing it does not; over the harmonic region,
                // where a note's start is told from a resonance only on the long frame.
                let onset = fast > (d.threshold + ONSET_DB).max(LIFT_ONSET_DB);
                if d.lift && onset && slow > d.threshold && k > d.series_bins {
                    sc.start_bins[starting] = k as u32;
                    sc.start_slow[starting] = slow;
                    starting += 1;
                }
            }
        }
        sc.starting = starting;
        starting > 0
    }

    /// One frame, second half: the starting bins read on the short frame `q` (its raw power
    /// per bin, when [`Detector::analyse`] asked for it), the curve, the spread and the
    /// time. Returns whether any bin's gain is not exactly 1.
    #[allow(clippy::needless_range_loop)] // several arrays a bin, side by side
    pub fn finish(&mut self, q: Option<&[f32]>, d: &Design, sc: &mut Scratch) -> bool {
        if self.idle {
            return false;
        }
        let k_n = d.bins;
        if d.depth > 0.0 && self.power {
            if let Some(q) = q {
                for i in 0..sc.starting {
                    let (k, slow) = (sc.start_bins[i] as usize, sc.start_slow[i]);
                    // Against the slow upper envelope (a bin the hull bridged reads it).
                    let ls = db(q[k] * d.lift_gain, d);
                    if ls > sc.upper[k] + LIFT_ALLOW_DB {
                        let lift = (ls - sc.upper[k] - LIFT_ALLOW_DB).min(MAX_LIFT_DB);
                        let confirmed = ((slow - d.threshold) / 2.0).clamp(0.0, 1.0);
                        sc.excess[k] = sc.excess[k].max(slow + lift * confirmed);
                        #[cfg(test)]
                        {
                            sc.parts[k].2 = lift;
                        }
                    }
                }
            }
            let (fine, clip) = (&mut sc.fine[..k_n], &mut sc.clip[..k_n]);
            for k in 0..k_n {
                let w = d.weight[k];
                fine[k] = if w > 0.0 && sc.level[k] > FLOOR_DB {
                    reduction_db(sc.excess[k], d.threshold, w * d.depth, d.mode)
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
            three(clip, &mut sc.raw[..k_n]);
        }
        let raw = &sc.raw[..k_n];
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
        self.quiet = !self.power && !touched;
        if self.quiet {
            // fx-b:lenis (phase B) — quiet, the detector is in the state silence leaves
            // and a reset makes: calm, no lift running, no onset remembered. Without this
            // a detector that went quiet within `CALM_MS` of a peak kept a short `since`
            // through any length of rest (an all-zero frame on a quiet detector is not
            // looked at), and its first frame back judged an onset differently from a
            // reset twin's (the engine's twins test, heavy settings: 1e-14 apart).
            self.onset.fill(0.0);
            self.since.fill(u16::MAX);
            self.rise.fill(0);
        }
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
    for (k, f) in d.fits[..n].iter().enumerate() {
        let (a, b) = (f.a as usize, f.b as usize);
        let sx = px[b] - px[a];
        let sux = pux[b] - pux[a];
        let line = sx * f.inv_m + f.c * (f.m * sux - f.su * sx);
        let slope = if d.slope_sides {
            (f.m * sux - f.su * sx) * f.inv_den
        } else {
            0.0
        };
        let left_raw = (f.inv_l > 0.0).then(|| (px[f.lb as usize] - px[f.la as usize]) * f.inv_l);
        let right_raw = (f.inv_r > 0.0).then(|| (px[f.rb as usize] - px[f.ra as usize]) * f.inv_r);
        // The sides moved along the box's slope to the bin, averaged; and the higher raw
        // side less the edge allowance (the knee of a roll-off).
        let moved = |s: Option<f64>, du: f64| s.map(|v| v + slope * du);
        let (left, right) = (moved(left_raw, f.du_l), moved(right_raw, f.du_r));
        let avg = match (left, right) {
            (Some(l), Some(r)) => 0.5 * (l + r),
            (Some(s), None) | (None, Some(s)) => s,
            (None, None) => line,
        };
        let high = left_raw
            .unwrap_or(f64::MIN)
            .max(right_raw.unwrap_or(f64::MIN));
        // How far under the higher raw side the envelope may sit: wide where the two sides
        // differ as a slope makes them (up to 12 dB), narrow where they differ as a cliff
        // does (24 dB and more: the edge of a band, the knee of a steep roll-off).
        let bend = match (left_raw, right_raw) {
            (Some(l), Some(r)) => (l - r).abs(),
            _ => f64::MAX,
        };
        let knee = ((bend - KNEE_FROM_DB) / KNEE_SPAN_DB).clamp(0.0, 1.0);
        let allow = d.edge_slope_db as f64 - (d.edge_slope_db - d.edge_db) as f64 * knee;
        out[k] = line.max(avg).max(high - allow) as f32;
    }
}

/// A bin's envelope estimate, precomputed from the design: the box `[a, b)` and its line's
/// constants (the value at the bin is `Σx/m + c·(m·Σux − Σu·Σx)`), and the two side means'
/// ranges (`inv_* = 0`: that side has fewer than three bins).
#[derive(Copy, Clone, Debug, Default)]
pub struct Fit {
    pub a: u32,
    pub b: u32,
    pub m: f64,
    pub inv_m: f64,
    pub su: f64,
    pub c: f64,
    pub la: u32,
    pub lb: u32,
    pub inv_l: f64,
    pub ra: u32,
    pub rb: u32,
    pub inv_r: f64,
    /// `1/den` of the box's line (0: none), and the bin's octave less each side's mean octave.
    pub inv_den: f64,
    pub du_l: f64,
    pub du_r: f64,
}

/// The upper envelope through the spectrum's prominent peaks, into `out` (dB): every local
/// maximum is a candidate; one standing more than [`HULL_DROP_DB`] under the straight line
/// between its neighbouring candidates is a valley's ripple, not a partial, and is dropped
/// (three passes); between two kept peaks no further apart than the lower one's frequency
/// (plus [`HULL_MIN_GAP`] bins) the envelope is that straight line, or the spectrum where it
/// stands higher; across wider gaps it is the spectrum itself. So the partials of a note,
/// however far apart, are joined into the outline over them — a bridge that adapts to the
/// spacing instead of a fixed width — and a resonance is a peak over its neighbouring peaks.
/// The candidates (every local maximum) are left in `cands` for the harmonic test; returns
/// how many.
fn hull(level: &[f32], peaks: &mut [u32], cands: &mut [u32], out: &mut [f32]) -> usize {
    let n = level.len();

    // Candidates: the local maxima, with both ends as anchors.
    let mut np = 0usize;
    for k in 0..n {
        let l = if k > 0 { level[k - 1] } else { f32::MIN };
        let r = if k + 1 < n { level[k + 1] } else { f32::MIN };
        if k == 0 || k + 1 == n || (level[k] >= l && level[k] > r) {
            peaks[np] = k as u32;
            np += 1;
        }
    }
    cands[..np].copy_from_slice(&peaks[..np]);
    let nc = np;
    let line = |a: usize, b: usize, at: usize| -> f32 {
        let t = (at - a) as f32 / (b - a) as f32;
        level[a] + t * (level[b] - level[a])
    };
    for _ in 0..HULL_PASSES {
        let mut w = 1usize;
        for i in 1..np.saturating_sub(1) {
            let (a, k, b) = (
                peaks[w - 1] as usize,
                peaks[i] as usize,
                peaks[i + 1] as usize,
            );
            if level[k] >= line(a, b, k) - HULL_DROP_DB {
                peaks[w] = k as u32;
                w += 1;
            }
        }
        if np >= 2 {
            peaks[w] = peaks[np - 1];
            w += 1;
        }
        np = w.min(np);
    }
    out[..n].copy_from_slice(level);
    for i in 1..np {
        let (a, b) = (peaks[i - 1] as usize, peaks[i] as usize);
        if b - a <= a.max(1) + HULL_MIN_GAP {
            for k in a + 1..b {
                out[k] = level[k].max(line(a, b, k));
            }
        }
    }
    nc
}

/// The kept peak nearest bin `t`, if one lies within `tol` bins.
fn peak_near(peaks: &[u32], t: usize, tol: usize) -> Option<usize> {
    let i = peaks.partition_point(|p| (*p as usize) < t);
    let near = [i.checked_sub(1), (i < peaks.len()).then_some(i)]
        .into_iter()
        .flatten()
        .map(|j| peaks[j] as usize)
        .min_by_key(|p| p.abs_diff(t))?;
    (near.abs_diff(t) <= tol).then_some(near)
}

/// Harmonic awareness under [`SERIES_HZ`]: a peak whose octave and twelfth are peaks too (or,
/// for an odd series, its twelfth and seventeenth; within 2.5 %, and no more than
/// [`SERIES_RANGE_DB`] under it) is a partial of a note,
/// and may stand over those partials by what a bright tone's partials do — `20·log10 m`
/// over the `m`-th (6 dB over its octave, 9.5 over its twelfth) — before it counts. Its
/// excess (over its main lobe, and along the line to the next such partial) is capped at
/// how far it exceeds that. A note's partials are then never resonances; a partial that a
/// resonance has lifted over the series still is, by as much as it was lifted; and an
/// inharmonic ring (a drum's shell, a room's mode) has no such partners and is judged by
/// the envelope alone.
fn series_caps(level: &[f32], peaks: &[u32], series_bins: usize, out: &mut [f32]) {
    let n = level.len();
    let mut last: Option<(usize, f32)> = None;
    for &p in peaks {
        let k = p as usize;
        if k < 2 || k > series_bins {
            last = None;
            continue;
        }
        let mut reference = f32::MIN;
        let mut found = [false; 3];
        for (i, (m, add)) in [(2usize, 6.02f32), (3, 9.54), (5, 13.98)]
            .into_iter()
            .enumerate()
        {
            let t = m * k;
            if t >= n {
                continue;
            }
            if let Some(q) = peak_near(peaks, t, 1 + t / 40) {
                if level[q] >= level[k] - SERIES_RANGE_DB {
                    reference = reference.max(level[q] + add);
                    found[i] = true;
                }
            }
        }
        // A partial: its octave and twelfth are partials too, or (an odd series: a clarinet,
        // a square) its twelfth and its seventeenth.
        if !((found[0] && found[1]) || (found[1] && found[2])) {
            last = None;
            continue;
        }
        let cap = level[k] - reference;
        for o in out[k.saturating_sub(2)..(k + 3).min(n)].iter_mut() {
            *o = o.min(cap);
        }
        if let Some((a, ca)) = last {
            for (j, o) in out.iter_mut().enumerate().take(k).skip(a + 1) {
                let t = (j - a) as f32 / (k - a) as f32;
                *o = o.min(ca + t * (cap - ca));
            }
        }
        last = Some((k, cap));
    }
}

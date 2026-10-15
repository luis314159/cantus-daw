//! **The pitch base** (round 28, ADR 0053): a monophonic pitch tracker for a voice or a
//! solo instrument, in two forms that share one core.
//!
//! - [`PitchTracker`] — **streaming**: a sample in, a [`PitchFrame`] out every
//!   [`Design::hop`] samples. Fixed hop, a fixed cost per hop, no allocation after `new`,
//!   exact rest (ADR 0031 §5.1). What Tonus, the real-time auto-tuner, hears with.
//! - [`analyse`] — **offline**: a whole take in, an [`Analysis`] out — the f0 per hop
//!   smoothed by a probabilistic (pYIN-style) Viterbi pass over every frame's candidates,
//!   voicing, level, the sibilance share, and the **epochs** (pitch marks one period apart,
//!   phase-locked to the waveform) the offline shifter ([`crate::psola::render`]) uses.
//!   What the pitch editor (§3c) analyses a recording with.
//!
//! Both are deterministic (the same bits on every machine: [`crate::math`] only, no
//! reordered sums) and our own implementation of published methods: YIN (de Cheveigné and
//! Kawahara, 2002) and pYIN (Mauch and Dixon, 2014 — the paper, not its GPL code).
//!
//! ## The method
//!
//! 1. The mono sum is low-passed (a windowed-sinc FIR, cut at about 3.6 kHz) and decimated
//!    to about 12 kHz ([`Design::decimation`]): a voice's fundamental and its first
//!    harmonics are all YIN needs, and the search costs a sixteenth.
//! 2. YIN's **cumulative mean normalised difference** over a window of 20 ms against lags
//!    from [`F_MAX`] down to [`F_MIN`]: the first dip under a threshold, walked down to its
//!    local minimum, refined by a parabola. The streaming form compares the newest window
//!    with the one a lag before (causal); the offline form centres every lag on the frame.
//! 3. **Fine refinement at the full rate**: the plain difference function on the raw mono
//!    signal at the lags around the coarse estimate, and a parabola through its minimum —
//!    the period to a hundredth of a sample, a fraction of a cent.
//! 4. **Voicing** from three cues, with hysteresis so it does not wobble: the
//!    **aperiodicity** (the CMNDF's minimum: a breath is noise and has no deep dip), the
//!    **level** (a gate at −60 dBFS RMS), and the **sibilance share** — the de-esser's cue
//!    (`sibilus_detect`): a sibilant's energy sits above the voice's formants, so the share
//!    of the window's energy above the decimator's band says "s", "sh", "f" even when a
//!    periodic voice sings under it.
//!
//! ## The lowest f0, and why
//!
//! [`F_MIN`] = 55 Hz (A1): below a bass singer's lowest sung note (C2, 65 Hz) with a
//! margin, and the period (18.2 ms) still fits once in the 20 ms window. Lower would
//! need a longer window — more lag on every voice for notes nobody sings. The window looks
//! back 38 ms in all; the tracker's look-back is not the shifter's latency (that is fixed,
//! [`crate::psola::Shifter::latency`]).

use crate::math;

#[path = "pitch_offline.rs"]
mod offline;
#[path = "pitch_stream.rs"]
mod stream;
#[path = "pitch_testkit.rs"]
pub mod testkit;
#[cfg(test)]
#[path = "pitch_tests.rs"]
mod tests;

pub use offline::{analyse, Analysis, Epoch};
pub use stream::PitchTracker;

/// The lowest fundamental tracked, Hz (A1).
pub const F_MIN: f32 = 55.0;
/// The highest fundamental tracked, Hz (above a soprano's high C, 1 047 Hz, and a
/// whistle's first notes).
pub const F_MAX: f32 = 1_600.0;
/// The analysis window, seconds.
pub const WINDOW_S: f32 = 0.020;
/// The rate the coarse search runs at, roughly, Hz.
pub const COARSE_RATE: f32 = 12_000.0;
/// The streaming hop at the coarse rate, samples (128 at 48 kHz: 2.7 ms).
pub const HOP_COARSE: usize = 32;
/// YIN's absolute threshold for the streaming tracker.
pub const THRESHOLD: f32 = 0.15;
/// The level gate: a window quieter than this (RMS, dBFS) is unvoiced.
pub const GATE_DB: f32 = -60.0;

/// One analysis frame.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct PitchFrame {
    /// The moment the frame measures, in samples from the start of the stream (streaming:
    /// the middle of the compared windows; offline: the frame's time).
    pub centre: f64,
    /// The best candidate's fundamental, Hz, voiced or not (0 when there is no candidate:
    /// silence).
    pub f0: f32,
    /// The best candidate's period at the full rate, samples (0 with `f0`).
    pub period: f32,
    /// The voicing decision.
    pub voiced: bool,
    /// The CMNDF at the chosen lag, 0 (perfectly periodic) … 1+ (noise).
    pub aperiodicity: f32,
    /// The window's RMS, dBFS (−120 for silence).
    pub level_db: f32,
    /// The share of the window's energy above the decimator's band, 0 … 1: high in
    /// sibilants and breath noise, low in a sung vowel.
    pub sibilance: f32,
    /// Offline only: the probability that the frame is voiced (the sum of its candidates'
    /// probabilities); streaming: 1 or 0, as `voiced`.
    pub voiced_prob: f32,
}

impl PitchFrame {
    /// The fundamental as a MIDI pitch (69 = A4 = 440 Hz), or `None` without one.
    pub fn midi(&self) -> Option<f32> {
        (self.f0 > 0.0).then(|| hz_to_midi(self.f0))
    }
}

/// Hz to a MIDI pitch (fractional): 440 Hz is 69.
#[inline]
pub fn hz_to_midi(hz: f32) -> f32 {
    69.0 + 12.0 * math::log2(hz / 440.0)
}

/// A MIDI pitch (fractional) to Hz.
#[inline]
pub fn midi_to_hz(m: f32) -> f32 {
    440.0 * math::exp2((m - 69.0) / 12.0)
}

/// The tracker's dimensions at one sample rate: everything both forms derive from it.
#[derive(Clone, Debug, PartialEq)]
pub struct Design {
    pub sample_rate: f32,
    /// The decimation factor `D`: the coarse rate is `sample_rate / D`.
    pub decimation: usize,
    /// The decimator's low-pass, `16·D` taps (one tap, unity, when `D` is 1).
    pub fir: Vec<f32>,
    /// The window at the coarse rate, samples.
    pub window: usize,
    /// The coarse lags searched, inclusive.
    pub tau_min: usize,
    pub tau_max: usize,
    /// The streaming hop at the full rate, samples.
    pub hop: usize,
}

impl Design {
    pub fn new(sample_rate: f32) -> Self {
        let sr = sample_rate.max(8_000.0);
        let d = ((sr / COARSE_RATE).round() as usize).max(1);
        let fs = sr / d as f32;
        let fir = if d == 1 {
            vec![1.0]
        } else {
            lowpass(16 * d, 0.3 / d as f32)
        };
        Design {
            sample_rate: sr,
            decimation: d,
            fir,
            window: (WINDOW_S * fs).round() as usize,
            tau_min: ((fs / F_MAX).floor() as usize).max(2),
            tau_max: (fs / F_MIN).ceil() as usize + 1,
            hop: HOP_COARSE * d,
        }
    }

    /// The coarse rate, Hz.
    pub fn coarse_rate(&self) -> f32 {
        self.sample_rate / self.decimation as f32
    }

    /// The full-rate window of the fine stage and the level, samples.
    pub fn fine_window(&self) -> usize {
        self.window * self.decimation
    }

    /// The decimator's delay, full-rate samples (it is linear phase).
    pub fn fir_delay(&self) -> f32 {
        (self.fir.len() - 1) as f32 * 0.5
    }
}

/// A windowed-sinc low-pass of `n` taps (Blackman), cut at `cutoff` cycles per sample,
/// unity at DC.
fn lowpass(n: usize, cutoff: f32) -> Vec<f32> {
    let mid = (n - 1) as f64 * 0.5;
    let pi = core::f64::consts::PI;
    let mut h: Vec<f64> = (0..n)
        .map(|k| {
            let t = k as f64 - mid;
            let sinc = if t == 0.0 {
                2.0 * cutoff as f64
            } else {
                math::sin(2.0 * pi * cutoff as f64 * t) / (pi * t)
            };
            let a = 2.0 * pi * k as f64 / (n - 1) as f64;
            let w = 0.42 - 0.5 * math::cos(a) + 0.08 * math::cos(2.0 * a);
            sinc * w
        })
        .collect();
    let sum: f64 = h.iter().sum();
    for v in &mut h {
        *v /= sum;
    }
    h.into_iter().map(|v| v as f32).collect()
}

/// The sum of squared differences of two equal slices.
#[inline]
fn diff(a: &[f32], b: &[f32]) -> f32 {
    let mut acc = 0.0f32;
    for (x, y) in a.iter().zip(b) {
        let d = x - y;
        acc += d * d;
    }
    acc
}

/// YIN's working buffers: no allocation after `new`.
#[derive(Clone, Debug)]
pub(crate) struct Yin {
    /// The cumulative mean normalised difference, lags 0 ..= tau_max.
    pub(crate) cmnd: Vec<f32>,
}

impl Yin {
    pub(crate) fn new(tau_max: usize) -> Self {
        Yin {
            cmnd: vec![1.0; tau_max + 2],
        }
    }

    /// The CMNDF from a difference function given lag by lag.
    fn fill(&mut self, tau_max: usize, mut d: impl FnMut(usize) -> f32) {
        self.cmnd[0] = 1.0;
        let mut sum = 0.0f32;
        for tau in 1..=tau_max {
            let acc = d(tau);
            sum += acc;
            self.cmnd[tau] = if sum > 0.0 {
                acc * tau as f32 / sum
            } else {
                1.0
            };
        }
    }

    /// The CMNDF of `x` (oldest first, at least `w + tau_max` long), causal: the newest
    /// `w` samples against the ones `τ` earlier.
    pub(crate) fn run(&mut self, x: &[f32], w: usize, tau_max: usize) {
        debug_assert!(x.len() >= w + tau_max);
        let base = x.len() - w;
        self.fill(tau_max, |tau| {
            diff(&x[base..], &x[base - tau..base - tau + w])
        });
    }

    /// The CMNDF of `x` around `centre`, for the offline form: lag `τ` compares the `w`
    /// samples from `centre − (w + τ)/2` with the ones `τ` later, so every lag measures the
    /// same moment (a causal window measures each lag half a lag earlier).
    pub(crate) fn run_centred(&mut self, x: &[f32], centre: usize, w: usize, tau_max: usize) {
        debug_assert!(centre > (w + tau_max) / 2 && centre + (w + tau_max) / 2 + 1 < x.len());
        self.fill(tau_max, |tau| {
            let o = centre - (w + tau) / 2;
            diff(&x[o..o + w], &x[o + tau..o + tau + w])
        });
    }

    /// The first lag in `lo ..= hi` whose CMNDF dips under `threshold`, walked down to the
    /// bottom of its dip; `None` when nothing dips that low.
    pub(crate) fn first_dip(&self, lo: usize, hi: usize, threshold: f32) -> Option<usize> {
        let c = &self.cmnd;
        let mut tau = lo;
        while tau <= hi {
            if c[tau] < threshold {
                while tau < hi && c[tau + 1] < c[tau] {
                    tau += 1;
                }
                return Some(tau);
            }
            tau += 1;
        }
        None
    }

    /// The lag of the lowest CMNDF in `lo ..= hi`.
    pub(crate) fn global_min(&self, lo: usize, hi: usize) -> usize {
        let mut best = lo;
        for tau in lo..=hi {
            if self.cmnd[tau] < self.cmnd[best] {
                best = tau;
            }
        }
        best
    }

    /// The bottom of the lowest dip within ±`span` lags of `around`.
    pub(crate) fn local_min_near(&self, around: usize, span: usize, lo: usize, hi: usize) -> usize {
        let a = around.saturating_sub(span).max(lo);
        let b = (around + span).min(hi);
        self.global_min(a, b.max(a))
    }

    /// `tau` refined by a parabola through the CMNDF at its neighbours.
    pub(crate) fn refine(&self, tau: usize, hi: usize) -> f32 {
        parabola(&self.cmnd, tau, hi)
    }
}

/// The vertex of the parabola through `y[i−1], y[i], y[i+1]`, as a fractional index
/// (`i` itself at an edge or a flat spot).
pub(crate) fn parabola(y: &[f32], i: usize, hi: usize) -> f32 {
    if i == 0 || i >= hi || i + 1 >= y.len() {
        return i as f32;
    }
    let (a, b, c) = (y[i - 1], y[i], y[i + 1]);
    let den = a - 2.0 * b + c;
    if den <= 0.0 {
        return i as f32;
    }
    i as f32 + (0.5 * (a - c) / den).clamp(-0.5, 0.5)
}

/// The minimum of `scratch[..n]` (lags `lo …`) refined by a parabola; `coarse` when it
/// lands on an edge (the two stages disagree).
fn fine_vertex(scratch: &[f32], n: usize, lo: usize, coarse: f32) -> f32 {
    let mut best = 0;
    for k in 1..n {
        if scratch[k] < scratch[best] {
            best = k;
        }
    }
    if best == 0 || best == n - 1 {
        return coarse;
    }
    lo as f32 + parabola(&scratch[..n], best, n - 1)
}

/// The fine stage, causal: the plain difference function of the full-rate `x` (oldest
/// first, newest `w` samples) at the lags within `span` of `coarse` (full-rate samples),
/// and the vertex of a parabola through its minimum. `scratch` holds the lags.
pub(crate) fn fine_period(
    x: &[f32],
    w: usize,
    coarse: f32,
    span: usize,
    scratch: &mut [f32],
) -> f32 {
    let c = coarse.round() as usize;
    let lo = c.saturating_sub(span).max(1);
    let hi = c + span;
    if hi + w > x.len() || hi - lo + 1 > scratch.len() {
        return coarse;
    }
    let base = x.len() - w;
    for (k, tau) in (lo..=hi).enumerate() {
        scratch[k] = diff(&x[base..], &x[base - tau..base - tau + w]);
    }
    fine_vertex(scratch, hi - lo + 1, lo, coarse)
}

/// The fine stage of the offline form: as [`fine_period`], each lag centred on `centre`.
pub(crate) fn fine_period_centred(
    x: &[f32],
    centre: usize,
    w: usize,
    coarse: f32,
    span: usize,
    scratch: &mut [f32],
) -> f32 {
    let c = coarse.round() as usize;
    let lo = c.saturating_sub(span).max(1);
    let hi = c + span;
    if centre <= (w + hi) / 2 || centre + (w + hi) / 2 + 1 >= x.len() || hi - lo + 1 > scratch.len()
    {
        return coarse;
    }
    for (k, tau) in (lo..=hi).enumerate() {
        let o = centre - (w + tau) / 2;
        scratch[k] = diff(&x[o..o + w], &x[o + tau..o + tau + w]);
    }
    fine_vertex(scratch, hi - lo + 1, lo, coarse)
}

/// The voicing cues of a window: the RMS level (dBFS) and the sibilance share, from the
/// energy of the raw window and of its decimated low band (scaled back to the full rate).
pub(crate) fn level_and_sibilance(
    raw_energy: f32,
    raw_len: usize,
    low_energy: f32,
    d: usize,
) -> (f32, f32) {
    if raw_energy <= 1e-20 || raw_len == 0 {
        return (-120.0, 0.0);
    }
    let ms = raw_energy / raw_len as f32;
    let level = (10.0 * math::log10(ms)).max(-120.0);
    let low = low_energy * d as f32;
    let sib = (1.0 - low / raw_energy).clamp(0.0, 1.0);
    (level, sib)
}

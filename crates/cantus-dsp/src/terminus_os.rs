//! Oversampling for Terminus: one Kaiser-windowed-sinc prototype, used three ways — a
//! polyphase **interpolator** (4×), a **true-peak detector** built on it, and a
//! **decimator** for the oversampled clipper.
//!
//! ## Why the prototype is shaped the way it is
//!
//! The cut-off sits exactly at the base rate's Nyquist frequency and the taps at every
//! multiple of the factor from the centre are forced to exactly zero (the centre to exactly
//! one). So phase 0 of the interpolator is a **pure delay**: its output is an input sample,
//! to the bit. The true-peak detector therefore always includes the sample peak, and a
//! clipper that clips nothing adds exactly nothing (see `terminus::Clipper`).
//!
//! 32 taps per phase and a Kaiser β of 6 put the pass band (±0.02 dB) up to about 0.44 of
//! the sample rate — 21 kHz at 48 kHz — and the images more than 60 dB down; the test below
//! measures both. Parabolic refinement between the oversampled points then reads the peak
//! of the reconstructed waveform, not just of the 4× points, so a 4× detector here reads a
//! near-Nyquist peak that a plain 4× meter under-reads by up to 0.7 dB.

/// The oversampling factor.
pub const FACTOR: usize = 4;
/// Half the prototype's length, in base-rate samples.
pub const HALF: usize = 16;
/// Taps of the prototype.
pub const LEN: usize = 2 * HALF * FACTOR + 1;
/// The history an interpolator keeps, in base-rate samples.
pub const TAPS: usize = 2 * HALF + 1;
/// Taps of phases 1–3 (their last tap in the history is zero); phase 0 is a pure delay and
/// needs none.
const PHASE_TAPS: usize = 2 * HALF;
/// The decimator's length, padded with zero taps to a multiple of 8 so it vectorises.
const DECIM_LEN: usize = LEN.div_ceil(8) * 8;
/// Delay of the interpolator, and of the decimator, in base-rate samples.
pub const DELAY: usize = HALF;
/// Delay of [`TruePeak`]: the interpolator's, plus one sample so a peak between two samples
/// is known before either of them is released.
pub const TP_DELAY: usize = DELAY + 1;
/// Kaiser window shape.
const BETA: f64 = 6.0;

/// Modified Bessel function of the first kind, order 0 (for the Kaiser window).
fn bessel_i0(x: f64) -> f64 {
    let mut sum = 1.0;
    let mut term = 1.0;
    let q = x * x / 4.0;
    for k in 1..64 {
        term *= q / (k * k) as f64;
        sum += term;
        if term < sum * 1e-17 {
            break;
        }
    }
    sum
}

/// The prototype low-pass at the oversampled rate, with a gain of [`FACTOR`] (so zero-stuffed
/// input comes out at its own level). The centre tap is exactly 1 and every tap a multiple
/// of [`FACTOR`] away from it is exactly 0.
pub fn prototype() -> [f64; LEN] {
    let mut h = [0.0; LEN];
    let c = (LEN - 1) / 2;
    let norm = bessel_i0(BETA);
    for (k, tap) in h.iter_mut().enumerate() {
        let m = k as f64 - c as f64;
        let off = k as i64 - c as i64;
        if off == 0 {
            *tap = 1.0;
            continue;
        }
        if off % FACTOR as i64 == 0 {
            *tap = 0.0;
            continue;
        }
        let t = m / FACTOR as f64;
        let sinc = (std::f64::consts::PI * t).sin() / (std::f64::consts::PI * t);
        let r = m / c as f64;
        let w = bessel_i0(BETA * (1.0 - r * r).max(0.0).sqrt()) / norm;
        *tap = sinc * w;
    }
    h
}

/// A low-pass at the oversampled rate cutting at `cutoff` (a fraction of the *base* rate):
/// a Kaiser-windowed sinc of the prototype's length and centre (so the same delay), with
/// the prototype's gain of [`FACTOR`] at DC exactly. For a decimator that must leave nothing
/// in the band the true-peak detector reads least well — the last few per cent below the
/// base rate's Nyquist frequency, where the prototype is still in its transition.
pub fn lowpass(cutoff: f64, beta: f64) -> [f64; LEN] {
    let mut h = [0.0; LEN];
    let c = (LEN - 1) / 2;
    let norm = bessel_i0(beta);
    let fc = cutoff / FACTOR as f64;
    for (k, tap) in h.iter_mut().enumerate() {
        let m = k as f64 - c as f64;
        let x = 2.0 * fc * m;
        let sinc = if m == 0.0 {
            1.0
        } else {
            (std::f64::consts::PI * x).sin() / (std::f64::consts::PI * x)
        };
        let r = m / c as f64;
        let w = bessel_i0(beta * (1.0 - r * r).max(0.0).sqrt()) / norm;
        *tap = 2.0 * fc * sinc * w;
    }
    let sum: f64 = h.iter().sum();
    for tap in &mut h {
        *tap *= FACTOR as f64 / sum;
    }
    h
}

/// A 4× polyphase interpolator. Push one base-rate sample, get the four points of the
/// reconstructed waveform at `n − DELAY + p/4`, `p = 0..4`; point 0 is `x[n − DELAY]`.
#[derive(Clone, Debug)]
pub struct Interp {
    /// Phases 1–3, the taps in the order of the history window (from its second-oldest).
    coef: [[f32; PHASE_TAPS]; FACTOR - 1],
    /// The last `TAPS` inputs, stored twice so the window is always one slice.
    ring: [f32; 2 * TAPS],
    pos: usize,
}

impl Default for Interp {
    fn default() -> Self {
        Self::new()
    }
}

impl Interp {
    pub fn new() -> Self {
        let h = prototype();
        let mut coef = [[0f32; PHASE_TAPS]; FACTOR - 1];
        for (row, p) in coef.iter_mut().zip(1..FACTOR) {
            for (m, c) in row.iter_mut().enumerate() {
                // Output p = Σ_j h[p + F·j] · x[n − j]. Window index i = m + 1 holds
                // x[n − (TAPS − 1 − i)]; j = TAPS − 1 reaches past the prototype (zero).
                let j = TAPS - 2 - m;
                let k = p + FACTOR * j;
                *c = if k < LEN { h[k] as f32 } else { 0.0 };
            }
        }
        Interp {
            coef,
            ring: [0.0; 2 * TAPS],
            pos: 0,
        }
    }

    pub fn reset(&mut self) {
        self.ring = [0.0; 2 * TAPS];
        self.pos = 0;
    }

    /// Store a sample without computing anything (the history stays current while the
    /// stage that uses it is switched off, so switching it on needs no warm-up).
    #[inline]
    pub fn feed(&mut self, x: f32) {
        self.ring[self.pos] = x;
        self.ring[self.pos + TAPS] = x;
        self.pos = if self.pos + 1 == TAPS {
            0
        } else {
            self.pos + 1
        };
    }

    /// The four points for the last sample fed.
    #[inline]
    pub fn points(&self) -> [f32; FACTOR] {
        let w = &self.ring[self.pos..self.pos + TAPS];
        let tail: &[f32; PHASE_TAPS] = w[1..].try_into().expect("the window is TAPS long");
        let mut out = [0f32; FACTOR];
        // Phase 0 is the sample itself, DELAY samples back: exact, and free.
        out[0] = w[TAPS - 1 - DELAY];
        for (o, c) in out[1..].iter_mut().zip(self.coef.iter()) {
            *o = dot(c, tail);
        }
        out
    }

    #[inline]
    pub fn push(&mut self, x: f32) -> [f32; FACTOR] {
        self.feed(x);
        self.points()
    }
}

/// A dot product of fixed length, a multiple of 8: eight partial sums so it vectorises,
/// added in a fixed order, so the result is the same on every run.
#[inline]
fn dot<const N: usize>(a: &[f32; N], b: &[f32; N]) -> f32 {
    let mut s = [0f32; 8];
    for (ca, cb) in a.chunks_exact(8).zip(b.chunks_exact(8)) {
        for l in 0..8 {
            s[l] += ca[l] * cb[l];
        }
    }
    ((s[0] + s[4]) + (s[1] + s[5])) + ((s[2] + s[6]) + (s[3] + s[7]))
}

/// The peak of a parabola through three equally spaced points, when the middle one is a
/// local maximum of the magnitude; otherwise the middle one's magnitude.
#[inline]
pub fn vertex(a: f32, b: f32, c: f32) -> f32 {
    let s = if b < 0.0 { -1.0 } else { 1.0 };
    let (a, b, c) = (a * s, b * s, c * s);
    if b <= 0.0 || a > b || c > b {
        return b.abs();
    }
    let den = a - 2.0 * b + c;
    if den >= 0.0 {
        return b;
    }
    let d = 0.5 * (a - c) / den;
    // The vertex of a concave parabola through a local maximum is within half a step of
    // it and not far above it; a bound keeps nonsense out of a detector.
    (b - 0.25 * (a - c) * d).min(b * 1.25)
}

/// A true-peak detector for one channel: the peak of the band-limited waveform around each
/// sample, from 4× points refined by parabolas. Push `x[n]`, get the level that applies to
/// sample `n − TP_DELAY`: the larger of the two intervals that touch it.
#[derive(Clone, Debug, Default)]
pub struct TruePeak {
    interp: Interp,
    /// The previous sample's four points.
    prev: [f32; FACTOR],
    /// The point just before `prev[0]`.
    before: f32,
    /// The peak of the interval before the one just finished.
    last_interval: f32,
}

impl TruePeak {
    pub fn new() -> Self {
        TruePeak {
            interp: Interp::new(),
            prev: [0.0; FACTOR],
            before: 0.0,
            last_interval: 0.0,
        }
    }

    pub fn reset(&mut self) {
        self.interp.reset();
        self.prev = [0.0; FACTOR];
        self.before = 0.0;
        self.last_interval = 0.0;
    }

    #[inline]
    pub fn push(&mut self, x: f32) -> f32 {
        let cur = self.interp.push(x);
        let seq = [
            self.before,
            self.prev[0],
            self.prev[1],
            self.prev[2],
            self.prev[3],
            cur[0],
        ];
        let mut interval = 0f32;
        for i in 1..=FACTOR {
            interval = interval.max(vertex(seq[i - 1], seq[i], seq[i + 1]));
        }
        let level = interval.max(self.last_interval);
        self.last_interval = interval;
        self.before = self.prev[FACTOR - 1];
        self.prev = cur;
        level
    }
}

/// A 4× → 1× decimator with the prototype's response (unity gain). Push the four points of
/// each base sample, oldest first; after the first of them, [`output`](Self::output) is the
/// filtered signal at the base-rate sample `DELAY` samples before those points.
#[derive(Clone, Debug)]
pub struct Decim {
    coef: [f32; DECIM_LEN],
    ring: [f32; 2 * DECIM_LEN],
    pos: usize,
}

impl Default for Decim {
    fn default() -> Self {
        Self::new()
    }
}

impl Decim {
    pub fn new() -> Self {
        Self::with_filter(&prototype())
    }

    /// A decimator through `h` (a filter at the oversampled rate of the prototype's length
    /// and centre, with a gain of [`FACTOR`] — [`prototype`] or [`lowpass`]).
    pub fn with_filter(h: &[f64; LEN]) -> Self {
        let mut coef = [0f32; DECIM_LEN];
        // Window index i holds d[m − (DECIM_LEN−1−i)]; y = Σ_k g[k] d[m − k], g = h / FACTOR,
        // zero past the prototype's length (the padding).
        for (i, c) in coef.iter_mut().enumerate() {
            let k = DECIM_LEN - 1 - i;
            *c = if k < LEN {
                (h[k] / FACTOR as f64) as f32
            } else {
                0.0
            };
        }
        Decim {
            coef,
            ring: [0.0; 2 * DECIM_LEN],
            pos: 0,
        }
    }

    pub fn reset(&mut self) {
        self.ring = [0.0; 2 * DECIM_LEN];
        self.pos = 0;
    }

    #[inline]
    pub fn push(&mut self, x: f32) {
        self.ring[self.pos] = x;
        self.ring[self.pos + DECIM_LEN] = x;
        self.pos = if self.pos + 1 == DECIM_LEN {
            0
        } else {
            self.pos + 1
        };
    }

    /// The filter's output at the newest point pushed.
    #[inline]
    pub fn output(&self) -> f32 {
        let w: &[f32; DECIM_LEN] = self.ring[self.pos..self.pos + DECIM_LEN]
            .try_into()
            .expect("the window is DECIM_LEN long");
        dot(&self.coef, w)
    }
}

/// The true peak of a whole buffer by the same detector, linear — what the meters and the
/// tests read.
pub fn true_peak_of(x: &[f32]) -> f32 {
    let mut tp = TruePeak::new();
    let mut m = 0f32;
    for &v in x {
        m = m.max(tp.push(v));
    }
    for _ in 0..TP_DELAY + 1 {
        m = m.max(tp.push(0.0));
    }
    m
}

/// The true peak of a buffer by brute force, for tests and offline checks: the waveform
/// rebuilt at 32 points per sample with a Kaiser-windowed sinc 129 samples long (β 12), in
/// f64 — in every interval where [`TruePeak`] reads within `near_db` of its loudest
/// reading (the detector only chooses where to look; the number is the reference's).
pub fn reference_true_peak(x: &[f32], near_db: f32) -> f32 {
    const R: usize = 32;
    const SIDE: i64 = 64;
    const W: usize = 2 * SIDE as usize + 2;
    let beta = 12.0;
    let norm = bessel_i0(beta);
    // Where to look: the detector's reading per sample.
    let mut tp = TruePeak::new();
    let mut level = vec![0f32; x.len()];
    for i in 0..x.len() + TP_DELAY {
        let l = tp.push(x.get(i).copied().unwrap_or(0.0));
        if i >= TP_DELAY {
            level[i - TP_DELAY] = l;
        }
    }
    let loudest = level.iter().fold(0f32, |m, v| m.max(*v));
    let floor = loudest * 10f32.powf(-near_db / 20.0);
    // The kernel at every offset t = j − k/R, tabulated once.
    let mut table = vec![0f64; R * W];
    for k in 0..R {
        for (m, j) in (-SIDE..=SIDE + 1).enumerate() {
            let t = j as f64 - k as f64 / R as f64;
            let sinc = if t.abs() < 1e-12 {
                1.0
            } else {
                (std::f64::consts::PI * t).sin() / (std::f64::consts::PI * t)
            };
            let r = t / (SIDE as f64 + 1.0);
            let w = if r.abs() >= 1.0 {
                0.0
            } else {
                bessel_i0(beta * (1.0 - r * r).sqrt()) / norm
            };
            table[k * W + m] = sinc * w;
        }
    }
    let mut peak = x.iter().fold(0f32, |m, v| m.max(v.abs()));
    let n = x.len() as i64;
    for i in 0..n - 1 {
        if level[i as usize].max(level[i as usize + 1]) < floor {
            continue;
        }
        for k in 1..R {
            let mut v = 0f64;
            let row = &table[k * W..(k + 1) * W];
            for (m, j) in (-SIDE..=SIDE + 1).enumerate() {
                let idx = i + j;
                if idx >= 0 && idx < n {
                    v += x[idx as usize] as f64 * row[m];
                }
            }
            peak = peak.max(v.abs() as f32);
        }
    }
    peak
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::f64::consts::PI;

    #[test]
    fn phase_zero_is_a_pure_delay() {
        let mut ip = Interp::new();
        let x: Vec<f32> = (0..200)
            .map(|i| ((i * 7919) % 97) as f32 / 97.0 - 0.5)
            .collect();
        for (n, &v) in x.iter().enumerate() {
            let p = ip.push(v);
            let want = if n >= DELAY { x[n - DELAY] } else { 0.0 };
            assert_eq!(p[0], want, "phase 0 at {n} is not x[n − {DELAY}]");
        }
    }

    /// The pass band is flat to 0.44·fs and the images are more than 60 dB down, measured
    /// on the prototype's own frequency response.
    #[test]
    fn the_prototype_passes_and_stops_where_it_says() {
        let h = prototype();
        let resp = |f_base: f64| {
            // Frequency as a fraction of the *base* rate; the prototype runs at FACTOR×.
            let w = 2.0 * PI * f_base / FACTOR as f64;
            let (mut re, mut im) = (0.0, 0.0);
            for (k, t) in h.iter().enumerate() {
                re += t * (w * k as f64).cos();
                im -= t * (w * k as f64).sin();
            }
            ((re * re + im * im).sqrt() / FACTOR as f64).log10() * 20.0
        };
        let mut worst_pass = 0f64;
        for i in 0..=440 {
            worst_pass = worst_pass.max(resp(i as f64 / 1000.0).abs());
        }
        assert!(worst_pass < 0.02, "pass band ripple {worst_pass} dB");
        let mut worst_stop = -200f64;
        for i in 560..=2000 {
            worst_stop = worst_stop.max(resp(i as f64 / 1000.0));
        }
        assert!(worst_stop < -60.0, "images only {worst_stop} dB down");
    }

    /// A sine's true peak, whatever its frequency (to 0.44·fs) and phase, reads within
    /// +0.03 / −0.03 dB of its amplitude — where a plain 4× meter reads up to 0.7 dB low.
    #[test]
    fn a_sine_reads_its_amplitude_between_the_samples() {
        let fs = 48_000.0;
        let mut worst_lo = 0f64;
        let mut worst_hi = 0f64;
        for fi in 0..40 {
            let f = 200.0 + fi as f64 * (0.44 * fs - 200.0) / 39.0;
            for ph in 0..7 {
                let phase = ph as f64 * 0.37;
                let x: Vec<f32> = (0..4000)
                    .map(|n| (0.5 * (2.0 * PI * f * n as f64 / fs + phase).sin()) as f32)
                    .collect();
                // Skip the start: the filter needs its history.
                let mut tp = TruePeak::new();
                let mut m = 0f32;
                for (n, &v) in x.iter().enumerate() {
                    let l = tp.push(v);
                    if n > 200 {
                        m = m.max(l);
                    }
                }
                let err = 20.0 * (m as f64 / 0.5).log10();
                worst_lo = worst_lo.min(err);
                worst_hi = worst_hi.max(err);
            }
        }
        eprintln!("sine true-peak error {worst_lo:+.4} … {worst_hi:+.4} dB");
        assert!(
            worst_lo > -0.03 && worst_hi < 0.03,
            "true peak error {worst_lo:+.4} … {worst_hi:+.4} dB"
        );
    }

    /// A raised-cosine fade over `len` samples at both ends: a tone switched on abruptly
    /// really does overshoot between its first samples, which is not what these tests ask.
    pub(crate) fn faded(mut x: Vec<f32>, len: usize) -> Vec<f32> {
        let n = x.len();
        for i in 0..len.min(n / 2) {
            let g = (0.5 - 0.5 * (PI * i as f64 / len as f64).cos()) as f32;
            x[i] *= g;
            x[n - 1 - i] *= g;
        }
        x
    }

    /// The classic inter-sample peak: a sine at fs/4 whose samples fall at ±45°, so every
    /// sample reads 3 dB under the waveform's peak.
    #[test]
    fn the_quarter_rate_sine_at_45_degrees_reads_3_db_over_its_samples() {
        let x: Vec<f32> = (0..4000)
            .map(|n| (PI / 2.0 * n as f64 + PI / 4.0).sin() as f32)
            .collect();
        let x = faded(x, 800);
        let sample_peak = x.iter().fold(0f32, |m, v| m.max(v.abs()));
        assert!((20.0 * sample_peak.log10() + 3.0103).abs() < 0.001);
        let tp = true_peak_of(&x);
        assert!(
            (20.0 * tp.log10()).abs() < 0.01,
            "read {}",
            20.0 * tp.log10()
        );
    }

    #[test]
    fn the_decimator_undoes_the_interpolator_in_the_pass_band() {
        let mut ip = Interp::new();
        let mut de = Decim::new();
        let fs = 48_000.0;
        let x: Vec<f32> = (0..3000)
            .map(|n| (0.4 * (2.0 * PI * 3_000.0 * n as f64 / fs).sin()) as f32)
            .collect();
        let mut worst = 0f32;
        for (n, &v) in x.iter().enumerate() {
            let p = ip.push(v);
            de.push(p[0]);
            let y = de.output();
            de.push(p[1]);
            de.push(p[2]);
            de.push(p[3]);
            if n > 2 * DELAY + 200 {
                worst = worst.max((y - x[n - 2 * DELAY]).abs());
            }
        }
        // The images the two filters leave: under −60 dB of the tone.
        assert!(worst < 0.4e-3, "round trip error {worst}");
    }
}

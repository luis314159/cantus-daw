//! Sibilus's delay lines and its linear-phase split.
//!
//! ## Why a linear-phase split, and why it is still a wire
//!
//! Split-band de-essing turns down one part of the spectrum. Written as
//!
//! ```text
//! y = x_d + (g − 1) · h ∗ x
//! ```
//!
//! where `h` is a **linear-phase** FIR (the band to reduce) and `x_d` is the input delayed
//! by the FIR's centre, the two terms are exactly in phase at every frequency, so the
//! response is `1 + (g − 1) · A(f)` with `A` the FIR's real amplitude: a cut of exactly
//! `g` inside the band, none outside it, and no bump or phase smear in between. At `g = 1`
//! the second term is not computed at all, so the output is the delayed input to the bit.
//! An IIR crossover cannot give both: it either sums to an all-pass (not a wire) or, used
//! the same way, limits how deep it can cut near its corner.
//!
//! The FIR is a Kaiser-windowed sinc (β = 6, about −60 dB of stop band) of [`fir_len`]
//! taps: 255 at 44.1/48 kHz, a transition about 0.7 kHz wide, and 127 samples (2.6 ms)
//! of latency — reported, and compensated by the host.

use super::Processing;

/// Taps of the split FIR at a sample rate: about 5.3 ms, always odd.
pub fn fir_len(sr: f32) -> usize {
    if sr <= 50_000.0 {
        255
    } else if sr <= 100_000.0 {
        511
    } else {
        1023
    }
}

/// A delay line that can hand out any recent stretch as one contiguous slice: every
/// sample is written twice, `cap` apart.
#[derive(Clone, Debug)]
pub(super) struct Line {
    buf: Vec<f32>,
    cap: usize,
    w: usize,
    /// Zeros pushed in a row (ADR 0031 §5.1).
    quiet: usize,
}

impl Line {
    pub fn new(cap: usize) -> Self {
        Line {
            buf: vec![0.0; 2 * cap],
            cap,
            w: 0,
            quiet: usize::MAX,
        }
    }

    pub fn clear(&mut self) {
        self.buf.iter_mut().for_each(|x| *x = 0.0);
        self.w = 0;
        self.quiet = usize::MAX;
    }

    /// Holds only zeros (ADR 0031 §5.1).
    pub fn is_clear(&self) -> bool {
        self.quiet >= self.cap
    }

    #[inline]
    pub fn push(&mut self, x: f32) {
        self.quiet = if x.to_bits() == 0 {
            self.quiet.saturating_add(1)
        } else {
            0
        };
        self.w += 1;
        if self.w == self.cap {
            self.w = 0;
        }
        self.buf[self.w] = x;
        self.buf[self.w + self.cap] = x;
    }

    /// The sample pushed `d` pushes ago (0 = the newest). `d < cap`.
    #[inline]
    pub fn at(&self, d: usize) -> f32 {
        self.buf[self.w + self.cap - d]
    }

    /// `len` samples ending `d` pushes ago, oldest first. `d + len <= cap`.
    #[inline]
    pub fn window(&self, d: usize, len: usize) -> &[f32] {
        let end = self.w + self.cap - d;
        &self.buf[end + 1 - len..=end]
    }
}

/// The zeroth-order modified Bessel function, for the Kaiser window.
fn bessel_i0(x: f64) -> f64 {
    let mut sum = 1.0;
    let mut term = 1.0;
    let q = x * x / 4.0;
    for k in 1..64 {
        term *= q / (k * k) as f64;
        sum += term;
        if term < sum * 1e-16 {
            break;
        }
    }
    sum
}

const KAISER_BETA: f64 = 6.0;

/// Add a windowed-sinc low-pass at `fc` (a fraction of the sample rate), normalised to
/// exactly 1 at DC, into `taps` with `sign`. Allocation-free: the sinc is computed twice,
/// once for the normalising sum and once to write.
fn add_lowpass(taps: &mut [f32], window: &[f64], fc: f32, sign: f64) {
    use std::f64::consts::PI;
    let n = taps.len();
    let c = (n / 2) as f64;
    let w = 2.0 * fc as f64;
    let sinc = |k: usize| {
        let x = k as f64 - c;
        if x == 0.0 {
            w
        } else {
            (PI * w * x).sin() / (PI * x)
        }
    };
    let sum: f64 = (0..n).map(|k| sinc(k) * window[k]).sum();
    for (k, t) in taps.iter_mut().enumerate() {
        *t += (sign * sinc(k) * window[k] / sum) as f32;
    }
}

/// The split filter: symmetric taps and the window they are cut with.
#[derive(Clone, Debug)]
pub(super) struct Fir {
    pub taps: Vec<f32>,
    window: Vec<f64>,
}

impl Fir {
    pub fn new(sr: f32) -> Self {
        let n = fir_len(sr);
        let i0b = bessel_i0(KAISER_BETA);
        let window = (0..n)
            .map(|k| {
                let r = 2.0 * k as f64 / (n - 1) as f64 - 1.0;
                bessel_i0(KAISER_BETA * (1.0 - r * r).max(0.0).sqrt()) / i0b
            })
            .collect();
        Fir {
            taps: vec![0.0; n],
            window,
        }
    }

    pub fn len(&self) -> usize {
        self.taps.len()
    }

    /// The FIR's centre: its delay, in samples.
    pub fn centre(&self) -> usize {
        self.taps.len() / 2
    }

    /// Design the band for a processing mode. Wide band needs no FIR and leaves it as is.
    pub fn design(&mut self, processing: Processing, sr: f32, lo: f32, hi: f32) {
        let n = self.taps.len();
        self.taps.iter_mut().for_each(|t| *t = 0.0);
        match processing {
            Processing::Wide => {}
            Processing::SplitHigh => {
                // Everything above `lo`: a unit impulse minus the low-pass.
                self.taps[n / 2] = 1.0;
                add_lowpass(&mut self.taps, &self.window, lo / sr, -1.0);
            }
            Processing::SplitBand => {
                if hi >= sr * 0.45 {
                    self.taps[n / 2] = 1.0;
                } else {
                    add_lowpass(&mut self.taps, &self.window, hi / sr, 1.0);
                }
                add_lowpass(&mut self.taps, &self.window, lo / sr, -1.0);
            }
        }
    }

    /// The band component of the stretch `s` (oldest first, `len()` long).
    #[inline]
    pub fn apply(&self, s: &[f32]) -> f32 {
        let n = self.taps.len();
        let c = n / 2;
        let h = &self.taps;
        // Symmetric taps: fold the two halves. Four accumulators, always summed in the
        // same order, so the result is the same whatever the block size.
        let mut acc = [0f32; 4];
        let mut j = 0;
        while j + 4 <= c {
            for a in 0..4 {
                acc[a] += h[j + a] * (s[j + a] + s[n - 1 - j - a]);
            }
            j += 4;
        }
        while j < c {
            acc[0] += h[j] * (s[j] + s[n - 1 - j]);
            j += 1;
        }
        (acc[0] + acc[1]) + (acc[2] + acc[3]) + h[c] * s[c]
    }

    /// The FIR's real (zero-phase) amplitude at `hz`: how much of a sine there is taken
    /// as "the band". What the editor draws the cut from.
    pub fn amplitude(&self, sr: f32, hz: f32) -> f32 {
        let n = self.taps.len();
        let c = n / 2;
        let w = 2.0 * std::f64::consts::PI * hz as f64 / sr as f64;
        let mut a = self.taps[c] as f64;
        for j in 1..=c {
            a += 2.0 * self.taps[c - j] as f64 * (w * j as f64).cos();
        }
        a as f32
    }
}

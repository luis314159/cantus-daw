//! The spectral envelope a transposition keeps: the **true envelope** (Röbel and Rodet,
//! DAFx 2005) — the cepstrally smoothed log spectrum, raised again and again to the
//! spectrum wherever the spectrum stands above it, until it rests on the partials' peaks
//! instead of running between them (what a plain cepstral envelope does, and why it would
//! lose a voice's formants under its harmonics).

// Index loops over spectra and frames read clearer than iterator chains here.
#![allow(clippy::needless_range_loop)]

use crate::aequalis::fft::Fft;
use crate::math;

/// Rounds of raising the smoothed envelope to the spectrum.
const ROUNDS: usize = 16;

pub(crate) struct Envelope {
    n: usize,
    /// The cepstral order: quefrencies kept, about a 1.25 ms resolution (an envelope that
    /// follows formants, not a voice's harmonics up to ~400 Hz).
    order: usize,
    fft: Fft,
    re: Vec<f32>,
    im: Vec<f32>,
    target: Vec<f32>,
}

impl Envelope {
    pub fn new(n: usize, rate: f32) -> Self {
        Envelope {
            n,
            order: ((rate / 800.0).round() as usize).clamp(8, n / 8),
            fft: Fft::new(n),
            re: vec![0.0; n],
            im: vec![0.0; n],
            target: vec![0.0; n / 2 + 1],
        }
    }

    /// The envelope of the magnitude spectrum `mag` (`n/2 + 1` bins), linear, into `out`.
    pub fn compute(&mut self, mag: &[f32], out: &mut [f32]) {
        let (n, half) = (self.n, self.n / 2);
        let top = mag.iter().fold(0.0f32, |m, v| m.max(*v));
        let floor = (top * 1e-5).max(1e-12);
        for (t, m) in self.target.iter_mut().zip(mag) {
            *t = math::ln(m.max(floor));
        }
        let mut smooth = vec![f32::NEG_INFINITY; half + 1];
        for _ in 0..ROUNDS {
            for k in 0..=half {
                let a = self.target[k].max(smooth[k]);
                self.target[k] = a;
                self.re[k] = a;
                self.im[k] = 0.0;
                if k > 0 && k < half {
                    self.re[n - k] = a;
                    self.im[n - k] = 0.0;
                }
            }
            // To the cepstrum, keep the low quefrencies, and back.
            self.fft.run(&mut self.re, &mut self.im, true);
            for q in self.order + 1..n - self.order {
                self.re[q] = 0.0;
                self.im[q] = 0.0;
            }
            self.fft.run(&mut self.re, &mut self.im, false);
            for (k, s) in smooth.iter_mut().enumerate() {
                *s = self.re[k] / n as f32;
            }
        }
        for (o, s) in out.iter_mut().zip(&smooth) {
            *o = math::exp(*s);
        }
    }
}

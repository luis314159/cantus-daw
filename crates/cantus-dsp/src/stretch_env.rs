//! The spectral envelope a transposition keeps: the **true envelope** (Röbel and Rodet,
//! DAFx 2005) — the cepstrally smoothed log spectrum, raised again and again to the
//! spectrum wherever the spectrum stands above it, until it rests on the partials' peaks
//! instead of running between them (what a plain cepstral envelope does, and why it would
//! lose a voice's formants under its harmonics).
//!
//! The envelope keeps only low quefrencies, so it is computed on the spectrum **pooled**
//! (the largest of every few bins: no peak lost) to a 512-point grid and read back between
//! its points — the same envelope at an eighth of the work for a 4 096-point frame.

// Index loops over spectra and frames read clearer than iterator chains here.
#![allow(clippy::needless_range_loop)]

use crate::aequalis::fft::Fft;
use crate::math;

/// Rounds of raising the smoothed envelope to the spectrum.
const ROUNDS: usize = 16;
/// The pooled grid's transform length (at most the frame's).
const GRID: usize = 512;

pub(crate) struct Envelope {
    /// The frame's transform length, and how many of its bins pool into one.
    n: usize,
    pool: usize,
    /// The pooled transform length.
    m: usize,
    /// The cepstral order on the pooled grid: about a 1.25 ms resolution (an envelope that
    /// follows formants, not a voice's harmonics up to ~400 Hz).
    order: usize,
    fft: Fft,
    re: Vec<f32>,
    im: Vec<f32>,
    target: Vec<f32>,
    smooth: Vec<f32>,
}

impl Envelope {
    pub fn new(n: usize, rate: f32) -> Self {
        let m = GRID.min(n);
        let pool = (n / m).max(1);
        // Cycles across the whole spectrum: the same on any grid that samples it.
        let order = ((rate / 800.0).round() as usize).clamp(4, m / 4);
        Envelope {
            n,
            pool,
            m,
            order,
            fft: Fft::new(m),
            re: vec![0.0; m],
            im: vec![0.0; m],
            target: vec![0.0; m / 2 + 1],
            smooth: vec![0.0; m / 2 + 1],
        }
    }

    /// The envelope of the magnitude spectrum `mag` (`n/2 + 1` bins), linear, into `out`.
    pub fn compute(&mut self, mag: &[f32], out: &mut [f32]) {
        let (m, half) = (self.m, self.m / 2);
        let top = mag.iter().fold(0.0f32, |a, v| a.max(*v));
        let floor = (top * 1e-5).max(1e-12);
        let p = self.pool;
        // Pooled: each grid point the largest of the bins around it.
        for (j, t) in self.target.iter_mut().enumerate() {
            let lo = (j * p).saturating_sub(p / 2);
            let hi = (j * p + p / 2 + 1).min(mag.len());
            let v = mag[lo.min(mag.len() - 1)..hi.max(lo + 1).min(mag.len())]
                .iter()
                .fold(0.0f32, |a, v| a.max(*v));
            *t = math::ln(v.max(floor));
        }
        self.smooth.fill(f32::NEG_INFINITY);
        for _ in 0..ROUNDS {
            for k in 0..=half {
                let a = self.target[k].max(self.smooth[k]);
                self.target[k] = a;
                self.re[k] = a;
                self.im[k] = 0.0;
                if k > 0 && k < half {
                    self.re[m - k] = a;
                    self.im[m - k] = 0.0;
                }
            }
            // To the cepstrum, keep the low quefrencies, and back.
            self.fft.run(&mut self.re, &mut self.im, true);
            for q in self.order + 1..m - self.order {
                self.re[q] = 0.0;
                self.im[q] = 0.0;
            }
            self.fft.run(&mut self.re, &mut self.im, false);
            for (k, s) in self.smooth.iter_mut().enumerate() {
                *s = self.re[k] / m as f32;
            }
        }
        // Back onto the frame's bins: between grid points, in the log domain.
        let bins = self.n / 2 + 1;
        for (k, o) in out.iter_mut().enumerate().take(bins) {
            let x = k as f32 / p as f32;
            let i = (x.floor() as usize).min(half);
            let j = (i + 1).min(half);
            let t = x - i as f32;
            *o = math::exp(self.smooth[i] + (self.smooth[j] - self.smooth[i]) * t);
        }
    }
}

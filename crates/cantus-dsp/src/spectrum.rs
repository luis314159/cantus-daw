//! A spectrum analyser for displays: two signals (an effect's input and its output) through
//! a Hann-windowed FFT, read back as levels at any frequency with fractional-octave
//! smoothing — what an EQ draws behind its curve.
//!
//! This runs on the UI thread, from samples the audio thread copied into a ring; nothing
//! here is on the audio path. The FFT is a plain iterative radix-2, which at 4096 points
//! twice a frame is a fraction of a millisecond and needs no dependency.

use crate::math;
use std::f32::consts::PI;

/// In-place complex FFT. `re.len()` must be a power of two.
pub fn fft(re: &mut [f32], im: &mut [f32]) {
    let n = re.len();
    debug_assert!(n.is_power_of_two() && im.len() == n);
    // Bit reversal.
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        let ang = -2.0 * PI / len as f32;
        let (wr, wi) = (math::cos(ang), math::sin(ang));
        for start in (0..n).step_by(len) {
            let (mut cr, mut ci) = (1.0f32, 0.0f32);
            for k in 0..len / 2 {
                let a = start + k;
                let b = a + len / 2;
                let tr = re[b] * cr - im[b] * ci;
                let ti = re[b] * ci + im[b] * cr;
                re[b] = re[a] - tr;
                im[b] = im[a] - ti;
                re[a] += tr;
                im[a] += ti;
                let ncr = cr * wr - ci * wi;
                ci = cr * wi + ci * wr;
                cr = ncr;
            }
        }
        len <<= 1;
    }
}

/// Two signals' spectra, updated from interleaved `(a, b)` sample pairs.
#[derive(Clone, Debug)]
pub struct SpectrumAnalyser {
    n: usize,
    hist: [Vec<f32>; 2],
    pos: usize,
    filled: usize,
    window: Vec<f32>,
    /// Scale that makes a full-scale sine read 0 dB.
    norm: f32,
    re: Vec<f32>,
    im: Vec<f32>,
    /// Power per bin, for bins `0..n/2`, per signal.
    power: [Vec<f32>; 2],
}

impl SpectrumAnalyser {
    pub fn new(n: usize) -> Self {
        let n = n.next_power_of_two().max(64);
        let window: Vec<f32> = (0..n)
            .map(|i| 0.5 - 0.5 * math::cos(2.0 * PI * i as f32 / n as f32))
            .collect();
        let sum: f32 = window.iter().sum();
        SpectrumAnalyser {
            n,
            hist: [vec![0.0; n], vec![0.0; n]],
            pos: 0,
            filled: 0,
            norm: 2.0 / sum,
            window,
            re: vec![0.0; n],
            im: vec![0.0; n],
            power: [vec![0.0; n / 2], vec![0.0; n / 2]],
        }
    }

    pub fn size(&self) -> usize {
        self.n
    }

    /// Add interleaved `(a, b)` pairs.
    pub fn push(&mut self, pairs: &[f32]) {
        for p in pairs.chunks_exact(2) {
            self.hist[0][self.pos] = p[0];
            self.hist[1][self.pos] = p[1];
            self.pos = (self.pos + 1) % self.n;
        }
        self.filled = (self.filled + pairs.len() / 2).min(self.n);
    }

    /// Recompute both spectra from the most recent `n` samples.
    pub fn compute(&mut self) {
        for s in 0..2 {
            for i in 0..self.n {
                let x = self.hist[s][(self.pos + i) % self.n];
                self.re[i] = x * self.window[i] * self.norm;
                self.im[i] = 0.0;
            }
            fft(&mut self.re, &mut self.im);
            for k in 0..self.n / 2 {
                self.power[s][k] = self.re[k] * self.re[k] + self.im[k] * self.im[k];
            }
        }
    }

    /// Level of signal `which` (0 or 1) around `hz`, averaged in power over `octaves`
    /// (1/6 is a good display smoothing), dB. −120 for silence.
    pub fn level_db(&self, which: usize, sample_rate: f32, hz: f32, octaves: f32) -> f32 {
        let bin_hz = sample_rate / self.n as f32;
        let half = math::powf(2f32, octaves * 0.5);
        let p = &self.power[which.min(1)];
        // Narrower than two bins (the low end on a log axis): interpolate between the two
        // nearest bins instead of averaging one, or the display is a staircase.
        if (hz * half - hz / half) / bin_hz < 2.0 {
            let pos = (hz / bin_hz).clamp(1.0, (self.n / 2 - 2) as f32);
            let k = pos.floor() as usize;
            let f = pos - k as f32;
            let power = (p[k] * (1.0 - f) + p[k + 1] * f) * 1.5;
            return if power > 1e-12 {
                10.0 * math::log10(power)
            } else {
                -120.0
            };
        }
        let lo = ((hz / half) / bin_hz).floor().max(1.0) as usize;
        let hi = (((hz * half) / bin_hz).ceil() as usize)
            .min(self.n / 2 - 1)
            .max(lo);
        let (sum, count) = (lo..=hi).fold((0.0f32, 0usize), |(s, c), k| (s + p[k], c + 1));
        // A sine spreads over the Hann main lobe (about 1.5 bins of power), so the average
        // over a narrow band is scaled back to the peak it came from.
        let power = if count > 0 {
            sum / count as f32 * (count as f32 / 1.5).max(1.0)
        } else {
            0.0
        };
        if power > 1e-12 {
            10.0 * math::log10(power)
        } else {
            -120.0
        }
    }

    pub fn has_data(&self) -> bool {
        self.filled == self.n
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sine_shows_up_at_its_frequency_at_its_level() {
        let sr = 48_000.0;
        let mut a = SpectrumAnalyser::new(4096);
        let pairs: Vec<f32> = (0..8192)
            .flat_map(|i| {
                let t = i as f32 / sr;
                let x = math::sin(2.0 * PI * 1_000.0 * t);
                [x, 0.25 * x]
            })
            .collect();
        a.push(&pairs);
        assert!(a.has_data());
        a.compute();
        let at = a.level_db(0, sr, 1_000.0, 1.0 / 6.0);
        assert!(at.abs() < 1.5, "a full-scale sine read {at} dB");
        let quarter = a.level_db(1, sr, 1_000.0, 1.0 / 6.0);
        assert!(
            (quarter - at + 12.04).abs() < 0.5,
            "a quarter is −12 dB: {quarter}"
        );
        let away = a.level_db(0, sr, 8_000.0, 1.0 / 6.0);
        assert!(away < at - 60.0, "leakage at 8 kHz: {away}");
    }

    #[test]
    fn the_fft_of_an_impulse_is_flat() {
        let mut re = vec![0.0; 16];
        let mut im = vec![0.0; 16];
        re[0] = 1.0;
        fft(&mut re, &mut im);
        for k in 0..16 {
            assert!((re[k] - 1.0).abs() < 1e-6 && im[k].abs() < 1e-6);
        }
    }
}

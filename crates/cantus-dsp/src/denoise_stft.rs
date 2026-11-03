//! The short-time Fourier transform the audio editor's offline tools share (ADR 0096): a
//! square-root-Hann window at 75 % overlap, whose analysis and synthesis windows together
//! sum to a constant, so a signal that is not touched comes back to within float rounding.
//! A frame's spectrum is handed to a closure, which may change bins `0..=N/2`; the mirrored
//! half is rebuilt (conjugate symmetry) so the output is real.
//!
//! Offline only — it allocates freely and never runs on the audio thread. Deterministic:
//! the window and the twiddles come from `crate::math`.

use crate::aequalis::fft::Fft;
use crate::math;

/// The transform's size and hop (75 % overlap).
pub const N: usize = 2048;
pub const HOP: usize = N / 4;
/// Bins that carry information: 0 (DC) to N/2 (Nyquist).
pub const BINS: usize = N / 2 + 1;

/// The transform with its window and scratch.
pub struct Stft {
    fft: Fft,
    window: Vec<f32>,
    re: Vec<f32>,
    im: Vec<f32>,
}

impl Default for Stft {
    fn default() -> Self {
        Self::new()
    }
}

impl Stft {
    pub fn new() -> Self {
        let window = (0..N)
            .map(|i| {
                let hann = 0.5 - 0.5 * math::cos(2.0 * std::f64::consts::PI * i as f64 / N as f64);
                hann.sqrt() as f32
            })
            .collect();
        Stft {
            fft: Fft::new(N),
            window,
            re: vec![0.0; N],
            im: vec![0.0; N],
        }
    }

    /// How many frames cover `len` samples: the first starts `N - HOP` before the signal,
    /// so every sample is under four frames.
    pub fn frames_for(len: usize) -> usize {
        if len == 0 {
            0
        } else {
            (len + N - HOP).div_ceil(HOP)
        }
    }

    /// The source index of frame `k`'s first sample (may be negative).
    pub fn frame_start(k: usize) -> isize {
        (k * HOP) as isize - (N - HOP) as isize
    }

    /// Analyse `x`, let `f(frame, re, im)` change bins `0..=N/2` of each frame's spectrum
    /// (`re` and `im` are `N` long; only the first [`BINS`] are to be written), and
    /// overlap-add what comes back. The output is as long as `x`.
    pub fn run<F>(&mut self, x: &[f32], mut f: F) -> Vec<f32>
    where
        F: FnMut(usize, &mut [f32], &mut [f32]),
    {
        let mut out = vec![0.0f32; x.len()];
        for k in 0..Self::frames_for(x.len()) {
            let s = Self::frame_start(k);
            for i in 0..N {
                let at = s + i as isize;
                let v = if at >= 0 && (at as usize) < x.len() {
                    x[at as usize]
                } else {
                    0.0
                };
                self.re[i] = v * self.window[i];
                self.im[i] = 0.0;
            }
            self.fft.run(&mut self.re, &mut self.im, false);
            f(k, &mut self.re, &mut self.im);
            // The conjugate half, from the half the closure left.
            for b in 1..N / 2 {
                self.re[N - b] = self.re[b];
                self.im[N - b] = -self.im[b];
            }
            self.im[0] = 0.0;
            self.im[N / 2] = 0.0;
            self.fft.run(&mut self.re, &mut self.im, true);
            // Hann at a hop of N/4 sums to 2; the transform pair scales by N.
            let scale = 0.5 / N as f32;
            for i in 0..N {
                let at = s + i as isize;
                if at >= 0 && (at as usize) < out.len() {
                    out[at as usize] += self.re[i] * self.window[i] * scale;
                }
            }
        }
        out
    }

    /// The mean power per bin of the frames that lie wholly inside `x` (no padding in
    /// them), and how many there were.
    pub fn mean_power(&mut self, x: &[f32]) -> (Vec<f32>, usize) {
        let mut sum = vec![0.0f64; BINS];
        let mut n = 0usize;
        let mut k = 0usize;
        while k * HOP + N <= x.len() {
            let frame = &x[k * HOP..k * HOP + N];
            for ((r, v), w) in self.re.iter_mut().zip(frame).zip(&self.window) {
                *r = v * w;
            }
            self.im.fill(0.0);
            self.fft.run(&mut self.re, &mut self.im, false);
            for (b, s) in sum.iter_mut().enumerate() {
                *s += (self.re[b] * self.re[b] + self.im[b] * self.im[b]) as f64;
            }
            n += 1;
            k += 1;
        }
        let mean = sum
            .iter()
            .map(|s| if n > 0 { (*s / n as f64) as f32 } else { 0.0 })
            .collect();
        (mean, n)
    }
}

/// A spectrogram for display: `frames` columns of [`BINS`] magnitudes, 8 bits each from
/// [`FLOOR_DB`] (0) to 0 dB (255), bin 0 first in each column.
pub struct Spectrogram {
    pub frames: usize,
    pub hop: usize,
    pub rate: u32,
    pub cells: Vec<u8>,
}

/// The closest columns a spectrogram draws (a zoomed-in view: about 1.3 ms at 48 kHz).
pub const MIN_HOP: usize = 64;

/// What the lowest cell is, dB (a full-scale sine reads about 0 dB).
pub const FLOOR_DB: f32 = -110.0;

impl Spectrogram {
    /// `x` is the signal (mono; mix channels first). `hop` samples between columns, at
    /// least [`MIN_HOP`]: a long take is read sparser so the picture stays a size a texture
    /// can hold.
    pub fn of(x: &[f32], rate: u32, hop: usize) -> Spectrogram {
        let hop = hop.max(MIN_HOP);
        let mut st = Stft::new();
        let frames = x.len().div_ceil(hop).max(1);
        let mut cells = vec![0u8; frames * BINS];
        // A full-scale sine peaks at (N / 2) × the window's mean, which is 2/π for
        // sqrt-Hann: dividing by that reads it as 0 dB.
        let norm = 1.0 / (N as f32 * 0.5 * (2.0 / std::f32::consts::PI));
        for k in 0..frames {
            // Centre the frame on its column.
            let centre = (k * hop) as isize;
            let s = centre - (N / 2) as isize;
            for i in 0..N {
                let at = s + i as isize;
                let v = if at >= 0 && (at as usize) < x.len() {
                    x[at as usize]
                } else {
                    0.0
                };
                st.re[i] = v * st.window[i];
                st.im[i] = 0.0;
            }
            st.fft.run(&mut st.re, &mut st.im, false);
            for b in 0..BINS {
                let m = math::hypot(st.re[b], st.im[b]) * norm;
                let db = if m > 1e-9 {
                    20.0 * math::log10(m)
                } else {
                    FLOOR_DB
                };
                let v = ((db - FLOOR_DB) / -FLOOR_DB).clamp(0.0, 1.0);
                cells[k * BINS + b] = (v * 255.0 + 0.5) as u8;
            }
        }
        Spectrogram {
            frames,
            hop,
            rate,
            cells,
        }
    }

    /// The cell's level, dB.
    pub fn db_at(&self, frame: usize, bin: usize) -> f32 {
        let v = self.cells[frame.min(self.frames - 1) * BINS + bin.min(BINS - 1)];
        FLOOR_DB + v as f32 / 255.0 * -FLOOR_DB
    }

    /// The frequency of `bin`, Hz.
    pub fn hz_of(&self, bin: usize) -> f32 {
        bin as f32 * self.rate as f32 / N as f32
    }

    /// The bin nearest `hz`.
    pub fn bin_of(&self, hz: f32) -> usize {
        ((hz * N as f32 / self.rate as f32).round().max(0.0) as usize).min(BINS - 1)
    }
}

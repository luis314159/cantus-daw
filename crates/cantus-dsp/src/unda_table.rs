//! A wavetable and its octave mip levels, built once off the audio thread and shared by
//! every voice that plays it (`research/instruments/wavetable.md` §4.3).
//!
//! A frame is one cycle of 2048 samples: harmonics 1…1023 (the DC and the Nyquist bin are
//! dropped). Level ℓ keeps harmonics up to `1024 >> ℓ` and is stored oversampled: level
//! 0 at 8192 samples, 1 at 4096, 2 and 3 at 2048, 4 at 1024, 5 at 512 and 6–10 at 256 —
//! every level at least 8× its top harmonic (the research proposed 2048 for levels 0–2;
//! at 2× and 4× their images folded back at −73 and −80 dB for a saw at A1). Each stored
//! cycle carries five guard samples (two before, three after) so a read never wraps.
//! About 77 KB a frame: 20 MB for a full 256-frame table.
//!
//! **Reading.** A read's images sit around multiples of the table's own rate (`L·f0`)
//! and fold back from multiples of the oversampled rate — into the audio band, for some
//! pitches, whatever the decimator does. So their level is decided by the interpolator:
//! levels stored at least 32× their top harmonic (7 and up) are read with the 4-point
//! Hermite cubic, the others with a 6-point, 5th-order Lagrange read, whose images are
//! 30–50 dB lower at the same oversampling (measured: a saw at A1 went from −59 to below
//! −90 dB of alias energy at 2×).
//!
//! The levels are made by truncating the frame's spectrum and one inverse FFT per level,
//! so every level is exactly band-limited. A table is normalised as a whole to a peak of
//! one on level 0, keeping the frames' relative levels.

use crate::aequalis::fft::Fft;

/// Samples in a frame (one cycle).
pub const FRAME: usize = 2048;
/// Harmonics a frame holds: 1…`HARMONICS` (the index is `h - 1`).
pub const HARMONICS: usize = FRAME / 2 - 1;
/// Mip levels.
pub const LEVELS: usize = 11;
/// Stored length of each level's cycle.
pub const LEVEL_LEN: [usize; LEVELS] = [8192, 4096, 2048, 2048, 1024, 512, 256, 256, 256, 256, 256];
/// The highest harmonic each level keeps.
pub const LEVEL_TOP: [usize; LEVELS] = [1023, 512, 256, 128, 64, 32, 16, 8, 4, 2, 1];
/// The most frames a table holds.
pub const MAX_FRAMES: usize = 256;
const GUARD: usize = 5;
/// Guard samples before a cycle.
const PRE: usize = 2;
/// Levels from here up are read with the Hermite cubic (they are ≥ 32× oversampled).
pub const CUBIC_FROM: usize = 7;

/// One harmonic: the cosine and sine amplitudes, `a·cos(2πht) + b·sin(2πht)`.
pub type Partial = (f32, f32);

#[derive(Clone, Debug)]
pub struct Table {
    frames: usize,
    data: Vec<f32>,
    base: [usize; LEVELS],
    /// Every frame's harmonics, normalised with the table (what Partes plays).
    partials: Vec<Partial>,
    /// The gain normalisation applied.
    norm: f32,
}

impl Table {
    /// A table from its frames' spectra (`spectra[f][h - 1]`), normalised as a whole to a
    /// peak of 1 (an imported table keeps its frames' relative levels, as Serum does).
    /// An empty list is one silent frame. Allocates: never on the audio thread.
    pub fn from_spectra(spectra: &[Vec<Partial>]) -> Table {
        Table::build(spectra, false)
    }

    /// The same, each frame normalised to a peak of 1 on its own: what the factory's
    /// sweeps want (their first frames would otherwise sit tens of dB down).
    pub fn from_spectra_each(spectra: &[Vec<Partial>]) -> Table {
        let n = LEVEL_LEN[0];
        let fft = Fft::new(n);
        let (mut re, mut im) = (vec![0.0f32; n], vec![0.0f32; n]);
        let scaled: Vec<Vec<Partial>> = spectra
            .iter()
            .map(|s| {
                re.iter_mut().for_each(|x| *x = 0.0);
                im.iter_mut().for_each(|x| *x = 0.0);
                for (i, &(a, b)) in s.iter().enumerate().take(HARMONICS) {
                    let h = i + 1;
                    re[h] = 0.5 * a;
                    im[h] = -0.5 * b;
                    re[n - h] = 0.5 * a;
                    im[n - h] = 0.5 * b;
                }
                fft.run(&mut re, &mut im, true);
                let peak = re.iter().fold(0.0f32, |m, x| m.max(x.abs()));
                let g = if peak > 1e-9 { 1.0 / peak } else { 1.0 };
                s.iter().map(|(a, b)| (a * g, b * g)).collect()
            })
            .collect();
        Table::build(&scaled, true)
    }

    #[allow(clippy::needless_range_loop)] // levels index three parallel tables
    fn build(spectra: &[Vec<Partial>], keep_level: bool) -> Table {
        let frames = spectra.len().clamp(1, MAX_FRAMES);
        let mut base = [0; LEVELS];
        let mut total = 0;
        for l in 0..LEVELS {
            base[l] = total;
            total += frames * (LEVEL_LEN[l] + GUARD);
        }
        let mut partials = vec![(0.0, 0.0); frames * HARMONICS];
        for (f, spec) in spectra.iter().take(frames).enumerate() {
            for (i, p) in spec.iter().take(HARMONICS).enumerate() {
                partials[f * HARMONICS + i] = *p;
            }
        }
        let mut t = Table {
            frames,
            data: vec![0.0; total],
            base,
            partials,
            norm: 1.0,
        };
        let fft = Fft::new(LEVEL_LEN[0]);
        let mut re = vec![0.0f32; LEVEL_LEN[0]];
        let mut im = vec![0.0f32; LEVEL_LEN[0]];
        let silent: Vec<Partial> = Vec::new();
        for f in 0..frames {
            let spec = spectra.get(f).unwrap_or(&silent);
            for l in 0..LEVELS {
                let n = LEVEL_LEN[l];
                re[..n].iter_mut().for_each(|x| *x = 0.0);
                im[..n].iter_mut().for_each(|x| *x = 0.0);
                for (i, &(a, b)) in spec.iter().enumerate().take(LEVEL_TOP[l].min(n / 2 - 1)) {
                    let h = i + 1;
                    // a·cos + b·sin = Re{(a − jb)·e^{jθ}}: bins h and n − h, halved.
                    re[h] = 0.5 * a;
                    im[h] = -0.5 * b;
                    re[n - h] = 0.5 * a;
                    im[n - h] = 0.5 * b;
                }
                fft.run(&mut re[..n], &mut im[..n], true);
                let off = t.offset(l, f) + PRE;
                t.data[off..off + n].copy_from_slice(&re[..n]);
            }
        }
        // Normalise on level 0, across every frame.
        let mut peak = 0.0f32;
        for f in 0..frames {
            peak = t.cycle(f).iter().fold(peak, |m, x| m.max(x.abs()));
        }
        if peak > 1e-9 && !keep_level {
            let g = 1.0 / peak;
            t.norm = g;
            t.data.iter_mut().for_each(|x| *x *= g);
            t.partials.iter_mut().for_each(|(a, b)| {
                *a *= g;
                *b *= g;
            });
        }
        for l in 0..LEVELS {
            let n = LEVEL_LEN[l];
            for f in 0..frames {
                // [x(n−2), x(n−1) | x(0) … x(n−1) | x(0), x(1), x(2)]
                let off = t.offset(l, f);
                t.data[off] = t.data[off + n];
                t.data[off + 1] = t.data[off + 1 + n];
                for k in 0..3 {
                    t.data[off + PRE + n + k] = t.data[off + PRE + k];
                }
            }
        }
        t
    }

    /// A table from cycles of samples (each of any power-of-two length ≥ 4; a longer
    /// cycle is band-limited to 1023 harmonics, a shorter one keeps what it has).
    pub fn from_cycles(cycles: &[Vec<f32>]) -> Table {
        let spectra: Vec<Vec<Partial>> = cycles.iter().map(|c| spectrum_of(c)).collect();
        Table::from_spectra(&spectra)
    }

    /// A table from audio: `samples` interleaved with `channels` (the first is used), cut
    /// into cycles of `frame` samples. More than [`MAX_FRAMES`] cycles are thinned
    /// evenly; a frame size that is not a power of two is resampled (cubic, periodic) to
    /// the next one before its spectrum is taken. Too short for one cycle: one cycle of
    /// whatever there is.
    pub fn from_audio(samples: &[f32], channels: usize, frame: usize) -> Table {
        let ch = channels.max(1);
        let mono: Vec<f32> = samples.iter().step_by(ch).copied().collect();
        let frame = frame.clamp(4, 1 << 16);
        let count = (mono.len() / frame).max(1);
        let frame = frame.min(mono.len().max(4));
        let keep = count.min(MAX_FRAMES);
        let cycles: Vec<Vec<f32>> = (0..keep)
            .map(|k| {
                // Evenly thinned: the first and the last cycle are always kept.
                let src = if keep > 1 {
                    k * (count - 1) / (keep - 1)
                } else {
                    0
                };
                let start = (src * frame).min(mono.len().saturating_sub(1));
                let end = (start + frame).min(mono.len());
                let mut cycle: Vec<f32> = mono[start..end].to_vec();
                cycle.resize(frame, 0.0);
                if frame.is_power_of_two() {
                    cycle
                } else {
                    resample_cycle(&cycle, frame.next_power_of_two())
                }
            })
            .collect();
        Table::from_cycles(&cycles)
    }

    pub fn frames(&self) -> usize {
        self.frames
    }

    #[inline]
    fn offset(&self, level: usize, frame: usize) -> usize {
        self.base[level] + frame * (LEVEL_LEN[level] + GUARD)
    }

    /// Frame `f`'s full-bandwidth cycle (level 0, `LEVEL_LEN[0]` samples) — for displays.
    pub fn cycle(&self, f: usize) -> &[f32] {
        let off = self.offset(0, f.min(self.frames - 1)) + PRE;
        &self.data[off..off + LEVEL_LEN[0]]
    }

    /// Level `level`'s cycle of frame `f`, as stored (without the guards).
    pub fn level_cycle(&self, level: usize, f: usize) -> &[f32] {
        let l = level.min(LEVELS - 1);
        let off = self.offset(l, f.min(self.frames - 1)) + PRE;
        &self.data[off..off + LEVEL_LEN[l]]
    }

    /// A read of `frame` at `level`, phase `phi` in [0, 1]: the 6-point Lagrange read
    /// below [`CUBIC_FROM`], the Hermite cubic from there up.
    #[inline]
    pub fn read(&self, level: usize, frame: usize, phi: f32) -> f32 {
        let n = LEVEL_LEN[level];
        let x = phi * n as f32;
        let i = (x as usize).min(n - 1);
        let t = x - i as f32;
        let off = self.offset(level, frame) + i;
        if level >= CUBIC_FROM {
            let d = &self.data[off + 1..off + 5];
            hermite(d[0], d[1], d[2], d[3], t)
        } else {
            lagrange6(&self.data[off..off + 6], t)
        }
    }

    /// Bytes held.
    pub fn bytes(&self) -> usize {
        self.data.len() * 4 + self.partials.len() * 8
    }

    /// Harmonic `k + 1` of frame `frame` as `(cos, sin)` amplitudes, normalised with the
    /// table; zero past the last harmonic.
    #[inline]
    pub fn partial(&self, frame: usize, k: usize) -> Partial {
        if k >= HARMONICS {
            return (0.0, 0.0);
        }
        self.partials[frame.min(self.frames - 1) * HARMONICS + k]
    }

    /// Where frame `frame` of `level` starts (for [`Table::apply`]); computed on the
    /// control grid, not per sample.
    #[inline]
    pub fn base(&self, level: usize, frame: usize) -> usize {
        self.offset(level.min(LEVELS - 1), frame.min(self.frames - 1))
    }

    /// A prepared read applied to the cycle starting at `base`: one bounds check, six
    /// multiply-adds.
    #[inline]
    pub fn apply(&self, base: usize, r: &Read) -> f32 {
        let d: &[f32; 6] = self.data[base + r.i..base + r.i + 6]
            .try_into()
            .expect("six taps");
        r.w[0] * d[0]
            + r.w[1] * d[1]
            + r.w[2] * d[2]
            + r.w[3] * d[3]
            + r.w[4] * d[4]
            + r.w[5] * d[5]
    }
}

/// A read of one level at one phase, prepared once and applied to as many frames of that
/// level as needed (the position crossfade reads two with the same weights).
#[derive(Copy, Clone, Debug, Default)]
pub struct Read {
    i: usize,
    w: [f32; 6],
}

impl Read {
    /// `level` at `phi` in [0, 1]: Lagrange weights below [`CUBIC_FROM`], Hermite (in the
    /// middle four taps) from there up.
    #[inline]
    pub fn new(level: usize, phi: f32) -> Read {
        let n = LEVEL_LEN[level];
        let x = phi * n as f32;
        let i = (x as usize).min(n - 1);
        let t = x - i as f32;
        let w = if level >= CUBIC_FROM {
            let t2 = t * t;
            let t3 = t2 * t;
            [
                0.0,
                -0.5 * t + t2 - 0.5 * t3,
                1.0 - 2.5 * t2 + 1.5 * t3,
                0.5 * t + 2.0 * t2 - 1.5 * t3,
                -0.5 * t2 + 0.5 * t3,
                0.0,
            ]
        } else {
            lagrange6_weights(t)
        };
        Read { i, w }
    }
}

/// The 4-point, 3rd-order Hermite interpolator between `x0` and `x1` at `t` in [0, 1).
#[inline]
pub fn hermite(xm1: f32, x0: f32, x1: f32, x2: f32, t: f32) -> f32 {
    let c1 = 0.5 * (x1 - xm1);
    let c2 = xm1 - 2.5 * x0 + 2.0 * x1 - 0.5 * x2;
    let c3 = 0.5 * (x2 - xm1) + 1.5 * (x0 - x1);
    ((c3 * t + c2) * t + c1) * t + x0
}

/// The 6-point, 5th-order Lagrange interpolator: `d` holds x(−2)…x(3), the result is at
/// `t` in [0, 1) between x(0) and x(1). The weights are products of `(t − m)` over the
/// other five points, built from prefix and suffix products.
#[inline]
pub fn lagrange6(d: &[f32], t: f32) -> f32 {
    let w = lagrange6_weights(t);
    w[0] * d[0] + w[1] * d[1] + w[2] * d[2] + w[3] * d[3] + w[4] * d[4] + w[5] * d[5]
}

/// The six Lagrange weights at `t`.
#[inline]
pub fn lagrange6_weights(t: f32) -> [f32; 6] {
    let (a, b, c, e, f, g) = (t + 2.0, t + 1.0, t, t - 1.0, t - 2.0, t - 3.0);
    let ab = a * b;
    let abc = ab * c;
    let abce = abc * e;
    let fg = f * g;
    let efg = e * fg;
    let cefg = c * efg;
    let w0 = b * cefg * (-1.0 / 120.0);
    let w1 = a * cefg * (1.0 / 24.0);
    let w2 = ab * efg * (-1.0 / 12.0);
    let w3 = abc * fg * (1.0 / 12.0);
    let w4 = abce * g * (-1.0 / 24.0);
    let w5 = abce * f * (1.0 / 120.0);
    [w0, w1, w2, w3, w4, w5]
}

/// A cycle's harmonics 1…1023 (fewer when the cycle is short). `cycle.len()` must be a
/// power of two.
pub fn spectrum_of(cycle: &[f32]) -> Vec<Partial> {
    let n = cycle.len().next_power_of_two().max(4);
    let fft = Fft::new(n);
    let mut re = vec![0.0f32; n];
    let mut im = vec![0.0f32; n];
    re[..cycle.len()].copy_from_slice(cycle);
    fft.run(&mut re, &mut im, false);
    let top = HARMONICS.min(n / 2 - 1);
    let s = 2.0 / n as f32;
    (1..=top).map(|h| (re[h] * s, -im[h] * s)).collect()
}

/// Resample one periodic cycle to `n` samples with a cubic read.
pub fn resample_cycle(cycle: &[f32], n: usize) -> Vec<f32> {
    let m = cycle.len().max(1);
    let at = |i: isize| cycle[i.rem_euclid(m as isize) as usize];
    (0..n)
        .map(|k| {
            let x = k as f32 * m as f32 / n as f32;
            let i = x.floor() as isize;
            hermite(at(i - 1), at(i), at(i + 1), at(i + 2), x - i as f32)
        })
        .collect()
}

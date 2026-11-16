//! The tonal balance: a long-term spectrum in fractional-octave bands, the difference
//! between the mix's and the reference's, and how much to trust it.
//!
//! ## The spectrum
//!
//! Frames of about a third of a second (16 384 samples at 44.1 and 48 kHz, 32 768 at
//! 88.2 and 96 kHz: bins under 3 Hz apart, so the 20 Hz third-octave has its own), Hann
//! windowed, half overlapped. Both channels go through **one** complex transform (left
//! as the real part, right as the imaginary) and are taken apart by symmetry; their powers
//! are summed, as BS.1770 sums channels. Each band sums the bins it covers, a bin at a band
//! edge in proportion to its overlap (a band narrower than a bin reads its share of the
//! bin), scaled so a band reads the **mean square** of the signal in it: a full-scale sine
//! reads −3.01 dB in its band, pink noise is flat across bands, and white noise rises by
//! 3.01 dB per octave.
//!
//! Bands are centred on `1 kHz · 2^(k/n)` (n = 3 or 6), from 20 Hz to 20 kHz (the top edge
//! under 0.49 of the rate), their edges half a band either side.
//!
//! ## Long-term, and level-free
//!
//! Each band's power is averaged over every frame whose whole power is above −70 dBFS
//! (silence does not pull the average down). The curve is drawn **relative to its own
//! K-weighted total** (BS.1770's weighting at each band's centre): two versions of one
//! mix, one 6 dB louder, draw the same curve, and the difference between two curves is
//! their tonal balance, never their level.
//!
//! ## Confidence
//!
//! Each band also keeps the mean and variance of its frame levels in dB. The difference's
//! uncertainty is `u = √(s²_mix/n_mix + s²_ref/n_ref)` with `n` the frames counted over 2
//! (half-overlapped frames are not independent), and its confidence `1/(1 + (u/0.5 dB)²)`
//! — 0.5 at an uncertainty of half a decibel — and 0 until both have
//! [`MIN_FRAMES`] frames, or in a band where either has no energy. It is a statistician's
//! honest guess for a stationary signal; on music it says where the comparison is still
//! moving.
//!
//! The analyser allocates in [`Tonal::new`] only. A frame's transform is ~1 ms of work every
//! ~170 ms, so the engine runs it on a worker from a copy of the audio (Phase B), never in
//! the audio callback.

use crate::aequalis::fft::Fft;
use crate::math;
use crate::terminus::loudness::KWeighting;

/// The most bands a curve has (1/6 octave over ten octaves is 61).
pub const MAX_BANDS: usize = 64;
/// Frames before a difference is trusted at all.
pub const MIN_FRAMES: u64 = 8;
/// The frame gate, mean square (−70 dBFS).
const GATE: f64 = 1e-7;
/// Under this a band holds nothing (−200 dB).
const FLOOR: f64 = 1e-20;
/// The uncertainty at which confidence is one half, dB.
const HALF_CONFIDENCE_DB: f64 = 0.5;

/// Bands per octave.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum Resolution {
    #[default]
    Third,
    Sixth,
}

impl Resolution {
    pub fn per_octave(self) -> i32 {
        match self {
            Resolution::Third => 3,
            Resolution::Sixth => 6,
        }
    }
}

/// The bands of a resolution at a rate: centres and edges, Hz.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Bands {
    pub count: usize,
    pub centre: [f32; MAX_BANDS],
    pub lo: [f32; MAX_BANDS],
    pub hi: [f32; MAX_BANDS],
}

impl Bands {
    pub fn new(resolution: Resolution, sample_rate: f32) -> Self {
        let n = resolution.per_octave();
        let half = 1.0 / (2 * n) as f64;
        let mut b = Bands {
            count: 0,
            centre: [0.0; MAX_BANDS],
            lo: [0.0; MAX_BANDS],
            hi: [0.0; MAX_BANDS],
        };
        let lowest = 20.0 * math::powf(2f64, -half);
        let highest = 20_000.0 * math::powf(2f64, half);
        let nyquist = 0.49 * sample_rate as f64;
        let mut k = (n as f64 * math::log2(20.0 / 1_000.0f64)).floor() as i32 - 1;
        while b.count < MAX_BANDS {
            let c = 1_000.0 * math::powf(2f64, k as f64 / n as f64);
            let hi = c * math::powf(2f64, half);
            k += 1;
            if c < lowest {
                continue;
            }
            if c > highest || hi > nyquist {
                break;
            }
            b.centre[b.count] = c as f32;
            b.lo[b.count] = (c * math::powf(2f64, -half)) as f32;
            b.hi[b.count] = hi as f32;
            b.count += 1;
        }
        b
    }
}

/// The FFT size for a rate: bins under 3 Hz apart.
pub fn frame_len(sample_rate: f32) -> usize {
    ((sample_rate / 3.0).max(1024.0) as usize).next_power_of_two()
}

/// |K(f)|², BS.1770's weighting at `hz` (both stages), from its coefficients.
pub fn k_weight(sample_rate: f32, hz: f32) -> f64 {
    let k = KWeighting::new(sample_rate);
    let w = std::f64::consts::TAU * hz as f64 / sample_rate as f64;
    let mag2 = |b: [f64; 3], a: [f64; 2]| {
        let (c1, s1) = (math::cos(w), math::sin(w));
        let (c2, s2) = (math::cos(2.0 * w), math::sin(2.0 * w));
        let nr = b[0] + b[1] * c1 + b[2] * c2;
        let ni = -(b[1] * s1 + b[2] * s2);
        let dr = 1.0 + a[0] * c1 + a[1] * c2;
        let di = -(a[0] * s1 + a[1] * s2);
        (nr * nr + ni * ni) / (dr * dr + di * di)
    };
    mag2(k.shelf.b, k.shelf.a) * mag2(k.highpass.b, k.highpass.a)
}

/// A long-term curve: each band's level relative to the curve's K-weighted total, dB, and
/// the spread of its frame levels.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Curve {
    pub bands: Bands,
    /// Relative level, dB (−∞ where the band holds nothing).
    pub db: [f32; MAX_BANDS],
    /// Absolute band power, dB (mean square).
    pub abs_db: [f32; MAX_BANDS],
    /// Variance of the frame levels, dB².
    pub var: [f32; MAX_BANDS],
    /// Each band's share of the curve's K-weighted power (they sum to 1): where its
    /// loudness is.
    pub share: [f32; MAX_BANDS],
    pub frames: u64,
}

/// The difference between two curves.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Difference {
    pub bands: Bands,
    /// Mix minus reference, dB (positive: the mix has more there).
    pub db: [f32; MAX_BANDS],
    /// Its uncertainty, dB.
    pub uncertainty: [f32; MAX_BANDS],
    /// 0 … 1.
    pub confidence: [f32; MAX_BANDS],
    /// The mix's [`Curve::share`].
    pub mix_share: [f32; MAX_BANDS],
}

/// Compare two curves of the same bands.
pub fn compare(mix: &Curve, reference: &Curve) -> Difference {
    let bands = mix.bands;
    let mut d = Difference {
        bands,
        db: [0.0; MAX_BANDS],
        uncertainty: [f32::INFINITY; MAX_BANDS],
        confidence: [0.0; MAX_BANDS],
        mix_share: mix.share,
    };
    let enough = mix.frames >= MIN_FRAMES && reference.frames >= MIN_FRAMES;
    for b in 0..bands.count.min(reference.bands.count) {
        let (m, r) = (mix.db[b], reference.db[b]);
        if !(m.is_finite() && r.is_finite()) {
            continue;
        }
        d.db[b] = m - r;
        if !enough {
            continue;
        }
        let n_m = (mix.frames as f64 / 2.0).max(1.0);
        let n_r = (reference.frames as f64 / 2.0).max(1.0);
        let u = (mix.var[b] as f64 / n_m + reference.var[b] as f64 / n_r).sqrt();
        d.uncertainty[b] = u as f32;
        d.confidence[b] = (1.0 / (1.0 + (u / HALF_CONFIDENCE_DB).powi(2))) as f32;
    }
    d
}

/// The long-term spectrum analyser of one source.
#[derive(Clone, Debug)]
pub struct Tonal {
    fft: Fft,
    len: usize,
    hop: usize,
    window: Box<[f32]>,
    /// The power of the window, Σ w².
    window_power: f64,
    ring: [Box<[f32]>; 2],
    pos: usize,
    /// Samples since the last frame.
    since: usize,
    filled: usize,
    re: Box<[f32]>,
    im: Box<[f32]>,
    bands: Bands,
    /// Each band's first and last bin.
    span: [(usize, usize); MAX_BANDS],
    kw: [f64; MAX_BANDS],
    sum_p: [f64; MAX_BANDS],
    sum_db: [f64; MAX_BANDS],
    sum_db2: [f64; MAX_BANDS],
    frames: u64,
    df: f64,
}

impl Tonal {
    pub fn new(sample_rate: f32, resolution: Resolution) -> Self {
        let len = frame_len(sample_rate);
        let window: Box<[f32]> = (0..len)
            .map(|i| {
                let x = std::f64::consts::TAU * i as f64 / len as f64;
                (0.5 - 0.5 * math::cos(x)) as f32
            })
            .collect();
        let window_power = window.iter().map(|w| (*w as f64) * (*w as f64)).sum();
        let bands = Bands::new(resolution, sample_rate);
        let df = sample_rate as f64 / len as f64;
        let mut span = [(0usize, 0usize); MAX_BANDS];
        let mut kw = [0.0; MAX_BANDS];
        for b in 0..bands.count {
            let lo = ((bands.lo[b] as f64 / df - 0.5).floor().max(1.0)) as usize;
            let hi = ((bands.hi[b] as f64 / df + 0.5).ceil() as usize).min(len / 2 - 1);
            span[b] = (lo, hi.max(lo));
            kw[b] = k_weight(sample_rate, bands.centre[b]);
        }
        Tonal {
            fft: Fft::new(len),
            len,
            hop: len / 2,
            window,
            window_power,
            ring: [vec![0.0; len].into(), vec![0.0; len].into()],
            pos: 0,
            since: 0,
            filled: 0,
            re: vec![0.0; len].into(),
            im: vec![0.0; len].into(),
            bands,
            span,
            kw,
            sum_p: [0.0; MAX_BANDS],
            sum_db: [0.0; MAX_BANDS],
            sum_db2: [0.0; MAX_BANDS],
            frames: 0,
            df,
        }
    }

    pub fn bands(&self) -> &Bands {
        &self.bands
    }

    /// Frames counted (those above the gate).
    pub fn frames(&self) -> u64 {
        self.frames
    }

    pub fn push(&mut self, left: &[f32], right: &[f32]) {
        for (&l, &r) in left.iter().zip(right) {
            self.ring[0][self.pos] = l;
            self.ring[1][self.pos] = r;
            self.pos = (self.pos + 1) % self.len;
            self.filled = (self.filled + 1).min(self.len);
            self.since += 1;
            if self.since >= self.hop && self.filled == self.len {
                self.since = 0;
                self.frame();
            }
        }
    }

    fn frame(&mut self) {
        let n = self.len;
        let mut ms = 0.0f64;
        for i in 0..n {
            let j = (self.pos + i) % n;
            let (l, r) = (self.ring[0][j], self.ring[1][j]);
            ms += l as f64 * l as f64 + r as f64 * r as f64;
            self.re[i] = l * self.window[i];
            self.im[i] = r * self.window[i];
        }
        if ms / (2 * n) as f64 <= GATE {
            return;
        }
        self.fft.run(&mut self.re, &mut self.im, false);
        // One-sided power of each channel, summed, as mean square per bin.
        let scale = 2.0 / (n as f64 * self.window_power);
        let bin_power = |k: usize| -> f64 {
            let (ar, ai) = (self.re[k] as f64, self.im[k] as f64);
            let (br, bi) = (self.re[n - k] as f64, -(self.im[n - k] as f64));
            // L = (Z[k] + conj Z[n−k]) / 2, R = (Z[k] − conj Z[n−k]) / 2j.
            let (lr, li) = ((ar + br) / 2.0, (ai + bi) / 2.0);
            let (rr, ri) = ((ai - bi) / 2.0, -(ar - br) / 2.0);
            (lr * lr + li * li + rr * rr + ri * ri) * scale
        };
        for b in 0..self.bands.count {
            let (lo_hz, hi_hz) = (self.bands.lo[b] as f64, self.bands.hi[b] as f64);
            let (k0, k1) = self.span[b];
            let mut p = 0.0;
            for k in k0..=k1 {
                let (a, z) = ((k as f64 - 0.5) * self.df, (k as f64 + 0.5) * self.df);
                let overlap = (z.min(hi_hz) - a.max(lo_hz)).max(0.0) / self.df;
                if overlap > 0.0 {
                    p += bin_power(k) * overlap;
                }
            }
            self.sum_p[b] += p;
            let l = 10.0 * math::log10(p.max(FLOOR));
            self.sum_db[b] += l;
            self.sum_db2[b] += l * l;
        }
        self.frames += 1;
    }

    /// The long-term curve so far.
    pub fn curve(&self) -> Curve {
        let mut c = Curve {
            bands: self.bands,
            db: [f32::NEG_INFINITY; MAX_BANDS],
            abs_db: [f32::NEG_INFINITY; MAX_BANDS],
            var: [0.0; MAX_BANDS],
            share: [0.0; MAX_BANDS],
            frames: self.frames,
        };
        if self.frames == 0 {
            return c;
        }
        let n = self.frames as f64;
        let mut weighted = 0.0;
        for b in 0..self.bands.count {
            weighted += self.sum_p[b] / n * self.kw[b];
        }
        let reference = 10.0 * math::log10(weighted.max(FLOOR));
        for b in 0..self.bands.count {
            let p = self.sum_p[b] / n;
            if p > FLOOR {
                let abs = 10.0 * math::log10(p);
                c.abs_db[b] = abs as f32;
                c.db[b] = (abs - reference) as f32;
                c.share[b] = (p * self.kw[b] / weighted.max(FLOOR)) as f32;
            }
            let mean = self.sum_db[b] / n;
            c.var[b] = (self.sum_db2[b] / n - mean * mean).max(0.0) as f32;
        }
        c
    }

    /// A new measurement; the frame being gathered runs on.
    pub fn restart(&mut self) {
        self.sum_p = [0.0; MAX_BANDS];
        self.sum_db = [0.0; MAX_BANDS];
        self.sum_db2 = [0.0; MAX_BANDS];
        self.frames = 0;
    }

    pub fn reset(&mut self) {
        self.restart();
        for r in &mut self.ring {
            r.iter_mut().for_each(|v| *v = 0.0);
        }
        self.pos = 0;
        self.since = 0;
        self.filled = 0;
    }
}

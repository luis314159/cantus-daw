//! Speculum's ear (round 34, ADR 0083): what a whole render sounds like, kept as numbers
//! fine enough to read **any range of it afterwards** — a section, a bar, the song — without
//! rendering again. Headless: no screen, no smoothing, every sample counted.
//!
//! An [`Ear`] hears one stereo stream (the master, a strip, a track) and keeps:
//!
//! - **a column every 10 ms**, the loudness meter's own unit (`terminus::loudness`): the
//!   K-weighted energy of the pair and of its mono fold-down, the true peak (4× oversampled,
//!   BS.1770 Annex 2) and the sample peak, the plain energy, the sums correlation is made
//!   of, mid and side energy, and the sharpest second difference (what a click is);
//! - **a spectrum every 4 096 samples** (8 192-point Hann frames, half overlapped): the mid
//!   and the side signal's power in 31 third-octave bands from 20 Hz to 20 kHz.
//!
//! [`Ear::reading`] turns any range of those into the numbers an engineer reads:
//! integrated loudness and its range (EBU R128, the house meter's gating, exactly), the
//! loudest short-term and momentary, true and sample peak, RMS and crest, correlation and
//! its lowest 400 ms window, width, what a mono fold-down loses, and the octave and
//! third-octave spectrum. A range's loudness is measured from the start of that range (the
//! meter reset there), as a meter started on it reads it.
//!
//! Hearing never changes the stream: an ear only reads what it is given.

use crate::math;
use crate::terminus::loudness::{ColumnLoudness, KWeighting};
use crate::terminus::os::TruePeak;

/// Third-octave bands, 20 Hz … 20 kHz (centres 1 kHz · 2^((k − 17)/3)).
pub const THIRDS: usize = 31;
/// Octave bands, 31.5 Hz … 16 kHz (each the sum of three thirds).
pub const OCTAVES: usize = 10;
/// The analysis frame and its hop, samples.
pub const FRAME: usize = 8_192;
pub const HOP: usize = FRAME / 2;
/// The correlation windows of [`Reading::correlation_min`], seconds.
pub const CORRELATION_WINDOW_S: f64 = 0.4;

/// The octave bands' nominal names (ISO 266), as analysers label them.
pub const OCTAVE_NAMES: [&str; OCTAVES] = [
    "31", "63", "125", "250", "500", "1k", "2k", "4k", "8k", "16k",
];

/// A third-octave band's centre, Hz.
pub fn third_hz(k: usize) -> f32 {
    1_000.0 * math::powf(2f32, (k as f32 - 17.0) / 3.0)
}

/// An octave band's centre, Hz (31.5, 63, 125 … 16 000).
pub fn octave_hz(o: usize) -> f32 {
    third_hz(2 + 3 * o)
}

/// One 10 ms column.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Column {
    /// K-weighted energy of the pair (both channels summed, as BS.1770 weighs them).
    pub k: f64,
    /// The same of the mono fold-down `(l + r) / 2` on both channels.
    pub k_mono: f64,
    /// Plain mean square of the pair (both channels summed).
    pub sq: f64,
    /// Σ l·r, Σ l², Σ r² — what correlation is made of.
    pub lr: f64,
    pub ll: f64,
    pub rr: f64,
    /// Mean square of mid `(l + r) / 2` and side `(l − r) / 2`.
    pub mid: f64,
    pub side: f64,
    /// Largest true peak and sample peak (linear).
    pub true_peak: f32,
    pub peak: f32,
    /// Largest |x[n] − 2x[n−1] + x[n−2]| of either channel: how sharp the sharpest corner is.
    pub sharp: f32,
}

/// One spectrum frame: third-octave power of mid and side (linear, a full-scale sine in
/// a band reads about 1).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Spectrum {
    pub mid: [f32; THIRDS],
    pub side: [f32; THIRDS],
}

/// What a range sounds like (see the module notes). Levels in dB are relative to full
/// scale; a measurement with nothing to measure is `None` or −∞.
#[derive(Clone, Debug, PartialEq)]
pub struct Reading {
    pub seconds: f64,
    pub integrated_lufs: Option<f64>,
    pub range_lu: Option<f64>,
    pub short_term_max_lufs: Option<f64>,
    pub momentary_max_lufs: Option<f64>,
    pub true_peak_dbtp: f32,
    pub sample_peak_dbfs: f32,
    pub rms_dbfs: f32,
    /// Sample peak over RMS, dB.
    pub crest_db: f32,
    pub correlation: Option<f32>,
    pub correlation_min: Option<f32>,
    /// Side energy against mid, dB (−∞ for mono, 0 for as much side as mid).
    pub width_db: f32,
    /// Integrated loudness of the mono fold-down against the pair's, LU (0: nothing lost).
    pub mono_loss_lu: Option<f64>,
    /// Mid power per band, dB (long-term average over the range).
    pub thirds_db: [f32; THIRDS],
    pub octaves_db: [f32; OCTAVES],
    pub side_octaves_db: [f32; OCTAVES],
}

/// One stream's ear (see the module notes).
#[derive(Clone, Debug)]
pub struct Ear {
    sr: f32,
    col_len: usize,
    k: [KWeighting; 2],
    tp: [TruePeak; 2],
    acc: Column,
    acc_n: usize,
    prev: [[f32; 2]; 2],
    columns: Vec<Column>,
    // ---- spectrum ----
    win: Vec<f32>,
    mid_hist: Vec<f32>,
    side_hist: Vec<f32>,
    filled: usize,
    re: Vec<f32>,
    im: Vec<f32>,
    /// FFT bin range of each third.
    bins: [(usize, usize); THIRDS],
    norm: f32,
    spectra: Vec<Spectrum>,
    frames: u64,
}

fn db(power: f64) -> f32 {
    if power > 0.0 {
        (10.0 * math::log10(power)) as f32
    } else {
        f32::NEG_INFINITY
    }
}

fn db_amp(x: f32) -> f32 {
    if x > 0.0 {
        20.0 * math::log10(x)
    } else {
        f32::NEG_INFINITY
    }
}

impl Ear {
    /// An ear for a stream at `sample_rate`, with room for `seconds_hint` of it (it grows
    /// past that). Allocates: never on the audio thread.
    pub fn new(sample_rate: f32, seconds_hint: f64) -> Ear {
        let col_len = crate::terminus::loudness::column_len(sample_rate);
        let cols = (seconds_hint.max(0.0) * 100.0) as usize + 2;
        let frames = (seconds_hint.max(0.0) * sample_rate as f64 / HOP as f64) as usize + 2;
        let n = FRAME as f32;
        let win: Vec<f32> = (0..FRAME)
            .map(|i| 0.5 - 0.5 * math::cos(2.0 * std::f32::consts::PI * i as f32 / n))
            .collect();
        let mean_w2 = win.iter().map(|w| w * w).sum::<f32>() / n;
        let mut bins = [(0, 0); THIRDS];
        let per_bin = sample_rate / n;
        let edge = math::powf(2f32, 1.0 / 6.0);
        for (k, b) in bins.iter_mut().enumerate() {
            let c = third_hz(k);
            let lo = ((c / edge / per_bin).round() as usize).max(1);
            let hi = ((c * edge / per_bin).round() as usize).clamp(lo + 1, FRAME / 2);
            *b = (lo.min(FRAME / 2 - 1), hi);
        }
        Ear {
            sr: sample_rate,
            col_len,
            k: [KWeighting::new(sample_rate); 2],
            tp: [TruePeak::new(), TruePeak::new()],
            acc: Column::default(),
            acc_n: 0,
            prev: [[0.0; 2]; 2],
            columns: Vec::with_capacity(cols),
            win,
            mid_hist: vec![0.0; FRAME],
            side_hist: vec![0.0; FRAME],
            filled: 0,
            re: vec![0.0; FRAME],
            im: vec![0.0; FRAME],
            bins,
            // A one-sided power spectrum of a Hann frame back to the mean square of the
            // signal (Parseval, the window's mean square divided out).
            norm: 2.0 / (n * n * mean_w2),
            spectra: Vec::with_capacity(frames),
            frames: 0,
        }
    }

    pub fn sample_rate(&self) -> f32 {
        self.sr
    }

    /// Samples heard.
    pub fn frames(&self) -> u64 {
        self.frames
    }

    /// Samples in one column.
    pub fn column_len(&self) -> usize {
        self.col_len
    }

    /// The whole columns heard so far.
    pub fn columns(&self) -> &[Column] {
        &self.columns
    }

    /// The spectra so far; frame `j` covers samples `j·HOP .. j·HOP + FRAME`.
    pub fn spectra(&self) -> &[Spectrum] {
        &self.spectra
    }

    /// Hear one block of the stream. A sample that is not a number is heard as silence.
    pub fn push(&mut self, left: &[f32], right: &[f32]) {
        for (&l, &r) in left.iter().zip(right) {
            let l = if l.is_finite() { l } else { 0.0 };
            let r = if r.is_finite() { r } else { 0.0 };
            self.sample(l, r);
        }
    }

    #[inline]
    fn sample(&mut self, l: f32, r: f32) {
        let kl = self.k[0].tick(l as f64);
        let kr = self.k[1].tick(r as f64);
        let km = 0.5 * (kl + kr);
        let (lf, rf) = (l as f64, r as f64);
        let (m, s) = (0.5 * (lf + rf), 0.5 * (lf - rf));
        let a = &mut self.acc;
        a.k += kl * kl + kr * kr;
        a.k_mono += 2.0 * km * km;
        a.sq += lf * lf + rf * rf;
        a.lr += lf * rf;
        a.ll += lf * lf;
        a.rr += rf * rf;
        a.mid += m * m;
        a.side += s * s;
        a.true_peak = a.true_peak.max(self.tp[0].push(l)).max(self.tp[1].push(r));
        a.peak = a.peak.max(l.abs()).max(r.abs());
        let d2 = |x: f32, p: [f32; 2]| (x - 2.0 * p[0] + p[1]).abs();
        a.sharp = a.sharp.max(d2(l, self.prev[0])).max(d2(r, self.prev[1]));
        self.prev = [[l, self.prev[0][0]], [r, self.prev[1][0]]];
        self.acc_n += 1;
        if self.acc_n == self.col_len {
            let n = self.col_len as f64;
            let mut c = std::mem::take(&mut self.acc);
            c.k /= n;
            c.k_mono /= n;
            c.sq /= n;
            c.mid /= n;
            c.side /= n;
            self.columns.push(c);
            self.acc_n = 0;
        }
        // The spectrum's frame: the newest FRAME samples of mid and side.
        // (Never full here: a full frame is analysed and halved at once below.)
        self.mid_hist[self.filled] = m as f32;
        self.side_hist[self.filled] = s as f32;
        self.filled += 1;
        self.frames += 1;
        if self.filled == FRAME {
            self.analyse();
            // Keep the newer half; the next frame completes HOP samples from now.
            self.mid_hist.copy_within(HOP.., 0);
            self.side_hist.copy_within(HOP.., 0);
            self.filled = FRAME - HOP;
        }
    }

    /// One frame: mid as the real part and side as the imaginary, one FFT, the two
    /// spectra separated by symmetry.
    fn analyse(&mut self) {
        for i in 0..FRAME {
            self.re[i] = self.mid_hist[i] * self.win[i];
            self.im[i] = self.side_hist[i] * self.win[i];
        }
        crate::spectrum::fft(&mut self.re, &mut self.im);
        let mut out = Spectrum {
            mid: [0.0; THIRDS],
            side: [0.0; THIRDS],
        };
        for (k, &(lo, hi)) in self.bins.iter().enumerate() {
            let (mut pm, mut ps) = (0f32, 0f32);
            for b in lo..hi {
                let (zr, zi) = (self.re[b], self.im[b]);
                let (wr, wi) = (self.re[FRAME - b], -self.im[FRAME - b]);
                // M = (Z + conj Z[N−k]) / 2, S = (Z − conj Z[N−k]) / 2i.
                let (mr, mi) = (0.5 * (zr + wr), 0.5 * (zi + wi));
                let (sr, si) = (0.5 * (zi - wi), -0.5 * (zr - wr));
                pm += mr * mr + mi * mi;
                ps += sr * sr + si * si;
            }
            out.mid[k] = pm * self.norm;
            out.side[k] = ps * self.norm;
        }
        self.spectra.push(out);
    }
}

impl Ear {
    /// The column range that samples `from..to` of the stream fall in (whole columns:
    /// a range starts and ends on the 10 ms grid, as a meter does).
    fn column_range(&self, from: u64, to: u64) -> (usize, usize) {
        let n = self.columns.len();
        let c0 = ((from / self.col_len as u64) as usize).min(n);
        let c1 = (to.div_ceil(self.col_len as u64) as usize).clamp(c0, n);
        (c0, c1)
    }

    /// The spectrum frames whose centre lies in samples `from..to` (the one nearest the
    /// range's middle when none does).
    fn frame_range(&self, from: u64, to: u64) -> (usize, usize) {
        let n = self.spectra.len();
        if n == 0 {
            return (0, 0);
        }
        let centre = |j: usize| (j * HOP + FRAME / 2) as u64;
        let j0 = (0..n).find(|&j| centre(j) >= from).unwrap_or(n);
        let j1 = (j0..n).find(|&j| centre(j) >= to).unwrap_or(n);
        if j1 > j0 {
            return (j0, j1);
        }
        let mid = (from + to) / 2;
        let j = ((mid.saturating_sub((FRAME / 2) as u64)) / HOP as u64) as usize;
        let j = j.min(n - 1);
        (j, j + 1)
    }

    /// Samples `from..to` of the stream, read (see the module notes).
    pub fn reading(&self, from: u64, to: u64) -> Reading {
        let (c0, c1) = self.column_range(from, to.min(self.frames));
        let cols = &self.columns[c0..c1];
        let mut loud = ColumnLoudness::new();
        let mut mono = ColumnLoudness::new();
        let mut t = Column::default();
        for c in cols {
            loud.push(c.k);
            mono.push(c.k_mono);
            t.sq += c.sq;
            t.lr += c.lr;
            t.ll += c.ll;
            t.rr += c.rr;
            t.mid += c.mid;
            t.side += c.side;
            t.true_peak = t.true_peak.max(c.true_peak);
            t.peak = t.peak.max(c.peak);
        }
        let n = cols.len().max(1) as f64;
        let integrated = loud.integrated();
        let finite = |x: f64| x.is_finite().then_some(x);
        let rms_dbfs = db(t.sq / n / 2.0);
        let sample_peak_dbfs = db_amp(t.peak);
        let corr = |lr: f64, ll: f64, rr: f64, n: f64| {
            // Both sides heard over −80 dBFS on average, or there is nothing to correlate.
            (ll / n > 1e-8 && rr / n > 1e-8).then(|| (lr / (ll * rr).sqrt()) as f32)
        };
        let samples = n * self.col_len as f64;
        let window = ((CORRELATION_WINDOW_S * 100.0) as usize).max(1);
        // Whole windows only (review 1: a last scrap of a few columns read as a window);
        // a range shorter than one window is its own.
        let whole = cols.len() < window;
        let correlation_min = cols
            .chunks(window)
            .filter(|w| whole || w.len() == window)
            .filter_map(|w| {
                let (lr, ll, rr) = w.iter().fold((0.0, 0.0, 0.0), |(a, b, c), x| {
                    (a + x.lr, b + x.ll, c + x.rr)
                });
                corr(lr, ll, rr, (w.len() * self.col_len) as f64)
            })
            .reduce(f32::min);
        let (j0, j1) = self.frame_range(from, to);
        let mut mid = [0f64; THIRDS];
        let mut side = [0f64; THIRDS];
        for f in &self.spectra[j0..j1] {
            for (m, x) in mid.iter_mut().zip(&f.mid) {
                *m += *x as f64;
            }
            for (m, x) in side.iter_mut().zip(&f.side) {
                *m += *x as f64;
            }
        }
        let frames = (j1 - j0).max(1) as f64;
        let thirds_db = std::array::from_fn(|k| db(mid[k] / frames));
        let octave = |p: &[f64; THIRDS], o: usize| {
            let k = 2 + 3 * o;
            db((p[k - 1] + p[k] + p[(k + 1).min(THIRDS - 1)]) / frames)
        };
        Reading {
            seconds: cols.len() as f64 * self.col_len as f64 / self.sr as f64,
            integrated_lufs: integrated,
            range_lu: loud.range(),
            short_term_max_lufs: finite(loud.max_short_term),
            momentary_max_lufs: finite(loud.max_momentary),
            true_peak_dbtp: db_amp(t.true_peak),
            sample_peak_dbfs,
            rms_dbfs,
            crest_db: sample_peak_dbfs - rms_dbfs,
            correlation: corr(t.lr, t.ll, t.rr, samples),
            correlation_min,
            width_db: match (t.mid > 0.0, t.side > 0.0) {
                (true, _) => db(t.side / t.mid),
                (false, true) => f32::INFINITY,
                (false, false) => f32::NEG_INFINITY,
            },
            mono_loss_lu: match (mono.integrated(), integrated) {
                (Some(m), Some(s)) => Some(m - s),
                _ => None,
            },
            thirds_db,
            octaves_db: std::array::from_fn(|o| octave(&mid, o)),
            side_octaves_db: std::array::from_fn(|o| octave(&side, o)),
        }
    }

    /// The whole stream, read.
    pub fn whole(&self) -> Reading {
        self.reading(0, self.frames)
    }

    /// The sharpest corner within `radius` samples of sample `at` ([`Column::sharp`], to
    /// the 10 ms column): a click shows as one much sharper than the music around it.
    pub fn sharpness_near(&self, at: u64, radius: u64) -> f32 {
        let (c0, c1) = self.column_range(at.saturating_sub(radius), at + radius + 1);
        self.columns[c0..c1]
            .iter()
            .map(|c| c.sharp)
            .fold(0f32, f32::max)
    }

    /// The mean mid power per third-octave band over the spectrum frames of samples
    /// `from..to`, linear — what the masking matrix compares, frame by frame
    /// ([`Self::spectra`]) or on average.
    pub fn band_power(&self, from: u64, to: u64) -> [f32; THIRDS] {
        let (j0, j1) = self.frame_range(from, to);
        let mut p = [0f64; THIRDS];
        for f in &self.spectra[j0..j1] {
            for (m, x) in p.iter_mut().zip(&f.mid) {
                *m += *x as f64;
            }
        }
        let n = (j1 - j0).max(1) as f64;
        std::array::from_fn(|k| (p[k] / n) as f32)
    }
}

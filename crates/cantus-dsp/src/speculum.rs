//! Speculum: the analyser's measurements (ADR 0056) — spectrum, spectrogram, oscilloscope,
//! vectorscope, correlation, loudness, peak and RMS — from a stereo stream.
//!
//! Speculum the effect is a pure pass-through: it changes no sample. What it shows is
//! measured here, **off the audio thread** (on the editor's), from every stereo frame of
//! its own slot as the engine copies them out (`cantus_engine::fx::speculum`). Everything
//! is counted in samples of that stream, never in frames of the screen, so what it reads
//! does not depend on how often it is drawn:
//!
//! - **Loudness** is the house meter (`terminus::loudness::LoudnessMeter`, BS.1770-4 /
//!   R128, conformance-tested): momentary and short-term as they come, the integrated and
//!   the range worked out at their own rate — every 200 ms of audio — and held between.
//! - **The spectrum** is a Hann-windowed FFT every quarter of its length (1 024 … 16 384
//!   points), scaled so a full-scale sine reads 0 dB, averaged over a time constant and
//!   with a peak hold that falls 6 dB a second, tilted by a slope around 1 kHz. A sine
//!   reads its level at its frequency (a parabola through the three bins at the top).
//! - **The spectrogram** keeps a row of 128 log-spaced bands for every analysis.
//! - **The oscilloscope** keeps five seconds of the mid signal and cuts a window free, from
//!   a rising zero crossing, or **on the beat**: with the song's position known for the
//!   newest sample, the window is the last whole stretch of `scope_beats` beats.
//! - **The vectorscope** keeps the last 4 096 frames as mid and side; **the correlation**
//!   and the RMS are exponential over 300 ms, corrected for their start.
//!
//! A sample that is not a number is read as silence: nothing it touches is poisoned.

use crate::aequalis::fft::Fft;
use crate::dynfx_meter::Hold;
use crate::math;
use crate::terminus::loudness::LoudnessMeter;

/// The FFT sizes, by a parameter's index.
pub const FFT_SIZES: [usize; 5] = [1_024, 2_048, 4_096, 8_192, 16_384];
const MAX_FFT: usize = 16_384;
/// The spectrogram: bands a row, rows kept.
pub const SPECTRO_BANDS: usize = 128;
pub const SPECTRO_ROWS: usize = 256;
/// The spectrogram's and the display's range, Hz.
pub const LO_HZ: f32 = 20.0;
pub const HI_HZ: f32 = 20_000.0;
/// The oscilloscope's memory (5.5 s at 48 kHz).
pub const SCOPE_LEN: usize = 1 << 18;
/// The vectorscope's frames.
pub const VECTOR_LEN: usize = 4_096;
/// The correlation's and the RMS's time constant, ms.
const AVG_MS: f32 = 300.0;
/// Integrated loudness and range are worked out this often, seconds of audio.
const LOUDNESS_EVERY: f32 = 0.2;
/// The peak hold's fall, dB a second.
const HOLD_FALL_DB: f32 = 6.0;

/// What the spectrum reads.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum Channel {
    /// (L + R) / 2.
    #[default]
    Mid,
    /// (L − R) / 2.
    Side,
    Left,
    Right,
    /// Left and right, two curves.
    Both,
}

impl Channel {
    pub fn from_index(i: f32) -> Channel {
        match i.round() as i32 {
            1 => Channel::Side,
            2 => Channel::Left,
            3 => Channel::Right,
            4 => Channel::Both,
            _ => Channel::Mid,
        }
    }

    /// Curves the spectrum draws.
    pub fn curves(self) -> usize {
        if self == Channel::Both {
            2
        } else {
            1
        }
    }
}

/// Where the oscilloscope's window starts.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum Sync {
    /// The last stretch, as it comes.
    Free,
    /// At a rising zero crossing.
    Trigger,
    /// On the beat (a rising crossing while the song is stopped).
    #[default]
    Beat,
}

impl Sync {
    pub fn from_index(i: f32) -> Sync {
        match i.round() as i32 {
            0 => Sync::Free,
            1 => Sync::Trigger,
            _ => Sync::Beat,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct SpeculumSettings {
    pub fft_size: usize,
    /// The spectrum's averaging time constant, ms (0: none).
    pub avg_ms: f32,
    /// dB an octave, around 1 kHz.
    pub slope_db: f32,
    pub hold: bool,
    pub channel: Channel,
    pub sync: Sync,
    /// The beat-synced window, beats.
    pub scope_beats: f64,
    /// The free and triggered window, ms.
    pub scope_ms: f32,
    /// The loudness the editor marks, LUFS.
    pub target_lufs: f32,
    /// The display's floor, dB.
    pub floor_db: f32,
}

impl Default for SpeculumSettings {
    fn default() -> Self {
        SpeculumSettings {
            fft_size: 8_192,
            avg_ms: 250.0,
            slope_db: 4.5,
            hold: true,
            channel: Channel::Mid,
            sync: Sync::Beat,
            scope_beats: 1.0,
            scope_ms: 20.0,
            target_lufs: -14.0,
            floor_db: -96.0,
        }
    }
}

/// The loudness readings.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Loudness {
    pub momentary: f64,
    pub short_term: f64,
    pub integrated: Option<f64>,
    pub range: Option<f64>,
    /// The largest true peak since the meter was reset, dBTP.
    pub true_peak_db: f32,
    /// Seconds measured.
    pub seconds: f64,
}

/// Peak and RMS per channel.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Levels {
    pub true_peak_db: f32,
    /// Held a second, dBFS.
    pub peak_db: [f32; 2],
    pub rms_db: [f32; 2],
}

/// The spectrogram band a frequency falls in.
pub fn band_of_hz(hz: f32) -> usize {
    let t = math::ln(hz.max(LO_HZ) / LO_HZ) / math::ln(HI_HZ / LO_HZ);
    ((t * SPECTRO_BANDS as f32) as usize).min(SPECTRO_BANDS - 1)
}

/// A band's lower edge, Hz.
pub fn band_hz(b: usize) -> f32 {
    LO_HZ * math::powf(HI_HZ / LO_HZ, b as f32 / SPECTRO_BANDS as f32)
}

fn db(p: f32) -> f32 {
    if p > 1e-20 {
        10.0 * math::log10(p)
    } else {
        -200.0
    }
}

#[inline]
fn sane(x: f32) -> f32 {
    if x.is_finite() {
        x
    } else {
        0.0
    }
}

/// The measurements of one stream.
#[derive(Clone, Debug)]
pub struct Analysis {
    sr: f32,
    set: SpeculumSettings,
    // ---- loudness ----
    loud: LoudnessMeter,
    scratch: [Vec<f32>; 2],
    integrated: Option<f64>,
    range: Option<f64>,
    loud_due: usize,
    loud_updates: u64,
    // ---- spectrum ----
    fft: Fft,
    win: Vec<f32>,
    norm: f32,
    hist: [Vec<f32>; 2],
    hpos: usize,
    filled: usize,
    since: usize,
    re: Vec<f32>,
    im: Vec<f32>,
    avg: [Vec<f32>; 2],
    peak: [Vec<f32>; 2],
    analysed: u64,
    spectro: Vec<f32>,
    rows: u64,
    // ---- scope and vectorscope ----
    scope: Vec<f32>,
    count: u64,
    last_beat: Option<f64>,
    spb: f64,
    vec: Vec<(f32, f32)>,
    // ---- correlation, peak, RMS ----
    c: [f64; 3],
    ms: [f64; 2],
    weight: f64,
    peaks: [Hold; 2],
}

impl Analysis {
    /// Everything allocated here, once.
    pub fn new(sample_rate: f32) -> Analysis {
        let mut a = Analysis {
            sr: sample_rate,
            set: SpeculumSettings::default(),
            loud: LoudnessMeter::new(sample_rate),
            scratch: [Vec::with_capacity(1_024), Vec::with_capacity(1_024)],
            integrated: None,
            range: None,
            loud_due: 0,
            loud_updates: 0,
            fft: Fft::new(MAX_FFT),
            win: vec![0.0; MAX_FFT],
            norm: 1.0,
            hist: [vec![0.0; MAX_FFT], vec![0.0; MAX_FFT]],
            hpos: 0,
            filled: 0,
            since: 0,
            re: vec![0.0; MAX_FFT],
            im: vec![0.0; MAX_FFT],
            avg: [vec![0.0; MAX_FFT / 2 + 1], vec![0.0; MAX_FFT / 2 + 1]],
            peak: [vec![0.0; MAX_FFT / 2 + 1], vec![0.0; MAX_FFT / 2 + 1]],
            analysed: 0,
            spectro: vec![-200.0; SPECTRO_BANDS * SPECTRO_ROWS],
            rows: 0,
            scope: vec![0.0; SCOPE_LEN],
            count: 0,
            last_beat: None,
            spb: 0.0,
            vec: vec![(0.0, 0.0); VECTOR_LEN],
            c: [0.0; 3],
            ms: [0.0; 2],
            weight: 0.0,
            peaks: [Hold::default(); 2],
        };
        a.window();
        a
    }

    pub fn settings(&self) -> &SpeculumSettings {
        &self.set
    }

    pub fn sample_rate(&self) -> f32 {
        self.sr
    }

    /// New settings: a new FFT size starts its averages over.
    pub fn set(&mut self, s: &SpeculumSettings) {
        let size = FFT_SIZES
            .iter()
            .copied()
            .find(|n| *n >= s.fft_size)
            .unwrap_or(MAX_FFT);
        let resize = size != self.set.fft_size;
        let rechannel = s.channel != self.set.channel;
        self.set = *s;
        self.set.fft_size = size;
        if resize {
            self.window();
        }
        if resize || rechannel {
            for v in self.avg.iter_mut().chain(self.peak.iter_mut()) {
                v.fill(0.0);
            }
            self.analysed = 0;
            self.since = 0;
        }
    }

    fn window(&mut self) {
        let n = self.set.fft_size;
        let mut sum = 0.0f64;
        for (k, w) in self.win.iter_mut().enumerate().take(n) {
            let v = 0.5 - 0.5 * math::cos(std::f64::consts::TAU * k as f64 / n as f64);
            *w = v as f32;
            sum += v;
        }
        // A full-scale sine at a bin's centre reads 0 dB: |X| = A·ΣW/2.
        self.norm = math::powi(2.0 / sum, 2) as f32;
    }

    /// The loudness meter, its integrated reading and its range, and its true peak, from
    /// the start again.
    pub fn reset_loudness(&mut self) {
        self.loud = LoudnessMeter::new(self.sr);
        self.integrated = None;
        self.range = None;
        self.loud_due = 0;
    }

    /// Frames of the stream, oldest first; `clock`: the song's position of the newest one
    /// in quarter notes and the tempo, while it plays.
    pub fn push(&mut self, l: &[f32], r: &[f32], clock: Option<(f64, f64)>) {
        let n = l.len().min(r.len());
        let a = 1.0 - math::exp(-1.0 / (f64::from(AVG_MS) * 0.001 * f64::from(self.sr)));
        let every = (LOUDNESS_EVERY * self.sr) as usize;
        let mask = MAX_FFT - 1;
        let hop = (self.set.fft_size / 4).max(256);
        let (mut pk_l, mut pk_r) = (0f32, 0f32);
        for i in 0..n {
            let (x, y) = (sane(l[i]), sane(r[i]));
            self.scratch[0].push(x);
            self.scratch[1].push(y);
            if self.scratch[0].len() == self.scratch[0].capacity() {
                self.flush_loudness();
            }
            self.hist[0][self.hpos] = x;
            self.hist[1][self.hpos] = y;
            self.hpos = (self.hpos + 1) & mask;
            self.filled = (self.filled + 1).min(MAX_FFT);
            self.since += 1;
            if self.since >= hop && self.filled >= self.set.fft_size {
                self.since = 0;
                self.analyse();
            }
            self.scope[(self.count as usize) % SCOPE_LEN] = 0.5 * (x + y);
            let h = std::f32::consts::FRAC_1_SQRT_2;
            self.vec[(self.count as usize) % VECTOR_LEN] = ((x + y) * h, (x - y) * h);
            self.count += 1;
            let (xd, yd) = (f64::from(x), f64::from(y));
            self.c[0] += (xd * yd - self.c[0]) * a;
            self.c[1] += (xd * xd - self.c[1]) * a;
            self.c[2] += (yd * yd - self.c[2]) * a;
            self.ms[0] += (xd * xd - self.ms[0]) * a;
            self.ms[1] += (yd * yd - self.ms[1]) * a;
            self.weight += (1.0 - self.weight) * a;
            pk_l = pk_l.max(x.abs());
            pk_r = pk_r.max(y.abs());
            self.loud_due += 1;
            if self.loud_due >= every {
                self.loud_due = 0;
                self.flush_loudness();
                self.integrated = self.loud.integrated();
                self.range = self.loud.range();
                self.loud_updates += 1;
            }
        }
        self.flush_loudness();
        let window = self.sr as u32;
        self.peaks[0].push(pk_l, n, window);
        self.peaks[1].push(pk_r, n, window);
        match clock {
            Some((beat, tempo)) if tempo > 0.0 => {
                self.last_beat = Some(beat);
                self.spb = 60.0 / tempo * f64::from(self.sr);
            }
            _ => self.last_beat = None,
        }
    }

    fn flush_loudness(&mut self) {
        if !self.scratch[0].is_empty() {
            self.loud.process(&self.scratch[0], &self.scratch[1]);
            self.scratch[0].clear();
            self.scratch[1].clear();
        }
    }

    /// One analysis of the newest `fft_size` frames.
    fn analyse(&mut self) {
        let n = self.set.fft_size;
        let mask = MAX_FFT - 1;
        let start = (self.hpos + MAX_FFT - n) & mask;
        let hop = (n / 4).max(256) as f32;
        let alpha = if self.set.avg_ms <= 0.0 {
            1.0
        } else {
            1.0 - math::exp(-hop / (self.set.avg_ms * 0.001 * self.sr))
        };
        let fall = math::powf(10f32, -HOLD_FALL_DB * hop / self.sr / 10.0);
        let curves = self.set.channel.curves();
        for which in 0..curves {
            for k in 0..n {
                let j = (start + k) & mask;
                let (l, r) = (self.hist[0][j], self.hist[1][j]);
                let s = match (self.set.channel, which) {
                    (Channel::Mid, _) => 0.5 * (l + r),
                    (Channel::Side, _) => 0.5 * (l - r),
                    (Channel::Left, _) | (Channel::Both, 0) => l,
                    _ => r,
                };
                self.re[k] = s * self.win[k];
                self.im[k] = 0.0;
            }
            self.fft.run(&mut self.re[..n], &mut self.im[..n], false);
            let first = self.analysed == 0;
            for b in 0..=n / 2 {
                let p = (self.re[b] * self.re[b] + self.im[b] * self.im[b]) * self.norm;
                let avg = &mut self.avg[which][b];
                *avg = if first { p } else { *avg + (p - *avg) * alpha };
                let pk = &mut self.peak[which][b];
                *pk = (*pk * fall).max(p);
            }
            if which == 0 {
                self.spectrogram_add(n);
            }
        }
        self.analysed += 1;
    }

    /// A row of the spectrogram from the analysis just made (in `re`/`im`).
    fn spectrogram_add(&mut self, n: usize) {
        let row = (self.rows as usize) % SPECTRO_ROWS;
        let bin_hz = self.sr / n as f32;
        for b in 0..SPECTRO_BANDS {
            let (f0, f1) = (band_hz(b), band_hz(b + 1));
            let k0 = ((f0 / bin_hz) as usize).min(n / 2);
            let k1 = ((f1 / bin_hz).ceil() as usize).clamp(k0 + 1, n / 2 + 1);
            let mut m = 0f32;
            for k in k0..k1 {
                m = m.max((self.re[k] * self.re[k] + self.im[k] * self.im[k]) * self.norm);
            }
            let centre = (f0 * f1).sqrt();
            self.spectro[row * SPECTRO_BANDS + b] = db(m) + self.tilt(centre);
        }
        self.rows += 1;
    }

    fn tilt(&self, hz: f32) -> f32 {
        if self.set.slope_db == 0.0 {
            0.0
        } else {
            self.set.slope_db * math::log2(hz.max(1.0) / 1_000.0)
        }
    }

    /// The power read at a frequency from a spectrum: the top of a parabola (in dB) through
    /// the three bins around a peak there, else the bins' level between.
    fn read(&self, spec: &[f32], hz: f32) -> f32 {
        let n = self.set.fft_size;
        let bin = (hz / self.sr * n as f32).clamp(1.0, (n / 2 - 1) as f32);
        let k0 = bin.floor() as usize;
        let m = if spec[k0 + 1] > spec[k0] { k0 + 1 } else { k0 };
        let m = m.clamp(1, n / 2 - 1);
        let (a, b, c) = (db(spec[m - 1]), db(spec[m]), db(spec[m + 1]));
        if b >= a && b >= c && (bin - m as f32).abs() <= 1.0 {
            let den = a - 2.0 * b + c;
            if den < 0.0 {
                let d = 0.5 * (a - c) / den;
                return b - 0.25 * (a - c) * d;
            }
            return b;
        }
        let t = bin - k0 as f32;
        db(spec[k0]) * (1.0 - t) + db(spec[k0 + 1]) * t
    }

    /// The averaged spectrum at `hz`, dB with the slope, for curve `which` (0, or 1 for the
    /// right with *Both*).
    pub fn spectrum_db(&self, which: usize, hz: f32) -> f32 {
        if self.analysed == 0 {
            return -200.0;
        }
        self.read(&self.avg[which.min(1)], hz) + self.tilt(hz)
    }

    /// The held peaks at `hz`, dB with the slope.
    pub fn peak_db(&self, which: usize, hz: f32) -> f32 {
        if self.analysed == 0 {
            return -200.0;
        }
        self.read(&self.peak[which.min(1)], hz) + self.tilt(hz)
    }

    /// The loudest averaged level between two frequencies, dB with the slope: what a
    /// display column that spans many bins draws.
    pub fn spectrum_span_db(&self, which: usize, f0: f32, f1: f32, held: bool) -> f32 {
        if self.analysed == 0 {
            return -200.0;
        }
        let n = self.set.fft_size;
        let spec = if held {
            &self.peak[which.min(1)]
        } else {
            &self.avg[which.min(1)]
        };
        let k0 = (f0 / self.sr * n as f32) as usize;
        let k1 = ((f1 / self.sr * n as f32).ceil() as usize).min(n / 2);
        if k1 <= k0 + 1 {
            let mid = 0.5 * (f0 + f1);
            return self.read(spec, mid) + self.tilt(mid);
        }
        let m = spec[k0..k1].iter().fold(0f32, |m, v| m.max(*v));
        db(m) + self.tilt((f0 * f1).sqrt())
    }

    /// Analyses made: new data since the editor looked.
    pub fn analyses(&self) -> u64 {
        self.analysed
    }

    pub fn spectrogram_rows(&self) -> u64 {
        self.rows
    }

    /// The row `ago` rows back from the newest, dB per band.
    pub fn spectrogram_row(&self, ago: usize) -> &[f32] {
        let r = (self.rows as usize + SPECTRO_ROWS - 1 - ago % SPECTRO_ROWS) % SPECTRO_ROWS;
        &self.spectro[r * SPECTRO_BANDS..(r + 1) * SPECTRO_BANDS]
    }

    pub fn loudness(&self) -> Loudness {
        Loudness {
            momentary: self.loud.momentary(),
            short_term: self.loud.short_term(),
            integrated: self.integrated,
            range: self.range,
            true_peak_db: self.loud.true_peak_db(),
            seconds: self.loud.columns.seconds(),
        }
    }

    /// How many times integrated loudness and range were worked out.
    pub fn loudness_updates(&self) -> u64 {
        self.loud_updates
    }

    pub fn levels(&self) -> Levels {
        let rms = |m: f64| {
            if self.weight > 0.0 && m > 0.0 {
                (10.0 * math::log10(m / self.weight)) as f32
            } else {
                -200.0
            }
        };
        let pk = |v: f32| {
            if v > 0.0 {
                20.0 * math::log10(v)
            } else {
                -200.0
            }
        };
        Levels {
            true_peak_db: self.loud.true_peak_db().max(-200.0),
            peak_db: [pk(self.peaks[0].get()), pk(self.peaks[1].get())],
            rms_db: [rms(self.ms[0]), rms(self.ms[1])],
        }
    }

    /// The phase correlation over the last 300 ms, −1 … 1; `None` in silence.
    pub fn correlation(&self) -> Option<f32> {
        let d = self.c[1] * self.c[2];
        (d > 1e-18).then(|| (self.c[0] / d.sqrt()).clamp(-1.0, 1.0) as f32)
    }

    /// The oscilloscope's window as `lo.len()` columns, the lowest and highest sample in
    /// each; false when there is not enough yet.
    pub fn scope_columns(&self, lo: &mut [f32], hi: &mut [f32]) -> bool {
        let cols = lo.len().min(hi.len());
        if cols == 0 || self.count == 0 {
            return false;
        }
        let newest = self.count - 1;
        let free_len = ((self.set.scope_ms * 0.001 * self.sr) as u64).max(cols as u64);
        let beat = match (self.set.sync, self.last_beat) {
            (Sync::Beat, Some(b)) if self.spb > 0.0 && self.set.scope_beats > 0.0 => {
                let len_b = self.set.scope_beats;
                let k = (b / len_b).floor() - 1.0;
                let back = (b - k * len_b) * self.spb;
                let len = (len_b * self.spb).round() as u64;
                let back = back.round() as u64;
                (back <= newest && len > 0).then(|| (newest - back, len))
            }
            _ => None,
        };
        let (start, len) = match beat {
            Some(w) => w,
            None => {
                let len = free_len;
                if len > newest + 1 {
                    return false;
                }
                let mut start = newest + 1 - len;
                if self.set.sync != Sync::Free {
                    // The latest rising crossing at or before the start, within half the
                    // memory.
                    let floor = (newest + 1).saturating_sub(SCOPE_LEN as u64 / 2).max(1);
                    let mut i = start;
                    while i > floor {
                        let (a, b) = (self.at(i - 1), self.at(i));
                        if a < 0.0 && b >= 0.0 {
                            start = i;
                            break;
                        }
                        i -= 1;
                    }
                }
                (start, len)
            }
        };
        if newest + 1 - start > SCOPE_LEN as u64 {
            return false;
        }
        for j in 0..cols {
            let a = start + len * j as u64 / cols as u64;
            let b = (start + len * (j as u64 + 1) / cols as u64).max(a + 1);
            let (mut mn, mut mx) = (f32::INFINITY, f32::NEG_INFINITY);
            for i in a..b.min(newest + 1) {
                let v = self.at(i);
                mn = mn.min(v);
                mx = mx.max(v);
            }
            if mn > mx {
                mn = 0.0;
                mx = 0.0;
            }
            lo[j] = mn;
            hi[j] = mx;
        }
        true
    }

    fn at(&self, i: u64) -> f32 {
        self.scope[(i as usize) % SCOPE_LEN]
    }

    /// Whether the oscilloscope is on the beat now (synced and the song's position known).
    pub fn on_beat(&self) -> bool {
        self.set.sync == Sync::Beat && self.last_beat.is_some()
    }

    /// The vectorscope's frames as (mid, side), oldest first.
    pub fn vector_points(&self) -> impl Iterator<Item = (f32, f32)> + '_ {
        let n = (self.count as usize).min(VECTOR_LEN);
        let start = self.count as usize - n;
        (start..start + n).map(move |i| self.vec[i % VECTOR_LEN])
    }

    /// Frames measured in all.
    pub fn frames(&self) -> u64 {
        self.count
    }
}

#[cfg(test)]
#[path = "speculum_tests.rs"]
mod tests;

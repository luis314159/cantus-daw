//! What Comparatio measures of one source (the mix, or the reference), sample by sample on
//! the audio thread, allocating nothing after [`Analyser::new`]:
//!
//! - **loudness** — Terminus's BS.1770-4 / EBU R128 meter ([`LoudnessMeter`]), the same
//!   code the master's meter, the export and Terminus's editor read (one code path): the
//!   momentary, short-term and integrated loudness with both gates, the loudness range
//!   (EBU Tech 3342) and the 4× true peak;
//! - **dynamics** — the crest factor (sample peak over RMS), the PLR (the largest true peak
//!   over the integrated loudness), the PSR (the largest true peak of the last three
//!   seconds over the short-term loudness, each 100 ms, now and averaged), and a histogram
//!   of the short-term loudness in 1 LU bins;
//! - **stereo** — correlation and width, whole and per band ([`super::stereo`]).
//!
//! ## The grid
//!
//! Everything steps on the loudness meter's own grid: 10 ms columns counted from the
//! start of the measurement, and a 100 ms step every ten columns. The analyser feeds the
//! meter slices that end on column boundaries, so it sees each column close and each step
//! end **at its sample**, whatever the block size — and every reading is the same at every
//! block size, to the bit.
//!
//! ## What the dynamics are measured over
//!
//! - **Crest factor**: the sample peak over the RMS of the 100 ms steps above −70 dBFS
//!   (unweighted, both channels): silence before, between and after does not lower the
//!   RMS. A sine reads 3.01 dB, a square 0 dB.
//! - **PLR** (peak to loudness ratio): `true peak max − integrated loudness`, LU.
//! - **PSR** (peak to short-term loudness ratio): `max true peak of the last 3 s − short-term
//!   loudness`, LU, every 100 ms once a short-term value exists; "mean" is the average over
//!   the steps whose short-term loudness passes the absolute gate.
//!
//! The true peak per step is the meter's own (it is read and cleared at each step's end);
//! its 4× detector's delay (17 samples) shifts which step a peak lands in by at most that.

use super::stereo::{self, Stereo};
use crate::math;
use crate::terminus::loudness::{column_len, LoudnessMeter, ABSOLUTE_GATE, STEP_COLUMNS};

/// Short-term window, in 100 ms steps.
pub const SHORT_STEPS: usize = 30;
/// Columns in the short-term window (3 s).
const SHORT_COLUMNS: u64 = 300;
/// The histogram's lowest bin's floor, LUFS; one bin per LU.
pub const HIST_LO: f64 = -60.0;
/// Bins of the short-term histogram (−60 … 0 LUFS); values outside land in the end bins.
pub const HIST_BINS: usize = 60;
/// The relative gate of the "whole window" test ([`Analyser::whole_window`]), LU under the
/// integrated loudness (the loudness range's gate).
pub const QUIET_LU: f64 = -20.0;
/// −70 dBFS as a mean square: the steps under it do not count for the crest factor.
const CREST_GATE: f64 = 1e-7;

/// One source's readings (`None`, or −∞ for the loudness, until a value exists).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Readings {
    pub momentary: f64,
    pub short_term: f64,
    pub integrated: Option<f64>,
    /// Loudness range, LU (EBU Tech 3342).
    pub range: Option<f64>,
    pub max_momentary: f64,
    pub max_short_term: f64,
    /// The largest true peak, dBTP (−∞ for silence).
    pub true_peak_db: f64,
    /// The largest sample, dBFS.
    pub sample_peak_db: f64,
    pub crest_db: Option<f64>,
    pub plr: Option<f64>,
    pub psr: Option<f64>,
    pub psr_mean: Option<f64>,
    /// Whole, lows, mids, highs, over the whole measurement.
    pub stereo: [stereo::Band; stereo::BANDS],
    /// The same over the last three seconds.
    pub stereo_recent: [stereo::Band; stereo::BANDS],
    /// Seconds measured.
    pub seconds: f64,
}

fn db(linear: f64) -> f64 {
    if linear > 0.0 {
        20.0 * math::log10(linear)
    } else {
        f64::NEG_INFINITY
    }
}

/// One source's analyser.
#[derive(Clone, Debug)]
pub struct Analyser {
    meter: LoudnessMeter,
    stereo: Stereo,
    col_len: usize,
    /// Samples into the column in progress.
    col_pos: usize,
    /// Columns since the measurement began.
    columns: u64,
    /// The step in progress: unweighted peak and sum of squares (both channels).
    step_sq: f64,
    step_n: u64,
    peak: f32,
    true_peak: f32,
    crest_sq: f64,
    crest_n: u64,
    tp_ring: [f32; SHORT_STEPS],
    tp_pos: usize,
    psr: Option<f64>,
    psr_sum: f64,
    psr_n: u64,
    hist: [u32; HIST_BINS],
    /// Steps in a row whose momentary loudness passed the gates.
    loud_steps: u64,
}

impl Analyser {
    pub fn new(sample_rate: f32) -> Self {
        Analyser {
            meter: LoudnessMeter::new(sample_rate),
            stereo: Stereo::new(sample_rate),
            col_len: column_len(sample_rate),
            col_pos: 0,
            columns: 0,
            step_sq: 0.0,
            step_n: 0,
            peak: 0.0,
            true_peak: 0.0,
            crest_sq: 0.0,
            crest_n: 0,
            tp_ring: [0.0; SHORT_STEPS],
            tp_pos: 0,
            psr: None,
            psr_sum: 0.0,
            psr_n: 0,
            hist: [0; HIST_BINS],
            loud_steps: 0,
        }
    }

    /// 100 ms steps closed since the measurement began.
    pub fn steps(&self) -> u64 {
        self.columns / STEP_COLUMNS as u64
    }

    /// Samples until the step in progress ends (1 … a step).
    pub fn to_step(&self) -> usize {
        let cols_left = STEP_COLUMNS - (self.columns % STEP_COLUMNS as u64) as usize;
        cols_left * self.col_len - self.col_pos
    }

    pub fn process(&mut self, left: &[f32], right: &[f32]) {
        let n = left.len().min(right.len());
        let mut i = 0;
        while i < n {
            let take = (self.col_len - self.col_pos).min(n - i);
            let (l, r) = (&left[i..i + take], &right[i..i + take]);
            self.meter.process(l, r);
            self.stereo.process(l, r);
            // Summed sample by sample into the step's own sum: the same additions in the
            // same order whatever the pieces.
            let mut sq = self.step_sq;
            let mut pk = self.peak;
            for (&a, &b) in l.iter().zip(r) {
                sq += a as f64 * a as f64 + b as f64 * b as f64;
                pk = pk.max(a.abs()).max(b.abs());
            }
            self.peak = pk;
            self.step_sq = sq;
            self.step_n += 2 * take as u64;
            self.col_pos += take;
            i += take;
            if self.col_pos == self.col_len {
                self.col_pos = 0;
                self.columns += 1;
                if self.columns.is_multiple_of(STEP_COLUMNS as u64) {
                    self.end_step();
                }
            }
        }
    }

    fn end_step(&mut self) {
        let tp = self.meter.max_true_peak;
        self.meter.max_true_peak = 0.0;
        self.true_peak = self.true_peak.max(tp);
        self.tp_ring[self.tp_pos] = tp;
        self.tp_pos = (self.tp_pos + 1) % SHORT_STEPS;
        if self.step_n > 0 && self.step_sq / self.step_n as f64 > CREST_GATE {
            self.crest_sq += self.step_sq;
            self.crest_n += self.step_n;
        }
        self.step_sq = 0.0;
        self.step_n = 0;
        self.stereo.end_step();
        let mom = self.meter.momentary();
        let passes = mom > ABSOLUTE_GATE
            && self.meter.integrated().is_none_or(|i| mom > i + QUIET_LU)
            && self.columns >= 40;
        self.loud_steps = if passes { self.loud_steps + 1 } else { 0 };
        self.psr = None;
        if self.columns >= SHORT_COLUMNS {
            let s = self.meter.short_term();
            if s > ABSOLUTE_GATE {
                let bin = ((s - HIST_LO).floor().max(0.0) as usize).min(HIST_BINS - 1);
                self.hist[bin] += 1;
                let tp_max = self.tp_ring.iter().fold(0f32, |m, v| m.max(*v));
                let psr = db(tp_max as f64) - s;
                self.psr = Some(psr);
                self.psr_sum += psr;
                self.psr_n += 1;
            }
        }
    }

    /// What it reads now.
    pub fn readings(&self) -> Readings {
        let integrated = self.meter.integrated();
        let tp_db = db(self.true_peak as f64);
        let crest = (self.crest_n > 0 && self.peak > 0.0).then(|| {
            let rms = (self.crest_sq / self.crest_n as f64).sqrt();
            db(self.peak as f64) - db(rms)
        });
        Readings {
            momentary: self.meter.momentary(),
            short_term: self.meter.short_term(),
            integrated,
            range: self.meter.range(),
            max_momentary: self.meter.columns.max_momentary,
            max_short_term: self.meter.columns.max_short_term,
            true_peak_db: tp_db,
            sample_peak_db: db(self.peak as f64),
            crest_db: crest,
            plr: integrated.filter(|_| self.true_peak > 0.0).map(|i| tp_db - i),
            psr: self.psr,
            psr_mean: (self.psr_n > 0).then(|| self.psr_sum / self.psr_n as f64),
            stereo: self.stereo.total(),
            stereo_recent: self.stereo.recent(),
            seconds: self.columns as f64 / 100.0,
        }
    }

    /// The short-term loudness every 100 ms since the measurement began, in 1 LU bins from
    /// [`HIST_LO`] (bin `i` holds `HIST_LO + i ≤ S < HIST_LO + i + 1`).
    pub fn histogram(&self) -> &[u32; HIST_BINS] {
        &self.hist
    }

    /// The short-term window is whole: three seconds measured, and every momentary value
    /// in them above the absolute gate and within 20 LU of the integrated loudness — a
    /// window that holds no quiet passage, not even the edge of one.
    pub fn whole_window(&self) -> bool {
        self.columns >= SHORT_COLUMNS && self.loud_steps >= SHORT_STEPS as u64
    }

    /// The short-term loudness, LUFS (−∞ until 3 s are in).
    pub fn short_term(&self) -> f64 {
        self.meter.short_term()
    }

    pub fn integrated(&self) -> Option<f64> {
        self.meter.integrated()
    }

    /// Start a new measurement here: every statistic empty; the filters, the momentary
    /// and short-term readings run on (BS.1770's blocks begin at the measurement's start).
    pub fn restart(&mut self) {
        self.meter.restart_measurement();
        self.stereo.restart();
        self.col_pos = 0;
        self.columns = 0;
        self.step_sq = 0.0;
        self.step_n = 0;
        self.peak = 0.0;
        self.true_peak = 0.0;
        self.meter.max_true_peak = 0.0;
        self.crest_sq = 0.0;
        self.crest_n = 0;
        self.tp_ring = [0.0; SHORT_STEPS];
        self.tp_pos = 0;
        self.psr = None;
        self.psr_sum = 0.0;
        self.psr_n = 0;
        self.hist = [0; HIST_BINS];
        self.loud_steps = 0;
    }

    /// As new, allocating nothing.
    pub fn reset(&mut self) {
        self.meter.reset();
        self.stereo.reset();
        self.restart();
    }

    /// Nothing that holds sound: the K-weighting, the true peak's interpolator and the
    /// band filters are empty (the statistics are readings, not sound).
    pub fn is_clear(&self) -> bool {
        self.meter.is_clear() && self.stereo.is_clear()
    }
}

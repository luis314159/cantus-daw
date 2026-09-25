//! Sibilus: the de-esser.
//!
//! A compressor whose detector hears only the band the sibilance lives in, and — for a
//! single voice — only what in that band *is* sibilance. It turns down the whole signal
//! (wide band) or only the band (split), with lookahead, stereo link, mid/side, an external
//! key, and two ways to hear what it does: the **trigger** (what the detector hears) and
//! the **removed** signal (the delta).
//!
//! ## Signal flow
//!
//! ```text
//! key ─► band (HP lo, LP hi, 24 dB/oct) ─► level ───────────┐
//!   └──► classifier (share × flatness) ─► confidence ───────┤ (Single vocal only)
//!                                                            ▼
//!            curve (Comprimo's, ∞:1, 6 dB knee, capped at Range) × confidence
//!                                  ─► link ─► attack / release ─► gain g
//! input ─► delay (lookahead [+ FIR centre]) ─► y = x_d + (g − 1)·band(x_d)
//! ```
//!
//! `band` is the input itself in wide band, and a linear-phase FIR in the split modes
//! (`sibilus_split.rs` has why). The detector runs `lookahead` ahead of the audio, so the
//! gain is already down when the first sample of an "s" arrives.
//!
//! ## The null-test contract
//!
//! - A reduction below 1e-6 dB is exactly zero, and at zero the output **is** the input
//!   delayed by [`Sibilus::latency`] — the sum above is not even computed. At lookahead 0
//!   in wide band the latency is 0, so it is the input itself, to the bit.
//! - Everything runs per sample; the classifier updates every [`UPDATE`] samples and the
//!   band edges glide every [`GLIDE`] samples, both counted from the start of the stream,
//!   so the output does not depend on the block size.
//! - Every filter state and average is flushed to zero below 1e-20, so a long silence
//!   costs no denormals and ends in exact zeros.

use crate::comprimo::{gain_reduction_db, CompSettings};

#[path = "sibilus_detect.rs"]
mod detect;
#[path = "sibilus_split.rs"]
mod split;

pub use detect::{classify, Features, FEATURE_MS, LEVEL_MS, SUB_BANDS};
pub use split::fir_len;

use detect::{BandDesign, Coefs2, Detector};
use split::{Fir, Line};

/// Samples between classifier updates.
pub const UPDATE: usize = 16;
/// Samples between steps of a band edge's glide.
pub const GLIDE: usize = 64;
/// The band edges' glide time constant, ms.
const GLIDE_MS: f32 = 30.0;
/// The longest lookahead, ms.
pub const MAX_LOOKAHEAD_MS: f32 = 15.0;
/// The knee of the de-esser's curve, dB, centred on the threshold.
pub const KNEE_DB: f32 = 6.0;
/// The curve's ratio: Comprimo's "∞". The band is held at the threshold, down to Range.
pub const RATIO: f32 = 100.0;
/// Columns of history the editor can read back (see [`Sibilus::column`]).
pub const HISTORY: usize = 9;
/// Reduction above which a stretch counts as a de-essing event, and below which it ends.
pub const EVENT_ON_DB: f32 = 1.0;
pub const EVENT_OFF_DB: f32 = 0.25;
/// The band's limits, Hz.
pub const MIN_HZ: f32 = 1_000.0;
pub const MAX_HZ: f32 = 20_000.0;

/// What the detector listens for.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Detection {
    /// One voice: the classifier decides what is sibilance, whatever its level.
    Voice,
    /// Anything in the band over the threshold: mixes, drums, an HF limiter.
    Allround,
}

/// What is turned down.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Processing {
    /// The whole signal.
    Wide,
    /// Everything above the band's low edge.
    SplitHigh,
    /// Only the band, low edge to high edge.
    SplitBand,
}

/// Which part of the stereo image is detected and processed.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum StereoMode {
    /// Left and right, linked by `link`.
    Stereo,
    /// Mid and side, each on its own detector, linked by `link`.
    MidSide,
    /// Only the mid; the side passes untouched.
    Mid,
    /// Only the side; the mid passes untouched.
    Side,
}

/// What comes out.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Listen {
    Normal,
    /// The key through the detection band: what the detector hears.
    Trigger,
    /// What is taken away: the input minus the output.
    Removed,
}

macro_rules! indexed {
    ($t:ident, [$($v:ident),+]) => {
        impl $t {
            pub const ALL: &'static [$t] = &[$($t::$v),+];
            pub fn from_index(i: usize) -> $t {
                Self::ALL[i.min(Self::ALL.len() - 1)]
            }
            pub fn index(self) -> usize {
                Self::ALL.iter().position(|x| *x == self).unwrap_or(0)
            }
        }
    };
}
indexed!(Detection, [Voice, Allround]);
indexed!(Processing, [Wide, SplitHigh, SplitBand]);
indexed!(StereoMode, [Stereo, MidSide, Mid, Side]);
indexed!(Listen, [Normal, Trigger, Removed]);

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct DeessSettings {
    pub detection: Detection,
    pub processing: Processing,
    /// The band level above which it reduces, dB (a full-scale sine in the band is 0).
    pub threshold_db: f32,
    /// The most it takes away, dB.
    pub range_db: f32,
    /// The detection band, Hz. Given in either order.
    pub lo_hz: f32,
    pub hi_hz: f32,
    pub attack_ms: f32,
    pub release_ms: f32,
    /// 0 … [`MAX_LOOKAHEAD_MS`]; rounded to whole samples.
    pub lookahead_ms: f32,
    /// 0 = each channel on its own, 1 = both take the larger reduction.
    pub link: f32,
    pub stereo: StereoMode,
    pub listen: Listen,
    /// The classifier's sensitivity, 0 … 1 (0.5 is the middle). Single vocal only.
    pub sensitivity: f32,
}

impl Default for DeessSettings {
    fn default() -> Self {
        DeessSettings {
            detection: Detection::Voice,
            processing: Processing::Wide,
            threshold_db: -30.0,
            range_db: 6.0,
            lo_hz: 5_000.0,
            hi_hz: 12_000.0,
            attack_ms: 1.0,
            release_ms: 40.0,
            lookahead_ms: 5.0,
            link: 1.0,
            stereo: StereoMode::Stereo,
            listen: Listen::Normal,
            sensitivity: 0.5,
        }
    }
}

impl DeessSettings {
    /// The curve as Comprimo's: ∞:1 above a soft knee, capped at Range.
    fn curve(&self) -> CompSettings {
        CompSettings {
            threshold_db: self.threshold_db,
            ratio: RATIO,
            knee_db: KNEE_DB,
            range_db: self.range_db.max(0.0),
            ..CompSettings::default()
        }
    }
}

/// How much a band level is reduced, before the classifier and the envelope: the static
/// curve the editor draws.
pub fn curve_db(level_db: f32, s: &DeessSettings) -> f32 {
    gain_reduction_db(level_db, &s.curve())
}

/// The band's edges as used: ordered, inside the audio band, at least a sixth of an octave
/// apart.
pub fn band_edges(lo: f32, hi: f32, sr: f32) -> (f32, f32) {
    let top = MAX_HZ.min(sr * 0.49);
    let (a, b) = if lo <= hi { (lo, hi) } else { (hi, lo) };
    let a = if a.is_finite() { a } else { 5_000.0 };
    let b = if b.is_finite() { b } else { 12_000.0 };
    // `max` then `min`, not `clamp`: at a sample rate so low that the top is under the
    // lowest edge (a host may ask for 1 kHz), the band collapses to the top instead of
    // panicking.
    let a = a.max(MIN_HZ).min(top / 2f32.powf(1.0 / 6.0));
    let b = b.max(a * 2f32.powf(1.0 / 6.0)).min(top);
    (a, b)
}

/// The latency a set of settings will have at a sample rate: lookahead, plus the FIR's
/// centre in the split modes. What the host can compute without the unit.
pub fn latency_for(s: &DeessSettings, sr: f32) -> usize {
    let look = (s.lookahead_ms.clamp(0.0, MAX_LOOKAHEAD_MS) * 0.001 * sr).round() as usize;
    let fir = match s.processing {
        Processing::Wide => 0,
        _ => fir_len(sr) / 2,
    };
    look + fir
}

/// Samples per column of history at a sample rate: about 11 ms.
pub fn chunk_for(sr: f32) -> usize {
    512 * ((sr / 48_000.0).round() as usize).max(1)
}

/// One column of the editor's history: a stretch of [`chunk_for`] samples, measured where
/// the audio leaves (so the level, the detector and the reduction line up).
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Column {
    /// Peak of the input, linear, both channels.
    pub input: f32,
    /// Peak of the processed output, linear (what Normal plays, whatever is listened to).
    pub output: f32,
    /// Highest detector level, dB.
    pub det_db: f32,
    /// Largest reduction in force, dB.
    pub gr_db: f32,
    /// Highest classifier confidence, 0 … 1 (1 throughout in Allround).
    pub conf: f32,
    /// The trigger's frequency from its zero crossings, Hz: the centre of what the band
    /// hears.
    pub hz: f32,
}

/// De-essing events: stretches where it took away more than [`EVENT_ON_DB`].
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct EventStats {
    pub count: u32,
    pub mean_gr_db: f32,
    pub max_gr_db: f32,
    /// The last event's centre frequency (zero crossings of the trigger), Hz.
    pub last_hz: f32,
    pub last_gr_db: f32,
}

#[derive(Copy, Clone, Debug, Default)]
struct Event {
    on: bool,
    peak_gr: f32,
    /// The loudest the detector was during the event, dB: the centre frequency is
    /// measured only while it is within 12 dB of that, not over the release tail.
    peak_det: f32,
    zc: u32,
    len: u32,
    /// The sum of every finished event's peak, for the mean.
    sum_gr: f64,
}

#[derive(Clone, Debug)]
pub struct Sibilus {
    sr: f32,
    s: DeessSettings,
    /// The band edges in force: they glide toward the settings'.
    lo: f32,
    hi: f32,
    k: Coefs2,
    det: [Detector; 2],
    fir: Fir,
    /// The input, L and R.
    audio: [Line; 2],
    /// The band signal of each detection channel, for Trigger and the zero crossings.
    trig: [Line; 2],
    /// Reduction per detection channel (dB), detector level (dB), confidence.
    ctl: [Line; 4],
    /// Reduction per detection channel, dB, as computed now.
    gr: [f32; 2],
    /// Samples since reset.
    t: u64,
    curve: CompSettings,
    att: f32,
    rel: f32,
    glide: f32,
    look: usize,
    centre: usize,
    chunk: usize,
    acc: Column,
    acc_zc: u32,
    acc_prev: f32,
    ring: [Column; HISTORY],
    seq: u64,
    ev: Event,
    stats: EventStats,
}

/// An empty column, ready to take maxima.
fn fresh() -> Column {
    Column {
        det_db: -120.0,
        ..Column::default()
    }
}

fn db(lin: f32) -> f32 {
    if lin > 1e-6 {
        20.0 * lin.log10()
    } else {
        -120.0
    }
}

impl Sibilus {
    pub fn new(sample_rate: f32) -> Self {
        let sr = sample_rate.max(1.0);
        let fir = Fir::new(sr);
        let cap = (MAX_LOOKAHEAD_MS * 0.001 * sr).ceil() as usize + fir.len() + 2;
        let line = || Line::new(cap);
        let mut d = Sibilus {
            sr,
            s: DeessSettings::default(),
            lo: 5_000.0,
            hi: 12_000.0,
            k: Coefs2::new(sr),
            det: [Detector::default(), Detector::default()],
            fir,
            audio: [line(), line()],
            trig: [line(), line()],
            ctl: [line(), line(), line(), line()],
            gr: [0.0; 2],
            t: 0,
            curve: CompSettings::default(),
            att: 0.0,
            rel: 0.0,
            glide: (-(GLIDE as f32) / (GLIDE_MS * 0.001 * sr)).exp(),
            look: 0,
            centre: 0,
            chunk: chunk_for(sr),
            acc: fresh(),
            acc_zc: 0,
            acc_prev: 0.0,
            ring: [Column::default(); HISTORY],
            seq: 0,
            ev: Event::default(),
            stats: EventStats::default(),
        };
        d.set(&DeessSettings::default());
        d.reset();
        d
    }

    pub fn sample_rate(&self) -> f32 {
        self.sr
    }

    pub fn settings(&self) -> &DeessSettings {
        &self.s
    }

    /// New settings. Band edges glide from where they are (snap before the first sample);
    /// everything else takes effect at once.
    pub fn set(&mut self, s: &DeessSettings) {
        let old_processing = self.s.processing;
        self.s = *s;
        let coef = |ms: f32| (-1.0 / (ms.max(0.01) * 0.001 * self.sr)).exp();
        self.curve = s.curve();
        self.att = coef(s.attack_ms);
        self.rel = coef(s.release_ms);
        self.look =
            (s.lookahead_ms.clamp(0.0, MAX_LOOKAHEAD_MS) * 0.001 * self.sr).round() as usize;
        self.centre = match s.processing {
            Processing::Wide => 0,
            _ => self.fir.centre(),
        };
        if self.t == 0 {
            self.snap_band();
        } else if old_processing != s.processing {
            self.fir.design(s.processing, self.sr, self.lo, self.hi);
        }
    }

    fn snap_band(&mut self) {
        let (lo, hi) = band_edges(self.s.lo_hz, self.s.hi_hz, self.sr);
        self.lo = lo;
        self.hi = hi;
        self.design_band();
    }

    fn design_band(&mut self) {
        let d = BandDesign::new(self.sr, self.lo, self.hi);
        self.det[0].set(&d);
        self.det[1].set(&d);
        self.fir
            .design(self.s.processing, self.sr, self.lo, self.hi);
    }

    pub fn reset(&mut self) {
        for l in self
            .audio
            .iter_mut()
            .chain(self.trig.iter_mut())
            .chain(self.ctl.iter_mut())
        {
            l.clear();
        }
        for d in &mut self.det {
            d.clear();
        }
        self.gr = [0.0; 2];
        self.t = 0;
        self.acc = fresh();
        self.acc_zc = 0;
        self.acc_prev = 0.0;
        self.ring = [Column::default(); HISTORY];
        self.seq = 0;
        self.ev = Event::default();
        self.stats = EventStats::default();
        self.snap_band();
    }

    /// Samples of delay this adds: lookahead, plus the split FIR's centre.
    pub fn latency(&self) -> usize {
        self.look + self.centre
    }

    /// The band edges in force (they glide toward the settings').
    pub fn band(&self) -> (f32, f32) {
        (self.lo, self.hi)
    }

    /// The reduction in force now, dB, per detection channel (L/R or M/S).
    pub fn reduction_db(&self) -> [f32; 2] {
        self.gr
    }

    /// What the classifier concluded last, per detection channel.
    pub fn features(&self) -> [Features; 2] {
        [self.det[0].features, self.det[1].features]
    }

    /// Columns completed since reset.
    pub fn seq(&self) -> u64 {
        self.seq
    }

    /// Samples per column.
    pub fn chunk(&self) -> usize {
        self.chunk
    }

    /// Column `index` (0 = the first since reset), if it is still kept: the last
    /// [`HISTORY`] are.
    pub fn column(&self, index: u64) -> Option<Column> {
        (index < self.seq && self.seq - index <= HISTORY as u64)
            .then(|| self.ring[(index % HISTORY as u64) as usize])
    }

    pub fn events(&self) -> EventStats {
        self.stats
    }

    /// The split's amplitude at `hz` for the band in force: 1 where it cuts fully, 0
    /// where it leaves alone. 1 everywhere in wide band.
    pub fn split_amplitude(&self, hz: f32) -> f32 {
        match self.s.processing {
            Processing::Wide => 1.0,
            _ => self.fir.amplitude(self.sr, hz),
        }
    }

    /// Process a stereo block in place. `key` is the external sidechain, when routed.
    pub fn process(&mut self, left: &mut [f32], right: &mut [f32], key: Option<(&[f32], &[f32])>) {
        let s = self.s;
        let voice = s.detection == Detection::Voice;
        let active = [s.stereo != StereoMode::Side, s.stereo != StereoMode::Mid];
        let ms = s.stereo != StereoMode::Stereo;
        let link = s.link.clamp(0.0, 1.0);
        let (tlo, thi) = band_edges(s.lo_hz, s.hi_hz, self.sr);
        let n = left.len().min(right.len());
        let nfir = self.fir.len();
        for i in 0..n {
            // Band edges glide on a fixed grid.
            if self.t % GLIDE as u64 == 0 && (self.lo != tlo || self.hi != thi) {
                let glide = self.glide;
                let step = |cur: f32, target: f32| {
                    let r = (cur / target).ln();
                    if r.abs() < 1e-3 {
                        target
                    } else {
                        target * (r * glide).exp()
                    }
                };
                self.lo = step(self.lo, tlo);
                self.hi = step(self.hi, thi);
                self.design_band();
            }
            let (xl, xr) = (left[i], right[i]);
            let (kl, kr) = match key {
                Some((a, b)) => (
                    a.get(i).copied().unwrap_or(0.0),
                    b.get(i).copied().unwrap_or(0.0),
                ),
                None => (xl, xr),
            };
            let keyed = if ms {
                [0.5 * (kl + kr), 0.5 * (kl - kr)]
            } else {
                [kl, kr]
            };

            // Detection, per channel.
            let update = self.t % UPDATE as u64 == 0;
            let mut band = [0f32; 2];
            let mut target = [0f32; 2];
            let mut det_db = -120f32;
            let mut conf = 0f32;
            for ch in 0..2 {
                if !active[ch] {
                    continue;
                }
                let (b, level) = self.det[ch].tick(keyed[ch], &self.k, voice);
                if voice && update {
                    self.det[ch].classify(s.sensitivity);
                }
                band[ch] = b;
                let ldb = db(level);
                let c = if voice {
                    self.det[ch].features.confidence
                } else {
                    1.0
                };
                target[ch] = c * gain_reduction_db(ldb, &self.curve);
                det_db = det_db.max(ldb);
                conf = conf.max(c);
            }
            // Link: each side moves toward the larger reduction.
            let loud = target[0].max(target[1]);
            for (t, on) in target.iter_mut().zip(active) {
                if on {
                    *t += (loud - *t) * link;
                }
            }
            // The envelope, in dB.
            let (att, rel) = (self.att, self.rel);
            for (gr, tgt) in self.gr.iter_mut().zip(target) {
                let a = if tgt > *gr { att } else { rel };
                let next = tgt + (*gr - tgt) * a;
                *gr = if next < 1e-6 { 0.0 } else { next };
            }
            self.ctl[0].push(self.gr[0]);
            self.ctl[1].push(self.gr[1]);
            self.ctl[2].push(det_db);
            self.ctl[3].push(conf);
            self.audio[0].push(xl);
            self.audio[1].push(xr);
            self.trig[0].push(band[0]);
            self.trig[1].push(band[1]);

            // The audio, `latency` behind; the reduction computed `centre` ago applies
            // now, so the detector is exactly `look` ahead of what it acts on.
            let late = self.look + self.centre;
            let gd = [self.ctl[0].at(self.centre), self.ctl[1].at(self.centre)];
            let xd = [self.audio[0].at(late), self.audio[1].at(late)];
            let (yl, yr) = if gd[0] == 0.0 && gd[1] == 0.0 {
                (xd[0], xd[1])
            } else {
                let g = gd.map(|d| {
                    if d == 0.0 {
                        0.0
                    } else {
                        10f32.powf(-d / 20.0) - 1.0
                    }
                });
                let b = match s.processing {
                    Processing::Wide => xd,
                    _ => [
                        self.fir.apply(self.audio[0].window(self.look, nfir)),
                        self.fir.apply(self.audio[1].window(self.look, nfir)),
                    ],
                };
                if ms {
                    let dm = g[0] * 0.5 * (b[0] + b[1]);
                    let ds = g[1] * 0.5 * (b[0] - b[1]);
                    (xd[0] + dm + ds, xd[1] + dm - ds)
                } else {
                    (xd[0] + g[0] * b[0], xd[1] + g[1] * b[1])
                }
            };
            let tr = [self.trig[0].at(late), self.trig[1].at(late)];
            let (ol, or) = match s.listen {
                Listen::Normal => (yl, yr),
                Listen::Removed => (xd[0] - yl, xd[1] - yr),
                Listen::Trigger => match s.stereo {
                    StereoMode::Stereo => (tr[0], tr[1]),
                    StereoMode::MidSide => (tr[0] + tr[1], tr[0] - tr[1]),
                    StereoMode::Mid => (tr[0], tr[0]),
                    StereoMode::Side => (tr[1], -tr[1]),
                },
            };
            left[i] = ol;
            right[i] = or;

            // What the editor draws, measured where the audio leaves.
            let gr_now = gd[0].max(gd[1]);
            let z = match s.stereo {
                StereoMode::Stereo => tr[0] + tr[1],
                StereoMode::Side => tr[1],
                _ => tr[0],
            };
            let crossed = (z >= 0.0) != (self.acc_prev >= 0.0);
            self.acc_prev = z;
            self.acc_zc += crossed as u32;
            let det_now = self.ctl[2].at(late);
            let a = &mut self.acc;
            a.input = a.input.max(xd[0].abs()).max(xd[1].abs());
            a.output = a.output.max(yl.abs()).max(yr.abs());
            a.det_db = a.det_db.max(det_now);
            a.gr_db = a.gr_db.max(gr_now);
            a.conf = a.conf.max(self.ctl[3].at(late));
            self.track_event(gr_now, det_now, crossed);
            self.t += 1;
            if self.t % self.chunk as u64 == 0 {
                let mut col = self.acc;
                col.hz = self.acc_zc as f32 * self.sr / (2.0 * self.chunk as f32);
                self.ring[(self.seq % HISTORY as u64) as usize] = col;
                self.seq += 1;
                self.acc = fresh();
                self.acc_zc = 0;
            }
        }
    }

    fn track_event(&mut self, gr: f32, det: f32, crossed: bool) {
        let ev = &mut self.ev;
        if !ev.on {
            if gr > EVENT_ON_DB {
                *ev = Event {
                    on: true,
                    peak_gr: gr,
                    peak_det: det,
                    zc: 0,
                    len: 0,
                    sum_gr: ev.sum_gr,
                };
            } else {
                return;
            }
        }
        ev.peak_gr = ev.peak_gr.max(gr);
        ev.peak_det = ev.peak_det.max(det);
        if det > ev.peak_det - 12.0 {
            ev.len += 1;
            ev.zc += crossed as u32;
        }
        if gr < EVENT_OFF_DB {
            ev.on = false;
            ev.sum_gr += ev.peak_gr as f64;
            let st = &mut self.stats;
            st.count += 1;
            st.max_gr_db = st.max_gr_db.max(ev.peak_gr);
            st.mean_gr_db = (ev.sum_gr / st.count as f64) as f32;
            st.last_gr_db = ev.peak_gr;
            st.last_hz = ev.zc as f32 * self.sr / (2.0 * ev.len.max(1) as f32);
        }
    }
}

#[cfg(test)]
#[path = "sibilus_tests.rs"]
mod tests;

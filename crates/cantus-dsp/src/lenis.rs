//! Lenis: the resonance suppressor (fx side round, wave B; DSP phase — the name is
//! provisional, [`NAME`]).
//!
//! It finds the narrow peaks that stick out of a sound's own smooth spectrum, moment to
//! moment — a ringing drum shell, a whistling amp, a voice's harsh 3 kHz — and turns each
//! down only while it rings, by as much as it sticks out. A dynamic equaliser with hundreds
//! of bands that place themselves.
//!
//! ## The signal path
//!
//! ```text
//!      ┌─────────────────────── ring (the dry, delayed by the latency) ──────────┬─ mix ─┬─ × out
//! in ──┤                                                                          │       │
//!      └─ every hop: frame ─ wa ─ FFT (L + jR) ─ detector(s) ─ gains ─ IFFT ─ ws ─┘  delta: dry − that
//! ```
//!
//! - **The transform** ([`stft`]): a 1024-point frame at 48 kHz (Standard; 512 Fast, 2048
//!   Precise, doubled at 88.2/96 kHz and again above) through an asymmetric low-delay window
//!   pair, a hop of `N/8`, both channels in one complex FFT. **Latency `N/2 − 1`**: 511
//!   samples (10.6 ms) at 48 kHz Standard, 255 Fast, 1023 Precise — fixed for a quality and
//!   rate, reported by [`Lenis::latency`].
//! - **The detector** ([`detect`]): each bin's averaged power in dB against the spectrum's
//!   own robust envelope; the excess over the threshold, through the curve, weighted by
//!   the focus, capped at the depth, spread and smoothed in time.
//! - **Channels**: Linked (one detector on both channels' power, the same gains on both),
//!   Independent (one each), Mid/Side (one on the mid, one on the side).
//!
//! ## The null-test contract
//!
//! - **A frame nobody turned down is not transformed back.** It adds its windowed input to
//!   the overlap straight away, and an output sample that no turned-down frame covers
//!   *is* the input delayed by the latency, to the bit — so at depth 0, at mix 0, on
//!   silence, or wherever nothing sticks out, Lenis is a pure delay of `latency()` samples.
//! - **On a grid.** Frames fall every hop counted from the start of the stream; settings
//!   given by [`Lenis::set`] wait for the next hop's first sample (the mix, the output and
//!   the delta switch then glide per sample, 10 ms). Nothing depends on the block size.
//! - **Exact rest.** Every average and reduction is flushed to +0.0 (−160 dBFS, and
//!   10⁻³ dB); after `N + 2M` zeros and that, the unit is at rest and [`Lenis::skip`] is
//!   exact. [`Lenis::reset`] keeps the stream's clock (the grid), so a reset twin and a
//!   processed one agree to the bit.
//! - **Delta** is `dry − (what the normal output would be before the output gain)`,
//!   computed as that subtraction, so at 0 dB out it is the input at the latency minus the
//!   output, to the bit.

#[path = "lenis_detect.rs"]
pub mod detect;
#[path = "lenis_stft.rs"]
pub mod stft;

#[cfg(test)]
#[path = "lenis_tests.rs"]
mod tests;
#[cfg(test)]
#[path = "lenis_tests_measure.rs"]
mod tests_measure;

use crate::aequalis::fft::Fft;
use crate::math;
use detect::{Design, Detector, Scratch};
use stft::Geometry;

/// The unit's name while it is provisional.
pub const NAME: &str = "Lenis";

pub const MIN_ATTACK_MS: f32 = 0.5;
pub const MAX_ATTACK_MS: f32 = 200.0;
pub const MIN_RELEASE_MS: f32 = 5.0;
pub const MAX_RELEASE_MS: f32 = 2000.0;
pub const MAX_OUT_DB: f32 = 24.0;
/// Focus bands.
pub const FOCUS_BANDS: usize = 6;
/// Points of the meter's spectrum, log-spaced from [`DISPLAY_LO_HZ`] to [`DISPLAY_HI_HZ`].
pub const DISPLAY: usize = 128;
pub const DISPLAY_LO_HZ: f32 = 20.0;
pub const DISPLAY_HI_HZ: f32 = 20_000.0;
/// The mix, output and delta glides, ms.
const GLIDE_MS: f32 = 10.0;

/// How the two channels are heard.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum Channels {
    /// One detector on both channels' power; the same reduction on both (the image holds).
    #[default]
    Linked,
    /// Each channel its own detector and reduction.
    Independent,
    /// One detector on the mid, one on the side.
    MidSide,
}

/// The curve.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum Mode {
    /// A knee (`e²/(e + 4 dB)` of the excess over the threshold): a peak just over it is
    /// barely touched, a big one almost as in Hard — gentle on what only nearly rings.
    #[default]
    Soft,
    /// The excess over the threshold, straight: at 50 % depth a peak is brought to the line.
    Hard,
}

/// The frame: resolution against latency.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum Quality {
    /// 512 points at 48 kHz: 5.3 ms, 94 Hz bins.
    Fast,
    /// 1024 points: 10.6 ms, 47 Hz bins.
    #[default]
    Standard,
    /// 2048 points: 21.3 ms, 23 Hz bins (the low mids resolved).
    Precise,
}

impl Quality {
    pub const ALL: [Quality; 3] = [Quality::Fast, Quality::Standard, Quality::Precise];

    /// The frame and synthesis half-length at `sr`: the 48 kHz size doubled at 88.2/96 kHz
    /// and again above, so the times stay.
    pub fn geometry(self, sr: f32) -> Geometry {
        let base = match self {
            Quality::Fast => 512,
            Quality::Standard => 1024,
            Quality::Precise => 2048,
        };
        let f = if sr <= 50_000.0 {
            1
        } else if sr <= 100_000.0 {
            2
        } else {
            4
        };
        Geometry::new(base * f, base * f / 4)
    }
}

/// One band of the focus curve.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct FocusBand {
    pub on: bool,
    pub hz: f32,
    /// −1 (left alone) … +1 (twice the reduction) at the centre.
    pub amount: f32,
    /// The width at half height, octaves (0.1 … 4).
    pub width_oct: f32,
}

impl Default for FocusBand {
    fn default() -> Self {
        FocusBand {
            on: false,
            hz: 1_000.0,
            amount: 0.0,
            width_oct: 1.0,
        }
    }
}

/// Lenis's settings in the units a mixing engineer reads. [`Default`] is the null
/// (depth 0).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct LenisSettings {
    /// How much: 0 (nothing) … 1. At 0.5 a resonance comes down to the line (the envelope
    /// plus the threshold) in Hard mode; at 1 in Soft mode, or twice as far in Hard.
    pub depth: f32,
    /// How narrow a peak must be to count: 0 (broad bumps too) … 1 (only narrow peaks).
    pub sharpness: f32,
    /// How far a peak must stand out before it is touched: 0 (2 dB) … 1 (8 dB).
    pub selectivity: f32,
    pub attack_ms: f32,
    pub release_ms: f32,
    /// The focus's limits: nothing under `low_hz` or over `high_hz` (a third-octave fade).
    pub low_hz: f32,
    pub high_hz: f32,
    pub bands: [FocusBand; FOCUS_BANDS],
    pub channels: Channels,
    pub mode: Mode,
    pub quality: Quality,
    /// Hear what is taken away.
    pub delta: bool,
    /// Dry/wet, 0 … 1.
    pub mix: f32,
    pub out_db: f32,
}

impl Default for LenisSettings {
    fn default() -> Self {
        LenisSettings {
            depth: 0.0,
            sharpness: 0.5,
            selectivity: 0.5,
            attack_ms: 5.0,
            release_ms: 80.0,
            low_hz: 20.0,
            high_hz: 20_000.0,
            bands: [FocusBand::default(); FOCUS_BANDS],
            channels: Channels::Linked,
            mode: Mode::Soft,
            quality: Quality::Standard,
            delta: false,
            mix: 1.0,
            out_db: 0.0,
        }
    }
}

/// The latency `s` gives at `sr`, samples.
pub fn latency_for(s: &LenisSettings, sr: f32) -> usize {
    s.quality.geometry(sr).latency()
}

/// What the editor draws, refreshed every frame.
#[derive(Clone, Debug, PartialEq)]
pub struct LenisMeter {
    /// The input's spectrum as the detectors hear it (the louder detector), dBFS per point.
    pub level_db: [f32; DISPLAY],
    /// The reduction, dB ≥ 0, per point (the most over the detectors and the point's bins).
    pub reduction_db: [f32; DISPLAY],
    /// The most any bin is turned down now, dB.
    pub max_reduction_db: f32,
    /// Frames analysed since the start (the editor sees when it moved).
    pub frames: u64,
}

impl Default for LenisMeter {
    fn default() -> Self {
        LenisMeter {
            level_db: [detect::FLOOR_DB; DISPLAY],
            reduction_db: [0.0; DISPLAY],
            max_reduction_db: 0.0,
            frames: 0,
        }
    }
}

/// The meter's point `p`'s frequency, Hz.
pub fn display_hz(p: usize) -> f32 {
    let t = p as f32 / (DISPLAY - 1) as f32;
    DISPLAY_LO_HZ * math::powf(DISPLAY_HI_HZ / DISPLAY_LO_HZ, t)
}

/// A glide toward a target, per sample, landing exactly.
#[derive(Copy, Clone, Debug)]
struct Glide {
    v: f32,
    t: f32,
}

impl Glide {
    fn at(v: f32) -> Glide {
        Glide { v, t: v }
    }

    #[inline]
    fn step(&mut self, k: f32) -> f32 {
        if self.v != self.t {
            self.v += (self.t - self.v) * k;
            if (self.v - self.t).abs() < 1e-6 {
                self.v = self.t;
            }
        }
        self.v
    }

    fn still(&self) -> bool {
        self.v == self.t
    }
}

/// The resonance suppressor.
pub struct Lenis {
    sr: f32,
    fft: Fft,
    g: Geometry,
    /// The settings in force, and those waiting for the next hop.
    s: LenisSettings,
    pending: Option<LenisSettings>,
    design: Design,
    det: [Detector; 2],
    scratch: Scratch,
    wa: Vec<f32>,
    ws: Vec<f32>,
    wp: Vec<f32>,
    wsum: f64,
    /// The input, both channels, `g.n` long (allocated for the largest frame).
    ring: [Vec<f32>; 2],
    /// The overlap, `2M` long.
    acc: [Vec<f32>; 2],
    re: Vec<f32>,
    im: Vec<f32>,
    pa: Vec<f32>,
    pb: Vec<f32>,
    /// The meter's bins for each point.
    disp: Vec<(u32, u32)>,
    meter: LenisMeter,
    /// Samples since the start of the stream (the grid).
    pos: u64,
    /// Exact zeros in a row at the input (both channels); `u64::MAX` for a ring never
    /// written.
    zero_run: u64,
    /// Output samples still covered by a frame that was turned down.
    wet_left: usize,
    mix: Glide,
    out: Glide,
    delta: Glide,
    k_glide: f32,
}

impl Lenis {
    /// A unit at `sample_rate`, sized for its largest quality. Allocates here only.
    pub fn new(sample_rate: f32) -> Lenis {
        let sr = sample_rate.max(8_000.0);
        let big = Quality::Precise.geometry(sr);
        let (nmax, bmax) = (big.n, big.bins());
        let s = LenisSettings::default();
        let g = s.quality.geometry(sr);
        let mut l = Lenis {
            sr,
            fft: Fft::new(nmax),
            g,
            s,
            pending: None,
            design: Design::new(bmax),
            det: [Detector::new(bmax), Detector::new(bmax)],
            scratch: Scratch::new(bmax),
            wa: vec![0.0; nmax],
            ws: vec![0.0; nmax],
            wp: vec![0.0; nmax],
            wsum: 1.0,
            ring: [vec![0.0; nmax], vec![0.0; nmax]],
            acc: [vec![0.0; nmax], vec![0.0; nmax]],
            re: vec![0.0; nmax],
            im: vec![0.0; nmax],
            pa: vec![0.0; bmax],
            pb: vec![0.0; bmax],
            disp: vec![(0, 0); DISPLAY],
            meter: LenisMeter::default(),
            pos: 0,
            zero_run: u64::MAX,
            wet_left: 0,
            mix: Glide::at(1.0),
            out: Glide::at(1.0),
            delta: Glide::at(0.0),
            k_glide: 1.0 - math::exp(-1.0 / (GLIDE_MS * 1e-3 * sr)),
        };
        l.shape(g);
        l.apply(s, true);
        l
    }

    /// A unit with a frame shape of its own (the measurements compare window pairs).
    #[cfg(test)]
    pub(crate) fn with_geometry(sample_rate: f32, n: usize, m: usize) -> Lenis {
        let mut l = Lenis::new(sample_rate);
        l.shape(Geometry::new(n, m));
        l.design.build(&l.s, l.sr, l.g.n, l.g.hop(), l.wsum);
        l
    }

    /// [`Lenis::set`] for a unit made by [`Lenis::with_geometry`]: the settings without
    /// their quality's shape.
    #[cfg(test)]
    pub(crate) fn set_keeping_shape(&mut self, s: &LenisSettings) {
        self.s = *s;
        self.design.build(s, self.sr, self.g.n, self.g.hop(), self.wsum);
        self.mix = Glide::at(s.mix);
        self.out = Glide::at(math::db_to_gain(s.out_db));
        self.delta = Glide::at(if s.delta { 1.0 } else { 0.0 });
    }

    pub fn sample_rate(&self) -> f32 {
        self.sr
    }

    /// The settings in force (a pending change shows from the next hop).
    pub fn settings(&self) -> &LenisSettings {
        self.pending.as_ref().unwrap_or(&self.s)
    }

    /// New settings, taken at the next hop's first sample. A new unit (nothing processed
    /// yet) takes them at once.
    pub fn set(&mut self, s: &LenisSettings) {
        if self.pos == 0 && self.zero_run == u64::MAX {
            self.pending = None;
            self.apply(*s, true);
        } else if *s == self.s {
            self.pending = None;
        } else {
            self.pending = Some(*s);
        }
    }

    /// Samples from the input to the output (both the processed and the dry path).
    pub fn latency(&self) -> usize {
        self.g.latency()
    }

    pub fn geometry(&self) -> Geometry {
        self.g
    }

    /// Bin `k`'s frequency, Hz.
    pub fn bin_hz(&self, k: usize) -> f32 {
        k as f32 * self.sr / self.g.n as f32
    }

    /// Detector `i`'s reduction per bin, dB (`i` = 0: linked, left or mid; 1: right or
    /// side). Bins `0..=n/2`.
    pub fn reduction_bins(&self, i: usize) -> &[f32] {
        &self.det[i.min(1)].reduction[..self.g.bins()]
    }

    pub fn meter(&self) -> &LenisMeter {
        &self.meter
    }

    /// Back to silence, the settings kept (a pending change taken now); the stream's clock
    /// is kept, so the frame grid stays where it was.
    pub fn reset(&mut self) {
        let s = self.pending.take().unwrap_or(self.s);
        self.apply(s, true);
    }

    /// At rest: silence in gives silence out and changes nothing, so [`Lenis::skip`] may
    /// stand for processing it.
    pub fn at_rest(&self) -> bool {
        self.rest_blocker().is_none()
    }

    /// What keeps the unit from rest, if anything.
    pub fn rest_blocker(&self) -> Option<&'static str> {
        let need = (self.g.n + self.g.span()) as u64;
        if self.zero_run < need {
            Some("input ring or overlap")
        } else if !self.det.iter().all(Detector::quiet) {
            Some("detector average or reduction")
        } else if self.wet_left != 0 {
            Some("turned-down frame")
        } else if self.pending.is_some() {
            Some("pending settings")
        } else if !(self.mix.still() && self.out.still() && self.delta.still()) {
            Some("glide")
        } else {
            None
        }
    }

    /// Stand for processing `frames` samples of silence at rest.
    pub fn skip(&mut self, frames: usize) {
        debug_assert!(self.at_rest());
        self.pos += frames as u64;
        self.zero_run = self.zero_run.saturating_add(frames as u64);
    }

    /// A new frame shape: the windows and the meter's bins (no allocation: the vectors are
    /// sized for the largest).
    fn shape(&mut self, g: Geometry) {
        self.g = g;
        self.wsum = stft::windows(g, &mut self.wa, &mut self.ws, &mut self.wp);
        let bin_hz = self.sr / g.n as f32;
        let last = g.bins() - 1;
        let step = math::powf(DISPLAY_HI_HZ / DISPLAY_LO_HZ, 1.0 / (DISPLAY - 1) as f32);
        let half = step.sqrt();
        for p in 0..DISPLAY {
            let f = display_hz(p);
            let lo = ((f / half / bin_hz).ceil() as usize).min(last);
            let hi = (((f * half / bin_hz).ceil() as usize).max(1) - 1).min(last);
            let near = ((f / bin_hz).round() as usize).min(last);
            self.disp[p] = if hi >= lo {
                (lo as u32, hi as u32)
            } else {
                (near as u32, near as u32)
            };
        }
        self.clear();
    }

    /// The audio state back to silence (the clock kept).
    fn clear(&mut self) {
        for r in self.ring.iter_mut().chain(self.acc.iter_mut()) {
            r.fill(0.0);
        }
        for d in self.det.iter_mut() {
            d.reset();
        }
        self.zero_run = u64::MAX;
        self.wet_left = 0;
        self.meter = LenisMeter {
            frames: self.meter.frames,
            ..LenisMeter::default()
        };
    }

    /// Put `s` in force. `cleared`: the audio state goes back to silence and the glides land.
    fn apply(&mut self, s: LenisSettings, cleared: bool) {
        let g = s.quality.geometry(self.sr);
        let reshaped = g != self.g;
        if reshaped {
            self.shape(g);
        } else if cleared {
            self.clear();
        }
        if s.channels != self.s.channels && !reshaped {
            // The detectors hear other signals now: they start again (the overlap, in left
            // and right, carries on).
            for d in self.det.iter_mut() {
                d.reset();
            }
        }
        self.s = s;
        self.design
            .build(&s, self.sr, self.g.n, self.g.hop(), self.wsum);
        self.mix.t = s.mix.clamp(0.0, 1.0);
        self.out.t = math::db_to_gain(s.out_db.clamp(-MAX_OUT_DB, MAX_OUT_DB));
        self.delta.t = if s.delta { 1.0 } else { 0.0 };
        if cleared || reshaped {
            self.mix.v = self.mix.t;
            self.out.v = self.out.t;
            self.delta.v = self.delta.t;
        }
    }
}

impl Lenis {
    /// Process a block in place (both channels; mono callers pass the same signal twice).
    pub fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        let len = left.len().min(right.len());
        let hop = self.g.hop() as u64;
        for i in 0..len {
            if self.pos % hop == 0 {
                if let Some(s) = self.pending.take() {
                    self.apply(s, false);
                }
            }
            let (n, span) = (self.g.n, self.g.span());
            let (xl, xr) = (left[i], right[i]);
            let w = (self.pos as usize) & (n - 1);
            self.ring[0][w] = xl;
            self.ring[1][w] = xr;
            self.zero_run = if xl.to_bits() == 0 && xr.to_bits() == 0 {
                self.zero_run.saturating_add(1)
            } else {
                0
            };
            if (self.pos + 1) % self.g.hop() as u64 == 0 {
                self.frame();
            }
            // The sample whose last frame is done: `2M − 1` back.
            let s = self.pos.wrapping_add(1).wrapping_sub(span as u64) as usize;
            let (ai, di) = (s & (span - 1), s & (n - 1));
            let (dl, dr) = (self.ring[0][di], self.ring[1][di]);
            let (wl, wr) = (self.acc[0][ai], self.acc[1][ai]);
            self.acc[0][ai] = 0.0;
            self.acc[1][ai] = 0.0;
            let mix = self.mix.step(self.k_glide);
            let out = self.out.step(self.k_glide);
            let delta = self.delta.step(self.k_glide);
            let (mut yl, mut yr) = if self.wet_left > 0 {
                self.wet_left -= 1;
                if mix == 1.0 {
                    (wl, wr)
                } else if mix == 0.0 {
                    (dl, dr)
                } else {
                    (dl + mix * (wl - dl), dr + mix * (wr - dr))
                }
            } else {
                (dl, dr)
            };
            if delta == 1.0 {
                yl = dl - yl;
                yr = dr - yr;
            } else if delta != 0.0 {
                yl += delta * ((dl - yl) - yl);
                yr += delta * ((dr - yr) - yr);
            }
            if out != 1.0 {
                yl *= out;
                yr *= out;
            }
            left[i] = yl;
            right[i] = yr;
            self.pos += 1;
        }
    }

    /// The frame that ends at `self.pos`.
    fn frame(&mut self) {
        let g = self.g;
        let (n, span) = (g.n, g.span());
        let bins = g.bins();
        let mask = n - 1;
        let base = (self.pos as usize).wrapping_add(1);
        self.meter.frames += 1;
        let silent = self.zero_run >= n as u64;
        let ms = self.s.channels == Channels::MidSide;
        if !silent {
            for i in 0..n {
                let j = base.wrapping_add(i) & mask;
                let (l, r) = (self.ring[0][j], self.ring[1][j]);
                let (a, b) = if ms {
                    (0.5 * (l + r), 0.5 * (l - r))
                } else {
                    (l, r)
                };
                self.re[i] = a * self.wa[i];
                self.im[i] = b * self.wa[i];
            }
            self.fft.run(&mut self.re[..n], &mut self.im[..n], false);
            for k in 0..bins {
                let (a, b, c, d) = stft::unpack(&self.re, &self.im, k, n);
                self.pa[k] = a * a + b * b;
                self.pb[k] = c * c + d * d;
            }
            if self.design.linked {
                for k in 0..bins {
                    self.pa[k] += self.pb[k];
                }
            }
        }
        // The detectors, and the meter from each.
        let detectors = if self.design.linked { 1 } else { 2 };
        let mut touched = false;
        self.meter.level_db.fill(detect::FLOOR_DB);
        self.meter.reduction_db.fill(0.0);
        self.meter.max_reduction_db = 0.0;
        for d in 0..detectors {
            let p = if silent {
                None
            } else if d == 0 {
                Some(&self.pa[..bins])
            } else {
                Some(&self.pb[..bins])
            };
            touched |= self.det[d].frame(p, &self.design, &mut self.scratch);
            let red = &self.det[d].reduction;
            for (q, &(lo, hi)) in self.disp.iter().enumerate() {
                let (lo, hi) = (lo as usize, hi as usize);
                let mut lv = detect::FLOOR_DB;
                let mut rd = 0.0f32;
                for k in lo..=hi {
                    lv = lv.max(self.scratch.level[k]);
                    rd = rd.max(red[k]);
                }
                self.meter.level_db[q] = self.meter.level_db[q].max(lv);
                self.meter.reduction_db[q] = self.meter.reduction_db[q].max(rd);
            }
            for &r in &red[..bins] {
                self.meter.max_reduction_db = self.meter.max_reduction_db.max(r);
            }
        }
        if silent {
            // An all-zero frame adds nothing (whatever its gains).
            return;
        }
        let first = n - span;
        if !touched {
            for i in first..n {
                let j = base.wrapping_add(i) & mask;
                let a = base.wrapping_add(i).wrapping_sub(n) & (span - 1);
                self.acc[0][a] += self.ring[0][j] * self.wp[i];
                self.acc[1][a] += self.ring[1][j] * self.wp[i];
            }
            return;
        }
        if self.design.linked {
            let gain = &self.det[0].gain;
            for k in 0..bins {
                let gk = gain[k];
                if gk != 1.0 {
                    let j = (n - k) & mask;
                    self.re[k] *= gk;
                    self.im[k] *= gk;
                    if j != k {
                        self.re[j] *= gk;
                        self.im[j] *= gk;
                    }
                }
            }
        } else {
            let (ga, gb) = (&self.det[0].gain, &self.det[1].gain);
            for k in 0..bins {
                let (a, b, c, d) = stft::unpack(&self.re, &self.im, k, n);
                stft::repack(
                    &mut self.re,
                    &mut self.im,
                    k,
                    n,
                    (a, b),
                    (c, d),
                    ga[k],
                    gb[k],
                );
            }
        }
        self.fft.run(&mut self.re[..n], &mut self.im[..n], true);
        for i in first..n {
            let a = base.wrapping_add(i).wrapping_sub(n) & (span - 1);
            let (p, q) = (self.re[i] * self.ws[i], self.im[i] * self.ws[i]);
            let (l, r) = if ms { (p + q, p - q) } else { (p, q) };
            self.acc[0][a] += l;
            self.acc[1][a] += r;
        }
        self.wet_left = span;
    }
}

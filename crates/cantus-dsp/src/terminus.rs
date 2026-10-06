//! Terminus, the limiter: a two-stage, true-peak mastering limiter with an oversampled
//! clipper in front, dither behind, and the loudness columns its editor meters from.
//!
//! ## The signal path
//!
//! ```text
//! in ─ DC filter ─ gain ─ clipper (4×) ─┬─ delay ────────────── × stage-1 gain ─ true-peak safety ─ clamp ─ 1:1 / match / Δ ─ dither ─ out
//!                                       └─ detector (4× TP) ─ transient stage ┐
//!                                                           ─ release stage  ┘ max
//! ```
//!
//! - **Two stages, linked separately.** The *transient* stage is a look-ahead brick wall: it
//!   reaches the reduction a peak needs by the time the peak leaves the delay line, and
//!   lets go within a few milliseconds. The *release* stage is an envelope with the Attack
//!   and Release knobs: a peak shorter than the attack barely moves it, a sustained overload
//!   does, and then it decides how the reduction recovers. The reduction applied is the
//!   larger of the two. Each stage has its own stereo link, so a one-sided transient can be
//!   caught on its side alone while the slow recovery stays centred.
//! - **Exact by construction.** The transient stage runs in whole micro-decibels (`i64`): a
//!   running maximum over the look-ahead window, then one or two moving averages whose
//!   lengths add up to the window. Every value averaged is at least the peak's requirement,
//!   so the average is too — the sample peak can never pass the ceiling, with no rounding
//!   drift however long it runs.
//! - **True peak, verified.** With *true peak* on, the detector reads the band-limited
//!   waveform between the samples (4×, refined; [`os`]), and a second, linked stage measures
//!   the *output* the same way and removes what gain modulation put back between the
//!   samples. The tests measure the result with an independent 32× reference.
//! - **Styles are settings you can read** ([`StyleSpec`]), printed by the editor:
//!
//! | Style | Attack ramp | Transient stage lets go in | Release stage |
//! |---|---|---|---|
//! | Clean | S-curve over the whole look-ahead | 40 ms | as set; slows to 2× under sustained reduction |
//! | Punch | straight, over the last 40 % | 10 ms | as set; quicker (to ½) when the reduction is light |
//! | Loud | straight, over the last 15 % | 2.5 ms | attack 2× later; quicker when light |
//! | Bus | straight, over the whole look-ahead | 30 ms | attack at ¼ — it engages early and pumps |
//! | Safe | S-curve over the whole look-ahead | 150 ms | takes every overload at once; slows to 3× |
//!
//! ## The null-test contract
//!
//! At 0 dB of gain, with the signal under the ceiling (and the knee), no clipper, no DC
//! filter, no dither and nothing to monitor, the output is the input delayed by
//! [`Terminus::latency`], to the bit. Everything is per sample and every glide and column is
//! counted from the start of the stream, so the output does not depend on the block size.

#[path = "terminus_loudness.rs"]
pub mod loudness;
#[path = "terminus_os.rs"]
pub mod os;
#[cfg(test)]
#[path = "terminus_tests_tp.rs"]
mod tests_tp;

use loudness::{column_len, KWeighting};
use os::{Decim, Interp, TruePeak, TP_DELAY};

/// The longest look-ahead, ms.
pub const MAX_LOOKAHEAD_MS: f32 = 5.0;
/// The true-peak safety stage's reach either side of a peak (samples), so the gain is flat
/// across the reconstruction filter's main lobe.
const SAFE_SPREAD: usize = 8;
/// The true-peak safety stage's attack ramp, samples.
const SAFE_RAMP: usize = 48;
/// The safety stage aims this far under the ceiling, dB: what its 4× detector can read low
/// on material with energy near Nyquist (0.05 until round 19; see `docs/plugins/limiter.md`,
/// decision 3, and `tests_tp`).
pub const SAFE_MARGIN_DB: f32 = 0.15;
/// How fast the safety stage lets go, ms.
const SAFE_RELEASE_MS: f32 = 5.0;
/// Where the clipper's residual (what clipping changes) is cut, as a fraction of the
/// sample rate. Clipping makes harmonics up to the 4× rate's Nyquist; the residual is
/// brought back through a low-pass that leaves nothing in the last few per cent below the
/// base rate's Nyquist, where the true-peak detectors' 4× reconstruction is still in its
/// transition and reads low (round 19: with the prototype there, clipped drums read up to
/// 0.24 dB over the ceiling by the 32× reference).
const CLIP_BAND: f64 = 0.40;
/// The clipper's low-pass window (Kaiser β): about 60 dB down from the base Nyquist.
const CLIP_BETA: f64 = 6.0;
/// Micro-decibels: the transient stage's integer unit.
const MICRO: f64 = 1e6;
/// Consecutive exact-zero samples after which dither falls silent too.
const BLANK_AFTER: u32 = 64;

/// One style's engine, spelled out.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct StyleSpec {
    /// How much of the look-ahead the transient stage's attack ramp uses (0…1), ending at
    /// the peak. Short ramps keep the front of a transient and are nearer to clipping.
    pub ramp: f32,
    /// An S-shaped ramp (two moving averages) rather than a straight one.
    pub s_curve: bool,
    /// How fast the transient stage lets go once a peak has passed, ms.
    pub transient_release_ms: f32,
    /// The release stage's attack, as a multiple of the Attack knob.
    pub attack_scale: f32,
    /// How the release stage's release follows the programme: above 0 it slows (to 1 + x
    /// times) under sustained reduction; below 0 it quickens (to 1 + x times) when the
    /// reduction is light.
    pub program: f32,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum LimStyle {
    Clean,
    Punch,
    Loud,
    Bus,
    Safe,
}

impl LimStyle {
    pub const ALL: [LimStyle; 5] = [
        LimStyle::Clean,
        LimStyle::Punch,
        LimStyle::Loud,
        LimStyle::Bus,
        LimStyle::Safe,
    ];

    pub fn from_index(i: usize) -> LimStyle {
        Self::ALL[i.min(Self::ALL.len() - 1)]
    }

    pub fn index(self) -> usize {
        Self::ALL.iter().position(|s| *s == self).unwrap_or(0)
    }

    pub fn spec(self) -> StyleSpec {
        match self {
            LimStyle::Clean => StyleSpec {
                ramp: 1.0,
                s_curve: true,
                transient_release_ms: 40.0,
                attack_scale: 1.0,
                program: 1.0,
            },
            LimStyle::Punch => StyleSpec {
                ramp: 0.4,
                s_curve: false,
                transient_release_ms: 10.0,
                attack_scale: 1.0,
                program: -0.5,
            },
            LimStyle::Loud => StyleSpec {
                ramp: 0.15,
                s_curve: false,
                transient_release_ms: 2.5,
                attack_scale: 2.0,
                program: -0.5,
            },
            LimStyle::Bus => StyleSpec {
                ramp: 1.0,
                s_curve: false,
                transient_release_ms: 30.0,
                attack_scale: 0.25,
                program: 0.0,
            },
            LimStyle::Safe => StyleSpec {
                ramp: 1.0,
                s_curve: true,
                transient_release_ms: 150.0,
                attack_scale: 0.0,
                program: 2.0,
            },
        }
    }
}

/// What leaves the limiter.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Monitor {
    /// The limited signal.
    Normal,
    /// The limited signal turned down by the gain: the limiting, without the loudness.
    Unity,
    /// The limited signal at the input's measured short-term loudness.
    Match,
}

impl Monitor {
    pub fn from_index(i: usize) -> Monitor {
        match i {
            1 => Monitor::Unity,
            2 => Monitor::Match,
            _ => Monitor::Normal,
        }
    }
}

/// Dither's target word length.
pub const DITHER_BITS: [u32; 6] = [0, 24, 22, 20, 18, 16];

/// Noise shaping of the dither.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Shaping {
    /// Flat TPDF.
    Flat,
    /// First-order: lower in the bass and mids, higher above about fs/6.
    Basic,
    /// Lipshitz's five-tap "minimally audible" curve: the error moved to where the ear is
    /// least sensitive, about 15 dB under flat below 4 kHz.
    Optimized,
}

impl Shaping {
    pub fn from_index(i: usize) -> Shaping {
        match i {
            1 => Shaping::Basic,
            2 => Shaping::Optimized,
            _ => Shaping::Flat,
        }
    }

    fn taps(self) -> &'static [f32] {
        match self {
            Shaping::Flat => &[],
            Shaping::Basic => &[1.0],
            Shaping::Optimized => &[2.033, -2.165, 1.959, -1.590, 0.6149],
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct LimSettings {
    /// Drive into the ceiling, dB (0…30).
    pub gain_db: f32,
    /// The most the output may reach, dBFS — dBTP with `true_peak`.
    pub ceiling_db: f32,
    pub style: LimStyle,
    pub lookahead_ms: f32,
    /// The release stage's attack and release, ms.
    pub attack_ms: f32,
    pub release_ms: f32,
    /// Stereo link of each stage, 0…1.
    pub link_transients: f32,
    pub link_release: f32,
    /// Width of the soft knee under the ceiling, dB; 0 is a hard knee.
    pub knee_db: f32,
    pub true_peak: bool,
    /// The clipper in front, 0…1: at 0 off; above, its threshold falls from 6 dB over the
    /// ceiling to the ceiling, so more of each peak is clipped and less limited.
    pub clip: f32,
    pub dc_filter: bool,
    pub monitor: Monitor,
    /// Hear what the limiter takes away (input minus output) instead.
    pub delta: bool,
    /// Index into [`DITHER_BITS`]; 0 is off.
    pub dither: usize,
    pub shaping: Shaping,
}

impl Default for LimSettings {
    fn default() -> Self {
        LimSettings {
            gain_db: 0.0,
            ceiling_db: -1.0,
            style: LimStyle::Clean,
            lookahead_ms: 2.0,
            attack_ms: 10.0,
            release_ms: 200.0,
            link_transients: 0.75,
            link_release: 1.0,
            knee_db: 0.0,
            true_peak: true,
            clip: 0.0,
            dc_filter: false,
            monitor: Monitor::Normal,
            delta: false,
            dither: 0,
            shaping: Shaping::Flat,
        }
    }
}

/// The static curve: how many dB of reduction a level earns against the ceiling with a
/// soft knee of `knee_db` centred on it. The output never passes the ceiling: above the
/// knee the reduction is the whole excess.
#[inline]
pub fn reduction_db(level_db: f32, ceiling_db: f32, knee_db: f32) -> f32 {
    let over = level_db - ceiling_db;
    let w = knee_db.max(0.0);
    if w > 0.0 {
        if over <= -0.5 * w {
            0.0
        } else if over < 0.5 * w {
            // Reaches the ceiling (reduction = excess + w/2) with slope 1 at the knee's top.
            (over + 0.5 * w).powi(2) / (2.0 * w)
        } else {
            over
        }
    } else {
        over.max(0.0)
    }
}

/// The output level for an input level on the static curve — what the editor draws.
pub fn transfer_db(level_db: f32, ceiling_db: f32, knee_db: f32) -> f32 {
    level_db - reduction_db(level_db, ceiling_db, knee_db)
}

/// Where a soft knee of this width starts bending, dB under the ceiling.
pub fn knee_start_db(knee_db: f32) -> f32 {
    -0.5 * knee_db.max(0.0)
}

fn lookahead_samples(ms: f32, sr: f32) -> usize {
    (ms.clamp(0.0, MAX_LOOKAHEAD_MS) * 0.001 * sr).round() as usize
}

/// Samples of delay a set of settings adds at a sample rate: what the editor shows and what
/// the engine compensates.
pub fn latency_of(s: &LimSettings, sample_rate: f32) -> usize {
    let clip = if s.clip > 0.0 { 2 * os::DELAY } else { 0 };
    let det = if s.true_peak { TP_DELAY } else { 0 };
    let safe = if s.true_peak {
        TP_DELAY + SAFE_SPREAD + SAFE_RAMP
    } else {
        0
    };
    clip + det + lookahead_samples(s.lookahead_ms, sample_rate) + safe
}

#[inline]
fn coef(ms: f32, sr: f32) -> f32 {
    if ms <= 0.0 {
        0.0
    } else {
        (-1.0 / (ms * 0.001 * sr)).exp()
    }
}

#[inline]
fn db(lin: f32) -> f32 {
    if lin > 1e-8 {
        20.0 * lin.log10()
    } else {
        -160.0
    }
}

/// A delay line of up to `cap − 1` samples.
#[derive(Clone, Debug)]
struct Delay<T: Copy + Default> {
    buf: Box<[T]>,
    pos: usize,
}

impl<T: Copy + Default> Delay<T> {
    fn new(max_delay: usize) -> Self {
        Delay {
            buf: vec![T::default(); max_delay + 1].into_boxed_slice(),
            pos: 0,
        }
    }

    fn clear(&mut self) {
        self.buf.iter_mut().for_each(|x| *x = T::default());
        self.pos = 0;
    }

    /// Holds only defaults (zeros): the position does not matter then.
    fn is_clear(&self) -> bool
    where
        T: PartialEq,
    {
        let zero = T::default();
        self.buf.iter().all(|x| *x == zero)
    }

    #[inline]
    fn push(&mut self, x: T, delay: usize) -> T {
        let cap = self.buf.len();
        self.buf[self.pos] = x;
        let out = self.buf[(self.pos + cap - delay.min(cap - 1)) % cap];
        self.pos = if self.pos + 1 == cap { 0 } else { self.pos + 1 };
        out
    }
}

/// Running maximum over the last `window` values (a monotonic queue).
#[derive(Clone, Debug)]
struct MaxWindow {
    idx: Box<[u64]>,
    val: Box<[i64]>,
    head: usize,
    len: usize,
    window: usize,
    n: u64,
}

impl MaxWindow {
    fn new(max_window: usize) -> Self {
        MaxWindow {
            idx: vec![0; max_window + 2].into_boxed_slice(),
            val: vec![0; max_window + 2].into_boxed_slice(),
            head: 0,
            len: 0,
            window: 1,
            n: 0,
        }
    }

    fn set(&mut self, window: usize) {
        self.window = window.clamp(1, self.val.len() - 2);
        self.head = 0;
        self.len = 0;
    }

    #[inline]
    fn push(&mut self, v: i64) -> i64 {
        let cap = self.val.len();
        while self.len > 0 {
            let back = (self.head + self.len - 1) % cap;
            if self.val[back] <= v {
                self.len -= 1;
            } else {
                break;
            }
        }
        let slot = (self.head + self.len) % cap;
        self.val[slot] = v;
        self.idx[slot] = self.n;
        self.len += 1;
        while self.idx[self.head] + self.window as u64 <= self.n {
            self.head = (self.head + 1) % cap;
            self.len -= 1;
        }
        self.n += 1;
        self.val[self.head]
    }
}

/// A moving average in integers, rounded up (so it never under-reads what it averages).
#[derive(Clone, Debug)]
struct BoxMean {
    ring: Box<[i64]>,
    len: usize,
    pos: usize,
    sum: i64,
}

impl BoxMean {
    fn new(max_len: usize) -> Self {
        BoxMean {
            ring: vec![0; max_len.max(1)].into_boxed_slice(),
            len: 1,
            pos: 0,
            sum: 0,
        }
    }

    fn set(&mut self, len: usize, fill: i64) {
        self.len = len.clamp(1, self.ring.len());
        self.pos = 0;
        self.ring[..self.len].iter_mut().for_each(|x| *x = fill);
        self.sum = fill * self.len as i64;
    }

    #[inline]
    fn push(&mut self, v: i64) -> i64 {
        if self.len == 1 {
            return v;
        }
        self.sum += v - self.ring[self.pos];
        self.ring[self.pos] = v;
        self.pos = if self.pos + 1 == self.len {
            0
        } else {
            self.pos + 1
        };
        let n = self.len as i64;
        (self.sum + n - 1) / n
    }
}

/// A look-ahead gain computer: hold the largest requirement for `hold` samples, let go
/// exponentially, and smooth with two moving averages. Output at `n` covers every input in
/// `[n − hold + 1, n − (b1 + b2 − 2)]` (see the module notes); values are micro-dB.
#[derive(Clone, Debug)]
struct Lookahead {
    hold: MaxWindow,
    b1: BoxMean,
    b2: BoxMean,
    release: f64,
    held: i64,
    out: i64,
    shape: (usize, usize, usize),
}

impl Lookahead {
    fn new(max: usize) -> Self {
        let mut l = Lookahead {
            hold: MaxWindow::new(max + 2),
            b1: BoxMean::new(max + 2),
            b2: BoxMean::new(max + 2),
            release: 0.0,
            held: 0,
            out: 0,
            shape: (0, 0, 0),
        };
        l.configure(1, 1, 1, 0.0);
        l
    }

    /// Change the windows. The output carries on from where it was.
    fn configure(&mut self, hold: usize, b1: usize, b2: usize, release: f64) {
        self.release = release;
        if self.shape == (hold, b1, b2) {
            return;
        }
        self.shape = (hold, b1, b2);
        self.hold.set(hold);
        self.b1.set(b1, self.out);
        self.b2.set(b2, self.out);
        self.held = self.out;
    }

    fn reset(&mut self) {
        self.out = 0;
        let (h, b1, b2) = self.shape;
        self.shape = (0, 0, 0);
        self.configure(h, b1, b2, self.release);
    }

    /// As [`Self::reset`] leaves it, for every input it can be given (requirements are
    /// never negative): no requirement held, both averages at zero, and nothing in the hold
    /// window but zeros — which any requirement pushes out, as from an empty window.
    fn is_clear(&self) -> bool {
        let w = &self.hold;
        let cap = w.val.len();
        self.out == 0
            && self.held == 0
            && self.b1.sum == 0
            && self.b2.sum == 0
            && self.b1.ring[..self.b1.len].iter().all(|v| *v == 0)
            && self.b2.ring[..self.b2.len].iter().all(|v| *v == 0)
            && (0..w.len).all(|k| w.val[(w.head + k) % cap] == 0)
    }

    #[inline]
    fn push(&mut self, v: i64) -> i64 {
        let h = self.hold.push(v);
        let decayed = (self.held as f64 * self.release) as i64;
        self.held = h.max(decayed);
        let s = self.b2.push(self.b1.push(self.held));
        self.out = s;
        s
    }
}

/// The oversampled clipper for one channel. It computes only *what clipping changes*, at
/// 4×, and adds that back, band-limited, to the delayed input — so a clipper that clips
/// nothing is an exact delay.
#[derive(Clone, Debug)]
struct Clipper {
    interp: Interp,
    decim: Decim,
    dry: Delay<f32>,
    /// The decimator holds only zeros (nothing clipped since it was last cleared).
    quiet: bool,
}

impl Clipper {
    fn new() -> Self {
        Clipper {
            interp: Interp::new(),
            decim: Decim::with_filter(&os::lowpass(CLIP_BAND, CLIP_BETA)),
            dry: Delay::new(2 * os::DELAY + 1),
            quiet: true,
        }
    }

    fn reset(&mut self) {
        self.interp.reset();
        self.decim.reset();
        self.dry.clear();
        self.quiet = true;
    }

    fn is_clear(&self) -> bool {
        self.quiet && self.interp.is_clear() && self.decim.is_clear() && self.dry.is_clear()
    }

    /// Soft clip to `t`: straight up to 70 % of it, then a tanh knee that never reaches it.
    #[inline]
    fn shape(u: f32, t: f32) -> f32 {
        let a = 0.7 * t;
        let m = u.abs();
        if m <= a {
            u
        } else {
            let w = t - a;
            (a + w * ((m - a) / w).tanh()).copysign(u)
        }
    }

    /// Keep the history current while switched off.
    #[inline]
    fn idle(&mut self, x: f32) {
        self.interp.feed(x);
        self.dry.push(x, 0);
        if !self.quiet {
            self.decim.reset();
            self.quiet = true;
        }
    }

    /// Returns the clipped sample (delayed by `2·DELAY`) and the largest reduction the
    /// clipper made among the four points, dB.
    #[inline]
    fn process(&mut self, x: f32, threshold: f32) -> (f32, f32) {
        let pts = self.interp.push(x);
        let dry = self.dry.push(x, 2 * os::DELAY);
        let mut worst = 0f32;
        let mut y = 0f32;
        let a = 0.7 * threshold;
        for (p, &u) in pts.iter().enumerate() {
            let d = if u.abs() <= a {
                0.0
            } else {
                let c = Self::shape(u, threshold);
                worst = worst.max(db(u.abs()) - db(c.abs()));
                self.quiet = false;
                c - u
            };
            self.decim.push(d);
            if p == 0 {
                y = self.decim.output();
            }
        }
        let out = if self.quiet { dry } else { dry + y };
        (out, worst)
    }
}

/// TPDF dither with optional error-feedback noise shaping, for one channel. Public so the
/// maximizer (`culmen`) dithers with the same code.
#[derive(Clone, Debug)]
pub struct Ditherer {
    rng: u32,
    err: [f32; 5],
    zeros: u32,
}

impl Ditherer {
    pub fn new(seed: u32) -> Self {
        Ditherer {
            rng: seed,
            err: [0.0; 5],
            zeros: 0,
        }
    }

    #[inline]
    fn uniform(&mut self) -> f32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 17;
        self.rng ^= self.rng << 5;
        (self.rng >> 8) as f32 / 16_777_216.0
    }

    /// One sample to `bits` bits; digital silence stays silent after 64 zeros.
    #[inline]
    pub fn process(&mut self, x: f32, bits: u32, shaping: Shaping) -> f32 {
        // Digital silence stays silent: no hiss after the song ends.
        if x == 0.0 {
            self.zeros = self.zeros.saturating_add(1);
            if self.zeros >= BLANK_AFTER {
                self.err = [0.0; 5];
                return 0.0;
            }
        } else {
            self.zeros = 0;
        }
        let q = 2f32.powi(1 - bits as i32);
        let mut fb = 0f32;
        for (h, e) in shaping.taps().iter().zip(self.err.iter()) {
            fb += h * e;
        }
        let v = x - fb;
        let d = (self.uniform() - self.uniform()) * q;
        let y = ((v + d) / q).round() * q;
        let y = y.clamp(-1.0, 1.0 - q);
        // Bounded, so a clamp at full scale cannot wind the shaping filter up.
        let e = (y - v).clamp(-4.0 * q, 4.0 * q);
        self.err.copy_within(0..4, 1);
        self.err[0] = e;
        y
    }
}

/// A DC blocker: a first-order high-pass at 5 Hz.
#[derive(Copy, Clone, Debug, Default)]
struct DcBlock {
    x1: f32,
    y1: f32,
}

impl DcBlock {
    fn is_clear(&self) -> bool {
        self.x1 == 0.0 && self.y1 == 0.0
    }

    #[inline]
    fn tick(&mut self, x: f32, r: f32) -> f32 {
        let y = x - self.x1 + r * self.y1;
        self.x1 = x;
        self.y1 = if y.abs() < 1e-20 { 0.0 } else { y };
        self.y1
    }
}

// ---- telemetry ------------------------------------------------------------------------

/// Floats in the telemetry block.
pub const TELEMETRY: usize = 64;
/// Header fields before the column ring.
pub const T_HEADER: usize = 8;
/// Floats per column.
pub const T_COLUMN: usize = 5;
/// Columns kept in the ring: 110 ms of history, so a frame that comes late loses nothing.
pub const T_COLUMNS: usize = (TELEMETRY - T_HEADER) / T_COLUMN;
/// The column counter wraps here: a multiple of the ring, exactly representable in f32.
pub const T_WRAP: u32 = (16_777_216 / T_COLUMNS as u32) * T_COLUMNS as u32;

/// Header field: columns published so far (mod [`T_WRAP`]).
pub const T_SEQ: usize = 0;
/// Header field: latency, samples.
pub const T_LATENCY: usize = 1;
/// Header field: sample rate.
pub const T_RATE: usize = 2;
/// Header field: the Match monitor's gain, dB.
pub const T_MATCH: usize = 3;
/// Header fields: the last column's output true peak, left and right, dBTP.
pub const T_OUT_L: usize = 4;
pub const T_OUT_R: usize = 5;
/// Header fields: the last column's largest reduction, left and right, dB.
pub const T_GR_L: usize = 6;
pub const T_GR_R: usize = 7;

/// One 10 ms column of what the limiter did.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Column {
    /// K-weighted energy of the output (the channels summed), for loudness.
    pub energy: f32,
    /// Input peak after the gain, dBFS.
    pub input_db: f32,
    /// Output true peak, dBTP.
    pub output_db: f32,
    /// Largest gain reduction, dB (positive).
    pub gr_db: f32,
    /// Largest reduction the clipper made, dB (positive).
    pub clip_db: f32,
}

impl Column {
    /// Read column `index` (a count of columns since the stream began) from a telemetry
    /// block. Valid for the last [`T_COLUMNS`] columns.
    pub fn read(t: &[f32], index: u32) -> Column {
        let at = T_HEADER + (index as usize % T_COLUMNS) * T_COLUMN;
        Column {
            energy: t[at],
            input_db: t[at + 1],
            output_db: t[at + 2],
            gr_db: t[at + 3],
            clip_db: t[at + 4],
        }
    }
}

#[derive(Copy, Clone, Debug, Default)]
struct ColumnAcc {
    n: usize,
    energy: f64,
    energy_in: f64,
    input: f32,
    out: [f32; 2],
    gr: [f32; 2],
    gr_max: f32,
    clip: f32,
}

// ---- the limiter ----------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct Terminus {
    sr: f32,
    s: LimSettings,
    // Derived from the settings.
    look: usize,
    ramp: usize,
    t_release: f64,
    // Glides, per sample.
    gain: f32,
    gain_target: f32,
    ceiling_db: f32,
    glide: f32,
    // Stages.
    dc: [DcBlock; 2],
    dc_r: f32,
    clip: [Clipper; 2],
    det: [TruePeak; 2],
    t_pre: [Delay<i64>; 2],
    t_stage: [Lookahead; 2],
    r_env: [f32; 2],
    r_delay: [Delay<f32>; 2],
    work: f32,
    audio: [Delay<f32>; 2],
    safe_det: [TruePeak; 2],
    safe: Lookahead,
    safe_audio: [Delay<f32>; 2],
    dry: [Delay<f32>; 2],
    // Metering.
    meter_tp: [TruePeak; 2],
    k_out: [KWeighting; 2],
    k_in: [KWeighting; 2],
    col_len: usize,
    acc: ColumnAcc,
    st_in: Box<[f64]>,
    st_out: Box<[f64]>,
    st_pos: usize,
    st_seen: usize,
    match_gain: f32,
    match_target: f32,
    match_glide: f32,
    dither: [Ditherer; 2],
    telemetry: [f32; TELEMETRY],
    seq: u32,
    /// The gain reduction in force at the last sample, dB, per channel.
    gr_now: [f32; 2],
    /// The ceiling the linear thresholds below were worked out for (NaN: none yet).
    derived_for: f32,
    /// The ceiling, the level where reduction starts (the knee's foot), and the clipper's
    /// threshold, linear.
    derived: (f32, f32, f32),
}

impl Terminus {
    pub fn new(sample_rate: f32) -> Self {
        let sr = sample_rate.max(1.0);
        let max_look = lookahead_samples(MAX_LOOKAHEAD_MS, sr) + 1;
        let max_latency = 2 * os::DELAY + TP_DELAY + max_look + TP_DELAY + SAFE_SPREAD + SAFE_RAMP;
        let col_len = column_len(sr);
        let mut t = Terminus {
            sr,
            s: LimSettings::default(),
            look: 0,
            ramp: 0,
            t_release: 0.0,
            gain: 1.0,
            gain_target: 1.0,
            ceiling_db: -1.0,
            glide: 1.0 - coef(10.0, sr),
            dc: [DcBlock::default(); 2],
            dc_r: (-2.0 * std::f32::consts::PI * 5.0 / sr).exp(),
            clip: [Clipper::new(), Clipper::new()],
            det: [TruePeak::new(), TruePeak::new()],
            t_pre: [Delay::new(max_look), Delay::new(max_look)],
            t_stage: [Lookahead::new(max_look), Lookahead::new(max_look)],
            r_env: [0.0; 2],
            r_delay: [Delay::new(max_look), Delay::new(max_look)],
            work: 0.0,
            audio: [
                Delay::new(TP_DELAY + max_look),
                Delay::new(TP_DELAY + max_look),
            ],
            safe_det: [TruePeak::new(), TruePeak::new()],
            safe: Lookahead::new(2 * SAFE_SPREAD + SAFE_RAMP + 2),
            safe_audio: [
                Delay::new(TP_DELAY + SAFE_SPREAD + SAFE_RAMP),
                Delay::new(TP_DELAY + SAFE_SPREAD + SAFE_RAMP),
            ],
            dry: [Delay::new(max_latency), Delay::new(max_latency)],
            meter_tp: [TruePeak::new(), TruePeak::new()],
            k_out: [KWeighting::new(sr); 2],
            k_in: [KWeighting::new(sr); 2],
            col_len,
            acc: ColumnAcc::default(),
            st_in: vec![0.0; loudness::SHORT_COLUMNS].into_boxed_slice(),
            st_out: vec![0.0; loudness::SHORT_COLUMNS].into_boxed_slice(),
            st_pos: 0,
            st_seen: 0,
            match_gain: 1.0,
            match_target: 1.0,
            match_glide: 1.0 - coef(1_000.0, sr),
            dither: [Ditherer::new(0x9e37_79b9), Ditherer::new(0x85eb_ca6b)],
            telemetry: [0.0; TELEMETRY],
            seq: 0,
            gr_now: [0.0; 2],
            derived_for: f32::NAN,
            derived: (1.0, 1.0, 1.0),
        };
        t.set(&LimSettings::default());
        t.reset();
        t
    }

    pub fn sample_rate(&self) -> f32 {
        self.sr
    }

    pub fn settings(&self) -> &LimSettings {
        &self.s
    }

    pub fn set(&mut self, s: &LimSettings) {
        let first = self.seq == 0 && self.acc.n == 0;
        self.s = *s;
        self.gain_target = if s.gain_db == 0.0 {
            1.0
        } else {
            10f32.powf(s.gain_db / 20.0)
        };
        if first {
            self.gain = self.gain_target;
            self.ceiling_db = s.ceiling_db;
        }
        let spec = s.style.spec();
        self.look = lookahead_samples(s.lookahead_ms, self.sr);
        self.ramp = ((self.look as f32 * spec.ramp).round() as usize).min(self.look);
        self.t_release = coef(spec.transient_release_ms, self.sr) as f64;
        let (b1, b2) = if spec.s_curve {
            let b1 = (self.ramp + 3) / 2;
            (b1, self.ramp + 2 - b1)
        } else {
            (self.ramp + 1, 1)
        };
        for t in &mut self.t_stage {
            t.configure(self.ramp + 1, b1, b2, self.t_release);
        }
        let b1 = (SAFE_RAMP + 3) / 2;
        self.safe.configure(
            2 * SAFE_SPREAD + SAFE_RAMP + 1,
            b1,
            SAFE_RAMP + 2 - b1,
            coef(SAFE_RELEASE_MS, self.sr) as f64,
        );
        self.telemetry[T_LATENCY] = self.latency() as f32;
        self.telemetry[T_RATE] = self.sr;
        self.derived_for = f32::NAN;
    }

    /// Linear thresholds for a ceiling: worked out when the ceiling moves, not per sample.
    fn derive(&mut self, ceil_db: f32) {
        let ceil_lin = 10f32.powf(ceil_db / 20.0);
        // Below the knee's foot nothing is reduced, so no logarithm is needed there.
        let quiet_below = ceil_lin * 10f32.powf(knee_start_db(self.s.knee_db) / 20.0);
        let clip_t = ceil_lin * 10f32.powf((1.0 - self.s.clip.clamp(0.0, 1.0)) * 6.0 / 20.0);
        self.derived = (ceil_lin, quiet_below, clip_t);
        self.derived_for = ceil_db;
    }

    /// Clear every state; glides land on their targets.
    pub fn reset(&mut self) {
        self.gain = self.gain_target;
        self.ceiling_db = self.s.ceiling_db;
        self.dc = [DcBlock::default(); 2];
        for c in &mut self.clip {
            c.reset();
        }
        for d in self
            .det
            .iter_mut()
            .chain(self.safe_det.iter_mut())
            .chain(self.meter_tp.iter_mut())
        {
            d.reset();
        }
        for i in 0..2 {
            self.t_pre[i].clear();
            self.t_stage[i].reset();
            self.r_delay[i].clear();
            self.audio[i].clear();
            self.safe_audio[i].clear();
            self.dry[i].clear();
            self.k_out[i].reset();
            self.k_in[i].reset();
        }
        self.safe.reset();
        self.r_env = [0.0; 2];
        self.work = 0.0;
        self.acc = ColumnAcc::default();
        self.st_in.iter_mut().for_each(|x| *x = 0.0);
        self.st_out.iter_mut().for_each(|x| *x = 0.0);
        self.st_pos = 0;
        self.st_seen = 0;
        self.match_gain = 1.0;
        self.match_target = 1.0;
        self.dither = [Ditherer::new(0x9e37_79b9), Ditherer::new(0x85eb_ca6b)];
        self.telemetry = [0.0; TELEMETRY];
        self.telemetry[T_LATENCY] = self.latency() as f32;
        self.telemetry[T_RATE] = self.sr;
        self.seq = 0;
        self.gr_now = [0.0; 2];
    }

    /// At rest (ADR 0031 §5.1): every stage as [`Self::reset`] leaves it — glides landed,
    /// filters, detectors, look-ahead windows, envelopes and delay lines empty — and the
    /// Match monitor's memory still: its three-second window all silence (so no column can
    /// move its level) and its gain landed; in Match mode, at unity as a new unit's is.
    /// The meters that only the editor reads do not count.
    ///
    /// **Never with dither on** (R10): the noise generator stops on silence but does not go
    /// back to its seed, so a new unit would dither the next sound with other noise. When
    /// the dither becomes counter-based this can hold after its shaping state clears.
    pub fn at_rest(&self) -> bool {
        let s = &self.s;
        let spec = s.style.spec();
        let a = &self.acc;
        if DITHER_BITS[s.dither.min(DITHER_BITS.len() - 1)] > 0
            || self.gain != self.gain_target
            || self.ceiling_db != s.ceiling_db
            || self.match_gain != self.match_target
            || (s.monitor == Monitor::Match && self.match_gain != 1.0)
            || (spec.program > 0.0 && !self.work_is_nothing())
            || self.r_env != [0.0; 2]
            || a.energy != 0.0
            || a.energy_in != 0.0
            || !self.dc.iter().all(DcBlock::is_clear)
            || !self.k_in.iter().chain(&self.k_out).all(KWeighting::is_clear)
        {
            return false;
        }
        crate::rest::all_zero_f64(&self.st_in)
            && crate::rest::all_zero_f64(&self.st_out)
            && self.det.iter().chain(&self.safe_det).all(TruePeak::is_clear)
            && self.t_stage.iter().all(Lookahead::is_clear)
            && self.safe.is_clear()
            && self.clip.iter().all(Clipper::is_clear)
            && self.t_pre.iter().all(Delay::is_clear)
            && self
                .r_delay
                .iter()
                .chain(&self.audio)
                .chain(&self.safe_audio)
                .chain(&self.dry)
                .all(Delay::is_clear)
    }

    /// Whether the programme-dependent release's memory (`work`) is as good as zero. It
    /// decays by `work / sr` a sample with no flush, so without FTZ it stops in the
    /// subnormals (near 2⁻¹³⁴) instead of at zero. Any value below `f32::MIN_POSITIVE` is
    /// exactly zero in every use: added to `1 / sr` it vanishes, and scaled by a difference
    /// of two coefficients (under 1) and added to a release coefficient of at least ½ it is
    /// far under half an ulp.
    fn work_is_nothing(&self) -> bool {
        self.work == 0.0 || (self.work < f32::MIN_POSITIVE && coef(self.s.release_ms, self.sr) >= 0.5)
    }

    /// Samples of silence skipped while at rest (ADR 0031 §5.1): the 10 ms columns the
    /// Match monitor's window counts go on as processing would have moved them — each
    /// one silent, so its window stays all silence and its level holds.
    pub fn skip(&mut self, frames: usize) {
        let total = self.acc.n + frames;
        let cols = total / self.col_len;
        if cols > 0 {
            let len = self.st_in.len();
            self.st_pos = (self.st_pos + cols % len) % len;
            self.st_seen = self.st_seen.saturating_add(cols);
            self.seq = ((self.seq as usize + cols % T_WRAP as usize) % T_WRAP as usize) as u32;
            self.acc = ColumnAcc::default();
        }
        self.acc.n = total % self.col_len;
    }

    /// Samples of delay this adds: look-ahead, the true-peak detectors and the clipper's
    /// filters. The engine compensates every other path by the same amount.
    pub fn latency(&self) -> usize {
        latency_of(&self.s, self.sr)
    }

    /// What the editor reads (see the `T_*` constants and [`Column`]).
    pub fn telemetry(&self) -> [f32; TELEMETRY] {
        self.telemetry
    }

    /// The gain reduction in force at the last sample, dB, left and right.
    pub fn gain_reduction_db(&self) -> [f32; 2] {
        self.gr_now
    }

    fn finish_column(&mut self) {
        let a = self.acc;
        let n = self.col_len as f64;
        let energy = a.energy / n;
        // Short-term (3 s) energy of the input before the gain and of the output: what the
        // Match monitor levels by.
        self.st_in[self.st_pos] = a.energy_in / n;
        self.st_out[self.st_pos] = energy;
        self.st_pos = (self.st_pos + 1) % self.st_in.len();
        self.st_seen += 1;
        if self.st_seen >= self.st_in.len() {
            let e_in: f64 = self.st_in.iter().sum::<f64>() / self.st_in.len() as f64;
            let e_out: f64 = self.st_out.iter().sum::<f64>() / self.st_out.len() as f64;
            // Hold through silence: only a signal above the absolute gate re-levels it.
            if loudness::lufs(e_in) > loudness::ABSOLUTE_GATE && e_out > 0.0 {
                self.match_target = (e_in / e_out).sqrt().clamp(0.01, 10.0) as f32;
            }
        }
        let at = T_HEADER + (self.seq as usize % T_COLUMNS) * T_COLUMN;
        self.telemetry[at] = energy as f32;
        self.telemetry[at + 1] = db(a.input);
        self.telemetry[at + 2] = db(a.out[0].max(a.out[1]));
        self.telemetry[at + 3] = a.gr_max;
        self.telemetry[at + 4] = a.clip;
        self.seq = (self.seq + 1) % T_WRAP;
        self.telemetry[T_SEQ] = self.seq as f32;
        self.telemetry[T_MATCH] = if self.s.monitor == Monitor::Match {
            db(self.match_gain)
        } else {
            0.0
        };
        self.telemetry[T_OUT_L] = db(a.out[0]);
        self.telemetry[T_OUT_R] = db(a.out[1]);
        self.telemetry[T_GR_L] = a.gr[0];
        self.telemetry[T_GR_R] = a.gr[1];
        self.acc = ColumnAcc::default();
    }

    /// Process a stereo block in place. `key`, when routed, is what the limiter listens to
    /// instead of its own input (stem mastering: every stem gets the master's reduction).
    pub fn process(&mut self, left: &mut [f32], right: &mut [f32], key: Option<(&[f32], &[f32])>) {
        let s = self.s;
        let spec = s.style.spec();
        let sr = self.sr;
        let n = left.len().min(right.len());
        let tp = s.true_peak;
        let det_delay = if tp { TP_DELAY } else { 0 };
        let look = self.look;
        let pre = look - self.ramp;
        let clip_on = s.clip > 0.0;
        let knee = s.knee_db.max(0.0);
        let a_att = coef(s.attack_ms * spec.attack_scale, sr);
        let a_rel = coef(s.release_ms, sr);
        // The programme-dependent release moves between these two coefficients (no
        // exponential per sample): as set, and at (1 + program) times as set.
        let a_prog = coef(s.release_ms * (1.0 + spec.program).max(0.05), sr);
        let lt = s.link_transients.clamp(0.0, 1.0);
        let lr = s.link_release.clamp(0.0, 1.0);
        let bits = DITHER_BITS[s.dither.min(DITHER_BITS.len() - 1)];
        let safe_delay = TP_DELAY + SAFE_SPREAD + SAFE_RAMP;
        let latency = self.latency();
        for i in 0..n {
            let raw = [left[i], right[i]];
            // Glides: gain in linear, ceiling in dB; both land exactly on their targets.
            if self.gain != self.gain_target {
                self.gain += (self.gain_target - self.gain) * self.glide;
                if (self.gain - self.gain_target).abs() <= 1e-6 * self.gain_target {
                    self.gain = self.gain_target;
                }
            }
            if self.ceiling_db != s.ceiling_db {
                self.ceiling_db += (s.ceiling_db - self.ceiling_db) * self.glide;
                if (self.ceiling_db - s.ceiling_db).abs() < 1e-4 {
                    self.ceiling_db = s.ceiling_db;
                }
            }
            let ceil_db = self.ceiling_db;
            if ceil_db != self.derived_for {
                self.derive(ceil_db);
            }
            let (ceil_lin, quiet_below, clip_t) = self.derived;

            let mut xg = [0f32; 2];
            let mut xc = [0f32; 2];
            let mut level = [0f32; 2];
            for ch in 0..2 {
                let ki = self.k_in[ch].tick(raw[ch] as f64);
                self.acc.energy_in += ki * ki;
                let x = if s.dc_filter {
                    self.dc[ch].tick(raw[ch], self.dc_r)
                } else {
                    raw[ch]
                };
                let x = x * self.gain;
                xg[ch] = x;
                xc[ch] = if clip_on {
                    let (y, c) = self.clip[ch].process(x, clip_t);
                    self.acc.clip = self.acc.clip.max(c);
                    y
                } else {
                    self.clip[ch].idle(x);
                    x
                };
                let d = match key {
                    Some((kl, kr)) => {
                        let k = if ch == 0 { kl } else { kr };
                        k.get(i).copied().unwrap_or(0.0) * self.gain
                    }
                    None => xc[ch],
                };
                level[ch] = if tp {
                    self.det[ch].push(d)
                } else {
                    self.det[ch].push(d);
                    d.abs()
                };
            }

            // What each channel needs, dB.
            let mut req = [0f32; 2];
            for ch in 0..2 {
                if level[ch] > quiet_below {
                    req[ch] = reduction_db(db(level[ch]), ceil_db, knee);
                }
            }
            let loud = req[0].max(req[1]);
            let mut gr = [0f32; 2];
            for ch in 0..2 {
                // The transient stage, in whole micro-dB.
                let need_t = req[ch] + (loud - req[ch]) * lt;
                let v = (need_t as f64 * MICRO).ceil() as i64;
                let v = self.t_pre[ch].push(v, pre);
                let t = self.t_stage[ch].push(v) as f32 / MICRO as f32;
                // The release stage.
                let need_r = req[ch] + (loud - req[ch]) * lr;
                let cur = self.r_env[ch];
                let next = if need_r > cur {
                    need_r + (cur - need_r) * a_att
                } else {
                    let a = if spec.program > 0.0 {
                        a_rel + (a_prog - a_rel) * self.work.min(1.0)
                    } else if spec.program < 0.0 {
                        let light = (1.0 - cur / 6.0).clamp(0.0, 1.0);
                        a_rel + (a_prog - a_rel) * light
                    } else {
                        a_rel
                    };
                    need_r + (cur - need_r) * a
                };
                self.r_env[ch] = if next < 1e-6 { 0.0 } else { next };
                let r = self.r_delay[ch].push(self.r_env[ch], look);
                let g = t.max(r);
                gr[ch] = if g < 1e-6 { 0.0 } else { g };
            }
            let working = self.r_env[0].max(self.r_env[1]) > 1.0;
            self.work += if working { 1.0 / sr } else { -self.work / sr };

            let mut y = [0f32; 2];
            for ch in 0..2 {
                let x = self.audio[ch].push(xc[ch], det_delay + look);
                y[ch] = if gr[ch] == 0.0 {
                    x
                } else {
                    x * 10f32.powf(-gr[ch] / 20.0)
                };
            }

            // The true-peak safety stage: what the gain changes put back between samples.
            let mut safe_gr = 0f32;
            if tp {
                let lv = self.safe_det[0].push(y[0]).max(self.safe_det[1].push(y[1]));
                let need = if lv > ceil_lin * 0.99 {
                    (db(lv) - (ceil_db - SAFE_MARGIN_DB)).max(0.0)
                } else {
                    0.0
                };
                let v = (need as f64 * MICRO).ceil() as i64;
                let sg = self.safe.push(v) as f32 / MICRO as f32;
                safe_gr = if sg < 1e-6 { 0.0 } else { sg };
                let g = if safe_gr == 0.0 {
                    1.0
                } else {
                    10f32.powf(-safe_gr / 20.0)
                };
                for (line, v) in self.safe_audio.iter_mut().zip(y.iter_mut()) {
                    let d = line.push(*v, safe_delay);
                    *v = if safe_gr == 0.0 { d } else { d * g };
                }
            }

            let mut out = [0f32; 2];
            for ch in 0..2 {
                // Only rounding can reach this: every stage above already holds the ceiling.
                let v = y[ch].clamp(-ceil_lin, ceil_lin);
                let ko = self.k_out[ch].tick(v as f64);
                self.acc.energy += ko * ko;
                let tpk = self.meter_tp[ch].push(v);
                self.acc.out[ch] = self.acc.out[ch].max(tpk);
                let total = gr[ch] + safe_gr;
                self.acc.gr[ch] = self.acc.gr[ch].max(total);
                self.acc.gr_max = self.acc.gr_max.max(total);
                self.gr_now[ch] = total;
                let dry = self.dry[ch].push(xg[ch], latency);
                // The input as it lines up with the output: delayed by the latency.
                self.acc.input = self.acc.input.max(dry.abs());
                let mut o = if s.delta { dry - v } else { v };
                match s.monitor {
                    Monitor::Normal => {}
                    Monitor::Unity => o /= self.gain,
                    Monitor::Match => o *= self.match_gain,
                }
                if bits > 0 {
                    o = self.dither[ch].process(o, bits, s.shaping);
                }
                out[ch] = o;
            }
            if self.match_gain != self.match_target {
                self.match_gain += (self.match_target - self.match_gain) * self.match_glide;
                if (self.match_gain - self.match_target).abs() < 1e-6 {
                    self.match_gain = self.match_target;
                }
            }
            left[i] = out[0];
            right[i] = out[1];
            self.acc.n += 1;
            if self.acc.n == self.col_len {
                self.finish_column();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::TAU;

    const FS: f32 = 48_000.0;

    fn tone(amp: f32, hz: f32, n: usize) -> Vec<f32> {
        (0..n)
            .map(|i| amp * (TAU * hz * i as f32 / FS).sin())
            .collect()
    }

    fn noise(n: usize, seed: u32) -> Vec<f32> {
        let mut s = seed.max(1);
        (0..n)
            .map(|_| {
                s ^= s << 13;
                s ^= s >> 17;
                s ^= s << 5;
                (s as f32 / u32::MAX as f32) * 2.0 - 1.0
            })
            .collect()
    }

    /// A drum-like test signal: decaying noise bursts over a low sine, with a spike.
    fn drums(n: usize) -> Vec<f32> {
        let nz = noise(n, 11);
        (0..n)
            .map(|i| {
                let beat = (i % 12_000) as f32 / FS;
                let env = (-beat * 30.0).exp();
                0.9 * env * nz[i] + 0.25 * (TAU * 55.0 * i as f32 / FS).sin()
            })
            .collect()
    }

    fn run(s: &LimSettings, l: &[f32], r: &[f32], block: usize) -> (Vec<f32>, Vec<f32>) {
        let mut t = Terminus::new(FS);
        t.set(s);
        t.reset();
        let (mut a, mut b) = (l.to_vec(), r.to_vec());
        for (x, y) in a.chunks_mut(block).zip(b.chunks_mut(block)) {
            t.process(x, y, None);
        }
        (a, b)
    }

    fn peak(x: &[f32]) -> f32 {
        x.iter().fold(0.0, |m, v| m.max(v.abs()))
    }

    /// The null test: at settings that do nothing the output is the input, delayed by the
    /// latency reported, to the bit — for every style, with and without true peak.
    #[test]
    fn a_limiter_with_nothing_to_do_is_an_exact_delay() {
        let src = tone(0.5, 997.0, 20_000); // −6 dBFS, ceiling −1
        let src_r = tone(0.3, 331.0, 20_000);
        for style in LimStyle::ALL {
            for tp in [false, true] {
                let s = LimSettings {
                    style,
                    true_peak: tp,
                    ..LimSettings::default()
                };
                let lat = latency_of(&s, FS);
                let (l, r) = run(&s, &src, &src_r, 256);
                for i in 0..src.len() {
                    let want = if i >= lat { src[i - lat] } else { 0.0 };
                    let want_r = if i >= lat { src_r[i - lat] } else { 0.0 };
                    assert_eq!(l[i], want, "{style:?} tp={tp} left at {i}");
                    assert_eq!(r[i], want_r, "{style:?} tp={tp} right at {i}");
                }
            }
        }
    }

    #[test]
    fn a_clipper_that_clips_nothing_is_an_exact_delay_too() {
        let src = tone(0.2, 440.0, 8_000);
        let s = LimSettings {
            clip: 0.5,
            ..LimSettings::default()
        };
        let lat = latency_of(&s, FS);
        assert_eq!(lat, latency_of(&LimSettings::default(), FS) + 2 * os::DELAY);
        let (l, _) = run(&s, &src, &src, 100);
        for i in lat..src.len() {
            assert_eq!(l[i], src[i - lat]);
        }
    }

    #[test]
    fn the_result_does_not_depend_on_the_block_size() {
        let src = drums(30_000);
        let src_r: Vec<f32> = src.iter().rev().copied().collect();
        for style in LimStyle::ALL {
            let s = LimSettings {
                style,
                gain_db: 12.0,
                clip: 0.3,
                knee_db: 2.0,
                link_transients: 0.5,
                dc_filter: true,
                dither: 5,
                shaping: Shaping::Optimized,
                monitor: Monitor::Match,
                ..LimSettings::default()
            };
            let reference = run(&s, &src, &src_r, 4096);
            for block in [1usize, 7, 64, 1000] {
                assert!(
                    run(&s, &src, &src_r, block) == reference,
                    "{style:?} at block {block}"
                );
            }
        }
    }

    #[test]
    fn it_is_deterministic_and_finite_on_nasty_input() {
        let mut src = noise(20_000, 5);
        for (i, x) in src.iter_mut().enumerate() {
            *x *= if i % 5_000 < 2_500 { 40.0 } else { 1e-30 };
        }
        let s = LimSettings {
            gain_db: 30.0,
            lookahead_ms: 0.0,
            ..LimSettings::default()
        };
        let a = run(&s, &src, &src, 333);
        let b = run(&s, &src, &src, 333);
        assert!(a == b);
        assert!(a.0.iter().all(|x| x.is_finite()));
    }

    /// Sample-peak mode: no sample ever passes the ceiling, at any look-ahead and style,
    /// on material pushed 20 dB into it.
    #[test]
    fn no_sample_passes_the_ceiling() {
        let src = drums(40_000);
        let src_r = noise(40_000, 9);
        for style in LimStyle::ALL {
            for look in [0.0, 0.3, 2.0, 5.0] {
                let s = LimSettings {
                    style,
                    gain_db: 20.0,
                    ceiling_db: -3.0,
                    lookahead_ms: look,
                    true_peak: false,
                    ..LimSettings::default()
                };
                let (l, r) = run(&s, &src, &src_r, 512);
                let c = 10f32.powf(-3.0 / 20.0);
                assert!(peak(&l) <= c && peak(&r) <= c, "{style:?} {look} ms");
            }
        }
    }

    /// Material built to make inter-sample peaks, band-limited to 0.44·fs: a quarter-rate
    /// sine at 45°, high sines at odd phases, a band-limited square, noise bursts.
    pub(crate) fn isp_corpus(n: usize) -> Vec<(&'static str, Vec<f32>, Vec<f32>)> {
        let fs = FS as f64;
        let pi = std::f64::consts::PI;
        let fade = |x: Vec<f32>| os::tests::faded(x, 2_000);
        let sine = |hz: f64, ph: f64, amp: f64| -> Vec<f32> {
            (0..n)
                .map(|i| (amp * (2.0 * pi * hz * i as f64 / fs + ph).sin()) as f32)
                .collect()
        };
        let quarter = fade(sine(fs / 4.0, pi / 4.0, 0.7));
        let mut high = vec![0f32; n];
        for (k, (hz, ph)) in [(19_300.0, 0.3), (15_100.0, 1.9), (11_025.0, 0.77)]
            .iter()
            .enumerate()
        {
            let s = sine(*hz, *ph, 0.3);
            for (i, v) in s.iter().enumerate() {
                // Bursts, so the gain moves: each tone swells in and out at its own rate.
                let env = (0.5 - 0.5 * (2.0 * pi * (3.0 + k as f64) * i as f64 / fs).cos()) as f32;
                high[i] += v * env;
            }
        }
        let high = fade(high);
        let mut square = vec![0f32; n];
        let f0 = 1_733.0;
        let mut h = 1.0;
        while f0 * h < 0.44 * fs {
            let s = sine(f0 * h, 0.0, 0.5 / h);
            for (a, b) in square.iter_mut().zip(s) {
                *a += b;
            }
            h += 2.0;
        }
        let square = fade(square);
        // Noise bursts through a steep low-pass at 0.44·fs (a windowed-sinc FIR).
        let raw = noise(n, 3);
        let taps: Vec<f64> = (-40i32..=40)
            .map(|j| {
                let t = j as f64;
                let c = 0.44 * 2.0;
                let sinc = if j == 0 {
                    c
                } else {
                    (pi * c * t).sin() / (pi * t)
                };
                sinc * (0.5 + 0.5 * (pi * t / 41.0).cos())
            })
            .collect();
        let mut bursts = vec![0f32; n];
        for i in 40..n - 40 {
            let mut v = 0f64;
            for (k, c) in taps.iter().enumerate() {
                v += raw[i + k - 40] as f64 * c;
            }
            let beat = (i % 9_000) as f32 / FS;
            bursts[i] = v as f32 * 0.8 * (-beat * 25.0).exp();
        }
        let bursts = fade(bursts);
        vec![
            ("quarter-rate sine at 45°", quarter.clone(), quarter),
            ("high sines", high.clone(), square.clone()),
            ("band-limited square", square.clone(), high),
            (
                "noise bursts",
                bursts.clone(),
                bursts.iter().rev().copied().collect(),
            ),
        ]
    }

    /// True-peak limiting holds: on the corpus above pushed 6–18 dB into a −1 dBTP ceiling,
    /// every style and look-ahead, the output's true peak measured by an independent 32×
    /// reconstruction never passes the ceiling. The worst case is printed (`--nocapture`).
    #[test]
    fn true_peak_never_passes_the_ceiling_by_a_32x_reference() {
        let ceiling = -1.0f32;
        let mut worst = f32::NEG_INFINITY;
        let mut worst_at = String::new();
        for (name, l, r) in isp_corpus(16_000) {
            for (style, look, gain) in [
                (LimStyle::Clean, 2.0, 12.0),
                (LimStyle::Loud, 0.0, 18.0),
                (LimStyle::Punch, 0.5, 6.0),
                (LimStyle::Safe, 5.0, 12.0),
                (LimStyle::Bus, 1.0, 9.0),
            ] {
                let s = LimSettings {
                    style,
                    lookahead_ms: look,
                    gain_db: gain,
                    ceiling_db: ceiling,
                    true_peak: true,
                    ..LimSettings::default()
                };
                let (a, b) = run(&s, &l, &r, 512);
                for out in [&a, &b] {
                    let tp = 20.0 * os::reference_true_peak(out, 0.5).log10();
                    if tp > worst {
                        worst = tp;
                        worst_at = format!("{name}, {style:?}, {look} ms, +{gain} dB");
                    }
                }
            }
        }
        eprintln!(
            "worst true peak {:+.3} dB re ceiling ({worst_at})",
            worst - ceiling
        );
        assert!(
            worst <= ceiling,
            "true peak {worst} dBTP over the ceiling ({worst_at})"
        );
    }

    #[test]
    fn the_latency_reported_is_the_latency_measured() {
        for (look, tp, clip) in [
            (0.0, false, 0.0),
            (1.0, false, 0.0),
            (5.0, true, 0.0),
            (2.0, true, 0.4),
        ] {
            let s = LimSettings {
                lookahead_ms: look,
                true_peak: tp,
                clip,
                ..LimSettings::default()
            };
            let mut x = vec![0.0f32; 4_000];
            x[100] = 0.5;
            let (l, _) = run(&s, &x, &x, 64);
            let at = l.iter().position(|v| *v != 0.0).unwrap();
            assert_eq!(
                at - 100,
                latency_of(&s, FS),
                "{look} ms tp={tp} clip={clip}"
            );
        }
    }

    /// Settled on a steady tone pushed 10 dB over, the output sits at the ceiling.
    #[test]
    fn a_steady_overload_settles_at_the_ceiling() {
        let s = LimSettings {
            gain_db: 10.0,
            ceiling_db: -1.0,
            ..LimSettings::default()
        };
        let src = tone(0.5, 200.0, 96_000);
        let (l, _) = run(&s, &src, &src, 512);
        let p = 20.0 * peak(&l[72_000..]).log10();
        assert!(p <= -1.0 && p > -1.3, "settled at {p} dB");
    }

    /// Each stage has its own link: a peak on the left alone, unlinked, leaves the right
    /// untouched; linked, both move.
    #[test]
    fn linking_is_per_stage() {
        let n = 24_000;
        let left: Vec<f32> = (0..n)
            .map(|i| {
                if (6_000..6_400).contains(&i) {
                    0.99
                } else {
                    0.1
                }
            })
            .collect();
        let right = vec![0.1f32; n];
        let base = LimSettings {
            gain_db: 6.0,
            ceiling_db: -1.0,
            attack_ms: 1.0,
            true_peak: false,
            style: LimStyle::Punch,
            ..LimSettings::default()
        };
        let unlinked = LimSettings {
            link_transients: 0.0,
            link_release: 0.0,
            ..base
        };
        let (_, r) = run(&unlinked, &left, &right, 256);
        let g = 10f32.powf(6.0 / 20.0) * 0.1;
        assert!(
            r.iter().skip(1_000).all(|v| (v - g).abs() < 1e-5),
            "right moved"
        );
        let linked = LimSettings {
            link_transients: 1.0,
            link_release: 1.0,
            ..base
        };
        let (_, r) = run(&linked, &left, &right, 256);
        let dip = r[6_000..7_000].iter().fold(1f32, |m, v| m.min(*v));
        assert!(dip < g * 0.8, "linked right did not move");
        // Transients linked, release unlinked: the right dips only for as long as the
        // transient stage holds, then comes back while the left is still recovering.
        let split = LimSettings {
            link_transients: 1.0,
            link_release: 0.0,
            ..base
        };
        let (l, r) = run(&split, &left, &right, 256);
        let lat = latency_of(&split, FS);
        let after = 6_400 + lat + 4_800; // 100 ms after the peak ends
        assert!((r[after] - g).abs() < 0.01 * g, "right still held");
        assert!(l[after] < g * 0.99, "left released too soon");
    }

    /// The styles are audibly different: on a sustained 100 Hz tone pushed 8 dB over, the
    /// distortion they add is ordered as the table says — Safe the cleanest, Loud the most.
    #[test]
    fn styles_differ_in_the_order_the_table_gives() {
        let src = tone(0.5, 100.0, 96_000);
        let thd = |style: LimStyle| {
            let s = LimSettings {
                style,
                gain_db: 8.0,
                ceiling_db: -1.0,
                true_peak: false,
                ..LimSettings::default()
            };
            let (l, _) = run(&s, &src, &src, 512);
            // Fit the fundamental over the last second and measure what is left.
            let tail = &l[48_000..];
            let (mut c, mut sn) = (0f64, 0f64);
            for (i, v) in tail.iter().enumerate() {
                let ph = TAU as f64 * 100.0 * (i + 48_000) as f64 / FS as f64;
                c += *v as f64 * ph.cos();
                sn += *v as f64 * ph.sin();
            }
            let n = tail.len() as f64;
            let (c, sn) = (2.0 * c / n, 2.0 * sn / n);
            let mut resid = 0f64;
            let mut total = 0f64;
            for (i, v) in tail.iter().enumerate() {
                let ph = TAU as f64 * 100.0 * (i + 48_000) as f64 / FS as f64;
                let fit = c * ph.cos() + sn * ph.sin();
                resid += (*v as f64 - fit).powi(2);
                total += (*v as f64).powi(2);
            }
            10.0 * (resid / total).log10()
        };
        let safe = thd(LimStyle::Safe);
        let clean = thd(LimStyle::Clean);
        let punch = thd(LimStyle::Punch);
        let loud = thd(LimStyle::Loud);
        let bus = thd(LimStyle::Bus);
        eprintln!(
            "residual re fundamental, 100 Hz +8 dB: safe {safe:.1}, clean {clean:.1}, punch {punch:.1}, bus {bus:.1}, loud {loud:.1} dB"
        );
        assert!(
            safe < clean && clean < punch && punch < loud,
            "THD (dB): safe {safe:.1}, clean {clean:.1}, punch {punch:.1}, loud {loud:.1}"
        );
        assert!(loud - safe > 10.0, "Loud and Safe barely differ");
    }

    #[test]
    fn the_knee_starts_under_the_ceiling_and_never_passes_it() {
        assert_eq!(reduction_db(-10.0, -1.0, 6.0), 0.0);
        assert!(reduction_db(-2.0, -1.0, 6.0) > 0.0, "inside the knee");
        for i in 0..200 {
            let l = -12.0 + i as f32 * 0.1;
            assert!(transfer_db(l, -1.0, 6.0) <= -1.0 + 1e-6);
        }
        assert!((transfer_db(5.0, -1.0, 6.0) - -1.0).abs() < 1e-6);
        assert_eq!(reduction_db(-1.5, -1.0, 0.0), 0.0);
    }

    #[test]
    fn unity_gain_hears_the_limiting_at_the_level_it_came_in() {
        let src = tone(0.3, 300.0, 48_000);
        let s = LimSettings {
            gain_db: 12.0,
            monitor: Monitor::Unity,
            true_peak: false,
            ..LimSettings::default()
        };
        let (l, _) = run(&s, &src, &src, 512);
        // 12 dB up, limited to −1 dBFS (0.891), then 12 dB down: 0.891 / 3.98 = 0.224.
        let p = peak(&l[24_000..]);
        assert!((p - 0.891 / 3.981).abs() < 0.01, "{p}");
    }

    #[test]
    fn match_levels_the_output_to_the_input_loudness() {
        let src = drums(48_000 * 8);
        let s = LimSettings {
            gain_db: 12.0,
            monitor: Monitor::Match,
            ..LimSettings::default()
        };
        let (l, r) = run(&s, &src, &src, 512);
        let mut a = loudness::LoudnessMeter::new(FS);
        let mut b = loudness::LoudnessMeter::new(FS);
        a.process(&src[48_000 * 5..], &src[48_000 * 5..]);
        b.process(&l[48_000 * 5..], &r[48_000 * 5..]);
        let (i_in, i_out) = (a.integrated().unwrap(), b.integrated().unwrap());
        assert!(
            (i_in - i_out).abs() < 0.5,
            "in {i_in:.2} out {i_out:.2} LUFS"
        );
    }

    #[test]
    fn delta_is_what_the_limiter_took_away() {
        let src = drums(20_000);
        let base = LimSettings {
            gain_db: 9.0,
            ..LimSettings::default()
        };
        let (out, _) = run(&base, &src, &src, 256);
        let (delta, _) = run(
            &LimSettings {
                delta: true,
                ..base
            },
            &src,
            &src,
            256,
        );
        let lat = latency_of(&base, FS);
        let g = 10f32.powf(9.0 / 20.0);
        for i in lat..src.len() {
            assert!((out[i] + delta[i] - src[i - lat] * g).abs() < 1e-5);
        }
    }

    #[test]
    fn the_dc_filter_removes_an_offset() {
        let src: Vec<f32> = tone(0.2, 440.0, 96_000).iter().map(|x| x + 0.3).collect();
        let s = LimSettings {
            dc_filter: true,
            ..LimSettings::default()
        };
        let (l, _) = run(&s, &src, &src, 512);
        let mean: f32 = l[48_000..].iter().sum::<f32>() / 48_000.0;
        assert!(mean.abs() < 1e-3, "{mean}");
    }

    /// The power of `err / q` below 3 kHz, as a share of what white noise of variance 1
    /// would put there — so flat TPDF (variance ¼) reads ¼.
    fn band_power(err: &[f32], q: f32) -> f32 {
        // RBJ low-pass, Q = 0.54 and 1.31: a fourth-order Butterworth at 3 kHz.
        let section = |qf: f64| {
            let w = std::f64::consts::TAU * 3_000.0 / FS as f64;
            let (c, sn) = (w.cos(), w.sin());
            let al = sn / (2.0 * qf);
            let a0 = 1.0 + al;
            (
                [(1.0 - c) / 2.0 / a0, (1.0 - c) / a0, (1.0 - c) / 2.0 / a0],
                [-2.0 * c / a0, (1.0 - al) / a0],
            )
        };
        let run = |x: &[f64]| {
            let mut y = x.to_vec();
            for qf in [0.5412, 1.3066] {
                let (b, a) = section(qf);
                let (mut z1, mut z2) = (0.0, 0.0);
                for v in y.iter_mut() {
                    let o = b[0] * *v + z1;
                    z1 = b[1] * *v - a[0] * o + z2;
                    z2 = b[2] * *v - a[1] * o;
                    *v = o;
                }
            }
            y
        };
        let x: Vec<f64> = err.iter().map(|e| (*e / q) as f64).collect();
        let y = run(&x);
        let p = y[2_000..].iter().map(|v| v * v).sum::<f64>() / (y.len() - 2_000) as f64;
        // The filter's share of unit white noise, measured the same way.
        let mut s = 7u32;
        let white: Vec<f64> = (0..x.len())
            .map(|_| {
                s ^= s << 13;
                s ^= s >> 17;
                s ^= s << 5;
                ((s as f64 / u32::MAX as f64) * 2.0 - 1.0) * 3f64.sqrt()
            })
            .collect();
        let w = run(&white);
        let pw = w[2_000..].iter().map(|v| v * v).sum::<f64>() / (w.len() - 2_000) as f64;
        (p / pw) as f32
    }

    /// Dither: off is a wire (the null test above); on, the error is TPDF at the word length
    /// (2 LSB peak to peak, 1/√6·… RMS about 0.58 LSB flat), shaped noise is lower in the
    /// band the ear hears best, and digital silence stays silent.
    #[test]
    fn dither_quantises_with_the_noise_it_promises() {
        let src = tone(0.25, 1_000.0, 48_000);
        for (shaping, lo_band_vs_flat) in [
            (Shaping::Flat, 0.0),
            (Shaping::Basic, -6.0),
            (Shaping::Optimized, -10.0),
        ] {
            let s = LimSettings {
                dither: 5, // 16 bits
                shaping,
                ..LimSettings::default()
            };
            let (l, _) = run(&s, &src, &src, 512);
            let lat = latency_of(&s, FS);
            let q = 2f32.powi(-15);
            // Every sample is on the 16-bit grid.
            assert!(l.iter().all(|v| ((v / q) - (v / q).round()).abs() < 1e-3));
            let err: Vec<f32> = (lat..src.len()).map(|i| l[i] - src[i - lat]).collect();
            let rms = (err.iter().map(|e| e * e).sum::<f32>() / err.len() as f32).sqrt() / q;
            if shaping == Shaping::Flat {
                // TPDF: uniform rounding (1/12) plus triangular dither (1/6) = 1/4 LSB².
                assert!((rms - 0.5).abs() < 0.05, "flat RMS {rms} LSB");
            }
            // Noise below 3 kHz, relative to flat: the error through a steep low-pass
            // (two Butterworth sections), against the same filter's share of white noise.
            let lo_db = 10.0 * (band_power(&err, q) / 0.25).log10();
            assert!(
                lo_db < lo_band_vs_flat + 1.0,
                "{shaping:?}: noise under 3 kHz {lo_db:.1} dB vs flat"
            );
        }
        let silence = vec![0.0f32; 4_000];
        let s = LimSettings {
            dither: 5,
            ..LimSettings::default()
        };
        let (l, _) = run(&s, &silence, &silence, 64);
        assert!(l[BLANK_AFTER as usize..].iter().all(|v| *v == 0.0));
    }

    #[test]
    fn the_clipper_takes_the_peaks_it_is_given() {
        let src = drums(24_000);
        let base = LimSettings {
            gain_db: 14.0,
            true_peak: false,
            ..LimSettings::default()
        };
        let mut t = Terminus::new(FS);
        t.set(&LimSettings { clip: 1.0, ..base });
        let (mut l, mut r) = (src.clone(), src.clone());
        t.process(&mut l, &mut r, None);
        let c = 10f32.powf(-1.0 / 20.0);
        assert!(peak(&l) <= c);
        // The telemetry saw it clip.
        let tel = t.telemetry();
        let seq = tel[T_SEQ] as u32;
        let clipped = (seq.saturating_sub(T_COLUMNS as u32)..seq)
            .map(|k| Column::read(&tel, k).clip_db)
            .fold(0f32, f32::max);
        assert!(clipped > 0.5, "clipper reported {clipped} dB");
    }

    /// How long a second of stereo takes, with everything on (true peak, clipper, dither,
    /// match). `cargo test --release -p cantus-dsp terminus_speed -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn terminus_speed() {
        let src = drums(48_000 * 10);
        // The parts, for scale: one true-peak detector, one K-weighting, per channel.
        let mut tp = os::TruePeak::new();
        let start = std::time::Instant::now();
        let mut m = 0f32;
        for x in &src {
            m = m.max(tp.push(*x));
        }
        eprintln!(
            "one true-peak detector: {:.0} ns per sample ({m})",
            start.elapsed().as_secs_f64() * 1e9 / src.len() as f64
        );
        let mut k = KWeighting::new(FS);
        let start = std::time::Instant::now();
        let mut e = 0f64;
        for x in &src {
            let y = k.tick(*x as f64);
            e += y * y;
        }
        eprintln!(
            "one K-weighting: {:.0} ns per sample ({e})",
            start.elapsed().as_secs_f64() * 1e9 / src.len() as f64
        );
        let all = LimSettings {
            gain_db: 10.0,
            clip: 0.3,
            dither: 5,
            shaping: Shaping::Optimized,
            monitor: Monitor::Match,
            ..LimSettings::default()
        };
        for (name, s) in [
            ("everything on", all),
            (
                "defaults, +10 dB",
                LimSettings {
                    gain_db: 10.0,
                    ..LimSettings::default()
                },
            ),
            (
                "sample peak, +10 dB",
                LimSettings {
                    gain_db: 10.0,
                    true_peak: false,
                    ..LimSettings::default()
                },
            ),
            ("nothing to do", LimSettings::default()),
        ] {
            let mut t = Terminus::new(FS);
            t.set(&s);
            let (mut l, mut r) = (src.clone(), src.clone());
            if name == "nothing to do" {
                l.iter_mut().chain(r.iter_mut()).for_each(|x| *x *= 0.3);
            }
            let start = std::time::Instant::now();
            for (a, b) in l.chunks_mut(256).zip(r.chunks_mut(256)) {
                t.process(a, b, None);
            }
            let secs = start.elapsed().as_secs_f64();
            eprintln!(
                "{name}: 10 s of stereo in {:.1} ms, {:.2} % of one core, {:.0} ns per frame",
                secs * 1e3,
                secs / 10.0 * 100.0,
                secs * 1e9 / src.len() as f64
            );
        }
    }

    #[test]
    fn telemetry_counts_columns_from_the_start_of_the_stream() {
        let mut t = Terminus::new(FS);
        let (mut l, mut r) = (tone(0.9, 50.0, 4_800), tone(0.9, 50.0, 4_800));
        t.process(&mut l, &mut r, None);
        let tel = t.telemetry();
        assert_eq!(tel[T_SEQ], 10.0, "ten 10 ms columns in 100 ms");
        assert_eq!(tel[T_LATENCY], t.latency() as f32);
        let c = Column::read(&tel, 9);
        assert!(c.input_db > -1.5 && c.gr_db > 0.0);
    }
}

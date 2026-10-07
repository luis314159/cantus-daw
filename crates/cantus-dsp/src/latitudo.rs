//! Latitudo: stereo control and mid/side, for mixing and for the master.
//!
//! Width, mid and side gain, balance, rotation, swap and polarity; mono below a frequency
//! through a phase-coherent crossover; width in three bands; a mid/side tone section (a
//! low and a high shelf on the mid and on the side — FL Patcher's mid/side preset, built
//! in); Haas widening, classic or mono-safe; and the meters a stereo decision needs: a
//! vectorscope's points, correlation (overall and per band) and balance, before and after.
//!
//! ## The signal path, in order
//!
//! 1. **Polarity and swap**, on the left and right inputs (`P`, a 2×2 matrix).
//! 2. **Haas, classic**: one channel delayed by up to [`HAAS_MAX_MS`]. Wide, and it combs
//!    when summed to mono — the editor says where.
//! 3. **Into mid and side**: `M = (L + R) / 2`, `S = (L − R) / 2`.
//! 4. **Haas, mono-safe**: the mid, delayed, is *added to the side*. `L + R` is `2M`
//!    whatever the side holds, so the mono sum is untouched: a Haas width that cannot
//!    comb in mono.
//! 5. **Tone**: a low and a high shelf on the mid, and on the side (Butterworth shelves of
//!    6 or 12 dB/oct, no overshoot either way).
//! 6. **Width by band**: the side split at two Linkwitz–Riley crossovers (24 dB/oct), each
//!    band scaled; the mid goes through the same all-passes, so mid and side stay in
//!    phase at every frequency and the image does not smear around the crossovers.
//! 7. **Mono below**: the side's low band removed at a Linkwitz–Riley crossover of 6, 12,
//!    24 or 48 dB/oct, the mid through its all-pass — again in phase: the bass is mono
//!    and nothing above it moves in the image. (At 6 dB/oct the split is complementary
//!    and the mid is not touched at all: a gentle side high-pass.)
//! 8. **Out of mid and side** with the mid and side gains and the width, what is heard
//!    (stereo, the mid alone, the side alone), then **rotation**, **balance** and the
//!    **output** gain — all one 2×2 matrix `Q`.
//!
//! When no stage of 2–7 is working, the whole effect is one 2×2 matrix on left and right,
//! `T = Q·E·P`; when that matrix is the identity — every default — the block is not
//! touched at all. **That is the null rule: at its defaults Latitudo returns its input to
//! the bit**, and it has no latency at any setting.
//!
//! ## Glides, on a grid
//!
//! Every value glides (gains in 10 ms, frequencies in 20 ms on a log scale, the Haas
//! delay in 40 ms); the glides step every [`UPDATE`] samples counted from the start of the
//! stream, and the matrices are interpolated linearly sample by sample between grid
//! points. A stage that starts or stops, or changes its structure (a slope), fades over
//! 10 ms, so nothing clicks. Everything is per sample from the stream's start, so the
//! output never depends on the block size.

use crate::multiplex::filters::{OnePole, Shelf, Slope, Xover, XoverCoefs};

#[path = "latitudo_run.rs"]
mod run;
#[path = "latitudo_view.rs"]
pub mod view;

pub use view::{band_width, tone_db, width_at};

/// Glides and coefficients step every this many samples, counted from the stream's start.
pub const UPDATE: usize = 16;
/// The longest Haas delay.
pub const HAAS_MAX_MS: f32 = 30.0;
/// `mono` at or below this is off (the parameter's floor).
pub const MONO_OFF_HZ: f32 = 20.0;
/// Vectorscope points in the telemetry.
pub const POINTS: usize = 48;
/// Points per second the vectorscope is sampled at (about 48 a frame at 60 Hz).
pub const POINT_RATE: f32 = 2_880.0;
/// The bands' crossovers never come closer than this ratio.
pub const MIN_BAND_RATIO: f32 = 1.5;

const GAIN_MS: f32 = 10.0;
const FREQ_MS: f32 = 20.0;
const HAAS_MS: f32 = 40.0;
const FADE_MS: f32 = 10.0;
/// The meters' integration time (correlation, balance, levels).
pub const METER_MS: f32 = 300.0;

/// What is heard.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Listen {
    /// The result.
    Stereo,
    /// The mid alone, in both speakers: the mono check.
    Mid,
    /// The side alone, in both speakers (in phase, so it can be judged as a sound).
    Side,
}

impl Listen {
    pub const ALL: [Listen; 3] = [Listen::Stereo, Listen::Mid, Listen::Side];
    pub fn from_index(i: usize) -> Listen {
        Self::ALL[i.min(Self::ALL.len() - 1)]
    }
    pub fn index(self) -> usize {
        self as usize
    }
    /// The listen matrix `[λ00, λ01, λ11]` on (mid, side): mid' = λ00 M + λ01 S,
    /// side' = λ11 S.
    fn matrix(self) -> [f32; 3] {
        match self {
            Listen::Stereo => [1.0, 0.0, 1.0],
            Listen::Mid => [1.0, 0.0, 0.0],
            Listen::Side => [0.0, 1.0, 0.0],
        }
    }
}

/// How the Haas delay widens.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum HaasMode {
    /// The right channel is late: the image leans left, and mono combs.
    DelayRight,
    /// The left channel is late: the image leans right, and mono combs.
    DelayLeft,
    /// The delayed mid is added to the side: wide, and the mono sum is untouched.
    MonoSafe,
}

impl HaasMode {
    pub const ALL: [HaasMode; 3] = [
        HaasMode::DelayRight,
        HaasMode::DelayLeft,
        HaasMode::MonoSafe,
    ];
    pub fn from_index(i: usize) -> HaasMode {
        Self::ALL[i.min(Self::ALL.len() - 1)]
    }
    pub fn index(self) -> usize {
        self as usize
    }
}

/// The mono crossover's slopes, by the parameter's index.
pub const MONO_SLOPES: [Slope; 4] = [Slope::S6, Slope::S12, Slope::S24, Slope::S48];
/// The tone shelves' slopes, by the parameter's index.
pub const TONE_SLOPES: [Slope; 2] = [Slope::S6, Slope::S12];

/// Latitudo's settings, in the units a musician reads.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct StereoSettings {
    /// The side against the mid: 0 mono, 1 as it came, 2 twice as wide.
    pub width: f32,
    pub mid_db: f32,
    pub side_db: f32,
    /// −1 (left) … +1 (right): the far side is turned down linearly.
    pub balance: f32,
    /// Degrees; positive turns the image toward the right.
    pub rotation_deg: f32,
    pub swap: bool,
    pub invert_l: bool,
    pub invert_r: bool,
    pub listen: Listen,
    /// Mono below this; at or under [`MONO_OFF_HZ`], off.
    pub mono_hz: f32,
    pub mono_slope: Slope,
    pub x_lo: f32,
    pub x_hi: f32,
    pub w_lo: f32,
    pub w_mid: f32,
    pub w_hi: f32,
    pub mid_lo_hz: f32,
    pub mid_lo_db: f32,
    pub mid_hi_hz: f32,
    pub mid_hi_db: f32,
    pub side_lo_hz: f32,
    pub side_lo_db: f32,
    pub side_hi_hz: f32,
    pub side_hi_db: f32,
    pub tone_slope: Slope,
    /// The Haas delay, ms; 0 is off.
    pub haas_ms: f32,
    pub haas_mode: HaasMode,
    /// Mono-safe: how much delayed mid goes into the side (0 … 1).
    pub haas_amount: f32,
    pub out_db: f32,
}

impl Default for StereoSettings {
    /// Every default leaves the signal untouched.
    fn default() -> Self {
        StereoSettings {
            width: 1.0,
            mid_db: 0.0,
            side_db: 0.0,
            balance: 0.0,
            rotation_deg: 0.0,
            swap: false,
            invert_l: false,
            invert_r: false,
            listen: Listen::Stereo,
            mono_hz: MONO_OFF_HZ,
            mono_slope: Slope::S24,
            x_lo: 250.0,
            x_hi: 4_000.0,
            w_lo: 1.0,
            w_mid: 1.0,
            w_hi: 1.0,
            mid_lo_hz: 150.0,
            mid_lo_db: 0.0,
            mid_hi_hz: 8_000.0,
            mid_hi_db: 0.0,
            side_lo_hz: 150.0,
            side_lo_db: 0.0,
            side_hi_hz: 8_000.0,
            side_hi_db: 0.0,
            tone_slope: Slope::S6,
            haas_ms: 0.0,
            haas_mode: HaasMode::DelayRight,
            haas_amount: 0.5,
            out_db: 0.0,
        }
    }
}

impl StereoSettings {
    /// Whether the mono crossover works.
    pub fn mono_on(&self) -> bool {
        self.mono_hz > MONO_OFF_HZ + 0.01
    }
    /// Whether any band's width differs from 100 %.
    pub fn bands_on(&self) -> bool {
        self.w_lo != 1.0 || self.w_mid != 1.0 || self.w_hi != 1.0
    }
    /// Whether any shelf has a gain.
    pub fn tone_on(&self) -> bool {
        self.mid_lo_db != 0.0
            || self.mid_hi_db != 0.0
            || self.side_lo_db != 0.0
            || self.side_hi_db != 0.0
    }
    pub fn haas_on(&self) -> bool {
        self.haas_ms > 0.0
    }
    /// The bands' crossovers, kept apart by [`MIN_BAND_RATIO`] (the higher one gives way).
    pub fn crossovers(&self) -> (f32, f32) {
        let lo = self.x_lo.max(1.0);
        (lo, self.x_hi.max(lo * MIN_BAND_RATIO))
    }
    /// The first frequency where a classic Haas delay cancels in mono, Hz (the notches
    /// then repeat every twice that); `None` when there is no comb.
    pub fn mono_comb_hz(&self) -> Option<f32> {
        (self.haas_on() && self.haas_mode != HaasMode::MonoSafe)
            .then(|| 1_000.0 / (2.0 * self.haas_ms))
    }
}

/// A value that glides toward its target, stepped once per grid point, and lands on it
/// exactly.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
struct Glide {
    v: f32,
    t: f32,
}

impl Glide {
    fn at(v: f32) -> Glide {
        Glide { v, t: v }
    }
    #[inline]
    fn step(&mut self, a: f32) {
        if self.v != self.t {
            let v = self.v + (self.t - self.v) * a;
            // Land when close, or when the step no longer moves the value (it would
            // hover a few ulps away for ever).
            self.v = if v == self.v || (self.t - v).abs() <= 1e-6 * self.t.abs().max(1.0) {
                self.t
            } else {
                v
            };
        }
    }
    #[inline]
    fn moving(&self) -> bool {
        self.v != self.t
    }
}

/// A stage's presence, 0 … 1, moving linearly over [`FADE_MS`] sample by sample.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
struct Fade {
    v: f32,
    t: f32,
}

impl Fade {
    #[inline]
    fn tick(&mut self, step: f32) {
        if self.v < self.t {
            self.v = (self.v + step).min(self.t);
        } else if self.v > self.t {
            self.v = (self.v - step).max(self.t);
        }
    }
    #[inline]
    fn live(&self) -> bool {
        self.v > 0.0 || self.t > 0.0
    }
}

/// The one-pole share of a glide over `ms`, for a step of [`UPDATE`] samples.
fn grid_share(ms: f32, sr: f32) -> f32 {
    let tau = (ms * 0.001 * sr).max(1e-3);
    1.0 - (-(UPDATE as f32) / tau).exp()
}

/// `10^(db/20)`, exactly 1 at 0 dB.
#[inline]
pub fn db_to_gain(db: f32) -> f32 {
    if db == 0.0 {
        1.0
    } else {
        10f32.powf(db / 20.0)
    }
}

/// Four values that move linearly from grid point to grid point, sample by sample.
#[derive(Copy, Clone, Debug, PartialEq)]
struct Ramp {
    cur: [f32; 4],
    next: [f32; 4],
    inc: [f32; 4],
}

impl Ramp {
    fn at(v: [f32; 4]) -> Ramp {
        Ramp {
            cur: v,
            next: v,
            inc: [0.0; 4],
        }
    }
    /// A new end for the coming grid step (called on the grid, where `cur == next`).
    fn retarget(&mut self, next: [f32; 4]) {
        self.cur = self.next;
        self.next = next;
        self.inc = std::array::from_fn(|i| (next[i] - self.cur[i]) / UPDATE as f32);
    }
    /// Advance to sample `j` of the grid step: the last sample lands on `next` exactly.
    #[inline]
    fn step(&mut self, j: usize) {
        if self.cur != self.next {
            if j + 1 == UPDATE {
                self.cur = self.next;
            } else {
                for i in 0..4 {
                    self.cur[i] += self.inc[i];
                }
            }
        }
    }
    fn still(&self) -> bool {
        self.cur == self.next
    }
}

const IDENTITY: [f32; 4] = [1.0, 0.0, 0.0, 1.0];
/// `Q` at rest: out of mid and side, `L = M + S`, `R = M − S`.
const DECODE: [f32; 4] = [1.0, 1.0, 1.0, -1.0];

/// Latitudo's processing state. Everything sized is made in [`Latitudo::new`]; nothing
/// after it allocates.
#[derive(Clone, Debug)]
pub struct Latitudo {
    sr: f32,
    s: StereoSettings,
    /// Samples since the start of the stream.
    n: u64,
    a_gain: f32,
    a_freq: f32,
    a_haas: f32,
    fade_step: f32,
    // The glides.
    width: Glide,
    mid_g: Glide,
    side_g: Glide,
    bal: Glide,
    rot: Glide,
    swap: Glide,
    pol_l: Glide,
    pol_r: Glide,
    lis: [Glide; 3],
    out_g: Glide,
    w: [Glide; 3],
    /// The bands' crossovers and the mono crossover, log₂ Hz.
    x_lo: Glide,
    x_hi: Glide,
    mono_f: Glide,
    /// Mid low, mid high, side low, side high: gains (dB) and frequencies (log₂ Hz).
    tone_db: [Glide; 4],
    tone_f: [Glide; 4],
    /// The Haas delay, samples.
    haas_d: Glide,
    haas_amt: Glide,
    // The stages' presence, and the structure each runs with now.
    mono_fade: Fade,
    mono_slope: Slope,
    band_fade: Fade,
    tone_fade: Fade,
    tone_slope: Slope,
    haas_fade: Fade,
    haas_mode: HaasMode,
    // What the grid derives.
    /// The full mid/side path runs this grid step (else the matrix `t` alone).
    full: bool,
    p: Ramp,
    q: Ramp,
    t: Ramp,
    /// The bands' widths and the mono-safe amount, per sample.
    wr: Ramp,
    /// The Haas delay in samples, per sample (the first value).
    dr: Ramp,
    mono_c: XoverCoefs,
    band_c: [XoverCoefs; 2],
    /// Per shelf: the filter's `g`, its section gain, and the gain after it.
    shelf_c: [(f32, f32, f32); 4],
    shelf_live: [bool; 4],
    // Filter state.
    /// The mono crossover: the side's split, the mid's all-pass.
    x_mono: [Xover; 2],
    /// The bands: the side at the low crossover, its rest at the high one, the low band
    /// through the high one's all-pass, the mid through both all-passes.
    x_band: [Xover; 5],
    shelves: [Shelf; 4],
    // The Haas line: left and right after polarity and swap, always written.
    line: [Vec<f32>; 2],
    cap: usize,
    pos: usize,
    /// Zeros written into each line in a row (ADR 0031 §5.1).
    line_quiet: [crate::rest::ZeroRun; 2],
    // The meters.
    meter_c: f32,
    /// (LL, RR, LR) of the input and of the output, over [`METER_MS`].
    acc_in: [f32; 3],
    acc_out: [f32; 3],
    /// The same per band of the output (low, mid, high).
    acc_band: [[f32; 3]; 3],
    /// The meters' split: one-poles at the bands' crossovers, per channel.
    an: [OnePole; 4],
    an_g: [f32; 2],
    /// The last block's output peaks.
    peak: [f32; 2],
    pts: [f32; POINTS],
    pt_head: usize,
    pts_done: u32,
    stride: u64,
    /// The settings every glide has reached, when nothing moves: a grid point then has
    /// nothing to do.
    settled: Option<StereoSettings>,
}

impl Latitudo {
    pub fn new(sample_rate: f32) -> Self {
        let sr = sample_rate.max(1.0);
        let cap = (HAAS_MAX_MS * 0.001 * sr).ceil() as usize + 4;
        let s = StereoSettings::default();
        let mut l = Latitudo {
            sr,
            s,
            n: 0,
            a_gain: grid_share(GAIN_MS, sr),
            a_freq: grid_share(FREQ_MS, sr),
            a_haas: grid_share(HAAS_MS, sr),
            fade_step: 1.0 / (FADE_MS * 0.001 * sr).max(1.0),
            width: Glide::default(),
            mid_g: Glide::default(),
            side_g: Glide::default(),
            bal: Glide::default(),
            rot: Glide::default(),
            swap: Glide::default(),
            pol_l: Glide::default(),
            pol_r: Glide::default(),
            lis: [Glide::default(); 3],
            out_g: Glide::default(),
            w: [Glide::default(); 3],
            x_lo: Glide::default(),
            x_hi: Glide::default(),
            mono_f: Glide::default(),
            tone_db: [Glide::default(); 4],
            tone_f: [Glide::default(); 4],
            haas_d: Glide::default(),
            haas_amt: Glide::default(),
            mono_fade: Fade::default(),
            mono_slope: s.mono_slope,
            band_fade: Fade::default(),
            tone_fade: Fade::default(),
            tone_slope: s.tone_slope,
            haas_fade: Fade::default(),
            haas_mode: s.haas_mode,
            full: false,
            p: Ramp::at(IDENTITY),
            q: Ramp::at(DECODE),
            t: Ramp::at(IDENTITY),
            wr: Ramp::at([1.0; 4]),
            dr: Ramp::at([0.0; 4]),
            mono_c: XoverCoefs::new(s.mono_slope, MONO_OFF_HZ, sr),
            band_c: [
                XoverCoefs::new(Slope::S24, s.x_lo, sr),
                XoverCoefs::new(Slope::S24, s.x_hi, sr),
            ],
            shelf_c: [(0.0, 1.0, 1.0); 4],
            shelf_live: [false; 4],
            x_mono: [Xover::default(); 2],
            x_band: [Xover::default(); 5],
            shelves: [Shelf::default(); 4],
            line: [vec![0.0; cap], vec![0.0; cap]],
            cap,
            pos: 0,
            line_quiet: [crate::rest::ZeroRun::FULL; 2],
            meter_c: 1.0 - (-1.0 / (METER_MS * 0.001 * sr)).exp(),
            acc_in: [0.0; 3],
            acc_out: [0.0; 3],
            acc_band: [[0.0; 3]; 3],
            an: [OnePole::default(); 4],
            an_g: [0.0; 2],
            peak: [0.0; 2],
            pts: [0.0; POINTS],
            pt_head: 0,
            pts_done: 0,
            stride: ((sr / POINT_RATE).round() as u64).max(1),
            settled: None,
        };
        l.reset();
        l
    }

    pub fn sample_rate(&self) -> f32 {
        self.sr
    }

    pub fn settings(&self) -> &StereoSettings {
        &self.s
    }

    /// New settings: the glides head for them from the next grid point. Nothing here
    /// allocates (it runs on the audio thread).
    pub fn set(&mut self, s: &StereoSettings) {
        self.s = *s;
    }

    /// No latency, at any setting.
    pub fn latency(&self) -> usize {
        0
    }

    /// Back to the start: every glide on its target, every stage at its settled presence,
    /// every filter and the line empty, the meters at rest. No allocation.
    pub fn reset(&mut self) {
        let s = self.s;
        let lg = |hz: f32| Glide::at(hz.max(1.0).log2());
        let (x_lo, x_hi) = s.crossovers();
        self.n = 0;
        self.width = Glide::at(s.width);
        self.mid_g = Glide::at(db_to_gain(s.mid_db));
        self.side_g = Glide::at(db_to_gain(s.side_db));
        self.bal = Glide::at(s.balance);
        self.rot = Glide::at(s.rotation_deg);
        self.swap = Glide::at(if s.swap { 1.0 } else { 0.0 });
        self.pol_l = Glide::at(if s.invert_l { -1.0 } else { 1.0 });
        self.pol_r = Glide::at(if s.invert_r { -1.0 } else { 1.0 });
        self.lis = s.listen.matrix().map(Glide::at);
        self.out_g = Glide::at(db_to_gain(s.out_db));
        self.w = [s.w_lo, s.w_mid, s.w_hi].map(Glide::at);
        self.x_lo = lg(x_lo);
        self.x_hi = lg(x_hi);
        self.mono_f = lg(s.mono_hz);
        self.tone_db = [s.mid_lo_db, s.mid_hi_db, s.side_lo_db, s.side_hi_db].map(Glide::at);
        self.tone_f = [s.mid_lo_hz, s.mid_hi_hz, s.side_lo_hz, s.side_hi_hz].map(lg);
        self.haas_d = Glide::at(s.haas_ms.clamp(0.0, HAAS_MAX_MS) * self.sr / 1_000.0);
        self.haas_amt = Glide::at(s.haas_amount);
        let on = |b: bool| {
            let v = if b { 1.0 } else { 0.0 };
            Fade { v, t: v }
        };
        self.mono_fade = on(s.mono_on());
        self.mono_slope = s.mono_slope;
        self.band_fade = on(s.bands_on());
        self.tone_fade = on(true);
        self.tone_slope = s.tone_slope;
        self.haas_fade = on(s.haas_on());
        self.haas_mode = s.haas_mode;
        self.x_mono = [Xover::default(); 2];
        self.x_band = [Xover::default(); 5];
        self.shelves = [Shelf::default(); 4];
        for l in &mut self.line {
            l.fill(0.0);
        }
        self.line_quiet = [crate::rest::ZeroRun::FULL; 2];
        self.pos = 0;
        self.acc_in = [0.0; 3];
        self.acc_out = [0.0; 3];
        self.acc_band = [[0.0; 3]; 3];
        self.an = [OnePole::default(); 4];
        self.peak = [0.0; 2];
        self.pts = [view::pack_point(0.0, 0.0); POINTS];
        self.pt_head = 0;
        self.pts_done = 0;
        // Derive everything once, so the first grid step starts from where it settles.
        self.settled = None;
        self.derive();
        self.p = Ramp::at(self.p.next);
        self.q = Ramp::at(self.q.next);
        self.t = Ramp::at(self.t.next);
        self.wr = Ramp::at(self.wr.next);
        self.dr = Ramp::at(self.dr.next);
    }
}

impl Latitudo {
    /// At rest (ADR 0031 §5.1): every glide, fade and structure where [`Self::reset`] puts
    /// them for these settings, the per-sample ramps still, every filter empty and the
    /// Haas line all zeros. The meters (levels, correlation, the scope) are the editor's
    /// and do not count. Whether the grid says it has settled does not matter: a grid point
    /// with nothing moving changes nothing.
    pub fn at_rest(&self) -> bool {
        let s = self.s;
        let lg = |hz: f32| hz.max(1.0).log2();
        let at = |g: &Glide, v: f32| g.v == v && g.t == v;
        let on = |f: &Fade, b: bool| {
            let v = if b { 1.0 } else { 0.0 };
            f.v == v && f.t == v
        };
        let (x_lo, x_hi) = s.crossovers();
        let lis = s.listen.matrix();
        let glides = at(&self.width, s.width)
            && at(&self.mid_g, db_to_gain(s.mid_db))
            && at(&self.side_g, db_to_gain(s.side_db))
            && at(&self.bal, s.balance)
            && at(&self.rot, s.rotation_deg)
            && at(&self.swap, if s.swap { 1.0 } else { 0.0 })
            && at(&self.pol_l, if s.invert_l { -1.0 } else { 1.0 })
            && at(&self.pol_r, if s.invert_r { -1.0 } else { 1.0 })
            && (0..3).all(|i| at(&self.lis[i], lis[i]))
            && at(&self.out_g, db_to_gain(s.out_db))
            && at(&self.w[0], s.w_lo)
            && at(&self.w[1], s.w_mid)
            && at(&self.w[2], s.w_hi)
            && at(&self.x_lo, lg(x_lo))
            && at(&self.x_hi, lg(x_hi))
            && at(&self.mono_f, lg(s.mono_hz))
            && [s.mid_lo_db, s.mid_hi_db, s.side_lo_db, s.side_hi_db]
                .iter()
                .zip(&self.tone_db)
                .all(|(v, g)| at(g, *v))
            && [s.mid_lo_hz, s.mid_hi_hz, s.side_lo_hz, s.side_hi_hz]
                .iter()
                .zip(&self.tone_f)
                .all(|(v, g)| at(g, lg(*v)))
            && at(
                &self.haas_d,
                s.haas_ms.clamp(0.0, HAAS_MAX_MS) * self.sr / 1_000.0,
            )
            && at(&self.haas_amt, s.haas_amount);
        let stages = on(&self.mono_fade, s.mono_on())
            && on(&self.band_fade, s.bands_on())
            && on(&self.tone_fade, true)
            && on(&self.haas_fade, s.haas_on())
            && self.mono_slope == s.mono_slope
            && self.tone_slope == s.tone_slope
            && self.haas_mode == s.haas_mode;
        // The ramps always end on what the glides derive, so still ramps are where a reset
        // puts them.
        let ramps = self.p.still()
            && self.q.still()
            && self.t.still()
            && self.wr.still()
            && self.dr.still();
        glides
            && stages
            && ramps
            && self.x_mono.iter().all(Xover::is_clear)
            && self.x_band.iter().all(Xover::is_clear)
            && self.shelves.iter().all(Shelf::is_clear)
            && self.line_quiet.iter().all(|q| q.covers(self.cap))
    }

    /// Samples of silence skipped while at rest (ADR 0031 §5.1): the grid's position moves
    /// on, so a parameter that moves later glides on the same samples as it would have.
    pub fn skip(&mut self, frames: usize) {
        self.n += frames as u64;
    }
}

#[cfg(test)]
#[path = "latitudo_tests.rs"]
mod tests;

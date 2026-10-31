//! Comprimo: the compressor. A gain computer with a soft knee and a range; a detector you
//! can read (peak or RMS, feed-forward or feedback, one or two release stages, an
//! exponential or linear release); lookahead with its latency reported; hold, auto and
//! program-dependent release; stereo link; a key from this strip or any other, through a
//! level, a high-pass and a six-band EQ with its own bypass; wet gain, dry gain and a mix
//! to 200 %; input and output gain; a **measured** auto gain; and five ways to hear it —
//! the output, the wet path alone, the key, the difference it makes, and a bypass at the
//! output's loudness.
//!
//! ## Styles are detectors you can read
//!
//! The benchmark's styles are black boxes; its users ask for the detector controls instead
//! (`research/fabfilter/pro-c.md`). Here a style *is* a [`StyleSpec`] — five numbers the
//! editor prints — and the engine drawer edits them directly, which turns the style into
//! "Custom (from …)":
//!
//! | Style | Detector | Feedback | Release |
//! |---|---|---|---|
//! | Clean | peak | none | as set |
//! | Punch | peak | 40 % | as set |
//! | Opto | RMS 30 ms | none | slows as it works (×3) |
//! | Bus | RMS 10 ms | 20 % | as set |
//! | Vocal | RMS 3 ms | none | slows a little (×0.8) |
//! | Mastering | peak | none | a slow second stage holds 70 % |
//! | Smooth | RMS 50 ms | none | slows (×1), second stage 50 % |
//! | Pumping | peak | none | linear in dB: a steady swell back |
//! | Classic | RMS 5 ms | 80 % | slows (×1.5), 30 % linear |
//!
//! ## The null-test contract
//!
//! Gain reduction is computed in dB, and a reduction below 1e-6 dB is exactly zero. Every
//! gain that would be exactly 1 is *skipped*, not multiplied, so a compressor that is not
//! compressing — with no make-up, no input or output gain, no dry path — hands every
//! sample through untouched: to the bit, delayed by the lookahead if there is one. That
//! holds with auto gain on too: the loudness of the input and of the uncompressed wet path
//! are then measured on the same samples, so their difference is exactly zero.
//!
//! Everything runs per sample, and every slower clock (the 100 ms loudness and statistics
//! grid) counts samples from the start of the stream, so the output and the telemetry do
//! not depend on the block size.

use crate::math;

#[path = "comprimo_loudness.rs"]
pub mod loudness;
#[path = "comprimo_oversample.rs"]
pub mod oversample;

use oversample::Oversampler;

use crate::aequalis::{design, Coefs, EqShape, MAX_SECTIONS};

/// The longest lookahead, ms. Also what the delay lines are sized for.
pub const MAX_LOOKAHEAD_MS: f32 = 20.0;
/// The ratio parameter's top: drawn and treated as ∞:1.
pub const RATIO_INF: f32 = 100.0;
/// A dry gain at or below this is off.
pub const DRY_OFF_DB: f32 = -60.0;
/// Bands of the key's EQ.
pub const KEY_BANDS: usize = 6;
/// Steps of the 100 ms grid kept for the statistics: ten seconds.
pub const STAT_HOPS: usize = 100;
/// Momentary (400 ms) loudness values reported per telemetry read, newest first — enough
/// for the editor to miss 800 ms of frames and still integrate every block.
pub const BLOCKS_REPORTED: usize = 8;
/// The share of the input's short-term loudness below which auto gain and the
/// equal-loudness compare hold still (LUFS): nothing to match in near silence.
pub const MATCH_GATE_LUFS: f32 = -50.0;
/// How fast auto gain and the compare gain follow their measurement, seconds.
const MATCH_SMOOTH_S: f32 = 0.5;
/// They glide in steps of this many samples, counted from the stream's start: the glide
/// takes half a second, so the steps are far below hearing, and the gain is not
/// recomputed every sample.
const MATCH_STEP: usize = 16;
/// `ln(10) / 20`: dB to the exponent of a gain, so a gain is one `exp` rather than a
/// `powf`.
const DB_TO_LN: f32 = std::f32::consts::LN_10 / 20.0;

/// A gain of `db` decibels.
#[inline]
fn gain_of(db: f32) -> f32 {
    math::exp(db * DB_TO_LN)
}
/// The second (slow) release stage charges over this long, seconds.
const DUAL_CHARGE_S: f32 = 0.4;
/// …and lets go this many times slower than the release.
const DUAL_SLOWER: f32 = 5.0;
/// A linear release recovers this many dB per release time — the same point an
/// exponential release reaches (63 % of 10 dB), so the two curves cross where the release
/// time says.
const LINEAR_DB_PER_RELEASE: f32 = 6.32;

/// One style's detector, spelled out.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct StyleSpec {
    /// The RMS window, ms; at or under [`PEAK_RMS_MS`] the detector is a peak detector.
    pub rms_ms: f32,
    /// How much of the detector hears the gain-reduced key (feedback) rather than the key
    /// itself (feed-forward), 0…1.
    pub feedback: f32,
    /// How much the release slows with sustained reduction: 0 = not at all.
    pub program_release: f32,
    /// The share of a second, slower release stage that holds the reduction built up over
    /// time, 0…1 (the dual release of the classic mastering compressors).
    pub dual: f32,
    /// The release's shape: 0 = exponential (fast, then slower), 1 = linear in dB.
    pub curve: f32,
}

/// An RMS window at or under this is a peak detector.
pub const PEAK_RMS_MS: f32 = 0.1;

impl StyleSpec {
    pub fn is_peak(&self) -> bool {
        self.rms_ms <= PEAK_RMS_MS
    }

    /// The same spec with every number inside its range.
    pub fn clamped(self) -> StyleSpec {
        StyleSpec {
            rms_ms: self.rms_ms.clamp(0.0, 300.0),
            feedback: self.feedback.clamp(0.0, 1.0),
            program_release: self.program_release.clamp(0.0, 4.0),
            dual: self.dual.clamp(0.0, 1.0),
            curve: self.curve.clamp(0.0, 1.0),
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Style {
    Clean,
    Punch,
    Opto,
    Bus,
    Vocal,
    Mastering,
    Smooth,
    Pumping,
    Classic,
}

impl Style {
    /// In the order of the `style` parameter: a style's index never changes.
    pub const ALL: [Style; 9] = [
        Style::Clean,
        Style::Punch,
        Style::Opto,
        Style::Bus,
        Style::Vocal,
        Style::Mastering,
        Style::Smooth,
        Style::Pumping,
        Style::Classic,
    ];

    pub fn from_index(i: usize) -> Style {
        Self::ALL[i.min(Self::ALL.len() - 1)]
    }

    pub fn index(self) -> usize {
        Self::ALL.iter().position(|s| *s == self).unwrap_or(0)
    }

    pub fn spec(self) -> StyleSpec {
        let s = |rms_ms, feedback, program_release, dual, curve| StyleSpec {
            rms_ms,
            feedback,
            program_release,
            dual,
            curve,
        };
        match self {
            Style::Clean => s(0.0, 0.0, 0.0, 0.0, 0.0),
            Style::Punch => s(0.0, 0.4, 0.0, 0.0, 0.0),
            Style::Opto => s(30.0, 0.0, 3.0, 0.0, 0.0),
            Style::Bus => s(10.0, 0.2, 0.0, 0.0, 0.0),
            Style::Vocal => s(3.0, 0.0, 0.8, 0.0, 0.0),
            Style::Mastering => s(0.0, 0.0, 0.0, 0.7, 0.0),
            Style::Smooth => s(50.0, 0.0, 1.0, 0.5, 0.0),
            Style::Pumping => s(0.0, 0.0, 0.0, 0.0, 1.0),
            Style::Classic => s(5.0, 0.8, 1.5, 0.0, 0.3),
        }
    }
}

/// What to hear.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Listen {
    /// The compressor's output: wet plus dry, after the output gain.
    Normal,
    /// The detector's input: the key, after its level, high-pass and EQ.
    Key,
    /// What the compression takes away (or, above 100 % mix, adds): the input minus the
    /// gain-reduced signal, before make-up.
    Delta,
    /// The wet path alone, with its make-up, no dry signal.
    Wet,
    /// The input, bypassed, at the output's measured loudness: an honest A/B, where
    /// louder cannot pass for better.
    Compare,
}

impl Listen {
    pub const ALL: [Listen; 5] = [
        Listen::Normal,
        Listen::Key,
        Listen::Delta,
        Listen::Wet,
        Listen::Compare,
    ];

    pub fn from_index(i: usize) -> Listen {
        Self::ALL[i.min(Self::ALL.len() - 1)]
    }

    pub fn index(self) -> usize {
        Self::ALL.iter().position(|l| *l == self).unwrap_or(0)
    }
}

/// One band of the key's EQ — Aequalis's shapes and design, on the detector.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct KeyBand {
    pub active: bool,
    pub shape: EqShape,
    pub freq: f32,
    pub gain_db: f32,
    pub q: f32,
    /// dB/octave for the cuts: 6 … 96.
    pub slope: f32,
}

impl Default for KeyBand {
    fn default() -> Self {
        KeyBand {
            active: false,
            shape: EqShape::Bell,
            freq: 1_000.0,
            gain_db: 0.0,
            q: 1.0,
            slope: 12.0,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct CompSettings {
    pub threshold_db: f32,
    /// 1 … [`RATIO_INF`]; the top is ∞:1.
    pub ratio: f32,
    pub knee_db: f32,
    /// The most gain it will take away, dB.
    pub range_db: f32,
    pub attack_ms: f32,
    pub release_ms: f32,
    pub auto_release: bool,
    pub hold_ms: f32,
    /// How far ahead the detector hears, ms; the audio is delayed by as much, and that is
    /// the latency reported.
    pub lookahead_ms: f32,
    /// The wet path's gain (make-up), dB.
    pub makeup_db: f32,
    /// Make up the loudness the compression takes away, measured.
    pub auto_gain: bool,
    /// The dry path's gain, dB; at or under [`DRY_OFF_DB`] there is no dry path.
    pub dry_db: f32,
    /// How much of the gain change is applied, 0 … 2: 0 is a wire, 2 doubles it.
    pub mix: f32,
    pub input_db: f32,
    pub output_db: f32,
    pub style: Style,
    /// The detector in force when it is not the style's own: the engine drawer.
    pub engine: Option<StyleSpec>,
    /// 0 = each channel on its own, 1 = both follow the louder.
    pub link: f32,
    /// The key's one-pole high-pass, Hz; 20 is off.
    pub key_hpf_hz: f32,
    /// The key's level into the detector, dB.
    pub key_gain_db: f32,
    /// The key's EQ is in.
    pub key_eq: bool,
    pub key_bands: [KeyBand; KEY_BANDS],
    pub listen: Listen,
    /// The gain stage and the detector run at 1, 2 or 4 times the rate
    /// ([`oversample`]); anything above 1 adds its latency.
    pub oversample: usize,
}

impl Default for CompSettings {
    fn default() -> Self {
        CompSettings {
            threshold_db: -18.0,
            ratio: 4.0,
            knee_db: 6.0,
            range_db: 60.0,
            attack_ms: 10.0,
            release_ms: 120.0,
            auto_release: false,
            hold_ms: 0.0,
            lookahead_ms: 0.0,
            makeup_db: 0.0,
            auto_gain: false,
            dry_db: DRY_OFF_DB,
            mix: 1.0,
            input_db: 0.0,
            output_db: 0.0,
            style: Style::Clean,
            engine: None,
            link: 1.0,
            key_hpf_hz: 20.0,
            key_gain_db: 0.0,
            key_eq: true,
            key_bands: [KeyBand::default(); KEY_BANDS],
            listen: Listen::Normal,
            oversample: 1,
        }
    }
}

impl CompSettings {
    /// The detector in force: the engine drawer's, or the style's own.
    pub fn detector(&self) -> StyleSpec {
        self.engine.unwrap_or_else(|| self.style.spec()).clamped()
    }

    /// Lookahead in samples at a rate.
    pub fn lookahead_samples(&self, sample_rate: f32) -> usize {
        let max = (MAX_LOOKAHEAD_MS * 0.001 * sample_rate).ceil() as usize;
        ((self.lookahead_ms.clamp(0.0, MAX_LOOKAHEAD_MS) * 0.001 * sample_rate).round() as usize)
            .min(max)
    }
}

/// `1 − 1/ratio`: how much of the excess is taken away. Exactly 1 at ∞:1.
pub fn slope(ratio: f32) -> f32 {
    if ratio >= RATIO_INF {
        1.0
    } else {
        1.0 - 1.0 / ratio.max(1.0)
    }
}

/// The static curve: how many dB of reduction a detector level earns. Soft knee of width
/// `knee` centred on the threshold; limited to `range`.
pub fn gain_reduction_db(level_db: f32, s: &CompSettings) -> f32 {
    let over = level_db - s.threshold_db;
    let slope = slope(s.ratio);
    let k = s.knee_db.max(0.0);
    let gr = if k > 0.0 && over.abs() <= k * 0.5 {
        slope * math::powi(over + k * 0.5, 2) / (2.0 * k)
    } else if over > 0.0 {
        slope * over
    } else {
        0.0
    };
    gr.clamp(0.0, s.range_db.max(0.0))
}

/// The output level for an input level, on the static curve — what the knee graph draws.
pub fn transfer_db(level_db: f32, s: &CompSettings) -> f32 {
    level_db - gain_reduction_db(level_db, s)
}

/// The reduction a steady level really settles at once the detector's feedback is
/// counted: a feedback detector hears the reduced signal, so it earns less than the
/// static curve says — its ratio is softer than the one set. Equal to
/// [`gain_reduction_db`] for a feed-forward detector. Solved by bisection (the excess is
/// monotone in the reduction), to a thousandth of a dB.
pub fn effective_gain_reduction_db(level_db: f32, s: &CompSettings) -> f32 {
    let fb = s.detector().feedback;
    let full = gain_reduction_db(level_db, s);
    if fb <= 0.0 || full <= 0.0 {
        return full;
    }
    let (mut lo, mut hi) = (0.0f32, full);
    for _ in 0..40 {
        let mid = 0.5 * (lo + hi);
        let heard = level_db + 20.0 * math::log10((1.0 - fb) + fb * math::powf(10f32, -mid / 20.0));
        if gain_reduction_db(heard, s) > mid {
            lo = mid;
        } else {
            hi = mid;
        }
        if hi - lo < 1e-3 {
            break;
        }
    }
    0.5 * (lo + hi)
}

/// What the display reads after each block.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct CompMeter {
    /// Peak input and output in the block, linear.
    pub input: f32,
    pub output: f32,
    /// Largest gain reduction in the block, dB (positive).
    pub gr_db: f32,
}

/// The layout of [`Comprimo::telemetry`]: what the editor reads, one index per number.
pub mod tele {
    /// Gain reduction now, dB, the larger channel; then each channel.
    pub const GR: usize = 0;
    pub const GR_L: usize = 1;
    pub const GR_R: usize = 2;
    /// The auto gain in force, dB.
    pub const AUTO_GAIN: usize = 3;
    /// The detector's level now (after the key's filters and the link), dBFS.
    pub const KEY_LEVEL: usize = 4;
    /// The output's momentary and short-term loudness, LUFS.
    pub const OUT_M: usize = 5;
    pub const OUT_S: usize = 6;
    /// The input's momentary and short-term loudness, LUFS.
    pub const IN_M: usize = 7;
    pub const IN_S: usize = 8;
    /// Gain reduction over the last ten seconds: the mean and the peak, dB.
    pub const GR_AVG: usize = 9;
    pub const GR_PEAK: usize = 10;
    /// Crest factor (peak over RMS) of the input and the output over ten seconds, dB.
    pub const CREST_IN: usize = 11;
    pub const CREST_OUT: usize = 12;
    /// The 100 ms steps since the stream started (exact to 2^24 steps: nineteen days).
    pub const HOPS: usize = 13;
    /// The last [`super::BLOCKS_REPORTED`] momentary loudnesses of the output, newest
    /// first: the blocks the editor integrates.
    pub const BLOCKS: usize = 14;
    /// Latency, samples.
    pub const LATENCY: usize = 22;
    /// The gain the equal-loudness compare applies to the input, dB.
    pub const COMPARE: usize = 23;
    /// The sample rate.
    pub const SAMPLE_RATE: usize = 24;
    /// The detector in force: RMS window (ms, 0 = peak), feedback, program release,
    /// dual-release share, release curve.
    pub const DET: usize = 25;
    /// Peak input and output of the last block, linear.
    pub const IN_PEAK: usize = 30;
    pub const OUT_PEAK: usize = 31;
    /// How many of the 64 are used.
    pub const USED: usize = 32;
}

/// A one-pole high-pass for the detector.
#[derive(Copy, Clone, Debug, Default)]
struct Hpf {
    a: f32,
    x1: f32,
    y1: f32,
}

/// Whether the auto gain's (or the compare's) glide, `next = t + (v − t)·s` every
/// `MATCH_STEP` samples landing within 10⁻⁴, will never move again: landed, or stalled
/// where the step rounds away (ADR 0031 §5.1).
fn step_still(v: f32, t: f32, s: f32) -> bool {
    if v == t {
        return true;
    }
    let next = t + (v - t) * s;
    let next = if (next - t).abs() < 1e-4 { t } else { next };
    next.to_bits() == v.to_bits()
}

impl Hpf {
    fn is_clear(&self) -> bool {
        crate::rest::z(self.x1) && crate::rest::z(self.y1)
    }

    fn set(&mut self, hz: f32, sr: f32) {
        self.a = if hz <= 20.0 {
            1.0
        } else {
            math::exp(-2.0 * std::f32::consts::PI * hz / sr)
        };
    }

    #[inline]
    fn tick(&mut self, x: f32) -> f32 {
        if self.a >= 1.0 {
            return x;
        }
        let y = self.a * (self.y1 + x - self.x1);
        self.x1 = x;
        self.y1 = if y.abs() < 1e-20 { 0.0 } else { y };
        y
    }
}

/// A fixed-capacity stereo delay line; always written, so a lookahead switched on later
/// reads the signal and not whatever was there before. Read at any number of taps.
#[derive(Clone)]
struct Delay {
    buf: Vec<[f32; 2]>,
    pos: usize,
}

impl Delay {
    fn new(capacity: usize) -> Self {
        Delay {
            buf: vec![[0.0; 2]; capacity.max(1)],
            pos: 0,
        }
    }

    fn is_clear(&self) -> bool {
        self.buf.iter().all(|x| crate::rest::all_zero(x))
    }

    #[inline]
    fn push(&mut self, x: [f32; 2]) {
        self.buf[self.pos] = x;
        self.pos += 1;
        if self.pos == self.buf.len() {
            self.pos = 0;
        }
    }

    /// The sample pushed `delay` pushes ago (0: the last one).
    #[inline]
    fn tap(&self, delay: usize) -> [f32; 2] {
        let cap = self.buf.len();
        let back = delay.min(cap - 1) + 1;
        let i = if self.pos >= back {
            self.pos - back
        } else {
            self.pos + cap - back
        };
        self.buf[i]
    }

    fn clear(&mut self) {
        self.buf.iter_mut().for_each(|x| *x = [0.0; 2]);
        self.pos = 0;
    }
}

/// One step of the statistics grid.
#[derive(Copy, Clone, Debug, Default)]
struct HopStat {
    gr_sum: f64,
    gr_max: f32,
    in_peak: f32,
    in_sq: f64,
    out_peak: f32,
    out_sq: f64,
    samples: u32,
}

#[derive(Clone)]
pub struct Comprimo {
    sr: f32,
    s: CompSettings,
    spec: StyleSpec,
    // Derived from the settings in `set`.
    in_g: f32,
    out_g: f32,
    dry_g: f32,
    key_g: f32,
    look: usize,
    hold: u32,
    att: f32,
    rms: f32,
    slow_charge: f32,
    slow_release: f32,
    smooth: f32,
    key_n: [usize; KEY_BANDS],
    key_coefs: [[Coefs; MAX_SECTIONS]; KEY_BANDS],
    // State.
    hpf: [Hpf; 2],
    /// Per channel, band, section: the biquad's two state variables.
    key_z: [[[[f64; 2]; MAX_SECTIONS]; KEY_BANDS]; 2],
    ms: [f32; 2],
    fast: [f32; 2],
    slow: [f32; 2],
    /// Gain reduction in force per channel, dB.
    gr: [f32; 2],
    /// The gain that reduction is, linear, for the feedback detector.
    g_prev: [f32; 2],
    hold_left: [u32; 2],
    /// How long it has been compressing, for program-dependent release (seconds, leaky).
    work: f32,
    key_level_db: f32,
    audio: Delay,
    key: Delay,
    /// Oversampling of the gain stage (the audio) and of the detector (the key), and the
    /// latency it adds; the last gain applied, which the high-rate gain ramps from.
    os: Oversampler,
    os_key: Oversampler,
    los: usize,
    g_last: [f32; 2],
    auto_db: f32,
    auto_target: f32,
    g_static: f32,
    compare_db: f32,
    compare_target: f32,
    g_compare: f32,
    lm_in: loudness::Meter,
    lm_wet: loudness::Meter,
    lm_out: loudness::Meter,
    hop_len: usize,
    hop_pos: usize,
    hops: u64,
    cur: HopStat,
    stats: [HopStat; STAT_HOPS],
    stat_pos: usize,
    blocks: [f32; BLOCKS_REPORTED],
    meter: CompMeter,
}

impl std::fmt::Debug for Comprimo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Comprimo")
            .field("sr", &self.sr)
            .field("settings", &self.s)
            .field("gr", &self.gr)
            .finish_non_exhaustive()
    }
}

impl Comprimo {
    pub fn new(sample_rate: f32) -> Self {
        let sr = sample_rate.max(1.0);
        let cap = (MAX_LOOKAHEAD_MS * 0.001 * sr).ceil() as usize + oversample::latency(4) + 1;
        let mut c = Comprimo {
            sr,
            s: CompSettings::default(),
            spec: Style::Clean.spec(),
            in_g: 1.0,
            out_g: 1.0,
            dry_g: 0.0,
            key_g: 1.0,
            look: 0,
            hold: 0,
            att: 0.0,
            rms: 0.0,
            slow_charge: 0.0,
            slow_release: 0.0,
            smooth: 0.0,
            key_n: [0; KEY_BANDS],
            key_coefs: [[Coefs::IDENTITY; MAX_SECTIONS]; KEY_BANDS],
            hpf: [Hpf::default(); 2],
            key_z: [[[[0.0; 2]; MAX_SECTIONS]; KEY_BANDS]; 2],
            ms: [0.0; 2],
            fast: [0.0; 2],
            slow: [0.0; 2],
            gr: [0.0; 2],
            g_prev: [1.0; 2],
            hold_left: [0; 2],
            work: 0.0,
            key_level_db: loudness::SILENCE,
            audio: Delay::new(cap),
            key: Delay::new(cap),
            os: Oversampler::new(),
            os_key: Oversampler::new(),
            los: 0,
            g_last: [1.0; 2],
            auto_db: 0.0,
            auto_target: 0.0,
            g_static: 1.0,
            compare_db: 0.0,
            compare_target: 0.0,
            g_compare: 1.0,
            lm_in: loudness::Meter::new(sr),
            lm_wet: loudness::Meter::new(sr),
            lm_out: loudness::Meter::new(sr),
            hop_len: ((sr as f64 * loudness::HOP_S).round() as usize).max(1),
            hop_pos: 0,
            hops: 0,
            cur: HopStat::default(),
            stats: [HopStat::default(); STAT_HOPS],
            stat_pos: 0,
            blocks: [loudness::SILENCE; BLOCKS_REPORTED],
            meter: CompMeter::default(),
        };
        c.set(&CompSettings::default());
        c
    }

    pub fn sample_rate(&self) -> f32 {
        self.sr
    }

    /// The one-pole coefficient for a time constant of `ms`. The release recomputes it
    /// every sample (auto and program-dependent release move it), so for the small
    /// exponents every time constant over a millisecond gives, a four-term series stands
    /// in for `exp` — exact to a few parts in a billion there.
    #[inline]
    fn coef(&self, ms: f32) -> f32 {
        let x = 1.0 / (ms.max(0.001) * 0.001 * self.sr);
        if x < 0.02 {
            1.0 - x * (1.0 - x * (0.5 - x * (1.0 / 6.0)))
        } else {
            math::exp(-x)
        }
    }

    fn db_gain(db: f32) -> f32 {
        if db == 0.0 {
            1.0
        } else {
            gain_of(db)
        }
    }

    /// New settings. Never allocates, never clears state: called on the audio thread with
    /// every snapshot.
    pub fn set(&mut self, s: &CompSettings) {
        self.s = *s;
        self.spec = s.detector();
        let sr = self.sr;
        for h in &mut self.hpf {
            h.set(s.key_hpf_hz, sr);
        }
        self.in_g = Self::db_gain(s.input_db);
        self.out_g = Self::db_gain(s.output_db);
        self.key_g = Self::db_gain(s.key_gain_db);
        self.dry_g = if s.dry_db <= DRY_OFF_DB {
            0.0
        } else {
            gain_of(s.dry_db)
        };
        self.look = s.lookahead_samples(sr);
        self.os.set_factor(s.oversample);
        self.os_key.set_factor(s.oversample);
        self.los = oversample::latency(self.os.factor());
        self.hold = (s.hold_ms.max(0.0) * 0.001 * sr) as u32;
        self.att = self.coef(s.attack_ms);
        self.rms = if self.spec.is_peak() {
            0.0
        } else {
            self.coef(self.spec.rms_ms)
        };
        self.slow_charge = self.coef(DUAL_CHARGE_S * 1_000.0);
        self.slow_release = self.coef(s.release_ms * DUAL_SLOWER);
        self.smooth = math::powi(self.coef(MATCH_SMOOTH_S * 1_000.0), MATCH_STEP as i32);
        for (b, band) in s.key_bands.iter().enumerate() {
            self.key_n[b] = if band.active && s.key_eq {
                design(
                    band.shape,
                    sr,
                    band.freq,
                    band.gain_db,
                    band.q,
                    band.slope,
                    &mut self.key_coefs[b],
                )
            } else {
                0
            };
        }
        self.update_static_gain();
    }

    fn update_static_gain(&mut self) {
        self.g_static = Self::db_gain(self.s.mix * self.s.makeup_db + self.auto_db);
    }

    /// Samples of silence skipped while at rest (ADR 0031 §5.1): the 100 ms steps the
    /// loudness meters and the auto gain's glide run on go on, each one silent.
    pub fn skip(&mut self, frames: usize) {
        let total = self.hop_pos + frames;
        let hops = total / self.hop_len;
        self.hop_pos = total % self.hop_len;
        if hops > 0 {
            for m in [&mut self.lm_in, &mut self.lm_wet, &mut self.lm_out] {
                m.skip_hops(hops);
            }
            self.hops += hops as u64;
        }
    }

    /// At rest (ADR 0031 §5.1): no reduction anywhere (fast, slow, held, the work that
    /// slows the release), every filter, detector, line and oversampler empty, the auto
    /// gain and the compare landed — and where a new unit has them when they are heard —
    /// and the three loudness meters' windows all silence, so no step to come can move
    /// them. The statistics and block loudness are the editor's.
    pub fn at_rest(&self) -> bool {
        self.rest_blocker().is_none()
    }

    /// What keeps it from resting, if anything: the first condition of [`Self::at_rest`]
    /// that fails, by name (for the tests and a profiler that asks why).
    pub fn rest_blocker(&self) -> Option<&'static str> {
        use crate::rest::{all_zero, z64};
        let s = &self.s;
        let goal = if s.auto_gain { self.auto_target } else { 0.0 };
        let checks: [(&'static str, &dyn Fn() -> bool); 14] = [
            ("auto gain glide", &|| {
                step_still(self.auto_db, goal, self.smooth)
            }),
            ("auto gain", &|| !s.auto_gain || self.auto_db == 0.0),
            ("compare glide", &|| {
                step_still(self.compare_db, self.compare_target, self.smooth)
            }),
            ("compare", &|| {
                s.listen != Listen::Compare || self.compare_db == 0.0
            }),
            ("reduction", &|| {
                all_zero(&self.gr) && all_zero(&self.fast) && all_zero(&self.slow)
            }),
            ("detector", &|| {
                all_zero(&self.ms) && self.hold_left == [0; 2]
            }),
            ("programme memory", &|| self.work.to_bits() == 0),
            ("gain stage", &|| {
                self.g_last == [1.0; 2] && self.g_prev == [1.0; 2]
            }),
            ("key filters", &|| {
                self.hpf.iter().all(Hpf::is_clear)
                    && self
                        .key_z
                        .iter()
                        .flatten()
                        .flatten()
                        .flatten()
                        .all(|v| z64(*v))
            }),
            ("loudness meters", &|| {
                [&self.lm_in, &self.lm_wet, &self.lm_out]
                    .iter()
                    .all(|m| m.is_clear())
            }),
            ("oversampler", &|| self.os.is_clear()),
            ("key oversampler", &|| self.os_key.is_clear()),
            ("audio line", &|| self.audio.is_clear()),
            ("key line", &|| self.key.is_clear()),
        ];
        checks.iter().find(|(_, ok)| !ok()).map(|(name, _)| *name)
    }

    pub fn reset(&mut self) {
        self.hpf = [Hpf::default(); 2];
        for h in &mut self.hpf {
            h.set(self.s.key_hpf_hz, self.sr);
        }
        self.key_z = [[[[0.0; 2]; MAX_SECTIONS]; KEY_BANDS]; 2];
        self.ms = [0.0; 2];
        self.fast = [0.0; 2];
        self.slow = [0.0; 2];
        self.gr = [0.0; 2];
        self.g_prev = [1.0; 2];
        self.hold_left = [0; 2];
        self.work = 0.0;
        self.key_level_db = loudness::SILENCE;
        self.audio.clear();
        self.key.clear();
        self.os.reset();
        self.os_key.reset();
        self.g_last = [1.0; 2];
        self.auto_db = 0.0;
        self.auto_target = 0.0;
        self.compare_db = 0.0;
        self.compare_target = 0.0;
        self.g_compare = 1.0;
        self.lm_in.reset();
        self.lm_wet.reset();
        self.lm_out.reset();
        self.hop_pos = 0;
        self.hops = 0;
        self.cur = HopStat::default();
        self.stats = [HopStat::default(); STAT_HOPS];
        self.stat_pos = 0;
        self.blocks = [loudness::SILENCE; BLOCKS_REPORTED];
        self.update_static_gain();
    }

    /// Samples of delay this adds: the lookahead, and the oversampling filters'.
    pub fn latency(&self) -> usize {
        self.look + self.los
    }

    /// What the last block did, for the display.
    pub fn meter(&self) -> CompMeter {
        self.meter
    }

    /// The auto gain in force now, dB.
    pub fn auto_gain_db(&self) -> f32 {
        self.auto_db
    }

    /// The key through its level, high-pass and EQ: what the detector hears.
    #[inline]
    fn filter_key(&mut self, kl: f32, kr: f32) -> [f32; 2] {
        let mut k = [kl, kr];
        if self.key_g != 1.0 {
            k[0] *= self.key_g;
            k[1] *= self.key_g;
        }
        let any_band = self.key_n.iter().any(|n| *n > 0);
        for (ch, x) in k.iter_mut().enumerate() {
            *x = self.hpf[ch].tick(*x);
            if !any_band {
                continue;
            }
            let mut y = *x as f64;
            for b in 0..KEY_BANDS {
                let n = self.key_n[b];
                if n == 0 {
                    continue;
                }
                let silent_in = y == 0.0;
                let mut tiny = silent_in;
                for sec in 0..n {
                    let c = &self.key_coefs[b][sec];
                    let z = &mut self.key_z[ch][b][sec];
                    let out = c.b0 as f64 * y + z[0];
                    z[0] = c.b1 as f64 * y - c.a1 as f64 * out + z[1];
                    z[1] = c.b2 as f64 * y - c.a2 as f64 * out;
                    // Far under the denormals' reach: nothing can linger there.
                    for v in z.iter_mut() {
                        if v.abs() < 1e-200 {
                            *v = 0.0;
                        }
                    }
                    tiny &= z[0].abs() < 1e-25 && z[1].abs() < 1e-25;
                    y = out;
                }
                // A band that hears silence and has all but died away is stopped whole:
                // flushing its sections one at a time would kick the ones downstream
                // (a steep cut's sections amplify each other's state) and keep it ringing
                // at the flush level for seconds.
                if tiny {
                    self.key_z[ch][b] = [[0.0; 2]; MAX_SECTIONS];
                    y = 0.0;
                }
            }
            *x = y as f32;
        }
        k
    }

    /// The gain computer and its ballistics for one sample of detector input.
    #[inline]
    fn detect(&mut self, keyed: [f32; 2]) {
        let s = &self.s;
        let spec = self.spec;
        let mut level = [0f32; 2];
        for ch in 0..2 {
            // Feedback: the detector hears the key as reduced by the gain in force.
            let d = if spec.feedback > 0.0 && self.g_prev[ch] != 1.0 {
                keyed[ch] * ((1.0 - spec.feedback) + spec.feedback * self.g_prev[ch])
            } else {
                keyed[ch]
            };
            level[ch] = if self.rms > 0.0 {
                self.ms[ch] = d * d + (self.ms[ch] - d * d) * self.rms;
                if self.ms[ch] < 1e-30 {
                    self.ms[ch] = 0.0;
                }
                self.ms[ch].sqrt()
            } else {
                d.abs()
            };
        }
        // Stereo link: each side moves toward the louder by `link`.
        let loud = level[0].max(level[1]);
        for l in &mut level {
            *l += (loud - *l) * s.link.clamp(0.0, 1.0);
        }
        let mut key_db = loudness::SILENCE;
        for (ch, l) in level.iter().enumerate() {
            let db = if *l > 1e-9 {
                20.0 * math::log10(*l)
            } else {
                -200.0
            };
            key_db = key_db.max(db);
            let target = gain_reduction_db(db, s);
            let cur = self.fast[ch];
            let next = if target > cur {
                self.hold_left[ch] = self.hold;
                target + (cur - target) * self.att
            } else if self.hold_left[ch] > 0 {
                self.hold_left[ch] -= 1;
                cur
            } else if cur > target {
                // Release: auto release follows how hard it is working; the
                // program-dependent styles slow down with how long it has been working.
                let mut ms = s.release_ms;
                if s.auto_release {
                    ms = (ms * (0.3 + cur / 6.0)).clamp(ms * 0.3, ms * 3.0);
                }
                ms *= 1.0 + spec.program_release * self.work.min(2.0);
                let exp = target + (cur - target) * self.coef(ms);
                if spec.curve > 0.0 {
                    let step = LINEAR_DB_PER_RELEASE / (ms.max(0.001) * 0.001 * self.sr);
                    let lin = (cur - step).max(target);
                    exp + (lin - exp) * spec.curve
                } else {
                    exp
                }
            } else {
                cur
            };
            self.fast[ch] = if next < 1e-6 { 0.0 } else { next };
            let mut gr = self.fast[ch];
            if spec.dual > 0.0 {
                // The second stage: charges slowly toward the reduction and lets go
                // slower still, so what has built up over time is held while a lone
                // transient comes back fast.
                let (sl, fast) = (self.slow[ch], self.fast[ch]);
                let next = if fast > sl {
                    fast + (sl - fast) * self.slow_charge
                } else {
                    fast + (sl - fast) * self.slow_release
                };
                self.slow[ch] = if next < 1e-6 { 0.0 } else { next };
                gr = gr.max(self.slow[ch] * spec.dual);
            }
            self.gr[ch] = gr;
        }
        self.key_level_db = key_db;
        // How long it has been working: rises while compressing more than a dB, leaks
        // away otherwise, over about a second.
        let working = self.gr[0].max(self.gr[1]) > 1.0;
        self.work += if working {
            1.0 / self.sr
        } else {
            -self.work / self.sr
        };
        if self.work < 1e-9 {
            self.work = 0.0;
        }
    }

    /// Process in place. `key` is the external sidechain, if one is routed in; otherwise
    /// the compressor listens to its own input.
    pub fn process(&mut self, left: &mut [f32], right: &mut [f32], key: Option<(&[f32], &[f32])>) {
        let n = left.len().min(right.len());
        let mut meter = CompMeter::default();
        for i in 0..n {
            let (mut xl, mut xr) = (left[i], right[i]);
            if self.in_g != 1.0 {
                xl *= self.in_g;
                xr *= self.in_g;
            }
            let (kl, kr) = match key {
                Some((a, b)) => (
                    a.get(i).copied().unwrap_or(0.0),
                    b.get(i).copied().unwrap_or(0.0),
                ),
                None => (xl, xr),
            };
            let keyed = self.filter_key(kl, kr);
            let factor = self.os.factor();
            if factor > 1 {
                // Oversampled, the detector hears the key's largest phase: the peaks
                // between the samples.
                let mut heard = [0f32; 2];
                for (ch, h) in heard.iter_mut().enumerate() {
                    let mut ph = [0f32; 4];
                    self.os_key.up(ch, keyed[ch], &mut ph);
                    *h = ph[..factor]
                        .iter()
                        .fold(0f32, |m, v| if v.abs() > m.abs() { *v } else { m });
                }
                self.detect(heard);
            } else {
                self.detect(keyed);
            }
            self.audio.push([xl, xr]);
            self.key.push(keyed);
            // Everything beside the gain stage waits for it: the lookahead, and the
            // oversampling filters' latency when they are in.
            let side = self.look + self.los;
            let x = self.audio.tap(side);
            let kd = self.key.tap(side);
            let into_os = self.audio.tap(self.look);

            // Auto gain and the compare gain glide toward their measurements, a step every
            // `MATCH_STEP` samples of the stream.
            if self.hop_pos.is_multiple_of(MATCH_STEP) {
                let goal = if self.s.auto_gain {
                    self.auto_target
                } else {
                    0.0
                };
                if self.auto_db != goal {
                    let next = goal + (self.auto_db - goal) * self.smooth;
                    self.auto_db = if (next - goal).abs() < 1e-4 {
                        goal
                    } else {
                        next
                    };
                    self.update_static_gain();
                }
                if self.compare_db != self.compare_target {
                    let next =
                        self.compare_target + (self.compare_db - self.compare_target) * self.smooth;
                    self.compare_db = if (next - self.compare_target).abs() < 1e-4 {
                        self.compare_target
                    } else {
                        next
                    };
                    self.g_compare = Self::db_gain(self.compare_db);
                }
            }

            let mix = self.s.mix;
            let mut red = x;
            let mut wet = x;
            let mut out = x;
            for ch in 0..2 {
                let gr = self.gr[ch];
                let g = if gr == 0.0 { 1.0 } else { gain_of(-gr) };
                self.g_prev[ch] = g;
                let g_red = if gr == 0.0 || mix == 0.0 {
                    1.0
                } else if mix == 1.0 {
                    g
                } else {
                    gain_of(-mix * gr)
                };
                if factor > 1 {
                    // The gain at the high rate, ramped from the last one across the
                    // phases, then back down: its sidebands above the band are filtered
                    // out instead of folding back.
                    let mut ph = [0f32; 4];
                    self.os.up(ch, into_os[ch], &mut ph);
                    let g0 = self.g_last[ch];
                    if g0 != 1.0 || g_red != 1.0 {
                        for (j, v) in ph[..factor].iter_mut().enumerate() {
                            let t = (j + 1) as f32 / factor as f32;
                            *v *= g0 + (g_red - g0) * t;
                        }
                    }
                    red[ch] = self.os.down(ch, &ph);
                    self.g_last[ch] = g_red;
                } else if g_red != 1.0 {
                    red[ch] = x[ch] * g_red;
                }
                wet[ch] = if self.g_static != 1.0 {
                    red[ch] * self.g_static
                } else {
                    red[ch]
                };
                out[ch] = if self.dry_g > 0.0 {
                    wet[ch] + x[ch] * self.dry_g
                } else {
                    wet[ch]
                };
                if self.out_g != 1.0 {
                    out[ch] *= self.out_g;
                }
            }

            self.lm_in.add(x[0], x[1]);
            self.lm_wet.add(red[0], red[1]);
            self.lm_out.add(out[0], out[1]);
            let gr_max = self.gr[0].max(self.gr[1]);
            let in_peak = x[0].abs().max(x[1].abs());
            let out_peak = out[0].abs().max(out[1].abs());
            let c = &mut self.cur;
            c.gr_sum += gr_max as f64;
            c.gr_max = c.gr_max.max(gr_max);
            c.in_peak = c.in_peak.max(in_peak);
            c.in_sq += math::powi(x[0] as f64, 2) + math::powi(x[1] as f64, 2);
            c.out_peak = c.out_peak.max(out_peak);
            c.out_sq += math::powi(out[0] as f64, 2) + math::powi(out[1] as f64, 2);
            c.samples += 1;
            self.hop_pos += 1;
            if self.hop_pos >= self.hop_len {
                self.end_hop();
            }

            meter.input = meter.input.max(in_peak);
            meter.output = meter.output.max(out_peak);
            meter.gr_db = meter.gr_db.max(gr_max);
            let heard = match self.s.listen {
                Listen::Normal => out,
                Listen::Wet => {
                    if self.out_g != 1.0 {
                        [wet[0] * self.out_g, wet[1] * self.out_g]
                    } else {
                        wet
                    }
                }
                Listen::Key => kd,
                Listen::Delta => [x[0] - red[0], x[1] - red[1]],
                Listen::Compare => {
                    if self.g_compare != 1.0 {
                        [x[0] * self.g_compare, x[1] * self.g_compare]
                    } else {
                        x
                    }
                }
            };
            left[i] = heard[0];
            right[i] = heard[1];
        }
        self.meter = meter;
    }

    /// Close a 100 ms step: loudness, what auto gain and the compare should be, the
    /// statistics, the output's block loudness.
    fn end_hop(&mut self) {
        let len = self.hop_pos;
        self.hop_pos = 0;
        self.lm_in.end_hop(len);
        self.lm_wet.end_hop(len);
        self.lm_out.end_hop(len);
        let lin = self.lm_in.short_term();
        if lin > MATCH_GATE_LUFS {
            // Both meters warm up the same way, so their difference is right from the
            // first step, before the three-second window has filled.
            self.auto_target = (lin - self.lm_wet.short_term()).clamp(-36.0, 36.0);
            self.compare_target = (self.lm_out.short_term() - lin).clamp(-36.0, 36.0);
        }
        self.stats[self.stat_pos] = std::mem::take(&mut self.cur);
        self.stat_pos = (self.stat_pos + 1) % STAT_HOPS;
        self.blocks.copy_within(0..BLOCKS_REPORTED - 1, 1);
        self.blocks[0] = self.lm_out.momentary();
        self.hops += 1;
    }

    /// What the editor draws, laid out as in [`tele`].
    pub fn telemetry(&self) -> [f32; 64] {
        let mut t = [0f32; 64];
        t[tele::GR] = self.gr[0].max(self.gr[1]);
        t[tele::GR_L] = self.gr[0];
        t[tele::GR_R] = self.gr[1];
        t[tele::AUTO_GAIN] = self.auto_db;
        t[tele::KEY_LEVEL] = self.key_level_db;
        t[tele::OUT_M] = self.lm_out.momentary();
        t[tele::OUT_S] = self.lm_out.short_term();
        t[tele::IN_M] = self.lm_in.momentary();
        t[tele::IN_S] = self.lm_in.short_term();
        let mut agg = HopStat::default();
        for h in &self.stats {
            agg.gr_sum += h.gr_sum;
            agg.gr_max = agg.gr_max.max(h.gr_max);
            agg.in_peak = agg.in_peak.max(h.in_peak);
            agg.in_sq += h.in_sq;
            agg.out_peak = agg.out_peak.max(h.out_peak);
            agg.out_sq += h.out_sq;
            agg.samples += h.samples;
        }
        let n = agg.samples.max(1) as f64;
        t[tele::GR_AVG] = (agg.gr_sum / n) as f32;
        t[tele::GR_PEAK] = agg.gr_max;
        let crest = |peak: f32, sq: f64| {
            let rms = (sq / (2.0 * n)).sqrt() as f32;
            if rms > 1e-9 && peak > 0.0 {
                20.0 * math::log10(peak / rms)
            } else {
                0.0
            }
        };
        t[tele::CREST_IN] = crest(agg.in_peak, agg.in_sq);
        t[tele::CREST_OUT] = crest(agg.out_peak, agg.out_sq);
        t[tele::HOPS] = self.hops as f32;
        t[tele::BLOCKS..tele::BLOCKS + BLOCKS_REPORTED].copy_from_slice(&self.blocks);
        t[tele::LATENCY] = (self.look + self.los) as f32;
        t[tele::COMPARE] = self.compare_db;
        t[tele::SAMPLE_RATE] = self.sr;
        let d = self.spec;
        t[tele::DET] = if d.is_peak() { 0.0 } else { d.rms_ms };
        t[tele::DET + 1] = d.feedback;
        t[tele::DET + 2] = d.program_release;
        t[tele::DET + 3] = d.dual;
        t[tele::DET + 4] = d.curve;
        t[tele::IN_PEAK] = self.meter.input;
        t[tele::OUT_PEAK] = self.meter.output;
        t
    }
}

/// How a compressor answers a burst: the gain reduction over time for a step up to 12 dB
/// over the threshold, held for 400 ms, then a step down 20 dB under it — the response
/// graph the editor draws beside the knobs, so "attack" means the same thing in every
/// style because you can see what it does.
#[derive(Clone, Debug, PartialEq)]
pub struct StepResponse {
    /// Gain reduction, dB, one value per point (the largest in its stretch).
    pub gr_db: Vec<f32>,
    /// The level reaching the output, dB over the threshold, per point.
    pub level_db: Vec<f32>,
    /// The whole burst's length, ms.
    pub length_ms: f32,
    /// When the detector hears the step up and the step down, ms — the clock the
    /// reduction is plotted on.
    pub step_ms: f32,
    pub drop_ms: f32,
    /// When they reach the output, ms: later by the lookahead.
    pub rise_ms: f32,
    pub fall_ms: f32,
    /// Measured: how long the reduction takes to reach 63 % of where it settles, and
    /// to let go of 63 % of it after the fall, ms. `None` when it never compresses.
    pub attack_ms: Option<f32>,
    pub release_ms: Option<f32>,
    /// Where it settles during the burst, dB.
    pub settled_db: f32,
}

/// Run the compressor as set over the test burst at `sample_rate`, with the key's filters
/// and the auto features out of the way (a picture of time, not of tone), and fold it into
/// `points` points. Allocates: for the editor, never the audio thread.
pub fn step_response(s: &CompSettings, sample_rate: f32, points: usize) -> StepResponse {
    let points = points.max(2);
    let sr = sample_rate.max(1_000.0);
    let lead_ms = 50.0;
    let hold_ms = 400.0;
    let tail_ms = 650.0;
    let length_ms = lead_ms + hold_ms + tail_ms;
    let n = (length_ms * 0.001 * sr) as usize;
    let rise = (lead_ms * 0.001 * sr) as usize;
    let fall = ((lead_ms + hold_ms) * 0.001 * sr) as usize;
    let settings = CompSettings {
        key_hpf_hz: 20.0,
        key_gain_db: 0.0,
        key_eq: false,
        auto_gain: false,
        input_db: 0.0,
        output_db: 0.0,
        makeup_db: 0.0,
        dry_db: DRY_OFF_DB,
        mix: 1.0,
        listen: Listen::Normal,
        ..*s
    };
    let mut c = Comprimo::new(sr);
    c.set(&settings);
    let hi = math::powf(10f32, (s.threshold_db + 12.0) / 20.0);
    let lo = math::powf(10f32, (s.threshold_db - 20.0) / 20.0);
    let look = c.latency();
    let mut gr = vec![0f32; n];
    let mut l = [0f32; 1];
    let mut r = [0f32; 1];
    for (i, g) in gr.iter_mut().enumerate() {
        let x = if (rise..fall).contains(&i) { hi } else { lo };
        l[0] = x;
        r[0] = x;
        c.process(&mut l, &mut r, None);
        *g = c.gr[0].max(c.gr[1]);
    }
    let settled = gr[fall.saturating_sub(1)];
    let ms_of = |i: usize| i as f32 / sr * 1_000.0;
    // The step reaches the output `look` samples after the detector heard it.
    let rise_out = rise + look;
    let fall_out = fall + look;
    let attack = (settled > 0.05)
        .then(|| {
            gr[rise..fall]
                .iter()
                .position(|g| *g >= settled * 0.632)
                .map(ms_of)
        })
        .flatten();
    let release = (settled > 0.05)
        .then(|| {
            gr[fall..]
                .iter()
                .position(|g| *g <= settled * 0.368)
                .map(ms_of)
        })
        .flatten();
    let per = n as f32 / points as f32;
    let mut gr_db = Vec::with_capacity(points);
    let mut level_db = Vec::with_capacity(points);
    for p in 0..points {
        let a = (p as f32 * per) as usize;
        let b = (((p + 1) as f32 * per) as usize).clamp(a + 1, n);
        gr_db.push(gr[a..b].iter().fold(0f32, |m, g| m.max(*g)));
        let mid = (a + b) / 2;
        level_db.push(if (rise_out..fall_out).contains(&mid) {
            12.0
        } else {
            -20.0
        });
    }
    StepResponse {
        gr_db,
        level_db,
        length_ms,
        step_ms: ms_of(rise),
        drop_ms: ms_of(fall),
        rise_ms: ms_of(rise_out),
        fall_ms: ms_of(fall_out),
        attack_ms: attack,
        release_ms: release,
        settled_db: settled,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(amp: f32, n: usize) -> Vec<f32> {
        (0..n)
            .map(|i| amp * math::sin(std::f32::consts::TAU * 440.0 * i as f32 / 48_000.0))
            .collect()
    }

    fn tone_at(hz: f32, amp: f32, n: usize) -> Vec<f32> {
        (0..n)
            .map(|i| amp * math::sin(std::f32::consts::TAU * hz * i as f32 / 48_000.0))
            .collect()
    }

    fn noise(seed: &mut u32) -> f32 {
        *seed ^= *seed << 13;
        *seed ^= *seed >> 17;
        *seed ^= *seed << 5;
        (*seed as f32 / u32::MAX as f32) * 2.0 - 1.0
    }

    /// Drum-like material: a noisy hit every 250 ms over a quiet bed.
    fn drums(n: usize) -> Vec<f32> {
        let mut seed = 11u32;
        (0..n)
            .map(|i| {
                let t = (i % 12_000) as f32 / 48_000.0;
                let env = math::exp(-t * 30.0);
                0.9 * env * noise(&mut seed) + 0.05 * math::sin(i as f32 * 0.037)
            })
            .collect()
    }

    fn settle(c: &mut Comprimo, amp: f32) -> Vec<f32> {
        let (mut l, mut r) = (tone(amp, 48_000), tone(amp, 48_000));
        c.process(&mut l, &mut r, None);
        l
    }

    fn peak(x: &[f32]) -> f32 {
        x.iter().fold(0.0, |m, v| m.max(v.abs()))
    }

    fn rms_db(x: &[f32]) -> f32 {
        let ms = x.iter().map(|v| math::powi(*v as f64, 2)).sum::<f64>() / x.len() as f64;
        (10.0 * math::log10(ms)) as f32
    }

    fn run(s: &CompSettings, l: &[f32], r: &[f32], block: usize) -> (Vec<f32>, Vec<f32>) {
        let mut c = Comprimo::new(48_000.0);
        c.set(s);
        let (mut l, mut r) = (l.to_vec(), r.to_vec());
        for (a, b) in l.chunks_mut(block).zip(r.chunks_mut(block)) {
            c.process(a, b, None);
        }
        (l, r)
    }

    #[test]
    fn the_static_curve_is_the_one_drawn() {
        let s = CompSettings {
            threshold_db: -20.0,
            ratio: 4.0,
            knee_db: 0.0,
            ..CompSettings::default()
        };
        assert_eq!(gain_reduction_db(-30.0, &s), 0.0);
        assert!(
            (transfer_db(-10.0, &s) - -17.5).abs() < 1e-4,
            "10 dB over at 4:1 is 2.5 over"
        );
        let soft = CompSettings { knee_db: 10.0, ..s };
        assert!(
            gain_reduction_db(-20.0, &soft) > 0.0,
            "the knee starts below the threshold"
        );
        assert!(
            (gain_reduction_db(-10.0, &soft) - 7.5).abs() < 1e-4,
            "above the knee, as hard"
        );
        let limited = CompSettings { range_db: 3.0, ..s };
        assert_eq!(
            gain_reduction_db(0.0, &limited),
            3.0,
            "range caps the reduction"
        );
        // The top of the ratio is ∞:1 exactly: everything over the threshold is taken.
        let inf = CompSettings {
            ratio: RATIO_INF,
            ..s
        };
        assert_eq!(transfer_db(-5.0, &inf), -20.0);
        // The widest knee, 72 dB, is still continuous and meets the hard curve at its
        // edges.
        let wide = CompSettings { knee_db: 72.0, ..s };
        assert_eq!(gain_reduction_db(-56.0, &wide), 0.0);
        assert!((gain_reduction_db(16.0, &wide) - 27.0).abs() < 1e-3);
        let mut last = 0.0;
        for i in 0..=1000 {
            let d = -80.0 + i as f32 * 0.1;
            let g = gain_reduction_db(d, &wide);
            // At most the slope's worth per 0.1 dB: no jump anywhere in the knee.
            assert!(g >= last && g - last <= 0.0751, "a step in the knee at {d}");
            last = g;
        }
    }

    /// Below the threshold, with no make-up, the output is the input to the bit — in
    /// every style, with auto gain on, at any mix, and with a lookahead (then delayed by
    /// exactly the latency it reports).
    #[test]
    fn a_compressor_that_is_not_compressing_touches_nothing() {
        let src = tone(0.1, 30_000); // −20 dBFS peak, −23 LUFS: auto gain measures it
        for style in Style::ALL {
            for (auto_gain, mix, look) in [(false, 1.0, 0.0), (true, 2.0, 0.0), (true, 0.5, 5.0)] {
                let s = CompSettings {
                    threshold_db: -6.0,
                    knee_db: 0.0,
                    style,
                    auto_gain,
                    mix,
                    lookahead_ms: look,
                    ..CompSettings::default()
                };
                let mut c = Comprimo::new(48_000.0);
                c.set(&s);
                let d = c.latency();
                assert_eq!(d, (look * 48.0) as usize);
                let (mut l, mut r) = (src.clone(), src.clone());
                c.process(&mut l, &mut r, None);
                assert!(l[..d].iter().all(|x| *x == 0.0));
                assert_eq!(
                    &l[d..],
                    &src[..src.len() - d],
                    "{style:?} auto {auto_gain} mix {mix} look {look} changed a quiet signal"
                );
                assert_eq!(c.auto_gain_db(), 0.0);
            }
        }
    }

    /// Mix at 0 % is a wire even while it is compressing hard.
    #[test]
    fn no_mix_is_a_wire() {
        let src = tone(1.0, 20_000);
        let s = CompSettings {
            threshold_db: -40.0,
            mix: 0.0,
            ..CompSettings::default()
        };
        let (l, _) = run(&s, &src, &src, 512);
        assert_eq!(l, src);
    }

    #[test]
    fn it_compresses_by_the_ratio_once_settled() {
        let mut c = Comprimo::new(48_000.0);
        c.set(&CompSettings {
            threshold_db: -20.0,
            ratio: 4.0,
            knee_db: 0.0,
            attack_ms: 1.0,
            release_ms: 50.0,
            ..CompSettings::default()
        });
        // A 0 dBFS peak tone, 20 dB over: 15 dB of reduction on the peak detector.
        let out = settle(&mut c, 1.0);
        let tail = peak(&out[40_000..]);
        let db = 20.0 * math::log10(tail);
        assert!((db - -15.0).abs() < 1.0, "settled at {db} dB");
        assert!((c.meter().gr_db - 15.0).abs() < 1.0);
    }

    /// At ∞:1 with a fast attack a steady tone comes out at the threshold.
    #[test]
    fn infinity_holds_the_threshold() {
        let mut c = Comprimo::new(48_000.0);
        c.set(&CompSettings {
            threshold_db: -12.0,
            ratio: RATIO_INF,
            knee_db: 0.0,
            attack_ms: 0.005,
            release_ms: 500.0,
            ..CompSettings::default()
        });
        let out = settle(&mut c, 1.0);
        let db = 20.0 * math::log10(peak(&out[40_000..]));
        assert!((db - -12.0).abs() < 0.1, "∞:1 settled at {db} dB");
    }

    /// Mix scales the gain change in dB: 200 % takes twice the reduction.
    #[test]
    fn mix_scales_the_reduction_in_db() {
        let src = tone(1.0, 48_000);
        let base = CompSettings {
            threshold_db: -20.0,
            ratio: 2.0,
            knee_db: 0.0,
            attack_ms: 1.0,
            ..CompSettings::default()
        };
        let at = |mix: f32| {
            let (l, _) = run(&CompSettings { mix, ..base }, &src, &src, 256);
            20.0 * math::log10(peak(&l[40_000..]))
        };
        let (full, half, double) = (at(1.0), at(0.5), at(2.0));
        assert!((full - -10.0).abs() < 0.3, "100 %: {full}");
        assert!((half - -5.0).abs() < 0.3, "50 %: {half}");
        assert!((double - -20.0).abs() < 0.5, "200 %: {double}");
    }

    /// The dry path is the input at its own gain, added to the wet: parallel compression.
    #[test]
    fn dry_gain_is_parallel_compression() {
        let src = tone(1.0, 24_000);
        let wet_only = CompSettings {
            threshold_db: -30.0,
            attack_ms: 1.0,
            ..CompSettings::default()
        };
        let parallel = CompSettings {
            dry_db: -6.0,
            ..wet_only
        };
        let (w, _) = run(&wet_only, &src, &src, 333);
        let (p, _) = run(&parallel, &src, &src, 333);
        let g = math::powf(10f32, -6.0 / 20.0);
        for i in 0..src.len() {
            assert!((p[i] - (w[i] + src[i] * g)).abs() < 1e-6);
        }
    }

    #[test]
    fn input_and_output_gain_are_where_they_say() {
        let src = tone(0.1, 24_000);
        // Input gain pushes a quiet tone over the threshold; output gain trims after.
        let s = CompSettings {
            threshold_db: -20.0,
            input_db: 12.0,
            output_db: -6.0,
            knee_db: 0.0,
            ratio: RATIO_INF,
            attack_ms: 0.5,
            ..CompSettings::default()
        };
        let (l, _) = run(&s, &src, &src, 64);
        let db = 20.0 * math::log10(peak(&l[20_000..]));
        assert!((db - -26.0).abs() < 0.2, "{db}");
    }

    #[test]
    fn a_key_from_elsewhere_ducks_the_signal_and_can_be_heard() {
        let mut c = Comprimo::new(48_000.0);
        c.set(&CompSettings {
            threshold_db: -30.0,
            ratio: 10.0,
            attack_ms: 1.0,
            ..CompSettings::default()
        });
        let quiet = tone(0.05, 24_000);
        let key = tone(1.0, 24_000);
        let (mut l, mut r) = (quiet.clone(), quiet.clone());
        c.process(&mut l, &mut r, Some((&key, &key)));
        assert!(
            peak(&l[20_000..]) < peak(&quiet) * 0.2,
            "a loud key ducks a quiet signal"
        );
        // Listening to the key gives back the key (no high-pass at 20 Hz, no EQ band).
        let mut k = Comprimo::new(48_000.0);
        k.set(&CompSettings {
            listen: Listen::Key,
            ..CompSettings::default()
        });
        let (mut l, mut r) = (quiet.clone(), quiet.clone());
        k.process(&mut l, &mut r, Some((&key, &key)));
        assert_eq!(l, key);
    }

    /// The key's EQ decides what the detector hears: a low cut stops a bass note from
    /// pumping the mix, a bell makes a narrow band trigger it, and the EQ's bypass puts it
    /// back as it was.
    #[test]
    fn the_key_eq_shapes_what_triggers_it() {
        let bass = tone_at(60.0, 0.8, 48_000);
        let base = CompSettings {
            threshold_db: -20.0,
            ratio: 8.0,
            attack_ms: 1.0,
            ..CompSettings::default()
        };
        let mut cut = base;
        cut.key_bands[0] = KeyBand {
            active: true,
            shape: EqShape::LowCut,
            freq: 300.0,
            slope: 48.0,
            ..KeyBand::default()
        };
        let gr = |s: &CompSettings| {
            let mut c = Comprimo::new(48_000.0);
            c.set(s);
            let (mut l, mut r) = (bass.clone(), bass.clone());
            c.process(&mut l, &mut r, None);
            c.telemetry()[tele::GR]
        };
        assert!(gr(&base) > 10.0, "the bass pumps it with no EQ");
        assert!(gr(&cut) < 0.5, "a 300 Hz low cut on the key: {}", gr(&cut));
        let bypassed = CompSettings {
            key_eq: false,
            ..cut
        };
        assert_eq!(gr(&bypassed), gr(&base), "the EQ's bypass is exact");
        // A bell boost: a tone that did not reach the threshold now does.
        let hi = tone_at(3_000.0, 0.05, 48_000); // −26 dBFS
        let mut bell = base;
        bell.key_bands[2] = KeyBand {
            active: true,
            shape: EqShape::Bell,
            freq: 3_000.0,
            gain_db: 12.0,
            q: 2.0,
            ..KeyBand::default()
        };
        let reduce = |s: &CompSettings| {
            let mut c = Comprimo::new(48_000.0);
            c.set(s);
            let (mut l, mut r) = (hi.clone(), hi.clone());
            c.process(&mut l, &mut r, None);
            c.telemetry()[tele::GR]
        };
        assert_eq!(reduce(&base), 0.0);
        assert!(reduce(&bell) > 2.0, "{}", reduce(&bell));
        // Heard through Key listen, the bell is 12 dB up at its centre.
        let mut k = Comprimo::new(48_000.0);
        k.set(&CompSettings {
            listen: Listen::Key,
            ..bell
        });
        let (mut l, mut r) = (hi.clone(), hi.clone());
        k.process(&mut l, &mut r, None);
        let lift = 20.0 * math::log10(peak(&l[24_000..]) / peak(&hi));
        assert!((lift - 12.0).abs() < 0.3, "{lift}");
    }

    #[test]
    fn delta_is_what_was_taken_away_and_wet_has_no_dry() {
        let s = CompSettings {
            threshold_db: -20.0,
            attack_ms: 1.0,
            ..CompSettings::default()
        };
        let src = tone(1.0, 10_000);
        let (out, _) = run(&s, &src, &src, 100);
        let delta = CompSettings {
            listen: Listen::Delta,
            ..s
        };
        let (d, _) = run(&delta, &src, &src, 100);
        for i in 0..src.len() {
            assert!((out[i] + d[i] - src[i]).abs() < 1e-6);
        }
        // With a dry path, Wet is the output without it.
        let parallel = CompSettings {
            dry_db: 0.0,
            makeup_db: 3.0,
            ..s
        };
        let (with_dry, _) = run(&parallel, &src, &src, 100);
        let (wet, _) = run(
            &CompSettings {
                listen: Listen::Wet,
                ..parallel
            },
            &src,
            &src,
            100,
        );
        for i in 0..src.len() {
            assert!((with_dry[i] - (wet[i] + src[i])).abs() < 1e-6);
        }
    }

    /// Auto gain measures: after it settles, the output's short-term loudness is the
    /// input's, within 0.5 LU, on drum-like material 12 dB into compression.
    #[test]
    fn auto_gain_matches_the_loudness_it_measures() {
        let src = drums(48_000 * 8);
        let s = CompSettings {
            threshold_db: -30.0,
            ratio: 4.0,
            attack_ms: 3.0,
            release_ms: 80.0,
            auto_gain: true,
            ..CompSettings::default()
        };
        let mut c = Comprimo::new(48_000.0);
        c.set(&s);
        let (mut l, mut r) = (src.clone(), src.clone());
        c.process(&mut l, &mut r, None);
        let t = c.telemetry();
        assert!(t[tele::GR_AVG] > 3.0, "it compresses: {}", t[tele::GR_AVG]);
        assert!(
            t[tele::AUTO_GAIN] > 3.0,
            "and makes up: {}",
            t[tele::AUTO_GAIN]
        );
        let diff = t[tele::OUT_S] - t[tele::IN_S];
        assert!(diff.abs() < 0.5, "output {} LU from the input", diff);
        // Off, the output is that much quieter.
        let (quiet, _) = run(
            &CompSettings {
                auto_gain: false,
                ..s
            },
            &src,
            &src,
            512,
        );
        let loud_by = rms_db(&l[48_000 * 5..]) - rms_db(&quiet[48_000 * 5..]);
        assert!(loud_by > 3.0, "{loud_by}");
    }

    /// The compare is the input at the output's loudness: within 0.5 LU of the output.
    #[test]
    fn compare_is_the_bypass_at_the_same_loudness() {
        let src = drums(48_000 * 8);
        let s = CompSettings {
            threshold_db: -30.0,
            ratio: 6.0,
            makeup_db: 4.0,
            ..CompSettings::default()
        };
        let (out, _) = run(&s, &src, &src, 256);
        let (cmp, _) = run(
            &CompSettings {
                listen: Listen::Compare,
                ..s
            },
            &src,
            &src,
            256,
        );
        let m = |x: &[f32]| {
            let mut lm = loudness::Meter::new(48_000.0);
            for v in &x[48_000 * 5..48_000 * 8] {
                lm.add(*v, *v);
            }
            lm.end_hop(48_000 * 3);
            lufs_of(&lm)
        };
        fn lufs_of(lm: &loudness::Meter) -> f32 {
            // One step holding all three seconds: its mean square is the last in the
            // ring; momentary over 4 steps divides by 4, so undo that.
            lm.momentary() + 10.0 * math::log10(4f32)
        }
        let (lo, lc) = (m(&out), m(&cmp));
        assert!((lo - lc).abs() < 0.5, "output {lo} LUFS, compare {lc} LUFS");
        // And it is not the output: its crest is the input's.
        assert_ne!(out, cmp);
    }

    /// Lookahead catches the transient: with the same attack, the output's first peak
    /// over the threshold is far smaller with 5 ms of lookahead than with none.
    #[test]
    fn lookahead_catches_the_transient() {
        let mut src = vec![0.0f32; 4_800];
        src.extend(tone(1.0, 9_600));
        let s = CompSettings {
            threshold_db: -20.0,
            ratio: RATIO_INF,
            knee_db: 0.0,
            attack_ms: 1.0,
            release_ms: 200.0,
            ..CompSettings::default()
        };
        let (none, _) = run(&s, &src, &src, 256);
        let ahead = CompSettings {
            lookahead_ms: 5.0,
            ..s
        };
        let (with, _) = run(&ahead, &src, &src, 256);
        let d = 240;
        let first = |x: &[f32], at: usize| 20.0 * math::log10(peak(&x[at..at + 480]));
        let (a, b) = (first(&none, 4_800), first(&with, 4_800 + d));
        assert!(b < a - 6.0, "no lookahead {a} dB, 5 ms {b} dB");
    }

    /// Hold keeps the reduction up for as long as it says before the release begins.
    #[test]
    fn hold_holds() {
        let resp = |hold_ms: f32| {
            step_response(
                &CompSettings {
                    threshold_db: -20.0,
                    attack_ms: 0.1,
                    release_ms: 50.0,
                    hold_ms,
                    ..CompSettings::default()
                },
                48_000.0,
                1_100,
            )
            .release_ms
            .unwrap()
        };
        let (none, held) = (resp(0.0), resp(100.0));
        assert!(
            (held - none - 100.0).abs() < 2.0,
            "no hold {none} ms, 100 ms hold {held} ms"
        );
    }

    /// The response graph measures what the knobs say for the plain detector, and shows
    /// how the other styles bend it.
    #[test]
    fn the_step_response_measures_attack_and_release() {
        let base = CompSettings {
            threshold_db: -20.0,
            ratio: 4.0,
            knee_db: 0.0,
            attack_ms: 10.0,
            release_ms: 100.0,
            ..CompSettings::default()
        };
        let r = step_response(&base, 48_000.0, 500);
        assert_eq!(r.gr_db.len(), 500);
        assert!((r.settled_db - 9.0).abs() < 0.05, "{}", r.settled_db);
        let (a, rel) = (r.attack_ms.unwrap(), r.release_ms.unwrap());
        assert!((a - 10.0).abs() < 1.0, "attack measured {a} ms");
        assert!((rel - 100.0).abs() < 5.0, "release measured {rel} ms");
        // A linear release crosses the same 63 % point: the release time means the same.
        let pumping = step_response(
            &CompSettings {
                style: Style::Pumping,
                ..base
            },
            48_000.0,
            500,
        );
        let lin = pumping.release_ms.unwrap();
        assert!(lin > rel * 0.6 && lin < rel * 1.6, "linear {lin} ms");
        // A second stage holds a long burst: Mastering lets go slower than Clean.
        let mastering = step_response(
            &CompSettings {
                style: Style::Mastering,
                ..base
            },
            48_000.0,
            500,
        );
        assert!(mastering.release_ms.unwrap() > rel * 1.5);
        // Nothing to measure when it never compresses.
        let off = step_response(&CompSettings { ratio: 1.0, ..base }, 48_000.0, 100);
        assert_eq!(off.attack_ms, None);
    }

    /// A feedback detector settles where the effective curve says, which is softer than
    /// the static one — the curve the editor draws for it.
    #[test]
    fn feedback_settles_on_the_effective_curve() {
        let s = CompSettings {
            threshold_db: -30.0,
            ratio: 8.0,
            knee_db: 0.0,
            attack_ms: 1.0,
            style: Style::Classic,
            ..CompSettings::default()
        };
        let r = step_response(&s, 48_000.0, 100);
        let want = effective_gain_reduction_db(-18.0, &s);
        assert!(
            (r.settled_db - want).abs() < 0.1,
            "settled {} dB, effective curve {want} dB",
            r.settled_db
        );
        assert!(
            want < gain_reduction_db(-18.0, &s) - 1.0,
            "feedback is softer"
        );
        let ff = CompSettings {
            style: Style::Clean,
            ..s
        };
        assert_eq!(
            effective_gain_reduction_db(-18.0, &ff),
            gain_reduction_db(-18.0, &ff)
        );
    }

    /// The engine drawer's numbers are the detector in force: a custom spec equal to a
    /// style's is that style, to the bit.
    #[test]
    fn a_custom_engine_equal_to_a_style_is_that_style() {
        let src = drums(48_000);
        for style in Style::ALL {
            let s = CompSettings {
                style,
                threshold_db: -28.0,
                ..CompSettings::default()
            };
            let custom = CompSettings {
                style: Style::Clean,
                engine: Some(style.spec()),
                ..s
            };
            assert_eq!(
                run(&s, &src, &src, 512),
                run(&custom, &src, &src, 512),
                "{style:?}"
            );
        }
    }

    /// Ten seconds of statistics: the mean and peak reduction and the crest factor, in and
    /// out, on material whose numbers are known.
    #[test]
    fn statistics_read_what_happened() {
        // A steady sine: crest 3.01 dB in; at ∞:1 it settles at the threshold.
        let src = tone(1.0, 48_000 * 11);
        let s = CompSettings {
            threshold_db: -12.0,
            ratio: RATIO_INF,
            knee_db: 0.0,
            attack_ms: 0.01,
            ..CompSettings::default()
        };
        let mut c = Comprimo::new(48_000.0);
        c.set(&s);
        let (mut l, mut r) = (src.clone(), src.clone());
        c.process(&mut l, &mut r, None);
        let t = c.telemetry();
        assert!(
            (t[tele::CREST_IN] - 3.01).abs() < 0.05,
            "{}",
            t[tele::CREST_IN]
        );
        assert!((t[tele::GR_PEAK] - 12.0).abs() < 0.1);
        assert!((t[tele::GR_AVG] - 12.0).abs() < 0.3, "{}", t[tele::GR_AVG]);
        assert_eq!(t[tele::HOPS], 110.0);
        // Drums with their peaks caught (instant attack, a millisecond of lookahead) lose
        // crest.
        let src = drums(48_000 * 10);
        let mut c = Comprimo::new(48_000.0);
        c.set(&CompSettings {
            threshold_db: -30.0,
            ratio: 8.0,
            attack_ms: 0.005,
            lookahead_ms: 1.0,
            release_ms: 60.0,
            ..CompSettings::default()
        });
        let (mut l, mut r) = (src.clone(), src.clone());
        c.process(&mut l, &mut r, None);
        let t = c.telemetry();
        assert!(
            t[tele::CREST_OUT] < t[tele::CREST_IN] - 3.0,
            "crest {} → {}",
            t[tele::CREST_IN],
            t[tele::CREST_OUT]
        );
        // The blocks reported are the output's momentary loudness, newest first.
        assert_eq!(t[tele::BLOCKS], t[tele::OUT_M]);
    }

    /// No sample that is not a number, and no state left hanging in the denormals, at the
    /// extremes of every range.
    #[test]
    fn extremes_stay_finite_and_silence_goes_to_zero() {
        let mut spec = CompSettings {
            threshold_db: -80.0,
            ratio: RATIO_INF,
            knee_db: 72.0,
            attack_ms: 0.005,
            release_ms: 5_000.0,
            hold_ms: 500.0,
            lookahead_ms: 20.0,
            makeup_db: 36.0,
            auto_gain: true,
            dry_db: 12.0,
            mix: 2.0,
            input_db: 24.0,
            output_db: -24.0,
            link: 0.0,
            key_hpf_hz: 500.0,
            key_gain_db: 24.0,
            engine: Some(StyleSpec {
                rms_ms: 300.0,
                feedback: 1.0,
                program_release: 4.0,
                dual: 1.0,
                curve: 1.0,
            }),
            ..CompSettings::default()
        };
        spec.key_bands[5] = KeyBand {
            active: true,
            shape: EqShape::HighCut,
            freq: 30.0,
            slope: 96.0,
            ..KeyBand::default()
        };
        let mut c = Comprimo::new(48_000.0);
        c.set(&spec);
        let mut l = drums(48_000);
        let mut r = l.iter().map(|x| -x).collect::<Vec<_>>();
        c.process(&mut l, &mut r, None);
        assert!(l.iter().chain(&r).all(|x| x.is_finite()));
        // Thirty seconds of silence: the filters (a 96 dB/octave cut at 30 Hz is the
        // slowest to die away) and the RMS window go to exact zero, not to the denormals.
        // The reduction is still letting go — a 5 s release that slows to 45 s under
        // program dependence is meant to be slow.
        let mut quiet = vec![0.0f32; 48_000 * 30];
        let mut quiet_r = quiet.clone();
        c.process(&mut quiet, &mut quiet_r, None);
        assert!(quiet.iter().all(|x| x.is_finite()));
        assert_eq!(c.ms, [0.0; 2]);
        assert!(c
            .key_z
            .iter()
            .flatten()
            .flatten()
            .flatten()
            .all(|z| *z == 0.0));
        // With a short release it lands on exact zero too, and so does the output.
        c.set(&CompSettings {
            release_ms: 5.0,
            engine: None,
            ..spec
        });
        let (mut a, mut b) = (vec![0.0f32; 48_000 * 2], vec![0.0f32; 48_000 * 2]);
        c.process(&mut a, &mut b, None);
        assert_eq!(c.gr, [0.0; 2]);
        assert_eq!(*a.last().unwrap(), 0.0);
    }

    /// Oversampled and not compressing, the gain stage is the reported latency's delay:
    /// flat to within ±0.01 dB up to 18 kHz, and the waveform lines up with the input
    /// delayed by exactly that many samples (the error under −60 dB of the signal).
    #[test]
    fn oversampling_is_a_clean_delay_of_its_latency() {
        for (factor, latency) in [(2usize, 31usize), (4, 36)] {
            assert_eq!(oversample::latency(factor), latency);
            for hz in [100.0f32, 1_000.0, 10_000.0, 18_000.0] {
                let src = tone_at(hz, 0.5, 24_000);
                let s = CompSettings {
                    threshold_db: 0.0,
                    ratio: 1.0,
                    oversample: factor,
                    ..CompSettings::default()
                };
                let mut c = Comprimo::new(48_000.0);
                c.set(&s);
                assert_eq!(c.latency(), latency);
                let (mut l, mut r) = (src.clone(), src.clone());
                c.process(&mut l, &mut r, None);
                let (out, want) = (&l[12_000..], &src[12_000 - latency..24_000 - latency]);
                let gain = 20.0 * math::log10(peak(out) / peak(want));
                assert!(gain.abs() < 0.01, "{factor}× at {hz} Hz: {gain} dB");
                let err: Vec<f32> = out.iter().zip(want).map(|(a, b)| a - b).collect();
                let rel = rms_db(&err) - rms_db(want);
                assert!(rel < -60.0, "{factor}× at {hz} Hz: error {rel} dB");
            }
        }
    }

    /// The filters keep what they must: a 20 kHz tone up-sampled has its image (at 28 kHz,
    /// where aliasing would land) at least 70 dB down.
    #[test]
    fn the_oversampling_filter_rejects_the_image() {
        let mut os = oversample::Oversampler::new();
        os.set_factor(2);
        let n = 16_384;
        let mut hi = Vec::with_capacity(2 * n);
        for i in 0..n {
            let x = math::sin(std::f32::consts::TAU * 20_000.0 * i as f32 / 48_000.0);
            let mut ph = [0f32; 4];
            os.up(0, x, &mut ph);
            hi.extend_from_slice(&ph[..2]);
        }
        // Correlate with the tone and with its image, at 96 kHz.
        let at = |hz: f32| {
            let (mut c, mut s) = (0f64, 0f64);
            for (i, v) in hi[4_096..].iter().enumerate() {
                let w = std::f64::consts::TAU * hz as f64 * i as f64 / 96_000.0;
                c += *v as f64 * math::cos(w);
                s += *v as f64 * math::sin(w);
            }
            (c * c + s * s).sqrt()
        };
        let (tone, image) = (at(20_000.0), at(28_000.0));
        let rejection = 20.0 * math::log10(image / tone);
        assert!(rejection < -70.0, "image at {rejection} dB");
    }

    /// Oversampled, the detector hears the peaks between samples: a 12 kHz sine whose
    /// samples all land at 71 % of its peak reads −3 dB at 1× and its true 0 dB at 2× and
    /// 4× — so a threshold at −1 dB lets it through at 1× and catches it oversampled.
    #[test]
    fn oversampled_the_detector_hears_true_peaks() {
        let src: Vec<f32> = (0..24_000)
            .map(|i| {
                math::sin(std::f32::consts::FRAC_PI_2 * i as f32 + std::f32::consts::FRAC_PI_4)
            })
            .collect();
        let gr = |factor: usize| {
            let mut c = Comprimo::new(48_000.0);
            c.set(&CompSettings {
                threshold_db: -1.0,
                ratio: RATIO_INF,
                knee_db: 0.0,
                attack_ms: 0.005,
                oversample: factor,
                ..CompSettings::default()
            });
            let (mut l, mut r) = (src.clone(), src.clone());
            c.process(&mut l, &mut r, None);
            c.telemetry()[tele::GR]
        };
        assert_eq!(gr(1), 0.0, "sample peaks at −3 dB stay under −1 dB");
        for factor in [2, 4] {
            let g = gr(factor);
            assert!((g - 1.0).abs() < 0.1, "{factor}×: {g} dB");
        }
    }

    /// Everything at once — lookahead, the key's EQ, dual and linear release, feedback,
    /// auto gain, the dry path, the compare — gives the same samples and the same
    /// telemetry whatever the block size, down to one sample.
    #[test]
    fn the_result_does_not_depend_on_the_block_size() {
        let mut settings = vec![];
        for style in Style::ALL {
            settings.push(CompSettings {
                style,
                threshold_db: -24.0,
                hold_ms: 5.0,
                auto_release: true,
                key_hpf_hz: 120.0,
                link: 0.5,
                ..CompSettings::default()
            });
        }
        let mut all = CompSettings {
            threshold_db: -30.0,
            lookahead_ms: 3.3,
            auto_gain: true,
            dry_db: -12.0,
            mix: 1.4,
            input_db: 2.0,
            output_db: -1.0,
            engine: Some(StyleSpec {
                rms_ms: 7.0,
                feedback: 0.3,
                program_release: 1.0,
                dual: 0.6,
                curve: 0.5,
            }),
            ..CompSettings::default()
        };
        all.key_bands[1] = KeyBand {
            active: true,
            shape: EqShape::LowCut,
            freq: 90.0,
            slope: 24.0,
            ..KeyBand::default()
        };
        all.key_bands[3] = KeyBand {
            active: true,
            shape: EqShape::Bell,
            freq: 2_500.0,
            gain_db: 6.0,
            ..KeyBand::default()
        };
        settings.push(all);
        settings.push(CompSettings {
            listen: Listen::Compare,
            ..all
        });
        settings.push(CompSettings {
            oversample: 4,
            listen: Listen::Delta,
            ..all
        });
        settings.push(CompSettings {
            oversample: 2,
            ..all
        });
        let src = drums(40_000);
        let right: Vec<f32> = src.iter().map(|x| x * 0.5).collect();
        for s in settings {
            let whole = |block: usize| {
                let mut c = Comprimo::new(48_000.0);
                c.set(&s);
                let (mut l, mut r) = (src.clone(), right.clone());
                for (a, b) in l.chunks_mut(block).zip(r.chunks_mut(block)) {
                    c.process(a, b, None);
                }
                (l, r, c.telemetry())
            };
            let reference = whole(512);
            for block in [1usize, 7, 64, 1000, 4096] {
                let got = whole(block);
                assert_eq!(got.0, reference.0, "{:?} at block {block}", s.style);
                assert_eq!(got.1, reference.1, "{:?} at block {block}", s.style);
                // The last block's peaks differ with the block's length; the rest must
                // not.
                assert_eq!(
                    got.2[..tele::IN_PEAK],
                    reference.2[..tele::IN_PEAK],
                    "telemetry at block {block}"
                );
            }
        }
    }
}

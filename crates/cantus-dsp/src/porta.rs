//! Porta: the gate and expander. A gate, a downward expander, an upward expander and a
//! ducker in one detector, with hysteresis, hold, lookahead, a filtered key that can come
//! from any strip, and a way to hear the key or what the gate takes away.
//!
//! ## One detector, a state machine, and a gain that moves in real time
//!
//! 1. **The key** — the input, or another strip's output — goes through the detector's
//!    high- and low-pass (Butterworth cascades from [`crate::aequalis::design`], matched to
//!    the analog curve), then a peak or RMS follower (RMS is sine-calibrated: a sine reads
//!    its peak level whatever the style, so a threshold means the same dB in every style).
//! 2. **A state machine with hysteresis**: the gate *opens* when the key reaches the
//!    threshold and *closes* only when it falls below the close threshold, `hyst` dB lower.
//!    After it falls, it *holds* fully open for the hold time, then *releases*. Four phases —
//!    closed, open, hold, release — and the display draws each one.
//! 3. **The static curve** ([`static_gain_db`]): below the threshold a downward expander
//!    of `ratio` (∞ is a gate), limited to `range` (∞ is silence); the Upward style raises
//!    what is above it instead, and Ducking lowers the signal while the key is above it. An
//!    open gate compares against the close threshold, a closed one against the open one: that
//!    is what hysteresis is.
//! 4. **The motion**: the gain travels in dB along a ramp whose speed is set so that a
//!    full swing (the whole range) takes exactly the attack or release time to come within
//!    1 % of its end, then through a one-pole that rounds the corners. The style sets how
//!    round (`StyleSpec::shape`: nearly straight lines in dB for Classic — exponential
//!    fades, the console sound — an S for Clean); the time you set is the time it takes in
//!    every style.
//!
//! ## Lookahead that does not move the latency
//!
//! With any lookahead at all, the audio is delayed by a fixed [`LOOKAHEAD_MAX_MS`] and the
//! key by that minus the lookahead, so the detector hears `look` ms early and the latency
//! the host compensates does not change while the knob turns. Off (0 ms) costs nothing.
//!
//! ## The null-test contract
//!
//! A gate that is fully open multiplies by exactly 1.0: the gain lands on 0 dB exactly (the
//! one-pole snaps within 10⁻⁴ dB) and 0 dB is not turned into a power of ten. So a signal
//! that never falls below the threshold comes out as it went in, to the bit — delayed by
//! the latency when lookahead is on. At `range` 0 nothing moves at all. Every step is per
//! sample, so the output does not depend on the block size; states below 10⁻²⁰ are
//! flushed, so silence stays exactly silent and no denormal stalls the CPU.

use crate::aequalis::{design, Coefs, EqShape, MAX_SECTIONS};
use crate::math;

/// With lookahead on, the audio is always this late, whatever the lookahead amount.
pub const LOOKAHEAD_MAX_MS: f32 = 10.0;
/// A ratio at or above this is ∞: a gate.
pub const INFINITE_RATIO: f32 = 99.5;
/// A range at or above this is ∞: a closed gate is silence.
pub const INFINITE_RANGE: f32 = 99.95;
/// The most the Upward style adds, whatever the range says.
pub const MAX_BOOST_DB: f32 = 24.0;
/// One column of the display's history, ms.
pub const COLUMN_MS: f32 = 5.0;
/// How long after an opening its peak is measured, for the hit's velocity.
pub const HIT_WINDOW_MS: f32 = 10.0;
/// Below this, a gain with ∞ range is exact silence.
const SILENCE_DB: f32 = -99.0;
/// The level of silence, dB.
const FLOOR_DB: f32 = -200.0;
/// The slope used for "∞:1".
const STEEP: f32 = 1.0e4;

/// What the gate does with its curve.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Mode {
    /// Lowers what is below the threshold: a gate or a downward expander.
    Down,
    /// Raises what is above the threshold: an upward expander.
    Up,
    /// Lowers the signal while the key is above the threshold.
    Duck,
}

/// One style, spelled out: nothing about it is hidden.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct StyleSpec {
    pub mode: Mode,
    /// 0 = a peak follower; otherwise the RMS window, ms.
    pub rms_ms: f32,
    /// How fast the peak follower lets go, ms (peak styles only).
    pub peak_fall_ms: f32,
    /// How round the opening and closing are: the share of the time spent rounding the
    /// corners. 0.02 is straight lines in dB (exponential fades); 0.15 is an S.
    pub shape: f32,
    /// The closing follows the key: the nearer the key is to the close threshold, the
    /// slower it closes — a tenth of the set speed just under it, the full speed 12 dB
    /// under — so a note fading through the threshold is let down, not cut.
    pub follow: bool,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Style {
    Classic,
    Clean,
    Vocal,
    Guitar,
    Upward,
    Ducking,
}

impl Style {
    pub const ALL: [Style; 6] = [
        Style::Classic,
        Style::Clean,
        Style::Vocal,
        Style::Guitar,
        Style::Upward,
        Style::Ducking,
    ];

    pub fn from_index(i: usize) -> Style {
        Self::ALL[i.min(Self::ALL.len() - 1)]
    }

    pub fn index(self) -> usize {
        Self::ALL.iter().position(|s| *s == self).unwrap_or(0)
    }

    pub fn spec(self) -> StyleSpec {
        let down = |rms_ms, peak_fall_ms, shape, follow| StyleSpec {
            mode: Mode::Down,
            rms_ms,
            peak_fall_ms,
            shape,
            follow,
        };
        match self {
            Style::Classic => down(0.0, 10.0, 0.02, false),
            Style::Clean => down(0.0, 30.0, 0.15, false),
            Style::Vocal => down(10.0, 0.0, 0.18, false),
            Style::Guitar => down(20.0, 0.0, 0.08, true),
            Style::Upward => StyleSpec {
                mode: Mode::Up,
                rms_ms: 0.0,
                peak_fall_ms: 10.0,
                shape: 0.1,
                follow: false,
            },
            Style::Ducking => StyleSpec {
                mode: Mode::Duck,
                rms_ms: 5.0,
                peak_fall_ms: 0.0,
                shape: 0.12,
                follow: false,
            },
        }
    }
}

/// What to hear.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Listen {
    /// The gated signal.
    Gate,
    /// The detector's input: the key, after its filters.
    Key,
    /// What the gate takes away (input − output); in the Upward style, what it adds.
    Delta,
}

impl Listen {
    pub fn from_index(i: usize) -> Listen {
        match i {
            1 => Listen::Key,
            2 => Listen::Delta,
            _ => Listen::Gate,
        }
    }
}

/// Where the gate is. The display's state lane draws these; they are ordered from least to
/// most open, so a column's phase is the most open one it saw.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Phase {
    Closed = 0,
    Release = 1,
    Hold = 2,
    Open = 3,
}

impl Phase {
    pub fn from_code(c: u32) -> Phase {
        match c {
            1 => Phase::Release,
            2 => Phase::Hold,
            3 => Phase::Open,
            _ => Phase::Closed,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct GateSettings {
    pub threshold_db: f32,
    /// How far below the threshold it closes.
    pub hysteresis_db: f32,
    /// 1 … 100; at [`INFINITE_RATIO`] and above, ∞.
    pub ratio: f32,
    /// The most it takes away, dB; at [`INFINITE_RANGE`] and above, ∞ (silence).
    pub range_db: f32,
    pub knee_db: f32,
    pub attack_ms: f32,
    pub hold_ms: f32,
    pub release_ms: f32,
    /// 0 is off (and costs no latency).
    pub lookahead_ms: f32,
    pub style: Style,
    /// The key's high-pass, Hz; 20 and below is off.
    pub hpf_hz: f32,
    /// The key's low-pass, Hz; 20 kHz and above is off.
    pub lpf_hz: f32,
    /// dB/octave, 12 … 48.
    pub key_slope: f32,
    pub listen: Listen,
    /// Opened by notes ([`Porta::note`]) instead of the key's level: a note-on opens it, the
    /// last note-off lets it hold and release (ADR 0022).
    pub midi: bool,
    /// With notes: how much less the softest note opens it than the hardest, dB — 0 opens
    /// every note fully.
    pub vel_db: f32,
}

/// A note the gate hears, in MIDI mode ([`GateSettings::midi`]).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Trigger {
    On {
        pitch: u8,
        velocity: u8,
    },
    Off {
        pitch: u8,
    },
    /// Every note stops (the transport stopped, sought or looped).
    AllOff,
}

/// Notes waiting for their sample: two blocks of what an exported plug-in takes in one
/// (`cantus_plugins::instance::NOTE_CAPACITY`, 1 024). With lookahead a note waits up to
/// the key's delay (at most [`LOOKAHEAD_MAX_MS`]), so one block's notes may still wait when
/// the next block's arrive: no note acts early while the notes arriving within any 10 ms
/// fit here — always, for host blocks at least that long. Past it the oldest acts early
/// (see [`Porta::note`]): its timing is lost, never a note.
pub const MAX_TRIGGERS: usize = 2048;

impl Default for GateSettings {
    fn default() -> Self {
        GateSettings {
            threshold_db: -40.0,
            hysteresis_db: 4.0,
            ratio: 100.0,
            range_db: 40.0,
            knee_db: 0.0,
            attack_ms: 1.0,
            hold_ms: 20.0,
            release_ms: 150.0,
            lookahead_ms: 0.0,
            style: Style::Classic,
            hpf_hz: 20.0,
            lpf_hz: 20_000.0,
            key_slope: 24.0,
            listen: Listen::Gate,
            midi: false,
            vel_db: 0.0,
        }
    }
}

impl GateSettings {
    /// The level at which an open gate closes.
    pub fn close_db(&self) -> f32 {
        self.threshold_db - self.hysteresis_db.max(0.0)
    }

    pub fn infinite_range(&self) -> bool {
        self.range_db >= INFINITE_RANGE
    }

    pub fn infinite_ratio(&self) -> bool {
        self.ratio >= INFINITE_RATIO
    }

    /// The full swing of the gain, dB: the range (Upward: at most [`MAX_BOOST_DB`]).
    pub fn swing_db(&self) -> f32 {
        let r = self.range_db.clamp(0.0, 100.0);
        match self.style.spec().mode {
            Mode::Up => r.min(MAX_BOOST_DB),
            _ => r,
        }
    }

    fn slope(&self) -> f32 {
        if self.infinite_ratio() {
            STEEP
        } else {
            (self.ratio - 1.0).max(0.0)
        }
    }

    pub fn hpf_on(&self) -> bool {
        self.hpf_hz > 20.5
    }

    pub fn lpf_on(&self) -> bool {
        self.lpf_hz < 19_999.0
    }
}

/// Expansion below a threshold: the gain (≤ 0 dB) for being `over` dB above it.
fn below(over: f32, slope: f32, knee: f32) -> f32 {
    let k = knee.max(0.0);
    if k > 0.0 && over.abs() < k * 0.5 {
        -slope * math::powi(k * 0.5 - over, 2) / (2.0 * k)
    } else if over < 0.0 {
        slope * over
    } else {
        0.0
    }
}

/// Expansion above a threshold: the gain (≥ 0 dB) for being `over` dB above it.
fn above(over: f32, slope: f32, knee: f32) -> f32 {
    let k = knee.max(0.0);
    if k > 0.0 && over.abs() < k * 0.5 {
        slope * math::powi(over + k * 0.5, 2) / (2.0 * k)
    } else if over > 0.0 {
        slope * over
    } else {
        0.0
    }
}

/// The gain the static curve asks for at a key level, before any timing, dB. `open` says
/// which threshold the gate compares against: an open one, the close threshold. In the
/// Upward style the curve does not depend on it (a continuous boost needs no hysteresis).
pub fn static_gain_db(level_db: f32, s: &GateSettings, open: bool) -> f32 {
    let swing = s.swing_db();
    let t = if open { s.close_db() } else { s.threshold_db };
    match s.style.spec().mode {
        Mode::Down => below(level_db - t, s.slope(), s.knee_db).max(-swing),
        Mode::Up => above(level_db - s.threshold_db, s.slope(), s.knee_db).min(swing),
        Mode::Duck => -above(level_db - t, s.slope(), s.knee_db).min(swing),
    }
}

/// The output level for a key level on the static curve — what the transfer display draws.
pub fn transfer_db(level_db: f32, s: &GateSettings, open: bool) -> f32 {
    level_db + static_gain_db(level_db, s, open)
}

/// The key filter's response at `hz`, dB — what the key strip draws.
pub fn key_filter_db(s: &GateSettings, sample_rate: f32, hz: f32) -> f32 {
    let mut c = [Coefs::IDENTITY; 2 * MAX_SECTIONS];
    let n = key_filter(s, sample_rate, &mut c);
    c[..n].iter().map(|c| c.magnitude_db(sample_rate, hz)).sum()
}

/// Design the key's filters into `out`; returns how many sections are used.
fn key_filter(s: &GateSettings, sr: f32, out: &mut [Coefs; 2 * MAX_SECTIONS]) -> usize {
    let slope = (s.key_slope / 6.0).round().clamp(2.0, 8.0) * 6.0;
    let mut n = 0;
    let mut add = |shape: EqShape, hz: f32| {
        let mut c = [Coefs::IDENTITY; MAX_SECTIONS];
        let k = design(
            shape,
            sr,
            hz,
            0.0,
            std::f32::consts::FRAC_1_SQRT_2,
            slope,
            &mut c,
        );
        for c in &c[..k] {
            out[n] = *c;
            n += 1;
        }
    };
    if s.hpf_on() {
        add(EqShape::LowCut, s.hpf_hz);
    }
    // A low-pass at or above 0.45 fs is no low-pass at all.
    if s.lpf_on() && s.lpf_hz < sr * 0.45 {
        add(EqShape::HighCut, s.lpf_hz);
    }
    n
}

/// The share of the time the ramp takes, so that ramp plus rounding reach 99 % of a full
/// swing in exactly the time set. With the rounding a one-pole of `f`·T, the ramp of `c`·T
/// arrives lagging by `(f/c)(1 − e^{−c/f})` of the swing, which then decays with `f`·T:
/// `c + f·ln(100 (f/c)(1 − e^{−c/f})) = 1`, solved by bisection.
pub fn ramp_share(f: f32) -> f32 {
    let f = f as f64;
    if f <= 1e-6 {
        return 1.0;
    }
    let g = |c: f64| c + f * math::ln(100.0 * (f / c) * (1.0 - math::exp(-c / f))) - 1.0;
    let (mut lo, mut hi) = (1e-6f64, 1.0f64);
    if g(lo) >= 0.0 {
        return lo as f32;
    }
    for _ in 0..50 {
        let mid = 0.5 * (lo + hi);
        if g(mid) < 0.0 {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    (0.5 * (lo + hi)) as f32
}

/// One section's state (transposed direct form II).
#[derive(Copy, Clone, Debug, Default)]
struct Sec {
    z1: f32,
    z2: f32,
}

impl Sec {
    #[inline]
    fn tick(&mut self, c: &Coefs, x: f32) -> f32 {
        let y = c.b0 * x + self.z1;
        self.z1 = c.b1 * x - c.a1 * y + self.z2;
        self.z2 = c.b2 * x - c.a2 * y;
        // Both at once: flushing one alone feeds the other a kick, and a section with its
        // poles near 1 can then hover at the threshold instead of coming to rest.
        if self.z1.abs() < 1e-20 && self.z2.abs() < 1e-20 {
            self.z1 = 0.0;
            self.z2 = 0.0;
        }
        y
    }
}

/// One column of the display's history, measured on the input's timeline (the key delayed
/// by the lookahead, so the picture lines up with the audio it acted on).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Column {
    /// Peak input, dB.
    pub input_db: f32,
    /// Peak of the gated signal, dB.
    pub output_db: f32,
    /// Loudest detector level, dB.
    pub key_db: f32,
    /// The deepest gain change (Upward: the largest boost), dB.
    pub gain_db: f32,
    /// The most open phase in the column.
    pub phase: Phase,
}

impl Default for Column {
    fn default() -> Self {
        Column {
            input_db: -120.0,
            output_db: -120.0,
            key_db: -120.0,
            gain_db: 0.0,
            phase: Phase::Closed,
        }
    }
}

/// The 64 numbers the editor reads (`cantus_engine`'s `FxUnit::telemetry`). Columns and
/// dB values are packed as whole numbers below 2²⁴, which an `f32` holds exactly.
pub mod telemetry {
    use super::{Column, Phase};

    /// Columns completed since the gate was made, modulo 2²⁴.
    pub const SEQ: usize = 0;
    /// One column's length, ms.
    pub const COLUMN_MS: usize = 1;
    /// The detector's level now, dB.
    pub const LEVEL: usize = 2;
    /// The gain now, dB (−120 for silence).
    pub const GAIN: usize = 3;
    /// The phase now (`Phase as u32`).
    pub const PHASE: usize = 4;
    /// Latency, samples.
    pub const LATENCY: usize = 5;
    /// Openings measured so far (hits), modulo 2²⁴.
    pub const HITS: usize = 6;
    /// The sample rate, Hz (to turn a hit's samples into song time).
    pub const SAMPLE_RATE: usize = 7;
    /// The last [`super::EVENTS`] hits, newest first, two numbers each: how many samples
    /// before the end of the last block the key crossed the threshold (on the input's
    /// timeline), and the key's peak in the [`super::HIT_WINDOW_MS`] after it, dB.
    pub const EVENTS_AT: usize = 8;
    /// The last [`super::HISTORY_COLUMNS`] columns, oldest first, two numbers each.
    pub const COLUMNS_AT: usize = 16;
    pub const WRAP: u32 = 1 << 24;

    /// dB to a whole number in 0.1 dB steps, −120 … +84 dB (11 bits).
    pub fn pack_db(db: f32) -> u32 {
        let db = if db.is_finite() { db } else { -120.0 };
        ((db.clamp(-120.0, 84.0) + 120.0) * 10.0).round() as u32
    }

    pub fn unpack_db(q: u32) -> f32 {
        (q & 0x7ff) as f32 / 10.0 - 120.0
    }

    /// A column as two exactly representable numbers.
    pub fn pack(c: &Column) -> [f32; 2] {
        let a = (pack_db(c.input_db) << 11) | pack_db(c.output_db);
        let b = ((c.phase as u32) << 22) | (pack_db(c.key_db) << 11) | pack_db(c.gain_db);
        [a as f32, b as f32]
    }

    pub fn unpack(a: f32, b: f32) -> Column {
        let (a, b) = (a.max(0.0) as u32, b.max(0.0) as u32);
        Column {
            input_db: unpack_db(a >> 11),
            output_db: unpack_db(a),
            key_db: unpack_db(b >> 11),
            gain_db: unpack_db(b),
            phase: Phase::from_code((b >> 22) & 3),
        }
    }

    /// How many new items a counter says arrived since `last`, allowing for the wrap.
    pub fn since(now: f32, last: f32) -> u32 {
        let (n, l) = (now.max(0.0) as u32, last.max(0.0) as u32);
        (n + WRAP - l) % WRAP
    }
}

/// Columns kept for the telemetry: at [`COLUMN_MS`] each, 120 ms — an editor that reads
/// every frame misses none.
pub const HISTORY_COLUMNS: usize = 24;
/// Hits kept for the telemetry.
pub const EVENTS: usize = 4;

/// Porta's processing state.
#[derive(Clone, Debug)]
pub struct Porta {
    sr: f32,
    s: GateSettings,
    spec: StyleSpec,
    // What `set` derives.
    /// The ramp's step per sample, in the style's domain (dB or amplitude).
    step_att: f32,
    step_rel: f32,
    pole_att: f32,
    pole_rel: f32,
    hold_n: u32,
    det_coef: f32,
    delay: usize,
    key_delay: usize,
    look_n: usize,
    coefs: [Coefs; 2 * MAX_SECTIONS],
    sections: usize,
    // Running state.
    filt: [[Sec; 2 * MAX_SECTIONS]; 2],
    env: [f32; 2],
    level_db: f32,
    phase: Phase,
    hold_left: u32,
    target_db: f32,
    /// The ramp and its rounding, in the style's domain.
    ramp: f32,
    smooth: f32,
    gain_db: f32,
    gain: f32,
    // Delay lines (allocated once, in `new`).
    cap: usize,
    pos: usize,
    audio: [Vec<f32>; 2],
    key: [Vec<f32>; 2],
    /// Zeros written into `audio` and `key` in a row (ADR 0031 §5.1).
    quiet: [crate::rest::ZeroRun; 4],
    levels: Vec<f32>,
    // The display's history and the hits.
    n: u64,
    col_len: u32,
    col_left: u32,
    col: Column,
    /// The column's input and output peaks, linear (made dB once, when it closes).
    col_in: f32,
    col_out: f32,
    cols: [[f32; 2]; HISTORY_COLUMNS],
    col_head: usize,
    cols_done: u32,
    events: [(u64, f32); EVENTS],
    ev_head: usize,
    events_done: u32,
    /// A hit being measured: where the key crossed (input timeline), its peak so far, and
    /// samples left in its window.
    pending: Option<(u64, f32, u32)>,
    /// The key has been below the close threshold since the last hit.
    armed: bool,
    /// Nothing processed since `new` or `reset`.
    fresh: bool,
    // MIDI mode: the notes waiting for their sample (due on the key's timeline), the
    // pitches held, and the velocity of the last note-on.
    triggers: [(u64, Trigger); MAX_TRIGGERS],
    trigger_count: usize,
    /// The earliest `due` among the waiting notes (`u64::MAX`: none), so a sample with
    /// nothing due costs one comparison.
    next_due: u64,
    held: [bool; 128],
    held_count: u32,
    velocity: u8,
}

impl Porta {
    pub fn new(sample_rate: f32) -> Self {
        let sr = sample_rate.max(1.0);
        let cap = (LOOKAHEAD_MAX_MS * 0.001 * sr).ceil() as usize + 2;
        let col_len = ((COLUMN_MS * 0.001 * sr).round() as u32).max(1);
        let mut p = Porta {
            sr,
            s: GateSettings::default(),
            spec: Style::Classic.spec(),
            step_att: 0.0,
            step_rel: 0.0,
            pole_att: 0.0,
            pole_rel: 0.0,
            hold_n: 0,
            det_coef: 0.0,
            delay: 0,
            key_delay: 0,
            look_n: 0,
            coefs: [Coefs::IDENTITY; 2 * MAX_SECTIONS],
            sections: 0,
            filt: [[Sec::default(); 2 * MAX_SECTIONS]; 2],
            env: [0.0; 2],
            level_db: FLOOR_DB,
            phase: Phase::Hold,
            hold_left: 0,
            target_db: 0.0,
            ramp: 0.0,
            smooth: 0.0,
            gain_db: 0.0,
            gain: 1.0,
            cap,
            pos: 0,
            audio: [vec![0.0; cap], vec![0.0; cap]],
            key: [vec![0.0; cap], vec![0.0; cap]],
            quiet: [crate::rest::ZeroRun::FULL; 4],
            levels: vec![FLOOR_DB; cap],
            n: 0,
            col_len,
            col_left: col_len,
            col: Column::default(),
            col_in: 0.0,
            col_out: 0.0,
            cols: [telemetry::pack(&Column::default()); HISTORY_COLUMNS],
            col_head: 0,
            cols_done: 0,
            events: [(0, FLOOR_DB); EVENTS],
            ev_head: 0,
            events_done: 0,
            pending: None,
            armed: true,
            fresh: true,
            triggers: [(0, Trigger::AllOff); MAX_TRIGGERS],
            trigger_count: 0,
            next_due: u64::MAX,
            held: [false; 128],
            held_count: 0,
            velocity: 127,
        };
        p.set(&GateSettings::default());
        p
    }

    /// A note, `offset` samples into the next [`Self::process`] call (MIDI mode; ignored
    /// otherwise, but kept track of, so switching modes finds the notes held). It acts on
    /// the key's timeline — with lookahead, as early as the key would be heard — exactly
    /// at its sample, whatever the block size. No allocation; a note past [`MAX_TRIGGERS`]
    /// waiting at once makes room by acting on the oldest at once, before it is due: its
    /// timing is lost, never a note (a dropped note-off would hold the gate open for good).
    pub fn note(&mut self, offset: usize, t: Trigger) {
        if self.trigger_count == MAX_TRIGGERS {
            let (_, oldest) = self.triggers[0];
            self.triggers.copy_within(1.., 0);
            self.trigger_count -= 1;
            self.apply_trigger(oldest);
        }
        let due = self.n + offset as u64 + self.key_delay as u64;
        self.triggers[self.trigger_count] = (due, t);
        self.trigger_count += 1;
        self.next_due = self.next_due.min(due);
    }

    /// Apply the notes due by now, in the order they came.
    fn apply_triggers(&mut self) {
        let now = self.n;
        let mut kept = 0;
        let mut next = u64::MAX;
        // In place: what stays moves down, never past what is still to be read.
        for i in 0..self.trigger_count {
            let (due, t) = self.triggers[i];
            if due > now {
                self.triggers[kept] = (due, t);
                kept += 1;
                next = next.min(due);
                continue;
            }
            self.apply_trigger(t);
        }
        self.trigger_count = kept;
        self.next_due = next;
    }

    /// One note acting on the notes held.
    fn apply_trigger(&mut self, t: Trigger) {
        match t {
            Trigger::On { pitch, velocity } => {
                let p = (pitch & 0x7f) as usize;
                if !self.held[p] {
                    self.held[p] = true;
                    self.held_count += 1;
                }
                self.velocity = velocity.clamp(1, 127);
            }
            Trigger::Off { pitch } => {
                let p = (pitch & 0x7f) as usize;
                if self.held[p] {
                    self.held[p] = false;
                    self.held_count -= 1;
                }
            }
            Trigger::AllOff => {
                self.held = [false; 128];
                self.held_count = 0;
            }
        }
    }

    /// In MIDI mode, the gain a note asks for (`open`) or no note (closed), dB: the curve's
    /// two ends, the opening less `vel_db` scaled by how soft the last note was.
    pub fn midi_gain_db(s: &GateSettings, open: bool, velocity: u8) -> f32 {
        let swing = s.swing_db();
        let soft = (s.vel_db.max(0.0) * (1.0 - velocity.clamp(1, 127) as f32 / 127.0)).min(swing);
        match (s.style.spec().mode, open) {
            // `0.0 - soft`, not `-soft`: a full opening is +0 dB, which the gain stage
            // turns into exactly 1.0.
            (Mode::Down, true) => 0.0 - soft,
            (Mode::Down, false) => -swing,
            (Mode::Up, true) => swing - soft,
            (Mode::Duck, true) => soft - swing,
            (_, false) => 0.0,
        }
    }

    pub fn sample_rate(&self) -> f32 {
        self.sr
    }

    pub fn settings(&self) -> &GateSettings {
        &self.s
    }

    /// New settings. Nothing here allocates: it runs on the audio thread.
    pub fn set(&mut self, s: &GateSettings) {
        let old_filters = (self.s.hpf_hz, self.s.lpf_hz, self.s.key_slope);
        self.s = *s;
        self.spec = s.style.spec();
        let sr = self.sr;
        let spec = self.spec;
        let c = ramp_share(spec.shape);
        let swing = s.swing_db().max(1e-6);
        // The ramp's step per sample (dB), so that a full swing takes `c` of the time.
        let per_sample = |ms: f32| swing / (c * ms.max(1e-3) * 0.001 * sr).max(1e-3);
        self.step_att = per_sample(s.attack_ms);
        self.step_rel = per_sample(s.release_ms);
        let pole = |ms: f32| {
            let tau = spec.shape * ms.max(0.0) * 0.001 * sr;
            if tau < 1e-3 {
                0.0
            } else {
                math::exp(-1.0 / tau)
            }
        };
        self.pole_att = pole(s.attack_ms);
        self.pole_rel = pole(s.release_ms);
        self.hold_n = (s.hold_ms.max(0.0) * 0.001 * sr).round() as u32;
        let det_ms = if spec.rms_ms > 0.0 {
            spec.rms_ms
        } else {
            spec.peak_fall_ms
        };
        self.det_coef = math::exp(-1.0 / (det_ms.max(0.01) * 0.001 * sr));
        let look = s.lookahead_ms.clamp(0.0, LOOKAHEAD_MAX_MS);
        if look > 0.0 {
            self.delay = ((LOOKAHEAD_MAX_MS * 0.001 * sr).round() as usize).min(self.cap - 1);
            self.look_n = ((look * 0.001 * sr).round() as usize).min(self.delay);
            self.key_delay = self.delay - self.look_n;
        } else {
            self.delay = 0;
            self.look_n = 0;
            self.key_delay = 0;
        }
        self.sections = key_filter(s, sr, &mut self.coefs);
        if old_filters != (s.hpf_hz, s.lpf_hz, s.key_slope) {
            // A different cascade: its old state means nothing to the new sections.
            self.filt = [[Sec::default(); 2 * MAX_SECTIONS]; 2];
        }
    }

    /// Back to the start: open (a gate starts open and closes if nothing is there), with
    /// empty delay lines.
    pub fn reset(&mut self) {
        self.filt = [[Sec::default(); 2 * MAX_SECTIONS]; 2];
        self.env = [0.0; 2];
        self.level_db = FLOOR_DB;
        self.phase = Phase::Hold;
        self.hold_left = 0;
        self.target_db = 0.0;
        self.ramp = 0.0;
        self.smooth = 0.0;
        self.gain_db = 0.0;
        self.gain = 1.0;
        for b in self.audio.iter_mut().chain(self.key.iter_mut()) {
            b.fill(0.0);
        }
        self.quiet = [crate::rest::ZeroRun::FULL; 4];
        self.levels.fill(FLOOR_DB);
        self.pos = 0;
        self.n = 0;
        self.col_left = self.col_len;
        self.col = Column::default();
        self.col_in = 0.0;
        self.col_out = 0.0;
        self.pending = None;
        self.armed = true;
        self.fresh = true;
        self.trigger_count = 0;
        self.next_due = u64::MAX;
        self.held = [false; 128];
        self.held_count = 0;
    }

    /// Samples of delay this adds: [`LOOKAHEAD_MAX_MS`] with lookahead on, else 0.
    pub fn latency(&self) -> usize {
        self.delay
    }

    /// The phase now.
    pub fn phase(&self) -> Phase {
        self.phase
    }

    /// The gain now, dB.
    pub fn gain_db(&self) -> f32 {
        self.gain_db
    }

    /// The detector's level now, dB.
    pub fn level_db(&self) -> f32 {
        self.level_db
    }

    /// Hits measured so far (openings whose peak window has passed).
    pub fn hits(&self) -> u32 {
        self.events_done
    }

    /// Whether every piece of state that rings is exactly at rest.
    pub fn is_quiet(&self) -> bool {
        self.env == [0.0; 2]
            && self
                .filt
                .iter()
                .flatten()
                .all(|s| s.z1 == 0.0 && s.z2 == 0.0)
    }

    /// At rest (ADR 0031 §5.1): only in MIDI mode, with no note held or waiting, closed
    /// and still (or not yet started, which the first sample makes the same), every filter
    /// and follower empty and both delay lines all zeros. Played by its key, a gate is never
    /// at rest: a reset gate starts *open* and closes after its hold, while one that has
    /// heard silence is closed — two different states, both right.
    pub fn at_rest(&self) -> bool {
        let s = &self.s;
        if !s.midi || self.trigger_count != 0 || self.held_count != 0 {
            return false;
        }
        let closed = Self::midi_gain_db(s, false, self.velocity);
        let still = self.fresh
            || (self.phase == Phase::Closed
                && self.target_db == closed
                && self.ramp == closed
                && self.smooth == closed);
        still
            && self.env.iter().all(|e| crate::rest::z(*e))
            && self
                .filt
                .iter()
                .flatten()
                .all(|s| crate::rest::z(s.z1) && crate::rest::z(s.z2))
            && self.quiet.iter().all(|q| q.covers(self.cap))
    }

    /// Samples of silence skipped while at rest (ADR 0031 §5.1): what the first of them
    /// does to a gate not yet started (close it, at once), and the clock the notes are
    /// timed on. The display's history does not move.
    pub fn skip(&mut self, frames: usize) {
        if frames == 0 {
            return;
        }
        if self.fresh && self.s.midi {
            self.fresh = false;
            let closed = Self::midi_gain_db(&self.s, false, self.velocity);
            self.phase = Phase::Closed;
            self.target_db = closed;
            self.ramp = closed;
            self.smooth = closed;
        }
        self.n += frames as u64;
        self.pos = (self.pos + frames % self.cap) % self.cap;
    }

    /// The editor's numbers (layout in [`telemetry`]).
    pub fn write_telemetry(&self, t: &mut [f32; 64]) {
        use telemetry::*;
        t.fill(0.0);
        t[SEQ] = (self.cols_done % WRAP) as f32;
        t[COLUMN_MS] = self.col_len as f32 * 1000.0 / self.sr;
        t[LEVEL] = self.level_db.max(-120.0);
        t[GAIN] = if self.gain == 0.0 {
            -120.0
        } else {
            self.gain_db
        };
        t[PHASE] = self.phase as u32 as f32;
        t[LATENCY] = self.delay as f32;
        t[HITS] = (self.events_done % WRAP) as f32;
        t[SAMPLE_RATE] = self.sr;
        for j in 0..EVENTS {
            let (at, peak) = self.events[(self.ev_head + EVENTS - 1 - j) % EVENTS];
            let ago = self.n.saturating_sub(at).min((WRAP - 1) as u64);
            t[EVENTS_AT + 2 * j] = ago as f32;
            t[EVENTS_AT + 2 * j + 1] = peak;
        }
        for j in 0..HISTORY_COLUMNS {
            let c = self.cols[(self.col_head + j) % HISTORY_COLUMNS];
            t[COLUMNS_AT + 2 * j] = c[0];
            t[COLUMNS_AT + 2 * j + 1] = c[1];
        }
    }

    fn push_event(&mut self, at: u64, peak: f32) {
        self.events[self.ev_head] = (at, peak);
        self.ev_head = (self.ev_head + 1) % EVENTS;
        self.events_done = self.events_done.wrapping_add(1) % telemetry::WRAP;
    }

    /// Process a stereo block in place. `key` is the external sidechain, when routed;
    /// otherwise the gate listens to its own input.
    pub fn process(&mut self, left: &mut [f32], right: &mut [f32], key: Option<(&[f32], &[f32])>) {
        let s = self.s;
        let spec = self.spec;
        let n = left.len().min(right.len());
        let cap = self.cap;
        let silence_ok = s.infinite_range() && spec.mode != Mode::Up;
        let opens_up = spec.mode != Mode::Duck;
        let rms = spec.rms_ms > 0.0;
        let hit_n = ((HIT_WINDOW_MS * 0.001 * self.sr) as u32).max(1);
        let settle_n = (0.001 * self.sr).round() as u32;
        for i in 0..n {
            if self.next_due <= self.n {
                self.apply_triggers();
            }
            let (xl, xr) = (left[i], right[i]);
            let (kl, kr) = match key {
                Some((a, b)) => (
                    a.get(i).copied().unwrap_or(0.0),
                    b.get(i).copied().unwrap_or(0.0),
                ),
                None => (xl, xr),
            };
            let pos = self.pos;
            self.audio[0][pos] = xl;
            self.audio[1][pos] = xr;
            self.key[0][pos] = kl;
            self.key[1][pos] = kr;
            let back = |d: usize| (pos + cap - d) % cap;
            let (dl, dr) = if self.delay == 0 {
                (xl, xr)
            } else {
                let j = back(self.delay);
                (self.audio[0][j], self.audio[1][j])
            };
            let (ql, qr) = if self.key_delay == 0 {
                (kl, kr)
            } else {
                let j = back(self.key_delay);
                (self.key[0][j], self.key[1][j])
            };
            // The key through its filters.
            let (mut fl, mut fr) = (ql, qr);
            for k in 0..self.sections {
                fl = self.filt[0][k].tick(&self.coefs[k], fl);
                fr = self.filt[1][k].tick(&self.coefs[k], fr);
            }
            // The follower: linked, the louder side decides.
            let a = self.det_coef;
            let level = if rms {
                for (e, x) in self.env.iter_mut().zip([fl, fr]) {
                    *e = x * x + (*e - x * x) * a;
                    if *e < 1e-30 {
                        *e = 0.0;
                    }
                }
                (2.0 * self.env[0].max(self.env[1])).sqrt()
            } else {
                for (e, x) in self.env.iter_mut().zip([fl, fr]) {
                    *e = x.abs().max(*e * a);
                    if *e < 1e-15 {
                        *e = 0.0;
                    }
                }
                self.env[0].max(self.env[1])
            };
            let l = if level > 1e-10 {
                20.0 * math::log10(level)
            } else {
                FLOOR_DB
            };
            self.level_db = l;

            // A gate starts open, and holds before it may close: for its hold time, for the
            // lookahead's delay line to fill, and a millisecond for the detector to settle.
            if self.fresh {
                self.fresh = false;
                if s.midi {
                    // Played by notes, it starts where no note is: closed, at once.
                    let closed = Self::midi_gain_db(&s, false, self.velocity);
                    self.phase = Phase::Closed;
                    self.target_db = closed;
                    self.ramp = closed;
                    self.smooth = closed;
                } else {
                    self.phase = Phase::Hold;
                    self.hold_left = self.hold_n + self.key_delay as u32 + settle_n;
                }
            }
            let t_open = s.threshold_db;
            let t_close = s.close_db();
            // A hit: the key rising through the threshold after it was below the close
            // threshold — whether or not the gate had closed in between.
            if l < t_close {
                self.armed = true;
            } else if l >= t_open && self.armed {
                self.armed = false;
                if let Some((at, peak, _)) = self.pending.take() {
                    self.push_event(at, peak);
                }
                let at = self.n.saturating_sub(self.key_delay as u64);
                self.pending = Some((at, l, hit_n));
            }
            // The state machine: open at the threshold, close below the close threshold,
            // hold, release. In MIDI mode a held note is "above" and none is "below".
            let (above_open, below_close) = if s.midi {
                (self.held_count > 0, self.held_count == 0)
            } else {
                (l >= t_open, l < t_close)
            };
            match self.phase {
                Phase::Closed | Phase::Release => {
                    if above_open {
                        self.phase = Phase::Open;
                    }
                }
                Phase::Open => {
                    if below_close {
                        if self.hold_n > 0 {
                            self.phase = Phase::Hold;
                            self.hold_left = self.hold_n;
                        } else {
                            self.phase = Phase::Release;
                        }
                    }
                }
                Phase::Hold => {
                    if !below_close {
                        self.phase = Phase::Open;
                    } else {
                        self.hold_left = self.hold_left.saturating_sub(1);
                        if self.hold_left == 0 {
                            self.phase = Phase::Release;
                        }
                    }
                }
            }
            if let Some((at, peak, left)) = self.pending {
                let peak = peak.max(l);
                if left <= 1 {
                    self.pending = None;
                    self.push_event(at, peak);
                } else {
                    self.pending = Some((at, peak, left - 1));
                }
            }
            // The target: the curve, except in hold, where the gate stays where it was.
            if self.phase != Phase::Hold {
                let open = matches!(self.phase, Phase::Open);
                self.target_db = if s.midi {
                    Self::midi_gain_db(&s, open, self.velocity)
                } else {
                    static_gain_db(l, &s, open)
                };
            }
            let target = self.target_db;

            // The motion: a ramp at the set speed, then the rounding, in dB.
            let goal = target;
            let rising = goal > self.ramp;
            let opening = rising == opens_up;
            let mut step = if opening {
                self.step_att
            } else {
                self.step_rel
            };
            if !opening && spec.follow && !s.midi {
                // Gently near the threshold, at the set speed 12 dB under it.
                step *= ((t_close - l) / 12.0).clamp(0.1, 1.0);
            }
            self.ramp = if rising {
                (self.ramp + step).min(goal)
            } else {
                (self.ramp - step).max(goal)
            };
            let pole = if (self.ramp > self.smooth) == opens_up {
                self.pole_att
            } else {
                self.pole_rel
            };
            self.smooth = self.ramp + (self.smooth - self.ramp) * pole;
            if (self.smooth - self.ramp).abs() < 1e-4 {
                self.smooth = self.ramp;
            }
            if self.phase == Phase::Release && self.ramp == goal && (self.smooth - goal).abs() < 0.1
            {
                self.phase = Phase::Closed;
            }
            self.gain = if self.smooth == 0.0 {
                1.0
            } else if silence_ok && self.smooth <= SILENCE_DB {
                0.0
            } else {
                math::db_to_gain(self.smooth)
            };
            self.gain_db = self.smooth;
            let g = self.gain;
            let (yl, yr) = (dl * g, dr * g);

            // The display's column, on the input's timeline.
            self.levels[pos] = l;
            let key_db = if self.look_n == 0 {
                l
            } else {
                self.levels[back(self.look_n)]
            };
            self.col_in = self.col_in.max(dl.abs()).max(dr.abs());
            self.col_out = self.col_out.max(yl.abs()).max(yr.abs());
            let c = &mut self.col;
            c.key_db = c.key_db.max(key_db);
            let shown = if g == 0.0 { -120.0 } else { self.gain_db };
            c.gain_db = if spec.mode == Mode::Up {
                c.gain_db.max(shown)
            } else {
                c.gain_db.min(shown)
            };
            c.phase = c.phase.max(self.phase);
            self.col_left -= 1;
            if self.col_left == 0 {
                self.push_column();
            }
            self.pos = (pos + 1) % cap;
            self.n += 1;

            let (ol, or) = match s.listen {
                Listen::Gate => (yl, yr),
                Listen::Key => (fl, fr),
                Listen::Delta if spec.mode == Mode::Up => (yl - dl, yr - dr),
                Listen::Delta => (dl - yl, dr - yr),
            };
            left[i] = ol;
            right[i] = or;
        }
        let pos = self.pos;
        for (q, line) in self
            .quiet
            .iter_mut()
            .zip(self.audio.iter().chain(self.key.iter()))
        {
            q.wrote(line, pos, n);
        }
    }

    /// Close the column: its peaks become dB, and it joins the history.
    fn push_column(&mut self) {
        let c = Column {
            input_db: lin_db(self.col_in),
            output_db: lin_db(self.col_out),
            ..self.col
        };
        self.cols[self.col_head] = telemetry::pack(&c);
        self.col_head = (self.col_head + 1) % HISTORY_COLUMNS;
        self.cols_done = (self.cols_done + 1) % telemetry::WRAP;
        self.col = Column::default();
        self.col_in = 0.0;
        self.col_out = 0.0;
        self.col_left = self.col_len;
    }
}

fn lin_db(x: f32) -> f32 {
    if x > 1e-6 {
        20.0 * math::log10(x)
    } else {
        -120.0
    }
}

/// What a passage taught the gate: where to open and where to close.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Learned {
    pub open_db: f32,
    pub close_db: f32,
    /// True when the passage had two populations (hits over bleed) and the thresholds sit
    /// between them; false when it was one (noise alone), and the gate closes above it.
    pub two_levels: bool,
}

/// Learn the thresholds from the key levels of a passage (one per display column, dB).
///
/// The levels are split in two by Otsu's method on a 1 dB histogram. If the two groups are
/// real — each at least 5 % of the passage, their means 12 dB apart or more — the passage
/// is hits over bleed: the gate closes 2 dB above the top of the bleed (the densest cluster
/// below the split, up to where its density falls under a quarter) and opens a third of
/// the way from there to the hits' mean (at least 4 dB higher). Otherwise the
/// passage is taken as noise alone: the gate closes 3 dB above its loudest 1 % and opens
/// `hysteresis_db` above that. `None` for a passage too short or silent to say.
pub fn learn_thresholds(levels_db: &[f32], hysteresis_db: f32) -> Option<Learned> {
    const LO: f32 = -100.0;
    const BINS: usize = 110;
    let mut v: Vec<f32> = levels_db
        .iter()
        .copied()
        .filter(|x| x.is_finite() && *x > LO)
        .collect();
    if v.len() < 20 {
        return None;
    }
    v.sort_by(f32::total_cmp);
    let pct = |v: &[f32], p: f32| v[((v.len() - 1) as f32 * p).round() as usize];
    let mut hist = [0usize; BINS];
    for x in &v {
        hist[((x - LO) as usize).min(BINS - 1)] += 1;
    }
    let total = v.len() as f64;
    let centre = |b: usize| (LO + b as f32 + 0.5) as f64;
    let sum_all: f64 = (0..BINS).map(|b| hist[b] as f64 * centre(b)).sum();
    let (mut w0, mut s0, mut best, mut split) = (0.0f64, 0.0f64, -1.0f64, 0usize);
    for (b, h) in hist.iter().enumerate().take(BINS - 1) {
        w0 += *h as f64;
        s0 += *h as f64 * centre(b);
        let w1 = total - w0;
        if w0 == 0.0 || w1 == 0.0 {
            continue;
        }
        let (m0, m1) = (s0 / w0, (sum_all - s0) / w1);
        let between = w0 * w1 * math::powi(m0 - m1, 2);
        if between > best {
            best = between;
            split = b + 1;
        }
    }
    let edge = LO + split as f32;
    let (low, high): (Vec<f32>, Vec<f32>) = v.iter().partition(|x| **x < edge);
    let share = |x: &Vec<f32>| x.len() as f64 / total;
    let mean = |x: &Vec<f32>| x.iter().map(|y| *y as f64).sum::<f64>() / x.len().max(1) as f64;
    let two = !low.is_empty()
        && !high.is_empty()
        && share(&low) >= 0.05
        && share(&high) >= 0.05
        && mean(&high) - mean(&low) >= 12.0;
    let (open, close) = if two {
        // The bleed is the densest cluster below the split; its top is where the density
        // falls under a quarter of its peak (a hit's decay passes through those levels
        // too, but thinly: a decay spends the same time in every dB).
        let smooth = |b: usize| {
            let lo = b.saturating_sub(1);
            let hi = (b + 1).min(BINS - 1);
            (lo..=hi).map(|j| hist[j] as f64).sum::<f64>() / (hi - lo + 1) as f64
        };
        let mode = (0..split)
            .max_by(|a, b| smooth(*a).total_cmp(&smooth(*b)))
            .unwrap_or(0);
        let peak = smooth(mode);
        let top = (mode..split)
            .find(|b| smooth(*b) < peak * 0.25)
            .unwrap_or(split);
        let close = LO + top as f32 + 2.0;
        let hits = mean(&high) as f32;
        let open = close + ((hits - close) / 3.0).max(4.0);
        (open, close)
    } else {
        let close = pct(&v, 0.99) + 3.0;
        (close + hysteresis_db.max(2.0), close)
    };
    let open = open.clamp(-80.0, 0.0);
    let close = close.clamp(-80.0, open);
    Some(Learned {
        open_db: open,
        close_db: close,
        two_levels: two,
    })
}

#[cfg(test)]
#[path = "porta_tests.rs"]
mod tests;

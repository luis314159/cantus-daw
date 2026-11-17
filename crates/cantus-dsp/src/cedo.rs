//! **Cedo**, the ducker (fx side round, wave B; DSP phase — no `EffectKind` yet). Latin
//! *cedo*, "I make way": a unit that makes room for something else.
//!
//! ## Two kinds of trigger
//!
//! - **Sidechain** ([`Trigger::Sidechain`]): the audio of another strip — a kick, a voice —
//!   through a high-pass or band-pass ([`env::KeyFilter`], Comprimo's key sections shared),
//!   against a **threshold**, firing an envelope that **attacks**, **holds** and
//!   **releases** in exact sample counts, the release linear, exponential or an S-curve
//!   ([`env`]). No key routed, or a key under the threshold, ducks nothing.
//! - **Tempo** and **MIDI** ([`Trigger::Tempo`], [`Trigger::Midi`]): a **drawn shape**
//!   ([`shape::Pump`]: breakpoints with curvature, Lapsus's curve), fired on every beat
//!   division of the song (with swing) or by the notes of another track — the classic
//!   sidechain pump with no audio needed. Its edges are smoothed by a centred kernel
//!   ([`CedoSettings::edge_ms`]).
//!
//! Both give an **amount** `a` (0 … 1) per channel; the band gain is
//! `G = 1 − a·mix·(1 − 10^(−depth/20))` — linear in the amount, so the shape drawn is the
//! level heard. A depth of
//! [`DEPTH_INF`] or more ducks to silence.
//!
//! ## Where it acts
//!
//! The whole signal, only the lows or only the highs ([`band::Band`]), split by a
//! minimum-phase or a linear-phase crossover ([`band::Crossover`]); both hand the delayed
//! input on to the bit while nothing is ducked.
//!
//! ## Latency
//!
//! **Lookahead** (0 … [`MAX_LOOKAHEAD_MS`]) delays the audio and not the detector, so the
//! duck is in place when the transient it answers arrives; the linear-phase crossover adds
//! its kernel's delay at the floor in force ([`band::linear_latency`], [`band::Floor`]: Auto
//! an octave under the crossover as last committed, or a fixed 40, 60, 80 or 120 Hz). [`Cedo::latency`] is their sum, and
//! everything — the sidechain's amount, the shape's song position, a note's place — is
//! aligned to the audio as it leaves.
//!
//! ## The contract
//!
//! - **Null:** a depth of 0, a mix of 0, or no trigger (no key, a silent key, the transport
//!   stopped for Tempo, no note for MIDI) hands every sample on untouched — to the bit,
//!   delayed by exactly [`Cedo::latency`].
//! - Everything runs per sample on counts from the stream's start, so nothing depends on
//!   the block size; the shape is a function of the song sample.
//! - **Rest** (ADR 0031 §5.1): every line, filter, envelope and note played out, the glides
//!   landed; [`Cedo::skip`] moves the clock.
//! - Allocates only in [`Cedo::new`].

#[path = "cedo_band.rs"]
pub mod band;
#[path = "cedo_env.rs"]
pub mod env;
#[path = "cedo_shape.rs"]
pub mod shape;
#[path = "cedo_testkit.rs"]
pub mod testkit;

use crate::comprimo::Delay;
use crate::dynfx_follow::{Detect, EnvelopeFollower};
use crate::fluctus::{CurvePoint, ShapeFn};
use crate::modfx_clock::{Clock, Transport, DIV_QUARTER};
use crate::modfx_line::Glide;
use band::{Band, Crossover, Split};
use env::{Detector, Envelope, KeyFilter, KeyFilterMode, ReleaseShape, Timing};
use shape::{Pump, Source};

/// The unit's name (provisional until the contract).
pub const NAME: &str = "Cedo";
/// The longest lookahead, ms.
pub const MAX_LOOKAHEAD_MS: f32 = 10.0;
/// A depth at or above this ducks to silence.
pub const DEPTH_INF: f32 = 80.0;
/// Notes waiting for their sample.
const MAX_NOTES: usize = 64;
/// How fast depth, mix and output follow a change, ms.
const GLIDE_MS: f32 = 10.0;

/// What fires the duck.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum Trigger {
    /// The audio of another strip.
    #[default]
    Sidechain,
    /// The drawn shape on every beat division of the song.
    Tempo,
    /// The drawn shape from the notes of another track.
    Midi,
}

impl Trigger {
    pub fn from_index(i: usize) -> Trigger {
        match i {
            1 => Trigger::Tempo,
            2 => Trigger::Midi,
            _ => Trigger::Sidechain,
        }
    }
}

/// What is heard.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum Listen {
    #[default]
    Output,
    /// What the duck takes away: the delayed input minus the output.
    Delta,
    /// The key as the detector hears it (after its filter), aligned to the output.
    Key,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct CedoSettings {
    pub trigger: Trigger,
    /// How far the duck goes, dB (0 … [`DEPTH_INF`], which is silence). 0 is the null.
    pub depth_db: f32,
    /// The key's level that fires the duck, dBFS (sample peak, after the key filter).
    pub threshold_db: f32,
    pub attack_ms: f32,
    /// How long the duck stays after the key's last sample over the threshold.
    pub hold_ms: f32,
    pub release_ms: f32,
    pub release_shape: ReleaseShape,
    pub key_filter: KeyFilterMode,
    /// The key's level as a sample peak, or as its RMS over `rms_ms`.
    pub detector: Detector,
    /// The RMS window, ms (`env::MIN_RMS_MS` … `env::MAX_RMS_MS`).
    pub rms_ms: f32,
    pub key_hz: f32,
    /// The band-pass's Q.
    pub key_q: f32,
    /// 0 … [`MAX_LOOKAHEAD_MS`]: the audio waits this long for the detector.
    pub lookahead_ms: f32,
    /// The shape's cycle: an index into the family's divisions (`modfx_clock::DIVISIONS`).
    pub division: usize,
    /// 0 straight … 1 every second cycle as late as MPC's 75 %.
    pub swing: f32,
    /// The shape's edges smoothed over this long, ms (0 … `shape::MAX_EDGE_MS`).
    pub edge_ms: f32,
    pub band: Band,
    pub crossover: Crossover,
    pub cross_hz: f32,
    /// The linear phase's floor (its latency): Auto, or 40, 60, 80, 120 Hz.
    pub floor: band::Floor,
    /// Auto's floor as last committed, Hz (an octave under the crossover when it was let
    /// go: `band::auto_floor`).
    pub auto_floor_hz: f32,
    /// 0: each channel ducks by its own key channel; 1: both by the louder.
    pub link: f32,
    pub mix: f32,
    pub out_db: f32,
    pub listen: Listen,
    /// MIDI: a note's velocity scales the depth in decibels (127 the whole depth).
    pub velocity: bool,
}

impl Default for CedoSettings {
    fn default() -> Self {
        CedoSettings {
            trigger: Trigger::Sidechain,
            depth_db: 0.0,
            threshold_db: -24.0,
            attack_ms: 1.0,
            hold_ms: 40.0,
            release_ms: 150.0,
            release_shape: ReleaseShape::Exponential,
            key_filter: KeyFilterMode::Off,
            detector: Detector::Peak,
            rms_ms: 10.0,
            key_hz: 100.0,
            key_q: 1.0,
            lookahead_ms: 0.0,
            division: DIV_QUARTER,
            swing: 0.0,
            edge_ms: 1.0,
            band: Band::Whole,
            crossover: Crossover::MinimumPhase,
            cross_hz: 120.0,
            floor: band::Floor::Auto,
            auto_floor_hz: band::AUTO_DEFAULT_HZ,
            link: 1.0,
            mix: 1.0,
            out_db: 0.0,
            listen: Listen::Output,
            velocity: false,
        }
    }
}

/// Lookahead in samples.
fn look_samples(ms: f32, sr: f32) -> usize {
    let ms = if ms.is_finite() {
        ms.clamp(0.0, MAX_LOOKAHEAD_MS)
    } else {
        0.0
    };
    (ms as f64 * 0.001 * sr as f64).round() as usize
}

/// The latency `s` gives at a rate, without a unit.
pub fn latency_for(s: &CedoSettings, sample_rate: f32) -> usize {
    let lin = s.band != Band::Whole && s.crossover == Crossover::LinearPhase;
    look_samples(s.lookahead_ms, sample_rate)
        + if lin {
            band::linear_latency(s.floor.hz(s.auto_floor_hz), sample_rate)
        } else {
            0
        }
}

/// The classic pump: the full depth on the beat, letting go a little faster at first and
/// easing back to nothing by 65 % of the cycle (`1 − (1 − u)^2.4`), then nothing until the
/// next beat. (Segment shapes are the automation editor's: 0 a curve, 2 a hold.)
pub const CLASSIC_PUMP: [CurvePoint; 2] = [
    CurvePoint {
        x: 0.0,
        y: 1.0,
        tension: -0.2,
        shape: 0,
    },
    CurvePoint {
        x: 0.65,
        y: 0.0,
        tension: 0.0,
        shape: 2,
    },
];

/// What the last block did, for the display.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct CedoMeter {
    /// The most gain taken away in the block, dB (positive).
    pub gr_db: f32,
    /// Each channel's reduction at the block's end, dB.
    pub gr_now: [f32; 2],
    /// The amount (0 … 1) at the block's end, the louder channel.
    pub amount: f32,
    /// The filtered key's peak in the block, dBFS (−200: silence or no key).
    pub key_db: f32,
    /// The key reached the threshold in the block.
    pub triggered: bool,
    /// Where the shape's cycle is at the block's end (0 … 1), −1 when none runs.
    pub phase: f32,
    pub in_db: f32,
    pub out_db: f32,
}

impl Default for CedoMeter {
    fn default() -> Self {
        CedoMeter {
            gr_db: 0.0,
            gr_now: [0.0; 2],
            amount: 0.0,
            key_db: -200.0,
            triggered: false,
            phase: -1.0,
            in_db: -200.0,
            out_db: -200.0,
        }
    }
}

#[derive(Clone)]
pub struct Cedo {
    sr: f32,
    s: CedoSettings,
    clock: Clock,
    pump: Pump,
    key_f: KeyFilter,
    timing: Timing,
    env: [Envelope; 2],
    /// The RMS detector's followers, per channel.
    rms: [EnvelopeFollower; 2],
    thr: f32,
    /// The sidechain's amounts, delayed by the split's latency.
    amt_line: Delay,
    dry: Delay,
    key_line: Delay,
    split: Split,
    look: usize,
    lat: usize,
    /// `mix·(1 − floor)`: the amount's weight on the gain.
    wgt: Glide,
    out: Glide,
    /// Notes waiting for their sample: (stream sample, velocity).
    notes: [(i64, u8); MAX_NOTES],
    note_n: usize,
    meter: CedoMeter,
}

impl std::fmt::Debug for Cedo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Cedo")
            .field("sr", &self.sr)
            .field("settings", &self.s)
            .field("latency", &self.lat)
            .finish_non_exhaustive()
    }
}

impl Cedo {
    /// A unit whose shape segments bend as `shape` says (the automation editor's
    /// `Shape::at`; [`crate::fluctus::linear`] for straight lines). The only place it
    /// allocates. Its shape is flat (no duck) until [`Cedo::set_shape`].
    pub fn new(sample_rate: f32, shape: ShapeFn) -> Cedo {
        let sr = sample_rate.max(1.0);
        let look_max = look_samples(MAX_LOOKAHEAD_MS, sr);
        // The lines are sized for the lowest floor's kernel.
        let lin = band::linear_latency(band::FLOOR_MIN_HZ, sr);
        let s = CedoSettings::default();
        let mut c = Cedo {
            sr,
            s,
            clock: Clock::new(),
            pump: Pump::new(sr, shape),
            key_f: KeyFilter::default(),
            timing: Timing::default(),
            env: [Envelope::default(); 2],
            rms: [EnvelopeFollower::new(sr), EnvelopeFollower::new(sr)],
            thr: 1.0,
            amt_line: Delay::new(lin + 2),
            dry: Delay::new(look_max + Split::history_needed(sr) + 8),
            key_line: Delay::new(look_max + lin + 2),
            split: Split::new(sr),
            look: 0,
            lat: 0,
            wgt: Glide::new(0.0, GLIDE_MS, sr, 1e-7),
            out: Glide::new(1.0, GLIDE_MS, sr, 1e-7),
            notes: [(0, 0); MAX_NOTES],
            note_n: 0,
            meter: CedoMeter::default(),
        };
        c.set(&s);
        c.reset();
        c
    }

    pub fn sample_rate(&self) -> f32 {
        self.sr
    }

    pub fn settings(&self) -> &CedoSettings {
        &self.s
    }

    /// The drawn shape (`y` the amount, 0 … 1; at most `lapsus::MAX_POINTS` points).
    pub fn set_shape(&mut self, points: &[CurvePoint]) {
        self.pump.set_shape(points);
    }

    pub fn shape_points(&self) -> &[CurvePoint] {
        self.pump.points()
    }

    /// New settings, between blocks. Never allocates; depth, mix and output glide.
    pub fn set(&mut self, s: &CedoSettings) {
        let sr = self.sr;
        self.s = *s;
        self.timing = Timing::new(s.attack_ms, s.hold_ms, s.release_ms, s.release_shape, sr);
        self.thr = crate::math::db_to_gain(s.threshold_db.clamp(-90.0, 6.0));
        let depth = if s.depth_db.is_finite() {
            s.depth_db.max(0.0)
        } else {
            0.0
        };
        let floor = if depth >= DEPTH_INF {
            0.0
        } else {
            crate::math::db_to_gain(-depth)
        };
        let mix = if s.mix.is_finite() {
            s.mix.clamp(0.0, 1.0)
        } else {
            0.0
        };
        self.wgt.target = if depth == 0.0 {
            0.0
        } else {
            mix * (1.0 - floor)
        };
        self.out.target = if s.out_db == 0.0 {
            1.0
        } else {
            crate::math::db_to_gain(s.out_db.clamp(-60.0, 24.0))
        };
        self.key_f.set(s.key_filter, s.key_hz, s.key_q, sr);
        let win = if s.rms_ms.is_finite() {
            s.rms_ms.clamp(env::MIN_RMS_MS, env::MAX_RMS_MS)
        } else {
            10.0
        };
        for f in &mut self.rms {
            f.set(Detect::Rms, win, win);
        }
        if s.detector != Detector::Rms || s.trigger != Trigger::Sidechain {
            self.rms.iter_mut().for_each(EnvelopeFollower::reset);
        }
        self.look = look_samples(s.lookahead_ms, sr);
        let (dry, look) = (&self.dry, self.look);
        self.split.set(
            s.band,
            s.crossover,
            s.cross_hz,
            s.floor.hz(s.auto_floor_hz),
            &|d| dry.tap(look + d),
        );
        self.lat = self.look + self.split.latency();
        let src = if s.trigger == Trigger::Midi {
            Source::Midi
        } else {
            Source::Tempo
        };
        self.pump.set_grid(s.division, s.swing, src);
        self.pump.set_edge(s.edge_ms);
        if s.trigger != Trigger::Sidechain {
            // The detector is not heard: nothing of it waits to be.
            self.env = [Envelope::default(); 2];
            self.key_f.reset();
            self.amt_line.clear();
            self.key_line.clear();
        }
        if s.trigger != Trigger::Midi {
            self.note_n = 0;
        }
    }

    pub fn set_transport(&mut self, t: &Transport) {
        self.clock.set(t);
    }

    /// A note-on (velocity 1 … 127) from the trigger track `offset` samples into the next
    /// [`Cedo::process`] call (MIDI trigger only); acted on at its sample.
    pub fn note(&mut self, offset: usize, velocity: u8) {
        if self.s.trigger != Trigger::Midi {
            return;
        }
        if self.note_n == MAX_NOTES {
            let (at, v) = self.notes[0];
            self.notes.copy_within(1.., 0);
            self.note_n -= 1;
            let k = self.velocity_scale(v);
            self.pump.fire(&self.clock, at, k);
        }
        let at = self.clock.stream(offset);
        // Kept in time order.
        let mut i = self.note_n;
        while i > 0 && self.notes[i - 1].0 > at {
            i -= 1;
        }
        self.notes.copy_within(i..self.note_n, i + 1);
        self.notes[i] = (at, velocity);
        self.note_n += 1;
    }

    /// What a note's velocity makes of the shape's amount: with `velocity` on, the amount
    /// `k` for which the full shape ducks by `depth·v/127` dB — `(1 − 10^(−depth·v/2540)) /
    /// (1 − 10^(−depth/20))` — so velocity scales the depth in decibels; 1 otherwise.
    pub(crate) fn velocity_scale(&self, velocity: u8) -> f32 {
        let depth = self.s.depth_db;
        if !self.s.velocity || !depth.is_finite() || depth <= 0.0 {
            return 1.0;
        }
        let v = velocity.clamp(1, 127) as f32 / 127.0;
        if v >= 1.0 {
            return 1.0;
        }
        let full = if depth >= DEPTH_INF {
            1.0
        } else {
            1.0 - crate::math::db_to_gain(-depth)
        };
        let part = 1.0 - crate::math::db_to_gain(-depth.min(DEPTH_INF) * v);
        (part / full).clamp(0.0, 1.0)
    }

    /// Samples of delay: the lookahead and the linear-phase kernel's.
    pub fn latency(&self) -> usize {
        self.lat
    }

    pub fn meter(&self) -> CedoMeter {
        self.meter
    }

    /// The magnitude, dB, the band split gives at `hz` with the duck at amount `a`
    /// (0 … 1): what the display draws under the shape.
    pub fn response_db(&self, hz: f32, a: f32) -> f32 {
        let g = 1.0 - a.clamp(0.0, 1.0) * self.wgt.target;
        self.split.response_db(hz, g)
    }
}

#[path = "cedo_run.rs"]
mod run;

#[cfg(test)]
#[path = "cedo_tests.rs"]
mod tests;
#[cfg(test)]
#[path = "cedo_tests_a2.rs"]
mod tests_a2;
#[cfg(test)]
#[path = "cedo_tests_floor.rs"]
mod tests_floor;
#[cfg(test)]
#[path = "cedo_tests_measure.rs"]
mod tests_measure;
#[cfg(test)]
#[path = "cedo_tests_shared.rs"]
mod tests_shared; // fxb:cedo

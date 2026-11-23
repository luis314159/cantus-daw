//! Arcus — the bowed string (round 42, ADR 0157): its DSP. A string in physical units —
//! its tension, mass and length, a bow pressing with a force in newtons and moving at a
//! speed in metres a second at a distance from the bridge — met by friction (the hyperbolic
//! law, or a contact temperature's), through Fides's body ([`crate::fides::body`]) and its
//! output stage. The cello and the double bass first; the violin and the viola in the same
//! engine ([`tables`]).
//!
//! **Playing.** A note is bowed (arco: an attack whose force and acceleration come from the
//! velocity, inside the region measured on this model), re-bowed in the other direction on a
//! string still ringing (détaché), slurred (legato: the finger moves at once, the bow goes on
//! — no latency), glided (portamento, only when asked), tremolo, or plucked (pizzicato:
//! Fides's pluck, [`crate::fides::exciter`], on the same string). The dynamic (bow speed,
//! force and contact point together) follows the velocity alone, or CC1 / CC11 / breath /
//! aftertouch / MPE pressure continuously; the vibrato is delayed and moves the string's
//! length, so the partials' amplitudes move through the body's modes.
//!
//! **Pitch.** A bowed string's pitch is not its free loop's (the stick and slip moves it):
//! a table measured offline per pitch × dynamic × contact point ([`pitch`]) corrects the
//! loop's length while the bow is drawn; a plucked or free string is exact.
//!
//! **Determinism and rest** are Fides's: every voice's control step on a grid of [`CTRL`]
//! samples from the stream's start (and once at the note's start), every random number a hash
//! of the seed, the key and the note's id; voices on the pool's lanes summed in voice order
//! (ADR 0038); a voice ends when its bow is off and its string is still, and [`Arcus::at_rest`]
//! is exact.

#[path = "arcus_pitch.rs"]
pub mod pitch;
#[path = "arcus_play.rs"]
mod play;
#[path = "arcus_render.rs"]
mod render;
#[path = "arcus_string.rs"]
pub mod string;
#[path = "arcus_tables.rs"]
pub mod tables;
#[cfg(test)]
#[path = "arcus_tests.rs"]
mod tests;
#[cfg(test)]
#[path = "arcus_tests_gate.rs"]
mod tests_gate;
#[cfg(test)]
#[path = "arcus_tests_gate2.rs"]
mod tests_gate2;
#[cfg(test)]
#[path = "arcus_tests_pitch.rs"]
mod tests_pitch;
#[path = "arcus_voice.rs"]
pub mod voice;

use crate::fides::body::{Body, BodySettings, Output};
use crate::fides::FidesSettings;
use crate::lamina::voice::HammerShape;
use crate::par::Lanes;
use crate::voices::{Action, Allocator, PlayMode};
use string::Friction;
use tables::Instrument;
use voice::Voice;

/// The control grid, in samples from the start of the stream (Fides's).
pub const CTRL: usize = crate::fides::CTRL;
/// The most voices one instance plays (double and triple stops, a divisi of a few).
pub const MAX_VOICES: usize = 8;
/// Voices fading out after they were taken by a new note.
pub const GHOSTS: usize = 2;
/// The render's chunk.
pub const CHUNK: usize = 256;

/// How a note is played: the instrument's technique, or a note's own articulation.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum Technique {
    #[default]
    Arco,
    Pizzicato,
    Tremolo,
}

impl Technique {
    pub const ALL: [Technique; 3] = [Technique::Arco, Technique::Pizzicato, Technique::Tremolo];

    pub fn from_index(i: f32) -> Technique {
        let k = (i.round().max(0.0) as usize).min(Self::ALL.len() - 1);
        Self::ALL[k]
    }
}

/// A note's own articulation (ADR 0158, handed in before its note-on).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Art {
    Sustain,
    Legato,
    Staccato,
    Spiccato,
    Pizzicato,
    Tremolo,
}

/// Which way the bow is drawn: down (from the frog) is `+`, up is `−`.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum BowDir {
    #[default]
    Down,
    Up,
}

/// How the bow's direction is chosen, in `bow.dir`'s order.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum DirMode {
    /// Alternating at every re-bow (a down-bow after a rest).
    #[default]
    Alternate,
    Down,
    Up,
}

impl DirMode {
    pub fn from_index(i: f32) -> DirMode {
        match i.round() as i32 {
            1 => DirMode::Down,
            2 => DirMode::Up,
            _ => DirMode::Alternate,
        }
    }
}

/// Which controller moves the dynamic, in `dyn.src`'s order.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum DynSource {
    /// Whichever of CC1, breath (CC2) and aftertouch moved last; CC11 scales it.
    #[default]
    Auto,
    ModWheel,
    Expression,
    Breath,
    Aftertouch,
    /// The velocity alone.
    Velocity,
}

impl DynSource {
    pub fn from_index(i: f32) -> DynSource {
        match i.round() as i32 {
            1 => DynSource::ModWheel,
            2 => DynSource::Expression,
            3 => DynSource::Breath,
            4 => DynSource::Aftertouch,
            5 => DynSource::Velocity,
            _ => DynSource::Auto,
        }
    }
}

/// Which controller moves the vibrato's depth, in `vib.src`'s order.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum VibSource {
    /// CC21 (delayed and automatic until it moves).
    #[default]
    Cc21,
    /// CC2 (breath), which then leaves the dynamic.
    Cc2,
    Aftertouch,
    /// Automatic only.
    Auto,
}

impl VibSource {
    pub fn from_index(i: f32) -> VibSource {
        match i.round() as i32 {
            1 => VibSource::Cc2,
            2 => VibSource::Aftertouch,
            3 => VibSource::Auto,
            _ => VibSource::Cc21,
        }
    }
}

/// Everything the unit sets (`cantus_engine::inst::arcus::settings`).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct ArcusSettings {
    pub instrument: Instrument,
    pub technique: Technique,
    pub poly: usize,
    pub mode: PlayMode,
    /// The finger's move on a slur, ms (no latency: it starts at the note-on).
    pub finger_ms: f32,
    /// Portamento on a slur, ms (0: none — only when asked).
    pub porta_ms: f32,
    pub bend_st: f32,
    pub transpose_st: f32,
    pub fine_ct: f32,
    /// The dynamic with no controller and no velocity's share (0 pp … 1 ff).
    pub dynamic: f32,
    /// How much the velocity sets the dynamic (0 none … 1 all).
    pub vel_dyn: f32,
    /// How much the velocity sharpens the attack (force and acceleration).
    pub vel_attack: f32,
    pub dyn_src: DynSource,
    /// The contact point: −1 towards the fingerboard … +1 towards the bridge.
    pub bow_pos: f32,
    /// The force's place in its region, −1 … +1 around the dynamic's.
    pub bow_force: f32,
    /// The bow's speed, × (0.5 … 2).
    pub bow_speed: f32,
    /// The attack's time scale, × (0.25 … 4).
    pub bow_attack: f32,
    /// The bow lifting after the key, ms.
    pub bow_release_ms: f32,
    /// The bow-hair noise layer, dB around the measured level (−60 off).
    pub bow_noise_db: f32,
    pub friction: Friction,
    pub dir: DirMode,
    /// The bow change's time, ms.
    pub change_ms: f32,
    /// Tremolo strokes a second.
    pub trem_rate: f32,
    /// Vibrato: depth × the instrument's, rate × its, delay and onset (ms), wander (0…1).
    pub vib_depth: f32,
    pub vib_rate: f32,
    pub vib_delay_ms: f32,
    pub vib_attack_ms: f32,
    pub vib_var: f32,
    pub vib_src: VibSource,
    /// Pizzicato: where it is plucked (from the bridge), the finger's softness (ms), how
    /// long it rings (× the string's).
    pub pizz_pos: f32,
    pub pizz_soft_ms: f32,
    pub pizz_decay: f32,
    /// The string's decay × its table's.
    pub decay: f32,
    /// How long a stopped string rings once the bow has lifted and the key is up, s.
    pub ring_s: f32,
    /// MPE: pressure on the dynamic (0 … 1 its share), slide on the contact point.
    pub mpe_press: f32,
    pub mpe_slide: f32,
    pub body_mix: f32,
    pub body_size: f32,
    pub body_ring: f32,
    pub lowcut_hz: f32,
    pub highcut_hz: f32,
    pub gain_db: f32,
    pub seed: u32,
}

impl Default for ArcusSettings {
    fn default() -> Self {
        ArcusSettings {
            instrument: Instrument::Cello,
            technique: Technique::Arco,
            poly: 4,
            mode: PlayMode::Legato,
            finger_ms: 3.0,
            porta_ms: 0.0,
            bend_st: 2.0,
            transpose_st: 0.0,
            fine_ct: 0.0,
            dynamic: 0.6,
            vel_dyn: 1.0,
            vel_attack: 0.5,
            dyn_src: DynSource::Auto,
            bow_pos: 0.0,
            bow_force: 0.0,
            bow_speed: 1.0,
            bow_attack: 1.0,
            bow_release_ms: 120.0,
            bow_noise_db: 0.0,
            friction: Friction::Hyperbolic,
            dir: DirMode::Alternate,
            change_ms: 25.0,
            trem_rate: 14.0,
            vib_depth: 1.0,
            vib_rate: 1.0,
            vib_delay_ms: 250.0,
            vib_attack_ms: 450.0,
            vib_var: 0.25,
            vib_src: VibSource::Cc21,
            pizz_pos: 0.22,
            pizz_soft_ms: 1.2,
            pizz_decay: 1.0,
            decay: 1.0,
            ring_s: 0.8,
            mpe_press: 1.0,
            mpe_slide: 0.5,
            body_mix: 1.0,
            body_size: 1.0,
            body_ring: 1.0,
            lowcut_hz: 10.0,
            highcut_hz: 20_000.0,
            gain_db: 0.0,
            seed: 0,
        }
    }
}

impl ArcusSettings {
    /// The body's settings: the instrument's body at its size × the knob's.
    pub fn body(&self) -> BodySettings {
        let spec = self.instrument.spec();
        BodySettings {
            kind: spec.body,
            mix: self.body_mix,
            size: spec.body_size * self.body_size,
            decay: self.body_ring,
        }
    }

    /// Fides's output stage's settings (the cuts and the gain; no tremolo).
    fn output(&self) -> FidesSettings {
        FidesSettings {
            lowcut_hz: self.lowcut_hz,
            highcut_hz: self.highcut_hz,
            gain_db: self.gain_db + self.instrument.spec().trim_db,
            trem_depth: 0.0,
            ..FidesSettings::default()
        }
    }
}

/// The instance-wide controllers the voices read.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Controls {
    /// The dynamic's controller, once it has moved (0 … 1).
    pub dynamic: Option<f32>,
    /// CC11 in Auto (a scale on the dynamic, 1 until it moves).
    pub expression: f32,
    /// The vibrato's controller, once it has moved.
    pub vibrato: Option<f32>,
    /// The pitch bend, −1 … 1.
    pub bend: f32,
    /// CC65 (portamento on).
    pub porta: bool,
}

impl Default for Controls {
    fn default() -> Self {
        Controls {
            dynamic: None,
            expression: 1.0,
            vibrato: None,
            bend: 0.0,
            porta: false,
        }
    }
}

/// Telemetry slots (from the frame's own, 8).
pub mod tel {
    pub const INSTRUMENT: usize = 8;
    pub const TECHNIQUE: usize = 9;
    pub const KEY: usize = 10;
    pub const F0: usize = 11;
    pub const STRING: usize = 12;
    pub const BETA: usize = 13;
    pub const FORCE: usize = 14;
    pub const SPEED: usize = 15;
    pub const DIR: usize = 16;
    pub const DYNAMIC: usize = 17;
    pub const VIBRATO: usize = 18;
    pub const STICK: usize = 19;
    pub const PEAK: usize = 20;
    pub const CPU_NS: usize = 21;
    pub const ALIVE: usize = 22;
    pub const LEVEL: usize = 23;
    pub const FMIN: usize = 24;
    pub const FMAX: usize = 25;
}

/// The instrument.
#[derive(Clone, Debug)]
pub struct Arcus {
    sr: f32,
    s: ArcusSettings,
    voices: Vec<Voice>,
    alloc: Allocator,
    ctl: Controls,
    sustain: bool,
    clock: u64,
    lanes: Lanes,
    used: [bool; MAX_VOICES + GHOSTS],
    buf_l: Vec<f32>,
    buf_r: Vec<f32>,
    body: Body,
    /// The radiation's first-order high-pass (its step, and each side's state `(x₁, y₁)`).
    rad_c: f32,
    rad: [(f32, f32); 4],
    out: Output,
    hammer: HammerShape,
    hammer_area: f32,
    /// The track's tuning: semitones each key sits from 12-TET at A440 (ADR 0155).
    tuning: [f32; 128],
    /// The next note's own articulation and bow (ADR 0158), by its id.
    pending: Option<(u64, Option<Art>, Option<BowDir>)>,
    /// The bow's direction for the next re-bow, when alternating.
    next_dir: BowDir,
    /// The pitch table (`None`: no correction, for the tool that measures it).
    table: Option<&'static pitch::PitchTable>,
    stolen: u32,
    newest: usize,
    peak: f32,
    cpu_ns: f32,
}

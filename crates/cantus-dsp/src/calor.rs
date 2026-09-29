//! Calor, the analog-modelling polysynth: its DSP (ADR 0023, `research/instruments/
//! analog.md` §8). Built on [`crate::va`].
//!
//! ```text
//!  note ─► allocator (poly / mono / legato, unison stack) ─► voice card (seeded trims)
//!  per voice, at 1×/2×/4×:
//!   OSC 1 ─┬─ sync ◄─ OSC 2, OSC 3        ┌ MIXER ─────────────┐   ┌ HPF ┐   ┌ VCF (5 models) ┐   ┌ VCA ┐
//!   SUB ◄──┘  xmod OSC 2 ─► OSC 1         │ levels drive the   ├──►│ pre ├──►│ drive, reso,   ├──►│ env ├─► pan
//!   NOISE                                 │ filter; feedback ◄─┼───┼─────┼───┼ bass comp, FM  │   │     │
//!   pitch: key + glide + bend + drift + card + vibrato + poly-mod + MPE      └────────────────┘   └─────┘
//!  instance: Σ voices ─► decimate (half-band) ─► DC block ─► chorus ─► gain ─► output stage
//! ```
//!
//! **One quality everywhere.** Draft (1×, one solver pass), Good (2×, two passes) or Best
//! (4×, solved to 10⁻⁶): the render uses the quality the player hears — there is no
//! offline switch to forget and no host flag to trust.
//!
//! **Deterministic analog.** Oscillator phases, drift, noise and a note's random value are
//! seeded from the note's id (a clip note's is stable), so a note sounds the same at every
//! block size and wherever playback starts. The card a note lands on comes from the same
//! hash, walked round the cards not sounding — round-robin like the hardware, without a
//! counter that forgets on a seek. Live notes (whose ids repeat per key) rotate.
//!
//! **Voices** run their controls on a 16-sample grid counted from their own start, and
//! everything that moves every sample (envelopes, filter FM, cross-mod) runs at the
//! oversampled rate. A stolen voice is not cut: its state moves to a ghost that fades over
//! three milliseconds while the new note starts fresh.

#[path = "calor_chorus.rs"]
pub mod chorus;
#[path = "calor_settings.rs"]
mod settings;
#[path = "calor_synth.rs"]
mod synth;
#[path = "calor_voice.rs"]
mod voice;

#[cfg(test)]
#[path = "calor_tests.rs"]
mod tests;

pub use settings::{src, CalorSettings, Target, TARGETS};
pub use synth::{tel, Calor, Expression};

/// Physical voices (notes × unison) an instance can sound at once.
pub const MAX_VOICES: usize = 32;
/// Voices fading out after being stolen.
pub const GHOSTS: usize = 8;
/// Base-rate samples between two control updates of a voice.
pub const CONTROL: usize = 16;
/// Voice cards.
pub const CARDS: usize = crate::va::CARDS;

/// Accuracy, which is also CPU: the oversampling factor and how hard the filters solve.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum Quality {
    Draft,
    #[default]
    Good,
    Best,
}

impl Quality {
    pub fn from_index(i: f32) -> Quality {
        match i.round() as i32 {
            0 => Quality::Draft,
            2 => Quality::Best,
            _ => Quality::Good,
        }
    }

    pub fn factor(self) -> usize {
        match self {
            Quality::Draft => 1,
            Quality::Good => 2,
            Quality::Best => 4,
        }
    }

    pub fn solve(self) -> crate::va::Solve {
        match self {
            Quality::Draft => crate::va::Solve::Once,
            Quality::Good => crate::va::Solve::Twice,
            Quality::Best => crate::va::Solve::Converge,
        }
    }
}

/// How a note chooses its voice card.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum Assign {
    /// Round the cards, as a polysynth's assigner does (seeded by the note).
    #[default]
    Rotate,
    /// The same key, the same card.
    Reuse,
    /// The card of the voice slot (the lowest free).
    Lowest,
}

impl Assign {
    pub fn from_index(i: f32) -> Assign {
        match i.round() as i32 {
            1 => Assign::Reuse,
            2 => Assign::Lowest,
            _ => Assign::Rotate,
        }
    }
}

/// An oscillator's circuit revision.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum OscRev {
    /// Mathematically clean shapes.
    Ideal,
    /// A charging capacitor's saw, a slightly rounded triangle, AC-coupled output.
    #[default]
    VintageA,
    /// All of that, more so.
    VintageB,
}

impl OscRev {
    pub fn from_index(i: f32) -> OscRev {
        match i.round() as i32 {
            0 => OscRev::Ideal,
            2 => OscRev::VintageB,
            _ => OscRev::VintageA,
        }
    }

    /// (saw curvature, triangle rounding).
    pub fn shape(self) -> (f32, f32) {
        match self {
            OscRev::Ideal => (0.0, 0.0),
            OscRev::VintageA => (0.05, 0.1),
            OscRev::VintageB => (0.1, 0.22),
        }
    }
}

/// Where the high-pass sits.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum HpfPos {
    #[default]
    Pre,
    Post,
    /// After the filter, with a +6 dB low shelf under 150 Hz.
    Boost,
}

impl HpfPos {
    pub fn from_index(i: f32) -> HpfPos {
        match i.round() as i32 {
            1 => HpfPos::Post,
            2 => HpfPos::Boost,
            _ => HpfPos::Pre,
        }
    }
}

/// LFO shapes.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum LfoShape {
    #[default]
    Triangle,
    Sine,
    SawUp,
    SawDown,
    Square,
    SampleHold,
    Smooth,
}

impl LfoShape {
    pub fn from_index(i: f32) -> LfoShape {
        match i.round() as i32 {
            1 => LfoShape::Sine,
            2 => LfoShape::SawUp,
            3 => LfoShape::SawDown,
            4 => LfoShape::Square,
            5 => LfoShape::SampleHold,
            6 => LfoShape::Smooth,
            _ => LfoShape::Triangle,
        }
    }
}

/// When an LFO starts its cycle.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum LfoRestart {
    /// Never: every voice in phase, on the stream's clock.
    #[default]
    Free,
    /// At each note, from its phase.
    Gate,
    /// At each note, from a phase drawn from the note.
    Random,
}

impl LfoRestart {
    pub fn from_index(i: f32) -> LfoRestart {
        match i.round() as i32 {
            1 => LfoRestart::Gate,
            2 => LfoRestart::Random,
            _ => LfoRestart::Free,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct LfoSettings {
    pub rate: f32,
    pub shape: LfoShape,
    /// Seconds of fade-in from the note's start (≤ 1 ms: none).
    pub delay: f32,
    pub restart: LfoRestart,
    /// Start phase, 0…1.
    pub phase: f32,
    pub unipolar: bool,
}

impl Default for LfoSettings {
    fn default() -> Self {
        LfoSettings {
            rate: 5.0,
            shape: LfoShape::Triangle,
            delay: 0.0,
            restart: LfoRestart::Free,
            phase: 0.0,
            unipolar: false,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct OscSettings {
    /// Octaves, semitones and cents from the note.
    pub octave: f32,
    pub semi: f32,
    pub fine: f32,
    pub saw: f32,
    pub pulse: f32,
    pub tri: f32,
    pub pw: f32,
    /// LFO 2 → pulse width, 0…1.
    pub pwm: f32,
    pub rev: OscRev,
    /// Hard-synced to oscillator 1 (oscillators 2 and 3).
    pub sync: bool,
}

impl Default for OscSettings {
    fn default() -> Self {
        OscSettings {
            octave: 0.0,
            semi: 0.0,
            fine: 0.0,
            saw: 1.0,
            pulse: 0.0,
            tri: 0.0,
            pw: 0.5,
            pwm: 0.0,
            rev: OscRev::VintageA,
            sync: false,
        }
    }
}

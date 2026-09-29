//! Unda, the wavetable / hybrid flagship synth: its DSP. Owned by this instrument's
//! builder (ADR 0023); the parameter model is `cantus_core::inst::wavetable`, the unit
//! `cantus_engine::inst::wavetable`, and `docs/instruments/unda.md` says what was built.
//!
//! Per voice: three wavetable oscillators (up to 16 unison lanes each, two warp slots),
//! a sub and a noise source → two filters (serial ↔ parallel) → the amplifier (envelope
//! 1), all at the oversampled rate (2× by default). The voices are summed there and **one
//! decimator per instrument** brings the sum back to the base rate.
//!
//! Modulation is evaluated on a 16-sample grid counted from the start of the stream (and
//! once more at every note-on), with the values ramped per sample in between, so the
//! output is identical at every block size. Every random number (start phases, drift,
//! the random detune law, S&H and noise) is a hash of the patch seed, the note's id and
//! key and what it is for — never of a global counter or the clock.

#[path = "unda_decim.rs"]
pub mod decim;
#[path = "unda_factory.rs"]
pub mod factory;
#[path = "unda_filter.rs"]
pub mod filter;
#[path = "unda_grana.rs"]
pub mod grana;
#[path = "unda_mod.rs"]
pub mod modsrc;
#[path = "unda_osc.rs"]
pub mod osc;
#[path = "unda_partes.rs"]
pub mod partes;
#[path = "unda_synth.rs"]
mod synth;
#[path = "unda_table.rs"]
pub mod table;
#[path = "unda_voice.rs"]
mod voice;

#[cfg(test)]
#[path = "unda_tests.rs"]
mod tests;

pub use synth::{Meter, Unda, MAX_VOICES};
pub use table::Table;

use crate::modmatrix::ModMatrix;
use crate::voices::{PlayMode, Priority};
use filter::FilterType;
use modsrc::{LfoMode, LfoWave, Shape};
use osc::Warp;

/// Samples (at the base rate) between two evaluations of the modulation.
pub const CONTROL: usize = 16;
/// The shared model's sources, in its order (`cantus_core::inst::modulation::SOURCES`):
/// a route's `src` indexes this.
pub const SOURCES: usize = 32;

/// Where each modulatable value sits in [`Settings::base`] (and a voice's modulated
/// copy). The engine maps parameter keys onto these.
pub mod target {
    pub const GAIN: usize = 0;
    pub const ROUTING: usize = 1;
    /// Oscillator `o`'s value `v` is `OSC + o * OSC_N + v`.
    pub const OSC: usize = 2;
    pub const OSC_N: usize = 17;
    pub const POS: usize = 0;
    pub const PITCH: usize = 1;
    pub const FINE: usize = 2;
    pub const LEVEL: usize = 3;
    pub const PAN: usize = 4;
    pub const DETUNE: usize = 5;
    pub const BLEND: usize = 6;
    pub const WIDTH: usize = 7;
    pub const SPREAD: usize = 8;
    pub const WARP1: usize = 9;
    pub const WARP2: usize = 10;
    /// Partes.
    pub const TILT: usize = 11;
    pub const STRETCH: usize = 12;
    pub const ODDEVEN: usize = 13;
    /// Grana.
    pub const DENSITY: usize = 14;
    pub const GLEN: usize = 15;
    pub const SPRAY: usize = 16;
    pub const SUB_LEVEL: usize = OSC + 3 * OSC_N;
    pub const SUB_PAN: usize = SUB_LEVEL + 1;
    pub const NOISE_LEVEL: usize = SUB_LEVEL + 2;
    pub const NOISE_PAN: usize = SUB_LEVEL + 3;
    pub const NOISE_COLOR: usize = SUB_LEVEL + 4;
    pub const NOISE_WIDTH: usize = SUB_LEVEL + 5;
    /// Filter `f`'s value `v` is `FLT + f * FLT_N + v`.
    pub const FLT: usize = SUB_LEVEL + 6;
    pub const FLT_N: usize = 6;
    pub const CUTOFF: usize = 0;
    pub const RES: usize = 1;
    pub const DRIVE: usize = 2;
    pub const KEY: usize = 3;
    pub const STEREO: usize = 4;
    pub const MIX: usize = 5;
    /// Envelope `e`'s value `v` is `ENV + e * ENV_N + v`.
    pub const ENV: usize = FLT + 2 * FLT_N;
    pub const ENV_N: usize = 6;
    pub const DELAY: usize = 0;
    pub const ATTACK: usize = 1;
    pub const HOLD: usize = 2;
    pub const DECAY: usize = 3;
    pub const SUSTAIN: usize = 4;
    pub const RELEASE: usize = 5;
    /// LFO `l`'s rate is `LFO_RATE + l`.
    pub const LFO_RATE: usize = ENV + 4 * ENV_N;
    pub const COUNT: usize = LFO_RATE + 8;
}

/// A target's range, for turning a route's depth (a fraction of the range) into units.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Range {
    pub min: f32,
    pub max: f32,
    /// Depth moves it in octaves (equal ratios) rather than units.
    pub log: bool,
}

impl Range {
    #[inline]
    pub fn apply(&self, base: f32, offset: f32) -> f32 {
        let v = if self.log && self.min > 0.0 {
            base * (self.max / self.min).powf(offset)
        } else {
            base + offset * (self.max - self.min)
        };
        v.clamp(self.min, self.max)
    }
}

/// Where a source goes.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum Route {
    #[default]
    F1,
    F2,
    Both,
    Direct,
}

impl Route {
    pub fn from_index(i: f32) -> Route {
        match i.round() as i32 {
            1 => Route::F2,
            2 => Route::Both,
            3 => Route::Direct,
            _ => Route::F1,
        }
    }
}

/// Which table an oscillator plays.
#[derive(Clone, Debug, Default)]
pub enum TableChoice {
    /// A factory table, by index.
    Factory(usize),
    /// An imported one (built off the audio thread, shared).
    Imported(std::sync::Arc<Table>),
    /// A file that could not be read: silence.
    #[default]
    Missing,
}

/// What an oscillator is.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum Engine {
    #[default]
    Wavetable,
    /// Additive: the table's frame as partials, tilted, balanced, stretched.
    Partes,
    /// Granular: grains of an imported sample, or of the table.
    Grana,
}

impl Engine {
    pub fn from_index(i: f32) -> Engine {
        match i.round() as i32 {
            1 => Engine::Partes,
            2 => Engine::Grana,
            _ => Engine::Wavetable,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct OscSettings {
    pub on: bool,
    pub engine: Engine,
    pub table: TableChoice,
    /// Grana's source when a file is imported (an `Arc` of the decoded audio).
    pub sample: Option<crate::pulsus::SampleRef>,
    pub partials: usize,
    pub window: grana::Window,
    /// Grana: pitch jitter (cents either way) and the probability of a reversed grain.
    pub pitch_rand: f32,
    pub reverse: f32,
    /// Octaves and semitones, whole.
    pub oct: f32,
    pub semi: f32,
    /// Start phase, 0…1 of a cycle.
    pub phase: f32,
    /// How much of a cycle the start phase is randomised, 0…1.
    pub rand: f32,
    pub route: Route,
    pub unison: usize,
    pub curve: usize,
    pub stack: usize,
    pub warp: [Warp; 2],
    pub warp_src: [usize; 2],
}

#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct EnvCurves {
    pub attack: f32,
    pub decay: f32,
    pub release: f32,
}

#[derive(Copy, Clone, Debug, Default)]
pub struct LfoSettings {
    pub wave: LfoWave,
    pub mode: LfoMode,
    /// 0…1 of a cycle.
    pub phase: f32,
    pub delay: f32,
    pub rise: f32,
    pub smooth: f32,
    pub shape: Shape,
}

#[derive(Copy, Clone, Debug, Default)]
pub struct FilterSettings {
    pub on: bool,
    pub ty: FilterType,
}

/// Everything the synth is told, resolved from the parameters (the engine's
/// `settings(spec)`). Allocation-free to build and to copy in.
#[derive(Clone, Debug)]
pub struct Settings {
    /// 1, 2 or 4.
    pub oversample: usize,
    pub polyphony: usize,
    pub mode: PlayMode,
    pub priority: Priority,
    pub glide_s: f32,
    pub glide_always: bool,
    pub bend_st: f32,
    pub vel_depth: f32,
    pub drift: f32,
    pub seed: u64,
    pub osc: [OscSettings; 3],
    pub sub_on: bool,
    pub sub_wave: usize,
    pub sub_oct: f32,
    pub sub_route: Route,
    pub noise_on: bool,
    pub noise_route: Route,
    pub filters: [FilterSettings; 2],
    pub env_curves: [EnvCurves; 4],
    pub lfo: [LfoSettings; 8],
    /// Every modulatable value, in its unit ([`target`]).
    pub base: [f32; target::COUNT],
    pub range: [Range; target::COUNT],
    pub routes: ModMatrix,
    /// Which sources a route reads (so the others are not computed).
    pub used_sources: [bool; SOURCES],
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            oversample: 2,
            polyphony: 16,
            mode: PlayMode::Poly,
            priority: Priority::Last,
            glide_s: 0.0,
            glide_always: false,
            bend_st: 2.0,
            vel_depth: 0.5,
            drift: 0.0,
            seed: 0,
            osc: Default::default(),
            sub_on: false,
            sub_wave: 0,
            sub_oct: -1.0,
            sub_route: Route::Direct,
            noise_on: false,
            noise_route: Route::F1,
            filters: [FilterSettings::default(); 2],
            env_curves: [EnvCurves::default(); 4],
            lfo: [LfoSettings::default(); 8],
            base: [0.0; target::COUNT],
            range: [Range::default(); target::COUNT],
            routes: ModMatrix::default(),
            used_sources: [false; SOURCES],
        }
    }
}

/// Source indices in the shared order.
pub mod source {
    pub const ENV: usize = 0;
    pub const LFO: usize = 4;
    pub const VEL: usize = 12;
    pub const RELVEL: usize = 13;
    pub const KEY: usize = 14;
    pub const RND: usize = 15;
    pub const MPE_BEND: usize = 16;
    pub const MPE_PRESS: usize = 17;
    pub const MPE_SLIDE: usize = 18;
    pub const MACRO: usize = 19;
    pub const MW: usize = 27;
    pub const AT: usize = 28;
    pub const PB: usize = 29;
    pub const EXPR: usize = 30;
    pub const SUS: usize = 31;
}

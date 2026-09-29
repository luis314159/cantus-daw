//! Calor's settings in real units, and its modulation sources and targets.

use super::chorus::ChorusMode;
use super::{Assign, HpfPos, LfoRestart, LfoSettings, OscSettings, Quality, CARDS};
use crate::va::drift::Variance;
use crate::va::{EnvSettings, FilterSettings, SubShape};
use crate::voices::{PlayMode, Priority};

/// Everything a Calor plays with, in real units, resolved from its parameters.
#[derive(Clone, Debug, PartialEq)]
pub struct CalorSettings {
    pub mode: PlayMode,
    pub priority: Priority,
    /// Notes at once (each is `stack` voices).
    pub voices: usize,
    pub stack: usize,
    /// Cents from the lowest to the highest voice of a stack.
    pub stack_detune: f32,
    /// 0…1 of the stereo field a stack spreads across.
    pub stack_spread: f32,
    pub assign: Assign,
    /// Seconds (0: off). In rate mode, seconds per octave.
    pub glide: f32,
    pub glide_rate: bool,
    pub glide_linear: bool,
    /// Glide only from a note still held.
    pub glide_legato: bool,
    /// Semitones.
    pub transpose: f32,
    /// Cents.
    pub fine: f32,
    pub bend_up: f32,
    pub bend_down: f32,
    pub osc: [OscSettings; 3],
    /// Oscillator 3 an LFO (seven octaves down).
    pub osc3_low: bool,
    /// Oscillator 3 follows the keys (else it stays at middle C).
    pub osc3_kbd: bool,
    /// Oscillator 2 → oscillator 1 exponential FM, 0…1 (1 = ±4 octaves).
    pub xmod: f32,
    pub polymod_env: f32,
    pub polymod_osc3: f32,
    pub polymod_freq: bool,
    pub polymod_pw: bool,
    pub polymod_vcf: bool,
    pub sub_shape: SubShape,
    pub noise_pink: bool,
    /// Mixer levels: oscillators 1–3, the sub, the noise.
    pub mix: [f32; 5],
    pub feedback: f32,
    /// Linear gain of the mixer into the filter.
    pub mix_drive: f32,
    /// Hz; at or under 10 the high-pass is out.
    pub hpf: f32,
    pub hpf_pos: HpfPos,
    pub filter: FilterSettings,
    pub cutoff: f32,
    /// 1.0 = one octave of cutoff per octave of key, around middle C.
    pub key_track: f32,
    /// Octaves of envelope 1.
    pub env_amount: f32,
    /// 0…1: how much velocity scales the envelope amount.
    pub env_velocity: f32,
    /// Octaves of LFO 2.
    pub lfo_amount: f32,
    /// Oscillator 3 → cutoff, 0…1 (1 = ±2 octaves at audio rate).
    pub filter_fm: f32,
    /// The amplifier follows the gate, not envelope 2.
    pub vca_gate: bool,
    pub vca_velocity: f32,
    pub vca_drive: f32,
    pub pan: f32,
    pub env: [EnvSettings; 2],
    /// Key scaling of each envelope's times (1: half per octave up).
    pub env_kbd: [f32; 2],
    /// Velocity → each envelope's level.
    pub env_vel: [f32; 2],
    /// A retrigger restarts from silence (the old sound fades out) instead of from the
    /// level the envelope is at.
    pub env_reset: [bool; 2],
    pub lfo: [LfoSettings; 2],
    /// LFO 1 → pitch, cents (fixed vibrato).
    pub lfo1_pitch: f32,
    pub wheel_vibrato: f32,
    pub wheel_mix: f32,
    pub wheel_pw: f32,
    pub wheel_vcf: f32,
    /// 0…1: Calibrated … Unruly. Scales drift and every variance (0.5 applies them as set).
    pub condition: f32,
    pub seed: u32,
    /// Cents of drift at condition 0.5.
    pub drift: f32,
    pub drift_rate: f32,
    /// The cards' spreads at condition 0.5.
    pub variance: Variance,
    /// Oscillators restart at phase 0 on each note (else free-running).
    pub phase_reset: bool,
    pub rolls: [u32; CARDS],
    pub quality: Quality,
    pub chorus: ChorusMode,
    pub chorus_mix: f32,
    /// Linear output gain.
    pub gain: f32,
}

impl CalorSettings {
    /// The Condition's scale on drift and variance: 0 at Calibrated, 1 at Worn (the
    /// amounts as set), 2 at Unruly.
    pub fn condition_scale(&self) -> f32 {
        2.0 * self.condition.clamp(0.0, 1.0)
    }

    /// The cards' spreads at the current Condition.
    pub fn card_variance(&self) -> Variance {
        self.variance.scaled(self.condition_scale())
    }
}

impl Default for CalorSettings {
    fn default() -> Self {
        let osc2 = OscSettings {
            fine: 7.0,
            ..Default::default()
        };
        let osc3 = OscSettings {
            fine: -5.0,
            ..Default::default()
        };
        CalorSettings {
            mode: PlayMode::Poly,
            priority: Priority::Last,
            voices: 8,
            stack: 1,
            stack_detune: 10.0,
            stack_spread: 0.5,
            assign: Assign::Rotate,
            glide: 0.0,
            glide_rate: false,
            glide_linear: false,
            glide_legato: false,
            transpose: 0.0,
            fine: 0.0,
            bend_up: 2.0,
            bend_down: 2.0,
            osc: [OscSettings::default(), osc2, osc3],
            osc3_low: false,
            osc3_kbd: true,
            xmod: 0.0,
            polymod_env: 0.0,
            polymod_osc3: 0.0,
            polymod_freq: true,
            polymod_pw: false,
            polymod_vcf: false,
            sub_shape: SubShape::Square1,
            noise_pink: false,
            mix: [0.8, 0.8, 0.0, 0.0, 0.0],
            feedback: 0.0,
            mix_drive: 1.0,
            hpf: 10.0,
            hpf_pos: HpfPos::Pre,
            filter: FilterSettings {
                reso: 0.2,
                bass_comp: 0.5,
                ..Default::default()
            },
            cutoff: 2000.0,
            key_track: 0.5,
            env_amount: 2.0,
            env_velocity: 0.3,
            lfo_amount: 0.0,
            filter_fm: 0.0,
            vca_gate: false,
            vca_velocity: 0.7,
            vca_drive: 0.0,
            pan: 0.0,
            env: [
                EnvSettings {
                    attack: 0.005,
                    decay: 0.3,
                    sustain: 0.3,
                    release: 0.3,
                    ..Default::default()
                },
                EnvSettings {
                    attack: 0.002,
                    decay: 0.2,
                    sustain: 0.8,
                    release: 0.25,
                    ..Default::default()
                },
            ],
            env_kbd: [0.0; 2],
            env_vel: [0.0; 2],
            env_reset: [false; 2],
            lfo: [
                LfoSettings::default(),
                LfoSettings {
                    rate: 0.8,
                    restart: LfoRestart::Gate,
                    ..Default::default()
                },
            ],
            lfo1_pitch: 0.0,
            wheel_vibrato: 30.0,
            wheel_mix: 0.0,
            wheel_pw: 0.0,
            wheel_vcf: 0.0,
            condition: 0.25,
            seed: 1,
            drift: 3.0,
            drift_rate: 0.2,
            variance: Variance {
                detune_cents: 4.0,
                cutoff_st: 0.3,
                env: 0.05,
                pw: 0.01,
                glide: 0.05,
                tracking_cents: 0.5,
                pan: 0.3,
            },
            phase_reset: false,
            rolls: [0; CARDS],
            quality: Quality::Good,
            chorus: ChorusMode::Off,
            chorus_mix: 0.5,
            gain: 1.0,
        }
    }
}

/// Source slots, in the order of `cantus_core::inst::analog::SOURCES`.
pub mod src {
    pub const ENV1: usize = 0;
    pub const ENV2: usize = 1;
    pub const LFO1: usize = 2;
    pub const LFO2: usize = 3;
    pub const VEL: usize = 4;
    pub const RELVEL: usize = 5;
    pub const KEY: usize = 6;
    pub const RND: usize = 7;
    pub const MPE_BEND: usize = 8;
    pub const MPE_PRESS: usize = 9;
    pub const MPE_SLIDE: usize = 10;
    /// Macros 1–8 are `MACRO1 + m`.
    pub const MACRO1: usize = 11;
    pub const MW: usize = 19;
    pub const AT: usize = 20;
    pub const PB: usize = 21;
    pub const EXPR: usize = 22;
    pub const SUS: usize = 23;
    pub const COUNT: usize = 24;
}

/// What a matrix route can move, each in its own unit (the engine scales a route's depth,
/// a fraction of the parameter's range, into it).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(u16)]
pub enum Target {
    /// Cents, every oscillator.
    Pitch,
    /// Cents, one oscillator.
    Osc1Pitch,
    Osc2Pitch,
    Osc3Pitch,
    /// Fraction of the cycle.
    Pw1,
    Pw2,
    Pw3,
    /// Levels, 0…1.
    Mix1,
    Mix2,
    Mix3,
    MixSub,
    MixNoise,
    Feedback,
    /// dB.
    MixDrive,
    /// Octaves.
    Cutoff,
    /// Resonance (1 = the threshold).
    Reso,
    /// dB.
    VcfDrive,
    Morph,
    FilterFm,
    /// Octaves.
    EnvAmount,
    BassComp,
    /// Octaves.
    Hpf,
    VcaDrive,
    /// −1…1.
    Pan,
    /// dB.
    Gain,
    /// Octaves of rate.
    Lfo1Rate,
    Lfo2Rate,
    Xmod,
    PolyEnv,
    PolyOsc3,
    /// Octaves of time for the envelopes' stages; sustain as a level.
    Env1A,
    Env1D,
    Env1S,
    Env1R,
    Env2A,
    Env2D,
    Env2S,
    Env2R,
    /// Cents of vibrato depth.
    Vibrato,
}

/// How many targets there are.
pub const TARGETS: usize = Target::Vibrato as usize + 1;

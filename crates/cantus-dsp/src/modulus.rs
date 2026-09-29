//! Modulus, the FM synth: its DSP (ADR 0023; research `fm-additive-granular.md` §4, §8.5).
//!
//! **Eight operators, any to any.** Each voice runs operators A…H as phase modulation of our
//! own sine table (or band-limited triangle, saw and square tables), driven by envelopes.
//! The matrix is free: an edge from *s* to *t* adds `index · y_s` radians to *t*'s phase.
//! Operators are evaluated in a fixed order, **H first, A last** (the DX7's 6 → 1): an edge
//! from a later letter to an earlier one uses this sample, the other way round (a cycle)
//! uses the previous one, and self-feedback goes through the DX7's two-sample average
//! (Tomisawa's anti-hunting filter, a zero at Nyquist), which is on by default and a switch.
//!
//! **Aliasing is a setting saved with the patch, and the render is the playback.** Each
//! voice runs at 1, 2, 4 or 8 times the host rate, chosen once at its note-on — by the
//! patch (`quality`), or by Auto from the Carson bandwidth the voice can reach there — and
//! comes back through polyphase IIR half-bands. The guard can also *tame*: scale the
//! voice's indices so its bandwidth stays below what its rate can carry. Feedback and
//! back edges are delayed by one *host* sample whatever the rate, so the quality setting
//! changes the aliasing and never the timbre. Nothing depends on the block size or on
//! whether the render is live: envelopes and modulation step on a 16-sample grid counted
//! from the start of the stream, and every random number is seeded from the note.
//!
//! **Envelopes** are DX7 rate/level envelopes (from the published measurements) or modern
//! time/level breakpoints with curves, a sustain point and a loop, per operator; with
//! keyboard level scaling (the DX break point, depths and four curves), keyboard rate
//! scaling and velocity sensitivity.

#[path = "modulus_env.rs"]
pub mod env;
#[path = "modulus_guard.rs"]
pub mod guard;
#[path = "modulus_synth.rs"]
mod synth;
#[path = "modulus_tables.rs"]
pub mod tables;
#[cfg(test)]
#[path = "modulus_tests.rs"]
mod tests;
#[path = "modulus_voice.rs"]
mod voice;

pub use synth::{baked_dx, baked_time, tel, Modulus};
pub use voice::{key_scaling_db, velocity_db};

use crate::modmatrix::ModMatrix;
use crate::voices::{PlayMode, Priority};

pub const OPS: usize = 8;
pub const ENV_POINTS: usize = 8;
/// Voices in the pool (a note with unison takes several).
pub const MAX_VOICES: usize = crate::voices::MAX_VOICES;
/// Envelopes and modulation step every this many host samples.
pub const CONTROL: usize = 16;
/// An output level at or below this is off.
pub const OUT_OFF: f32 = -60.0;
/// One voice at full level: −18 dB, so an eight-note chord stays under 0 dBFS.
pub const VOICE_SCALE: f32 = 0.125;
/// Phase units (2³² per cycle) per radian.
pub const PHASE_PER_RAD: f32 = 4_294_967_296.0 / std::f32::consts::TAU;

/// `quality`.
pub const QUALITY_ECO: u8 = 0;
pub const QUALITY_AUTO: u8 = 1;
/// `guard`.
pub const GUARD_OFF: u8 = 0;
pub const GUARD_SHOW: u8 = 1;
pub const GUARD_TAME: u8 = 2;

/// The oversampling factor a fixed quality asks for (Auto decides per voice).
pub fn fixed_factor(quality: u8) -> Option<usize> {
    match quality {
        QUALITY_ECO => Some(1),
        2 => Some(2),
        3 => Some(4),
        4 => Some(8),
        _ => None,
    }
}

/// A sine of `x` cycles, for control-rate shapes (the LFO); cheap and the same everywhere.
#[inline]
pub fn fast_sin_cycles(x: f32) -> f32 {
    // Bhaskara-style parabola, refined: error under 0.1 %.
    let x = x - x.floor();
    let t = if x < 0.5 { x * 2.0 } else { x * 2.0 - 1.0 };
    let y = 4.0 * t * (1.0 - t);
    let y = 0.225 * (y * y - y) + y;
    if x < 0.5 {
        y
    } else {
        -y
    }
}

#[inline]
pub fn db_to_lin(db: f32) -> f32 {
    (db * (std::f32::consts::LOG2_10 / 20.0)).exp2()
}

/// A breakpoint envelope's settings: `n` points reached one after the other from `l0`.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct TimeEnvSettings {
    /// Points in use, 1…8.
    pub n: usize,
    /// The point held while the key is down (1…n), 0 for none (a one-shot).
    pub sus: usize,
    /// The point a held note loops back to from `sus`, 0 for none.
    pub lp: usize,
    pub l0: f32,
    /// Seconds to reach each point.
    pub t: [f32; ENV_POINTS],
    /// Levels, 0…1.
    pub l: [f32; ENV_POINTS],
    /// Bends, −1…1.
    pub c: [f32; ENV_POINTS],
    /// Times shorten by this per octave above C4 (−1…1: ×2^(−k) per octave).
    pub key_time: f32,
    /// Times shorten with velocity (−1…1).
    pub vel_time: f32,
}

impl Default for TimeEnvSettings {
    fn default() -> Self {
        let mut t = [0.1; ENV_POINTS];
        let mut l = [0.0; ENV_POINTS];
        t[..3].copy_from_slice(&[0.005, 0.3, 0.25]);
        l[..2].copy_from_slice(&[1.0, 0.7]);
        TimeEnvSettings {
            n: 3,
            sus: 2,
            lp: 0,
            l0: 0.0,
            t,
            l,
            c: [0.0; ENV_POINTS],
            key_time: 0.0,
            vel_time: 0.0,
        }
    }
}

/// A DX7 envelope: four rates and four levels, 0…99.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct DxEnvSettings {
    pub rates: [u8; 4],
    pub levels: [u8; 4],
}

impl Default for DxEnvSettings {
    fn default() -> Self {
        DxEnvSettings {
            rates: [99; 4],
            levels: [99, 99, 99, 0],
        }
    }
}

/// One operator.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct OpSettings {
    pub on: bool,
    /// 0 sine, 1 triangle, 2 saw, 3 square.
    pub wave: u8,
    pub fixed: bool,
    pub ratio: f32,
    /// Cents.
    pub detune: f32,
    /// Hz added after the ratio: beating that stays constant across the keyboard.
    pub offset: f32,
    pub fixed_hz: f32,
    /// dB, 0 at full: scales everything the operator sends (index and output).
    pub level_db: f32,
    /// Velocity sensitivity, 0…1: at 1, velocity 0.1 is 40 dB down.
    pub vel: f32,
    /// Start phase, degrees.
    pub phase: f32,
    pub key_sync: bool,
    /// Follows the pitch envelope.
    pub penv: bool,
    /// LFO 1 amplitude-modulation sensitivity, 0…3.
    pub ams: u8,
    pub out_db: f32,
    /// −1…1.
    pub pan: f32,
    /// Self-feedback, radians.
    pub fb: f32,
    pub dx: bool,
    pub time: TimeEnvSettings,
    pub dxenv: DxEnvSettings,
    pub ks_break: u8,
    /// dB at two octaves from the break point.
    pub ks_ldepth: f32,
    pub ks_rdepth: f32,
    /// 0 −lin, 1 −exp, 2 +exp, 3 +lin (the DX's order).
    pub ks_lcurve: u8,
    pub ks_rcurve: u8,
    /// DX keyboard rate scaling, 0…7.
    pub ks_rate: u8,
}

impl Default for OpSettings {
    fn default() -> Self {
        OpSettings {
            on: true,
            wave: 0,
            fixed: false,
            ratio: 1.0,
            detune: 0.0,
            offset: 0.0,
            fixed_hz: 440.0,
            level_db: 0.0,
            vel: 0.0,
            phase: 0.0,
            key_sync: true,
            penv: true,
            ams: 0,
            out_db: OUT_OFF,
            pan: 0.0,
            fb: 0.0,
            dx: false,
            time: TimeEnvSettings::default(),
            dxenv: DxEnvSettings::default(),
            ks_break: 60,
            ks_ldepth: 0.0,
            ks_rdepth: 0.0,
            ks_lcurve: 0,
            ks_rcurve: 0,
            ks_rate: 0,
        }
    }
}

/// Everything a Modulus plays, as plain numbers (the engine fills it from the spec).
#[derive(Clone, Debug)]
pub struct Patch {
    pub ops: [OpSettings; OPS],
    /// `m[s][t]`: radians from operator *s* into *t*; the diagonal is unused (`fb`).
    pub m: [[f32; OPS]; OPS],
    pub gain_db: f32,
    pub voices: usize,
    pub mode: PlayMode,
    pub priority: Priority,
    /// Seconds.
    pub glide: f32,
    pub unison: usize,
    /// Cents between the outermost unison copies.
    pub uni_detune: f32,
    pub uni_spread: f32,
    pub transpose: f32,
    /// Cents.
    pub tune: f32,
    pub pb_range: f32,
    pub quality: u8,
    pub guard: u8,
    pub fb_avg: bool,
    /// Cents of seeded per-operator drift.
    pub drift: f32,
    /// Pitch envelope: levels (st) and the times (s) between them; the third is held.
    pub penv_l: [f32; 4],
    pub penv_t: [f32; 3],
    pub lfo_rate: [f32; 2],
    pub lfo_wave: [u8; 2],
    pub lfo_delay: [f32; 2],
    pub lfo_sync: [bool; 2],
    /// LFO 1 → pitch, cents; → amplitude of the operators with `ams`, dB.
    pub lfo1_pitch: f32,
    pub lfo1_amp: f32,
    /// The modulation envelope: attack, decay (s), sustain (0…1), release (s).
    pub menv: [f32; 4],
    /// The easy macros, −1…1: brightness (every index ×4^b), harmonic (modulator ratios
    /// stretched), detune, and envelope attack/decay/sustain/release offsets.
    pub bright: f32,
    pub harm: f32,
    pub detune_macro: f32,
    pub env_macro: [f32; 4],
    /// The shared macros' values (sources).
    pub macros: [f32; 8],
    pub routes: ModMatrix,
}

impl Default for Patch {
    fn default() -> Self {
        let mut ops = [OpSettings::default(); OPS];
        ops[0].out_db = 0.0;
        Patch {
            ops,
            m: [[0.0; OPS]; OPS],
            gain_db: 0.0,
            voices: 16,
            mode: PlayMode::Poly,
            priority: Priority::Last,
            glide: 0.0,
            unison: 1,
            uni_detune: 10.0,
            uni_spread: 0.5,
            transpose: 0.0,
            tune: 0.0,
            pb_range: 2.0,
            quality: QUALITY_AUTO,
            guard: GUARD_SHOW,
            fb_avg: true,
            drift: 0.0,
            penv_l: [0.0; 4],
            penv_t: [0.1; 3],
            lfo_rate: [5.0, 2.0],
            lfo_wave: [0, 1],
            lfo_delay: [0.0; 2],
            lfo_sync: [true; 2],
            lfo1_pitch: 0.0,
            lfo1_amp: 0.0,
            menv: [0.01, 0.5, 0.0, 0.3],
            bright: 0.0,
            harm: 0.0,
            detune_macro: 0.0,
            env_macro: [0.0; 4],
            macros: [0.0; 8],
            routes: ModMatrix::default(),
        }
    }
}

/// Modulation sources, in the order of `cantus_core::inst::fm::SOURCES`.
pub mod source {
    pub const ENV1: usize = 0;
    pub const LFO1: usize = 1;
    pub const LFO2: usize = 2;
    pub const VEL: usize = 3;
    pub const RELVEL: usize = 4;
    pub const KEY: usize = 5;
    pub const RND: usize = 6;
    pub const MPE_BEND: usize = 7;
    pub const MPE_PRESS: usize = 8;
    pub const MPE_SLIDE: usize = 9;
    pub const MACRO: usize = 10;
    pub const MW: usize = 18;
    pub const AT: usize = 19;
    pub const PB: usize = 20;
    pub const EXPR: usize = 21;
    pub const SUS: usize = 22;
    pub const COUNT: usize = 23;
}

/// Modulation targets, each in its own unit (the engine scales a route's depth into it):
/// dB, cents, radians, −1…1, octaves for ratios and rates.
pub mod target {
    use super::OPS;
    pub const GAIN: usize = 0;
    pub const TUNE: usize = 1;
    /// `M + s·8 + t`.
    pub const M: usize = 2;
    pub const FB: usize = M + OPS * OPS;
    pub const OUT: usize = FB + OPS;
    pub const PAN: usize = OUT + OPS;
    pub const LEVEL: usize = PAN + OPS;
    pub const DETUNE: usize = LEVEL + OPS;
    pub const RATIO: usize = DETUNE + OPS;
    pub const LFO1_RATE: usize = RATIO + OPS;
    pub const LFO1_PITCH: usize = LFO1_RATE + 1;
    pub const LFO1_AMP: usize = LFO1_RATE + 2;
    pub const LFO2_RATE: usize = LFO1_RATE + 3;
    pub const BRIGHT: usize = LFO1_RATE + 4;
    pub const HARM: usize = LFO1_RATE + 5;
    pub const MDETUNE: usize = LFO1_RATE + 6;
    pub const COUNT: usize = LFO1_RATE + 7;
}

//! Exemplar, the multisample sampler: its DSP (ADR 0023, `docs/instruments/exemplar.md`,
//! `research/instruments/sampler.md` §4 and §9).
//!
//! - **Zones** map keys × velocities to samples, in **groups**; a note starts one *layer*
//!   per zone it matches ([`voice`]), crossfaded by velocity and key, chosen by round robin
//!   or by a random number drawn from the note itself, so a render repeats exactly.
//! - **The resampler** ([`resample`], [`play`]) is alias-free by default: octave mip-maps,
//!   a windowed sinc at twice the output rate and a half-band decimator. At a ratio of
//!   exactly 1 it hands back the file's own samples, bit for bit.
//! - **Loops** (forward, ping-pong, backward; sustain or continuous) with linear or
//!   equal-power crossfades; **release triggers** that lose `rt_decay` dB per second held,
//!   optionally starting at the level the note had reached; exclusive groups (choke).
//!
//! No allocation after [`Exemplar::new`]: zones, layers and voices live in fixed tables,
//! and samples arrive as `Arc`s whose last owner is never the audio thread.

#[path = "exemplar_resample.rs"]
pub mod resample;

#[path = "exemplar_play.rs"]
pub mod play;

#[path = "exemplar_voice.rs"]
pub mod voice;

#[path = "exemplar_builtin.rs"]
pub mod builtin;

#[path = "exemplar_loop.rs"]
pub mod analysis;

#[path = "exemplar_engine.rs"]
mod engine;

#[cfg(test)]
#[path = "exemplar_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "exemplar_tests2.rs"]
mod tests2;

pub use engine::{crossfade, mix, note_random, LIVE_IDS};

pub use play::{LoopDir, Region, Source};
pub use resample::{halve, Quality, Reader, MAX_LEVELS};
pub use voice::{EnvShape, LayerInfo};

use crate::modmatrix::ModMatrix;
use crate::voices::{Allocator, PlayMode, MAX_VOICES};
use voice::Layer;

/// Zones an instrument holds.
pub const MAX_ZONES: usize = 1024;
/// Groups an instrument holds.
pub const MAX_GROUPS: usize = 64;
/// Layers sounding at once, all notes together (a stolen one fades out in its place).
pub const MAX_LAYERS: usize = 256;
/// Fade for a stolen or choked layer (ADR: at least 5 ms).
pub const STEAL_MS: f32 = 5.0;

/// Modulation sources, by index (the tags of `cantus_core::inst::modulation`).
pub const SOURCES: [&str; 21] = [
    "env1", "env2", "lfo1", "lfo2", "vel", "relvel", "key", "rnd", "macro1", "macro2", "macro3",
    "macro4", "macro5", "macro6", "macro7", "macro8", "mw", "at", "pb", "expr", "sus",
];
const SRC_GLOBAL: usize = 8;

/// Modulation targets, by index, with the span a depth of 1 covers in the target's unit.
pub const TARGETS: [(&str, f32); 9] = [
    ("gain", 60.0),
    ("pan", 200.0),
    ("tune", 200.0),
    ("transpose", 96.0),
    ("filter.cutoff", 9.97),
    ("filter.reso", 100.0),
    ("lfo1.pitch", 100.0),
    ("lfo1.rate", 12.0),
    ("lfo2.rate", 12.0),
];

pub fn source_index(tag: &str) -> Option<u16> {
    SOURCES.iter().position(|s| *s == tag).map(|i| i as u16)
}

pub fn target_index(key: &str) -> Option<u16> {
    TARGETS.iter().position(|t| t.0 == key).map(|i| i as u16)
}

/// The instrument's own settings, in musician's units.
#[derive(Clone, Debug, PartialEq)]
pub struct Globals {
    pub gain_db: f32,
    /// −1 … 1.
    pub pan: f32,
    pub transpose: f32,
    pub tune_ct: f32,
    /// Velocity → amplitude, −1 … 1 (1: amplitude follows velocity², SFZ's default).
    pub vel_amp: f32,
    pub voices: usize,
    pub mode: PlayMode,
    pub glide_ms: f32,
    pub bend_range: f32,
    pub quality: Quality,
    /// Amp and modulation envelopes, times in ms, sustain 0…1.
    pub amp: [f32; 5],
    pub env2: [f32; 4],
    /// 0 off, 1 low-pass, 2 band-pass, 3 high-pass.
    pub filter: u8,
    pub cutoff: f32,
    pub reso: f32,
    /// Cutoff follows the key, 0…1 (1: an octave per octave).
    pub filter_key: f32,
    /// Semitones of cutoff at full velocity.
    pub filter_vel: f32,
    /// Semitones of cutoff at full envelope 2.
    pub filter_env: f32,
    pub lfo_rate: [f32; 2],
    pub lfo_shape: [u8; 2],
    pub lfo1_delay_ms: f32,
    /// Vibrato depth from LFO 1, cents.
    pub vibrato_ct: f32,
    /// Sample start moved later by up to this many ms as velocity falls.
    pub start_vel_ms: f32,
    /// Keyswitch range (−1: none) and the switch in force before any is pressed.
    pub sw_lo: i16,
    pub sw_hi: i16,
    pub sw_default: i16,
    pub macros: [f32; 8],
}

impl Default for Globals {
    fn default() -> Self {
        Globals {
            gain_db: 0.0,
            pan: 0.0,
            transpose: 0.0,
            tune_ct: 0.0,
            vel_amp: 1.0,
            voices: 32,
            mode: PlayMode::Poly,
            glide_ms: 0.0,
            bend_range: 2.0,
            quality: Quality::High,
            amp: [0.1, 0.1, 1_000.0, 1.0, 250.0],
            env2: [0.1, 500.0, 0.0, 250.0],
            filter: 0,
            cutoff: 20_000.0,
            reso: 0.0,
            filter_key: 0.0,
            filter_vel: 0.0,
            filter_env: 0.0,
            lfo_rate: [5.0, 0.5],
            lfo_shape: [0, 1],
            lfo1_delay_ms: 0.0,
            vibrato_ct: 0.0,
            start_vel_ms: 0.0,
            sw_lo: -1,
            sw_hi: -1,
            sw_default: -1,
            macros: [0.0; 8],
        }
    }
}

/// A group: levels added to its zones', and an envelope that overrides the instrument's
/// where set (`None`: follow the instrument).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GroupParams {
    pub gain_db: f32,
    pub pan: f32,
    pub tune_ct: f32,
    /// attack, hold, decay (ms), sustain (0…1), release (ms).
    pub amp: [Option<f32>; 5],
    pub vel_amp: Option<f32>,
    /// Layers of this group sounding at once (0: no limit).
    pub poly: u8,
    pub mute: bool,
}

/// When a zone starts.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum Trig {
    #[default]
    Attack,
    Release,
    First,
    Legato,
    ReleaseKey,
}

/// One zone, as the engine plays it.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct ZoneParams {
    pub lo: u8,
    pub hi: u8,
    pub root: u8,
    pub vlo: u8,
    pub vhi: u8,
    pub tune_ct: f32,
    pub gain_db: f32,
    /// −1 … 1.
    pub pan: f32,
    /// Cents per key.
    pub keytrack: f32,
    pub group: u8,
    pub region: Region,
    pub one_shot: bool,
    /// Loops only while the key is held (then plays on to the end).
    pub sustain_loop: bool,
    /// Crossfade widths (velocity steps, keys) in and out.
    pub vfin: u8,
    pub vfout: u8,
    pub kfin: u8,
    pub kfout: u8,
    /// Crossfade curve: gain (amplitudes sum to 1) rather than power (squares do).
    pub fade_gain: bool,
    /// Round-robin position (1…) of `seqlen`.
    pub seq: u8,
    pub seqlen: u8,
    /// Plays when the note's random number is in `[rlo, rhi)`.
    pub rlo: f32,
    pub rhi: f32,
    pub trig: Trig,
    /// Release samples lose this many dB per second the note was held.
    pub rt_decay: f32,
    /// A release sample starts at the level the note had reached.
    pub rt_follow: bool,
    /// Exclusive class this zone belongs to, and the class that silences it (0: none).
    pub excl: u8,
    pub offby: u8,
    /// Silenced by a release rather than a 5 ms fade.
    pub off_normal: bool,
    /// Plays only while this keyswitch is in force (−1: always).
    pub sw: i16,
    pub delay_ms: f32,
}

impl Default for ZoneParams {
    fn default() -> Self {
        ZoneParams {
            lo: 0,
            hi: 127,
            root: 60,
            vlo: 1,
            vhi: 127,
            tune_ct: 0.0,
            gain_db: 0.0,
            pan: 0.0,
            keytrack: 100.0,
            group: 0,
            region: Region::default(),
            one_shot: false,
            sustain_loop: false,
            vfin: 0,
            vfout: 0,
            kfin: 0,
            kfout: 0,
            fade_gain: false,
            seq: 1,
            seqlen: 1,
            rlo: 0.0,
            rhi: 1.0,
            trig: Trig::Attack,
            rt_decay: 0.0,
            rt_follow: false,
            excl: 0,
            offby: 0,
            off_normal: false,
            sw: -1,
            delay_ms: 0.0,
        }
    }
}

/// What a voice slot remembers of its note.
#[derive(Copy, Clone, Debug, Default)]
struct Slot {
    key: u8,
    id: u64,
    vel: f32,
    /// Sample clock at the note-on (for release decay).
    start: u64,
    /// Round-robin index and random number drawn for the note.
    rr: u32,
    rnd: f32,
    /// Key still down (a pedal may still hold the note after it goes up).
    key_down: bool,
    /// Its release samples have been started.
    released: bool,
    /// The level the note's attack layers had reached when released.
    reached: f32,
    layers: u16,
}

/// The sampler.
#[derive(Debug)]
pub struct Exemplar {
    fs: f32,
    readers: [Reader; 4],
    globals: Globals,
    groups: Box<[GroupParams]>,
    zones: Box<[ZoneParams]>,
    sources: Box<[Source]>,
    /// Zones in play order (ascending index), `len` of them.
    order: Box<[u16]>,
    order_len: usize,
    layers: Box<[Layer]>,
    slots: [Slot; MAX_VOICES],
    alloc: Allocator,
    matrix: ModMatrix,
    /// Global controller values: mw, at, pb (semitones), expr, sus.
    mw: f32,
    at: f32,
    bend: f32,
    expr: f32,
    sus: f32,
    keyswitch: i16,
    /// Per-key counters for live notes' round robin.
    live_rr: [u32; 128],
    clock: u64,
    serial: u64,
    stolen: u32,
    /// Keys held down (for the first and legato triggers).
    keys_down: u32,
    released_buf: [usize; MAX_VOICES],
    peak: f32,
    /// The parallel render (ADR 0038): a lane per sounding layer, whether each lane's layer
    /// ended in the span, and the layers that ended this block (told to the allocator in
    /// layer order at its end).
    lanes: crate::par::Lanes,
    lane_end: [u16; LANES],
    ended: Box<[bool]>,
}

/// The longest span one fork of the parallel render covers, in frames.
const SPAN: usize = 128;
/// Layers the parallel render gives a lane; with more sounding it renders serially.
const LANES: usize = 64;

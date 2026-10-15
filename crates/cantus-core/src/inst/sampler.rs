//! Exemplar — the multisample sampler: its parameter model (ADR 0023,
//! `docs/instruments/exemplar.md`, `research/instruments/sampler.md` §9).
//!
//! An Exemplar instrument is **groups → zones → samples**, stored flat in its
//! [`NativeSpec`](super::NativeSpec) like every native instrument, so an edit is a command
//! with undo, and an import is one batch of them:
//!
//! | Keys | What |
//! |---|---|
//! | `gain`, `pan`, `tune`, `amp.*`, `filter.*`, `lfo*`, `env2.*` … | the instrument ([`PARAMS`]) |
//! | `g{m}.*` (m < [`MAX_GROUPS`]) | a group: level, pan, tune, its envelope when it overrides the instrument's ([`GROUP_PARAMS`]) |
//! | `z{n}.*` (n < [`MAX_ZONES`]) | a zone: key and velocity ranges, root, tune, gain, pan, start/end, loop, crossfades, triggers, round robin ([`ZONE_PARAMS`]) |
//! | `z{n}.sample` (text) | the zone's audio file — the asset key the engine decodes |
//! | `z{n}.gen` | or one of the built-in sounds, when the zone has no file |
//!
//! [`param`] matches the `g{m}.` and `z{n}.` patterns rather than listing every index. A
//! zone exists when it has a file or a built-in sound; its other keys are only stored when
//! they differ from their defaults. A group's envelope keys **inherit**: stored, they
//! override the instrument's; cleared, the group follows the instrument again.
//!
//! Keys are forever once shipped: projects, presets and automation hold them.

use super::{find, InstParamInfo};
use crate::effect::p;
use crate::model::TrackIcon;

pub const NAME_KEY: &str = "instrument.sampler";
pub const ICON: TrackIcon = TrackIcon::Keys;
/// Offered in the instrument picker.
pub const READY: bool = true;

/// Zones an instrument can hold (`z0` … `z1023`).
pub const MAX_ZONES: usize = 1024;
/// Groups an instrument can hold (`g0` … `g63`).
pub const MAX_GROUPS: usize = 64;
/// Round-robin positions a zone can name.
pub const MAX_SEQ: usize = 16;

/// The instrument's own numbers: what the generic editors, automation and the modulation
/// matrix list.
pub const PARAMS: &[InstParamInfo] = &[
    p("gain", "inst.gain", -48.0, 12.0, 0.0, "dB", false, false),
    p("pan", "smp.pan", -100.0, 100.0, 0.0, "%", false, false),
    p(
        "transpose",
        "smp.transpose",
        -48.0,
        48.0,
        0.0,
        "st",
        false,
        true,
    ),
    p("tune", "smp.tune", -100.0, 100.0, 0.0, "ct", false, false),
    p(
        "vel.amp",
        "smp.vel_amp",
        -100.0,
        100.0,
        100.0,
        "%",
        false,
        false,
    ),
    p("voices", "smp.voices", 1.0, 64.0, 32.0, "", false, true),
    p("mode", "smp.mode", 0.0, 2.0, 0.0, "", false, true),
    p("glide", "smp.glide", 0.0, 5000.0, 0.0, "ms", false, false),
    p("bend", "smp.bend", 0.0, 24.0, 2.0, "st", false, true),
    p("quality", "smp.quality", 0.0, 3.0, 2.0, "", false, true),
    // Round robin: 1 in order (a sequenced note's turn is its ordinal on the track: no
    // sample twice in a row, the same turns from any start), 0 seeded from the note's id
    // (every Exemplar saved before round 28 — schema 28's migration writes it). Round 28.
    p("rr.order", "smp.rr_order", 0.0, 1.0, 1.0, "", false, true),
    // A drum kit (round 28): the piano roll and the channel rack show a row per drum — every
    // key a zone plays, named by its group — instead of a keyboard ([`kit_lanes`]).
    p("kit", "smp.kit", 0.0, 1.0, 0.0, "", false, true),
    p(
        "amp.attack",
        "smp.attack",
        0.1,
        20_000.0,
        0.1,
        "ms",
        true,
        false,
    ),
    p(
        "amp.hold", "smp.hold", 0.1, 20_000.0, 0.1, "ms", true, false,
    ),
    p(
        "amp.decay",
        "smp.decay",
        1.0,
        30_000.0,
        1_000.0,
        "ms",
        true,
        false,
    ),
    p(
        "amp.sustain",
        "smp.sustain",
        0.0,
        100.0,
        100.0,
        "%",
        false,
        false,
    ),
    p(
        "amp.release",
        "smp.release",
        1.0,
        30_000.0,
        250.0,
        "ms",
        true,
        false,
    ),
    p(
        "filter.type",
        "smp.filter_type",
        0.0,
        3.0,
        0.0,
        "",
        false,
        true,
    ),
    p(
        "filter.cutoff",
        "smp.cutoff",
        20.0,
        20_000.0,
        20_000.0,
        "Hz",
        true,
        false,
    ),
    p(
        "filter.reso",
        "smp.reso",
        0.0,
        100.0,
        0.0,
        "%",
        false,
        false,
    ),
    p(
        "filter.key",
        "smp.filter_key",
        0.0,
        100.0,
        0.0,
        "%",
        false,
        false,
    ),
    p(
        "filter.vel",
        "smp.filter_vel",
        0.0,
        96.0,
        0.0,
        "st",
        false,
        false,
    ),
    p(
        "filter.env",
        "smp.filter_env",
        -96.0,
        96.0,
        0.0,
        "st",
        false,
        false,
    ),
    p(
        "env2.attack",
        "smp.env2_attack",
        0.1,
        20_000.0,
        0.1,
        "ms",
        true,
        false,
    ),
    p(
        "env2.decay",
        "smp.env2_decay",
        1.0,
        30_000.0,
        500.0,
        "ms",
        true,
        false,
    ),
    p(
        "env2.sustain",
        "smp.env2_sustain",
        0.0,
        100.0,
        0.0,
        "%",
        false,
        false,
    ),
    p(
        "env2.release",
        "smp.env2_release",
        1.0,
        30_000.0,
        250.0,
        "ms",
        true,
        false,
    ),
    p(
        "lfo1.rate",
        "smp.lfo1_rate",
        0.01,
        40.0,
        5.0,
        "Hz",
        true,
        false,
    ),
    p(
        "lfo1.shape",
        "smp.lfo_shape",
        0.0,
        3.0,
        0.0,
        "",
        false,
        true,
    ),
    p(
        "lfo1.delay",
        "smp.lfo_delay",
        0.0,
        5_000.0,
        0.0,
        "ms",
        false,
        false,
    ),
    p(
        "lfo1.pitch",
        "smp.vibrato",
        0.0,
        100.0,
        0.0,
        "ct",
        false,
        false,
    ),
    p(
        "lfo2.rate",
        "smp.lfo2_rate",
        0.01,
        40.0,
        0.5,
        "Hz",
        true,
        false,
    ),
    p(
        "lfo2.shape",
        "smp.lfo_shape",
        0.0,
        3.0,
        1.0,
        "",
        false,
        true,
    ),
    p(
        "start.vel",
        "smp.start_vel",
        0.0,
        1_000.0,
        0.0,
        "ms",
        false,
        false,
    ),
    p("sw.lo", "smp.sw_lo", -1.0, 127.0, -1.0, "", false, true),
    p("sw.hi", "smp.sw_hi", -1.0, 127.0, -1.0, "", false, true),
    p(
        "sw.default",
        "smp.sw_default",
        -1.0,
        127.0,
        -1.0,
        "",
        false,
        true,
    ),
];

/// A group's numbers, as `g{m}.<field>`. The key printed here is the template `g.<field>`.
pub const GROUP_PARAMS: &[InstParamInfo] = &[
    p("g.gain", "smp.gain", -48.0, 24.0, 0.0, "dB", false, false),
    p("g.pan", "smp.pan", -100.0, 100.0, 0.0, "%", false, false),
    p(
        "g.tune", "smp.tune", -4_800.0, 4_800.0, 0.0, "ct", false, false,
    ),
    p(
        "g.attack",
        "smp.attack",
        0.1,
        20_000.0,
        0.1,
        "ms",
        true,
        false,
    ),
    p("g.hold", "smp.hold", 0.1, 20_000.0, 0.1, "ms", true, false),
    p(
        "g.decay",
        "smp.decay",
        1.0,
        30_000.0,
        1_000.0,
        "ms",
        true,
        false,
    ),
    p(
        "g.sustain",
        "smp.sustain",
        0.0,
        100.0,
        100.0,
        "%",
        false,
        false,
    ),
    p(
        "g.release",
        "smp.release",
        1.0,
        30_000.0,
        250.0,
        "ms",
        true,
        false,
    ),
    p(
        "g.vel",
        "smp.vel_amp",
        -100.0,
        100.0,
        100.0,
        "%",
        false,
        false,
    ),
    p("g.poly", "smp.group_poly", 0.0, 64.0, 0.0, "", false, true),
    p("g.mute", "smp.mute", 0.0, 1.0, 0.0, "", false, true),
];

/// A zone's numbers, as `z{n}.<field>`. The key printed here is the template `z.<field>`.
/// Frames (`start`, `end`, `ls`, `le`) are exact integers up to 2²⁴ in an `f32`.
pub const ZONE_PARAMS: &[InstParamInfo] = &[
    p("z.gen", "smp.gen", 0.0, 32.0, 0.0, "", false, true),
    p("z.group", "smp.group", 0.0, 63.0, 0.0, "", false, true),
    p("z.lo", "smp.lo", 0.0, 127.0, 0.0, "", false, true),
    p("z.hi", "smp.hi", 0.0, 127.0, 127.0, "", false, true),
    p("z.root", "smp.root", 0.0, 127.0, 60.0, "", false, true),
    p("z.vlo", "smp.vlo", 1.0, 127.0, 1.0, "", false, true),
    p("z.vhi", "smp.vhi", 1.0, 127.0, 127.0, "", false, true),
    p(
        "z.tune", "smp.tune", -4_800.0, 4_800.0, 0.0, "ct", false, false,
    ),
    p("z.gain", "smp.gain", -48.0, 24.0, 0.0, "dB", false, false),
    p("z.pan", "smp.pan", -100.0, 100.0, 0.0, "%", false, false),
    p(
        "z.key",
        "smp.keytrack",
        0.0,
        1_200.0,
        100.0,
        "ct",
        false,
        false,
    ),
    p(
        "z.start",
        "smp.start",
        0.0,
        16_777_215.0,
        0.0,
        "",
        false,
        true,
    ),
    p("z.end", "smp.end", 0.0, 16_777_215.0, 0.0, "", false, true),
    p("z.mode", "smp.loop_mode", 0.0, 3.0, 0.0, "", false, true),
    p("z.dir", "smp.loop_dir", 0.0, 2.0, 0.0, "", false, true),
    p(
        "z.ls",
        "smp.loop_start",
        0.0,
        16_777_215.0,
        0.0,
        "",
        false,
        true,
    ),
    p(
        "z.le",
        "smp.loop_end",
        0.0,
        16_777_215.0,
        0.0,
        "",
        false,
        true,
    ),
    p("z.xf", "smp.loop_xf", 0.0, 1_000.0, 0.0, "ms", false, false),
    p("z.lxc", "smp.loop_xf_curve", 0.0, 1.0, 0.0, "", false, true),
    p("z.rev", "smp.reverse", 0.0, 1.0, 0.0, "", false, true),
    p("z.vfin", "smp.vfin", 0.0, 127.0, 0.0, "", false, true),
    p("z.vfout", "smp.vfout", 0.0, 127.0, 0.0, "", false, true),
    p("z.kfin", "smp.kfin", 0.0, 127.0, 0.0, "", false, true),
    p("z.kfout", "smp.kfout", 0.0, 127.0, 0.0, "", false, true),
    p("z.fc", "smp.fade_curve", 0.0, 1.0, 0.0, "", false, true),
    p("z.seq", "smp.seq", 1.0, 16.0, 1.0, "", false, true),
    p("z.seqlen", "smp.seqlen", 1.0, 16.0, 1.0, "", false, true),
    p("z.rlo", "smp.rlo", 0.0, 1.0, 0.0, "", false, false),
    p("z.rhi", "smp.rhi", 0.0, 1.0, 1.0, "", false, false),
    p("z.trig", "smp.trigger", 0.0, 5.0, 0.0, "", false, true),
    p(
        "z.rtdecay",
        "smp.rt_decay",
        0.0,
        100.0,
        0.0,
        "dB/s",
        false,
        false,
    ),
    p(
        "z.rtfollow",
        "smp.rt_follow",
        0.0,
        1.0,
        0.0,
        "",
        false,
        true,
    ),
    p("z.excl", "smp.excl", 0.0, 64.0, 0.0, "", false, true),
    p("z.offby", "smp.offby", 0.0, 64.0, 0.0, "", false, true),
    p("z.offmode", "smp.offmode", 0.0, 1.0, 0.0, "", false, true),
    p("z.sw", "smp.sw", -1.0, 127.0, -1.0, "", false, true),
    p(
        "z.delay",
        "smp.delay",
        0.0,
        10_000.0,
        0.0,
        "ms",
        false,
        false,
    ),
    // A controller-triggered zone (`trig` = [`Trigger::Controller`]): the controller and
    // the range its value must cross into (round 26, ADR 0023's amendment).
    p("z.cc", "smp.cc", 0.0, 127.0, 64.0, "", false, true),
    p("z.cclo", "smp.cclo", 0.0, 127.0, 64.0, "", false, true),
    p("z.cchi", "smp.cchi", 0.0, 127.0, 127.0, "", false, true),
];

pub fn params() -> &'static [InstParamInfo] {
    PARAMS
}

/// The modulation sources it produces (tags from `modulation::SOURCES`).
pub const SOURCES: &[&str] = &[
    "env1", "env2", "lfo1", "lfo2", "vel", "relvel", "key", "rnd", "macro1", "macro2", "macro3",
    "macro4", "macro5", "macro6", "macro7", "macro8", "mw", "at", "pb", "expr", "sus",
];

/// Split `z{n}.field` (or `g{m}.field`, with `prefix` `b'g'`) into its index and field.
/// Allocation-free: the engine calls it on the audio thread.
#[inline]
pub fn split_indexed(key: &str, prefix: u8, max: usize) -> Option<(usize, &str)> {
    let b = key.as_bytes();
    if b.len() < 3 || b[0] != prefix || !b[1].is_ascii_digit() {
        return None;
    }
    let mut i = 1;
    let mut n = 0usize;
    while i < b.len() && b[i].is_ascii_digit() {
        n = n.checked_mul(10)?.checked_add((b[i] - b'0') as usize)?;
        i += 1;
        if i - 1 > 5 {
            return None;
        }
    }
    // No leading zeros ("z01"), so one index has one spelling.
    if i > 2 && b[1] == b'0' {
        return None;
    }
    if i >= b.len() || b[i] != b'.' || n >= max {
        return None;
    }
    Some((n, &key[i + 1..]))
}

/// `z{n}.field` → `(n, field)`.
#[inline]
pub fn zone_key(key: &str) -> Option<(usize, &str)> {
    split_indexed(key, b'z', MAX_ZONES)
}

/// `g{m}.field` → `(m, field)`.
#[inline]
pub fn group_key(key: &str) -> Option<(usize, &str)> {
    split_indexed(key, b'g', MAX_GROUPS)
}

fn template(list: &'static [InstParamInfo], field: &str) -> Option<&'static InstParamInfo> {
    list.iter()
        .find(|i| i.key.len() == field.len() + 2 && &i.key[2..] == field)
}

pub fn param(key: &str) -> Option<&'static InstParamInfo> {
    if let Some((_, field)) = zone_key(key) {
        return template(ZONE_PARAMS, field);
    }
    if let Some((_, field)) = group_key(key) {
        return template(GROUP_PARAMS, field);
    }
    find(PARAMS, key).or_else(|| super::modulation::param(key))
}

pub fn is_text_key(key: &str) -> bool {
    super::modulation::is_text_key(key)
        || is_asset_key(key)
        || matches!(zone_key(key), Some((_, "name")))
        || matches!(group_key(key), Some((_, "name")))
}

pub fn is_asset_key(key: &str) -> bool {
    matches!(zone_key(key), Some((_, "sample")))
}

// ---- fields by index -------------------------------------------------------------------

/// Index of each zone field in [`ZONE_PARAMS`] (and in [`Zone::values`]).
pub mod zf {
    pub const GEN: usize = 0;
    pub const GROUP: usize = 1;
    pub const LO: usize = 2;
    pub const HI: usize = 3;
    pub const ROOT: usize = 4;
    pub const VLO: usize = 5;
    pub const VHI: usize = 6;
    pub const TUNE: usize = 7;
    pub const GAIN: usize = 8;
    pub const PAN: usize = 9;
    pub const KEYTRACK: usize = 10;
    pub const START: usize = 11;
    pub const END: usize = 12;
    pub const MODE: usize = 13;
    pub const DIR: usize = 14;
    pub const LS: usize = 15;
    pub const LE: usize = 16;
    pub const XF: usize = 17;
    pub const LXC: usize = 18;
    pub const REV: usize = 19;
    pub const VFIN: usize = 20;
    pub const VFOUT: usize = 21;
    pub const KFIN: usize = 22;
    pub const KFOUT: usize = 23;
    pub const FC: usize = 24;
    pub const SEQ: usize = 25;
    pub const SEQLEN: usize = 26;
    pub const RLO: usize = 27;
    pub const RHI: usize = 28;
    pub const TRIG: usize = 29;
    pub const RTDECAY: usize = 30;
    pub const RTFOLLOW: usize = 31;
    pub const EXCL: usize = 32;
    pub const OFFBY: usize = 33;
    pub const OFFMODE: usize = 34;
    pub const SW: usize = 35;
    pub const DELAY: usize = 36;
    pub const CC: usize = 37;
    pub const CCLO: usize = 38;
    pub const CCHI: usize = 39;
    pub const COUNT: usize = 40;
}

/// Index of each group field in [`GROUP_PARAMS`].
pub mod gf {
    pub const GAIN: usize = 0;
    pub const PAN: usize = 1;
    pub const TUNE: usize = 2;
    pub const ATTACK: usize = 3;
    pub const HOLD: usize = 4;
    pub const DECAY: usize = 5;
    pub const SUSTAIN: usize = 6;
    pub const RELEASE: usize = 7;
    pub const VEL: usize = 8;
    pub const POLY: usize = 9;
    pub const MUTE: usize = 10;
    pub const COUNT: usize = 11;
}

/// A zone field's index from its name (`"lo"` → [`zf::LO`]). A `match`, so the engine's
/// `configure` can read thousands of keys without a search.
#[inline]
pub fn zone_field(field: &str) -> Option<usize> {
    Some(match field {
        "gen" => zf::GEN,
        "group" => zf::GROUP,
        "lo" => zf::LO,
        "hi" => zf::HI,
        "root" => zf::ROOT,
        "vlo" => zf::VLO,
        "vhi" => zf::VHI,
        "tune" => zf::TUNE,
        "gain" => zf::GAIN,
        "pan" => zf::PAN,
        "key" => zf::KEYTRACK,
        "start" => zf::START,
        "end" => zf::END,
        "mode" => zf::MODE,
        "dir" => zf::DIR,
        "ls" => zf::LS,
        "le" => zf::LE,
        "xf" => zf::XF,
        "lxc" => zf::LXC,
        "rev" => zf::REV,
        "vfin" => zf::VFIN,
        "vfout" => zf::VFOUT,
        "kfin" => zf::KFIN,
        "kfout" => zf::KFOUT,
        "fc" => zf::FC,
        "seq" => zf::SEQ,
        "seqlen" => zf::SEQLEN,
        "rlo" => zf::RLO,
        "rhi" => zf::RHI,
        "trig" => zf::TRIG,
        "rtdecay" => zf::RTDECAY,
        "rtfollow" => zf::RTFOLLOW,
        "excl" => zf::EXCL,
        "offby" => zf::OFFBY,
        "offmode" => zf::OFFMODE,
        "sw" => zf::SW,
        "delay" => zf::DELAY,
        "cc" => zf::CC,
        "cclo" => zf::CCLO,
        "cchi" => zf::CCHI,
        _ => return None,
    })
}

/// A group field's index from its name.
#[inline]
pub fn group_field(field: &str) -> Option<usize> {
    Some(match field {
        "gain" => gf::GAIN,
        "pan" => gf::PAN,
        "tune" => gf::TUNE,
        "attack" => gf::ATTACK,
        "hold" => gf::HOLD,
        "decay" => gf::DECAY,
        "sustain" => gf::SUSTAIN,
        "release" => gf::RELEASE,
        "vel" => gf::VEL,
        "poly" => gf::POLY,
        "mute" => gf::MUTE,
        _ => return None,
    })
}

/// The zone field's name (`"lo"`), from [`ZONE_PARAMS`].
pub fn zone_field_name(i: usize) -> &'static str {
    &ZONE_PARAMS[i].key[2..]
}

/// The group field's name (`"attack"`), from [`GROUP_PARAMS`].
pub fn group_field_name(i: usize) -> &'static str {
    &GROUP_PARAMS[i].key[2..]
}

/// Loop and playback modes (`z{n}.mode`).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum LoopMode {
    /// Plays to its end or until the release ends (SFZ `no_loop`).
    #[default]
    NoLoop,
    /// Plays to its end whatever the note does (SFZ `one_shot`).
    OneShot,
    /// Loops for as long as it sounds, through the release (SFZ `loop_continuous`).
    Continuous,
    /// Loops while the key is held, then plays on to the end (SFZ `loop_sustain`).
    Sustain,
}

impl LoopMode {
    pub fn from_index(v: f32) -> LoopMode {
        match v.round() as i32 {
            1 => LoopMode::OneShot,
            2 => LoopMode::Continuous,
            3 => LoopMode::Sustain,
            _ => LoopMode::NoLoop,
        }
    }
    pub fn loops(self) -> bool {
        matches!(self, LoopMode::Continuous | LoopMode::Sustain)
    }
}

/// When a zone starts (`z{n}.trig`).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum Trigger {
    /// On the note-on.
    #[default]
    Attack,
    /// On the note-off (or when the pedal lets the note go), SFZ `trigger=release`.
    Release,
    /// On a note-on when no other note is held.
    First,
    /// On a note-on while another note is held.
    Legato,
    /// On the key going up, even with the pedal down (SFZ `release_key`).
    ReleaseKey,
    /// When controller `z{n}.cc` crosses into `cclo..=cchi` (SFZ `on_loccN`/`on_hiccN`:
    /// a piano's pedal noises), whatever the keys do; released when it leaves the range.
    Controller,
}

impl Trigger {
    pub fn from_index(v: f32) -> Trigger {
        match v.round() as i32 {
            1 => Trigger::Release,
            2 => Trigger::First,
            3 => Trigger::Legato,
            4 => Trigger::ReleaseKey,
            5 => Trigger::Controller,
            _ => Trigger::Attack,
        }
    }
}

// ---- zones and groups as values (the editor's and the importers' view) -----------------

/// One zone, read out of a spec or about to be written into one. `values` holds every
/// numeric field by [`zf`] index; `sample` is the file, `name` an optional label.
#[derive(Clone, Debug, PartialEq)]
pub struct Zone {
    pub values: [f32; zf::COUNT],
    pub sample: Option<String>,
    pub name: Option<String>,
}

impl Default for Zone {
    fn default() -> Self {
        let mut values = [0.0; zf::COUNT];
        for (i, v) in values.iter_mut().enumerate() {
            *v = ZONE_PARAMS[i].default;
        }
        Zone {
            values,
            sample: None,
            name: None,
        }
    }
}

impl Zone {
    /// A zone playing `sample` over `lo..=hi`, rooted at `root`.
    pub fn new(sample: Option<String>, lo: u8, hi: u8, root: u8) -> Zone {
        let mut z = Zone {
            sample,
            ..Zone::default()
        };
        z.set(zf::LO, lo as f32);
        z.set(zf::HI, hi as f32);
        z.set(zf::ROOT, root as f32);
        z
    }

    pub fn get(&self, field: usize) -> f32 {
        self.values[field]
    }

    /// Set a field, clamped and rounded as the spec would store it.
    pub fn set(&mut self, field: usize, v: f32) -> &mut Self {
        let info = &ZONE_PARAMS[field];
        let v = if v.is_finite() {
            v.clamp(info.min, info.max)
        } else {
            info.default
        };
        self.values[field] = if info.stepped { v.round() } else { v };
        self
    }

    pub fn lo(&self) -> u8 {
        self.values[zf::LO] as u8
    }
    pub fn hi(&self) -> u8 {
        self.values[zf::HI] as u8
    }
    pub fn root(&self) -> u8 {
        self.values[zf::ROOT] as u8
    }
    pub fn vlo(&self) -> u8 {
        self.values[zf::VLO] as u8
    }
    pub fn vhi(&self) -> u8 {
        self.values[zf::VHI] as u8
    }

    /// Whether the zone makes sound at all: a file or a built-in.
    pub fn has_source(&self) -> bool {
        self.sample.is_some() || self.values[zf::GEN] > 0.0
    }

    /// The `(key, value)` pairs zone `n` stores: only the fields away from their default.
    pub fn values_for(&self, n: usize) -> Vec<(String, f32)> {
        (0..zf::COUNT)
            .filter(|i| self.values[*i] != ZONE_PARAMS[*i].default)
            .map(|i| (format!("z{n}.{}", zone_field_name(i)), self.values[i]))
            .collect()
    }

    /// The `(key, text)` pairs zone `n` stores.
    pub fn texts_for(&self, n: usize) -> Vec<(String, String)> {
        let mut out = Vec::new();
        if let Some(s) = &self.sample {
            out.push((format!("z{n}.sample"), s.clone()));
        }
        if let Some(s) = &self.name {
            out.push((format!("z{n}.name"), s.clone()));
        }
        out
    }

    /// Zone `n` of `spec`, or `None` when it has no source.
    pub fn of(spec: &super::NativeSpec, n: usize) -> Option<Zone> {
        let mut z = Zone {
            sample: spec.text(&format!("z{n}.sample")).map(str::to_string),
            name: spec.text(&format!("z{n}.name")).map(str::to_string),
            ..Zone::default()
        };
        for i in 0..zf::COUNT {
            if let Some(v) = spec.params.get(&format!("z{n}.{}", zone_field_name(i))) {
                z.values[i] = *v;
            }
        }
        z.has_source().then_some(z)
    }
}

/// Every zone of a spec, by index, in index order. One pass over the stored keys.
pub fn zones(spec: &super::NativeSpec) -> Vec<(usize, Zone)> {
    let mut map: std::collections::BTreeMap<usize, Zone> = std::collections::BTreeMap::new();
    for (k, v) in &spec.params {
        if let Some((n, field)) = zone_key(k) {
            if let Some(i) = zone_field(field) {
                map.entry(n).or_default().values[i] = *v;
            }
        }
    }
    for (k, t) in &spec.text {
        match zone_key(k) {
            Some((n, "sample")) => map.entry(n).or_default().sample = Some(t.clone()),
            Some((n, "name")) => map.entry(n).or_default().name = Some(t.clone()),
            _ => {}
        }
    }
    map.into_iter().filter(|(_, z)| z.has_source()).collect()
}

/// The first zone index nothing uses.
pub fn free_zone(spec: &super::NativeSpec) -> Option<usize> {
    let used: std::collections::BTreeSet<usize> = spec
        .params
        .keys()
        .chain(spec.text.keys())
        .filter_map(|k| zone_key(k).map(|(n, _)| n))
        .collect();
    (0..MAX_ZONES).find(|n| !used.contains(n))
}

/// Every key (numeric and text) zone `n` has stored — what removing it clears.
pub fn zone_keys(spec: &super::NativeSpec, n: usize) -> (Vec<String>, Vec<String>) {
    let mine = |k: &&String| zone_key(k).is_some_and(|(i, _)| i == n);
    (
        spec.params.keys().filter(mine).cloned().collect(),
        spec.text.keys().filter(mine).cloned().collect(),
    )
}

/// A group's numbers: `None` where the group stores nothing (so it follows the
/// instrument, for the envelope, or the default, for the rest).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Group {
    pub values: [Option<f32>; gf::COUNT],
    pub name: Option<String>,
}

impl Group {
    pub fn of(spec: &super::NativeSpec, m: usize) -> Group {
        let mut g = Group {
            name: spec.text(&format!("g{m}.name")).map(str::to_string),
            ..Group::default()
        };
        for i in 0..gf::COUNT {
            g.values[i] = spec
                .params
                .get(&format!("g{m}.{}", group_field_name(i)))
                .copied();
        }
        g
    }

    pub fn values_for(&self, m: usize) -> Vec<(String, f32)> {
        (0..gf::COUNT)
            .filter_map(|i| self.values[i].map(|v| (format!("g{m}.{}", group_field_name(i)), v)))
            .collect()
    }
}

/// Semitones a zone can be played above its root at most: its top key, its tune, the
/// instrument's transpose and bend range, and `margin` for modulation. What sizes the
/// octave mip-map the engine builds for its sample.
pub fn max_shift_semitones(spec: &super::NativeSpec, zone: &Zone, margin: f32) -> f32 {
    max_shift_of(spec, &zone.values, margin)
}

/// [`max_shift_semitones`] from a zone's numbers (allocation-free: the engine's
/// `configure` sizes a streamed zone's mip levels with it, as the sample cache sizes an
/// in-memory one's).
pub fn max_shift_of(spec: &super::NativeSpec, v: &[f32; zf::COUNT], margin: f32) -> f32 {
    let (hi, root) = (v[zf::HI] as u8, v[zf::ROOT] as u8);
    let keys = (hi as f32 - root as f32).max(0.0) * v[zf::KEYTRACK] / 100.0;
    let tune = (v[zf::TUNE] + spec.get("tune").max(0.0)) / 100.0;
    keys + tune + spec.get("transpose").max(0.0) + spec.get("bend") + margin
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inst::{InstrumentKind, NativeSpec};

    #[test]
    fn indexed_keys_parse_one_spelling_only() {
        assert_eq!(zone_key("z0.lo"), Some((0, "lo")));
        assert_eq!(zone_key("z1023.sample"), Some((1023, "sample")));
        assert_eq!(zone_key("z1024.lo"), None, "past the last zone");
        assert_eq!(zone_key("z01.lo"), None, "no leading zeros");
        assert_eq!(zone_key("z.lo"), None);
        assert_eq!(zone_key("zz1.lo"), None);
        assert_eq!(zone_key("z1lo"), None);
        assert_eq!(zone_key("z99999999999999999999.lo"), None);
        assert_eq!(group_key("g63.attack"), Some((63, "attack")));
        assert_eq!(group_key("g64.attack"), None);
        assert_eq!(zone_key("gain"), None);
    }

    #[test]
    fn patterns_resolve_to_their_templates() {
        let k = InstrumentKind::Sampler;
        assert_eq!(k.param("z7.root").map(|i| i.key), Some("z.root"));
        assert_eq!(k.param("g3.release").map(|i| i.key), Some("g.release"));
        assert!(k.param("z7.nonsense").is_none());
        assert!(k.param("gain").is_some() && k.param("mod3.amt").is_some());
        assert!(k.is_asset_key("z12.sample") && k.is_text_key("z12.sample"));
        assert!(k.is_text_key("z12.name") && k.is_text_key("g2.name"));
        assert!(!k.is_asset_key("z12.name") && !k.is_asset_key("sample"));
        assert!(!k.is_text_key("z12.lo"));
        for (i, info) in ZONE_PARAMS.iter().enumerate() {
            assert_eq!(zone_field(zone_field_name(i)), Some(i), "{}", info.key);
            assert!(info.min <= info.default && info.default <= info.max);
        }
        for (i, info) in GROUP_PARAMS.iter().enumerate() {
            assert_eq!(group_field(group_field_name(i)), Some(i), "{}", info.key);
        }
        assert_eq!(ZONE_PARAMS.len(), zf::COUNT);
        assert_eq!(GROUP_PARAMS.len(), gf::COUNT);
    }

    #[test]
    fn a_zone_round_trips_through_a_spec_storing_only_what_moved() {
        let mut spec = NativeSpec::new(InstrumentKind::Sampler);
        let mut z = Zone::new(Some("C:/s/a.wav".into()), 48, 59, 55);
        z.set(zf::VHI, 90.0).set(zf::TUNE, -12.5).set(zf::MODE, 3.0);
        let values = z.values_for(4);
        assert_eq!(values.len(), 6, "{values:?}");
        for (k, v) in values {
            assert_eq!(spec.set(&k, v), Some(v));
        }
        for (k, t) in z.texts_for(4) {
            spec.set_text(&k, Some(t)).unwrap();
        }
        assert_eq!(Zone::of(&spec, 4), Some(z.clone()));
        assert_eq!(zones(&spec), vec![(4, z)]);
        assert_eq!(free_zone(&spec), Some(0));
        let (nums, texts) = zone_keys(&spec, 4);
        assert_eq!((nums.len(), texts.len()), (6, 1));
        assert!(Zone::of(&spec, 5).is_none());
        assert_eq!(spec.assets().count(), 1);
    }

    #[test]
    fn frames_are_exact_integers_and_ranges_clamp() {
        let mut spec = NativeSpec::new(InstrumentKind::Sampler);
        assert_eq!(spec.set("z0.le", 16_777_215.0), Some(16_777_215.0));
        assert_eq!(spec.set("z0.ls", 123_456.4), Some(123_456.0));
        assert_eq!(spec.set("z0.hi", 300.0), Some(127.0));
        assert_eq!(
            spec.get("z9.root"),
            60.0,
            "an unstored zone key reads its default"
        );
        assert_eq!(spec.get("g1.release"), 250.0);
    }

    #[test]
    fn a_group_reads_none_where_it_follows_the_instrument() {
        let mut spec = NativeSpec::new(InstrumentKind::Sampler);
        spec.set("g2.release", 40.0);
        let g = Group::of(&spec, 2);
        assert_eq!(g.values[gf::RELEASE], Some(40.0));
        assert_eq!(g.values[gf::ATTACK], None);
        assert_eq!(g.values_for(2), vec![("g2.release".to_string(), 40.0)]);
    }
}

/// General MIDI's name for a percussion key (35 … 81), as the GM 1 sound set lists them.
pub fn gm_drum_name(key: u8) -> Option<&'static str> {
    const NAMES: [&str; 47] = [
        "Acoustic Bass Drum",
        "Bass Drum 1",
        "Side Stick",
        "Acoustic Snare",
        "Hand Clap",
        "Electric Snare",
        "Low Floor Tom",
        "Closed Hi-Hat",
        "High Floor Tom",
        "Pedal Hi-Hat",
        "Low Tom",
        "Open Hi-Hat",
        "Low-Mid Tom",
        "Hi-Mid Tom",
        "Crash Cymbal 1",
        "High Tom",
        "Ride Cymbal 1",
        "Chinese Cymbal",
        "Ride Bell",
        "Tambourine",
        "Splash Cymbal",
        "Cowbell",
        "Crash Cymbal 2",
        "Vibraslap",
        "Ride Cymbal 2",
        "Hi Bongo",
        "Low Bongo",
        "Mute Hi Conga",
        "Open Hi Conga",
        "Low Conga",
        "High Timbale",
        "Low Timbale",
        "High Agogo",
        "Low Agogo",
        "Cabasa",
        "Maracas",
        "Short Whistle",
        "Long Whistle",
        "Short Guiro",
        "Long Guiro",
        "Claves",
        "Hi Wood Block",
        "Low Wood Block",
        "Mute Cuica",
        "Open Cuica",
        "Mute Triangle",
        "Open Triangle",
    ];
    key.checked_sub(35)
        .and_then(|i| NAMES.get(i as usize))
        .copied()
}

/// Whether an Exemplar is a drum kit (`kit` = 1, round 28): its editors show a row per
/// drum rather than a keyboard.
pub fn is_kit(spec: &super::NativeSpec) -> bool {
    spec.get("kit") >= 0.5
}

/// A kit's rows: every key a zone plays (zones a controller starts have none), ascending,
/// each named by the first zone's group name on it, else by General MIDI, else as a pitch.
/// One pass over the stored keys, no map: the editors ask every frame.
pub fn kit_lanes(spec: &super::NativeSpec) -> Vec<(crate::model::Pitch, String)> {
    // Per key: the group of the first zone playing it (+1; 0: no zone).
    let mut group_on = [0u8; 128];
    let defaults = |i: usize| ZONE_PARAMS[i].default;
    // The stored keys of a zone are next to each other in the map ("z1." sorts before
    // "z10."): gather each zone's range, group and trigger as its keys go by.
    let mut cur: Option<usize> = None;
    let mut v = [0f32; 4];
    let mut name = String::new();
    let mut flush = |n: usize, v: &[f32; 4], name: &mut String| {
        use std::fmt::Write;
        name.clear();
        let _ = write!(name, "z{n}.sample");
        let sourced = spec.text.contains_key(name.as_str());
        let generated = {
            name.clear();
            let _ = write!(name, "z{n}.gen");
            spec.params.get(name.as_str()).is_some_and(|g| *g > 0.0)
        };
        if !(sourced || generated) || v[3] as usize == 5 {
            return;
        }
        let (lo, hi) = (
            v[0].clamp(0.0, 127.0) as usize,
            v[1].clamp(0.0, 127.0) as usize,
        );
        for k in lo..=hi {
            if group_on[k] == 0 {
                group_on[k] = (v[2].clamp(0.0, 254.0) as u8) + 1;
            }
        }
    };
    let fresh = || {
        [
            defaults(zf::LO),
            defaults(zf::HI),
            defaults(zf::GROUP),
            defaults(zf::TRIG),
        ]
    };
    for (k, val) in &spec.params {
        let Some((n, field)) = zone_key(k) else {
            continue;
        };
        if cur != Some(n) {
            if let Some(c) = cur {
                flush(c, &v, &mut name);
            }
            cur = Some(n);
            v = fresh();
        }
        match field {
            "lo" => v[0] = *val,
            "hi" => v[1] = *val,
            "group" => v[2] = *val,
            "trig" => v[3] = *val,
            _ => {}
        }
    }
    if let Some(c) = cur {
        flush(c, &v, &mut name);
    }
    // A zone with only its sample stored (every number at its default) has no number key:
    // it plays the whole keyboard.
    for (k, _) in &spec.text {
        if let Some((n, "sample")) = zone_key(k) {
            name.clear();
            let any = spec
                .params
                .range(format!("z{n}.")..format!("z{n}/"))
                .next()
                .is_some();
            if !any {
                for g in group_on.iter_mut().filter(|g| **g == 0) {
                    *g = 1;
                }
            }
        }
    }
    let mut out = Vec::new();
    for (key, g) in group_on.iter().enumerate() {
        if *g == 0 {
            continue;
        }
        let pitch = key as crate::model::Pitch;
        let label = spec
            .text(&format!("g{}.name", g - 1))
            .map(str::to_string)
            .or_else(|| gm_drum_name(pitch).map(str::to_string))
            .unwrap_or_else(|| crate::model::pitch_name(pitch));
        out.push((pitch, label));
    }
    out
}

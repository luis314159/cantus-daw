//! Discrimen, the transient splitter (ADR 0056): its parameter model.
//! Owned by Discrimen; nothing else edits this file.
//!
//! Keys are forever once shipped (automation, saved projects and the exported plug-ins' ids
//! hold them). **Every default leaves the signal untouched, to the bit**: the two parts,
//! unprocessed, add up to the input exactly.
//!
//! | Key | What | Range | Default |
//! |---|---|---|---|
//! | `mode` | zero latency, or a 2 ms look-ahead | 0 / 1 | zero latency |
//! | `sens` | how little of a jump counts as a transient | 0 … 100 % | 50 % |
//! | `speed` | how long a transient lasts | 1 … 50 ms | 10 ms |
//! | `t.gain`, `b.gain` | the transient's and the body's gain | −24 … +24 dB | 0 |
//! | `t.comp`, `b.comp` | each part's one-knob compressor | 0 … 100 % | 0 |
//! | `t.tilt`, `b.tilt` | each part's tilt around 1 kHz | −6 … +6 dB | 0 |
//! | `listen` | both, the transient alone, the body alone | 0 … 2 | both |
//! | `out` | output gain | −24 … +24 dB | 0 |
//!
//! `mode` sets the latency (ADR 0056: a configuration's, never a level's).

use crate::effect::{p, EffectParamInfo};

pub const NAME_KEY: &str = "effect.discrimen";
pub const SPARSE: bool = false;
pub const TAKES_SIDECHAIN: bool = false;
/// Whether it hears a track's notes ([`crate::EffectSlot::midi_from`], ADR 0022).
pub const TAKES_NOTES: bool = false;
/// The locale prefix of each stepped parameter's choices, by key: choice `i` is
/// `{prefix}.{i}` in the locale files. What the exported plug-in shows a host for them
/// (ADR 0020, `cantus_plugins::params`).
pub const CHOICES: &[(&str, &str)] = &[("mode", "discrimen.mode"), ("listen", "discrimen.listen")];

pub const PARAMS: &[EffectParamInfo] = &[
    p("mode", "fx.discrimen.mode", 0.0, 1.0, 0.0, "", false, true),
    p(
        "sens",
        "fx.discrimen.sens",
        0.0,
        1.0,
        0.5,
        "%",
        false,
        false,
    ),
    p(
        "speed",
        "fx.discrimen.speed",
        1.0,
        50.0,
        10.0,
        "ms",
        true,
        false,
    ),
    p(
        "t.gain",
        "fx.discrimen.t.gain",
        -24.0,
        24.0,
        0.0,
        "dB",
        false,
        false,
    ),
    p(
        "t.comp",
        "fx.discrimen.t.comp",
        0.0,
        1.0,
        0.0,
        "%",
        false,
        false,
    ),
    p(
        "t.tilt",
        "fx.discrimen.t.tilt",
        -6.0,
        6.0,
        0.0,
        "dB",
        false,
        false,
    ),
    p(
        "b.gain",
        "fx.discrimen.b.gain",
        -24.0,
        24.0,
        0.0,
        "dB",
        false,
        false,
    ),
    p(
        "b.comp",
        "fx.discrimen.b.comp",
        0.0,
        1.0,
        0.0,
        "%",
        false,
        false,
    ),
    p(
        "b.tilt",
        "fx.discrimen.b.tilt",
        -6.0,
        6.0,
        0.0,
        "dB",
        false,
        false,
    ),
    p(
        "listen",
        "fx.discrimen.listen",
        0.0,
        2.0,
        0.0,
        "",
        false,
        true,
    ),
    p(
        "out",
        "fx.discrimen.out",
        -24.0,
        24.0,
        0.0,
        "dB",
        false,
        false,
    ),
];

pub fn params() -> &'static [EffectParamInfo] {
    PARAMS
}

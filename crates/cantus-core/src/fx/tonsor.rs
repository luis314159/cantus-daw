//! Tonsor, the clipper, with a true-peak ceiling (ADR 0056): its parameter model.
//! Owned by Tonsor; nothing else edits this file.
//!
//! Keys are forever once shipped (automation, saved projects and the exported plug-ins' ids
//! hold them). **Every default leaves the signal untouched, to the bit**: a 0 dBFS ceiling
//! with no knee, at 1× and with the guard off, changes no sample that is not over 0 dBFS.
//!
//! | Key | What | Range | Default |
//! |---|---|---|---|
//! | `drive` | input gain into the curve | −12 … +24 dB | 0 |
//! | `ceil` | the ceiling (dBTP with `tp` on) | −24 … 0 dB | 0 |
//! | `knee` | how far under the ceiling the curve starts to bend | 0 … 12 dB | 0 |
//! | `curve` | hard, soft, analog (`cantus_dsp::tonsor::Curve`) | 0 … 2 | soft |
//! | `os` | oversampling 1×, 2×, 4×, 8×, 16× | 0 … 4 | 1× |
//! | `tp` | the true-peak ceiling (a guard with a look-ahead) | 0 / 1 | off |
//! | `match` | gain match: the drive taken back out after the curve | 0 / 1 | off |
//! | `out` | output gain | −24 … +24 dB | 0 |
//!
//! `os` and `tp` set the latency (ADR 0056: a configuration's, never a level's).

use crate::effect::{p, EffectParamInfo};

pub const NAME_KEY: &str = "effect.tonsor";
pub const SPARSE: bool = false;
pub const TAKES_SIDECHAIN: bool = false;
/// Whether it hears a track's notes ([`crate::EffectSlot::midi_from`], ADR 0022).
pub const TAKES_NOTES: bool = false;
/// The locale prefix of each stepped parameter's choices, by key: choice `i` is
/// `{prefix}.{i}` in the locale files. What the exported plug-in shows a host for them
/// (ADR 0020, `cantus_plugins::params`).
pub const CHOICES: &[(&str, &str)] = &[
    ("curve", "tonsor.curve"),
    ("os", "tonsor.os"),
    ("tp", "tonsor.tp"),
    ("match", "tonsor.match"),
];

pub const PARAMS: &[EffectParamInfo] = &[
    p("drive", "fx.tonsor.drive", -12.0, 24.0, 0.0, "dB", false, false),
    p("ceil", "fx.tonsor.ceil", -24.0, 0.0, 0.0, "dB", false, false),
    p("knee", "fx.tonsor.knee", 0.0, 12.0, 0.0, "dB", false, false),
    p("curve", "fx.tonsor.curve", 0.0, 2.0, 1.0, "", false, true),
    p("os", "fx.tonsor.os", 0.0, 4.0, 0.0, "", false, true),
    p("tp", "fx.tonsor.tp", 0.0, 1.0, 0.0, "", false, true),
    p("match", "fx.tonsor.match", 0.0, 1.0, 0.0, "", false, true),
    p("out", "fx.tonsor.out", -24.0, 24.0, 0.0, "dB", false, false),
];

pub fn params() -> &'static [EffectParamInfo] {
    PARAMS
}

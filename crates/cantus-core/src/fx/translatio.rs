//! Translatio, the frequency shifter (ADR 0055): its parameter model.
//! Owned by Translatio; nothing else edits this file.
//!
//! Keys are forever once shipped (automation, saved projects and the exported plug-ins' ids
//! hold them). **Every default leaves the signal untouched, to the bit**: the mix starts at
//! 0; the shift waits at a slow 10 Hz.
//!
//! | Key | What | Range | Default |
//! |---|---|---|---|
//! | `shift` | every partial moves this many hertz (negative: the other way) | −2 000 … 2 000 Hz | 10 Hz |
//! | `side` | which sideband: up, down, both (up left, down right) (`cantus_dsp::translatio::Side`) | 0 … 2 | up |
//! | `fb` | the shifted output back into the input: spirals | 0 … 95 % | 0 |
//! | `lfo` | the LFO's swing of the shift either way | 0 … 1 000 Hz | 0 |
//! | `sync` | the LFO follows the song (`div`) or runs free (`rate`) | 0 / 1 | free |
//! | `div` | the synced LFO | 8/1 … 1/32 | 1/1 |
//! | `rate` | the free LFO | 0.02 … 10 Hz | 0.2 Hz |
//! | `shape` | the LFO's shape (the family's six) | 0 … 5 | sine |
//! | `mix` | equal-gain crossfade; 100 % the shifted signal alone | 0 … 100 % | 0 |
//! | `out` | output gain | −24 … +24 dB | 0 |

use crate::effect::{p, EffectParamInfo};

pub const NAME_KEY: &str = "effect.translatio";
pub const SPARSE: bool = false;
pub const TAKES_SIDECHAIN: bool = false;
/// Whether it hears a track's notes ([`crate::EffectSlot::midi_from`], ADR 0022).
pub const TAKES_NOTES: bool = false;
/// The locale prefix of each stepped parameter's choices, by key (`modfx.*`: the family's
/// words, kept in Tremor's locale files).
pub const CHOICES: &[(&str, &str)] = &[
    ("side", "translatio.side"),
    ("sync", "modfx.sync"),
    ("div", "modfx.div"),
    ("shape", "modfx.shape"),
];

pub const PARAMS: &[EffectParamInfo] = &[
    p(
        "shift",
        "fx.translatio.shift",
        -2_000.0,
        2_000.0,
        10.0,
        "Hz",
        false,
        false,
    ),
    p("side", "fx.translatio.side", 0.0, 2.0, 0.0, "", false, true),
    p("fb", "fx.translatio.fb", 0.0, 0.95, 0.0, "%", false, false),
    p("lfo", "fx.translatio.lfo", 0.0, 1_000.0, 0.0, "Hz", false, false),
    p("sync", "modfx.p.sync", 0.0, 1.0, 0.0, "", false, true),
    p("div", "modfx.p.div", 0.0, 16.0, 3.0, "", false, true),
    p("rate", "modfx.p.rate", 0.02, 10.0, 0.2, "Hz", true, false),
    p("shape", "modfx.p.shape", 0.0, 5.0, 0.0, "", false, true),
    p("mix", "fx.translatio.mix", 0.0, 1.0, 0.0, "%", false, false),
    p("out", "fx.translatio.out", -24.0, 24.0, 0.0, "dB", false, false),
];

pub fn params() -> &'static [EffectParamInfo] {
    PARAMS
}

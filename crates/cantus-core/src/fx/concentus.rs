//! Concentus, the chorus (Juno, Dimension) (ADR 0055): its parameter model.
//! Owned by Concentus; nothing else edits this file.
//!
//! Keys are forever once shipped (automation, saved projects and the exported plug-ins' ids
//! hold them). **Every default leaves the signal untouched, to the bit**: the mix starts at
//! 0; the rest waits, set to the Juno's first button, for the mix to be turned up.
//!
//! | Key | What | Range | Default |
//! |---|---|---|---|
//! | `mode` | Juno I, Juno II, Juno I+II, Dimension 1 … 4, Free (`cantus_dsp::concentus::Mode`) | 0 … 7 | Juno I |
//! | `mix` | equal-gain crossfade: dry and wet both full at 50 % | 0 … 100 % | 0 |
//! | `voices` | Free: how many | 1 … 4 | 3 |
//! | `sync` | Free: the rate follows the song (`div`) or runs free (`rate`) | 0 / 1 | free |
//! | `div` | Free: the synced rate | 8/1 … 1/32 | 1/1 |
//! | `rate` | Free: the free rate | 0.05 … 10 Hz | 0.8 Hz |
//! | `shape` | Free: sine or triangle | 0 / 1 | triangle |
//! | `delay` | Free: the centre delay | 1 … 30 ms | 7 ms |
//! | `depth` | Free: the sweep either side | 0 … 8 ms | 2 ms |
//! | `spread` | the stereo spread: the right channel's opposite sweep, the voices' places | 0 … 100 % | 100 % |
//! | `tone` | the wet path's low-pass (a bucket brigade's darkness); 20 kHz is open | 2 … 20 kHz | 9 kHz |
//! | `out` | output gain | −24 … +24 dB | 0 |

use crate::effect::{p, EffectParamInfo};

pub const NAME_KEY: &str = "effect.concentus";
pub const SPARSE: bool = false;
pub const TAKES_SIDECHAIN: bool = false;
/// Whether it hears a track's notes ([`crate::EffectSlot::midi_from`], ADR 0022).
pub const TAKES_NOTES: bool = false;
/// The locale prefix of each stepped parameter's choices, by key (`modfx.*`: the family's
/// words, kept in Tremor's locale files).
pub const CHOICES: &[(&str, &str)] = &[
    ("mode", "concentus.mode"),
    ("sync", "modfx.sync"),
    ("div", "modfx.div"),
    ("shape", "modfx.shape"),
];

pub const PARAMS: &[EffectParamInfo] = &[
    p("mode", "fx.concentus.mode", 0.0, 7.0, 0.0, "", false, true),
    p("mix", "fx.concentus.mix", 0.0, 1.0, 0.0, "%", false, false),
    p(
        "voices",
        "fx.concentus.voices",
        1.0,
        4.0,
        3.0,
        "",
        false,
        true,
    ),
    p("sync", "modfx.p.sync", 0.0, 1.0, 0.0, "", false, true),
    p("div", "modfx.p.div", 0.0, 16.0, 3.0, "", false, true),
    p("rate", "modfx.p.rate", 0.05, 10.0, 0.8, "Hz", true, false),
    p("shape", "modfx.p.shape", 0.0, 1.0, 1.0, "", false, true),
    p(
        "delay",
        "fx.concentus.delay",
        1.0,
        30.0,
        7.0,
        "ms",
        true,
        false,
    ),
    p(
        "depth",
        "fx.concentus.depth",
        0.0,
        8.0,
        2.0,
        "ms",
        false,
        false,
    ),
    p(
        "spread",
        "fx.concentus.spread",
        0.0,
        1.0,
        1.0,
        "%",
        false,
        false,
    ),
    p(
        "tone",
        "fx.concentus.tone",
        2_000.0,
        20_000.0,
        9_000.0,
        "Hz",
        true,
        false,
    ),
    p(
        "out",
        "fx.concentus.out",
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

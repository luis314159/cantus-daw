//! Phasis, the phaser (ADR 0055): its parameter model.
//! Owned by Phasis; nothing else edits this file.
//!
//! Keys are forever once shipped (automation, saved projects and the exported plug-ins' ids
//! hold them). **Every default leaves the signal untouched, to the bit**: the mix starts at
//! 0; the rest waits, set to a four-stage pedal, for the mix to be turned up.
//!
//! | Key | What | Range | Default |
//! |---|---|---|---|
//! | `stages` | first-order allpass stages (a notch for every two) | 2 … 12 | 4 |
//! | `mix` | equal-gain crossfade, both full at 50 % (the classic phaser) | 0 … 100 % | 0 |
//! | `lo`, `hi` | the sweep's bounds, on a log scale | 20 … 2 000 Hz, 500 … 16 000 Hz | 200 Hz, 2 kHz |
//! | `fb` | feedback round the stages, either polarity | −95 … 95 % | 0 |
//! | `sync` | the LFO follows the song (`div`) or runs free (`rate`) | 0 / 1 | free |
//! | `div` | the synced sweep | 8/1 … 1/32 | 1/2 |
//! | `rate` | the free sweep | 0.02 … 10 Hz | 0.5 Hz |
//! | `shape` | sine, triangle, square, saw up, saw down, random | 0 … 5 | sine |
//! | `edge` | the soft edge of the stepped shapes | 0 … 100 % | 30 % |
//! | `spread` | the right channel's LFO ahead of the left | 0 … 180° | 0 |
//! | `out` | output gain | −24 … +24 dB | 0 |

use crate::effect::{p, EffectParamInfo};

pub const NAME_KEY: &str = "effect.phasis";
pub const SPARSE: bool = false;
pub const TAKES_SIDECHAIN: bool = false;
/// Whether it hears a track's notes ([`crate::EffectSlot::midi_from`], ADR 0022).
pub const TAKES_NOTES: bool = false;
/// The locale prefix of each stepped parameter's choices, by key (`modfx.*`: the family's
/// words, kept in Tremor's locale files).
pub const CHOICES: &[(&str, &str)] = &[
    ("sync", "modfx.sync"),
    ("div", "modfx.div"),
    ("shape", "modfx.shape"),
];

pub const PARAMS: &[EffectParamInfo] = &[
    p(
        "stages",
        "fx.phasis.stages",
        2.0,
        12.0,
        4.0,
        "",
        false,
        true,
    ),
    p("mix", "fx.phasis.mix", 0.0, 1.0, 0.0, "%", false, false),
    p(
        "lo",
        "fx.phasis.lo",
        20.0,
        2_000.0,
        200.0,
        "Hz",
        true,
        false,
    ),
    p(
        "hi",
        "fx.phasis.hi",
        500.0,
        16_000.0,
        2_000.0,
        "Hz",
        true,
        false,
    ),
    p("fb", "fx.phasis.fb", -0.95, 0.95, 0.0, "%", false, false),
    p("sync", "modfx.p.sync", 0.0, 1.0, 0.0, "", false, true),
    p("div", "modfx.p.div", 0.0, 16.0, 5.0, "", false, true),
    p("rate", "modfx.p.rate", 0.02, 10.0, 0.5, "Hz", true, false),
    p("shape", "modfx.p.shape", 0.0, 5.0, 0.0, "", false, true),
    p("edge", "modfx.p.edge", 0.0, 1.0, 0.3, "%", false, false),
    p(
        "spread",
        "modfx.p.spread",
        0.0,
        180.0,
        0.0,
        "\u{b0}",
        false,
        false,
    ),
    p("out", "fx.phasis.out", -24.0, 24.0, 0.0, "dB", false, false),
];

pub fn params() -> &'static [EffectParamInfo] {
    PARAMS
}

//! Tremor, tremolo and auto-pan (ADR 0055): its parameter model.
//! Owned by Tremor; nothing else edits this file.
//!
//! Keys are forever once shipped (automation, saved projects and the exported plug-ins' ids
//! hold them). **Every default leaves the signal untouched, to the bit**: the depth starts
//! at 0, and everything else waits, set to a musical value, for the depth to be turned up.
//!
//! | Key | What | Range | Default |
//! |---|---|---|---|
//! | `mode` | tremolo, auto-pan, harmonic (`cantus_dsp::tremor::Mode`) | 0 … 2 | tremolo |
//! | `depth` | how far the level (or the pan) moves | 0 … 100 % | 0 |
//! | `sync` | the rate follows the song (`div`) or runs free (`rate`) | 0 / 1 | synced |
//! | `div` | the synced rate (`cantus_dsp::modfx_clock::DIVISIONS`) | 8/1 … 1/32 | 1/8 |
//! | `rate` | the free rate | 0.05 … 20 Hz | 4 Hz |
//! | `shape` | sine, triangle, square, saw up, saw down, random | 0 … 5 | sine |
//! | `edge` | the soft edge of the square, the saws and the random steps | 0 … 100 % | 30 % |
//! | `swing` | every second synced cycle late: 0 straight, 100 % half a cycle late (an MPC's 75 %) | 0 … 100 % | 0 |
//! | `spread` | the right channel's LFO ahead of the left | 0 … 180° | 0 |
//! | `offset` | where in its cycle the LFO starts | 0 … 360° | 0 |
//! | `xover` | the harmonic mode's crossover | 100 … 4 000 Hz | 700 Hz |
//! | `out` | output gain | −24 … +24 dB | 0 |

use crate::effect::{p, EffectParamInfo};

pub const NAME_KEY: &str = "effect.tremor";
pub const SPARSE: bool = false;
pub const TAKES_SIDECHAIN: bool = false;
/// Whether it hears a track's notes ([`crate::EffectSlot::midi_from`], ADR 0022).
pub const TAKES_NOTES: bool = false;
/// The locale prefix of each stepped parameter's choices, by key: choice `i` is
/// `{prefix}.{i}` in the locale files. What the exported plug-in shows a host for them
/// (ADR 0020, `cantus_plugins::params`). `modfx.*` is the family's, kept in Tremor's
/// locale files.
pub const CHOICES: &[(&str, &str)] = &[
    ("mode", "tremor.mode"),
    ("sync", "modfx.sync"),
    ("div", "modfx.div"),
    ("shape", "modfx.shape"),
];

pub const PARAMS: &[EffectParamInfo] = &[
    p("mode", "fx.tremor.mode", 0.0, 2.0, 0.0, "", false, true),
    p("depth", "fx.tremor.depth", 0.0, 1.0, 0.0, "%", false, false),
    p("sync", "modfx.p.sync", 0.0, 1.0, 1.0, "", false, true),
    p("div", "modfx.p.div", 0.0, 16.0, 11.0, "", false, true),
    p("rate", "modfx.p.rate", 0.05, 20.0, 4.0, "Hz", true, false),
    p("shape", "modfx.p.shape", 0.0, 5.0, 0.0, "", false, true),
    p("edge", "modfx.p.edge", 0.0, 1.0, 0.3, "%", false, false),
    p("swing", "modfx.p.swing", 0.0, 1.0, 0.0, "%", false, false),
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
    p(
        "offset",
        "modfx.p.offset",
        0.0,
        360.0,
        0.0,
        "\u{b0}",
        false,
        false,
    ),
    p(
        "xover",
        "fx.tremor.xover",
        100.0,
        4_000.0,
        700.0,
        "Hz",
        true,
        false,
    ),
    p("out", "fx.tremor.out", -24.0, 24.0, 0.0, "dB", false, false),
];

pub fn params() -> &'static [EffectParamInfo] {
    PARAMS
}

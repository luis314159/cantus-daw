//! Vibrans, the vibrato (ADR 0055): its parameter model.
//! Owned by Vibrans; nothing else edits this file.
//!
//! Keys are forever once shipped (automation, saved projects and the exported plug-ins' ids
//! hold them). **Every default leaves the signal untouched, to the bit**: the depth starts
//! at 0 cents; the rest waits, set to a singer's vibrato, for the depth to be turned up.
//!
//! | Key | What | Range | Default |
//! |---|---|---|---|
//! | `depth` | the pitch's swing either way | 0 … 100 ct | 0 |
//! | `sync` | the rate follows the song (`div`) or runs free (`rate`) | 0 / 1 | free |
//! | `div` | the synced rate (`cantus_dsp::modfx_clock::DIVISIONS`) | 8/1 … 1/32 | 1/16 |
//! | `rate` | the free rate | 0.2 … 15 Hz | 5.5 Hz |
//! | `shape` | sine or triangle | 0 / 1 | sine |
//! | `rise` | the depth fades in after each silence | 0 … 2 000 ms | 0 |
//! | `spread` | the right channel's LFO ahead of the left | 0 … 180° | 0 |
//! | `mix` | the moving copy against the dry (lower: a chorus) | 0 … 100 % | 100 % |
//! | `out` | output gain | −24 … +24 dB | 0 |

use crate::effect::{p, EffectParamInfo};

pub const NAME_KEY: &str = "effect.vibrans";
pub const SPARSE: bool = false;
pub const TAKES_SIDECHAIN: bool = false;
/// Whether it hears a track's notes ([`crate::EffectSlot::midi_from`], ADR 0022).
pub const TAKES_NOTES: bool = false;
/// The locale prefix of each stepped parameter's choices, by key (the family's words,
/// kept in Tremor's locale files).
pub const CHOICES: &[(&str, &str)] = &[
    ("sync", "modfx.sync"),
    ("div", "modfx.div"),
    ("shape", "modfx.shape"),
];

pub const PARAMS: &[EffectParamInfo] = &[
    p("depth", "fx.vibrans.depth", 0.0, 100.0, 0.0, "ct", false, false),
    p("sync", "modfx.p.sync", 0.0, 1.0, 0.0, "", false, true),
    p("div", "modfx.p.div", 0.0, 16.0, 14.0, "", false, true),
    p("rate", "modfx.p.rate", 0.2, 15.0, 5.5, "Hz", true, false),
    p("shape", "modfx.p.shape", 0.0, 1.0, 0.0, "", false, true),
    p("rise", "fx.vibrans.rise", 0.0, 2_000.0, 0.0, "ms", false, false),
    p("spread", "modfx.p.spread", 0.0, 180.0, 0.0, "\u{b0}", false, false),
    p("mix", "fx.vibrans.mix", 0.0, 1.0, 1.0, "%", false, false),
    p("out", "fx.vibrans.out", -24.0, 24.0, 0.0, "dB", false, false),
];

pub fn params() -> &'static [EffectParamInfo] {
    PARAMS
}

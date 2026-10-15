//! Impetus, the transient processor (ADR 0056): its parameter model.
//! Owned by Impetus; nothing else edits this file.
//!
//! Keys are forever once shipped (automation, saved projects and the exported plug-ins' ids
//! hold them). **Every default leaves the signal untouched, to the bit**: at 0 % attack and
//! 0 % sustain the gain is exactly 1, whatever else is set.
//!
//! | Key | What | Range | Default |
//! |---|---|---|---|
//! | `attack` | the onsets' gain, a share of their level difference | −100 … +100 % | 0 |
//! | `sustain` | the tails' gain | −100 … +100 % | 0 |
//! | `speed` | how long an onset lasts (the slow attack) | 1 … 50 ms | 15 ms |
//! | `length` | how long a tail is heard as one (the long release) | 20 … 1 000 ms | 250 ms |
//! | `band` | full, highs only, lows only (`cantus_dsp::impetus::Band`) | 0 … 2 | full |
//! | `xover` | the split band's crossover | 40 … 8 000 Hz | 200 Hz |
//! | `out` | output gain | −24 … +24 dB | 0 |

use crate::effect::{p, EffectParamInfo};

pub const NAME_KEY: &str = "effect.impetus";
pub const SPARSE: bool = false;
/// The detector can listen to another strip ([`crate::EffectSlot::sidechain`]).
pub const TAKES_SIDECHAIN: bool = true;
/// Whether it hears a track's notes ([`crate::EffectSlot::midi_from`], ADR 0022).
pub const TAKES_NOTES: bool = false;
/// The locale prefix of each stepped parameter's choices, by key: choice `i` is
/// `{prefix}.{i}` in the locale files. What the exported plug-in shows a host for them
/// (ADR 0020, `cantus_plugins::params`).
pub const CHOICES: &[(&str, &str)] = &[("band", "impetus.band")];

pub const PARAMS: &[EffectParamInfo] = &[
    p("attack", "fx.impetus.attack", -1.0, 1.0, 0.0, "%", false, false),
    p("sustain", "fx.impetus.sustain", -1.0, 1.0, 0.0, "%", false, false),
    p("speed", "fx.impetus.speed", 1.0, 50.0, 15.0, "ms", true, false),
    p("length", "fx.impetus.length", 20.0, 1_000.0, 250.0, "ms", true, false),
    p("band", "fx.impetus.band", 0.0, 2.0, 0.0, "", false, true),
    p("xover", "fx.impetus.xover", 40.0, 8_000.0, 200.0, "Hz", true, false),
    p("out", "fx.impetus.out", -24.0, 24.0, 0.0, "dB", false, false),
];

pub fn params() -> &'static [EffectParamInfo] {
    PARAMS
}

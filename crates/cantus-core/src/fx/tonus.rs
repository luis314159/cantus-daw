//! Tonus, the real-time auto-tuner: its parameter model.
//! Owned by Tonus; nothing else edits this file.
//!
//! **Round 28's contract**: a placeholder until its builder writes the real table. Keys
//! are forever once shipped (automation, saved projects and the exported plug-ins' ids
//! hold them). **Every default leaves the signal untouched, to the bit.**
//!
//! | Key | What | Range |
//! |---|---|---|
//! | `out` | output gain (the placeholder) | −24 … +24 dB |

use crate::effect::{p, EffectParamInfo};

pub const NAME_KEY: &str = "effect.tonus";
pub const SPARSE: bool = false;
pub const TAKES_SIDECHAIN: bool = false;
/// It hears a track's notes ([`crate::EffectSlot::midi_from`], ADR 0022): the target
/// notes it tunes to.
pub const TAKES_NOTES: bool = true;
/// The locale prefix of each stepped parameter's choices, by key: choice `i` is
/// `{prefix}.{i}` in the locale files. What the exported plug-in shows a host for them
/// (ADR 0020, `cantus_plugins::params`).
pub const CHOICES: &[(&str, &str)] = &[];

pub const PARAMS: &[EffectParamInfo] = &[p(
    "out",
    "fx.tonus.out",
    -24.0,
    24.0,
    0.0,
    "dB",
    false,
    false,
)];

pub fn params() -> &'static [EffectParamInfo] {
    PARAMS
}

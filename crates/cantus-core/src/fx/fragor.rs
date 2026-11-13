//! Fragor, the distortion and fuzz pedals (TS, RAT, DS-1, Fuzz Face, Big Muff), circuit-modelled: its parameter model.
//! Owned by Fragor; nothing else edits this file.
//!
//! **The fx side round's contract**: a placeholder until its builder writes the real table.
//! Keys are forever once shipped (automation, saved projects and the exported plug-ins' ids
//! hold them). **Every default leaves the signal untouched, to the bit.**
//!
//! | Key | What | Range |
//! |---|---|---|
//! | `out` | output gain (the placeholder) | −24 … +24 dB |

use crate::effect::{p, EffectParamInfo};

pub const NAME_KEY: &str = "effect.fragor";
pub const SPARSE: bool = false;
pub const TAKES_SIDECHAIN: bool = false;
/// Whether it hears a track's notes ([`crate::EffectSlot::midi_from`], ADR 0022).
pub const TAKES_NOTES: bool = false;
/// The locale prefix of each stepped parameter's choices, by key: choice `i` is
/// `{prefix}.{i}` in the locale files. What the exported plug-in shows a host for them
/// (ADR 0020, `cantus_plugins::params`).
pub const CHOICES: &[(&str, &str)] = &[];

pub const PARAMS: &[EffectParamInfo] = &[p(
    "out",
    "fx.fragor.out",
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The null rule: every default is the value that leaves the signal untouched.
    #[test]
    fn the_placeholder_is_a_null_at_its_default() {
        assert_eq!(PARAMS.len(), 1);
        assert_eq!(PARAMS[0].key, "out");
        assert_eq!(PARAMS[0].default, 0.0);
    }
}

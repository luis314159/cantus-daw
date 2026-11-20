//! Nitor — the one-knob enhancer (round 41, ADR 0151): its parameter model. Owned by the
//! `nitor` builder; the contract declares the kind, its stub and the key it follows.
//!
//! What FL Studio's Soundgoodizer, Low Lifter and the Maximus/Emphasis family do, in one
//! unit (BACKLOG §6l B9, B10, B12): a few curated multiband characters under one *amount*
//! knob, a low-end enhancer (sub-harmonics and tightening, tuned to the song's key) and an
//! exciter by band. The name is the Latin for brightness, splendour.
//!
//! Keys are forever once shipped. **Every default leaves the signal untouched, to the bit**
//! (the amount at zero is a wire).
//!
//! | Key | What | Range | Default |
//! |---|---|---|---|
//! | `out` | output gain | −24 … +24 dB | 0 |
//! | `key` | the key the low end is tuned to: *the song's* (what the engine hands every effect, ADR 0153), or a fixed one (the exported plug-in has no song) | song, C … B | song |
//!
//! **The contract's stub** is a wire times `out`; the builder adds the amount, the
//! characters, the low end and the exciter.

use crate::effect::{p, EffectParamInfo};

pub const NAME_KEY: &str = "effect.nitor";
pub const SPARSE: bool = false;
pub const TAKES_SIDECHAIN: bool = false;
/// Whether it hears a track's notes ([`crate::EffectSlot::midi_from`], ADR 0022).
pub const TAKES_NOTES: bool = false;
/// The locale prefix of each stepped parameter's choices, by key.
pub const CHOICES: &[(&str, &str)] = &[("key", "nitor.key")];

/// `key`'s value that follows the song's key (ADR 0153: the engine hands every effect the
/// project's key); 1 … 12 are C … B.
pub const KEY_FOLLOWS_SONG: f32 = 0.0;

pub const PARAMS: &[EffectParamInfo] = &[
    p("out", "fx.nitor.out", -24.0, 24.0, 0.0, "dB", false, false),
    p("key", "fx.nitor.key", 0.0, 12.0, 0.0, "", false, true),
];

pub fn params() -> &'static [EffectParamInfo] {
    PARAMS
}

/// The tonic Nitor tunes to: `key` = 0 follows `song` (the project's tonic, 0 = C … 11 =
/// B, `None` when the song has no key: then C); 1 … 12 fix it.
pub fn tonic(key: f32, song: Option<u8>) -> u8 {
    let k = key.round().clamp(0.0, 12.0) as u8;
    if k == 0 {
        song.map(|t| t % 12).unwrap_or(0)
    } else {
        k - 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_default_is_a_null_and_the_key_follows_the_song() {
        let out = PARAMS.iter().find(|p| p.key == "out").unwrap();
        assert_eq!(out.default, 0.0);
        let key = PARAMS.iter().find(|p| p.key == "key").unwrap();
        assert_eq!(key.default, KEY_FOLLOWS_SONG);
        assert_eq!(tonic(0.0, Some(7)), 7);
        assert_eq!(tonic(0.0, None), 0);
        assert_eq!(tonic(3.0, Some(7)), 2);
    }
}

//! Nitor — the one-knob enhancer (round 41, ADR 0151): its parameter model. Owned by the
//! `nitor` builder.
//!
//! What FL Studio's Soundgoodizer, Low Lifter and the Maximus/Emphasis family do, in one
//! unit (BACKLOG §6l B9, B10, B12): a few curated multiband characters under one *amount*
//! knob, a low end (a sub tuned to the song's key or an octave under the bass, harmonics,
//! tightening, a lift on the tonic) and an exciter by band — level-matched, so a change is
//! heard as colour and not as loudness. The DSP is `cantus_dsp::nitor`. The name is the Latin
//! for brightness, splendour.
//!
//! Keys are forever once shipped. **Every default leaves the signal untouched, to the bit**
//! (the amount at zero is a wire; so is every level at zero).
//!
//! | Key | What | Range | Default |
//! |---|---|---|---|
//! | `amount` | the one knob: how much of the character | 0 … 100 % | 0 |
//! | `char` | the character: Clear, Punch, Warm, Air, Loud | 0 … 4 | Clear |
//! | `sub` | the sub's level | 0 … 100 % | 0 |
//! | `submode` | what the sub follows: an octave under the bass, or the song's tonic | 0, 1 | octave |
//! | `harm` | the bass's harmonics (heard on small speakers) | 0 … 100 % | 0 |
//! | `tight` | the low end's decay shortened after each hit | 0 … 100 % | 0 |
//! | `lift` | a bell on the tonic's fundamental and its octave (up to +6 dB) | 0 … 100 % | 0 |
//! | `xover` | the low band's crossover | 40 … 250 Hz | 100 Hz |
//! | `body`, `pres`, `air` | the exciter's harmonics at 400 Hz, 2.5 kHz and over 7 kHz | 0 … 100 % | 0 |
//! | `color` | the harmonics' colour: even (warm) … odd (bright) | 0 … 100 % | 50 % |
//! | `match` | the output matched in loudness to the input | off, on | on |
//! | `mix` | dry/wet | 0 … 100 % | 100 % |
//! | `out` | output gain | −24 … +24 dB | 0 |
//! | `key` | the key the low end is tuned to: *the song's* (what the engine hands every effect, ADR 0153), or a fixed one (the exported plug-in has no song) | song, C … B | song |

use crate::effect::{p, EffectParamInfo};

pub const NAME_KEY: &str = "effect.nitor";
pub const SPARSE: bool = false;
pub const TAKES_SIDECHAIN: bool = false;
/// Whether it hears a track's notes ([`crate::EffectSlot::midi_from`], ADR 0022).
pub const TAKES_NOTES: bool = false;
/// The locale prefix of each stepped parameter's choices, by key.
pub const CHOICES: &[(&str, &str)] = &[
    ("char", "fx.nitor.char"),
    ("submode", "fx.nitor.submode"),
    ("match", "fx.nitor.match"),
    ("key", "fx.nitor.key"),
];

/// `key`'s value that follows the song's key (ADR 0153: the engine hands every effect the
/// project's key); 1 … 12 are C … B.
pub const KEY_FOLLOWS_SONG: f32 = 0.0;

/// The characters, by `char`'s value (`cantus_dsp::nitor::Character`'s order).
pub const CHARACTERS: usize = 5;

pub const PARAMS: &[EffectParamInfo] = &[
    p(
        "amount",
        "fx.nitor.amount",
        0.0,
        1.0,
        0.0,
        "%",
        false,
        false,
    ),
    p(
        "char",
        "fx.nitor.char",
        0.0,
        (CHARACTERS - 1) as f32,
        0.0,
        "",
        false,
        true,
    ),
    p("sub", "fx.nitor.sub", 0.0, 1.0, 0.0, "%", false, false),
    p(
        "submode",
        "fx.nitor.submode",
        0.0,
        1.0,
        0.0,
        "",
        false,
        true,
    ),
    p("harm", "fx.nitor.harm", 0.0, 1.0, 0.0, "%", false, false),
    p("tight", "fx.nitor.tight", 0.0, 1.0, 0.0, "%", false, false),
    p("lift", "fx.nitor.lift", 0.0, 1.0, 0.0, "%", false, false),
    p(
        "xover",
        "fx.nitor.xover",
        40.0,
        250.0,
        100.0,
        "Hz",
        true,
        false,
    ),
    p("body", "fx.nitor.body", 0.0, 1.0, 0.0, "%", false, false),
    p("pres", "fx.nitor.pres", 0.0, 1.0, 0.0, "%", false, false),
    p("air", "fx.nitor.air", 0.0, 1.0, 0.0, "%", false, false),
    p("color", "fx.nitor.color", 0.0, 1.0, 0.5, "%", false, false),
    p("match", "fx.nitor.match", 0.0, 1.0, 1.0, "", false, true),
    p("mix", "fx.nitor.mix", 0.0, 1.0, 1.0, "%", false, false),
    p("out", "fx.nitor.out", -24.0, 24.0, 0.0, "dB", false, false),
    p("key", "fx.nitor.key", 0.0, 12.0, 0.0, "", false, true),
];

pub fn params() -> &'static [EffectParamInfo] {
    PARAMS
}

/// Whether a Nitor whose `key` is `key` tunes to the song's key (as [`tonic`] reads it).
pub fn follows_song(key: f32) -> bool {
    key.round().clamp(0.0, 12.0) as u8 == 0
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
        let get = |k: &str| PARAMS.iter().find(|p| p.key == k).unwrap().default;
        // The levels that colour start at zero; the rest only shape what they do.
        for k in [
            "amount", "sub", "harm", "tight", "lift", "body", "pres", "air", "out",
        ] {
            assert_eq!(get(k), 0.0, "{k}");
        }
        assert_eq!(get("mix"), 1.0);
        assert_eq!(get("key"), KEY_FOLLOWS_SONG);
        assert_eq!(tonic(0.0, Some(7)), 7);
        assert_eq!(tonic(0.0, None), 0);
        assert_eq!(tonic(3.0, Some(7)), 2);
    }

    #[test]
    fn every_stepped_parameter_names_its_choices() {
        for p in PARAMS.iter().filter(|p| p.stepped) {
            assert!(CHOICES.iter().any(|(k, _)| *k == p.key), "{}", p.key);
        }
        let mut keys: Vec<&str> = PARAMS.iter().map(|p| p.key).collect();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), PARAMS.len());
    }
}

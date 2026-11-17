//! Libra, the utility (ADR 0130): its parameter model. Owned by Libra; nothing
//! else edits this file.
//!
//! The everyday corrections of a channel, in this order: **DC removal**, the **channel
//! operation** (swap, mono, one side to both, mid, side), **polarity** per channel, a
//! **delay** per channel (to align two microphones), **width** (the side's share), the
//! **balance** (ADR 0012's balance: unity at the centre and on the near side) and the
//! **gain**; **mute** last. The tuner, the correlation meter and the goniometer are the
//! editor's; the tuner's reference pitch is a parameter only so it is kept with the unit.
//!
//! Keys are forever once shipped. **Every default leaves the signal untouched, to the bit.**
//!
//! | Key | What | Range | Default |
//! |---|---|---|---|
//! | `out` | gain (smoothed) | −36 … +36 dB | 0 |
//! | `pan` | balance | −100 … 100 % (left … right) | 0 |
//! | `width` | the side's share (0: mono, 100 %: as it is, 200 %: twice as wide) | 0 … 200 % | 100 % |
//! | `chan` | stereo, swap, mono, left to both, right to both, mid, side | 0 … 6 | stereo |
//! | `pol.l`, `pol.r` | polarity of each channel | normal / inverted | normal |
//! | `dc` | DC removal (a 5 Hz high-pass) | off / on | off |
//! | `delay.l`, `delay.r` | each channel late by this much (whole samples at the session's rate) | 0 … 100 ms | 0 |
//! | `delay.unit` | the delays shown in ms or in samples (the editor's; changes nothing heard) | ms / samples | ms |
//! | `mute` | silence | off / on | off |
//! | `tuner.ref` | the tuner's A (the editor's; changes nothing heard) | 415 … 466 Hz | 440 Hz |

use crate::effect::{p, EffectParamInfo};

pub const NAME_KEY: &str = "effect.libra";
pub const SPARSE: bool = false;
pub const TAKES_SIDECHAIN: bool = false;
/// Whether it hears a track's notes ([`crate::EffectSlot::midi_from`], ADR 0022).
pub const TAKES_NOTES: bool = false;
/// The locale prefix of each stepped parameter's choices, by key.
pub const CHOICES: &[(&str, &str)] = &[
    ("chan", "libra.chan"),
    ("pol.l", "libra.pol"),
    ("pol.r", "libra.pol"),
    ("dc", "libra.onoff"),
    ("delay.unit", "libra.unit"),
    ("mute", "libra.onoff"),
];

/// The longest delay, ms.
pub const MAX_DELAY_MS: f32 = 100.0;

pub const PARAMS: &[EffectParamInfo] = &[
    p("out", "fx.libra.out", -36.0, 36.0, 0.0, "dB", false, false),
    p("pan", "fx.libra.pan", -1.0, 1.0, 0.0, "%", false, false),
    p("width", "fx.libra.width", 0.0, 2.0, 1.0, "%", false, false),
    p("chan", "fx.libra.chan", 0.0, 6.0, 0.0, "", false, true),
    p("pol.l", "fx.libra.pol.l", 0.0, 1.0, 0.0, "", false, true),
    p("pol.r", "fx.libra.pol.r", 0.0, 1.0, 0.0, "", false, true),
    p("dc", "fx.libra.dc", 0.0, 1.0, 0.0, "", false, true),
    p(
        "delay.l",
        "fx.libra.delay.l",
        0.0,
        MAX_DELAY_MS,
        0.0,
        "ms",
        false,
        false,
    ),
    p(
        "delay.r",
        "fx.libra.delay.r",
        0.0,
        MAX_DELAY_MS,
        0.0,
        "ms",
        false,
        false,
    ),
    p(
        "delay.unit",
        "fx.libra.delay.unit",
        0.0,
        1.0,
        0.0,
        "",
        false,
        true,
    ),
    p("mute", "fx.libra.mute", 0.0, 1.0, 0.0, "", false, true),
    p(
        "tuner.ref",
        "fx.libra.tuner.ref",
        415.0,
        466.0,
        440.0,
        "Hz",
        false,
        false,
    ),
];

pub fn params() -> &'static [EffectParamInfo] {
    PARAMS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_unique_and_defaults_in_range() {
        for (i, a) in PARAMS.iter().enumerate() {
            assert!(a.min <= a.default && a.default <= a.max, "{}", a.key);
            for b in &PARAMS[i + 1..] {
                assert_ne!(a.key, b.key);
            }
        }
        assert!(
            PARAMS.iter().any(|p| p.key == "out"),
            "the contract's key stays"
        );
    }

    #[test]
    fn the_defaults_are_a_null() {
        let kind = crate::EffectKind::Utility;
        for (key, null) in [
            ("out", 0.0),
            ("pan", 0.0),
            ("width", 1.0),
            ("chan", 0.0),
            ("pol.l", 0.0),
            ("pol.r", 0.0),
            ("dc", 0.0),
            ("delay.l", 0.0),
            ("delay.r", 0.0),
            ("mute", 0.0),
        ] {
            assert_eq!(kind.param(key).unwrap().default, null, "{key}");
        }
    }

    #[test]
    fn every_choice_list_names_a_stepped_key() {
        for (key, _) in CHOICES {
            let info = PARAMS.iter().find(|p| p.key == *key).expect(key);
            assert!(info.stepped, "{key}");
        }
    }
}

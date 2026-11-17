//! Rota, the rotary speaker: horn and drum, slow and fast, two microphones (ADR 0136,
//! provisional): its parameter model. Owned by the rota builder; nothing else edits this
//! file.
//!
//! Keys are forever once shipped. **Every default leaves the signal untouched, to the
//! bit**: the mix starts at 0; the rest waits, set to the published Leslie figures, for the
//! mix to be turned up (the starting preset does).
//!
//! | Key | What | Range | Default |
//! |---|---|---|---|
//! | `speed` | stop (the brake), slow (chorale), fast (tremolo) | 0 … 2 | slow |
//! | `hslow`, `hfast` | the horn's speeds | 20 … 80, 300 … 500 rpm | 40.32, 423.36 |
//! | `dslow`, `dfast` | the drum's speeds | 20 … 80, 250 … 450 rpm | 36, 357.3 |
//! | `ramp` | the spin-up and spin-down times, scaled | 0.25 … 4 × | 1 × |
//! | `balance` | horn against drum | −12 … +12 dB | 0 |
//! | `spread` | the microphones' angle | 0 … 180° | 120° |
//! | `dist` | the microphones' distance | 10 … 300 cm | 60 cm |
//! | `open` | an open-backed cabinet | 0 / 1 | closed |
//! | `drive` | the cabinet's valve amp | 0 … 100 % | 0 |
//! | `notes` | a note played toggles slow and fast | 0 / 1 | on |
//! | `mix` | wet share | 0 … 100 % | 0 |
//! | `out` | output gain | −24 … +24 dB | 0 |

use crate::effect::{p, EffectParamInfo};

pub const NAME_KEY: &str = "effect.rota";
pub const SPARSE: bool = false;
pub const TAKES_SIDECHAIN: bool = false;
/// Whether it hears a track's notes ([`crate::EffectSlot::midi_from`], ADR 0022): a note
/// throws the speed switch.
pub const TAKES_NOTES: bool = true; // fx3:rota
/// The locale prefix of each stepped parameter's choices, by key.
pub const CHOICES: &[(&str, &str)] = &[
    ("speed", "fx.rota.speed"),
    ("open", "fx.rota.open"),
    ("notes", "fx.rota.notes"),
];

pub const PARAMS: &[EffectParamInfo] = &[
    p("speed", "fx.rota.speed", 0.0, 2.0, 1.0, "", false, true),
    p(
        "hslow",
        "fx.rota.hslow",
        20.0,
        80.0,
        40.32,
        "rpm",
        false,
        false,
    ),
    p(
        "hfast",
        "fx.rota.hfast",
        300.0,
        500.0,
        423.36,
        "rpm",
        false,
        false,
    ),
    p(
        "dslow",
        "fx.rota.dslow",
        20.0,
        80.0,
        36.0,
        "rpm",
        false,
        false,
    ),
    p(
        "dfast",
        "fx.rota.dfast",
        250.0,
        450.0,
        357.3,
        "rpm",
        false,
        false,
    ),
    p(
        "ramp",
        "fx.rota.ramp",
        0.25,
        4.0,
        1.0,
        "\u{d7}",
        true,
        false,
    ),
    p(
        "balance",
        "fx.rota.balance",
        -12.0,
        12.0,
        0.0,
        "dB",
        false,
        false,
    ),
    p(
        "spread",
        "fx.rota.spread",
        0.0,
        180.0,
        120.0,
        "\u{b0}",
        false,
        false,
    ),
    p("dist", "fx.rota.dist", 10.0, 300.0, 60.0, "cm", true, false),
    p("open", "fx.rota.open", 0.0, 1.0, 0.0, "", false, true),
    p("drive", "fx.rota.drive", 0.0, 1.0, 0.0, "%", false, false),
    p("notes", "fx.rota.notes", 0.0, 1.0, 1.0, "", false, true),
    p("mix", "fx.rota.mix", 0.0, 1.0, 0.0, "%", false, false),
    p("out", "fx.rota.out", -24.0, 24.0, 0.0, "dB", false, false),
];

pub fn params() -> &'static [EffectParamInfo] {
    PARAMS
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The null: the mix at 0; the blessed `out` kept; every choice has its key.
    #[test]
    fn the_mix_starts_at_zero() {
        let d = |k: &str| PARAMS.iter().find(|p| p.key == k).unwrap().default;
        assert_eq!(d("mix"), 0.0);
        assert_eq!(d("out"), 0.0);
        for (k, _) in CHOICES {
            assert!(PARAMS.iter().any(|p| p.key == *k && p.stepped), "{k}");
        }
    }
}

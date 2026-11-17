//! Arca, amp and cabinet: preamp valves, the tone stack, the power amp, the cabinet by
//! impulse response (ADR 0134): its parameter model. Owned by the arca
//! builder; nothing else edits this file.
//!
//! Keys are forever once shipped. **The null**: an amp colours by design — no setting of
//! it is transparent — so the null is *no amp and no cabinet*, the defaults: the signal
//! untouched, to the bit, no latency. The starting preset chooses an amp and its cabinet.
//!
//! | Key | What | Range | Default |
//! |---|---|---|---|
//! | `model` | none, clean, crunch, chime, high gain, bass | 0 … 5 | none |
//! | `gain` | the preamp's volume (or gain) pot | 0 … 100 % | 50 % |
//! | `bass`, `mid`, `treble` | the tone stack's pots | 0 … 100 % | 50 % |
//! | `presence` | the power amp's presence (the chime's cut, turned the other way) | 0 … 100 % | 50 % |
//! | `master` | a master volume before the phase inverter (100 %: as built, none) | 0 … 100 % | 100 % |
//! | `sag` | the supply's softness (50 %: the amp's own rectifier) | 0 … 100 % | 50 % |
//! | `cab` | none, 1×12 open, 2×12 open, 4×12 vintage, 4×12 modern, 4×10, the loaded file | 0 … 6 | none |
//! | `mic` | dynamic, ribbon, condenser | 0 … 2 | dynamic |
//! | `micpos` | from the cone's centre to its edge | 0 … 100 % | 30 % |
//! | `micdist` | from the grille | 1 … 30 cm | 3 cm |
//! | `os` | oversampling: 2×, 4×, 8× | 0 … 2 | 4× |
//! | `mix` | wet share | 0 … 100 % | 100 % |
//! | `out` | output gain | −24 … +24 dB | 0 |
//!
//! The cabinet and the microphone's three choose an impulse response, made off the audio
//! thread: moving them while playing swaps the response whole, a few blocks later.

use crate::effect::{p, EffectParamInfo};

pub const NAME_KEY: &str = "effect.arca";
pub const SPARSE: bool = false;
pub const TAKES_SIDECHAIN: bool = false;
/// Whether it hears a track's notes ([`crate::EffectSlot::midi_from`], ADR 0022).
pub const TAKES_NOTES: bool = false;
/// The locale prefix of each stepped parameter's choices, by key.
pub const CHOICES: &[(&str, &str)] = &[
    ("model", "fx.arca.model"),
    ("cab", "fx.arca.cab"),
    ("mic", "fx.arca.mic"),
    ("os", "fx.arca.os"),
];

/// The cabinet choice that plays the loaded file ([`crate::EffectSpec::ir`]).
pub const CAB_FILE: f32 = 6.0;

pub const PARAMS: &[EffectParamInfo] = &[
    p("model", "fx.arca.model", 0.0, 5.0, 0.0, "", false, true),
    p("gain", "fx.arca.gain", 0.0, 1.0, 0.5, "%", false, false),
    p("bass", "fx.arca.bass", 0.0, 1.0, 0.5, "%", false, false),
    p("mid", "fx.arca.mid", 0.0, 1.0, 0.5, "%", false, false),
    p("treble", "fx.arca.treble", 0.0, 1.0, 0.5, "%", false, false),
    p(
        "presence",
        "fx.arca.presence",
        0.0,
        1.0,
        0.5,
        "%",
        false,
        false,
    ),
    p("master", "fx.arca.master", 0.0, 1.0, 1.0, "%", false, false),
    p("sag", "fx.arca.sag", 0.0, 1.0, 0.5, "%", false, false),
    p("cab", "fx.arca.cab", 0.0, 6.0, 0.0, "", false, true),
    p("mic", "fx.arca.mic", 0.0, 2.0, 0.0, "", false, true),
    p("micpos", "fx.arca.micpos", 0.0, 1.0, 0.3, "%", false, false),
    p(
        "micdist",
        "fx.arca.micdist",
        1.0,
        30.0,
        3.0,
        "cm",
        false,
        false,
    ),
    p("os", "fx.arca.os", 0.0, 2.0, 1.0, "", false, true),
    p("mix", "fx.arca.mix", 0.0, 1.0, 1.0, "%", false, false),
    p("out", "fx.arca.out", -24.0, 24.0, 0.0, "dB", false, false),
];

pub fn params() -> &'static [EffectParamInfo] {
    PARAMS
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The null: no amp, no cabinet; the blessed `out` kept; every choice has its key.
    #[test]
    fn the_defaults_are_no_amp_and_no_cabinet() {
        let d = |k: &str| PARAMS.iter().find(|p| p.key == k).unwrap().default;
        assert_eq!(d("model"), 0.0);
        assert_eq!(d("cab"), 0.0);
        assert_eq!(d("out"), 0.0);
        for (k, _) in CHOICES {
            assert!(PARAMS.iter().any(|p| p.key == *k && p.stepped), "{k}");
        }
    }
}

//! Comparatio, the reference comparator (ADR 0146, provisional): its parameter model. Owned
//! by the wave-C builder; the DSP is `cantus_dsp::comparatio`, whose report
//! (`docs/round-fx/comparatio.md`) says why it is built as it is.
//!
//! **A monitoring unit.** You load a reference track (a file, stored by path in
//! [`crate::EffectSpec::ir`] and set with `Command::SetEffectIr`, as Aula's response and
//! Arca's cabinet are — never copied into the project) and switch between your mix and the
//! reference **at the same loudness**. Its place is the master's last insert; it may sit on
//! any strip. **An export, a freeze, a stem or any other render hears the mix, never the
//! reference**: the engine takes the reference out of every snapshot it renders from
//! (`cantus_engine::fx::comparatio_store::strip_for_render`).
//!
//! Keys are forever once shipped. **Every default leaves the signal untouched, to the
//! bit**: the mix heard. With no reference loaded the mix is heard whatever is set.
//!
//! | Key | What | Range |
//! |---|---|---|
//! | `listen` | what is heard: the mix, the reference, X (one of them, hidden), the blind preference's One and Two | 0 … 4 |
//! | `match` | how the reference's level follows the mix's: integrated, short-term, by hand | 0 … 2 |
//! | `offset` | added to the match in every mode | −24 … +24 dB |
//! | `refl` | the integrated match reads the reference over what has played, or over its whole file | 0 … 1 |
//! | `fade` | the switch's crossfade | 1 … 500 ms |
//! | `sync` | where the reference plays: its own playhead while the song plays, locked to the song, a loop | 0 … 2 |
//! | `soff` | locked: the reference ahead of the song by | −600 … +600 s |
//! | `lstart`, `lend` | the loop's region in the reference (an end at or before the start: to the reference's end) | 0 … 3 600 s |
//! | `seed`, `trial` | the blind test's order and the trial under way | 0 … 65 535, 0 … 999 |
//! | `res` | the tonal picture in thirds or sixths of an octave (the display's; no sample changes) | 0 … 1 |

use crate::effect::{p, EffectParamInfo};

pub const NAME_KEY: &str = "effect.comparatio";
pub const SPARSE: bool = false;
pub const TAKES_SIDECHAIN: bool = false;
pub const TAKES_NOTES: bool = false;

/// `listen`'s values (the DSP's `Listen`, in this order).
pub const LISTEN_MIX: f32 = 0.0;
pub const LISTEN_REFERENCE: f32 = 1.0;
pub const LISTEN_X: f32 = 2.0;
pub const LISTEN_ONE: f32 = 3.0;
pub const LISTEN_TWO: f32 = 4.0;

/// The locale prefix of each stepped parameter's choices, by key (ADR 0020).
pub const CHOICES: &[(&str, &str)] = &[
    ("listen", "fx.comparatio.listen"),
    ("match", "fx.comparatio.match"),
    ("refl", "fx.comparatio.refl"),
    ("sync", "fx.comparatio.sync"),
    ("res", "fx.comparatio.res"),
];

const fn switch(key: &'static str, label: &'static str, max: f32, default: f32) -> EffectParamInfo {
    p(key, label, 0.0, max, default, "", false, true)
}

const fn seconds(key: &'static str, label: &'static str, lo: f32, hi: f32) -> EffectParamInfo {
    p(key, label, lo, hi, 0.0, "s", false, false)
}

pub const PARAMS: &[EffectParamInfo] = &[
    switch("listen", "fx.comparatio.listen", 4.0, LISTEN_MIX),
    switch("match", "fx.comparatio.match", 2.0, 0.0),
    p(
        "offset",
        "fx.comparatio.offset",
        -24.0,
        24.0,
        0.0,
        "dB",
        false,
        false,
    ),
    switch("refl", "fx.comparatio.refl", 1.0, 0.0),
    p(
        "fade",
        "fx.comparatio.fade",
        1.0,
        500.0,
        30.0,
        "ms",
        true,
        false,
    ),
    switch("sync", "fx.comparatio.sync", 2.0, 0.0),
    seconds("soff", "fx.comparatio.soff", -600.0, 600.0),
    seconds("lstart", "fx.comparatio.lstart", 0.0, 3_600.0),
    seconds("lend", "fx.comparatio.lend", 0.0, 3_600.0),
    p(
        "seed",
        "fx.comparatio.seed",
        0.0,
        65_535.0,
        0.0,
        "",
        false,
        true,
    ),
    p(
        "trial",
        "fx.comparatio.trial",
        0.0,
        999.0,
        0.0,
        "",
        false,
        true,
    ),
    switch("res", "fx.comparatio.res", 1.0, 0.0),
];

pub fn params() -> &'static [EffectParamInfo] {
    PARAMS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_unique_defaults_in_range_and_choices_name_stepped_keys() {
        for (i, a) in PARAMS.iter().enumerate() {
            assert!(a.min <= a.default && a.default <= a.max, "{}", a.key);
            for b in &PARAMS[i + 1..] {
                assert_ne!(a.key, b.key);
            }
        }
        for (k, _) in CHOICES {
            let info = PARAMS.iter().find(|p| p.key == *k).expect(k);
            assert!(info.stepped, "{k}");
        }
        let listen = PARAMS.iter().find(|p| p.key == "listen").unwrap();
        assert_eq!(listen.default, LISTEN_MIX, "the null: the mix heard");
        assert_eq!(listen.max, LISTEN_TWO);
    }
}

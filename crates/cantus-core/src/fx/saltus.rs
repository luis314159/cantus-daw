//! Saltus, the pitch unit: octaver, pitch shifter and harmoniser, whammy (ADR 0132):
//! its parameter model. Owned by the saltus builder; nothing else edits this
//! file. The DSP is `cantus_dsp::saltus`; the ADR says why it is built as it is.
//!
//! Keys are forever once shipped (automation, saved projects and the exported plug-ins' ids
//! hold them). **Every default leaves the signal untouched, to the bit**: Shift, mono, voice 1
//! at 0 semitones and full level, voice 2 silent, no dry — the input delayed by the fixed
//! latency (128 samples at 48 kHz), which delay compensation takes back. The other modes'
//! defaults are musical, since they are heard only when chosen (a third and a fifth above,
//! the octave down, the whammy's heel at unison and toe an octave up).
//!
//! | Key | What | Range |
//! |---|---|---|
//! | `mode` | shift, harmony, octave, whammy | 0 … 3 |
//! | `engine` | Shift and Whammy: mono (one note at a time, formants kept, 2.7 ms) or poly (chords and mixes, 85 ms) | 0 … 1 |
//! | `ostyle` | Octave: analog (the divider, no latency) or clean (2.7 ms) | 0 … 1 |
//! | `hsrc` | Harmony counts its degrees in the key and scale, in the chord held on MIDI, or sings the MIDI melody | 0 … 2 |
//! | `st1`, `st2` | each voice's interval in Shift | −24 … +24 st |
//! | `ct1`, `ct2` | each voice's fine offset (Shift and Harmony) | −100 … +100 cents |
//! | `deg1`, `deg2` | each voice's interval in Harmony, scale degrees (±7 an octave in a seven-note scale) | −14 … +14 |
//! | `lvl1`, `lvl2` | each voice's level | 0 … 100 % |
//! | `pan1`, `pan2` | each voice's balance | −100 … +100 % |
//! | `dry` | the dry signal, delayed to line up | 0 … 100 % |
//! | `key`, `scale` | Harmony's key and scale (Tonus's thirteen scales) | 0 … 11, 0 … 12 |
//! | `formant` | keep the formants (1) or let them follow the pitch (0) | 0 … 1 |
//! | `throat` | the formant shift | −12 … +12 st |
//! | `pedal` | the whammy's pedal, heel (0) to toe (100 %) | 0 … 100 % |
//! | `heel`, `toe` | the whammy's intervals at each end of the pedal | −24 … +24 st |
//! | `glide` | how fast the voice follows the pedal | 0 … 500 ms |
//! | `wsrc` | what plays the whammy: the pedal (0) or the notes held on MIDI (1: a note held presses the toe) | 0 … 1 |
//! | `sub1`, `sub2`, `up1` | Octave: one down, two down, one up | 0 … 100 % |
//! | `a4` | the reference pitch | 415 … 466 Hz |
//! | `out` | the output level | −24 … +24 dB |

use crate::effect::{p, EffectParamInfo};

pub const NAME_KEY: &str = "effect.saltus";
pub const SPARSE: bool = false;
pub const TAKES_SIDECHAIN: bool = false;
/// It hears a track's notes ([`crate::EffectSlot::midi_from`], ADR 0022): the chord or the
/// melody the harmony sings, and the notes that play the whammy.
pub const TAKES_NOTES: bool = true; // fx3:saltus

/// The four modes, in `mode`'s order.
pub const MODES: usize = 4;

/// The locale prefix of each stepped parameter's choices, by key: choice `i` is
/// `{prefix}.{i}` in the locale files. What the exported plug-in shows a host for them
/// (ADR 0020, `cantus_plugins::params`).
pub const CHOICES: &[(&str, &str)] = &[
    ("mode", "fx.saltus.mode"),
    ("engine", "fx.saltus.engine"),
    ("ostyle", "fx.saltus.ostyle"),
    ("hsrc", "fx.saltus.hsrc"),
    ("key", "fx.saltus.key"),
    ("scale", "fx.saltus.scale"),
    ("formant", "fx.saltus.formant"),
    ("wsrc", "fx.saltus.wsrc"),
];

const fn step(key: &'static str, label: &'static str, max: f32, default: f32) -> EffectParamInfo {
    p(key, label, 0.0, max, default, "", false, true)
}

const fn semis(key: &'static str, label: &'static str, default: f32) -> EffectParamInfo {
    p(key, label, -24.0, 24.0, default, "st", false, true)
}

const fn cents(key: &'static str, label: &'static str) -> EffectParamInfo {
    p(key, label, -100.0, 100.0, 0.0, "ct", false, false)
}

const fn degree(key: &'static str, label: &'static str, default: f32) -> EffectParamInfo {
    p(key, label, -14.0, 14.0, default, "", false, true)
}

const fn amount(key: &'static str, label: &'static str, default: f32) -> EffectParamInfo {
    p(key, label, 0.0, 1.0, default, "%", false, false)
}

const fn balance(key: &'static str, label: &'static str) -> EffectParamInfo {
    p(key, label, -1.0, 1.0, 0.0, "%", false, false)
}

pub const PARAMS: &[EffectParamInfo] = &[
    // The order is a host's list and the pedal's row on Tabula's board (its face's
    // knobs, then the first others): what a pedal needs first. Keys, not places, are kept.
    step("mode", "fx.saltus.mode", 3.0, 0.0),
    step("engine", "fx.saltus.engine", 1.0, 0.0),
    step("ostyle", "fx.saltus.ostyle", 1.0, 0.0),
    step("hsrc", "fx.saltus.hsrc", 2.0, 0.0),
    semis("st1", "fx.saltus.st1", 0.0),
    amount("lvl1", "fx.saltus.lvl1", 1.0),
    amount("dry", "fx.saltus.dry", 0.0),
    amount("pedal", "fx.saltus.pedal", 0.0),
    p("out", "fx.saltus.out", -24.0, 24.0, 0.0, "dB", false, false),
    p(
        "throat",
        "fx.saltus.throat",
        -12.0,
        12.0,
        0.0,
        "st",
        false,
        false,
    ),
    p(
        "glide",
        "fx.saltus.glide",
        0.0,
        500.0,
        40.0,
        "ms",
        false,
        false,
    ),
    cents("ct1", "fx.saltus.ct1"),
    degree("deg1", "fx.saltus.deg1", 2.0),
    balance("pan1", "fx.saltus.pan1"),
    semis("st2", "fx.saltus.st2", 0.0),
    cents("ct2", "fx.saltus.ct2"),
    degree("deg2", "fx.saltus.deg2", 4.0),
    amount("lvl2", "fx.saltus.lvl2", 0.0),
    balance("pan2", "fx.saltus.pan2"),
    step("key", "fx.saltus.key", 11.0, 0.0),
    // Major: Tonus's scales, in `cantus_dsp::tonus::Scale`'s order (0 is chromatic).
    step("scale", "fx.saltus.scale", 12.0, 1.0),
    step("formant", "fx.saltus.formant", 1.0, 1.0),
    semis("heel", "fx.saltus.heel", 0.0),
    semis("toe", "fx.saltus.toe", 12.0),
    step("wsrc", "fx.saltus.wsrc", 1.0, 0.0),
    amount("sub1", "fx.saltus.sub1", 1.0),
    amount("sub2", "fx.saltus.sub2", 0.0),
    amount("up1", "fx.saltus.up1", 0.0),
    p(
        "a4",
        "fx.saltus.a4",
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
        // The contract's placeholder key stays (its plug-in ids are blessed).
        let out = PARAMS.iter().find(|p| p.key == "out").expect("out");
        assert_eq!((out.default, out.unit), (0.0, "dB"));
        assert_eq!(PARAMS[0].max as usize + 1, MODES);
    }
}

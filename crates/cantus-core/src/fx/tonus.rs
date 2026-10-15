//! Tonus, the real-time auto-tuner: its parameter model. Owned by Tonus; nothing else edits
//! this file. The DSP is `cantus_dsp::tonus`; ADR 0053 says why it is built as it is.
//!
//! Keys are forever once shipped (automation, saved projects and the exported plug-ins'
//! ids hold them). Tonus is a corrector: at its defaults it **does** correct (a chromatic
//! tuner at a quick, natural speed — what a singer expects on inserting it). Its nulls are
//! **amount 0**, every note on **bypass**, a MIDI source with **no note held**, and the
//! slot's own bypass: each is the input delayed by the fixed latency, bit for bit.
//!
//! | Key | What | Range |
//! |---|---|---|
//! | `src` | where the targets come from: 0 key and scale, 1 the notes held on MIDI, 2 the MIDI melody (the newest note, exactly) | 0 … 2 |
//! | `key` | the key, C … B | 0 … 11 |
//! | `scale` | chromatic, major, minor, harmonic and melodic minor, dorian, phrygian, lydian, mixolydian, locrian, major and minor pentatonic, blues | 0 … 12 |
//! | `speed` | the retune speed (0: the hard effect) | 0 … 400 ms |
//! | `amount` | how much of the correction is applied | 0 … 100 % |
//! | `human` | humanise: longer notes keep more of their movement | 0 … 100 % |
//! | `flex` | deviations smaller than this are left alone | 0 … 100 cents |
//! | `vib` | the singer's vibrato, as much as the speed lets through: −100 % removes it, +100 % doubles it | −100 … +100 % |
//! | `vdepth`, `vrate` | a created vibrato's depth and rate | 0 … 100 cents, 2 … 10 Hz |
//! | `formant` | keep the formants (1) or let them follow the pitch (0) | 0 … 1 |
//! | `throat` | the formant shift | −12 … +12 st |
//! | `trans` | a transposition | −24 … +24 st |
//! | `a4` | the reference pitch | 415 … 466 Hz |
//! | `n0` … `n11` | each pitch class, C … B: as the scale (0), on (1), removed (2), bypassed (3) | 0 … 3 |

use crate::effect::{p, EffectParamInfo};

pub const NAME_KEY: &str = "effect.tonus";
pub const SPARSE: bool = false;
pub const TAKES_SIDECHAIN: bool = false;
/// It hears a track's notes ([`crate::EffectSlot::midi_from`], ADR 0022): the target
/// notes it tunes to.
pub const TAKES_NOTES: bool = true;

/// The per-note keys, C … B.
pub const NOTE_KEYS: [&str; 12] = [
    "n0", "n1", "n2", "n3", "n4", "n5", "n6", "n7", "n8", "n9", "n10", "n11",
];

/// The locale prefix of each stepped parameter's choices, by key: choice `i` is
/// `{prefix}.{i}` in the locale files. What the exported plug-in shows a host for them
/// (ADR 0020, `cantus_plugins::params`).
pub const CHOICES: &[(&str, &str)] = &[
    ("src", "fx.tonus.src"),
    ("key", "fx.tonus.key"),
    ("scale", "fx.tonus.scale"),
    ("formant", "fx.tonus.formant"),
    ("n0", "fx.tonus.note"),
    ("n1", "fx.tonus.note"),
    ("n2", "fx.tonus.note"),
    ("n3", "fx.tonus.note"),
    ("n4", "fx.tonus.note"),
    ("n5", "fx.tonus.note"),
    ("n6", "fx.tonus.note"),
    ("n7", "fx.tonus.note"),
    ("n8", "fx.tonus.note"),
    ("n9", "fx.tonus.note"),
    ("n10", "fx.tonus.note"),
    ("n11", "fx.tonus.note"),
];

const fn note(key: &'static str, label: &'static str) -> EffectParamInfo {
    p(key, label, 0.0, 3.0, 0.0, "", false, true)
}

pub const PARAMS: &[EffectParamInfo] = &[
    p("src", "fx.tonus.src", 0.0, 2.0, 0.0, "", false, true),
    p("key", "fx.tonus.key", 0.0, 11.0, 0.0, "", false, true),
    p("scale", "fx.tonus.scale", 0.0, 12.0, 0.0, "", false, true),
    p("speed", "fx.tonus.speed", 0.0, 400.0, 20.0, "ms", false, false),
    p("amount", "fx.tonus.amount", 0.0, 1.0, 1.0, "%", false, false),
    p("human", "fx.tonus.human", 0.0, 1.0, 0.0, "%", false, false),
    p("flex", "fx.tonus.flex", 0.0, 100.0, 0.0, "ct", false, false),
    p("vib", "fx.tonus.vib", -1.0, 1.0, 0.0, "%", false, false),
    p("vdepth", "fx.tonus.vdepth", 0.0, 100.0, 0.0, "ct", false, false),
    p("vrate", "fx.tonus.vrate", 2.0, 10.0, 5.5, "Hz", false, false),
    p("formant", "fx.tonus.formant", 0.0, 1.0, 1.0, "", false, true),
    p("throat", "fx.tonus.throat", -12.0, 12.0, 0.0, "st", false, false),
    p("trans", "fx.tonus.trans", -24.0, 24.0, 0.0, "st", false, true),
    p("a4", "fx.tonus.a4", 415.0, 466.0, 440.0, "Hz", false, false),
    note("n0", "fx.tonus.n0"),
    note("n1", "fx.tonus.n1"),
    note("n2", "fx.tonus.n2"),
    note("n3", "fx.tonus.n3"),
    note("n4", "fx.tonus.n4"),
    note("n5", "fx.tonus.n5"),
    note("n6", "fx.tonus.n6"),
    note("n7", "fx.tonus.n7"),
    note("n8", "fx.tonus.n8"),
    note("n9", "fx.tonus.n9"),
    note("n10", "fx.tonus.n10"),
    note("n11", "fx.tonus.n11"),
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
        for k in NOTE_KEYS {
            assert!(CHOICES.iter().any(|c| c.0 == k), "{k}");
        }
    }
}

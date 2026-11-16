//! Loquax, the vocoder (ADR 0139, provisional): its parameter model. Owned by the wave-A
//! builder; the DSP is `cantus_dsp::loquax`, whose report (`docs/round-fx/loquax.md`) says
//! why it is built as it is.
//!
//! Keys are forever once shipped (automation, saved projects and the exported plug-ins' ids
//! hold them). **Every default leaves the signal untouched, to the bit**: the mix at 0 —
//! nothing runs but the output gain, and at 0 dB that is not applied.
//!
//! **Who speaks and who sings.** The strip's own sound is the voice (the modulator); the
//! carrier is the built-in synthesiser played by the notes of the track the slot listens
//! to (`EffectSlot::midi_from`, ADR 0022), the **sidechain** — another strip routed to the
//! slot as its key, the engine's ordinary sidechain path — or both. `swap` trades the
//! roles: the strip is the carrier and the key is the voice.
//!
//! | Key | What | Range |
//! |---|---|---|
//! | `mix` | the strip's own sound to the vocoded | 0 … 100 % |
//! | `src` | the carrier: the sidechain, the synthesiser or both | 0 … 2 |
//! | `swap` | the strip is the carrier and the sidechain the voice | 0 … 1 |
//! | `bands`, `order` | 8 … 64 bands; slopes of order 4, 6 or 8 | 8 … 64, 0 … 2 |
//! | `lo`, `hi`, `width` | the bands' range and their width in spacings | 40 … 1 000 Hz, 1.5 … 18 kHz, 0.5 … 2 |
//! | `att`, `rel` | the bands' followers | 0.1 … 100 ms, 5 … 1 000 ms |
//! | `formant`, `bshift` | the synthesis bands moved (semitones), the levels moved (bands) | −24 … +24 st, −16 … +16 |
//! | `tilt`, `flat` | dB per octave about 1 kHz; the carrier flattened band by band | −6 … +6, 0 … 100 % |
//! | `sib`, `sibhz`, `sibgate` | the sibilance pass-through, its corner, only on consonants | 0 … 100 %, 2 … 12 kHz, 0 … 1 |
//! | `unv` | noise into the carrier on consonants | 0 … 100 % |
//! | `hold` | the followers frozen: the vowel sung on | 0 … 1 |
//! | `spread` | bands alternately left and right | 0 … 100 % |
//! | `cpass` | the raw carrier into the output | 0 … 100 % |
//! | `saw`, `pulse`, `pw`, `noise` | the synthesiser's blend and pulse width | 0 … 100 %, 5 … 50 % |
//! | `uni`, `detune`, `uspread` | copies per voice, their detune and spread | 1 … 7, 0 … 100 ct, 0 … 100 % |
//! | `oct`, `satt`, `srel`, `slevel` | octave, attack, release, level | −3 … +3, 0.5 … 2 000 ms, 5 … 5 000 ms, −48 … +12 dB |
//! | `robot`, `rnote`, `rgate` | the drone: on, its note, the voice's gate | 0 … 1, 24 … 96, −80 … 0 dB |
//! | `b0` … `b63` | each band's level (the display's bars) | −24 … +12 dB |
//! | `out` | the output level | −24 … +24 dB |

use crate::effect::{p, EffectParamInfo};

pub const NAME_KEY: &str = "effect.loquax";
pub const SPARSE: bool = false;
/// The carrier (or, swapped, the voice) can be another strip.
pub const TAKES_SIDECHAIN: bool = true;
/// The synthesiser plays the notes of the track the slot listens to.
pub const TAKES_NOTES: bool = true;

/// The most bands, and so the band-level keys `b0` … `b63`.
pub const MAX_BANDS: usize = 64;

/// The locale prefix of each stepped parameter's choices, by key (ADR 0020).
pub const CHOICES: &[(&str, &str)] = &[
    ("src", "fx.loquax.src"),
    ("swap", "fx.loquax.swap"),
    ("order", "fx.loquax.order"),
    ("sibgate", "fx.loquax.sibgate"),
    ("hold", "fx.loquax.hold"),
    ("robot", "fx.loquax.robot"),
];

const fn amount(key: &'static str, label: &'static str, default: f32) -> EffectParamInfo {
    p(key, label, 0.0, 1.0, default, "%", false, false)
}

const fn switch(key: &'static str, label: &'static str, max: f32, default: f32) -> EffectParamInfo {
    p(key, label, 0.0, max, default, "", false, true)
}

const fn ms(key: &'static str, label: &'static str, lo: f32, hi: f32, d: f32) -> EffectParamInfo {
    p(key, label, lo, hi, d, "ms", true, false)
}

const fn hz(key: &'static str, label: &'static str, lo: f32, hi: f32, d: f32) -> EffectParamInfo {
    p(key, label, lo, hi, d, "Hz", true, false)
}

const fn band(key: &'static str, label: &'static str) -> EffectParamInfo {
    p(key, label, -24.0, 12.0, 0.0, "dB", false, false)
}

/// The keys `b0` … `b63`, in band order.
pub const BAND_KEYS: [&str; MAX_BANDS] = [
    "b0", "b1", "b2", "b3", "b4", "b5", "b6", "b7", "b8", "b9", "b10", "b11", "b12", "b13", "b14",
    "b15", "b16", "b17", "b18", "b19", "b20", "b21", "b22", "b23", "b24", "b25", "b26", "b27",
    "b28", "b29", "b30", "b31", "b32", "b33", "b34", "b35", "b36", "b37", "b38", "b39", "b40",
    "b41", "b42", "b43", "b44", "b45", "b46", "b47", "b48", "b49", "b50", "b51", "b52", "b53",
    "b54", "b55", "b56", "b57", "b58", "b59", "b60", "b61", "b62", "b63",
];

/// The band levels' label keys, `fx.loquax.b0` … (each band named by its number).
const BAND_LABELS: [&str; MAX_BANDS] = [
    "fx.loquax.b0",
    "fx.loquax.b1",
    "fx.loquax.b2",
    "fx.loquax.b3",
    "fx.loquax.b4",
    "fx.loquax.b5",
    "fx.loquax.b6",
    "fx.loquax.b7",
    "fx.loquax.b8",
    "fx.loquax.b9",
    "fx.loquax.b10",
    "fx.loquax.b11",
    "fx.loquax.b12",
    "fx.loquax.b13",
    "fx.loquax.b14",
    "fx.loquax.b15",
    "fx.loquax.b16",
    "fx.loquax.b17",
    "fx.loquax.b18",
    "fx.loquax.b19",
    "fx.loquax.b20",
    "fx.loquax.b21",
    "fx.loquax.b22",
    "fx.loquax.b23",
    "fx.loquax.b24",
    "fx.loquax.b25",
    "fx.loquax.b26",
    "fx.loquax.b27",
    "fx.loquax.b28",
    "fx.loquax.b29",
    "fx.loquax.b30",
    "fx.loquax.b31",
    "fx.loquax.b32",
    "fx.loquax.b33",
    "fx.loquax.b34",
    "fx.loquax.b35",
    "fx.loquax.b36",
    "fx.loquax.b37",
    "fx.loquax.b38",
    "fx.loquax.b39",
    "fx.loquax.b40",
    "fx.loquax.b41",
    "fx.loquax.b42",
    "fx.loquax.b43",
    "fx.loquax.b44",
    "fx.loquax.b45",
    "fx.loquax.b46",
    "fx.loquax.b47",
    "fx.loquax.b48",
    "fx.loquax.b49",
    "fx.loquax.b50",
    "fx.loquax.b51",
    "fx.loquax.b52",
    "fx.loquax.b53",
    "fx.loquax.b54",
    "fx.loquax.b55",
    "fx.loquax.b56",
    "fx.loquax.b57",
    "fx.loquax.b58",
    "fx.loquax.b59",
    "fx.loquax.b60",
    "fx.loquax.b61",
    "fx.loquax.b62",
    "fx.loquax.b63",
];

/// The table's head: every parameter but the band levels.
const HEAD_LEN: usize = 36;
const HEAD: [EffectParamInfo; HEAD_LEN] = [
    // A host's list and the panel's first row: what a vocoder needs first.
    amount("mix", "fx.loquax.mix", 0.0),
    switch("src", "fx.loquax.src", 2.0, 1.0),
    switch("swap", "fx.loquax.swap", 1.0, 0.0),
    p("bands", "fx.loquax.bands", 8.0, 64.0, 32.0, "", false, true),
    p(
        "formant",
        "fx.loquax.formant",
        -24.0,
        24.0,
        0.0,
        "st",
        false,
        false,
    ),
    p("out", "fx.loquax.out", -24.0, 24.0, 0.0, "dB", false, false),
    switch("order", "fx.loquax.order", 2.0, 1.0),
    hz("lo", "fx.loquax.lo", 40.0, 1_000.0, 100.0),
    hz("hi", "fx.loquax.hi", 1_500.0, 18_000.0, 8_000.0),
    p(
        "width",
        "fx.loquax.width",
        0.5,
        2.0,
        1.0,
        "\u{d7}",
        false,
        false,
    ),
    ms("att", "fx.loquax.att", 0.1, 100.0, 2.0),
    ms("rel", "fx.loquax.rel", 5.0, 1_000.0, 40.0),
    p(
        "bshift",
        "fx.loquax.bshift",
        -16.0,
        16.0,
        0.0,
        "",
        false,
        false,
    ),
    p(
        "tilt",
        "fx.loquax.tilt",
        -6.0,
        6.0,
        0.0,
        "dB/oct",
        false,
        false,
    ),
    amount("flat", "fx.loquax.flat", 1.0),
    amount("sib", "fx.loquax.sib", 0.5),
    hz("sibhz", "fx.loquax.sibhz", 2_000.0, 12_000.0, 5_000.0),
    switch("sibgate", "fx.loquax.sibgate", 1.0, 1.0),
    amount("unv", "fx.loquax.unv", 0.5),
    switch("hold", "fx.loquax.hold", 1.0, 0.0),
    amount("spread", "fx.loquax.spread", 0.0),
    amount("cpass", "fx.loquax.cpass", 0.0),
    amount("saw", "fx.loquax.saw", 1.0),
    amount("pulse", "fx.loquax.pulse", 0.0),
    p("pw", "fx.loquax.pw", 0.05, 0.5, 0.3, "%", false, false),
    amount("noise", "fx.loquax.noise", 0.04),
    p("uni", "fx.loquax.uni", 1.0, 7.0, 3.0, "", false, true),
    p(
        "detune",
        "fx.loquax.detune",
        0.0,
        100.0,
        12.0,
        "ct",
        false,
        false,
    ),
    amount("uspread", "fx.loquax.uspread", 0.5),
    p("oct", "fx.loquax.oct", -3.0, 3.0, 0.0, "", false, true),
    ms("satt", "fx.loquax.satt", 0.5, 2_000.0, 5.0),
    ms("srel", "fx.loquax.srel", 5.0, 5_000.0, 180.0),
    p(
        "slevel",
        "fx.loquax.slevel",
        -48.0,
        12.0,
        -6.0,
        "dB",
        false,
        false,
    ),
    switch("robot", "fx.loquax.robot", 1.0, 0.0),
    p(
        "rnote",
        "fx.loquax.rnote",
        24.0,
        96.0,
        48.0,
        "",
        false,
        true,
    ),
    p(
        "rgate",
        "fx.loquax.rgate",
        -80.0,
        0.0,
        -45.0,
        "dB",
        false,
        false,
    ),
];

const fn table() -> [EffectParamInfo; HEAD_LEN + MAX_BANDS] {
    let mut t = [HEAD[0]; HEAD_LEN + MAX_BANDS];
    let mut i = 0;
    while i < HEAD.len() {
        t[i] = HEAD[i];
        i += 1;
    }
    let mut b = 0;
    while b < MAX_BANDS {
        t[HEAD.len() + b] = band(BAND_KEYS[b], BAND_LABELS[b]);
        b += 1;
    }
    t
}

static TABLE: [EffectParamInfo; HEAD_LEN + MAX_BANDS] = table();
pub static PARAMS: &[EffectParamInfo] = &TABLE;

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
        for (b, k) in BAND_KEYS.iter().enumerate() {
            assert_eq!(*k, format!("b{b}"));
            assert!(PARAMS.iter().any(|p| p.key == *k));
        }
        let mix = PARAMS.iter().find(|p| p.key == "mix").unwrap();
        assert_eq!(mix.default, 0.0, "the null: the mix at 0");
    }
}

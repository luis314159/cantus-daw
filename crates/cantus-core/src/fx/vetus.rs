//! Vetus, lo-fi (ADR 0133, provisional): its parameter model. Owned by Vetus; nothing else
//! edits this file.
//!
//! Six modules in a fixed order — **bits** and **rate** (the cheap converter), **wobble**
//! (the speed of a tape or a record), **age** (dropouts and wear), **noise** (crackle, hiss,
//! hum: what the medium adds) and **tone** (the bandwidth of the medium that plays it back,
//! its noise included) — each with its switch, and one **Character** macro that takes every
//! module that is on towards its oldest.
//!
//! Keys are forever once shipped (automation, saved projects and the exported plug-ins' ids
//! hold them). **Every default leaves the signal untouched, to the bit**: every module is off
//! (its settings wait, at a musical value, for its switch), Character is 0, the mix 100 % of
//! an untouched wet path and the output 0 dB.
//!
//! | Key | What | Range | Default |
//! |---|---|---|---|
//! | `mix` | dry/wet | 0 … 100 % | 100 % |
//! | `out` | output gain | −24 … +24 dB | 0 |
//! | `char` | Character: every module that is on, towards its oldest | 0 … 100 % | 0 |
//! | `bits.on` | the quantiser | off / on | off |
//! | `bits.depth` | bits (fractional: the levels are 2^bits) — 24 without dither is a wire | 1 … 24 | 8 |
//! | `bits.dither` | triangular dither of one step before the quantiser | off / on | off |
//! | `bits.mode` | the quantiser's character: rounding, truncation (a cheap converter's), μ-law (a telephone's) | 0 … 2 | rounding |
//! | `rate.on` | the sample-rate reduction (a sample and hold) | off / on | off |
//! | `rate.hz` | the new sample rate | 1 … 48 kHz | 12 kHz |
//! | `rate.filter` | none (aliasing and the hold's images: the sound), anti-alias, anti-alias and smoothing | 0 … 2 | none |
//! | `wob.on` | wow and flutter: a delay moved as Vibrans moves it | off / on | off |
//! | `wow` | the slow pitch swing either way | 0 … 100 ct | 12 ct |
//! | `wow.sync`, `wow.div`, `wow.rate` | the wow on the song (8/1 … 1/32) or free (0.05 … 5 Hz) | | free, 1/1, 0.5 Hz |
//! | `wow.drift` | the share of the wow that wanders (a tape's, not a sine's) | 0 … 100 % | 50 % |
//! | `flut` | the fast pitch flutter either way | 0 … 30 ct | 4 ct |
//! | `flut.rate` | the flutter's rate | 2 … 30 Hz | 9 Hz |
//! | `age.on` | dropouts and wear | off / on | off |
//! | `age.drop` | dropouts: how often and how deep | 0 … 100 % | 30 % |
//! | `age.wear` | worn oxide: the level and the brightness wander slowly | 0 … 100 % | 30 % |
//! | `noise.on` | crackle, hiss and hum | off / on | off |
//! | `crackle` | a record's crackle and pops | 0 … 100 % | 35 % |
//! | `hiss` | the hiss (100 % is −24 dBFS) | 0 … 100 % | 25 % |
//! | `hiss.kind` | tape (bright) or vinyl surface (dark) | 0 / 1 | tape |
//! | `hum` | mains hum and its buzz (100 % is −30 dBFS) | 0 … 100 % | 0 |
//! | `hum.hz` | 50 or 60 Hz | 0 / 1 | 50 Hz |
//! | `noise.follow` | how much the noise follows the music (100 %: only while it plays) | 0 … 100 % | 0 |
//! | `tone.on` | the medium's bandwidth | off / on | off |
//! | `tone.medium` | its colour: tape, radio, telephone, gramophone | 0 … 3 | tape |
//! | `tone.lo`, `tone.hi` | the band's edges | 20 Hz … 2 kHz, 500 Hz … 20 kHz | 50 Hz, 12 kHz |
//! | `tone.res` | the edges' resonance | 0 … 100 % | 20 % |

use crate::effect::{p, EffectParamInfo};

pub const NAME_KEY: &str = "effect.vetus";
pub const SPARSE: bool = false;
pub const TAKES_SIDECHAIN: bool = false;
/// Whether it hears a track's notes ([`crate::EffectSlot::midi_from`], ADR 0022).
pub const TAKES_NOTES: bool = false;
/// The locale prefix of each stepped parameter's choices, by key: choice `i` is
/// `{prefix}.{i}` in the locale files. What the exported plug-in shows a host for them
/// (ADR 0020, `cantus_plugins::params`).
pub const CHOICES: &[(&str, &str)] = &[
    ("bits.on", "vetus.onoff"),
    ("bits.dither", "vetus.onoff"),
    ("bits.mode", "vetus.bits_mode"),
    ("rate.on", "vetus.onoff"),
    ("rate.filter", "vetus.rate_filter"),
    ("wob.on", "vetus.onoff"),
    ("wow.sync", "modfx.sync"),
    ("wow.div", "modfx.div"),
    ("age.on", "vetus.onoff"),
    ("noise.on", "vetus.onoff"),
    ("hiss.kind", "vetus.hiss_kind"),
    ("hum.hz", "vetus.hum_hz"),
    ("tone.on", "vetus.onoff"),
    ("tone.medium", "vetus.medium"),
];

/// The modules' switches, in the signal's order.
pub const SWITCHES: [&str; 6] = [
    "bits.on", "rate.on", "wob.on", "age.on", "noise.on", "tone.on",
];

/// The media of `tone.medium`, and each one's band (low and high edge, Hz): what the
/// editor sets the edges to when a medium is chosen.
pub const MEDIA: usize = 4;
pub const MEDIUM_BANDS: [(f32, f32); MEDIA] = [
    (50.0, 12_000.0), // tape: a cassette's
    (120.0, 4_500.0), // radio: an AM broadcast's
    (300.0, 3_400.0), // telephone: the telephone band
    (180.0, 5_000.0), // gramophone: an acoustic horn's
];

pub const PARAMS: &[EffectParamInfo] = &[
    p("mix", "fx.vetus.mix", 0.0, 1.0, 1.0, "%", false, false),
    p("out", "fx.vetus.out", -24.0, 24.0, 0.0, "dB", false, false),
    p("char", "fx.vetus.char", 0.0, 1.0, 0.0, "%", false, false),
    // Bits.
    p(
        "bits.on",
        "fx.vetus.bits.on",
        0.0,
        1.0,
        0.0,
        "",
        false,
        true,
    ),
    p(
        "bits.depth",
        "fx.vetus.bits.depth",
        1.0,
        24.0,
        8.0,
        "",
        false,
        false,
    ),
    p(
        "bits.dither",
        "fx.vetus.bits.dither",
        0.0,
        1.0,
        0.0,
        "",
        false,
        true,
    ),
    p(
        "bits.mode",
        "fx.vetus.bits.mode",
        0.0,
        2.0,
        0.0,
        "",
        false,
        true,
    ),
    // Rate.
    p(
        "rate.on",
        "fx.vetus.rate.on",
        0.0,
        1.0,
        0.0,
        "",
        false,
        true,
    ),
    p(
        "rate.hz",
        "fx.vetus.rate.hz",
        1_000.0,
        48_000.0,
        12_000.0,
        "Hz",
        true,
        false,
    ),
    p(
        "rate.filter",
        "fx.vetus.rate.filter",
        0.0,
        2.0,
        0.0,
        "",
        false,
        true,
    ),
    // Wobble.
    p("wob.on", "fx.vetus.wob.on", 0.0, 1.0, 0.0, "", false, true),
    p("wow", "fx.vetus.wow", 0.0, 100.0, 12.0, "ct", false, false),
    p("wow.sync", "modfx.p.sync", 0.0, 1.0, 0.0, "", false, true),
    p("wow.div", "modfx.p.div", 0.0, 16.0, 3.0, "", false, true),
    p(
        "wow.rate",
        "fx.vetus.wow.rate",
        0.05,
        5.0,
        0.5,
        "Hz",
        true,
        false,
    ),
    p(
        "wow.drift",
        "fx.vetus.wow.drift",
        0.0,
        1.0,
        0.5,
        "%",
        false,
        false,
    ),
    p("flut", "fx.vetus.flut", 0.0, 30.0, 4.0, "ct", false, false),
    p(
        "flut.rate",
        "fx.vetus.flut.rate",
        2.0,
        30.0,
        9.0,
        "Hz",
        true,
        false,
    ),
    // Age.
    p("age.on", "fx.vetus.age.on", 0.0, 1.0, 0.0, "", false, true),
    p(
        "age.drop",
        "fx.vetus.age.drop",
        0.0,
        1.0,
        0.3,
        "%",
        false,
        false,
    ),
    p(
        "age.wear",
        "fx.vetus.age.wear",
        0.0,
        1.0,
        0.3,
        "%",
        false,
        false,
    ),
    // Noise.
    p(
        "noise.on",
        "fx.vetus.noise.on",
        0.0,
        1.0,
        0.0,
        "",
        false,
        true,
    ),
    p(
        "crackle",
        "fx.vetus.crackle",
        0.0,
        1.0,
        0.35,
        "%",
        false,
        false,
    ),
    p("hiss", "fx.vetus.hiss", 0.0, 1.0, 0.25, "%", false, false),
    p(
        "hiss.kind",
        "fx.vetus.hiss.kind",
        0.0,
        1.0,
        0.0,
        "",
        false,
        true,
    ),
    p("hum", "fx.vetus.hum", 0.0, 1.0, 0.0, "%", false, false),
    p("hum.hz", "fx.vetus.hum.hz", 0.0, 1.0, 0.0, "", false, true),
    p(
        "noise.follow",
        "fx.vetus.noise.follow",
        0.0,
        1.0,
        0.0,
        "%",
        false,
        false,
    ),
    // Tone.
    p(
        "tone.on",
        "fx.vetus.tone.on",
        0.0,
        1.0,
        0.0,
        "",
        false,
        true,
    ),
    p(
        "tone.medium",
        "fx.vetus.tone.medium",
        0.0,
        3.0,
        0.0,
        "",
        false,
        true,
    ),
    p(
        "tone.lo",
        "fx.vetus.tone.lo",
        20.0,
        2_000.0,
        50.0,
        "Hz",
        true,
        false,
    ),
    p(
        "tone.hi",
        "fx.vetus.tone.hi",
        500.0,
        20_000.0,
        12_000.0,
        "Hz",
        true,
        false,
    ),
    p(
        "tone.res",
        "fx.vetus.tone.res",
        0.0,
        1.0,
        0.2,
        "%",
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
        // The contract's placeholder key stays: its plug-in id is blessed.
        assert!(PARAMS.iter().any(|p| p.key == "out"));
    }

    /// The null rule: every module is off, Character is 0, the output 0 dB.
    #[test]
    fn the_defaults_are_a_null() {
        let kind = crate::EffectKind::LoFi;
        for key in SWITCHES {
            assert_eq!(kind.param(key).unwrap().default, 0.0, "{key}");
        }
        assert_eq!(kind.param("char").unwrap().default, 0.0);
        assert_eq!(kind.param("out").unwrap().default, 0.0);
        assert_eq!(kind.param("mix").unwrap().default, 1.0);
    }

    /// Every choice list names a stepped key of the table, and every switch has one.
    #[test]
    fn every_choice_list_names_a_stepped_key() {
        for (key, _) in CHOICES {
            let info = PARAMS.iter().find(|p| p.key == *key).expect(key);
            assert!(info.stepped, "{key}");
        }
        for key in SWITCHES {
            assert!(CHOICES.iter().any(|(k, _)| *k == key), "{key}");
        }
    }

    /// The media's bands sit inside the edges' ranges.
    #[test]
    fn every_medium_band_is_in_range() {
        let kind = crate::EffectKind::LoFi;
        let (lo, hi) = (
            kind.param("tone.lo").unwrap(),
            kind.param("tone.hi").unwrap(),
        );
        assert_eq!(kind.param("tone.medium").unwrap().max as usize + 1, MEDIA);
        for (a, b) in MEDIUM_BANDS {
            assert!(lo.min <= a && a <= lo.max && hi.min <= b && b <= hi.max);
        }
    }
}

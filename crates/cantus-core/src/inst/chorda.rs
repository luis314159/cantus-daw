//! Chorda — the modelled grand piano: its parameter model (ADR 0059). Owned by this
//! instrument's builder (ADR 0023).
//!
//! Values in the instrument's own units: A4 in Hz, the unison spread in cents, levels and
//! the tilt in dB; shares (stretch, stiffness, decay, sympathy…) as fractions shown in %,
//! where 1 (100 %) is the model as measured. Keys are forever once shipped.
//!
//! | Group | Keys |
//! |---|---|
//! | Model | `model` (1: the first model; a later one never changes a song made with this), `a4` (Hz), `stretch` (%), `stiffness` (%) |
//! | Hammer | `hardness` (−1 softer … 1 harder felt), `hysteresis` (%), `dynamics` (%), `vel.curve` |
//! | Strings | `decay` (%), `unison` (ct), `coupling` (%) |
//! | Resonance | `sympathy` (%), `phantom` (%) |
//! | Pedals | `pedal` (the sustain pedal as a parameter, 0 up … 1 down: what an automation lane or a modulator drives; the higher of it and controller 64 plays), `damper` (%), `una` (%: how far the soft pedal shifts the hammers) |
//! | Body | `body` (%), `tilt` (dB), `width` (%) |
//! | Output | `gain` (dB) |
//! | Voicing by hand (model 2; r42 `chorda-presets`, ADR 0098's eighth amendment; every default the shipped voicing to the bit) | Hammers `voicing.felt_bass`, `voicing.felt_treble` (−1 … 1, as `hardness` at A0 and C8), `voicing.mass` (%); Strings `voicing.decay_bass`, `.decay_mid`, `.decay_treble` (%), `voicing.hf` (%: the overtones' sustain), `voicing.hf_spread` (%), `voicing.uneven` (%: the unison's unevenness), `voicing.polar` (%: the second polarisation); Soundboard `voicing.formant_hz` (Hz), `.formant_db` (dB), `.formant_q`; Action `voicing.knock` (%), `voicing.knock_tone` (oct), `voicing.key_return` (%), `voicing.damper` (%: the dampers' strength), `voicing.damper_bass` (%), `voicing.damped` (dB: the floor a damped note ends at); Microphones `voicing.itd` (%: the arrival-time spread); Tone `voicing.top_hz` (Hz: the top roll-off), `voicing.amount` (%: each key's own partial voicing) |
//! | Model 2 (ADR 0098) | `room` (%: the room's level), `noise` (%: the hammer's, the action's and the dampers' noises), `persp` (%: the microphones, over the hammers 0 … the audience 1), `lid` (%: closed 0 … open 1), `roomsize` (%), `colour` (%: Chorda's own voicing 0 … a recorded grand's middle 1, −1 further its own way; r42) — model 1 ignores them; `width` is model 2's pair width too |
//!
//! `a4`, `stretch` and `stiffness` shape every string's dispersion design, computed off the
//! audio thread: they are set, not modulated (a modulation route to them does nothing).

use super::{find, InstParamInfo};
use crate::effect::p;
use crate::model::TrackIcon;

pub const NAME_KEY: &str = "instrument.chorda";
pub const ICON: TrackIcon = TrackIcon::Keys;
/// Offered in the instrument picker (round 28: finished, ADR 0059).
pub const READY: bool = true;

/// The model a new instance plays (a key absent from a spec reads as this). Round 37
/// (ADR 0098): 2. Every spec saved before says `model` = 1 (schema 54's migration).
pub const MODEL: f32 = 2.0;

/// Set, never automated or modulated: the model a song was made with.
pub const SET_KEYS: [&str; 1] = ["model"];

/// The keys a design is computed from: never modulated.
pub const DESIGN_KEYS: [&str; 3] = ["a4", "stretch", "stiffness"];

pub const PARAMS: &[InstParamInfo] = &[
    p("model", "chorda.model", 1.0, 2.0, MODEL, "", false, true),
    p("a4", "chorda.a4", 415.0, 466.0, 440.0, "Hz", false, false),
    p(
        "stretch",
        "chorda.stretch",
        0.0,
        2.0,
        1.0,
        "%",
        false,
        false,
    ),
    p(
        "stiffness",
        "chorda.stiffness",
        0.5,
        1.5,
        1.0,
        "%",
        false,
        false,
    ),
    p(
        "hardness",
        "chorda.hardness",
        -1.0,
        1.0,
        0.0,
        "%",
        false,
        false,
    ),
    p(
        "hysteresis",
        "chorda.hysteresis",
        0.0,
        0.98,
        0.9,
        "%",
        false,
        false,
    ),
    p(
        "dynamics",
        "chorda.dynamics",
        0.5,
        1.5,
        1.0,
        "%",
        false,
        false,
    ),
    p(
        "vel.curve",
        "chorda.vel_curve",
        0.4,
        2.5,
        1.0,
        "",
        true,
        false,
    ),
    p("decay", "chorda.decay", 0.25, 4.0, 1.0, "%", true, false),
    p("unison", "chorda.unison", 0.0, 8.0, 1.0, "ct", false, false),
    p(
        "coupling",
        "chorda.coupling",
        0.0,
        0.9,
        0.5,
        "%",
        false,
        false,
    ),
    p(
        "sympathy",
        "chorda.sympathy",
        0.0,
        2.0,
        1.0,
        "%",
        false,
        false,
    ),
    p(
        "phantom",
        "chorda.phantom",
        0.0,
        3.0,
        1.0,
        "%",
        false,
        false,
    ),
    p("pedal", "chorda.pedal", 0.0, 1.0, 0.0, "%", false, false),
    p("damper", "chorda.damper", 0.0, 1.0, 1.0, "%", false, false),
    p("una", "chorda.una", 0.0, 1.0, 1.0, "%", false, false),
    p("body", "chorda.body", 0.0, 2.0, 1.0, "%", false, false),
    p("tilt", "chorda.tilt", -12.0, 12.0, 0.0, "dB", false, false),
    p("width", "chorda.width", 0.0, 1.0, 0.7, "%", false, false),
    p("gain", "inst.gain", -48.0, 12.0, 0.0, "dB", false, false),
    p("room", "chorda.room", 0.0, 1.0, 0.35, "%", false, false),
    p("noise", "chorda.noise", 0.0, 2.0, 1.0, "%", false, false),
    // Model 2's perspective (round 37, voicing pass two): the microphones from over the
    // hammers (0) through the player (35 %) to the audience (1), the lid closed (0) … open
    // (1), the room's size (40 %: as voiced). r37:piano
    p("persp", "chorda.persp", 0.0, 1.0, 0.35, "%", false, false),
    p("lid", "chorda.lid", 0.0, 1.0, 1.0, "%", false, false),
    p(
        "roomsize",
        "chorda.roomsize",
        0.0,
        1.0,
        0.4,
        "%",
        false,
        false,
    ),
    // r42 (`chorda-c17`, ADR 0098's sixth amendment): model 2's colour — Chorda's own
    // voicing (0) toward a recorded grand's middle (1), or further its own way (−1). A key
    // absent from an older song reads 0: its sound unchanged.
    p("colour", "chorda.colour", -1.0, 1.0, 0.0, "%", false, false),
    // r42 (`chorda-presets`, ADR 0098's eighth amendment): model 2's voicing by hand — every
    // default the shipped voicing's own number (candidate 18 to the bit; absent from an older
    // song, the same). The value-controls' defaults are the shipped values
    // (`cantus-engine`'s `the_voicing_controls_defaults_are_the_shipped_voicing`).
    p(
        "voicing.felt_bass",
        "chorda.v.felt_bass",
        -1.0,
        1.0,
        0.0,
        "%",
        false,
        false,
    ),
    p(
        "voicing.felt_treble",
        "chorda.v.felt_treble",
        -1.0,
        1.0,
        0.0,
        "%",
        false,
        false,
    ),
    p(
        "voicing.mass",
        "chorda.v.mass",
        0.5,
        2.0,
        1.0,
        "%",
        false,
        false,
    ),
    p(
        "voicing.decay_bass",
        "chorda.v.decay_bass",
        0.25,
        4.0,
        1.0,
        "%",
        true,
        false,
    ),
    p(
        "voicing.decay_mid",
        "chorda.v.decay_mid",
        0.25,
        4.0,
        1.0,
        "%",
        true,
        false,
    ),
    p(
        "voicing.decay_treble",
        "chorda.v.decay_treble",
        0.25,
        4.0,
        1.0,
        "%",
        true,
        false,
    ),
    p(
        "voicing.hf",
        "chorda.v.hf",
        0.25,
        4.0,
        1.0,
        "%",
        true,
        false,
    ),
    p(
        "voicing.hf_spread",
        "chorda.v.hf_spread",
        0.0,
        2.0,
        1.0,
        "%",
        false,
        false,
    ),
    p(
        "voicing.uneven",
        "chorda.v.uneven",
        0.0,
        3.0,
        1.0,
        "%",
        false,
        false,
    ),
    p(
        "voicing.polar",
        "chorda.v.polar",
        0.0,
        4.0,
        1.0,
        "%",
        false,
        false,
    ),
    p(
        "voicing.formant_hz",
        "chorda.v.formant_hz",
        150.0,
        1_500.0,
        600.0,
        "Hz",
        true,
        false,
    ),
    p(
        "voicing.formant_db",
        "chorda.v.formant_db",
        -12.0,
        24.0,
        12.0,
        "dB",
        false,
        false,
    ),
    p(
        "voicing.formant_q",
        "chorda.v.formant_q",
        0.5,
        8.0,
        3.0,
        "",
        true,
        false,
    ),
    p(
        "voicing.knock",
        "chorda.v.knock",
        0.0,
        4.0,
        1.0,
        "%",
        false,
        false,
    ),
    p(
        "voicing.knock_tone",
        "chorda.v.knock_tone",
        -2.0,
        2.0,
        0.0,
        "oct",
        false,
        false,
    ),
    p(
        "voicing.key_return",
        "chorda.v.key_return",
        0.0,
        3.0,
        1.0,
        "%",
        false,
        false,
    ),
    p(
        "voicing.damper",
        "chorda.v.damper",
        0.25,
        4.0,
        1.0,
        "%",
        true,
        false,
    ),
    p(
        "voicing.damper_bass",
        "chorda.v.damper_bass",
        0.25,
        4.0,
        1.0,
        "%",
        true,
        false,
    ),
    p(
        "voicing.damped",
        "chorda.v.damped",
        -120.0,
        -30.0,
        -70.0,
        "dB",
        false,
        false,
    ),
    p(
        "voicing.itd",
        "chorda.v.itd",
        0.0,
        3.0,
        1.0,
        "%",
        false,
        false,
    ),
    p(
        "voicing.top_hz",
        "chorda.v.top_hz",
        2_000.0,
        20_000.0,
        TOP_HZ,
        "Hz",
        true,
        false,
    ),
    p(
        "voicing.amount",
        "chorda.v.amount",
        0.0,
        2.0,
        1.0,
        "%",
        false,
        false,
    ),
];

/// The shipped voicing's top roll-off (candidate 16's 11 059.2 Hz, as an `f32`): the
/// `voicing.top_hz` control's default, which plays the voicing's own number to the bit.
pub const TOP_HZ: f32 = 11_059.2;

/// r42 (`chorda-presets`): the voicing controls an editor groups (ADR 0098's eighth
/// amendment), each group with Chorda's own controls that belong there — the musician's
/// order: hammers, strings, soundboard and case, action, microphones, tone.
pub const VOICING_GROUPS: [(&str, &[&str]); 6] = [
    (
        "chorda.v.hammers",
        &[
            "hardness",
            "voicing.felt_bass",
            "voicing.felt_treble",
            "voicing.mass",
        ],
    ),
    (
        "chorda.v.strings",
        &[
            "decay",
            "voicing.decay_bass",
            "voicing.decay_mid",
            "voicing.decay_treble",
            "voicing.hf",
            "voicing.hf_spread",
            "unison",
            "voicing.uneven",
            "voicing.polar",
        ],
    ),
    (
        "chorda.v.board",
        &[
            "body",
            "voicing.formant_hz",
            "voicing.formant_db",
            "voicing.formant_q",
            "coupling",
            "sympathy",
        ],
    ),
    (
        "chorda.v.action",
        &[
            "noise",
            "voicing.knock",
            "voicing.knock_tone",
            "voicing.key_return",
            "voicing.damper",
            "voicing.damper_bass",
            "voicing.damped",
        ],
    ),
    (
        "chorda.v.mics",
        &["persp", "voicing.itd", "width", "lid", "room", "roomsize"],
    ),
    (
        "chorda.v.tone",
        &["voicing.top_hz", "tilt", "colour", "voicing.amount"],
    ),
];

pub fn params() -> &'static [InstParamInfo] {
    PARAMS
}

/// r42:tuning — whether Chorda plays in its track's tuning (ADR 0155): when the resolved
/// tuning's table is not 12-TET at A440 to the bit. Then that tuning is the diapason and its
/// per-key offsets are designed in; Chorda's own `a4` is not used. Otherwise (every older
/// song, a track playing `Equal`) Chorda's `a4` is the diapason, as before. `stretch`,
/// `stiffness` and `unison` are Chorda's own in every case, on top of the offsets.
pub fn follows_tuning(table: &crate::tuning::TuningTable) -> bool {
    !table.is_equal()
}

/// r42:tuning — the diapason Chorda plays at (what the engine designs and the editor shows):
/// its own `a4` when its track's resolved tuning leaves every key at 12-TET A440
/// ([`follows_tuning`]), the tuning's `a4_hz` otherwise.
pub fn effective_a4(spec: &super::NativeSpec, resolved: &crate::tuning::Tuning) -> f64 {
    if follows_tuning(&resolved.offsets()) {
        resolved.a4_hz
    } else {
        f64::from(spec.get("a4"))
    }
}

/// The modulation sources it produces (tags from `modulation::SOURCES`).
pub const SOURCES: &[&str] = &[
    "macro1", "macro2", "macro3", "macro4", "macro5", "macro6", "macro7", "macro8", "vel", "key",
    "mw", "at", "expr", "sus",
];

pub fn param(key: &str) -> Option<&'static InstParamInfo> {
    find(PARAMS, key).or_else(|| super::modulation::param(key))
}

pub fn is_text_key(key: &str) -> bool {
    // The look its scene draws (ADR 0104): text, never a parameter. r37:scene
    super::chorda_look::KEYS.contains(&key) || super::modulation::is_text_key(key)
}

pub fn is_asset_key(key: &str) -> bool {
    let _ = key;
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_key_is_unique_and_its_default_inside_its_range() {
        let mut seen = std::collections::HashSet::new();
        for i in PARAMS {
            assert!(seen.insert(i.key), "{} twice", i.key);
            assert!(i.min <= i.default && i.default <= i.max, "{}", i.key);
        }
        for k in DESIGN_KEYS {
            assert!(param(k).is_some(), "{k}");
        }
        assert_eq!(param("gain").unwrap().default, 0.0);
    }

    /// r42 (`chorda-presets`): every voicing control is in a group once, every group's key is
    /// a parameter, and each is automatable (MIDI learn takes what automation does).
    #[test]
    fn every_voicing_control_is_grouped_once_and_automatable() {
        let mut seen = std::collections::HashSet::new();
        for (_, keys) in VOICING_GROUPS {
            for k in keys {
                assert!(param(k).is_some(), "{k}");
                assert!(seen.insert(*k), "{k} in two groups");
                assert!(crate::InstrumentKind::Chorda.automatable(k), "{k}");
            }
        }
        for i in PARAMS.iter().filter(|i| i.key.starts_with("voicing.")) {
            assert!(seen.contains(i.key), "{} in no group", i.key);
        }
    }
}

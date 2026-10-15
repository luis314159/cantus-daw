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
//! | Pedals | `damper` (%), `una` (%: how far the soft pedal shifts the hammers) |
//! | Body | `body` (%), `tilt` (dB), `width` (%) |
//! | Output | `gain` (dB) |
//!
//! `a4`, `stretch` and `stiffness` shape every string's dispersion design, computed off the
//! audio thread: they are set, not modulated (a modulation route to them does nothing).

use super::{find, InstParamInfo};
use crate::effect::p;
use crate::model::TrackIcon;

pub const NAME_KEY: &str = "instrument.chorda";
pub const ICON: TrackIcon = TrackIcon::Keys;
/// Offered in the instrument picker once it is finished.
pub const READY: bool = false;

/// The model a new instance plays (a key absent from a spec reads as this).
pub const MODEL: f32 = 1.0;

/// The keys a design is computed from: never modulated.
pub const DESIGN_KEYS: [&str; 3] = ["a4", "stretch", "stiffness"];

pub const PARAMS: &[InstParamInfo] = &[
    p("model", "chorda.model", 1.0, 1.0, MODEL, "", false, true),
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
    p("damper", "chorda.damper", 0.0, 1.0, 1.0, "%", false, false),
    p("una", "chorda.una", 0.0, 1.0, 1.0, "%", false, false),
    p("body", "chorda.body", 0.0, 2.0, 1.0, "%", false, false),
    p("tilt", "chorda.tilt", -12.0, 12.0, 0.0, "dB", false, false),
    p("width", "chorda.width", 0.0, 1.0, 0.7, "%", false, false),
    p("gain", "inst.gain", -48.0, 12.0, 0.0, "dB", false, false),
];

pub fn params() -> &'static [InstParamInfo] {
    PARAMS
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
    super::modulation::is_text_key(key)
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
}

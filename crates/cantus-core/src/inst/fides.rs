//! Fides — the resonator instrument (round 41, ADR 0150): its parameter model. Owned by
//! the `fides` builder; the contract declares the kind, its stub and its keys' family.
//!
//! An exciter (pluck, pick, a bow by a friction curve, a mallet, noise, a live input) into
//! a resonator (waveguide strings after Gravis's, modal bars, plates, membranes and tubes)
//! with a body: the class of Ableton's Collision and Tension, Logic's Sculpture and AAS
//! Chromaphone (BACKLOG §3e item 6, §6l A2). The name is the Latin for a lyre's strings —
//! a stringed instrument — and for faith.
//!
//! **The contract's stub** makes no sound and has one parameter, `gain`: the builder writes
//! the real table (keys are forever once shipped) and adds the kind to
//! [`super::InstrumentKind::ALL`] in the commit that makes it sound, as Tympanum did.
//!
//! | Key | What | Range | Default |
//! |---|---|---|---|
//! | `gain` | output level | −48 … +12 dB | 0 |

use super::{find, InstParamInfo};
use crate::effect::p;
use crate::model::TrackIcon;

pub const NAME_KEY: &str = "instrument.fides";
pub const ICON: TrackIcon = TrackIcon::Strings;
/// Offered in the instrument picker once it is built (the `fides` builder sets it).
pub const READY: bool = false;
/// The model version stored with every new instance (ADR 0059's rule): an improved model
/// is a new number, and a song keeps the one it was made with.
pub const MODEL: f32 = 1.0;

pub const PARAMS: &[InstParamInfo] =
    &[p("gain", "inst.gain", -48.0, 12.0, 0.0, "dB", false, false)];

pub fn params() -> &'static [InstParamInfo] {
    PARAMS
}

/// The modulation sources it produces (tags from `modulation::SOURCES`).
pub const SOURCES: &[&str] = &[
    "macro1", "macro2", "macro3", "macro4", "macro5", "macro6", "macro7", "macro8", "vel", "key",
    "mw", "at", "pb", "expr", "sus",
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

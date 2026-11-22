//! Arcus — the bowed string (round 42, ADR 0157): its parameter model. Owned by the `arcus`
//! builder; the contract declares the kind, its stub and its keys' family.
//!
//! A modelled bowed string on Fides's bow, waveguide and body: the cello and the double bass
//! first (arco, legato, détaché, pizzicato, tremolo, vibrato from the body), the violin and
//! the viola if they pass the same gate (BACKLOG §3e, *Strings and winds*). The name is the
//! Latin for the bow.
//!
//! **The contract's stub** makes no sound and has one parameter, `gain`: the builder writes
//! the real table (keys are forever once shipped) and adds the kind to
//! [`super::InstrumentKind::ALL`] in the commit that makes it sound, as Fides did.
//!
//! | Key | What | Range | Default |
//! |---|---|---|---|
//! | `gain` | output level | −48 … +12 dB | 0 |

use super::{find, InstParamInfo};
use crate::effect::p;
use crate::model::TrackIcon;

pub const NAME_KEY: &str = "instrument.arcus";
pub const ICON: TrackIcon = TrackIcon::Strings;
/// Offered in the instrument picker once it is built (the `arcus` builder sets it).
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

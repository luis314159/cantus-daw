//! Chorda — the modelled grand piano: its parameter model. Owned by this instrument's
//! builder (ADR 0023).
//!
//! **Round 28's contract**: a scaffold — only the output gain every instrument keeps —
//! until its builder writes the real table. Keys are forever once shipped.
//!
//! | Group | Keys |
//! |---|---|
//! | Output | `gain` (dB) |

use super::{find, InstParamInfo};
use crate::effect::p;
use crate::model::TrackIcon;

pub const NAME_KEY: &str = "instrument.chorda";
pub const ICON: TrackIcon = TrackIcon::Keys;
/// Offered in the instrument picker once it is built: the scaffold stays out of the menu.
pub const READY: bool = false;

pub const PARAMS: &[InstParamInfo] =
    &[p("gain", "inst.gain", -48.0, 12.0, 0.0, "dB", false, false)];

pub fn params() -> &'static [InstParamInfo] {
    PARAMS
}

/// The modulation sources it produces (tags from `modulation::SOURCES`): none yet.
pub const SOURCES: &[&str] = &[];

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

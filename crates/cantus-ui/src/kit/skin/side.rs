//! The fx side round's faces (provisional ADRs 0130–0133). **The contract's placeholders**:
//! each new unit wears a sibling's look under a key of its own, so every unit keeps a sheet
//! of its own (`tests/skins.rs`) until its builder draws the real face. The looks are
//! borrowed from the tone family, whose tests compare its sheets among themselves only (the
//! dynamics and modulation families refuse a stranger sharing a hue and a material with
//! theirs). Each builder replaces its own sheet here — and only its own — with one that
//! differs from every other unit's in hue or material, display included.

use super::{tone, Skin};
use cantus_core::EffectKind;

/// Cribrum, the auto filter: for now Aequalis's look (a filter is the EQ's cousin).
pub const CRIBRUM: Skin = Skin {
    key: "fx.cribrum",
    ..tone::AEQUALIS
};

/// Fragor, the pedals: for now Fervor's look (drive and colour).
pub const FRAGOR: Skin = Skin {
    key: "fx.fragor",
    ..tone::FERVOR
};

/// Lapsus, the time shaper: for now Delay's look (the song's recent past, drawn in time).
pub const LAPSUS: Skin = Skin {
    key: "fx.lapsus",
    ..tone::DELAY
};

/// Vetus, lo-fi: for now Speculum's look.
pub const VETUS: Skin = Skin {
    key: "fx.vetus",
    ..tone::SPECULUM
};

/// Libra, the utility: for now Latitudo's look (the stereo utility).
pub const LIBRA: Skin = Skin {
    key: "fx.libra",
    ..tone::LATITUDO
};

/// The sheet of a unit of the side round.
pub fn of(kind: EffectKind) -> Option<&'static Skin> {
    match kind {
        EffectKind::AutoFilter => Some(&CRIBRUM),
        EffectKind::Pedal => Some(&FRAGOR),
        EffectKind::TimeShaper => Some(&LAPSUS),
        EffectKind::LoFi => Some(&VETUS),
        EffectKind::Utility => Some(&LIBRA),
        _ => None,
    }
}

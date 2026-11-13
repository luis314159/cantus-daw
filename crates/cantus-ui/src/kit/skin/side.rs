//! The fx side round's faces (provisional ADRs 0130–0133). **The contract's placeholders**:
//! each new unit wears a sibling's look under a key of its own, so every unit keeps a sheet
//! of its own (`tests/skins.rs`) until its builder draws the real face. The looks are
//! borrowed from the tone family, whose tests compare its sheets among themselves only (the
//! dynamics and modulation families refuse a stranger sharing a hue and a material with
//! theirs). Each builder replaces its own sheet here — and only its own — with one that
//! differs from every other unit's in hue or material, display included.

use super::super::color::Hsl;
use super::{
    tone, Case, Display, KnobShape, Lettering, Material, Ornaments, Pair, Plate, Skin, SwitchShape,
    Tint, Weight,
}; // fx:fragor

const fn hsl(h: f32, s: f32, l: f32) -> Hsl {
    Hsl { h, s, l }
} // fx:fragor
use cantus_core::EffectKind;

/// Cribrum, the auto filter: for now Aequalis's look (a filter is the EQ's cousin).
pub const CRIBRUM: Skin = Skin {
    key: "fx.cribrum",
    ..tone::AEQUALIS
};

// fx:fragor — Fragor's own face (ADR 0131): a stage at night — a matte black board
// under a magenta-red accent with a cyan second voice, the sound drawn in phosphor, skirted
// knobs, rocker switches, engraved capitals and screws. Its pedals wear their own finishes
// (`skin/pedals.rs`).
pub const FRAGOR: Skin = Skin {
    key: "fx.fragor",
    surface: Tint {
        hue: 250.0,
        sat: 0.14,
        amount: 0.40,
    },
    accent: Some(Pair {
        dark: hsl(338.0, 0.86, 0.63),
        light: hsl(338.0, 0.74, 0.42),
    }),
    accent2: Some(Pair {
        dark: hsl(186.0, 0.72, 0.58),
        light: hsl(192.0, 0.82, 0.30),
    }),
    material: Material::Matte,
    display: Display::Phosphor,
    knob: KnobShape::Skirted,
    switch: SwitchShape::Rocker,
    lettering: Lettering {
        case: Case::Upper,
        tracking: 0.12,
        weight: Weight::SemiBold,
        condensed: true,
        mono_values: false,
    },
    plate: Plate::Engraved,
    ornaments: Ornaments {
        screws: true,
        vents: false,
        rules: true,
        rack_ears: false,
    },
}; // fx:fragor

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

//! Fides's face (round 41, ADR 0150): a luthier's bench — a lyre's and a marimba's warm
//! rosewood, gold for the strings and the struck bars, a teal for what rings, the
//! instrument drawn on a plan's paper as its display; capped knobs, pill switches and an
//! engraved plate. Owned by the `fides` builder; one sheet in the house's family
//! (`instruments.rs` lists it), never a copy of a product's dress.

use super::{
    Case, Display, KnobShape, Lettering, Material, Pair, Plate, Skin, SwitchShape, Tint, Weight,
};
use crate::kit::color::Hsl;

const fn pair(h: f32, s: f32, dark_l: f32, light_l: f32) -> Pair {
    Pair {
        dark: Hsl { h, s, l: dark_l },
        light: Hsl { h, s, l: light_l },
    }
}

/// Fides: rosewood and gold, the ringing partials in teal.
pub const FIDES: Skin = Skin {
    key: "inst.fides",
    surface: Tint {
        hue: 22.0,
        sat: 0.16,
        amount: 0.8,
    },
    accent: Some(pair(38.0, 0.78, 0.66, 0.30)),
    accent2: Some(pair(176.0, 0.60, 0.62, 0.27)),
    material: Material::Wood,
    display: Display::Paper,
    knob: KnobShape::Cap,
    switch: SwitchShape::Pill,
    lettering: Lettering {
        case: Case::AsIs,
        tracking: 0.06,
        weight: Weight::Medium,
        condensed: false,
        mono_values: false,
    },
    plate: Plate::Engraved,
    ornaments: super::Ornaments {
        screws: false,
        vents: false,
        rules: true,
        rack_ears: false,
    },
};

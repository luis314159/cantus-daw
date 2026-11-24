//! Arcus's face (round 42, ADR 0157) — **design sheet v2, for the owner's approval**: a
//! cello's case — varnished maple (its hue a maple's 32°, not a rose's), a garnet accent for the bow and what it presses
//! (the varnish's red, cooler than Fides's copper and every other wood's gold), an amber
//! second accent for the rosin's region (where the Helmholtz motion holds), the instrument
//! drawn under glass as its display; skirted knobs, pill switches and an engraved plate.
//! Owned by the `arcus` builder; one sheet in the house's family (`instruments.rs` lists
//! it), never a copy of a product's dress.

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

/// Arcus: varnished maple, the bow in garnet (335°: 33° from Fides's copper, the nearest
/// wood; the house rule keeps no two faces of one material within 20°), the rosin in amber.
pub const ARCUS: Skin = Skin {
    key: "inst.arcus",
    surface: Tint {
        hue: 32.0,
        sat: 0.20,
        amount: 0.85,
    },
    accent: Some(pair(335.0, 0.62, 0.74, 0.34)),
    accent2: Some(pair(40.0, 0.78, 0.64, 0.30)),
    material: Material::Wood,
    display: Display::Glass,
    knob: KnobShape::Skirted,
    switch: SwitchShape::Pill,
    lettering: Lettering {
        case: Case::AsIs,
        tracking: 0.05,
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

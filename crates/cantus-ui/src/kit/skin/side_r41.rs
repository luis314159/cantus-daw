//! r41 — round 41's faces: Nitor (ADR 0151; design sheet v1, for the owner's approval).
//!
//! **Nitor, the morning star**: a pearl lacquer with a rose blush around a dark glass — the
//! corona — where the one knob sits like a sun; its rays are the harmonics the exciter
//! adds (sun gold), its ring the character's four bands (gold where it lifts, dawn rose where
//! it holds back); cap knobs, pill switches, wide-spaced light capitals. Its voices —
//! dawn rose over sun gold on lacquer — are no other unit's (`side.rs`'s tests).

use super::{
    chsl, Case, Display, KnobShape, Lettering, Material, Ornaments, Pair, Plate, Skin,
    SwitchShape, Tint, Weight,
};

/// Nitor's sheet.
pub const NITOR: Skin = Skin {
    key: "fx.nitor",
    surface: Tint {
        hue: 336.0,
        sat: 0.12,
        amount: 0.22,
    },
    // Dawn rose leads (arcs, lamps, the chosen facet's edge); sun gold is the second voice
    // (the rays, the tonic): a gold lead on lacquer is Vinculum's.
    accent: Some(Pair {
        dark: chsl(330.0, 0.66, 0.70),
        light: chsl(330.0, 0.62, 0.38),
    }),
    accent2: Some(Pair {
        dark: chsl(46.0, 0.92, 0.62),
        light: chsl(40.0, 0.88, 0.33),
    }),
    material: Material::Lacquer,
    display: Display::Phosphor,
    knob: KnobShape::Cap,
    switch: SwitchShape::Pill,
    lettering: Lettering {
        case: Case::Upper,
        tracking: 0.16,
        weight: Weight::Regular,
        condensed: false,
        mono_values: false,
    },
    plate: Plate::Badge,
    ornaments: Ornaments {
        screws: false,
        vents: false,
        rules: true,
        rack_ears: false,
    },
};

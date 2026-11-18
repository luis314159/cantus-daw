//! fxc — wave C's faces (Purus, Comparatio; ADRs 0142–0143), each with its
//! colourways. Every display dark in both themes; fine strokes; no vents. Each face's voices
//! sit apart from every other unit's of the side round (`side.rs`'s tests), wave B's
//! (Vinculum, Lenis, Cedo) included.

use super::{
    chsl, Case, Colourway, Display, KnobShape, Lettering, Material, Ornaments, Pair, Plate, Skin,
    SwitchShape, Tint, Weight,
};

/// Purus, the restoration bench: a precision instrument in machined metal on a restorer's
/// cutting mat — its screen a spectrogram, what is taken out a cool residue falling out of a
/// warm picture, the hum's harmonics a comb of amber needles; dial knobs, bat toggles, an
/// engraved plate, condensed capitals, mono values.
const fn purus(
    key: &'static str,
    material: Material,
    surface: Tint,
    accent: Pair,
    second: Pair,
) -> Skin {
    Skin {
        key,
        surface,
        accent: Some(accent),
        accent2: Some(second),
        material,
        display: Display::Scope,
        knob: KnobShape::Dial,
        switch: SwitchShape::Bat,
        lettering: Lettering {
            case: Case::Upper,
            tracking: 0.10,
            weight: Weight::Medium,
            condensed: true,
            mono_values: true,
        },
        plate: Plate::Engraved,
        ornaments: Ornaments {
            screws: true,
            vents: false,
            rules: true,
            rack_ears: false,
        },
    }
}

/// **Champagne**: champagne enamel hammered over metal on a green cutting mat; the residue
/// a cool blue, the hum's needles amber-yellow.
pub const PURUS: Skin = purus(
    "fx.purus",
    Material::Hammered,
    Tint {
        hue: 44.0,
        sat: 0.16,
        amount: 0.30,
    },
    Pair {
        dark: chsl(212.0, 0.70, 0.66),
        light: chsl(212.0, 0.72, 0.32),
    },
    Pair {
        dark: chsl(64.0, 0.86, 0.62),
        light: chsl(64.0, 0.80, 0.36),
    },
);
/// **Surgical**: white enamel on a navy mat; the residue a green, the needles coral.
pub const PURUS_SURGICAL: Skin = purus(
    "fx.purus.surgical",
    Material::Matte,
    Tint {
        hue: 210.0,
        sat: 0.05,
        amount: 0.12,
    },
    Pair {
        dark: chsl(128.0, 0.62, 0.60),
        light: chsl(128.0, 0.66, 0.28),
    },
    Pair {
        dark: chsl(14.0, 0.82, 0.68),
        light: chsl(14.0, 0.74, 0.40),
    },
);
/// **Graphite**: graphite carbon on a slate mat; the residue a cold periwinkle, the needles lime.
pub const PURUS_GRAPHITE: Skin = purus(
    "fx.purus.graphite",
    Material::Carbon,
    Tint {
        hue: 220.0,
        sat: 0.06,
        amount: 0.22,
    },
    // fx/final — 214° -> 246°, a cold periwinkle: the X-Y pad (Tactus, carbon) holds 222° and
    // Cribrum 190° (the house rule: 20° apart on one material).
    Pair {
        dark: chsl(246.0, 0.66, 0.74),
        light: chsl(246.0, 0.62, 0.36),
    },
    Pair {
        dark: chsl(80.0, 0.70, 0.60),
        light: chsl(80.0, 0.66, 0.28),
    },
);
pub const PURUS_COLOURWAYS: [Colourway; 3] = [
    Colourway {
        id: "champagne",
        name_key: "purus.colourway.champagne",
        sheet: &PURUS,
    },
    Colourway {
        id: "surgical",
        name_key: "purus.colourway.surgical",
        sheet: &PURUS_SURGICAL,
    },
    Colourway {
        id: "graphite",
        name_key: "purus.colourway.graphite",
        sheet: &PURUS_GRAPHITE,
    },
];

/// Comparatio, the reference comparator: a two-deck console between wooden cheeks — your
/// mix's deck lit in one voice, the reference's in the other, the screen between them dark;
/// skirted knobs, lamp switches, a legend plate, capitals as a console prints them.
const fn comparatio(
    key: &'static str,
    material: Material,
    surface: Tint,
    accent: Pair,
    second: Pair,
) -> Skin {
    Skin {
        key,
        surface,
        accent: Some(accent),
        accent2: Some(second),
        material,
        display: Display::Vfd,
        knob: KnobShape::Skirted,
        switch: SwitchShape::Lamp,
        lettering: Lettering {
            case: Case::Upper,
            tracking: 0.14,
            weight: Weight::SemiBold,
            condensed: false,
            mono_values: true,
        },
        plate: Plate::Legend,
        ornaments: Ornaments {
            screws: true,
            vents: false,
            rules: true,
            rack_ears: false,
        },
    }
}

/// **Walnut**: walnut cheeks around a smoked console; your mix rose, the reference blue.
pub const COMPARATIO: Skin = comparatio(
    "fx.comparatio",
    Material::Wood,
    Tint {
        hue: 26.0,
        sat: 0.22,
        amount: 0.30,
    },
    Pair {
        dark: chsl(338.0, 0.88, 0.62),
        light: chsl(338.0, 0.84, 0.36),
    },
    Pair {
        dark: chsl(214.0, 0.78, 0.70),
        light: chsl(214.0, 0.70, 0.40),
    },
);
/// **Broadcast**: brushed steel, a broadcast desk's; your mix lime, the reference magenta.
pub const COMPARATIO_BROADCAST: Skin = comparatio(
    "fx.comparatio.broadcast",
    Material::Brushed,
    Tint {
        hue: 200.0,
        sat: 0.05,
        amount: 0.24,
    },
    Pair {
        dark: chsl(78.0, 0.66, 0.58),
        light: chsl(78.0, 0.70, 0.27),
    },
    Pair {
        dark: chsl(322.0, 0.64, 0.70),
        light: chsl(322.0, 0.56, 0.40),
    },
);
/// **Ivory**: an ivory console printed like a vintage meter; your mix gold, the reference
/// green.
pub const COMPARATIO_IVORY: Skin = comparatio(
    "fx.comparatio.ivory",
    Material::Paper,
    Tint {
        hue: 40.0,
        sat: 0.26,
        amount: 0.42,
    },
    Pair {
        dark: chsl(70.0, 0.74, 0.62),
        light: chsl(70.0, 0.70, 0.40),
    },
    Pair {
        dark: chsl(154.0, 0.66, 0.60),
        light: chsl(154.0, 0.70, 0.28),
    },
);
pub const COMPARATIO_COLOURWAYS: [Colourway; 3] = [
    Colourway {
        id: "walnut",
        name_key: "comparatio.colourway.walnut",
        sheet: &COMPARATIO,
    },
    Colourway {
        id: "broadcast",
        name_key: "comparatio.colourway.broadcast",
        sheet: &COMPARATIO_BROADCAST,
    },
    Colourway {
        id: "ivory",
        name_key: "comparatio.colourway.ivory",
        sheet: &COMPARATIO_IVORY,
    },
];

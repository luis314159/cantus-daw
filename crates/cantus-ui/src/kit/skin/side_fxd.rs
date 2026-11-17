//! fxd — wave D's faces (the X-Y pad and the formula controller; ADRs 0147–0148,
//! provisional), three colourways each. Every display dark in both themes; fine strokes;
//! no vents. Their voices sit apart from every other unit's (`side.rs`'s tests).

use super::{
    chsl, Case, Colourway, Display, KnobShape, Lettering, Material, Ornaments, Pair, Plate, Skin,
    SwitchShape, Tint, Weight,
};

/// The X-Y pad: a touch surface of dark glass in a machined frame, its grid and puck a
/// cyan light, its path the second voice; soft caps, pill switches, an engraved plate,
/// screws and rules, light capitals.
const fn pad(key: &'static str, material: Material, surface: Tint, accent: Pair, second: Pair) -> Skin {
    Skin {
        key,
        surface,
        accent: Some(accent),
        accent2: Some(second),
        material,
        display: Display::Glass,
        knob: KnobShape::Soft,
        switch: SwitchShape::Pill,
        lettering: Lettering {
            case: Case::Upper,
            tracking: 0.14,
            weight: Weight::Medium,
            condensed: false,
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

/// **Titanium**: anodised titanium grey, the puck a cyan light, the path amber.
pub const XY: Skin = pad(
    "fx.xy",
    Material::Anodised,
    Tint {
        hue: 214.0,
        sat: 0.07,
        amount: 0.30,
    },
    Pair {
        dark: chsl(188.0, 0.78, 0.60),
        light: chsl(190.0, 0.80, 0.30),
    },
    Pair {
        dark: chsl(38.0, 0.86, 0.62),
        light: chsl(34.0, 0.80, 0.38),
    },
);
/// **Copper**: a copper-anodised frame, the puck a magenta light, the path gold.
pub const XY_COPPER: Skin = pad(
    "fx.xy.copper",
    Material::Anodised,
    Tint {
        hue: 18.0,
        sat: 0.26,
        amount: 0.34,
    },
    Pair {
        dark: chsl(326.0, 0.70, 0.68),
        light: chsl(328.0, 0.62, 0.40),
    },
    Pair {
        dark: chsl(52.0, 0.80, 0.62),
        light: chsl(48.0, 0.76, 0.34),
    },
);
/// **Frost**: a frosted-glass frame, the puck a mint light, the path violet.
pub const XY_FROST: Skin = pad(
    "fx.xy.frost",
    Material::Glass,
    Tint {
        hue: 196.0,
        sat: 0.10,
        amount: 0.30,
    },
    Pair {
        dark: chsl(156.0, 0.62, 0.62),
        light: chsl(158.0, 0.66, 0.28),
    },
    Pair {
        dark: chsl(272.0, 0.60, 0.74),
        light: chsl(270.0, 0.52, 0.44),
    },
);
pub const XY_COLOURWAYS: [Colourway; 3] = [
    Colourway {
        id: "titanium",
        name_key: "fxd.xy.look.titanium",
        sheet: &XY,
    },
    Colourway {
        id: "copper",
        name_key: "fxd.xy.look.copper",
        sheet: &XY_COPPER,
    },
    Colourway {
        id: "frost",
        name_key: "fxd.xy.look.frost",
        sheet: &XY_FROST,
    },
];

/// The formula controller: a scientific calculator's case — the formula typed on a
/// vacuum-fluorescent line over a plotting screen, the plot the second voice; dial knobs,
/// bat switches, a badge plate, screws, condensed capitals, mono values.
const fn calc(key: &'static str, material: Material, surface: Tint, accent: Pair, second: Pair) -> Skin {
    Skin {
        key,
        surface,
        accent: Some(accent),
        accent2: Some(second),
        material,
        display: Display::Vfd,
        knob: KnobShape::Dial,
        switch: SwitchShape::Bat,
        lettering: Lettering {
            case: Case::Upper,
            tracking: 0.06,
            weight: Weight::SemiBold,
            condensed: true,
            mono_values: true,
        },
        plate: Plate::Badge,
        ornaments: Ornaments {
            screws: true,
            vents: false,
            rules: false,
            rack_ears: false,
        },
    }
}

/// **Bakelite**: a brown bakelite case, the formula an aqua fluorescence, the plot orange.
pub const FORMULA: Skin = calc(
    "fx.formula",
    Material::Matte,
    Tint {
        hue: 26.0,
        sat: 0.30,
        amount: 0.40,
    },
    Pair {
        dark: chsl(176.0, 0.72, 0.58),
        light: chsl(178.0, 0.78, 0.27),
    },
    Pair {
        dark: chsl(22.0, 0.86, 0.62),
        light: chsl(20.0, 0.80, 0.40),
    },
);
/// **Ivory**: an ivory card case, the formula a phosphor green, the plot a scarlet.
pub const FORMULA_IVORY: Skin = calc(
    "fx.formula.ivory",
    Material::Paper,
    Tint {
        hue: 46.0,
        sat: 0.20,
        amount: 0.32,
    },
    Pair {
        dark: chsl(98.0, 0.62, 0.60),
        light: chsl(100.0, 0.66, 0.28),
    },
    Pair {
        dark: chsl(354.0, 0.72, 0.64),
        light: chsl(352.0, 0.66, 0.42),
    },
);
/// **Ebony**: a black lacquer case, the formula an amber fluorescence, the plot sky blue.
pub const FORMULA_EBONY: Skin = calc(
    "fx.formula.ebony",
    Material::Lacquer,
    Tint {
        hue: 230.0,
        sat: 0.04,
        amount: 0.18,
    },
    Pair {
        dark: chsl(34.0, 0.90, 0.60),
        light: chsl(30.0, 0.86, 0.36),
    },
    Pair {
        dark: chsl(204.0, 0.70, 0.68),
        light: chsl(206.0, 0.66, 0.40),
    },
);
pub const FORMULA_COLOURWAYS: [Colourway; 3] = [
    Colourway {
        id: "bakelite",
        name_key: "fxd.formula.look.bakelite",
        sheet: &FORMULA,
    },
    Colourway {
        id: "ivory",
        name_key: "fxd.formula.look.ivory",
        sheet: &FORMULA_IVORY,
    },
    Colourway {
        id: "ebony",
        name_key: "fxd.formula.look.ebony",
        sheet: &FORMULA_EBONY,
    },
];

/// Every wave-D sheet, faces and colourways (the house-rule tests take them in).
pub const FXD_SHEETS: [&Skin; 6] = [&XY, &XY_COPPER, &XY_FROST, &FORMULA, &FORMULA_IVORY, &FORMULA_EBONY];

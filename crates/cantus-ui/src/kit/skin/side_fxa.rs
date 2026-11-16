//! fxa — wave A's faces (Loquax, Fractus, Caelum; ADRs 0139–0141, provisional), each with
//! its colourways. Every display dark in both themes; fine strokes; no vents. Each face's
//! voices sit apart from every other unit's of the side round (`side.rs`'s tests).

use super::{
    chsl, Case, Colourway, Display, KnobShape, Lettering, Material, Ornaments, Pair, Plate, Skin,
    SwitchShape, Tint, Weight,
};

/// Loquax, the vocoder: a studio vocoder's console — brushed aluminium, its bands a column
/// of green light on a phosphor screen (the voice lighting them), the carrier's second
/// voice a violet; chicken-head knobs, rocker switches, an engraved plate, rules, screws.
const fn loquax(
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
        display: Display::Phosphor,
        knob: KnobShape::ChickenHead,
        switch: SwitchShape::Rocker,
        lettering: Lettering {
            case: Case::Upper,
            tracking: 0.12,
            weight: Weight::Medium,
            condensed: false,
            mono_values: false,
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

pub const LOQUAX: Skin = loquax(
    "fx.loquax",
    Material::Brushed,
    Tint {
        hue: 200.0,
        sat: 0.05,
        amount: 0.35,
    },
    Pair {
        dark: chsl(128.0, 0.70, 0.58),
        light: chsl(130.0, 0.72, 0.28),
    },
    Pair {
        dark: chsl(296.0, 0.55, 0.72),
        light: chsl(292.0, 0.50, 0.40),
    },
);
/// **Ink**: black lacquer, the bands a pale green, the carrier a rose.
pub const LOQUAX_INK: Skin = loquax(
    "fx.loquax.ink",
    Material::Lacquer,
    Tint {
        hue: 240.0,
        sat: 0.06,
        amount: 0.20,
    },
    Pair {
        dark: chsl(118.0, 0.55, 0.66),
        light: chsl(120.0, 0.60, 0.30),
    },
    Pair {
        dark: chsl(326.0, 0.60, 0.70),
        light: chsl(328.0, 0.55, 0.40),
    },
);
/// **Tolex**: a road case's black tolex, the bands a cool cyan-green, the carrier violet.
pub const LOQUAX_ROAD: Skin = loquax(
    "fx.loquax.road",
    Material::FineTolex,
    Tint {
        hue: 210.0,
        sat: 0.03,
        amount: 0.22,
    },
    Pair {
        dark: chsl(146.0, 0.62, 0.58),
        light: chsl(150.0, 0.66, 0.27),
    },
    Pair {
        dark: chsl(262.0, 0.55, 0.74),
        light: chsl(258.0, 0.50, 0.42),
    },
);
pub const LOQUAX_COLOURWAYS: [Colourway; 3] = [
    Colourway {
        id: "aluminium",
        name_key: "loquax.colourway.aluminium",
        sheet: &LOQUAX,
    },
    Colourway {
        id: "ink",
        name_key: "loquax.colourway.ink",
        sheet: &LOQUAX_INK,
    },
    Colourway {
        id: "road",
        name_key: "loquax.colourway.road",
        sheet: &LOQUAX_ROAD,
    },
];

/// Fractus, the glitch sequencer: a black anodised grid box — the lanes on an oscilloscope's
/// graticule (dark in every theme), the painted steps hot pink, the cloud's grains an acid
/// yellow-green; soft caps, lamp switches, a stripe plate, condensed capitals, mono values.
const fn fractus(
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
        knob: KnobShape::Cap,
        switch: SwitchShape::Lamp,
        lettering: Lettering {
            case: Case::Upper,
            tracking: 0.08,
            weight: Weight::SemiBold,
            condensed: true,
            mono_values: true,
        },
        plate: Plate::Stripe,
        ornaments: Ornaments {
            screws: false,
            vents: false,
            rules: true,
            rack_ears: false,
        },
    }
}

pub const FRACTUS: Skin = fractus(
    "fx.fractus",
    Material::Anodised,
    Tint {
        hue: 250.0,
        sat: 0.05,
        amount: 0.25,
    },
    Pair {
        dark: chsl(334.0, 0.85, 0.66),
        light: chsl(336.0, 0.80, 0.42),
    },
    Pair {
        dark: chsl(100.0, 0.80, 0.58),
        light: chsl(104.0, 0.82, 0.27),
    },
);
/// **Signal**: white card, the steps a signal red-pink, the grains cyan.
pub const FRACTUS_SIGNAL: Skin = fractus(
    "fx.fractus.signal",
    Material::Paper,
    Tint {
        hue: 220.0,
        sat: 0.04,
        amount: 0.60,
    },
    Pair {
        dark: chsl(344.0, 0.80, 0.66),
        light: chsl(346.0, 0.78, 0.42),
    },
    Pair {
        dark: chsl(184.0, 0.70, 0.58),
        light: chsl(186.0, 0.75, 0.28),
    },
);
/// **Acid**: violet lacquer, the steps acid green, the grains pink.
pub const FRACTUS_ACID: Skin = fractus(
    "fx.fractus.acid",
    Material::Lacquer,
    Tint {
        hue: 268.0,
        sat: 0.30,
        amount: 0.45,
    },
    Pair {
        dark: chsl(96.0, 0.80, 0.58),
        light: chsl(100.0, 0.82, 0.27),
    },
    Pair {
        dark: chsl(318.0, 0.70, 0.70),
        light: chsl(320.0, 0.65, 0.42),
    },
);
pub const FRACTUS_COLOURWAYS: [Colourway; 3] = [
    Colourway {
        id: "carbon",
        name_key: "fractus.colourway.carbon",
        sheet: &FRACTUS,
    },
    Colourway {
        id: "signal",
        name_key: "fractus.colourway.signal",
        sheet: &FRACTUS_SIGNAL,
    },
    Colourway {
        id: "acid",
        name_key: "fractus.colourway.acid",
        sheet: &FRACTUS_ACID,
    },
];

/// Caelum, the shimmer reverb: a satin night blue, its sky seen
/// through glass — the tail a periwinkle light, the shimmer's second voice an orchid; soft
/// knobs, pill switches, a badge plate, light spaced capitals.
const fn caelum(
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
        display: Display::Glass,
        knob: KnobShape::Soft,
        switch: SwitchShape::Pill,
        lettering: Lettering {
            case: Case::Upper,
            tracking: 0.20,
            weight: Weight::Regular,
            condensed: false,
            mono_values: false,
        },
        plate: Plate::Badge,
        ornaments: Ornaments {
            screws: false,
            vents: false,
            rules: false,
            rack_ears: false,
        },
    }
}

pub const CAELUM: Skin = caelum(
    "fx.caelum",
    Material::Matte,
    Tint {
        hue: 236.0,
        sat: 0.40,
        amount: 0.55,
    },
    Pair {
        dark: chsl(250.0, 0.72, 0.74),
        light: chsl(248.0, 0.60, 0.44),
    },
    Pair {
        dark: chsl(306.0, 0.58, 0.76),
        light: chsl(304.0, 0.50, 0.44),
    },
);
/// **Dawn**: a pearl card, the tail an orchid light, the shimmer a peach glow.
pub const CAELUM_DAWN: Skin = caelum(
    "fx.caelum.dawn",
    Material::Paper,
    Tint {
        hue: 340.0,
        sat: 0.18,
        amount: 0.30,
    },
    Pair {
        dark: chsl(286.0, 0.58, 0.76),
        light: chsl(288.0, 0.50, 0.44),
    },
    Pair {
        dark: chsl(24.0, 0.70, 0.74),
        light: chsl(22.0, 0.66, 0.42),
    },
);
/// **Aurora**: a deep green-black glass, the tail an aurora green, the shimmer violet.
pub const CAELUM_AURORA: Skin = caelum(
    "fx.caelum.aurora",
    Material::Glass,
    Tint {
        hue: 170.0,
        sat: 0.22,
        amount: 0.40,
    },
    Pair {
        dark: chsl(156.0, 0.62, 0.62),
        light: chsl(158.0, 0.66, 0.28),
    },
    Pair {
        dark: chsl(290.0, 0.52, 0.76),
        light: chsl(288.0, 0.46, 0.44),
    },
);
pub const CAELUM_COLOURWAYS: [Colourway; 3] = [
    Colourway {
        id: "night",
        name_key: "caelum.colourway.night",
        sheet: &CAELUM,
    },
    Colourway {
        id: "dawn",
        name_key: "caelum.colourway.dawn",
        sheet: &CAELUM_DAWN,
    },
    Colourway {
        id: "aurora",
        name_key: "caelum.colourway.aurora",
        sheet: &CAELUM_AURORA,
    },
];

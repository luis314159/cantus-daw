//! fxb — wave B's faces (Vinculum, Lenis, Cedo; ADRs 0142–0144, provisional), each with
//! its colourways. Every display dark in both themes; fine strokes; no vents. Each face's
//! voices sit apart from every other unit's of the side round (`side.rs`'s tests).

use super::{
    chsl, Case, Colourway, Display, KnobShape, Lettering, Material, Ornaments, Pair, Plate, Skin,
    SwitchShape, Tint, Weight,
};

/// Vinculum, the bus compressor: a rack unit — its faceplate anodised, rack ears and their
/// screws, its gain reduction on backlit needle meters (a warm lamp-lit card), the
/// reduction's red; skirted knobs on stepped positions, bat switches, an engraved legend.
const fn vinculum(
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
        display: Display::Lcd,
        knob: KnobShape::Skirted,
        switch: SwitchShape::Bat,
        lettering: Lettering {
            case: Case::Upper,
            tracking: 0.10,
            weight: Weight::Medium,
            condensed: true,
            mono_values: true,
        },
        plate: Plate::Legend,
        ornaments: Ornaments {
            screws: true,
            vents: false,
            rules: true,
            rack_ears: true,
        },
    }
}

/// **Console**: a charcoal anodised faceplate, the meters' lamp a warm yellow, the
/// reduction a signal red.
pub const VINCULUM: Skin = vinculum(
    "fx.vinculum",
    Material::Anodised,
    Tint {
        hue: 214.0,
        sat: 0.10,
        amount: 0.35,
    },
    Pair {
        dark: chsl(52.0, 0.86, 0.62),
        light: chsl(46.0, 0.80, 0.36),
    },
    Pair {
        dark: chsl(2.0, 0.72, 0.62),
        light: chsl(2.0, 0.68, 0.42),
    },
);
/// **Broadcast**: a cream lacquered faceplate of a broadcast chain, the needle lamp a
/// pale peach, the reduction an oxblood.
pub const VINCULUM_BROADCAST: Skin = vinculum(
    "fx.vinculum.broadcast",
    Material::Lacquer,
    Tint {
        hue: 40.0,
        sat: 0.30,
        amount: 0.30,
    },
    Pair {
        dark: chsl(28.0, 0.78, 0.68),
        light: chsl(26.0, 0.72, 0.40),
    },
    Pair {
        dark: chsl(350.0, 0.62, 0.62),
        light: chsl(350.0, 0.60, 0.36),
    },
);
/// **Steel**: brushed steel, the meters lit a cold green, the reduction a coral.
pub const VINCULUM_STEEL: Skin = vinculum(
    "fx.vinculum.steel",
    Material::Brushed,
    Tint {
        hue: 200.0,
        sat: 0.06,
        amount: 0.25,
    },
    Pair {
        dark: chsl(132.0, 0.58, 0.62),
        light: chsl(134.0, 0.60, 0.30),
    },
    Pair {
        dark: chsl(12.0, 0.70, 0.66),
        light: chsl(12.0, 0.66, 0.42),
    },
);
pub const VINCULUM_COLOURWAYS: [Colourway; 3] = [
    Colourway {
        id: "console",
        name_key: "vinculum.colourway.console",
        sheet: &VINCULUM,
    },
    Colourway {
        id: "broadcast",
        name_key: "vinculum.colourway.broadcast",
        sheet: &VINCULUM_BROADCAST,
    },
    Colourway {
        id: "steel",
        name_key: "vinculum.colourway.steel",
        sheet: &VINCULUM_STEEL,
    },
];

/// Lenis, the resonance suppressor: a precision measuring instrument — an enamelled lab
/// case, its spectrum a calm sea-green line on a dark graticule, the cuts lit in a warm
/// coral; cap knobs, lamp switches, a badge plate, fine rules.
const fn lenis(
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
            case: Case::AsIs,
            tracking: 0.04,
            weight: Weight::Regular,
            condensed: false,
            mono_values: true,
        },
        plate: Plate::Badge,
        ornaments: Ornaments {
            screws: true,
            vents: false,
            rules: true,
            rack_ears: false,
        },
    }
}

/// **Laboratory**: a grey-green enamel case, the spectrum sea-green, the cuts coral.
pub const LENIS: Skin = lenis(
    "fx.lenis",
    Material::Lacquer,
    Tint {
        hue: 160.0,
        sat: 0.14,
        amount: 0.40,
    },
    Pair {
        dark: chsl(172.0, 0.58, 0.60),
        light: chsl(174.0, 0.62, 0.28),
    },
    Pair {
        dark: chsl(16.0, 0.78, 0.66),
        light: chsl(14.0, 0.70, 0.42),
    },
);
/// **Ivory**: an ivory instrument card, the spectrum a deep teal-blue, the cuts a rose.
pub const LENIS_IVORY: Skin = lenis(
    "fx.lenis.ivory",
    Material::Paper,
    Tint {
        hue: 46.0,
        sat: 0.20,
        amount: 0.22,
    },
    Pair {
        dark: chsl(192.0, 0.56, 0.62),
        light: chsl(194.0, 0.66, 0.30),
    },
    Pair {
        dark: chsl(340.0, 0.62, 0.68),
        light: chsl(342.0, 0.58, 0.42),
    },
);
/// **Graphite**: a graphite anodised case, the spectrum a pale jade, the cuts a gold.
pub const LENIS_GRAPHITE: Skin = lenis(
    "fx.lenis.graphite",
    Material::Anodised,
    Tint {
        hue: 230.0,
        sat: 0.06,
        amount: 0.30,
    },
    Pair {
        dark: chsl(150.0, 0.44, 0.66),
        light: chsl(152.0, 0.50, 0.30),
    },
    Pair {
        dark: chsl(40.0, 0.80, 0.62),
        light: chsl(38.0, 0.74, 0.36),
    },
);
pub const LENIS_COLOURWAYS: [Colourway; 3] = [
    Colourway {
        id: "laboratory",
        name_key: "lenis.colourway.laboratory",
        sheet: &LENIS,
    },
    Colourway {
        id: "ivory",
        name_key: "lenis.colourway.ivory",
        sheet: &LENIS_IVORY,
    },
    Colourway {
        id: "graphite",
        name_key: "lenis.colourway.graphite",
        sheet: &LENIS_GRAPHITE,
    },
];

/// Cedo, the ducker: a mechanical pump — a machined body, its ducking envelope a magenta
/// trace on a dark drum, the key's hits a lime; arc knobs, rocker switches, a stripe plate.
const fn cedo(
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
        knob: KnobShape::Arc,
        switch: SwitchShape::Rocker,
        lettering: Lettering {
            case: Case::Upper,
            tracking: 0.14,
            weight: Weight::SemiBold,
            condensed: false,
            mono_values: false,
        },
        plate: Plate::Stripe,
        ornaments: Ornaments {
            screws: true,
            vents: false,
            rules: false,
            rack_ears: false,
        },
    }
}

/// **Workshop**: a gunmetal machined body, the pump magenta, the key's hits lime.
pub const CEDO: Skin = cedo(
    "fx.cedo",
    Material::Brushed,
    Tint {
        hue: 250.0,
        sat: 0.08,
        amount: 0.35,
    },
    Pair {
        dark: chsl(326.0, 0.70, 0.66),
        light: chsl(326.0, 0.62, 0.42),
    },
    Pair {
        dark: chsl(84.0, 0.66, 0.58),
        light: chsl(86.0, 0.70, 0.30),
    },
);
/// **Signal**: a signal-orange enamel body, the pump an electric violet, the hits aqua.
pub const CEDO_SIGNAL: Skin = cedo(
    "fx.cedo.signal",
    Material::Lacquer,
    Tint {
        hue: 22.0,
        sat: 0.52,
        amount: 0.45,
    },
    Pair {
        dark: chsl(270.0, 0.62, 0.72),
        light: chsl(268.0, 0.56, 0.44),
    },
    Pair {
        dark: chsl(184.0, 0.62, 0.58),
        light: chsl(186.0, 0.66, 0.30),
    },
);
/// **Midnight**: a matte navy body, the pump a hot pink-red, the hits a citrus yellow.
pub const CEDO_MIDNIGHT: Skin = cedo(
    "fx.cedo.midnight",
    Material::Matte,
    Tint {
        hue: 222.0,
        sat: 0.36,
        amount: 0.50,
    },
    Pair {
        dark: chsl(346.0, 0.76, 0.66),
        light: chsl(346.0, 0.70, 0.42),
    },
    Pair {
        dark: chsl(62.0, 0.78, 0.60),
        light: chsl(58.0, 0.74, 0.32),
    },
);
pub const CEDO_COLOURWAYS: [Colourway; 3] = [
    Colourway {
        id: "workshop",
        name_key: "cedo.colourway.workshop",
        sheet: &CEDO,
    },
    Colourway {
        id: "signal",
        name_key: "cedo.colourway.signal",
        sheet: &CEDO_SIGNAL,
    },
    Colourway {
        id: "midnight",
        name_key: "cedo.colourway.midnight",
        sheet: &CEDO_MIDNIGHT,
    },
];

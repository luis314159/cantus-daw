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
/// Its screens are a vacuum-fluorescent glass, dark in both themes (the owner's rule; an
/// LCD's pale glass was the contract's placeholder).
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
        display: Display::Vfd,
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

/// **Console**: a charcoal anodised faceplate (drawn by the editor, `kit::skin::racks`)
/// in a lacquered black case, the meters' lamp a warm yellow, the reduction a signal red.
pub const VINCULUM: Skin = vinculum(
    "fx.vinculum",
    Material::Lacquer,
    Tint {
        hue: 214.0,
        sat: 0.10,
        amount: 0.35,
    },
    Pair {
        dark: chsl(64.0, 0.84, 0.62),
        light: chsl(58.0, 0.82, 0.32),
    },
    Pair {
        dark: chsl(6.0, 0.72, 0.62),
        light: chsl(6.0, 0.68, 0.42),
    },
);
/// **Broadcast**: a cream enamelled faceplate of a broadcast chain in a case of fine vinyl,
/// the needle lamp a pale peach, the reduction an oxblood.
pub const VINCULUM_BROADCAST: Skin = vinculum(
    "fx.vinculum.broadcast",
    Material::FineTolex,
    Tint {
        hue: 40.0,
        sat: 0.30,
        amount: 0.30,
    },
    Pair {
        dark: chsl(20.0, 0.78, 0.68),
        light: chsl(20.0, 0.72, 0.40),
    },
    Pair {
        dark: chsl(348.0, 0.62, 0.62),
        light: chsl(348.0, 0.60, 0.36),
    },
);
/// **Steel**: brushed stainless, the meters lit an ice blue, the reduction a coral.
pub const VINCULUM_STEEL: Skin = vinculum(
    "fx.vinculum.steel",
    Material::Brushed,
    Tint {
        hue: 200.0,
        sat: 0.06,
        amount: 0.25,
    },
    Pair {
        dark: chsl(232.0, 0.70, 0.72),
        light: chsl(232.0, 0.62, 0.42),
    },
    Pair {
        dark: chsl(24.0, 0.74, 0.64),
        light: chsl(22.0, 0.70, 0.40),
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

/// Lenis, the resonance suppressor: a bench analyser — an enamelled instrument on a lab
/// bench (drawn by the editor, `kit::skin::analysers`), its spectrum a phosphor trace on a
/// dark graticule, the cuts hanging under it in a second phosphor; cap knobs, lamp keys, a
/// badge plate, fine printed rules. Each colourway's trace and cut sit apart from every
/// other unit's two voices (`side.rs`'s tests).
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

/// **Laboratory**: a grey-green enamel panel in a lacquered blue-grey case, the spectrum
/// an aqua phosphor, the cuts a rose.
pub const LENIS: Skin = lenis(
    "fx.lenis",
    Material::Lacquer,
    Tint {
        hue: 152.0,
        sat: 0.10,
        amount: 0.40,
    },
    Pair {
        dark: chsl(166.0, 0.62, 0.60),
        light: chsl(168.0, 0.66, 0.28),
    },
    Pair {
        dark: chsl(348.0, 0.72, 0.68),
        light: chsl(348.0, 0.62, 0.42),
    },
);
/// **Ivory**: an ivory enamel panel, the spectrum an amber phosphor, the cuts a cobalt
/// (the kit's accent the cobalt: amber on an enamel is another unit's).
pub const LENIS_IVORY: Skin = lenis(
    "fx.lenis.ivory",
    Material::Paper,
    Tint {
        hue: 44.0,
        sat: 0.20,
        amount: 0.22,
    },
    Pair {
        dark: chsl(228.0, 0.62, 0.72),
        light: chsl(226.0, 0.58, 0.40),
    },
    Pair {
        dark: chsl(44.0, 0.86, 0.60),
        light: chsl(40.0, 0.80, 0.34),
    },
);
/// **Graphite**: a graphite anodised panel, the spectrum a white phosphor (a voice too
/// pale to count as a colour), the cuts an orange.
pub const LENIS_GRAPHITE: Skin = lenis(
    "fx.lenis.graphite",
    Material::Anodised,
    Tint {
        hue: 228.0,
        sat: 0.06,
        amount: 0.30,
    },
    Pair {
        dark: chsl(228.0, 0.24, 0.84),
        light: chsl(228.0, 0.22, 0.30),
    },
    Pair {
        dark: chsl(24.0, 0.88, 0.62),
        light: chsl(22.0, 0.80, 0.40),
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

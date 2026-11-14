//! The fx side round's faces (provisional ADRs 0130–0133). **The contract's placeholders**:
//! each new unit wears a sibling's look under a key of its own, so every unit keeps a sheet
//! of its own (`tests/skins.rs`) until its builder draws the real face. The looks are
//! borrowed from the tone family, whose tests compare its sheets among themselves only (the
//! dynamics and modulation families refuse a stranger sharing a hue and a material with
//! theirs). Each builder replaces its own sheet here — and only its own — with one that
//! differs from every other unit's in hue or material, display included.

use super::super::color::Hsl;
use super::{
    tone, Case, Colourway, Display, KnobShape, Lettering, Material, Ornaments, Pair, Plate, Skin,
    SwitchShape, Tint, Weight,
};

const fn hsl(h: f32, s: f32, l: f32) -> Hsl {
    Hsl { h, s, l }
} // fx:fragor
use cantus_core::EffectKind;

// fx:cribrum — Cribrum's own face (ADR 0130, provisional), in three colourways the user
// chooses among (ADR 0130 §Colourways); the first, Nocturne, is its face. Every display is
// a lit screen, dark in both themes (the owner's choice for the round's displays).

const fn chsl(h: f32, s: f32, l: f32) -> Hsl {
    Hsl { h, s, l }
}

/// Cribrum's face apart from its colours: machined caps, lamp keys, a printed legend, screws
/// and rules, condensed capitals and mono readouts.
const fn cribrum(
    key: &'static str,
    surface: Tint,
    accent: Pair,
    accent2: Pair,
    material: Material,
    display: Display,
) -> Skin {
    Skin {
        key,
        surface,
        accent: Some(accent),
        accent2: Some(accent2),
        material,
        display,
        knob: KnobShape::Cap,
        switch: SwitchShape::Lamp,
        lettering: Lettering {
            case: Case::Upper,
            tracking: 0.10,
            weight: Weight::SemiBold,
            condensed: true,
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

/// Cribrum, the auto filter (*the sieve*) — **Nocturne**, its face: violet carbon weave, the
/// response ice-cyan on a dark phosphor screen over the music it sieves, the movement amber.
/// No other unit is violet carbon (the flanger's violet is lacquer).
pub const CRIBRUM: Skin = cribrum(
    "fx.cribrum",
    Tint {
        hue: 272.0,
        sat: 0.26,
        amount: 0.6,
    },
    Pair {
        dark: chsl(190.0, 0.80, 0.66),
        light: chsl(194.0, 0.80, 0.30),
    },
    Pair {
        dark: chsl(40.0, 0.90, 0.64),
        light: chsl(34.0, 0.80, 0.36),
    },
    Material::Carbon,
    Display::Phosphor,
);

/// Cribrum in **Brass**: gold anodised aluminium, the response teal on a fluorescent
/// display, the movement brick.
pub const CRIBRUM_BRASS: Skin = cribrum(
    "fx.cribrum.brass",
    Tint {
        hue: 48.0,
        sat: 0.34,
        amount: 0.6,
    },
    Pair {
        dark: chsl(172.0, 0.72, 0.56),
        light: chsl(176.0, 0.80, 0.26),
    },
    Pair {
        dark: chsl(8.0, 0.78, 0.66),
        light: chsl(6.0, 0.70, 0.42),
    },
    Material::Anodised,
    Display::Vfd,
);

/// Cribrum in **Petrol**: deep teal-blue lacquer, the response hot orange on a phosphor
/// screen, the movement lime.
pub const CRIBRUM_PETROL: Skin = cribrum(
    "fx.cribrum.petrol",
    Tint {
        hue: 192.0,
        sat: 0.34,
        amount: 0.6,
    },
    Pair {
        dark: chsl(24.0, 0.95, 0.62),
        light: chsl(20.0, 0.85, 0.40),
    },
    Pair {
        dark: chsl(84.0, 0.70, 0.62),
        light: chsl(88.0, 0.70, 0.28),
    },
    Material::Lacquer,
    Display::Phosphor,
);

/// Cribrum's colourways, its face first.
pub const CRIBRUM_COLOURWAYS: [Colourway; 3] = [
    Colourway {
        id: "nocturne",
        name_key: "cribrum.colourway.nocturne",
        sheet: &CRIBRUM,
    },
    Colourway {
        id: "brass",
        name_key: "cribrum.colourway.brass",
        sheet: &CRIBRUM_BRASS,
    },
    Colourway {
        id: "petrol",
        name_key: "cribrum.colourway.petrol",
        sheet: &CRIBRUM_PETROL,
    },
];

// fx:fragor — Fragor's face (ADR 0131), the owner's pick (palette A, 2026-11-13): a valve
// amp at night — warm amber, a tube's glow, on smoked glass; the sound drawn on an amber
// phosphor screen behind glass (dark in every theme, as a device's screen), a white-hot second
// voice, skirted knobs, rocker switches, engraved capitals, screws and rules. Its tubes,
// grille, jewel lights and brass are drawn by `views/fx/valve_draw.rs`; its pedals wear
// their own finishes (`skin/pedals.rs`).
pub const FRAGOR: Skin = Skin {
    key: "fx.fragor",
    surface: Tint {
        hue: 32.0,
        sat: 0.16,
        amount: 0.35,
    },
    accent: Some(Pair {
        dark: hsl(36.0, 0.92, 0.58),
        light: hsl(30.0, 0.85, 0.38),
    }),
    accent2: Some(Pair {
        // A filament's white heat, nearly unsaturated: the second voice keeps its own
        // colour beside the amber, not a second amber (the displays' rule).
        dark: hsl(44.0, 0.18, 0.84),
        light: hsl(28.0, 0.45, 0.28),
    }),
    // Smoked glass, as a valve's envelope and the screen's pane: the amber family's
    // matte, brushed, anodised, lacquer, carbon and vinyl are taken near this hue
    // (Phasis, Fervor, Chorda, Aula …).
    material: Material::Glass,
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

// fx:vetus — Vetus's face (ADR 0133, provisional) in three colourways (ADR 0130
// §Colourways); the first, Sleeve, is its face. The record and the spectrum sit on a dark
// display in every colourway and theme (the round's rule for displays); the record keeps its
// own colours (`skin/records.rs`).

/// Vetus's face apart from its colours: dials with printed scales, bat-handle toggles, a
/// badge, screws and vents, wide spaced capitals — a hi-fi of the seventies.
const fn vetus(
    key: &'static str,
    surface: Tint,
    accent: Pair,
    accent2: Pair,
    material: Material,
    display: Display,
) -> Skin {
    Skin {
        key,
        surface,
        accent: Some(accent),
        accent2: Some(accent2),
        material,
        display,
        knob: KnobShape::Dial,
        switch: SwitchShape::Bat,
        lettering: Lettering {
            case: Case::Upper,
            tracking: 0.14,
            weight: Weight::SemiBold,
            condensed: false,
            mono_values: true,
        },
        plate: Plate::Badge,
        ornaments: Ornaments {
            screws: true,
            vents: true,
            rules: false,
            rack_ears: false,
        },
    }
}

/// Vetus, lo-fi (*old*) — **Sleeve**, its face: a record sleeve's printed card in warm
/// cream, the music vermilion on a dark phosphor screen, the medium's band teal. No other
/// unit is vermilion on paper.
pub const VETUS: Skin = vetus(
    "fx.vetus",
    Tint {
        hue: 36.0,
        sat: 0.30,
        amount: 0.55,
    },
    Pair {
        dark: chsl(12.0, 0.82, 0.63),
        light: chsl(10.0, 0.78, 0.40),
    },
    Pair {
        dark: chsl(176.0, 0.55, 0.58),
        light: chsl(182.0, 0.75, 0.26),
    },
    Material::Paper,
    Display::Phosphor,
);

/// Vetus in **Walnut**: a hi-fi cabinet's oiled walnut, brass on a fluorescent display, the
/// band ivory.
pub const VETUS_WALNUT: Skin = vetus(
    "fx.vetus.walnut",
    Tint {
        hue: 24.0,
        sat: 0.36,
        amount: 0.6,
    },
    Pair {
        dark: chsl(44.0, 0.78, 0.60),
        light: chsl(38.0, 0.75, 0.32),
    },
    Pair {
        dark: chsl(40.0, 0.30, 0.84),
        light: chsl(24.0, 0.55, 0.28),
    },
    Material::Wood,
    Display::Vfd,
);

/// Vetus in **Cassette**: a tape's black shell, its label's red, the band amber.
pub const VETUS_CASSETTE: Skin = vetus(
    "fx.vetus.cassette",
    Tint {
        hue: 220.0,
        sat: 0.08,
        amount: 0.5,
    },
    Pair {
        dark: chsl(356.0, 0.80, 0.62),
        light: chsl(354.0, 0.75, 0.42),
    },
    Pair {
        dark: chsl(40.0, 0.90, 0.62),
        light: chsl(32.0, 0.80, 0.34),
    },
    Material::Matte,
    Display::Scope,
);

/// Vetus's colourways, its face first.
pub const VETUS_COLOURWAYS: [Colourway; 3] = [
    Colourway {
        id: "sleeve",
        name_key: "vetus.colourway.sleeve",
        sheet: &VETUS,
    },
    Colourway {
        id: "walnut",
        name_key: "vetus.colourway.walnut",
        sheet: &VETUS_WALNUT,
    },
    Colourway {
        id: "cassette",
        name_key: "vetus.colourway.cassette",
        sheet: &VETUS_CASSETTE,
    },
]; // fx:vetus

/// Libra, the utility: for now Latitudo's look (the stereo utility).
pub const LIBRA: Skin = Skin {
    key: "fx.libra",
    ..tone::LATITUDO
};

/// fx:colourways — the colourways a unit of the side round offers (empty: one face).
pub fn colourways(kind: EffectKind) -> &'static [Colourway] {
    match kind {
        EffectKind::AutoFilter => &CRIBRUM_COLOURWAYS, // fx:cribrum
        EffectKind::LoFi => &VETUS_COLOURWAYS,        // fx:vetus
        _ => &[],
    }
}

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

// fx:fragor — the two palettes the owner was shown beside the one he chose (2026-11-13):
// kept as data, to become Fragor's colourways when the colourway mechanism lands.
/// B: cream and brass, vintage.
pub const FRAGOR_B: Skin = Skin {
    key: "fx.fragor.b",
    surface: Tint {
        hue: 42.0,
        sat: 0.30,
        amount: 0.55,
    },
    accent: Some(Pair {
        dark: hsl(44.0, 0.60, 0.56),
        light: hsl(40.0, 0.70, 0.32),
    }),
    accent2: Some(Pair {
        dark: hsl(8.0, 0.50, 0.60),
        light: hsl(6.0, 0.55, 0.36),
    }),
    material: Material::Brushed,
    display: Display::Vfd,
    ..FRAGOR
}; // fx:fragor
/// C: steel with a single signal red.
pub const FRAGOR_C: Skin = Skin {
    key: "fx.fragor.c",
    surface: Tint {
        hue: 212.0,
        sat: 0.06,
        amount: 0.30,
    },
    accent: Some(Pair {
        dark: hsl(4.0, 0.80, 0.58),
        light: hsl(2.0, 0.74, 0.44),
    }),
    accent2: Some(Pair {
        dark: hsl(210.0, 0.08, 0.78),
        light: hsl(214.0, 0.10, 0.38),
    }),
    material: Material::Anodised,
    display: Display::Scope,
    ..FRAGOR
}; // fx:fragor
/// Fragor's colourways besides its own face: B, C. fx:fragor
pub const FRAGOR_COLOURWAYS: [&Skin; 2] = [&FRAGOR_B, &FRAGOR_C];

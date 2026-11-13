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

/// Fragor, the pedals: for now Fervor's look (drive and colour).
pub const FRAGOR: Skin = Skin {
    key: "fx.fragor",
    ..tone::FERVOR
};

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

/// fx:colourways — the colourways a unit of the side round offers (empty: one face).
pub fn colourways(kind: EffectKind) -> &'static [Colourway] {
    match kind {
        EffectKind::AutoFilter => &CRIBRUM_COLOURWAYS, // fx:cribrum
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

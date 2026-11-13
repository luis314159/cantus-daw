//! The fx side round's faces (provisional ADRs 0130–0133). **The contract's placeholders**:
//! each new unit wears a sibling's look under a key of its own, so every unit keeps a sheet
//! of its own (`tests/skins.rs`) until its builder draws the real face. The looks are
//! borrowed from the tone family, whose tests compare its sheets among themselves only (the
//! dynamics and modulation families refuse a stranger sharing a hue and a material with
//! theirs). Each builder replaces its own sheet here — and only its own — with one that
//! differs from every other unit's in hue or material, display included.

use super::super::color::Hsl;
use super::{
    tone, Case, Display, KnobShape, Lettering, Material, Ornaments, Pair, Plate, Skin, SwitchShape,
    Tint, Weight,
}; // fx:fragor

const fn hsl(h: f32, s: f32, l: f32) -> Hsl {
    Hsl { h, s, l }
} // fx:fragor
use cantus_core::EffectKind;

/// Cribrum, the auto filter: for now Aequalis's look (a filter is the EQ's cousin).
pub const CRIBRUM: Skin = Skin {
    key: "fx.cribrum",
    ..tone::AEQUALIS
};

// fx:fragor — Fragor's face (ADR 0131), the owner's pick (palette A, 2026-11-13): a valve
// amp at night — warm amber, a tube's glow, on dark matte; the sound drawn on an amber
// phosphor screen behind glass (dark in every theme, as a device's screen), a cream second
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
        dark: hsl(44.0, 0.40, 0.82),
        light: hsl(28.0, 0.45, 0.28),
    }),
    material: Material::Matte,
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

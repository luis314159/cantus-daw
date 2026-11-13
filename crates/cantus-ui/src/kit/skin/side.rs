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
};
use cantus_core::EffectKind;

// fx:cribrum — Cribrum's own face (ADR 0130, provisional).
/// Cribrum, the auto filter (*the sieve*): a 70s filter bank's racing-green lacquer, the
/// response glowing acid lime behind smoked glass over the music it sieves, the movement
/// (the LFO, the curve, the follower) in coral, machined caps, lamp keys, a printed legend,
/// screws and rules, condensed capitals and mono readouts. No other unit is green lacquer.
pub const CRIBRUM: Skin = Skin {
    key: "fx.cribrum",
    surface: Tint {
        hue: 142.0,
        sat: 0.30,
        amount: 0.6,
    },
    accent: Some(Pair {
        dark: Hsl {
            h: 78.0,
            s: 0.82,
            l: 0.60,
        },
        light: Hsl {
            h: 82.0,
            s: 0.80,
            l: 0.28,
        },
    }),
    accent2: Some(Pair {
        dark: Hsl {
            h: 10.0,
            s: 0.82,
            l: 0.70,
        },
        light: Hsl {
            h: 8.0,
            s: 0.70,
            l: 0.44,
        },
    }),
    material: Material::Lacquer,
    display: Display::Glass,
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
};

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

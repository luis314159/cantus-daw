//! The fx side round's faces (provisional ADRs 0130–0133). **The contract's placeholders**:
//! each new unit wears a sibling's look under a key of its own, so every unit keeps a sheet
//! of its own (`tests/skins.rs`) until its builder draws the real face. The looks are
//! borrowed from the tone family, whose tests compare its sheets among themselves only (the
//! dynamics and modulation families refuse a stranger sharing a hue and a material with
//! theirs). Each builder replaces its own sheet here — and only its own — with one that
//! differs from every other unit's in hue or material, display included.

use super::{tone, Skin};
use cantus_core::EffectKind;

/// Cribrum, the auto filter: for now Aequalis's look (a filter is the EQ's cousin).
pub const CRIBRUM: Skin = Skin {
    key: "fx.cribrum",
    ..tone::AEQUALIS
};

/// Fragor, the pedals: for now Fervor's look (drive and colour).
pub const FRAGOR: Skin = Skin {
    key: "fx.fragor",
    ..tone::FERVOR
};

// fx:lapsus — Lapsus's own face (ADR 0132, provisional).
/// Lapsus, the time shaper (*a slip*): a DJ's deck at midnight — a petrol body under smoked
/// glass (a CD deck's top), the time curve glowing amber on a vacuum-fluorescent screen
/// (the old decks' displays), the level in ice blue, soft rubber knobs, lamp keys, a
/// painted stripe across the header and printed rules, upper-case lettering and
/// mono readouts. Every unit with an accent within 20° of amber is another material
/// (Phasis is matte, Fervor vinyl, Aula brushed, Chorda lacquer…): glass is free there.
pub const LAPSUS: Skin = Skin {
    key: "fx.lapsus",
    surface: super::Tint {
        hue: 192.0,
        sat: 0.26,
        amount: 0.6,
    },
    accent: Some(super::Pair {
        dark: super::super::color::Hsl {
            h: 36.0,
            s: 0.95,
            l: 0.60,
        },
        light: super::super::color::Hsl {
            h: 32.0,
            s: 0.90,
            l: 0.36,
        },
    }),
    accent2: Some(super::Pair {
        dark: super::super::color::Hsl {
            h: 190.0,
            s: 0.75,
            l: 0.66,
        },
        light: super::super::color::Hsl {
            h: 194.0,
            s: 0.80,
            l: 0.32,
        },
    }),
    material: super::Material::Glass,
    display: super::Display::Vfd,
    knob: super::KnobShape::Soft,
    switch: super::SwitchShape::Lamp,
    lettering: super::Lettering {
        case: super::Case::Upper,
        tracking: 0.08,
        weight: super::Weight::SemiBold,
        condensed: false,
        mono_values: true,
    },
    plate: super::Plate::Stripe,
    ornaments: super::Ornaments {
        screws: false,
        vents: false,
        rules: true,
        rack_ears: false,
    },
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

//! The tone, space and analysis faces (round 32, ADR 0074). Owned by the `skin` builder,
//! after the kit (`docs/round32/BUILDING.md`): each unit's face is data here — its tint,
//! its accents, its material, its display, its controls' shapes, its lettering, its plate
//! and its ornaments. The design notes are in `docs/round32/skin.md` (second delivery).

use super::super::color::Hsl;
use super::{
    Case, Display, KnobShape, Lettering, Material, Ornaments, Pair, Plate, Skin, SwitchShape, Tint,
    Weight,
};
use cantus_core::EffectKind;

const fn hsl(h: f32, s: f32, l: f32) -> Hsl {
    Hsl { h, s, l }
}

/// Aequalis, the parametric EQ: a precision instrument — blue anodised aluminium, the
/// curve behind smoked glass, small capped knobs, an engraved name, printed rules.
pub const AEQUALIS: Skin = Skin {
    key: "fx.eq",
    surface: Tint {
        hue: 205.0,
        sat: 0.20,
        amount: 0.45,
    },
    accent: Some(Pair {
        dark: hsl(196.0, 0.82, 0.62),
        light: hsl(199.0, 0.88, 0.34),
    }),
    // The analyser's second voice behind the curve: amber, a meter lamp's colour.
    accent2: Some(Pair {
        dark: hsl(38.0, 0.85, 0.62),
        light: hsl(32.0, 0.85, 0.38),
    }),
    material: Material::Anodised,
    display: Display::Glass,
    knob: KnobShape::Cap,
    switch: SwitchShape::Pill,
    lettering: Lettering {
        case: Case::AsIs,
        tracking: 0.02,
        weight: Weight::SemiBold,
        condensed: false,
        mono_values: true,
    },
    plate: Plate::Engraved,
    ornaments: Ornaments {
        screws: false,
        vents: false,
        rules: true,
        rack_ears: false,
    },
};

/// Fervor, saturation and colour: a valve amplifier — warm vinyl amp covering, the glowing in
/// an amber vacuum-fluorescent window, chicken-head knobs, bat toggles, a badge, screws
/// and vents.
pub const FERVOR: Skin = Skin {
    key: "fx.saturator",
    surface: Tint {
        hue: 16.0,
        sat: 0.32,
        amount: 0.55,
    },
    accent: Some(Pair {
        dark: hsl(26.0, 0.94, 0.58),
        light: hsl(18.0, 0.86, 0.40),
    }),
    // The harmonics' second voice: a pale filament yellow.
    accent2: Some(Pair {
        dark: hsl(50.0, 0.90, 0.64),
        light: hsl(44.0, 0.88, 0.34),
    }),
    material: Material::Tolex,
    display: Display::Vfd,
    knob: KnobShape::ChickenHead,
    switch: SwitchShape::Bat,
    lettering: Lettering {
        case: Case::Upper,
        tracking: 0.10,
        weight: Weight::SemiBold,
        condensed: true,
        mono_values: false,
    },
    plate: Plate::Badge,
    ornaments: Ornaments {
        screws: true,
        vents: true,
        rules: false,
        rack_ears: false,
    },
};

/// Spatium, the reverb (its room): a hall at night — deep blue lacquer, the room drawn
/// in a blue phosphor glow, soft knobs, lamps, a stripe under the name.
pub const SPATIUM: Skin = Skin {
    key: "fx.space",
    surface: Tint {
        hue: 240.0,
        sat: 0.34,
        amount: 0.6,
    },
    // Indigo, not Culmen's sapphire (212°, lacquer too): siblings across families never
    // share a hue within 20° and a material.
    accent: Some(Pair {
        dark: hsl(238.0, 0.72, 0.72),
        light: hsl(242.0, 0.62, 0.48),
    }),
    // The early reflections' cold cyan against the tail's blue.
    accent2: Some(Pair {
        dark: hsl(184.0, 0.62, 0.60),
        light: hsl(188.0, 0.72, 0.30),
    }),
    material: Material::Lacquer,
    display: Display::Phosphor,
    knob: KnobShape::Soft,
    switch: SwitchShape::Lamp,
    lettering: Lettering {
        case: Case::AsIs,
        tracking: 0.06,
        weight: Weight::Medium,
        condensed: false,
        mono_values: false,
    },
    plate: Plate::Stripe,
    ornaments: Ornaments {
        screws: false,
        vents: false,
        rules: false,
        rack_ears: false,
    },
};

/// Latitudo, stereo control: a broadcast goniometer — brushed steel in a rack, the
/// stereo field on a scope's graticule in green, dial knobs, rockers, a printed legend,
/// screws and rack ears.
pub const LATITUDO: Skin = Skin {
    key: "fx.stereo",
    surface: Tint {
        hue: 160.0,
        sat: 0.12,
        amount: 0.4,
    },
    accent: Some(Pair {
        dark: hsl(150.0, 0.62, 0.55),
        light: hsl(154.0, 0.78, 0.29),
    }),
    // The side channel: a warm orange against the mid's green.
    accent2: Some(Pair {
        dark: hsl(28.0, 0.86, 0.62),
        light: hsl(22.0, 0.82, 0.40),
    }),
    material: Material::Brushed,
    display: Display::Scope,
    knob: KnobShape::Dial,
    switch: SwitchShape::Rocker,
    lettering: Lettering {
        case: Case::Upper,
        tracking: 0.08,
        weight: Weight::Medium,
        condensed: false,
        mono_values: true,
    },
    plate: Plate::Legend,
    ornaments: Ornaments {
        screws: true,
        vents: false,
        rules: false,
        rack_ears: true,
    },
};

/// Speculum, the analyser: a bench instrument — grey-green enamel, the spectrum on a
/// backlit LCD, value rings only, pills, condensed mono lettering, printed rules.
pub const SPECULUM: Skin = Skin {
    key: "fx.analyzer",
    surface: Tint {
        hue: 95.0,
        sat: 0.10,
        amount: 0.5,
    },
    accent: Some(Pair {
        dark: hsl(78.0, 0.62, 0.56),
        light: hsl(84.0, 0.80, 0.27),
    }),
    // The held peak (or the second channel): a signal red.
    accent2: Some(Pair {
        dark: hsl(4.0, 0.80, 0.62),
        light: hsl(2.0, 0.78, 0.40),
    }),
    material: Material::Matte,
    display: Display::Lcd,
    knob: KnobShape::Arc,
    switch: SwitchShape::Pill,
    lettering: Lettering {
        case: Case::Upper,
        tracking: 0.04,
        weight: Weight::Medium,
        condensed: true,
        mono_values: true,
    },
    plate: Plate::Legend,
    ornaments: Ornaments {
        screws: false,
        vents: false,
        rules: true,
        rack_ears: false,
    },
};

/// Tonus, the real-time auto-tuner: a vocal booth's glass — magenta behind smoked glass,
/// the pitch drawn on chart paper, soft knobs, lamps, a stripe.
pub const TONUS: Skin = Skin {
    key: "fx.auto_tune",
    surface: Tint {
        hue: 318.0,
        sat: 0.22,
        amount: 0.45,
    },
    accent: Some(Pair {
        dark: hsl(326.0, 0.78, 0.66),
        light: hsl(330.0, 0.70, 0.40),
    }),
    // The sung pitch against the corrected one: a teal.
    accent2: Some(Pair {
        dark: hsl(176.0, 0.60, 0.55),
        light: hsl(180.0, 0.75, 0.28),
    }),
    material: Material::Glass,
    display: Display::Paper,
    knob: KnobShape::Soft,
    switch: SwitchShape::Lamp,
    lettering: Lettering {
        case: Case::AsIs,
        tracking: 0.03,
        weight: Weight::SemiBold,
        condensed: false,
        mono_values: false,
    },
    plate: Plate::Stripe,
    ornaments: Ornaments {
        screws: false,
        vents: false,
        rules: true,
        rack_ears: false,
    },
};

/// The sheet of an effect of this family.
pub fn of(kind: EffectKind) -> Option<&'static Skin> {
    match kind {
        EffectKind::Eq => Some(&AEQUALIS),
        EffectKind::Saturator => Some(&FERVOR),
        EffectKind::Space => Some(&SPATIUM),
        EffectKind::Stereo => Some(&LATITUDO),
        EffectKind::Analyzer => Some(&SPECULUM),
        EffectKind::AutoTune => Some(&TONUS),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every sheet of this family.
    const ALL: [&Skin; 6] = [&AEQUALIS, &FERVOR, &SPATIUM, &LATITUDO, &SPECULUM, &TONUS];

    fn gap(a: &Skin, b: &Skin, dark: bool) -> f32 {
        let (x, y) = (
            a.accent.map_or(0.0, |p| p.get(dark).h),
            b.accent.map_or(0.0, |p| p.get(dark).h),
        );
        let g = (x - y).rem_euclid(360.0);
        g.min(360.0 - g)
    }

    /// Each unit has a face of its own: not plain, and no two siblings sharing both an
    /// accent hue (within 20°) and a material, or a hue and a display
    /// (`docs/round32/BUILDING.md`).
    #[test]
    fn every_tone_unit_has_a_face_of_its_own_unlike_its_siblings() {
        for s in ALL {
            assert!(!s.is_plain(), "{} is plain", s.key);
            assert!(s.accent.is_some(), "{} has a hue of its own", s.key);
        }
        for (i, a) in ALL.iter().enumerate() {
            for b in &ALL[i + 1..] {
                let near = gap(a, b, true) < 20.0 || gap(a, b, false) < 20.0;
                assert!(
                    !(near && a.material == b.material),
                    "{} and {} share a hue and a material",
                    a.key,
                    b.key
                );
                assert!(
                    !(near && a.display == b.display),
                    "{} and {} share a hue and a display",
                    a.key,
                    b.key
                );
            }
        }
    }

    /// Every sheet keeps the kit's contrast promises in every preset — on its tint, under
    /// its accent, over its material.
    #[test]
    fn every_tone_sheet_keeps_the_contrast_floor_in_every_preset() {
        let bad: Vec<String> = ALL
            .iter()
            .flat_map(|s| super::super::contrast_failures(s))
            .collect();
        assert!(bad.is_empty(), "{bad:#?}");
    }
}

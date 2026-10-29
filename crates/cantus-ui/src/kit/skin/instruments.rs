//! The instruments' faces (round 32, ADR 0074). Owned by the `instruments` builder
//! (`docs/round32/BUILDING.md`, `docs/round32/instruments.md` for the design notes).
//!
//! Each sheet is data: the surface ladder's tint, the accent pair (one colour per theme,
//! WCAG AA on the tinted decks and displays — `tests` below hold it), the material, the
//! display's look, the control shapes and the lettering. Inspired by the instruments each
//! family recalls — a wavetable synth's dark glass, a 70s polysynth's brushed panel, an FM
//! synth's membrane and its display, a sampler's anodised case, a bass amplifier's Tolex,
//! an electric piano's lacquer, an organ's walnut, a grand's black lacquer and its
//! soundboard, a drum machine's lamp keys — never a copy of a product's dress or name.
//!
//! No two units share both accent hue (within 20°) and material.

use super::{
    Case, Display, KnobShape, Lettering, Material, Ornaments, Pair, Plate, Skin, SwitchShape, Tint,
    Weight,
};
use crate::kit::color::{mix, shade, with_alpha, Hsl};
use crate::kit::{light, Brush, Fill, State, Style};
use cantus_core::inst::InstrumentKind;
use egui::{Pos2, Rect, Vec2};

/// Organum's switch: a console organ's ivory stop tab in the lamp's place — standing up
/// when off; tipped down when on, its back showing above the face and the pilot amber lit
/// in the tab's foot.
fn stop_tab(b: &mut Brush, rect: Rect, s: &Style, st: State) {
    let t = &s.t;
    let a = st.alpha();
    let f = |c: egui::Color32| c.gamma_multiply(a);
    let px = b.px();
    let d = t.depth.clamp(0.0, 1.5);
    let lamp = s.skin.accent2_in(t.dark).unwrap_or(s.hue);
    let ivory = mix(t.key_white, lamp, 0.16);
    let unit = rect.width().min(rect.height());
    let body = b.snap_rect(Rect::from_center_size(
        rect.center(),
        Vec2::new(unit * 0.78, rect.height().min(unit * 1.5)),
    ));
    let r = s.r(1.5);
    // The slot it sits in.
    b.rrect(
        body.expand(px),
        r,
        Fill::Solid(f(with_alpha(t.dark_shade, 0.55))),
    );
    let lit = if st.hovered { 0.04 } else { 0.0 };
    if st.lit {
        // Tipped down: the back of the tab above, the face foreshortened below.
        let split = body.top() + body.height() * 0.38;
        let back = Rect::from_min_max(body.min, Pos2::new(body.right(), split));
        let face = Rect::from_min_max(Pos2::new(body.left(), split), body.max);
        b.rrect(
            back,
            r,
            Fill::V(f(shade(ivory, -0.34)), f(shade(ivory, -0.22))),
        );
        b.rrect(
            face,
            r,
            Fill::V(
                f(shade(ivory, -0.02 * d + lit)),
                f(shade(ivory, -0.10 * d + lit)),
            ),
        );
        let dot = Rect::from_center_size(
            Pos2::new(face.center().x, face.bottom() - face.height() * 0.3),
            Vec2::new(face.width() * 0.5, (face.height() * 0.18).max(px * 2.0)),
        );
        light::glow(b, dot.center(), unit * 0.6, lamp, 0.55 * a);
        b.rrect(b.snap_rect(dot), px, Fill::Solid(f(lamp)));
    } else {
        // Standing: the whole face, lit from above, a shadow under its lip.
        b.rrect(
            body,
            r,
            Fill::V(
                f(shade(ivory, 0.06 * d + lit)),
                f(shade(ivory, -0.08 * d + lit)),
            ),
        );
        b.hline(
            body.left() + px,
            body.right() - px,
            body.top() + px * 1.5,
            px,
            f(with_alpha(t.light, 0.5)),
        );
        b.hline(
            body.left(),
            body.right(),
            body.bottom() - px * 0.5,
            px,
            f(with_alpha(t.dark_shade, 0.45 * d)),
        );
    }
}

/// Modulus's switch: a flat membrane key with a printed colour bar along its top (an 80s
/// digital synth's) — the bar lit in the unit's hue when on, printed dim when off.
fn membrane(b: &mut Brush, rect: Rect, s: &Style, st: State) {
    let t = &s.t;
    let a = st.alpha();
    let f = |c: egui::Color32| c.gamma_multiply(a);
    let px = b.px();
    let unit = rect.width().min(rect.height());
    let key = b.snap_rect(Rect::from_center_size(
        rect.center(),
        Vec2::new(unit * 0.86, rect.height().min(unit * 1.2)),
    ));
    let face = if st.active {
        shade(t.knob_body, -0.04)
    } else if st.hovered {
        shade(t.knob_body, 0.10)
    } else {
        shade(t.knob_body, 0.06)
    };
    b.rrect(key, s.r(1.0), Fill::Solid(f(with_alpha(t.dark_shade, 0.6))));
    b.rrect(key.shrink(px), s.r(1.0), Fill::Solid(f(face)));
    let bar = b.snap_rect(Rect::from_min_size(
        key.min + Vec2::new(px * 2.0, px * 2.0),
        Vec2::new(key.width() - px * 4.0, (key.height() * 0.24).max(px * 2.0)),
    ));
    if st.lit {
        light::glow(b, bar.center(), unit * 0.6, s.hue, 0.5 * a);
        b.rect(bar, Fill::Solid(f(s.hue)));
    } else {
        b.rect(bar, Fill::Solid(f(mix(face, s.hue, 0.32))));
    }
}

/// An accent: one HSL per theme.
const fn pair(h: f32, s: f32, dark_l: f32, light_l: f32) -> Pair {
    Pair {
        dark: Hsl { h, s, l: dark_l },
        light: Hsl { h, s, l: light_l },
    }
}

const NO_ORNAMENTS: Ornaments = Ornaments {
    screws: false,
    vents: false,
    rules: false,
    rack_ears: false,
};

/// Unda, the wavetable flagship: deep indigo glass, the table glowing cyan on phosphor,
/// a magenta second voice; value rings without bodies, wide-tracked capitals.
pub const UNDA: Skin = Skin {
    key: "inst.wavetable",
    surface: Tint {
        hue: 224.0,
        sat: 0.30,
        amount: 0.75,
    },
    accent: Some(pair(190.0, 0.85, 0.62, 0.26)),
    accent2: Some(pair(305.0, 0.70, 0.82, 0.38)),
    material: Material::Glass,
    display: Display::Phosphor,
    knob: KnobShape::Arc,
    switch: SwitchShape::Pill,
    lettering: Lettering {
        case: Case::Upper,
        tracking: 0.14,
        weight: Weight::Medium,
        condensed: false,
        mono_values: false,
    },
    plate: Plate::Stripe,
    ornaments: NO_ORNAMENTS,
};

/// Calor, the analog polysynth ("heat"): a warm charcoal brushed panel, orange legends,
/// machined caps, rockers, a scope's graticule under its filter and envelopes, screws.
pub const CALOR: Skin = Skin {
    key: "inst.analog",
    surface: Tint {
        hue: 22.0,
        sat: 0.10,
        amount: 0.85,
    },
    accent: Some(pair(24.0, 0.90, 0.75, 0.31)),
    accent2: Some(pair(48.0, 0.90, 0.52, 0.22)),
    material: Material::Brushed,
    display: Display::Scope,
    knob: KnobShape::Cap,
    switch: SwitchShape::Rocker,
    lettering: Lettering {
        case: Case::Upper,
        tracking: 0.08,
        weight: Weight::SemiBold,
        condensed: false,
        mono_values: false,
    },
    plate: Plate::Legend,
    ornaments: Ornaments {
        screws: true,
        vents: false,
        rules: true,
        rack_ears: false,
    },
};

/// Modulus, FM and additive: a matte umber panel (an 80s digital synth's), its operators
/// and spectrum on a liquid-crystal display, membrane keys with a teal-green bar (its own
/// painter, [`membrane`]), soft knobs, condensed capitals and mono readouts.
pub const MODULUS: Skin = Skin {
    key: "inst.fm",
    surface: Tint {
        hue: 18.0,
        sat: 0.18,
        amount: 0.85,
    },
    accent: Some(pair(155.0, 0.65, 0.54, 0.24)),
    accent2: Some(pair(35.0, 0.85, 0.66, 0.27)),
    material: Material::Matte,
    display: Display::Lcd,
    knob: KnobShape::Soft,
    switch: SwitchShape::Custom(membrane),
    lettering: Lettering {
        case: Case::Upper,
        tracking: 0.06,
        weight: Weight::Medium,
        condensed: true,
        mono_values: true,
    },
    plate: Plate::Stripe,
    ornaments: NO_ORNAMENTS,
};

/// Exemplar, the multisample sampler: a grey-violet anodised case, its zones and waveform
/// glowing violet on a phosphor screen (the first sampling workstations' green CRTs, in
/// its own hue), the loop's points in teal, value rings, pills.
pub const EXEMPLAR: Skin = Skin {
    key: "inst.sampler",
    surface: Tint {
        hue: 262.0,
        sat: 0.14,
        amount: 0.8,
    },
    accent: Some(pair(268.0, 0.70, 0.84, 0.50)),
    accent2: Some(pair(175.0, 0.60, 0.56, 0.24)),
    material: Material::Anodised,
    display: Display::Phosphor,
    knob: KnobShape::Arc,
    switch: SwitchShape::Pill,
    lettering: Lettering {
        case: Case::AsIs,
        tracking: 0.02,
        weight: Weight::SemiBold,
        condensed: false,
        mono_values: true,
    },
    plate: Plate::Engraved,
    ornaments: NO_ORNAMENTS,
};

/// Gravis, the bass ("heavy"): a bass amplifier's black Tolex, a deep blue pilot light and
/// a red second voice, chicken-head pointers, bat toggles, a badge, the string and the 808
/// body on a scope.
pub const GRAVIS: Skin = Skin {
    key: "inst.bass",
    surface: Tint {
        hue: 220.0,
        sat: 0.04,
        amount: 0.9,
    },
    accent: Some(pair(222.0, 0.70, 0.77, 0.40)),
    accent2: Some(pair(356.0, 0.80, 0.83, 0.40)),
    material: Material::Tolex,
    display: Display::Scope,
    knob: KnobShape::ChickenHead,
    switch: SwitchShape::Bat,
    lettering: Lettering {
        case: Case::Upper,
        tracking: 0.18,
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

/// Lamina, the electric piano: a 70s keyboard's burgundy lacquer, rose and the tone bars'
/// brass, skirted knobs, the tines and the pickup behind glass.
pub const LAMINA: Skin = Skin {
    key: "inst.electric_piano",
    surface: Tint {
        hue: 340.0,
        sat: 0.14,
        amount: 0.75,
    },
    accent: Some(pair(325.0, 0.65, 0.81, 0.40)),
    accent2: Some(pair(42.0, 0.70, 0.62, 0.28)),
    material: Material::Lacquer,
    display: Display::Glass,
    knob: KnobShape::Skirted,
    switch: SwitchShape::Rocker,
    lettering: Lettering {
        case: Case::AsIs,
        tracking: 0.04,
        weight: Weight::Medium,
        condensed: false,
        mono_values: false,
    },
    plate: Plate::Badge,
    ornaments: NO_ORNAMENTS,
};

/// Organum, the tonewheel organ: walnut cheeks, a magic eye's green and a pilot lamp's
/// amber, numbered dials, ivory stop tabs (its own painter, [`stop_tab`]), the drawbars
/// and the rotor on a printed chart.
pub const ORGANUM: Skin = Skin {
    key: "inst.organ",
    surface: Tint {
        hue: 28.0,
        sat: 0.28,
        amount: 0.8,
    },
    accent: Some(pair(95.0, 0.60, 0.56, 0.25)),
    accent2: Some(pair(35.0, 0.85, 0.66, 0.27)),
    material: Material::Wood,
    display: Display::Paper,
    knob: KnobShape::Dial,
    switch: SwitchShape::Custom(stop_tab),
    lettering: Lettering {
        case: Case::Upper,
        tracking: 0.10,
        weight: Weight::SemiBold,
        condensed: false,
        mono_values: false,
    },
    plate: Plate::Engraved,
    ornaments: Ornaments {
        screws: false,
        vents: false,
        rules: true,
        rack_ears: false,
    },
};

/// Chorda, the grand piano: black lacquer (white lacquer in a light theme), the cast
/// frame's gold and the hammers' felt red; its strings over a spruce soundboard behind
/// glass; black skirted knobs with a gold line; its name on a gold badge, as on a
/// fallboard.
pub const CHORDA: Skin = Skin {
    key: "inst.chorda",
    // Piano black: the theme's ladder with its blue cast taken out (a hair warm), so the
    // lacquer reads black, not the kit's slate.
    surface: Tint {
        hue: 35.0,
        sat: 0.03,
        amount: 1.0,
    },
    // The cast frame's gold, rich enough to read as gold on the rings and the badge.
    accent: Some(pair(43.0, 0.75, 0.60, 0.27)),
    accent2: Some(pair(5.0, 0.70, 0.81, 0.40)),
    material: Material::Lacquer,
    display: Display::Glass,
    knob: KnobShape::Skirted,
    switch: SwitchShape::Pill,
    lettering: Lettering {
        case: Case::AsIs,
        tracking: 0.03,
        weight: Weight::Regular,
        condensed: false,
        mono_values: false,
    },
    plate: Plate::Badge,
    ornaments: NO_ORNAMENTS,
};

/// Pulsus, the drum machine: a dark carbon case, lamp keys that glow 808 yellow with an
/// orange second voice, the hit on a yellow vacuum-fluorescent display, soft knobs, a
/// screen-printed legend, condensed wide capitals.
pub const PULSUS: Skin = Skin {
    key: "inst.pulsus",
    surface: Tint {
        hue: 210.0,
        sat: 0.06,
        amount: 0.9,
    },
    accent: Some(pair(55.0, 0.95, 0.45, 0.20)),
    accent2: Some(pair(12.0, 0.90, 0.80, 0.36)),
    material: Material::Carbon,
    display: Display::Vfd,
    knob: KnobShape::Soft,
    switch: SwitchShape::Lamp,
    lettering: Lettering {
        case: Case::Upper,
        tracking: 0.20,
        weight: Weight::SemiBold,
        condensed: true,
        mono_values: false,
    },
    plate: Plate::Legend,
    ornaments: Ornaments {
        screws: true,
        vents: false,
        rules: true,
        rack_ears: false,
    },
};

/// Every instrument sheet, Pulsus last.
#[cfg(test)]
pub const ALL: [&Skin; 9] = [
    &UNDA, &CALOR, &MODULUS, &EXEMPLAR, &GRAVIS, &LAMINA, &ORGANUM, &CHORDA, &PULSUS,
];

/// The sheet of an instrument.
pub fn of(kind: InstrumentKind) -> &'static Skin {
    match kind {
        InstrumentKind::Wavetable => &UNDA,
        InstrumentKind::Analog => &CALOR,
        InstrumentKind::Fm => &MODULUS,
        InstrumentKind::Sampler => &EXEMPLAR,
        InstrumentKind::Bass => &GRAVIS,
        InstrumentKind::ElectricPiano => &LAMINA,
        InstrumentKind::Organ => &ORGANUM,
        InstrumentKind::Chorda => &CHORDA,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kit::color::contrast;
    use crate::kit::Tokens;

    fn hue_gap(a: f32, b: f32) -> f32 {
        let d = (a - b).rem_euclid(360.0);
        d.min(360.0 - d)
    }

    #[test]
    fn every_instrument_wears_a_face_of_its_own() {
        for s in ALL {
            assert!(!s.is_plain(), "{} is plain", s.key);
            assert!(s.accent.is_some() && s.surface.amount > 0.0, "{}", s.key);
        }
    }

    #[test]
    fn no_two_instruments_share_both_hue_and_material() {
        for (i, a) in ALL.iter().enumerate() {
            for b in &ALL[i + 1..] {
                let (ha, hb) = (a.accent.unwrap().dark.h, b.accent.unwrap().dark.h);
                assert!(
                    a.material != b.material || hue_gap(ha, hb) > 20.0,
                    "{} and {} look alike",
                    a.key,
                    b.key
                );
            }
        }
    }

    /// Each unit's accent, as the sheet gives it (before the kit's floor would move it),
    /// reads 4.5:1 on its tinted decks and card titles in every theme preset — the colour
    /// chosen is the colour seen — and its display, in the look the unit shows (a phosphor,
    /// an LCD, paper…, not the plain ink), has its words at 4.5:1 and both its voices at 3:1.
    #[test]
    fn every_accent_reads_as_chosen_and_every_display_in_its_look_in_every_theme() {
        let mut bad = Vec::new();
        for (name, make) in crate::kit::appearance::PRESETS {
            let base = Tokens::for_theme(&make());
            for s in ALL {
                let t = s.tokens(&base);
                let a = s.accent_in(base.dark).unwrap();
                for (on, bg) in [("deck", t.deck), ("title", t.deck_title)] {
                    let c = contrast(a, bg);
                    if c < 4.5 {
                        bad.push(format!("{name} {}: accent on {on} {c:.2}", s.key));
                    }
                }
                let d = super::super::display_ink(&Style::new(base).skinned(s));
                for (what, fg, need) in [
                    ("text", d.text, 4.5),
                    ("trace", d.trace, 3.0),
                    ("second voice", d.trace2, 3.0),
                ] {
                    let c = contrast(fg, d.ground);
                    if c < need {
                        bad.push(format!("{name} {}: {what} on its display {c:.2}", s.key));
                    }
                }
            }
        }
        assert!(bad.is_empty(), "{}", bad.join("\n"));
    }

    /// Every pairing the kit promises (text 4.5:1 on its surfaces, the unit's hue 4.5:1
    /// on its decks and displays, outlines 3:1) holds in every theme preset, worn.
    #[test]
    fn every_face_keeps_the_kit_s_contrast_in_every_theme() {
        let bad: Vec<String> = ALL
            .iter()
            .flat_map(|s| super::super::contrast_failures(s))
            .collect();
        assert!(bad.is_empty(), "{}", bad.join("\n"));
    }
}

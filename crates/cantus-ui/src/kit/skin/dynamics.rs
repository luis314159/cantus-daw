//! The dynamics family's faces (round 32, ADR 0074). Owned by the `dynamics` builder
//! (`docs/round32/BUILDING.md`, `docs/round32/dynamics.md` for the design notes).
//!
//! Gain reduction is these units' story, so each face is built around the display that
//! tells it: Comprimo's needle and chart, Terminus's ceiling, Multiplex's glowing bands,
//! Sibilus's sibilance on phosphor, Porta's door on a liquid-crystal panel, Culmen's bands
//! on a mastering rack's fluorescent display, Tonsor's transfer curve on an X–Y scope,
//! Impetus's attack under glass and Discrimen's two traces on a dual-beam scope.
//! Inspired by the hardware and plug-ins each recalls — a 70s FET rack compressor, a
//! modern brickwall, a mastering processor's VFD, a broadcast de-esser, a guitarist's
//! noise gate, a drum amplifier, a lab instrument's printed panel — never a copy of a
//! product's dress or name.
//!
//! Every sibling has a material of its own (nine units, nine materials) and an accent hue
//! of its own, spread round the wheel; each sheet is data, plus at most the unit's own
//! knob painter ([`super::KnobShape::Custom`]).

use super::{
    Case, Display, KnobShape, Lettering, Material, Ornaments, Pair, Plate, Skin, SwitchShape, Tint,
    Weight,
};
use crate::kit::brush::{Brush, Fill};
use crate::kit::color::{mix, shade, with_alpha, Hsl};
use crate::kit::knob::{angle, paint_live, value_hue, Geometry, KnobLook};
use crate::kit::{State, Style};
use cantus_core::EffectKind;
use egui::{Pos2, Vec2};

/// An accent: one HSL per theme (light enough to read on a dark deck, dark enough on a
/// light one — the tests below hold both in every preset).
const fn pair(h: f32, s: f32, dark_l: f32, light_l: f32) -> Pair {
    Pair {
        dark: Hsl { h, s, l: dark_l },
        light: Hsl { h, s, l: light_l },
    }
}

const fn tint(hue: f32, sat: f32, amount: f32) -> Tint {
    Tint { hue, sat, amount }
}

const fn letters(
    case: Case,
    tracking: f32,
    weight: Weight,
    condensed: bool,
    mono_values: bool,
) -> Lettering {
    Lettering {
        case,
        tracking,
        weight,
        condensed,
        mono_values,
    }
}

const fn ornaments(screws: bool, vents: bool, rules: bool, rack_ears: bool) -> Ornaments {
    Ornaments {
        screws,
        vents,
        rules,
        rack_ears,
    }
}

/// Comprimo, the compressor: a 70s FET rack unit's black anodised face with rack ears and
/// screws, a backlit amber VU needle reading the gain reduction, its level history and
/// transfer drawn on cream chart paper; black skirted knobs, bat toggles, a badge.
pub const COMPRIMO: Skin = Skin {
    key: "fx.compressor",
    surface: tint(215.0, 0.05, 0.9),
    accent: Some(pair(46.0, 0.92, 0.60, 0.23)),
    accent2: None,
    material: Material::Anodised,
    display: Display::Paper,
    knob: KnobShape::Skirted,
    switch: SwitchShape::Bat,
    lettering: letters(Case::Upper, 0.12, Weight::SemiBold, false, false),
    plate: Plate::Badge,
    ornaments: ornaments(true, false, false, true),
};

/// Terminus, the limiter ("the boundary"): a modern brickwall's flat graphite with a red
/// cast, the ceiling a red line under the name, the waveform and its gain reduction
/// behind dark glass; value rings without bodies, pills, mono readouts.
pub const TERMINUS: Skin = Skin {
    key: "fx.limiter",
    surface: tint(350.0, 0.10, 0.7),
    accent: Some(pair(4.0, 0.88, 0.78, 0.38)),
    accent2: Some(pair(200.0, 0.70, 0.74, 0.34)),
    material: Material::Matte,
    display: Display::Glass,
    knob: KnobShape::Arc,
    switch: SwitchShape::Pill,
    lettering: letters(Case::AsIs, 0.02, Weight::Medium, false, true),
    plate: Plate::Stripe,
    ornaments: ornaments(false, false, false, false),
};

/// Multiplex, multiband dynamics: smoked violet glass, its bands glowing violet and cyan
/// on phosphor, value rings, engraved condensed capitals, rules between the bands' cards.
pub const MULTIPLEX: Skin = Skin {
    key: "fx.multiband",
    surface: tint(262.0, 0.22, 0.75),
    accent: Some(pair(276.0, 0.80, 0.79, 0.45)),
    accent2: Some(pair(186.0, 0.75, 0.62, 0.28)),
    material: Material::Glass,
    display: Display::Phosphor,
    knob: KnobShape::Arc,
    switch: SwitchShape::Pill,
    lettering: letters(Case::Upper, 0.10, Weight::Medium, true, false),
    plate: Plate::Engraved,
    ornaments: ornaments(false, false, true, false),
};

/// Sibilus, the de-esser: a broadcast rack's brushed silver with screws and vents, the
/// sibilance an ice-cyan glow on phosphor, machined caps,
/// rockers, a screen-printed legend, condensed capitals and mono readouts.
pub const SIBILUS: Skin = Skin {
    key: "fx.deesser",
    surface: tint(205.0, 0.06, 0.85),
    accent: Some(pair(180.0, 0.80, 0.52, 0.22)),
    accent2: None,
    material: Material::Brushed,
    display: Display::Phosphor,
    knob: KnobShape::Cap,
    switch: SwitchShape::Rocker,
    lettering: letters(Case::Upper, 0.08, Weight::Medium, true, true),
    plate: Plate::Legend,
    ornaments: ornaments(true, true, false, false),
};

/// Porta, the gate and expander ("the door"): a mahogany door in a wooden case, a green
/// pilot for open and an amber one for shut on a liquid-crystal panel, soft rubber knobs,
/// bat toggles, an engraved name, screws.
pub const PORTA: Skin = Skin {
    key: "fx.gate",
    surface: tint(12.0, 0.30, 0.8),
    accent: Some(pair(138.0, 0.65, 0.58, 0.24)),
    accent2: Some(pair(38.0, 0.90, 0.62, 0.30)),
    material: Material::Wood,
    display: Display::Lcd,
    knob: KnobShape::Soft,
    switch: SwitchShape::Bat,
    lettering: letters(Case::Upper, 0.10, Weight::SemiBold, false, false),
    plate: Plate::Engraved,
    ornaments: ornaments(true, false, false, false),
};

/// Culmen, the mastering maximizer ("the summit"): a mastering rack's deep sapphire
/// lacquer with rack ears, its bands and loudness on a fluorescent display, LED-ring
/// encoders, lamp keys, a badge, wide light capitals.
pub const CULMEN: Skin = Skin {
    key: "fx.maximizer",
    surface: tint(222.0, 0.30, 0.8),
    accent: Some(pair(212.0, 0.85, 0.74, 0.37)),
    accent2: None,
    material: Material::Lacquer,
    display: Display::Vfd,
    knob: KnobShape::Custom(led_ring_encoder),
    switch: SwitchShape::Lamp,
    lettering: letters(Case::Upper, 0.18, Weight::Regular, false, false),
    plate: Plate::Badge,
    ornaments: ornaments(false, false, false, true),
};

/// Tonsor, the clipper ("the shearer"): carbon weave, a hot orange beam drawing the
/// transfer curve on an X–Y scope's graticule, machined caps, bat toggles, a printed
/// legend, vents, condensed capitals and mono readouts.
pub const TONSOR: Skin = Skin {
    key: "fx.clipper",
    surface: tint(20.0, 0.06, 0.9),
    accent: Some(pair(26.0, 0.95, 0.70, 0.31)),
    accent2: None,
    material: Material::Carbon,
    display: Display::Scope,
    knob: KnobShape::Cap,
    switch: SwitchShape::Bat,
    lettering: letters(Case::Upper, 0.14, Weight::SemiBold, true, true),
    plate: Plate::Legend,
    ornaments: ornaments(false, true, false, false),
};

/// Impetus, the transient processor ("the onset"): a drum amplifier's black vinyl covering, a lime
/// pilot for attack and a violet one for sustain behind glass, chicken-head pointers, lamp
/// keys, a badge, screws, condensed wide capitals.
pub const IMPETUS: Skin = Skin {
    key: "fx.transient",
    surface: tint(90.0, 0.02, 0.9),
    accent: Some(pair(80.0, 0.80, 0.52, 0.21)),
    accent2: Some(pair(285.0, 0.70, 0.80, 0.45)),
    material: Material::Tolex,
    display: Display::Glass,
    knob: KnobShape::ChickenHead,
    switch: SwitchShape::Lamp,
    lettering: letters(Case::Upper, 0.16, Weight::SemiBold, true, false),
    plate: Plate::Badge,
    ornaments: ornaments(true, false, false, false),
};

/// Discrimen, the transient splitter ("the dividing line"): a lab instrument's printed
/// card, the transient magenta and the body teal as a dual-beam scope's two channels,
/// numbered dials, pills, a stripe, printed rules, mono readouts.
pub const DISCRIMEN: Skin = Skin {
    key: "fx.transient_split",
    surface: tint(36.0, 0.10, 0.7),
    accent: Some(pair(318.0, 0.70, 0.80, 0.39)),
    accent2: Some(pair(172.0, 0.70, 0.56, 0.26)),
    material: Material::Paper,
    display: Display::Scope,
    knob: KnobShape::Dial,
    switch: SwitchShape::Pill,
    lettering: letters(Case::AsIs, 0.04, Weight::Medium, false, true),
    plate: Plate::Stripe,
    ornaments: ornaments(false, false, true, false),
};

/// Culmen's knob: a mastering rack's encoder — a dark machined cap with a lit index, inside
/// a ring of LEDs that light from the default to the value in the unit's hue (unlit ones a
/// dim glass bead), the default's LED a touch brighter when off, the live modulation dot on
/// the ring. Same contract as the kit's knob: it covers the knob's square, every state.
pub fn led_ring_encoder(b: &mut Brush, c: Pos2, s: &Style, st: State, look: &KnobLook<'_>) {
    let g = Geometry::of(look.size, s);
    let a = st.alpha();
    let f = |col: egui::Color32| col.gamma_multiply(a);
    let t = &s.t;
    let hue = value_hue(s, st);
    let (v, d) = (look.value.clamp(0.0, 1.0), look.default.clamp(0.0, 1.0));
    // The LEDs: one every 4.5 pt of the ring's sweep, an odd count so one sits at the top.
    let sweep = angle(1.0) - angle(0.0);
    let n = ((sweep * g.track_r / s.s(4.5)).round() as i32).clamp(9, 31) | 1;
    let dot = (g.track_w * 0.55).max(s.s(1.1));
    // An unlit LED: a bead of dark glass, between the deck and the track.
    let unlit = mix(t.deck, t.knob_track, if t.dark { 0.35 } else { 0.6 });
    let (lo, hi) = (d.min(v), d.max(v));
    let step = 1.0 / (n - 1) as f32;
    for i in 0..n {
        let p = i as f32 * step;
        let at = c + Vec2::angled(angle(p)) * g.track_r;
        let on =
            !st.disabled && (v - d).abs() > 0.002 && p >= lo - step * 0.5 && p <= hi + step * 0.5;
        if on {
            crate::kit::light::glow(b, at, dot * 3.4, hue, if t.dark { 0.8 } else { 0.35 } * a);
            b.circle(at, dot, Fill::Solid(f(hue)));
            b.circle(
                at - Vec2::splat(dot * 0.3),
                dot * 0.4,
                Fill::Solid(f(with_alpha(t.light, 0.45))),
            );
        } else {
            let base = if (p - d).abs() < step * 0.5 {
                mix(unlit, t.text_dim, 0.45)
            } else {
                unlit
            };
            b.circle(at, dot * 0.8, Fill::Solid(f(base)));
        }
    }
    // The cap: a dark machined body, a bevel lit from above, a fine chamfer ring.
    let body = if t.dark {
        shade(t.knob_body, -0.04)
    } else {
        shade(t.knob_body, -0.02)
    };
    let br = g.body_r;
    b.circle(c, br + s.s(1.0), Fill::Solid(f(shade(body, -0.10))));
    b.circle(
        c,
        br,
        Fill::V(
            f(shade(body, 0.10 * t.depth)),
            f(shade(body, -0.08 * t.depth)),
        ),
    );
    b.circle_stroke(c, br * 0.78, s.s(0.8), f(with_alpha(t.light, 0.10)));
    if st.hovered && !st.disabled {
        b.circle(c, br, Fill::Solid(with_alpha(t.light, 0.06)));
    }
    // The index: a short lit line at the cap's edge.
    let dir = Vec2::angled(angle(v));
    b.segment(
        c + dir * (br * 0.45),
        c + dir * (br * 0.88),
        g.pointer_w,
        f(t.knob_pointer),
        true,
    );
    paint_live(b, c, s, st, look, g.track_r, g.track_w);
}

/// The units whose editors draw a second voice (`views::fx::dynfx::second_voice`, Porta's
/// shut pilot, Multiplex's bands): only these declare one.
#[cfg(test)]
const DRAWS_A_SECOND_VOICE: [&str; 5] = [
    "fx.limiter",
    "fx.multiband",
    "fx.gate",
    "fx.transient",
    "fx.transient_split",
];

/// Every sheet of the family.
#[cfg(test)]
pub const ALL: [&Skin; 9] = [
    &COMPRIMO, &TERMINUS, &MULTIPLEX, &SIBILUS, &PORTA, &CULMEN, &TONSOR, &IMPETUS, &DISCRIMEN,
];

/// The sheet of an effect of this family.
pub fn of(kind: EffectKind) -> Option<&'static Skin> {
    match kind {
        EffectKind::Compressor => Some(&COMPRIMO),
        EffectKind::Limiter => Some(&TERMINUS),
        EffectKind::Multiband => Some(&MULTIPLEX),
        EffectKind::DeEsser => Some(&SIBILUS),
        EffectKind::Gate => Some(&PORTA),
        EffectKind::Maximizer => Some(&CULMEN),
        EffectKind::Clipper => Some(&TONSOR),
        EffectKind::Transient => Some(&IMPETUS),
        EffectKind::TransientSplit => Some(&DISCRIMEN),
        _ => None,
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
    fn every_dynamics_unit_wears_a_face_of_its_own() {
        for s in ALL {
            assert!(!s.is_plain(), "{} is plain", s.key);
            assert!(s.accent.is_some(), "{}", s.key);
            // A second voice only where the editor draws one (review of round 32: colours
            // no one sees made the sheets look more distinct than the units are).
            assert_eq!(
                s.accent2.is_some(),
                DRAWS_A_SECOND_VOICE.contains(&s.key),
                "{}",
                s.key
            );
            assert!(s.surface.amount > 0.0, "{}", s.key);
        }
        for kind in EffectKind::ALL {
            if let Some(s) = of(kind) {
                assert!(ALL.iter().any(|a| a.key == s.key), "{kind:?}");
            }
        }
    }

    /// Siblings never look alike: nine materials for nine units, accents at least 20°
    /// apart round the wheel.
    #[test]
    fn siblings_differ_in_material_and_hue() {
        for (i, a) in ALL.iter().enumerate() {
            for b in &ALL[i + 1..] {
                assert_ne!(a.material, b.material, "{} and {}", a.key, b.key);
                let (ha, hb) = (a.accent.unwrap().dark.h, b.accent.unwrap().dark.h);
                assert!(hue_gap(ha, hb) >= 20.0, "{} and {}", a.key, b.key);
            }
        }
    }

    /// No unit of ours, in any family, shares both accent hue (within 20°) and material
    /// with one of this family's — checked against every sheet that has landed.
    #[test]
    fn no_unit_of_any_family_shares_hue_and_material_with_ours() {
        for unit in super::super::Unit::all() {
            let other = super::super::of(unit);
            let Some(oa) = other.accent else { continue };
            for s in ALL {
                if s.key == other.key {
                    continue;
                }
                let h = s.accent.unwrap().dark.h;
                assert!(
                    s.material != other.material || hue_gap(h, oa.dark.h) > 20.0,
                    "{} and {} look alike",
                    s.key,
                    other.key
                );
            }
        }
    }

    /// The pairings of every theme preset, worn by every unit of the family, that fall
    /// short — only those whose name contains `only`, when given.
    fn short_pairings(only: Option<&str>) -> Vec<String> {
        let mut bad = Vec::new();
        for (name, make) in crate::kit::appearance::PRESETS {
            let base = Tokens::for_theme(&make());
            for s in ALL {
                let t = s.tokens(&base);
                for (what, fg, bg, need) in t.pairings() {
                    if only.is_some_and(|o| !what.contains(o)) {
                        continue;
                    }
                    let c = contrast(fg, bg);
                    if c + 1e-3 < need {
                        bad.push(format!("{name} {}: {what} {c:.2} < {need}", s.key));
                    }
                }
            }
        }
        bad
    }

    /// Each unit's hue (its accent, over every function hue) reads 4.5:1 on its decks,
    /// its card titles and its displays in every theme preset.
    #[test]
    fn every_accent_reads_on_its_decks_and_displays_in_every_theme() {
        let bad = short_pairings(Some(" hue on "));
        assert!(bad.is_empty(), "{}", bad.join("\n"));
    }

    /// The second voice (a display's second trace, the other lamp) is a graphic: 3:1 on
    /// the unit's decks and its ink in every preset.
    #[test]
    fn every_second_voice_shows_on_its_decks_and_displays_in_every_theme() {
        let mut bad = Vec::new();
        for (name, make) in crate::kit::appearance::PRESETS {
            let base = Tokens::for_theme(&make());
            for s in ALL {
                let t = s.tokens(&base);
                let Some(c2) = s.accent2_in(t.dark) else {
                    continue;
                };
                for (n, bg) in [("deck", t.deck), ("ink", t.ink)] {
                    let c = contrast(c2, bg);
                    if c + 1e-3 < 3.0 {
                        bad.push(format!("{name} {}: accent2 on {n} {c:.2}", s.key));
                    }
                }
            }
        }
        assert!(bad.is_empty(), "{}", bad.join("\n"));
    }

    /// Every pairing the kit promises holds in every theme preset, worn — ours and the
    /// kit's own check of a sheet (`skin::contrast_failures`).
    #[test]
    fn every_face_keeps_the_kit_s_contrast_in_every_theme() {
        let mut bad = short_pairings(None);
        for s in ALL {
            bad.extend(super::super::contrast_failures(s));
        }
        assert!(bad.is_empty(), "{}", bad.join("\n"));
    }
}

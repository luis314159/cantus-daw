//! The modulation family's faces (round 32, ADR 0074). Owned by the `modulation` builder
//! (`docs/round32/BUILDING.md`, `docs/round32/modulation.md` for the design notes).
//!
//! Movement is these units' story, so each face is built around the display that tells
//! it: Concentus's bucket-brigade lines under glass, Flexus's comb glowing on a CRT,
//! Phasis's notches on a fluorescent display, Tremor's shape on a chart recorder's paper,
//! Vibrans's pitch wobble on a tuner's liquid-crystal panel, Anulus's sidebands on a lab
//! oscilloscope, Translatio's shifted spectrum on a spectrum analyser's phosphor and
//! Fluctus's drawn shape under dark glass. Inspired by the hardware and plug-ins each
//! recalls — an 80s stereo ensemble rack, a 70s stomp-box flanger and phaser, a 60s
//! amplifier's tremolo channel, a photocell vibe, a modular's ring modulator, a lab
//! frequency shifter, a modern volume shaper — never a copy of a product's dress or name.
//!
//! Every sibling has a material of its own (eight units, eight materials) and an accent
//! hue of its own, spread round the wheel (30°, 52°, 100°, 160°, 206°, 245°, 285°, 330°);
//! each sheet is data, plus Anulus's ring knob ([`super::KnobShape::Custom`]).

use super::{
    Case, Display, KnobShape, Lettering, Material, Ornaments, Pair, Plate, Skin, SwitchShape, Tint,
    Weight,
};
use crate::kit::brush::{Brush, Fill};
use crate::kit::color::{shade, with_alpha, Hsl};
use crate::kit::knob::{angle, paint_live, paint_track, value_hue, Geometry, KnobLook};
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

/// Concentus, the chorus ("singing together"): an 80s stereo ensemble rack's brushed
/// silver with rack ears and screws, its bucket-brigade lines under glass — the left
/// voices ice blue, the right ones amber — machined caps, lamp keys for the modes, a
/// screen-printed legend, wide capitals.
pub const CONCENTUS: Skin = Skin {
    key: "fx.chorus",
    surface: tint(210.0, 0.08, 0.85),
    accent: Some(pair(206.0, 0.80, 0.72, 0.34)),
    accent2: Some(pair(38.0, 0.90, 0.62, 0.30)),
    material: Material::Brushed,
    display: Display::Glass,
    knob: KnobShape::Cap,
    switch: SwitchShape::Lamp,
    lettering: letters(Case::Upper, 0.14, Weight::Medium, false, false),
    plate: Plate::Legend,
    ornaments: ornaments(true, false, false, true),
};

/// Flexus, the through-zero flanger ("the bend"): a 70s stomp box's violet lacquer, its
/// comb glowing violet on a CRT with the zero crossing in lime, skirted knobs, bat
/// toggles, a badge, screws, condensed letters.
pub const FLEXUS: Skin = Skin {
    key: "fx.flanger",
    surface: tint(282.0, 0.22, 0.8),
    accent: Some(pair(285.0, 0.75, 0.80, 0.40)),
    accent2: Some(pair(100.0, 0.70, 0.55, 0.24)),
    material: Material::Lacquer,
    display: Display::Phosphor,
    knob: KnobShape::Skirted,
    switch: SwitchShape::Bat,
    lettering: letters(Case::AsIs, 0.0, Weight::SemiBold, true, false),
    plate: Plate::Badge,
    ornaments: ornaments(true, false, false, false),
};

/// Phasis, the phaser: a 70s stomp box's matte orange powder coat, its notches glowing
/// teal on a fluorescent display, soft rubber knobs, pills, a stripe under the name,
/// screws, capitals and mono readouts.
pub const PHASIS: Skin = Skin {
    key: "fx.phaser",
    surface: tint(24.0, 0.18, 0.75),
    accent: Some(pair(30.0, 0.92, 0.62, 0.30)),
    accent2: Some(pair(178.0, 0.65, 0.55, 0.24)),
    material: Material::Matte,
    display: Display::Vfd,
    knob: KnobShape::Soft,
    switch: SwitchShape::Pill,
    lettering: letters(Case::Upper, 0.06, Weight::SemiBold, false, true),
    plate: Plate::Stripe,
    ornaments: ornaments(true, false, false, false),
};

/// Tremor, tremolo and auto-pan: a 60s amplifier's seafoam vinyl covering with vents, the level's
/// shape on a chart recorder's paper with the other side in amber, numbered dials,
/// rockers, an engraved name.
pub const TREMOR: Skin = Skin {
    key: "fx.tremolo",
    surface: tint(165.0, 0.14, 0.8),
    accent: Some(pair(160.0, 0.60, 0.56, 0.24)),
    accent2: Some(pair(40.0, 0.80, 0.68, 0.30)),
    material: Material::Tolex,
    display: Display::Paper,
    knob: KnobShape::Dial,
    switch: SwitchShape::Rocker,
    lettering: letters(Case::AsIs, 0.02, Weight::Medium, false, false),
    plate: Plate::Engraved,
    ornaments: ornaments(false, true, false, false),
};

/// Vibrans, the vibrato: a photocell vibe's periwinkle lamp behind smoked glass, the
/// pitch's wobble in cents on a tuner's liquid-crystal panel with a rose second voice,
/// value rings, pills, the name etched in the glass, mono readouts.
pub const VIBRANS: Skin = Skin {
    key: "fx.vibrato",
    surface: tint(240.0, 0.16, 0.8),
    accent: Some(pair(245.0, 0.75, 0.82, 0.47)),
    accent2: Some(pair(340.0, 0.70, 0.80, 0.42)),
    material: Material::Glass,
    display: Display::Lcd,
    knob: KnobShape::Arc,
    switch: SwitchShape::Pill,
    lettering: letters(Case::AsIs, 0.08, Weight::Regular, false, true),
    plate: Plate::Engraved,
    ornaments: ornaments(false, false, false, false),
};

/// Anulus, the ring modulator ("the ring"): a modular's ring-modulator module in a
/// walnut case, its sidebands on a lab oscilloscope in gold with the carrier in orchid,
/// knobs that are rings, rockers, a printed legend, screws and rules, condensed capitals
/// and mono readouts.
pub const ANULUS: Skin = Skin {
    key: "fx.ring_mod",
    surface: tint(30.0, 0.24, 0.75),
    accent: Some(pair(52.0, 0.72, 0.56, 0.20)),
    accent2: Some(pair(300.0, 0.60, 0.80, 0.42)),
    material: Material::Wood,
    display: Display::Scope,
    knob: KnobShape::Custom(ring_knob),
    switch: SwitchShape::Rocker,
    lettering: letters(Case::Upper, 0.12, Weight::Medium, true, true),
    plate: Plate::Legend,
    ornaments: ornaments(true, false, true, false),
};

/// Translatio, the frequency shifter ("the carrying across"): a lab instrument's black
/// anodised rack with ears and vents, the spectrum moved on a spectrum analyser's green
/// phosphor with the right sideband in coral, pointer knobs, lamp keys, a stripe, wide
/// light capitals and mono readouts.
pub const TRANSLATIO: Skin = Skin {
    key: "fx.freq_shift",
    surface: tint(95.0, 0.06, 0.9),
    accent: Some(pair(100.0, 0.70, 0.55, 0.25)),
    accent2: Some(pair(14.0, 0.85, 0.72, 0.38)),
    material: Material::Anodised,
    display: Display::Phosphor,
    knob: KnobShape::ChickenHead,
    switch: SwitchShape::Lamp,
    lettering: letters(Case::Upper, 0.18, Weight::Regular, false, true),
    plate: Plate::Stripe,
    ornaments: ornaments(false, true, false, true),
};

/// Fluctus, the volume shaper ("the wave"): a modern shaper's carbon weave, its drawn
/// shape magenta under dark glass with the playhead in cyan, value rings, pills, a badge,
/// printed rules, condensed letters.
pub const FLUCTUS: Skin = Skin {
    key: "fx.volume_shaper",
    surface: tint(325.0, 0.08, 0.85),
    accent: Some(pair(330.0, 0.75, 0.76, 0.37)),
    accent2: Some(pair(190.0, 0.75, 0.58, 0.28)),
    material: Material::Carbon,
    display: Display::Glass,
    knob: KnobShape::Arc,
    switch: SwitchShape::Pill,
    lettering: letters(Case::AsIs, 0.0, Weight::SemiBold, true, false),
    plate: Plate::Badge,
    ornaments: ornaments(false, false, true, false),
};

/// Anulus's knob: a ring — *anulus* is Latin for ring. The body is a torus lit from above,
/// the face showing through its hole, the pointer a notch cut across the ring; round it the
/// kit's track, the value arc from the default, the default's notch and the live dot.
fn ring_knob(b: &mut Brush, c: Pos2, s: &Style, st: State, look: &KnobLook<'_>) {
    let g = Geometry::of(look.size, s);
    let t = &s.t;
    let a = st.alpha();
    let f = |col: egui::Color32| col.gamma_multiply(a);
    paint_track(
        b,
        c,
        s,
        st,
        look,
        (g.track_r, g.track_w),
        g.r,
        value_hue(s, st),
    );
    let outer = g.body_r;
    let inner = outer * 0.46;
    let band = outer - inner;
    let mid = (outer + inner) * 0.5;
    // Its shadow on the face, the rim, the body, the light on its upper half.
    b.circle(
        c + Vec2::new(0.0, s.s(1.0)),
        outer + b.px(),
        Fill::Solid(f(with_alpha(t.dark_shade, 0.30))),
    );
    b.circle_stroke(c, mid, band, f(shade(t.knob_body, -0.07)));
    b.circle_stroke(c, mid, band - 2.0 * b.px(), f(t.knob_body));
    let pi = std::f32::consts::PI;
    b.arc(
        c,
        outer - band * 0.32,
        pi * 1.1,
        pi * 1.9,
        band * 0.34,
        f(with_alpha(t.light, 0.05 + 0.10 * t.depth.min(1.0))),
        false,
    );
    // The face through the hole, in the ring's shadow at the top.
    b.circle(
        c,
        inner,
        Fill::V(f(shade(t.deck, -0.06)), f(shade(t.deck, -0.01))),
    );
    b.circle_stroke(c, inner, b.px(), f(with_alpha(t.dark_shade, 0.45)));
    if st.hovered && !st.disabled {
        let tint = with_alpha(t.light, if t.dark { 0.07 } else { 0.10 });
        b.circle_stroke(c, mid, band, tint);
    }
    // The pointer: a notch across the ring.
    let dir = Vec2::angled(angle(look.value.clamp(0.0, 1.0)));
    b.segment(
        c + dir * (inner + b.px()),
        c + dir * (outer - b.px()),
        g.pointer_w,
        f(t.knob_pointer),
        true,
    );
    paint_live(b, c, s, st, look, g.track_r, g.track_w);
}

/// Every sheet of the family.
#[cfg(test)]
pub const ALL: [&Skin; 8] = [
    &CONCENTUS,
    &FLEXUS,
    &PHASIS,
    &TREMOR,
    &VIBRANS,
    &ANULUS,
    &TRANSLATIO,
    &FLUCTUS,
];

/// The sheet of an effect of this family.
pub fn of(kind: EffectKind) -> Option<&'static Skin> {
    match kind {
        EffectKind::Chorus => Some(&CONCENTUS),
        EffectKind::Flanger => Some(&FLEXUS),
        EffectKind::Phaser => Some(&PHASIS),
        EffectKind::Tremolo => Some(&TREMOR),
        EffectKind::Vibrato => Some(&VIBRANS),
        EffectKind::RingMod => Some(&ANULUS),
        EffectKind::FreqShift => Some(&TRANSLATIO),
        EffectKind::VolumeShaper => Some(&FLUCTUS),
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
    fn every_modulation_unit_wears_a_face_of_its_own() {
        for s in ALL {
            assert!(!s.is_plain(), "{} is plain", s.key);
            assert!(s.accent.is_some() && s.accent2.is_some(), "{}", s.key);
            assert!(s.surface.amount > 0.0, "{}", s.key);
            assert_ne!(s.material, Material::Plain, "{}", s.key);
            assert_ne!(s.display, Display::Plain, "{}", s.key);
        }
        for kind in EffectKind::ALL {
            if let Some(s) = of(kind) {
                assert!(ALL.iter().any(|a| a.key == s.key), "{kind:?}");
            }
        }
        // Every unit of the family has its sheet, and only those.
        let mut keys: Vec<&str> = EffectKind::ALL
            .iter()
            .filter_map(|k| of(*k))
            .map(|s| s.key)
            .collect();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), ALL.len());
    }

    /// Siblings never look alike: eight materials for eight units, accents more than 20°
    /// apart round the wheel.
    #[test]
    fn siblings_differ_in_material_and_hue() {
        for (i, a) in ALL.iter().enumerate() {
            for b in &ALL[i + 1..] {
                assert_ne!(a.material, b.material, "{} and {}", a.key, b.key);
                let (ha, hb) = (a.accent.unwrap().dark.h, b.accent.unwrap().dark.h);
                assert!(hue_gap(ha, hb) > 20.0, "{} and {}", a.key, b.key);
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

    /// The second voice (a display's second trace, the other channel) is a graphic: 3:1
    /// on the unit's decks and its ink in every preset.
    #[test]
    fn every_second_voice_shows_on_its_decks_and_displays_in_every_theme() {
        let mut bad = Vec::new();
        for (name, make) in crate::kit::appearance::PRESETS {
            let base = Tokens::for_theme(&make());
            for s in ALL {
                let t = s.tokens(&base);
                let c2 = s.accent2_in(t.dark).unwrap();
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

    /// Every pairing the kit promises holds in every theme preset, worn.
    #[test]
    fn every_face_keeps_the_kit_s_contrast_in_every_theme() {
        let mut bad = short_pairings(None);
        for s in ALL {
            bad.extend(super::super::contrast_failures(s));
        }
        assert!(bad.is_empty(), "{}", bad.join("\n"));
    }
}

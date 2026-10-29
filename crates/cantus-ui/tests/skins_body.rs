//! A unit's body paints in its face (round 32, ADR 0074): no editor can lose its skin by
//! building its own style from the plain tokens.
//!
//! The installed tokens' surfaces (the ladder a skin tints) are set to a sentinel pink;
//! every unit then wears a probe skin that tints them violet. Each editor is drawn in its
//! window, and every vertex painted inside its body (the device ground's rect) is read:
//! one still in the sentinel's hue was painted from the plain tokens — a part of the body
//! that would not wear the unit's face. Its own test binary: it installs tokens for the
//! whole process.

use cantus_core::inst::InstrumentKind;
use cantus_core::EffectKind;
use cantus_engine::EngineHandle;
use cantus_ui::kit::color::Hsl;
use cantus_ui::kit::skin::{self, Pair, Skin, Tint};
use cantus_ui::kit::{inspect, Tok, Tokens};
use cantus_ui::{views, welcome_project, CantusApp};
use egui::{Pos2, Rect, Shape};

/// The sentinel's hue, and the probe skin's.
const SENTINEL: f32 = 330.0;
const PROBE_HUE: f32 = 250.0;

static PROBE: Skin = Skin {
    surface: Tint {
        hue: PROBE_HUE,
        sat: 0.45,
        amount: 1.0,
    },
    accent: Some(Pair::both(Hsl {
        h: PROBE_HUE,
        s: 0.7,
        l: 0.6,
    })),
    ..Skin::plain("test.probe")
};

/// The surfaces `Skin::tokens` tints.
const SURFACES: [Tok; 13] = [
    Tok::Ink,
    Tok::Well,
    Tok::Panel,
    Tok::PanelRaised,
    Tok::PanelLight,
    Tok::DeviceGround,
    Tok::Deck,
    Tok::DeckTitle,
    Tok::ValuePill,
    Tok::MeterWell,
    Tok::KnobBody,
    Tok::KnobRim,
    Tok::KnobTrack,
];

fn sentinel_tokens() -> Tokens {
    let mut t = Tokens::slate();
    for tok in SURFACES {
        let h = Hsl::of(t.get(tok));
        t.set(
            tok,
            Hsl {
                h: SENTINEL,
                s: 0.5,
                l: h.l.clamp(0.08, 0.92),
            }
            .rgb(),
        );
    }
    t
}

#[derive(Copy, Clone)]
enum What {
    Fx(EffectKind),
    Inst(#[allow(dead_code)] InstrumentKind),
    Pulsus,
}

const UNITS: &[(&str, What)] = &[
    ("eq", What::Fx(EffectKind::Eq)),
    ("comp", What::Fx(EffectKind::Compressor)),
    ("limiter", What::Fx(EffectKind::Limiter)),
    ("multiband", What::Fx(EffectKind::Multiband)),
    ("deesser", What::Fx(EffectKind::DeEsser)),
    ("gate", What::Fx(EffectKind::Gate)),
    ("culmen", What::Fx(EffectKind::Maximizer)),
    ("tonsor", What::Fx(EffectKind::Clipper)),
    ("impetus", What::Fx(EffectKind::Transient)),
    ("discrimen", What::Fx(EffectKind::TransientSplit)),
    ("fervor", What::Fx(EffectKind::Saturator)),
    ("space", What::Fx(EffectKind::Space)),
    ("latitudo", What::Fx(EffectKind::Stereo)),
    ("speculum", What::Fx(EffectKind::Analyzer)),
    ("tonus", What::Fx(EffectKind::AutoTune)),
    ("concentus", What::Fx(EffectKind::Chorus)),
    ("flexus", What::Fx(EffectKind::Flanger)),
    ("phasis", What::Fx(EffectKind::Phaser)),
    ("tremor", What::Fx(EffectKind::Tremolo)),
    ("vibrans", What::Fx(EffectKind::Vibrato)),
    ("anulus", What::Fx(EffectKind::RingMod)),
    ("translatio", What::Fx(EffectKind::FreqShift)),
    ("fluctus", What::Fx(EffectKind::VolumeShaper)),
    ("unda", What::Inst(InstrumentKind::Wavetable)),
    ("calor", What::Inst(InstrumentKind::Analog)),
    ("modulus", What::Inst(InstrumentKind::Fm)),
    ("exemplar", What::Inst(InstrumentKind::Sampler)),
    ("gravis", What::Inst(InstrumentKind::Bass)),
    ("lamina", What::Inst(InstrumentKind::ElectricPiano)),
    ("organum", What::Inst(InstrumentKind::Organ)),
    ("chorda", What::Inst(InstrumentKind::Chorda)),
    ("pulsus", What::Pulsus),
];

/// Every mesh vertex of `shapes` inside `rect`, with its colour.
fn vertices_in(
    shapes: &[egui::epaint::ClippedShape],
    rect: Rect,
    out: &mut Vec<(Pos2, egui::Color32)>,
) {
    fn walk(s: &Shape, rect: Rect, out: &mut Vec<(Pos2, egui::Color32)>) {
        match s {
            Shape::Mesh(m) => {
                for v in &m.vertices {
                    if rect.contains(v.pos) {
                        out.push((v.pos, v.color));
                    }
                }
            }
            Shape::Vec(v) => v.iter().for_each(|s| walk(s, rect, out)),
            _ => {}
        }
    }
    for s in shapes {
        walk(&s.shape, rect, out);
    }
}

fn in_sentinel_hue(c: egui::Color32) -> bool {
    if c.a() < 40 {
        return false;
    }
    let [r, g, b, a] = c.to_array();
    // Unmultiply (vertex colours are premultiplied).
    let un = |v: u8| ((v as f32 * 255.0 / a as f32).round().min(255.0)) as u8;
    let h = Hsl::of(egui::Color32::from_rgb(un(r), un(g), un(b)));
    // The sentinel exactly — its hue and its saturation (shades keep both); a palette
    // colour of the unit's own (Spatium's band handles) is not one.
    (h.s - 0.5).abs() < 0.06 && (h.h - SENTINEL).abs() < 5.0
}

fn in_probe_hue(c: egui::Color32) -> bool {
    if c.a() < 40 {
        return false;
    }
    let [r, g, b, a] = c.to_array();
    let un = |v: u8| ((v as f32 * 255.0 / a as f32).round().min(255.0)) as u8;
    let h = Hsl::of(egui::Color32::from_rgb(un(r), un(g), un(b)));
    h.s > 0.2 && (h.h - PROBE_HUE).abs() < 8.0
}

#[test]
fn every_units_body_paints_in_its_face_never_with_the_plain_tokens() {
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
    skin::set_override(Some(&PROBE));
    let mut problems = Vec::new();
    // Every unit, then a control: the first one with faces off, which must be caught.
    let runs = UNITS
        .iter()
        .map(|u| (true, u))
        .chain(std::iter::once((false, &UNITS[0])));
    for (faces, (name, what)) in runs {
        skin::set_enabled(faces);
        let ctx = egui::Context::default();
        cantus_ui::design::install_fonts(&ctx);
        cantus_ui::design::install_style(&ctx);
        cantus_ui::theme::apply(&cantus_ui::theme::slate(), &ctx);
        cantus_ui::kit::tokens::install(sentinel_tokens());
        inspect::set_enabled(&ctx, true);
        let mut app = CantusApp::from_parts(Some(welcome_project()), EngineHandle::dummy(), false);
        app.demo_editor(name);
        if let What::Fx(kind) = what {
            views::fx::demo_feed(&mut app, *kind);
        }
        app.fx.pos = Some(Pos2::ZERO);
        let size = egui::vec2(1400.0, 900.0);
        let mut last = None;
        for i in 0..4 {
            let input = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
                time: Some(i as f64 / 60.0),
                ..Default::default()
            };
            let mut out = ctx.run_ui(input, |ui| match what {
                What::Fx(_) => {
                    let c = ui.ctx().clone();
                    views::eq_editor::window(&mut app, &c);
                    views::comp_editor::window(&mut app, &c);
                    views::fx::window(&mut app, &c);
                }
                What::Inst(_) => views::inst::window_in(&mut app, ui),
                What::Pulsus => views::pulsus_editor::window_in(&mut app, ui),
            });
            out.textures_delta.clear();
            last = Some(out);
        }
        let out = last.expect("frames ran");
        let items = inspect::items(&ctx);
        let Some(body) = items.iter().find(|i| i.part == "device_ground") else {
            problems.push(format!("{name}: no device ground painted"));
            continue;
        };
        let worn = items
            .iter()
            .filter(|i| body.rect.contains_rect(i.rect))
            .all(|i| i.skin == "test.probe");
        if !worn && faces {
            problems.push(format!(
                "{name}: a part of the body registered outside the unit's face"
            ));
        }
        let mut verts = Vec::new();
        vertices_in(&out.shapes, body.rect.shrink(1.0), &mut verts);
        let leaked: Vec<_> = verts.iter().filter(|(_, c)| in_sentinel_hue(*c)).collect();
        if !faces {
            if leaked.is_empty() {
                problems.push(format!("{name}, faces off: the plain tokens went unseen"));
            }
            continue;
        }
        if !verts.iter().any(|(_, c)| in_probe_hue(*c)) {
            problems.push(format!(
                "{name}: nothing in the body wears the probe's tint"
            ));
        }
        if !leaked.is_empty() {
            let at = leaked[0].0;
            let parts: Vec<&str> = items
                .iter()
                .filter(|i| i.rect.contains(at))
                .map(|i| i.part)
                .collect();
            let bb = leaked.iter().fold(Rect::NOTHING, |r, (p, _)| {
                r.union(Rect::from_min_max(*p, *p))
            });
            let mut cols: Vec<_> = leaked.iter().map(|(_, c)| *c).collect();
            cols.dedup();
            eprintln!("{name}: box {bb:?} colours {cols:?} under {parts:?}");
            problems.push(format!(
                "{name}: {} of {} vertices painted with the plain tokens, the first at {:?}",
                leaked.len(),
                verts.len(),
                leaked[0].0
            ));
        }
    }
    skin::set_override(None);
    skin::set_enabled(true);
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

/// The review of delivery 2: the header's menus (⋯, presets, sizes) wore the face for an
/// effect but not for an instrument or Pulsus — their popups ran after the shell took its
/// face off. Each unit's open menu is painted in its own face.
#[test]
fn every_unit_s_header_menu_wears_its_face() {
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
    skin::set_override(None);
    skin::set_enabled(true);
    let mut bad = Vec::new();
    for (name, what) in [
        ("unda", What::Inst(InstrumentKind::Wavetable)),
        ("calor", What::Inst(InstrumentKind::Analog)),
        ("pulsus", What::Pulsus),
    ] {
        let ctx = egui::Context::default();
        cantus_ui::design::install_fonts(&ctx);
        cantus_ui::design::install_style(&ctx);
        cantus_ui::theme::apply(&cantus_ui::theme::slate(), &ctx);
        inspect::set_enabled(&ctx, true);
        let mut app = CantusApp::from_parts(Some(welcome_project()), EngineHandle::dummy(), false);
        app.demo_editor(name);
        let want = match what {
            What::Inst(kind) => {
                app.inst.popup = views::inst::Popup::More;
                skin::of_instrument(kind).key
            }
            _ => {
                app.pulsus.popup = views::pulsus_editor::Popup::More;
                skin::of(skin::Unit::Pulsus).key
            }
        };
        for i in 0..3 {
            let input = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(1400.0, 900.0))),
                time: Some(i as f64 / 60.0),
                ..Default::default()
            };
            let mut out = ctx.run_ui(input, |ui| match what {
                What::Inst(_) => views::inst::window_in(&mut app, ui),
                _ => views::pulsus_editor::window_in(&mut app, ui),
            });
            out.textures_delta.clear();
        }
        let menus: Vec<&str> = inspect::items(&ctx)
            .iter()
            .filter(|i| i.part == "menu")
            .map(|i| i.skin)
            .collect();
        if menus.is_empty() {
            bad.push(format!("{name}: no menu painted"));
        } else if menus.iter().any(|k| *k != want) {
            bad.push(format!("{name}: the menu painted in {menus:?}, not {want}"));
        }
    }
    assert!(bad.is_empty(), "{bad:#?}");
}

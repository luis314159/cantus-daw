//! Each tone unit paints the second voice its sheet declares (round 32, the review of the
//! skin's delivery 2): Aequalis's analyser, Fervor's odd harmonics, Spatium's early
//! reflections, Latitudo's side, Speculum's held peak and Tonus's sung pitch are drawn in
//! the display look's second voice (`Ink::of(s).look.trace2`), in Slate and in Paper.

use cantus_core::EffectKind;
use cantus_engine::EngineHandle;
use cantus_ui::kit::skin;
use cantus_ui::kit::{inspect, Style};
use cantus_ui::{views, welcome_project, CantusApp};
use egui::{Pos2, Rect, Shape};

const UNITS: &[(&str, EffectKind)] = &[
    ("eq", EffectKind::Eq),
    ("fervor", EffectKind::Saturator),
    ("space", EffectKind::Space),
    ("latitudo", EffectKind::Stereo),
    ("speculum", EffectKind::Analyzer),
    ("tonus", EffectKind::AutoTune),
];

/// The colour of every vertex inside `rect` that is not nearly clear, unmultiplied.
fn colours_in(shapes: &[egui::epaint::ClippedShape], rect: Rect, out: &mut Vec<[u8; 3]>) {
    fn walk(s: &Shape, rect: Rect, out: &mut Vec<[u8; 3]>) {
        match s {
            Shape::Mesh(m) => {
                for v in &m.vertices {
                    if rect.contains(v.pos) && v.color.a() >= 100 {
                        let [r, g, b, _] = v.color.to_srgba_unmultiplied();
                        out.push([r, g, b]);
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

#[test]
fn every_tone_unit_paints_its_second_voice() {
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
    skin::set_override(None);
    skin::set_enabled(true);
    let mut missing = Vec::new();
    for (theme_name, theme) in [
        ("slate", cantus_ui::theme::slate()),
        ("paper", cantus_ui::theme::paper()),
    ] {
        for (name, kind) in UNITS {
            let ctx = egui::Context::default();
            cantus_ui::design::install_fonts(&ctx);
            cantus_ui::design::install_style(&ctx);
            cantus_ui::theme::apply(&theme, &ctx);
            inspect::set_enabled(&ctx, true);
            let mut app =
                CantusApp::from_parts(Some(welcome_project()), EngineHandle::dummy(), false);
            app.demo_editor(name);
            views::fx::demo_feed(&mut app, *kind);
            app.fx.pos = Some(Pos2::ZERO);
            let mut last = None;
            for i in 0..6 {
                let input = egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(1400.0, 900.0))),
                    time: Some(i as f64 / 60.0),
                    ..Default::default()
                };
                let mut out = ctx.run_ui(input, |ui| {
                    let c = ui.ctx().clone();
                    views::eq_editor::window(&mut app, &c);
                    views::fx::window(&mut app, &c);
                });
                out.textures_delta.clear();
                last = Some(out);
            }
            let out = last.expect("frames ran");
            let sheet = skin::of_effect(*kind);
            let s = Style::plain().skinned(sheet);
            let want = skin::display_ink(&s).trace2.to_srgba_unmultiplied();
            let items = inspect::items(&ctx);
            let mut seen = Vec::new();
            for d in items.iter().filter(|i| i.part == "display") {
                colours_in(&out.shapes, d.rect, &mut seen);
            }
            let close = |c: &[u8; 3]| (0..3).all(|i| c[i].abs_diff(want[i]) <= 6);
            if !seen.iter().any(close) {
                missing.push(format!(
                    "{name} ({theme_name}): no shape in its display in the second voice {want:?}"
                ));
            }
        }
    }
    assert!(missing.is_empty(), "{missing:#?}");
}

//! Helpers every plug-in's UI tests share: `mod support;` at the top of a test file.
#![allow(dead_code)]

use cantus_core::Project;
use cantus_engine::EngineHandle;
use cantus_ui::CantusApp;

/// An app with no audio device, in English.
pub fn app(project: Project) -> CantusApp {
    cantus_ui::i18n::default_thread_locale(cantus_ui::i18n::Locale::En);
    CantusApp::from_parts(Some(project), EngineHandle::dummy(), false)
}

/// A context set up as the real one is: bundled fonts and the type scale. The test's
/// thread speaks English unless it chose otherwise, as in [`app`], so tests that read
/// painted text do not depend on the machine's `LANG` (a Spanish system painted "Sin
/// bandas") nor on another test switching language at the same time.
pub fn ctx() -> egui::Context {
    cantus_ui::i18n::default_thread_locale(cantus_ui::i18n::Locale::En);
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    ctx
}

/// Every piece of text a frame painted, joined with `" | "`.
pub fn painted_text(shapes: &[egui::epaint::ClippedShape]) -> String {
    fn walk(shape: &egui::Shape, out: &mut String) {
        match shape {
            egui::Shape::Text(t) => {
                out.push_str(t.galley.text());
                out.push_str(" | ");
            }
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
            _ => {}
        }
    }
    let mut out = String::new();
    for s in shapes {
        walk(&s.shape, &mut out);
    }
    out
}

/// Run `frames` frames of `draw` at `size` and return the text the last one painted.
pub fn paint(
    ctx: &egui::Context,
    size: egui::Vec2,
    frames: usize,
    mut draw: impl FnMut(&mut egui::Ui),
) -> String {
    let mut text = String::new();
    for i in 0..frames.max(1) {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
            time: Some(i as f64 * 0.25),
            ..Default::default()
        };
        let mut out = ctx.run_ui(input, |ui| draw(ui));
        out.textures_delta.clear();
        text = painted_text(&out.shapes);
    }
    text
}

/// The editor window of whatever effect editor is open, painted headless.
pub fn paint_editors(app: &mut CantusApp, size: egui::Vec2) -> String {
    let ctx = ctx();
    paint(&ctx, size, 3, |ui| {
        let ctx = ui.ctx().clone();
        cantus_ui::views::eq_editor::window(app, &ctx);
        cantus_ui::views::comp_editor::window(app, &ctx);
        cantus_ui::views::fx::window(app, &ctx);
    })
}

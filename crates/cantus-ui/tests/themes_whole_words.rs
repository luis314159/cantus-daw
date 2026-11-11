//! Round 39, *themes*: every shipped theme keeps the whole window's words whole (ADR 0109).
//! A theme brings its face (Barlow, Sofia, Plex), and a face sets every width: the top bar's
//! menus are all there or folded together into ≡ (never Help dropped off the end), and no
//! word of the window — the bar, the panels, the status line — is elided or left out for
//! want of room, in every theme, at 1920 × 1080 at 125 % and at 1600 × 900 (100 % and
//! 125 %). The app `CantusApp::assemble` puts together on the song it opens with, playing.
//!
//! Its own test binary: the installed tokens and the face are process-wide.

use cantus_engine::EngineHandle;
use cantus_ui::app::Files;
use cantus_ui::kit::appearance::{Mode, PRESETS};
use cantus_ui::kit::text::probe;
use cantus_ui::CantusApp;
use egui::{Pos2, Rect};

const MENUS: [&str; 6] = ["File", "Edit", "Clip", "View", "Options", "Help"];

struct Rig {
    app: CantusApp,
    ctx: egui::Context,
    t: f64,
    size: egui::Vec2,
}

impl Rig {
    fn new(dir: &std::path::Path) -> Rig {
        std::fs::write(
            dir.join("settings.json"),
            r#"{"audio_setup":{"setup_done":true}}"#,
        )
        .unwrap();
        std::env::set_var("CANTUS_DATA_DIR", dir);
        cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
        let files = Files::read(dir, Some(cantus_ui::i18n::Locale::En));
        let ctx = egui::Context::default();
        cantus_ui::design::install_fonts(&ctx);
        cantus_ui::design::install_style(&ctx);
        let (engine, _proc) = EngineHandle::manual(48_000.0);
        let mut app = CantusApp::assemble(
            &ctx,
            files,
            Some(cantus_ui::demos::startup_project()),
            engine,
            None,
            None,
            None,
            false,
        );
        app.workflow.start.close();
        app.toggle_play();
        Rig {
            app,
            ctx,
            t: 0.0,
            size: egui::vec2(1536.0, 864.0),
        }
    }

    /// One frame; every text egui painted, and whether it was elided.
    fn frame(&mut self) -> Vec<(String, bool)> {
        self.t += 1.0 / 60.0;
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, self.size)),
            time: Some(self.t),
            predicted_dt: 1.0 / 60.0,
            ..Default::default()
        };
        let app = &mut self.app;
        let mut out = self.ctx.run_ui(input, |ui| app.frame(ui));
        out.textures_delta.clear();
        fn walk(shape: &egui::Shape, out: &mut Vec<(String, bool)>) {
            match shape {
                egui::Shape::Text(t) => out.push((t.galley.text().to_string(), t.galley.elided)),
                egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
                _ => {}
            }
        }
        let mut texts = Vec::new();
        for c in &out.shapes {
            walk(&c.shape, &mut texts);
        }
        texts
    }

    fn at(&mut self, screen: (f32, f32), scale: f32) {
        self.ctx.set_pixels_per_point(scale);
        self.size = egui::vec2(screen.0, screen.1) / scale;
        for _ in 0..4 {
            self.frame();
        }
    }
}

/// A data folder whose appearance names `id` in its own light, as a user's would after
/// choosing it: the program starts in it.
fn data_in(id: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let dark = PRESETS
        .iter()
        .find(|(k, _)| *k == id)
        .is_none_or(|(_, m)| m().dark);
    let mut a = cantus_ui::kit::appearance::Appearance {
        mode: if dark { Mode::Dark } else { Mode::Light },
        ..Default::default()
    };
    a.assign(a.mode, format!("preset:{id}"));
    std::fs::write(dir.path().join("appearance.json"), a.to_json()).unwrap();
    dir
}

#[test]
fn every_theme_keeps_the_menus_and_the_status_line_whole() {
    let mut bad = Vec::new();
    for (id, _) in PRESETS {
        let dir = data_in(id);
        let mut r = Rig::new(dir.path());
        for (screen, scale) in [
            ((1920.0, 1080.0), 1.25),
            ((1600.0, 900.0), 1.0),
            ((1600.0, 900.0), 1.25),
        ] {
            r.at(screen, scale);
            probe::start();
            let texts = r.frame();
            let at = format!("{id} at {}x{} {}%", screen.0, screen.1, scale * 100.0);
            assert!(
                texts.iter().any(|(t, _)| t == "Arrangement"),
                "{at}: the window's words are read ({} texts)",
                texts.len()
            );
            let menus = MENUS
                .iter()
                .filter(|m| texts.iter().any(|(t, _)| t == *m))
                .count();
            if menus != 0 && menus != MENUS.len() {
                let missing: Vec<&str> = MENUS
                    .iter()
                    .filter(|m| !texts.iter().any(|(t, _)| t == **m))
                    .copied()
                    .collect();
                bad.push(format!("{at}: the menus lack {missing:?}"));
            }
            for c in probe::take() {
                bad.push(format!(
                    "{at}: {:?} shown as {:?} ({})",
                    c.full, c.shown, c.how
                ));
            }
            for (t, elided) in texts {
                if elided {
                    bad.push(format!("{at}: {t:?} elided"));
                }
            }
        }
    }
    bad.dedup();
    assert!(bad.is_empty(), "{} cut:\n{}", bad.len(), bad.join("\n"));
}

//! Screenshots of the export dialog (ADR 0033) without a screen:
//! `cargo run -p cantus-ui --example shoot_export -- <out-dir> [scale] [locale]`.
//! The arrangement with the dialog over it, in the light and the dark theme: the
//! Streaming preset (MP3, −14 LUFS), the stems for a mastering engineer, the podcast, a
//! custom FLAC with tags, and an export under way.

use cantus_engine::EngineHandle;
use cantus_export::{Depth, Dither, Format, Preset};
use cantus_ui::raster::shoot;
use cantus_ui::views::{self, export, export_dialog};
use cantus_ui::{welcome_project, CantusApp};
use std::path::PathBuf;

fn main() {
    let mut args = std::env::args().skip(1);
    let dir = PathBuf::from(args.next().unwrap_or_else(|| "shots".into()));
    let scale: f32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(1.0);
    let locale = match args.next().as_deref() {
        Some("es") => cantus_ui::i18n::Locale::Es,
        _ => cantus_ui::i18n::Locale::En,
    };
    std::fs::create_dir_all(&dir).expect("output dir");
    cantus_ui::i18n::set_locale(locale);
    let code = locale.code();
    let scratch = std::env::temp_dir().join("cantus-shoot-export");
    let _ = std::fs::create_dir_all(&scratch);

    for kind in ["streaming", "stems", "podcast", "flac", "running"] {
        for dark in [false, true] {
            let ctx = egui::Context::default();
            cantus_ui::design::install_fonts(&ctx);
            cantus_ui::design::install_style(&ctx);
            let theme = if dark {
                cantus_ui::theme::ink_and_gold()
            } else {
                cantus_ui::theme::default_theme()
            };
            cantus_ui::theme::apply(&theme, &ctx);
            let mut app =
                CantusApp::from_parts(Some(welcome_project()), EngineHandle::dummy(), false);
            app.settings_path = scratch.join("settings.json");
            export::open(&mut app, None);
            {
                let d = app.export.dialog.as_mut().unwrap();
                d.tags.artist = "Luis Ángel Almazán".into();
                d.tags.year = "2026".into();
                *d = match kind {
                    "streaming" | "running" => Preset::Streaming.apply(d),
                    "stems" => Preset::MasteringStems.apply(d),
                    "podcast" => Preset::Podcast.apply(d),
                    _ => {
                        let mut c = d.clone();
                        c.format = Format::Flac;
                        c.depth = Depth::Int16;
                        c.dither = Dither::Shaped;
                        c.tags.album = "Salmos".into();
                        c.tags.comment = "Colosenses 3:23".into();
                        c
                    }
                };
            }
            if kind == "running" {
                let d = app.export.dialog.as_mut().unwrap();
                d.tail_seconds = 30.0;
                export::start(&mut app, &scratch.join("Welcome.mp3"));
            }
            let theme_name = if dark { "dark" } else { "light" };
            let path = dir.join(format!("export-{kind}-{theme_name}-{code}.png"));
            shoot(
                &ctx,
                egui::vec2(1280.0, 800.0),
                scale,
                6,
                Some(&path),
                |root| {
                    let ctx = root.ctx().clone();
                    egui::Panel::top(egui::Id::new("t"))
                        .show(root, |ui| views::transport::show(&mut app, ui));
                    egui::CentralPanel::default()
                        .show(root, |ui| views::arrangement::show(&mut app, ui));
                    export_dialog::show(&mut app, &ctx);
                },
            );
            if app.export.busy() {
                export::stop(&mut app);
                export::wait(&mut app);
            }
            println!("{}", path.display());
        }
    }
}

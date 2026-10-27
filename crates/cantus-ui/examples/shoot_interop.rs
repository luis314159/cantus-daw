//! Screenshots of the interchange dialogs (ADR 0021) without a screen:
//! `cargo run -p cantus-ui --example shoot_interop -- <out-dir> [scale] [locale]`.
//! The arrangement with each dialog open over it — MIDI import, MIDI export, the report
//! of a foreign DAWproject — in the light and the dark theme. (Stems are exported from the
//! export dialog: `shoot_export`.)

use cantus_engine::EngineHandle;
use cantus_project::{dawproject, midi};
use cantus_ui::raster::shoot;
use cantus_ui::views::{self, interop};
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

    // A MIDI file of the welcome song, to import back, and a foreign DAWproject's report.
    let song = midi::export(&welcome_project(), &midi::ExportOptions::default()).unwrap();
    let mid = std::env::temp_dir().join("cantus-shoot-song.mid");
    std::fs::write(&mid, &song).unwrap();
    let foreign = {
        let mut z = dawproject::zip::ZipWriter::new();
        let xml = include_str!("../../cantus-project/tests/fixtures/dawproject/foreign.xml");
        z.add("project.xml", xml.as_bytes(), true);
        z.finish()
    };
    let report = dawproject::import(&foreign).unwrap().report;

    for kind in ["midi-import", "midi-export", "report"] {
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
            match kind {
                "midi-import" => interop::open_midi_import(&mut app, &mid),
                "midi-export" => {
                    app.interop.midi_export = Some(interop::MidiExportDialog {
                        scope: interop::MidiScope::Song,
                        ppq: 960,
                        played: true,
                    })
                }
                _ => {
                    app.interop.report = Some(interop::ReportWindow {
                        title: cantus_ui::i18n::tf(
                            "interop.report.title_import",
                            &[("file", &"Demo.dawproject")],
                        ),
                        report: report.clone(),
                    })
                }
            }
            let theme_name = if dark { "dark" } else { "light" };
            let path = dir.join(format!("interop-{kind}-{theme_name}-{code}.png"));
            shoot(
                &ctx,
                egui::vec2(1200.0, 760.0),
                scale,
                6,
                Some(&path),
                |root| {
                    let ctx = root.ctx().clone();
                    egui::Panel::top(egui::Id::new("t"))
                        .show(root, |ui| views::transport::show(&mut app, ui));
                    egui::CentralPanel::default()
                        .show(root, |ui| views::arrangement::show(&mut app, ui));
                    interop::frame(&mut app, &ctx);
                },
            );
            println!("{}", path.display());
        }
    }
}

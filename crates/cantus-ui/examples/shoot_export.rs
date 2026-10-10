//! Screenshots of the export dialog (ADR 0033) without a screen:
//! `cargo run -p cantus-ui --example shoot_export -- <out-dir> [scale] [locale]`.
//! The arrangement with the dialog over it, in the light and the dark theme: the
//! Streaming preset (MP3, −14 LUFS), the stems for a mastering engineer, the podcast, a
//! custom FLAC with tags, an export under way (its bar following the render), and from
//! round 23 the sample-rate list open over the dialog (clicked as a musician clicks it),
//! the time selection, the loop brace, a range asked for that is not there, the stems of
//! the tracks through the master chain, and a finished export's summary.

use cantus_core::Ticks;
use cantus_engine::EngineHandle;
use cantus_export::{Depth, Dither, Format, Preset, Rate, Scope};
use cantus_ui::raster::shoot_events;
use cantus_ui::views::{self, export, export_dialog};
use cantus_ui::{welcome_project, CantusApp};
use std::path::PathBuf;

const KINDS: [&str; 11] = [
    "streaming",
    "stems",
    "podcast",
    "flac",
    "running",
    "list",
    "selection",
    "loop",
    "no-range",
    "stems-tracks",
    "done",
];

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

    for kind in KINDS {
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
            let mut project = welcome_project();
            if kind == "stems-tracks" {
                // A master chain, so *Through the master chain* is offered.
                let fx = project.make_effect(cantus_core::EffectKind::Limiter);
                project.master.effects.push(fx);
            }
            let mut app = CantusApp::from_parts(Some(project), EngineHandle::dummy(), false);
            app.settings_path = scratch.join("settings.json");
            if kind == "selection" {
                app.set_time_selection(Ticks::beats(4), Ticks::beats(20));
            }
            export::open(&mut app, None);
            set_up(&mut app, kind, &scratch);
            let theme_name = if dark { "dark" } else { "light" };
            let path = dir.join(format!("export-{kind}-{theme_name}-{code}.png"));
            let frames = if kind == "list" { 12 } else { 6 };
            shoot_events(
                &ctx,
                egui::vec2(1440.0, 900.0),
                scale,
                frames,
                Some(&path),
                |frame, ctx| {
                    if kind == "list" {
                        click_on(ctx, "export_audio.row.rate", frame)
                    } else {
                        Vec::new()
                    }
                },
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

/// The dialog's choices for `kind`.
fn set_up(app: &mut CantusApp, kind: &str, scratch: &std::path::Path) {
    {
        let d = app.export.dialog.as_mut().unwrap();
        d.tags.artist = "Luis Ángel Almazán".into();
        d.tags.year = "2026".into();
        *d = match kind {
            "streaming" | "running" => Preset::Streaming.apply(d),
            "stems" => Preset::MasteringStems.apply(d),
            "podcast" => Preset::Podcast.apply(d),
            "flac" => {
                let mut c = d.clone();
                c.format = Format::Flac;
                c.depth = Depth::Int16;
                c.dither = Dither::Shaped;
                c.tags.album = "Salmos".into();
                c.tags.comment = "Colosenses 3:23".into();
                c
            }
            "list" => {
                let mut c = d.clone();
                c.format = Format::Wav;
                c.rate = Rate::Session;
                c
            }
            "selection" => {
                let mut c = d.clone();
                c.scope = Scope::Selection;
                c
            }
            "loop" => {
                let mut c = d.clone();
                c.scope = Scope::Loop;
                c
            }
            "stems-tracks" => {
                let mut c = Preset::MasteringStems.apply(d);
                c.stems.tracks = true;
                c.stems.master = true;
                c.stems.through_master = true;
                c
            }
            _ => d.clone(),
        };
    }
    match kind {
        "running" => {
            app.export.dialog.as_mut().unwrap().tail_seconds = 30.0;
            export::start(app, &scratch.join("Welcome.mp3"));
        }
        "no-range" => {
            app.time_selection = None;
            app.export.asked = Some(Scope::Selection);
        }
        "done" => {
            app.export.dialog.as_mut().unwrap().tail_seconds = 0.5;
            export::start(app, &scratch.join("Welcome.mp3"));
            export::wait(app);
        }
        _ => {}
    }
}

/// A click on the control the dialog published as `name`: moved over it, pressed and
/// released on the frames after it was first drawn.
fn click_on(ctx: &egui::Context, name: &str, frame: usize) -> Vec<egui::Event> {
    let Some(at) = export_dialog::geo(ctx, name).map(|r| r.center()) else {
        return Vec::new();
    };
    let button = |pressed| egui::Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    };
    match frame {
        3 => vec![egui::Event::PointerMoved(at)],
        4 => vec![button(true)],
        5 => vec![button(false)],
        // Then away from the field, so the list is drawn without the field's hover.
        6 => vec![egui::Event::PointerMoved(egui::pos2(4.0, 4.0))],
        _ => Vec::new(),
    }
}

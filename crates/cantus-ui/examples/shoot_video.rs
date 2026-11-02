//! Screenshots of the video export dialog (ADR 0088) without a screen:
//! `cargo run -p cantus-ui --example shoot_video -- <out-dir> [scale] [locale]`.
//! The arrangement with the dialog over it, in the light and the dark theme: vertical MP4
//! for social media, a square WebM, a transparent MOV layer, wide PNG frames on a green
//! key, the preview scrubbed into the song, and an export under way.

use cantus_engine::EngineHandle;
use cantus_ui::raster::shoot_events;
use cantus_ui::video_export::{self as vx, Back, Format, Shape};
use cantus_ui::views;
use cantus_ui::{welcome_project, CantusApp};
use std::path::PathBuf;

const KINDS: [&str; 6] = ["mp4", "webm", "mov", "png", "scrubbed", "running"];

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
    let scratch = std::env::temp_dir().join("cantus-shoot-video");
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
            let mut app =
                CantusApp::from_parts(Some(welcome_project()), EngineHandle::dummy(), false);
            app.settings_path = scratch.join("settings.json");
            // An interface in the top bar, as on a working machine (not "No audio").
            app.audio_setup.demo = Some(cantus_ui::audio_setup::demo::live_on_umc());
            vx::open(&mut app);
            {
                let d = app.video.dialog.as_mut().unwrap();
                match kind {
                    "webm" => {
                        d.format = Format::Webm;
                        d.shape = Shape::Square;
                    }
                    "mov" => {
                        d.format = Format::Mov;
                        d.background = Back::Transparent;
                    }
                    "png" => {
                        d.format = Format::Png;
                        d.shape = Shape::Landscape;
                        d.background = Back::Green;
                    }
                    _ => {}
                }
            }
            if kind == "scrubbed" {
                app.video.scrub = 6.3;
            }
            // The preview as the file draws it: wait for its own render of the moment shown
            // (the picture dances to the beat under a "rendering…" chip until it is in).
            if kind != "running" {
                let d = app.video.dialog.clone().unwrap();
                let t0 = std::time::Instant::now();
                while vx::preview_timelines(&mut app, &d) == vx::PreviewState::Rendering
                    && t0.elapsed() < std::time::Duration::from_secs(120)
                {
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
            }
            if kind == "running" {
                let d = app.video.dialog.clone().unwrap();
                let mut d2 = d.clone();
                d2.format = Format::Mov;
                let _ = std::fs::remove_file(scratch.join("running.mov"));
                let _ = vx::start(&mut app, &scratch.join("running.mov"), &d2, true);
                std::thread::sleep(std::time::Duration::from_millis(900));
            }
            let theme_name = if dark { "dark" } else { "light" };
            let path = dir.join(format!("video-{kind}-{theme_name}-{code}.png"));
            shoot_events(
                &ctx,
                egui::vec2(1440.0, 900.0),
                scale,
                6,
                Some(&path),
                |_, _| Vec::new(),
                |root| {
                    let ctx = root.ctx().clone();
                    egui::Panel::top(egui::Id::new("t"))
                        .show(root, |ui| views::transport::show(&mut app, ui));
                    egui::CentralPanel::default()
                        .show(root, |ui| views::arrangement::show(&mut app, ui));
                    vx::frame(&mut app, &ctx);
                },
            );
            if app.video.busy() {
                vx::stop(&mut app);
                vx::wait(&mut app);
            }
            println!("{}", path.display());
        }
    }
}

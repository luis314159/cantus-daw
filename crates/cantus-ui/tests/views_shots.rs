//! The design review's pictures of the editing views, as an ignored test so they come out
//! of the same build as the tests: `CANTUS_SHOTS=<dir> cargo test -p cantus-ui --test
//! views_shots -- --ignored`. `CANTUS_SHOTS_FILTER` keeps the shots whose name contains
//! every comma-separated word; `CANTUS_SHOTS_SCALE` sets the pixels per point (0.75, 2…);
//! `CANTUS_SHOTS_LANG=es` paints them in Spanish. The example `shoot_views` draws the same.

mod support;

use cantus_engine::EngineHandle;
use cantus_ui::raster::shoot;
use cantus_ui::{stress_project, views, welcome_project, CantusApp};
use std::path::PathBuf;

#[derive(Copy, Clone)]
enum View {
    Arrangement,
    PianoRoll,
    Rack,
}

#[test]
#[ignore = "writes pictures for the design review; run with --ignored"]
fn shoot_the_editing_views() {
    let Ok(dir) = std::env::var("CANTUS_SHOTS") else {
        return;
    };
    let dir = PathBuf::from(dir);
    std::fs::create_dir_all(&dir).expect("output dir");
    let filters: Vec<String> = std::env::var("CANTUS_SHOTS_FILTER")
        .unwrap_or_default()
        .split(',')
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect();
    let scale: f32 = std::env::var("CANTUS_SHOTS_SCALE")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1.0);
    let spanish = std::env::var("CANTUS_SHOTS_LANG").is_ok_and(|l| l == "es");
    cantus_ui::i18n::set_locale(if spanish {
        cantus_ui::i18n::Locale::Es
    } else {
        cantus_ui::i18n::Locale::En
    });
    cantus_ui::i18n::default_thread_locale(if spanish {
        cantus_ui::i18n::Locale::Es
    } else {
        cantus_ui::i18n::Locale::En
    });
    let themes = [
        ("slate", cantus_ui::theme::slate()),
        ("paper", cantus_ui::theme::paper()),
    ];
    for (tn, theme) in &themes {
        for (w, h) in [(1280.0f32, 720.0f32), (1920.0, 1080.0)] {
            for stress in [false, true] {
                for (vn, view) in [
                    ("arrangement", View::Arrangement),
                    ("pianoroll", View::PianoRoll),
                    ("rack", View::Rack),
                ] {
                    let name = format!(
                        "{vn}-{tn}-{}-{}x{}{}{}",
                        if stress { "stress" } else { "welcome" },
                        w as u32,
                        h as u32,
                        if spanish { "-es" } else { "" },
                        if scale != 1.0 {
                            format!("-x{scale}")
                        } else {
                            String::new()
                        },
                    );
                    if !filters.iter().all(|f| name.contains(f.as_str())) {
                        continue;
                    }
                    let ctx = egui::Context::default();
                    cantus_ui::design::install_fonts(&ctx);
                    cantus_ui::design::install_style(&ctx);
                    cantus_ui::theme::apply(theme, &ctx);
                    let project = if stress {
                        stress_project()
                    } else {
                        welcome_project()
                    };
                    let mut app =
                        CantusApp::from_parts(Some(project), EngineHandle::dummy(), false);
                    if let Some(t) = app.project().tracks.first() {
                        if let Some(c) = t.clips.first() {
                            let (tid, cid) = (t.id, c.id);
                            app.select_clip(tid, cid);
                        }
                    }
                    if !stress {
                        app.set_time_selection(
                            cantus_core::Ticks::beats(8),
                            cantus_core::Ticks::beats(12),
                        );
                    }
                    // Shots are in points; a scale draws the same layout at more pixels.
                    let path = dir.join(format!("{name}.png"));
                    shoot(
                        &ctx,
                        egui::vec2(w / scale, h / scale),
                        scale,
                        4,
                        Some(&path),
                        |root| {
                            egui::CentralPanel::default().frame(egui::Frame::NONE).show(
                                root,
                                |ui| match view {
                                    View::Arrangement => views::arrangement::show(&mut app, ui),
                                    View::PianoRoll => views::piano_roll::show(&mut app, ui),
                                    View::Rack => views::channel_rack::show(&mut app, ui),
                                },
                            );
                        },
                    );
                    eprintln!("wrote {}", path.display());
                }
            }
        }
    }
}

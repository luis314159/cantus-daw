//! Screenshots of the editing views without a screen: the arrangement, the piano roll and
//! the channel rack, in Slate and Paper, at 1280×720 and 1920×1080, on the welcome project
//! and the stress project — the design review of ADR 0024 Phase B for the views.
//!
//! `cargo run -p cantus-ui --example shoot_views -- <out-dir> [filter…]`
//!
//! A filter keeps the shots whose name contains it (`arrangement`, `paper`, `1920`,
//! `stress`…). Each view fills the canvas, as it does in a dock tab.

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

impl View {
    fn name(self) -> &'static str {
        match self {
            View::Arrangement => "arrangement",
            View::PianoRoll => "pianoroll",
            View::Rack => "rack",
        }
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let dir = PathBuf::from(args.next().unwrap_or_else(|| "shots".into()));
    let filters: Vec<String> = args.collect();
    std::fs::create_dir_all(&dir).expect("output dir");
    let spanish = filters.iter().any(|f| f == "es");
    cantus_ui::i18n::set_locale(if spanish {
        cantus_ui::i18n::Locale::Es
    } else {
        cantus_ui::i18n::Locale::En
    });
    let filters: Vec<String> = filters.into_iter().filter(|f| f != "es").collect();
    let scale: f32 = filters
        .iter()
        .find_map(|f| f.strip_prefix("x").and_then(|s| s.parse().ok()))
        .unwrap_or(1.0);
    let filters: Vec<String> = filters
        .into_iter()
        .filter(|f| !(f.starts_with('x') && f[1..].parse::<f32>().is_ok()))
        .collect();

    let themes = [
        ("night", cantus_ui::theme::default_theme()),
        ("slate", cantus_ui::theme::slate()),
        ("paper", cantus_ui::theme::paper()),
    ];
    let sizes = [(1280.0f32, 720.0f32), (1920.0, 1080.0)];
    let views_ = [View::Arrangement, View::PianoRoll, View::Rack];
    for (tn, theme) in &themes {
        for (w, h) in sizes {
            for stress in [false, true] {
                for view in views_ {
                    let name = format!(
                        "{}-{}-{}-{}x{}{}",
                        view.name(),
                        tn,
                        if stress { "stress" } else { "welcome" },
                        w as u32,
                        h as u32,
                        if spanish { "-es" } else { "" },
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
                        let (tid, cid) = (t.id, t.clips.first().map(|c| c.id));
                        if let Some(cid) = cid {
                            app.select_clip(tid, cid);
                        }
                    }
                    if !stress {
                        // A time selection and a selected note, so the review sees them.
                        app.set_time_selection(
                            cantus_core::Ticks::beats(8),
                            cantus_core::Ticks::beats(12),
                        );
                    }
                    let path = dir.join(format!("{name}.png"));
                    shoot(&ctx, egui::vec2(w, h), scale, 4, Some(&path), |root| {
                        egui::CentralPanel::default()
                            .frame(egui::Frame::NONE)
                            .show(root, |ui| match view {
                                View::Arrangement => views::arrangement::show(&mut app, ui),
                                View::PianoRoll => views::piano_roll::show(&mut app, ui),
                                View::Rack => views::channel_rack::show(&mut app, ui),
                            });
                    });
                    println!("wrote {}", path.display());
                }
            }
        }
    }
}

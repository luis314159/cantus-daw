//! Names in other scripts, with the system font fallback armed as in the real window
//! (round 26, `font_fallback.rs`): tracks in Japanese, Arabic and Hebrew, a clip in Korean,
//! a strip in Hindi, a track in Thai; the whole window (arrangement and mixer) at 100 % and
//! 200 %.
//!
//! `cargo run -p cantus-ui --example shoot_scripts [-- out_dir]`
//!
//! Writes `scripts-<view>-<zoom>.png` into `out_dir` (default `target/shoot-scripts`).

use cantus_core::Command;
use cantus_engine::EngineHandle;
use cantus_ui::app::{CantusApp, CentralView};
use cantus_ui::{font_fallback, raster, welcome_project};
use std::path::PathBuf;

const SIZE: egui::Vec2 = egui::vec2(1400.0, 860.0);

fn names(app: &mut CantusApp) {
    let tracks: Vec<_> = app.project().tracks.iter().map(|t| t.id).collect();
    let track_names = ["ドラム・キット", "طبول", "תופים", "กลอง"];
    for (id, name) in tracks.iter().zip(track_names) {
        app.exec(Command::SetTrackName {
            id: *id,
            name: name.into(),
        });
    }
    let t0 = app.project().tracks[0].id;
    if let Some(clip) = app.project().tracks[0].clips.first().map(|c| c.id) {
        app.exec(Command::SetClipName {
            track: t0,
            clip,
            name: "드럼 패턴".into(),
        });
    }
    if let Some(strip) = app.project().tracks[0].strip {
        app.exec(Command::SetStripName {
            id: strip,
            name: "मुख्य ड्रम".into(),
        });
    }
}

fn main() {
    let dir = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/shoot-scripts")
        });
    std::fs::create_dir_all(&dir).expect("create the output directory");
    font_fallback::arm();
    // The whole window: the arrangement above, the mixer below.
    for (view, central) in [("window", CentralView::Mixer)] {
        for zoom in [1.0f32, 2.0] {
            let ctx = egui::Context::default();
            cantus_ui::design::install_fonts(&ctx);
            cantus_ui::design::install_style(&ctx);
            let tmp = std::env::temp_dir().join("cantus-shoot-scripts");
            let mut app =
                CantusApp::from_parts(Some(welcome_project()), EngineHandle::dummy(), false);
            app.settings_path = tmp.join("settings.json");
            app.layouts_path = tmp.join("layouts.json");
            app.keymap_path = tmp.join("keymap.toml");
            app.appearance_path = tmp.join("appearance.json");
            app.attach(&ctx);
            names(&mut app);
            app.show_central(central);
            let path = dir.join(format!("scripts-{view}-{}.png", (zoom * 100.0) as u32));
            // Enough frames for the system faces to be read (on their own thread) and
            // installed; a short pause each frame lets the reader finish.
            raster::shoot(&ctx, SIZE, zoom, 24, Some(&path), |root| {
                std::thread::sleep(std::time::Duration::from_millis(15));
                app.frame(root)
            });
            println!("{}", path.display());
        }
    }
}

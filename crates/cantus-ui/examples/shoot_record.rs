//! The recording mode on screen without a screen (round 36, ADR 0092).
//!
//! ```text
//! cargo run -p cantus-ui --example shoot_record -- [out-dir]
//! ```
//!
//! The whole window with the mode's bar (on, and preparing), the question before it starts
//! and the one when leaving — in Cantus Night and Paper, English and Spanish, at 100 % and
//! 200 %, and a narrow window.

use cantus_core::*;
use cantus_engine::EngineHandle;
use cantus_ui::app::CantusApp;
use cantus_ui::i18n::{self, Locale};
use cantus_ui::kit::appearance::Mode;
use cantus_ui::raster;
use std::path::PathBuf;

/// Four tracks, the voice armed: three strips baked, the voice's live.
fn song() -> Project {
    let mut p = cantus_ui::starter_project();
    p.tracks.clear();
    for name in ["Drums", "Bass", "Keys", "Voice"] {
        let mut t = p.make_track(name);
        let mut c = p.make_clip(name, Ticks::ZERO, Ticks::beats(16));
        for k in 0..16 {
            c.notes
                .push(p.make_note(Ticks::beats(k), Ticks::QUARTER, 48 + (k % 12) as u8, 96));
        }
        t.clips.push(c);
        p.tracks.push(t);
    }
    p.tracks[3].armed = true;
    p
}

#[derive(Clone, Copy)]
enum Show {
    On,
    Preparing,
    Ask,
    Leave,
}

fn shoot(
    dir: &std::path::Path,
    dark: bool,
    lang: Locale,
    zoom: f32,
    size: (f32, f32),
    show: Show,
    name: &str,
) {
    i18n::set_locale(lang);
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    let tmp = std::env::temp_dir().join("cantus-shoot-record");
    let mut app = CantusApp::from_parts(Some(song()), EngineHandle::dummy(), false);
    app.settings_path = tmp.join("settings.json");
    app.layouts_path = tmp.join("layouts.json");
    app.keymap_path = tmp.join("keymap.toml");
    app.appearance_path = tmp.join("appearance.json");
    app.appearance.mode = if dark { Mode::Dark } else { Mode::Light };
    let mode = app.appearance.mode;
    let preset = if dark { "cantus_night" } else { "cantus_paper" };
    app.appearance.assign(mode, format!("preset:{preset}"));
    app.attach(&ctx);
    // Renders (a frozen target with none is made in the background) never in the user's cache.
    app.engine.frozen.set_app_dir(tmp.join("renders"));
    match show {
        Show::On => cantus_ui::record_mode::stage_for_shot(&mut app, false),
        Show::Preparing => cantus_ui::record_mode::stage_for_shot(&mut app, true),
        Show::Ask => app.record_mode_ask(),
        Show::Leave => {
            cantus_ui::record_mode::stage_for_shot(&mut app, false);
            app.freeze.record.leaving = true;
        }
    }
    let (w, h) = size;
    let points = egui::vec2(w / zoom, h / zoom);
    let theme = if dark { "night" } else { "paper" };
    let path = dir.join(format!(
        "record-{name}-{theme}-{}-k{}.png",
        lang.code(),
        (zoom * 100.0) as u32
    ));
    raster::shoot(&ctx, points, zoom, 8, Some(&path), |root| app.frame(root));
    println!("{}", path.display());
}

fn main() {
    std::thread::Builder::new()
        .stack_size(64 << 20)
        .spawn(run)
        .unwrap()
        .join()
        .unwrap();
}

fn run() {
    let dir = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/shots/record"));
    std::fs::create_dir_all(&dir).expect("output dir");
    for dark in [true, false] {
        for lang in [Locale::En, Locale::Es] {
            for zoom in [1.0f32, 2.0] {
                for (show, name) in [
                    (Show::On, "on"),
                    (Show::Preparing, "preparing"),
                    (Show::Ask, "ask"),
                    (Show::Leave, "leave"),
                ] {
                    shoot(&dir, dark, lang, zoom, (1440.0, 860.0), show, name);
                }
            }
        }
        shoot(
            &dir,
            dark,
            Locale::Es,
            1.0,
            (900.0, 640.0),
            Show::On,
            "narrow",
        );
    }
}

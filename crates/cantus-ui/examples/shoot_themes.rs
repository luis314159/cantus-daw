//! Every shipped theme on the same view, as the owner judges a look (round 39, *themes*):
//! the whole window at 1920 × 1080 at 125 %, the song the program opens on, the Chords track
//! chosen with a clip selected, playing in a loop of bars 5–9 — the arrangement with the
//! mixer under it; and Settings ▸ Appearance, at its top and scrolled to its foot.
//!
//! `cargo run --release -p cantus-ui --example shoot_themes -- <out-dir> [id…] [--look]`
//! writes `<out-dir>/full/<id>.png` for each theme (all, or the ids named) and, with
//! `--look`, `<out-dir>/look-<id>-top.png` and `-foot.png` for Slate Night, Slate Paper and
//! Print. `tools/themes/contact_sheet.py` makes the contact sheet from the full shots.

use cantus_engine::EngineHandle;
use cantus_ui::app::CantusApp;
use cantus_ui::kit::appearance::{Mode, PRESETS};
use cantus_ui::raster;
use cantus_ui::views::workspace_settings::{open, SettingsPage};
use egui::vec2;
use std::path::{Path, PathBuf};

const SIZE: (f32, f32) = (1920.0, 1080.0);
const ZOOM: f32 = 1.25;

fn main() {
    let mut args = std::env::args().skip(1);
    let dir = PathBuf::from(args.next().unwrap_or_else(|| "shots".into()));
    let rest: Vec<String> = args.collect();
    let look = rest.iter().any(|a| a == "--look");
    let only: Vec<&String> = rest.iter().filter(|a| !a.starts_with("--")).collect();
    std::fs::create_dir_all(dir.join("full")).expect("output dir");
    if look {
        for id in ["slate_night", "slate_paper", "print"] {
            for foot in [false, true] {
                let name = format!("look-{id}-{}.png", if foot { "foot" } else { "top" });
                let path = dir.join(name);
                window(&path, id, true, foot);
                println!("{}", path.display());
            }
        }
        if only.is_empty() {
            return;
        }
    }
    for (id, _) in PRESETS {
        if !only.is_empty() && !only.iter().any(|o| *o == id) {
            continue;
        }
        let path = dir.join("full").join(format!("{id}.png"));
        window(&path, id, false, false);
        println!("{}", path.display());
    }
}

/// One whole window in theme `id`: the arrangement, or Settings ▸ Appearance (`look`), at
/// its top or wheeled down to the groups below.
fn window(path: &Path, id: &str, look: bool, foot: bool) {
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    let (engine, mut processor) = EngineHandle::manual(48_000.0);
    let project = cantus_ui::demos::startup_project();
    let mut app = CantusApp::from_parts(Some(project), engine, false);
    let tmp = std::env::temp_dir().join("cantus-shoot-themes");
    app.settings_path = tmp.join("settings.json");
    app.layouts_path = tmp.join("layouts.json");
    app.keymap_path = tmp.join("keymap.toml");
    app.appearance_path = tmp.join("appearance.json");
    let dark = PRESETS
        .iter()
        .find(|(k, _)| *k == id)
        .is_none_or(|(_, make)| make().dark);
    app.appearance.mode = if dark { Mode::Dark } else { Mode::Light };
    let mode = app.appearance.mode;
    app.appearance.assign(mode, format!("preset:{id}"));
    app.attach(&ctx);
    // The Chords track chosen and its first clip selected.
    let chords = app.project().tracks.iter().find_map(|t| {
        if !t.name.to_lowercase().contains("chord") {
            return None;
        }
        t.clips
            .iter()
            .find(|c| !c.notes.is_empty())
            .map(|c| (t.id, c.id))
    });
    if let Some((t, c)) = chords {
        app.select_clip(t, c);
    }
    if let Some(strip) = app.project().tracks.get(1).and_then(|t| t.strip) {
        app.select_strip(strip);
    }
    let bar = cantus_core::PPQ * 4;
    app.loop_span(cantus_core::Ticks(bar * 4), cantus_core::Ticks(bar * 8));
    app.engine
        .seek(cantus_core::Ticks(bar * 5 + cantus_core::PPQ * 2));
    if look {
        open(&mut app, SettingsPage::Appearance);
    }
    app.sync_engine();
    app.toggle_play();
    let mut audio = vec![0f32; 800 * 2];
    let points = vec2(SIZE.0 / ZOOM, SIZE.1 / ZOOM);
    // The page's middle, under the pointer, for the wheel.
    let body = egui::pos2(points.x * 0.5 + 100.0, points.y * 0.5);
    raster::shoot_events(
        &ctx,
        points,
        ZOOM,
        30,
        Some(path),
        |frame, _| {
            if !foot || frame != 4 {
                return Vec::new();
            }
            vec![
                egui::Event::PointerMoved(body),
                egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: vec2(0.0, -330.0),
                    phase: egui::TouchPhase::Move,
                    modifiers: egui::Modifiers::NONE,
                },
            ]
        },
        |root| {
            for _ in 0..3 {
                processor.process(&mut audio, 2);
            }
            app.frame(root);
        },
    );
}

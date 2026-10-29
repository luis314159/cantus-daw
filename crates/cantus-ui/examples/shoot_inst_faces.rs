//! Round 32, the instruments' faces (ADR 0074): every instrument's editor and Pulsus,
//! maximised, in Slate and Paper, at 100 % and 200 %, in its Play and Edit views — the
//! instruments builder's look at its own work.
//!
//! `cargo run -p cantus-ui --example shoot_inst_faces [-- out_dir [unit…] [--zoom=1] [--view=play] [--theme=slate] [--plain]]`
//!
//! Writes `<unit>-<view>-<theme>-<zoom>.png` into `out_dir` (default
//! `target/shoot-inst-faces`).

use cantus_engine::EngineHandle;
use cantus_ui::kit::appearance::Mode;
use cantus_ui::views::workspace_device as device;
use cantus_ui::{raster, welcome_project, CantusApp};
use std::path::{Path, PathBuf};

const UNITS: [&str; 9] = [
    "unda", "calor", "modulus", "exemplar", "gravis", "lamina", "organum", "chorda", "pulsus",
];

const SIZE: egui::Vec2 = egui::vec2(1500.0, 940.0);

fn shoot(dir: &Path, unit: &str, edit: bool, dark: bool, zoom: f32) {
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    let tmp = std::env::temp_dir().join("cantus-shoot-inst-faces");
    let mut app = CantusApp::from_parts(Some(welcome_project()), EngineHandle::dummy(), false);
    app.settings_path = tmp.join("settings.json");
    app.layouts_path = tmp.join("layouts.json");
    app.keymap_path = tmp.join("keymap.toml");
    app.appearance_path = tmp.join("appearance.json");
    let mode = if dark { Mode::Dark } else { Mode::Light };
    app.appearance.mode = mode;
    app.appearance.assign(
        mode,
        if dark { "preset:slate" } else { "preset:paper" }.into(),
    );
    app.attach(&ctx);
    let name = match (unit, edit) {
        ("pulsus", true) => "pulsus-edit",
        ("pulsus", false) => "pulsus-play",
        _ => unit,
    };
    app.demo_editor(name);
    if let Some(k) = cantus_ui::views::inst::kind_from_name(unit) {
        cantus_ui::views::inst::demo_feed(&mut app, k);
        app.inst.view = if edit {
            cantus_ui::views::inst::View::Edit
        } else {
            cantus_ui::views::inst::View::Play
        };
    }
    let track = app.selected_track.unwrap_or(app.project().tracks[0].id);
    let key = app.instrument_window_key(track);
    let theme = if dark { "slate" } else { "paper" };
    let view = if edit { "edit" } else { "play" };
    let path = dir.join(format!(
        "{unit}-{view}-{theme}-{}.png",
        (zoom * 100.0).round() as u32
    ));
    let mut frame = 0;
    raster::shoot_at(
        &ctx,
        SIZE,
        zoom,
        14,
        Some(&path),
        |_| None,
        |root| {
            if frame == 6 {
                if let Some(key) = key {
                    device::toggle_maximise(root.ctx(), key);
                }
            }
            app.frame(root);
            frame += 1;
        },
    );
    println!("{}", path.display());
}

fn main() {
    let mut args = std::env::args().skip(1);
    let dir = args.next().map(PathBuf::from).unwrap_or_else(|| {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/shoot-inst-faces")
    });
    let rest: Vec<String> = args.collect();
    let zooms: Vec<f32> = rest
        .iter()
        .find_map(|a| a.strip_prefix("--zoom="))
        .map(|z| z.split(',').filter_map(|v| v.parse().ok()).collect())
        .unwrap_or_else(|| vec![1.0, 2.0]);
    let views: Vec<bool> = match rest.iter().find_map(|a| a.strip_prefix("--view=")) {
        Some("play") => vec![false],
        Some("edit") => vec![true],
        _ => vec![false, true],
    };
    let themes: Vec<bool> = match rest.iter().find_map(|a| a.strip_prefix("--theme=")) {
        Some("slate") => vec![true],
        Some("paper") => vec![false],
        _ => vec![true, false],
    };
    // `--plain`: the kit's one look, for comparison (the user's switch off).
    if rest.iter().any(|a| a == "--plain") {
        cantus_ui::kit::skin::set_enabled(false);
    }
    let only: Vec<&String> = rest.iter().filter(|a| !a.starts_with("--")).collect();
    std::fs::create_dir_all(&dir).expect("create the output directory");
    for unit in UNITS {
        if !only.is_empty() && !only.iter().any(|o| o.as_str() == unit) {
            continue;
        }
        for &edit in &views {
            for &dark in &themes {
                for &zoom in &zooms {
                    shoot(&dir, unit, edit, dark, zoom);
                }
            }
        }
    }
}

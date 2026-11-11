//! Slate Lapis (round 39, `lapis2`) as the owner judges it: the whole window at 1920 × 1080
//! at 125 %, on the song the program opens on, the Chords track chosen and playing in a
//! loop of bars 5–9 — the arrangement (a clip selected), the mixer maximised in Wide, the
//! piano roll (a few notes selected) and Settings ▸ Appearance — in Slate Lapis Night and
//! Paper and in Slate Night and Paper; and per view one picture side by side, Slate at the
//! left and Slate Lapis at the right, Night above and Paper below.
//!
//! `cargo run -p cantus-ui --example shoot_lapis -- <out-dir> [case…]` writes
//! `<case>-<theme>.png` and `sbs-<case>.png`. `--slate` shoots Slate's only (for the
//! pixel-for-pixel check that Slate did not move).

use cantus_engine::EngineHandle;
use cantus_ui::app::{CantusApp, CentralView};
use cantus_ui::kit::appearance::Mode;
use cantus_ui::raster;
use cantus_ui::views::mixer::layout::WidthMode;
use cantus_ui::views::workspace::Pane;
use cantus_ui::views::workspace_settings::{open, SettingsPage};
use egui::vec2;
use std::path::{Path, PathBuf};

const CASES: [&str; 4] = ["arrangement", "mixer", "pianoroll", "look"];
const THEMES: [(&str, bool); 4] = [
    ("slate_night", true),
    ("slate_paper", false),
    ("slate_lapis_night", true),
    ("slate_lapis_paper", false),
];
const SIZE: (f32, f32) = (1920.0, 1080.0);
const ZOOM: f32 = 1.25;

fn main() {
    let mut args = std::env::args().skip(1);
    let dir = PathBuf::from(args.next().unwrap_or_else(|| "shots".into()));
    let rest: Vec<String> = args.collect();
    let slate_only = rest.iter().any(|a| a == "--slate");
    let only: Vec<&String> = rest.iter().filter(|a| !a.starts_with("--")).collect();
    std::fs::create_dir_all(&dir).expect("output dir");
    for case in CASES {
        if !only.is_empty() && !only.iter().any(|o| *o == case) {
            continue;
        }
        for (theme, dark) in THEMES {
            if slate_only && theme.contains("lapis") {
                continue;
            }
            let path = dir.join(format!("{case}-{theme}.png"));
            window(&path, case, theme, dark);
            println!("{}", path.display());
        }
        if !slate_only {
            let path = dir.join(format!("sbs-{case}.png"));
            side_by_side(&dir, case, &path);
            println!("{}", path.display());
        }
    }
}

fn window(path: &Path, case: &str, theme: &str, dark: bool) {
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    let (engine, mut processor) = EngineHandle::manual(48_000.0);
    let project = cantus_ui::demos::startup_project();
    let mut app = CantusApp::from_parts(Some(project), engine, false);
    let tmp = std::env::temp_dir().join("cantus-shoot-lapis");
    app.settings_path = tmp.join("settings.json");
    app.layouts_path = tmp.join("layouts.json");
    app.keymap_path = tmp.join("keymap.toml");
    app.appearance_path = tmp.join("appearance.json");
    app.appearance.mode = if dark { Mode::Dark } else { Mode::Light };
    // Both lights of the family, as a click on its picture in Settings sets them.
    let family = theme.trim_end_matches("_night").trim_end_matches("_paper");
    app.appearance
        .assign(Mode::Dark, format!("preset:{family}_night"));
    app.appearance
        .assign(Mode::Light, format!("preset:{family}_paper"));
    app.attach(&ctx);
    // The Chords track chosen, its first clip selected, and in the piano roll a few of
    // its notes.
    let chords = app.project().tracks.iter().find_map(|t| {
        if !t.name.to_lowercase().contains("chord") {
            return None;
        }
        t.clips.iter().find(|c| !c.notes.is_empty()).map(|c| {
            let notes: Vec<_> = c.notes.iter().take(6).map(|n| n.id).collect();
            (t.id, c.id, notes)
        })
    });
    if let Some((t, c, notes)) = chords {
        app.select_clip(t, c);
        app.selected_notes = notes;
    }
    let bar = cantus_core::PPQ * 4;
    app.loop_span(cantus_core::Ticks(bar * 4), cantus_core::Ticks(bar * 8));
    app.engine
        .seek(cantus_core::Ticks(bar * 5 + cantus_core::PPQ * 2));
    let mut maximise = false;
    match case {
        "mixer" => {
            app.show_central(CentralView::Mixer);
            cantus_ui::views::mixer::set_width_mode(&mut app, WidthMode::Wide);
            maximise = true;
        }
        "pianoroll" => {
            if let (Some(t), Some(c)) = (app.selected_track, app.selected_clip) {
                app.piano_roll.key_height = 12.0;
                cantus_ui::views::piano_roll::fit_clip(&mut app, t, c);
            }
            app.show_central(CentralView::PianoRoll);
        }
        "look" => open(&mut app, SettingsPage::Appearance),
        _ => {}
    }
    app.sync_engine();
    app.toggle_play();
    let mut audio = vec![0f32; 800 * 2];
    let points = vec2(SIZE.0 / ZOOM, SIZE.1 / ZOOM);
    let mut frames = 0;
    raster::shoot(&ctx, points, ZOOM, 30, Some(path), |root| {
        frames += 1;
        // The mixer takes the window under the top bar, the inspector beside it.
        if frames == 2 && maximise {
            app.workspace.toggle_fold(Pane::Arrangement);
        }
        for _ in 0..3 {
            processor.process(&mut audio, 2);
        }
        app.frame(root);
    });
}

/// Slate | Slate Lapis, Night above and Paper below, at full size.
fn side_by_side(dir: &Path, case: &str, out: &Path) {
    let load = |theme: &str| {
        let bytes = std::fs::read(dir.join(format!("{case}-{theme}.png"))).expect("a shot");
        tiny_skia::Pixmap::decode_png(&bytes).expect("decode")
    };
    let grid = [
        [load("slate_night"), load("slate_lapis_night")],
        [load("slate_paper"), load("slate_lapis_paper")],
    ];
    let (w, h) = (grid[0][0].width(), grid[0][0].height());
    let gap = 8;
    let mut sheet = tiny_skia::Pixmap::new(2 * w + gap, 2 * h + gap).expect("sheet");
    sheet.fill(tiny_skia::Color::from_rgba8(128, 128, 128, 255));
    for (row, pair) in grid.iter().enumerate() {
        for (col, shot) in pair.iter().enumerate() {
            sheet.draw_pixmap(
                (col as u32 * (w + gap)) as i32,
                (row as u32 * (h + gap)) as i32,
                shot.as_ref(),
                &tiny_skia::PixmapPaint::default(),
                tiny_skia::Transform::identity(),
                None,
            );
        }
    }
    sheet.save_png(out).expect("write the sheet");
}

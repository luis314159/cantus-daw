//! Round 42, `violet-paper`: Violet Paper as the owner judges a theme — the whole window at
//! 1920 × 1080 at 125 %, on the song the program opens on (the Chords track chosen, a few of
//! its notes selected, playing in a loop of bars 5–9) — beside the Paper themes nearest it.
//!
//! `cargo run -p cantus-ui --example shoot_violet -- <out-dir> [case…] [--themes a,b,…]
//! [--sbs]` writes `<case>-<theme>.png`; `--sbs` also writes `sbs-<case>.png`, the first two
//! themes side by side (the default pair: Slate Lapis Paper at the left, Violet Paper at the
//! right).
//!
//! Cases: `arrangement`, `mixer` (Wide, two strips muted, one soloed), `pianoroll`, `rack`
//! (the channel rack), `browser` (the sample browser at the left), `chorda` and `arcus`
//! (the instrument's window and its scene), `unda` (a unit skin), `look` (Settings ▸
//! Appearance), `dialog` (the unsaved-changes question over the window), `menu` (the bar's
//! Edit menu open).
//!
//! A theme named `<file>.json` is a theme file worn as the user's (a variant being judged,
//! `tools/themes/variant.py`); `--map` writes `tokens-map.json`, the kit's tokens by field
//! and key, which that tool reads.

use cantus_engine::EngineHandle;
use cantus_ui::app::{CantusApp, CentralView};
use cantus_ui::kit::appearance::Mode;
use cantus_ui::raster;
use cantus_ui::views::mixer::layout::WidthMode;
use cantus_ui::views::workspace::Pane;
use cantus_ui::views::workspace_settings::{open, SettingsPage};
use egui::vec2;
use std::path::{Path, PathBuf};

const CASES: [&str; 11] = [
    "arrangement",
    "mixer",
    "pianoroll",
    "rack",
    "browser",
    "chorda",
    "arcus",
    "unda",
    "look",
    "dialog",
    "menu",
];
const THEMES: [&str; 3] = ["slate_lapis_paper", "violet_paper", "slate_paper"];
const SIZE: (f32, f32) = (1920.0, 1080.0);
const ZOOM: f32 = 1.25;

fn main() {
    let mut args = std::env::args().skip(1);
    let dir = PathBuf::from(args.next().unwrap_or_else(|| "shots".into()));
    let rest: Vec<String> = args.collect();
    std::fs::create_dir_all(&dir).expect("output dir");
    if rest.iter().any(|a| a == "--map") {
        // The kit's tokens by field and by key, for `tools/themes/variant.py`.
        let map: Vec<(&str, &str)> = cantus_ui::kit::tokens::Tok::ALL
            .iter()
            .map(|t| (t.field(), t.key()))
            .collect();
        let path = dir.join("tokens-map.json");
        std::fs::write(&path, serde_json::to_string_pretty(&map).expect("json")).expect("map");
        println!("{}", path.display());
        // And Violet Paper as a theme file: the template a variant is written over.
        let path = dir.join("violet_paper.theme.json");
        cantus_ui::theme::save_file(&cantus_ui::theme_added::violet_paper(), &path).expect("theme");
        println!("{}", path.display());
        return;
    }
    let themes: Vec<String> = rest
        .iter()
        .position(|a| a == "--themes")
        .and_then(|i| rest.get(i + 1))
        .map(|s| s.split(',').map(str::to_string).collect())
        .unwrap_or_else(|| THEMES.iter().map(|s| s.to_string()).collect());
    let sbs = rest.iter().any(|a| a == "--sbs");
    let only: Vec<&String> = rest
        .iter()
        .filter(|a| !a.starts_with("--") && !themes.contains(a) && CASES.contains(&a.as_str()))
        .collect();
    for case in CASES {
        if !only.is_empty() && !only.iter().any(|o| *o == case) {
            continue;
        }
        for theme in &themes {
            let label = theme_label(theme);
            let path = dir.join(format!("{case}-{label}.png"));
            window(&path, case, theme);
            println!("{}", path.display());
        }
        if sbs && themes.len() >= 2 {
            let out = dir.join(format!("sbs-{case}.png"));
            let paths = [
                dir.join(format!("{case}-{}.png", theme_label(&themes[0]))),
                dir.join(format!("{case}-{}.png", theme_label(&themes[1]))),
            ];
            side_by_side(&paths, &out);
            println!("{}", out.display());
        }
    }
}

/// A shot's name for a theme: a preset's id, or a theme file's stem.
fn theme_label(theme: &str) -> String {
    Path::new(theme)
        .file_stem()
        .map_or(theme.to_string(), |s| s.to_string_lossy().into_owned())
}

fn window(path: &Path, case: &str, theme: &str) {
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    let (engine, mut processor) = EngineHandle::manual(48_000.0);
    let inst = matches!(case, "chorda" | "arcus" | "unda");
    let project = if inst {
        cantus_ui::welcome_project()
    } else {
        cantus_ui::demos::startup_project()
    };
    let mut app = CantusApp::from_parts(Some(project), engine, false);
    let tmp = std::env::temp_dir().join("cantus-shoot-violet");
    app.settings_path = tmp.join("settings.json");
    app.layouts_path = tmp.join("layouts.json");
    app.keymap_path = tmp.join("keymap.toml");
    app.appearance_path = tmp.join("appearance.json");
    // A preset by its id, or a theme file (`*.json`, a variant being judged) as the user's.
    let file = theme
        .ends_with(".json")
        .then(|| cantus_ui::theme::load_file(Path::new(theme)).expect("a theme file"));
    let dark = match &file {
        Some(t) => t.dark,
        None => !(theme.contains("paper") || theme == "print" || theme == "daylight"),
    };
    let mode = if dark { Mode::Dark } else { Mode::Light };
    app.appearance.mode = mode;
    match file {
        Some(t) => {
            app.appearance.assign(mode, format!("user:{}", t.name));
            app.appearance.themes.push(t);
        }
        None => app.appearance.assign(mode, format!("preset:{theme}")),
    }
    app.attach(&ctx);
    if inst {
        app.demo_editor(case);
        if let Some(k) = cantus_ui::views::inst::kind_from_name(case) {
            cantus_ui::views::inst::demo_feed(&mut app, k);
        }
    } else {
        stage(&mut app, case);
    }
    let menu = case == "menu";
    let mut audio = vec![0f32; 800 * 2];
    let points = vec2(SIZE.0 / ZOOM, SIZE.1 / ZOOM);
    let mut frames = 0;
    raster::shoot_events(
        &ctx,
        points,
        ZOOM,
        30,
        Some(path),
        |f, _| {
            let mut ev = Vec::new();
            if menu && (12..=14).contains(&f) {
                // A click on the bar's *Edit*: the menu with its checks and shortcuts.
                let at = egui::pos2(85.0, 14.0);
                ev.push(egui::Event::PointerMoved(at));
                if f > 12 {
                    ev.push(egui::Event::PointerButton {
                        pos: at,
                        button: egui::PointerButton::Primary,
                        pressed: f == 13,
                        modifiers: egui::Modifiers::NONE,
                    });
                }
            }
            if menu && f > 14 {
                // The pointer on a row, for its hover.
                ev.push(egui::Event::PointerMoved(egui::pos2(110.0, 92.0)));
            }
            ev
        },
        |root| {
            frames += 1;
            if frames == 2 && case == "mixer" {
                app.workspace.toggle_fold(Pane::Arrangement);
            }
            if !inst {
                for _ in 0..3 {
                    processor.process(&mut audio, 2);
                }
            }
            app.frame(root);
        },
    );
}

/// The song as a case shows it: the Chords track chosen, a few of its notes selected, a
/// loop, the transport playing; then the case's view.
fn stage(app: &mut CantusApp, case: &str) {
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
    match case {
        "mixer" => {
            app.show_central(CentralView::Mixer);
            cantus_ui::views::mixer::set_width_mode(app, WidthMode::Wide);
            // Mute and solo, so their states show beside each other.
            let ids: Vec<_> = app.project().strips.iter().map(|x| x.id).collect();
            for id in ids.iter().skip(2).take(2) {
                app.exec(cantus_core::Command::SetStripMute {
                    id: *id,
                    muted: true,
                });
            }
            if let Some(id) = ids.get(5) {
                app.exec(cantus_core::Command::SetStripSolo {
                    id: *id,
                    soloed: true,
                });
            }
        }
        "pianoroll" => {
            if let (Some(t), Some(c)) = (app.selected_track, app.selected_clip) {
                app.piano_roll.key_height = 12.0;
                cantus_ui::views::piano_roll::fit_clip(app, t, c);
            }
            app.show_central(CentralView::PianoRoll);
        }
        "rack" => app.show_central(CentralView::ChannelRack),
        "browser" => app.workspace.reveal(Pane::Browser),
        "look" => open(app, SettingsPage::Appearance),
        "dialog" => app.unsaved_prompt = Some(cantus_ui::views::unsaved::Then::New),
        _ => {}
    }
    app.sync_engine();
    app.toggle_play();
}

/// Two shots side by side, a gap between them.
fn side_by_side(paths: &[PathBuf; 2], out: &Path) {
    let load = |p: &PathBuf| {
        let bytes = std::fs::read(p).expect("a shot");
        tiny_skia::Pixmap::decode_png(&bytes).expect("decode")
    };
    let (a, b) = (load(&paths[0]), load(&paths[1]));
    let (w, h) = (a.width(), a.height());
    let gap = 12;
    let mut sheet = tiny_skia::Pixmap::new(2 * w + gap, h).expect("sheet");
    sheet.fill(tiny_skia::Color::from_rgba8(128, 128, 128, 255));
    for (i, shot) in [a, b].iter().enumerate() {
        sheet.draw_pixmap(
            (i as u32 * (w + gap)) as i32,
            0,
            shot.as_ref(),
            &tiny_skia::PixmapPaint::default(),
            tiny_skia::Transform::identity(),
            None,
        );
    }
    sheet.save_png(out).expect("write the sheet");
}

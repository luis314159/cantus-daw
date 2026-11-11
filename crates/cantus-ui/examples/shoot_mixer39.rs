//! Round 39's mixer as des-mixer drew it, the **whole window** headless: the demo song the
//! program opens on (Neon Tide), the mixer maximised with Chords selected, playing, in
//! Slate Night and Slate Paper, at 1280 × 720 (100 %) and 1920 × 1080 (125 %).
//!
//! `cargo run --release -p cantus-ui --example shoot_mixer39 -- <out-dir> [filter…]`
//!
//! Writes `mixer39-<case>-<theme>-<w>x<h>.png`. Cases: `wide` (playing), `idle`
//! (stopped: grooves, no "−∞"), `compact`, `large`, `es` (Spanish, a strip soloed),
//! `docked` (the arrangement above, the desk short: the rows that leave).

use cantus_engine::EngineHandle;
use cantus_ui::app::CentralView;
use cantus_ui::i18n::{self, Locale};
use cantus_ui::kit::appearance::Mode;
use cantus_ui::views::mixer::layout::WidthMode;
use cantus_ui::views::workspace::Pane;
use cantus_ui::{raster, CantusApp};
use std::path::{Path, PathBuf};

#[derive(Copy, Clone)]
struct Case {
    name: &'static str,
    mode: WidthMode,
    play: bool,
    locale: Locale,
    solo: bool,
    maximise: bool,
}

fn shoot(dir: &Path, case: Case, preset: &str, dark: bool, (w, h): (f32, f32), zoom: f32) {
    i18n::set_locale(case.locale);
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    let (engine, mut processor) = EngineHandle::manual(48_000.0);
    let mut app = CantusApp::from_parts(Some(cantus_ui::demos::startup_project()), engine, false);
    let tmp = std::env::temp_dir().join("cantus-shoot-mixer39");
    app.settings_path = tmp.join("settings.json");
    app.layouts_path = tmp.join("layouts.json");
    app.keymap_path = tmp.join("keymap.toml");
    app.appearance_path = tmp.join("appearance.json");
    app.appearance.mode = if dark { Mode::Dark } else { Mode::Light };
    let mode = app.appearance.mode;
    app.appearance.assign(mode, format!("preset:{preset}"));
    app.attach(&ctx);
    app.show_central(CentralView::Mixer);
    cantus_ui::views::mixer::set_width_mode(&mut app, case.mode);
    let pick = app
        .project()
        .strips
        .iter()
        .find(|s| s.name == "Chords")
        .map(|s| s.id)
        .or_else(|| {
            let p = app.project();
            p.tracks
                .iter()
                .find(|t| t.name == "Chords")
                .and_then(|t| t.strip)
        })
        .unwrap_or(app.project().strips[5].id);
    app.select_strip(pick);
    if case.solo {
        let lead = app.project().strips.get(7).map(|s| s.id);
        if let Some(id) = lead {
            app.exec(cantus_core::Command::SetStripSolo { id, soloed: true });
        }
    }
    app.sync_engine();
    if case.play {
        app.toggle_play();
    }
    let mut audio = vec![0f32; 800 * 2];
    let points = egui::vec2(w / zoom, h / zoom);
    let file = format!(
        "mixer39-{}-{preset}-{}x{}.png",
        case.name, w as u32, h as u32
    );
    let path = dir.join(&file);
    let mut frames = 0;
    raster::shoot(&ctx, points, zoom, 30, Some(&path), |root| {
        frames += 1;
        // The mixer takes the window under the top bar, the inspector beside it (the
        // arrangement folded to its rail; "docked" keeps it open above).
        if frames == 2 && case.maximise {
            app.workspace.toggle_fold(Pane::Arrangement);
        }
        if case.play {
            for _ in 0..3 {
                processor.process(&mut audio, 2);
            }
        }
        app.frame(root);
    });
    println!("{}", path.display());
}

fn main() {
    let mut args = std::env::args().skip(1);
    let dir = args
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/shoot-mixer39"));
    let only: Vec<String> = args.collect();
    std::fs::create_dir_all(&dir).expect("create the output directory");
    let base = Case {
        name: "wide",
        mode: WidthMode::Wide,
        play: true,
        locale: Locale::En,
        solo: false,
        maximise: true,
    };
    let cases = [
        base,
        Case {
            name: "idle",
            play: false,
            ..base
        },
        Case {
            name: "compact",
            mode: WidthMode::Compact,
            ..base
        },
        Case {
            name: "large",
            mode: WidthMode::Xl,
            ..base
        },
        Case {
            name: "es",
            locale: Locale::Es,
            solo: true,
            ..base
        },
        Case {
            name: "docked",
            maximise: false,
            ..base
        },
    ];
    for case in cases {
        for (preset, dark) in [("slate_night", true), ("slate_paper", false)] {
            for (size, zoom) in [((1280.0, 720.0), 1.0), ((1920.0, 1080.0), 1.25)] {
                let tag = format!("{}-{preset}-{}", case.name, size.0 as u32);
                if !only.is_empty() && !only.iter().any(|o| tag.contains(o.as_str())) {
                    continue;
                }
                shoot(&dir, case, preset, dark, size, zoom);
            }
        }
    }
}

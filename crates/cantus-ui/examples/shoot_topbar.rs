//! The top bar (round 28, ADR 0052), screenshotted without a screen — to judge against FL's
//! strip at every width:
//!
//! - `bar`: the bar alone at 800, 1280, 1920 and 2560 pixels wide;
//! - `overflow`: the same with the ⋯ menu open (what folded, usable there);
//! - at 1920 only: `metronome` (its options open), `metre` (the time signature's editor),
//!   `customise` (the panel), `lufs` (the meter in loudness, the song in pattern mode, a
//!   track armed, the typing keyboard on).
//!
//! Each at 75, 100 and 200 %, in English and Spanish, in Cantus Night and Paper.
//!
//! `cargo run --release -p cantus-ui --example shoot_topbar -- <out-dir> [filter…]` writes
//! `topbar-<what>-<width>-<theme>-<lang>-<scale>.png`; a filter keeps the names holding it.

use cantus_engine::EngineHandle;
use cantus_ui::i18n::Locale;
use cantus_ui::views::transport;
use cantus_ui::{welcome_project, CantusApp};
use egui::vec2;
use std::path::PathBuf;

const WIDTHS: [f32; 4] = [800.0, 1280.0, 1920.0, 2560.0];
const ONLY_1920: [&str; 4] = ["metronome", "metre", "customise", "lufs"];

fn main() {
    let mut args = std::env::args().skip(1);
    let dir = PathBuf::from(args.next().unwrap_or_else(|| "shots".into()));
    let filters: Vec<String> = args.collect();
    std::fs::create_dir_all(&dir).expect("output dir");
    let whats = ["bar", "overflow"].into_iter().chain(ONLY_1920);
    for what in whats {
        for width in WIDTHS {
            if ONLY_1920.contains(&what) && width != 1920.0 {
                continue;
            }
            for theme in ["night", "paper"] {
                for lang in [Locale::En, Locale::Es] {
                    for scale in [0.75f32, 1.0, 2.0] {
                        let name = format!(
                            "topbar-{what}-{}-{theme}-{}-{}",
                            width as u32,
                            lang.code(),
                            (scale * 100.0) as u32
                        );
                        if !filters.is_empty()
                            && !filters.iter().any(|f| name.contains(f.as_str()))
                        {
                            continue;
                        }
                        cantus_ui::i18n::set_locale(lang);
                        let path = dir.join(format!("{name}.png"));
                        shoot(&path, what, width, theme, scale);
                        println!("wrote {}", path.display());
                    }
                }
            }
        }
    }
}

fn context(theme: &str) -> egui::Context {
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    let t = if theme == "paper" {
        cantus_ui::theme::paper()
    } else {
        cantus_ui::theme_family::cantus_night()
    };
    cantus_ui::theme::apply(&t, &ctx);
    ctx
}

fn app_for(theme: &str) -> CantusApp {
    use cantus_ui::kit::appearance::Mode;
    let mut app = CantusApp::from_parts(Some(welcome_project()), EngineHandle::dummy(), false);
    let tmp = std::env::temp_dir().join("cantus-shoot-topbar");
    app.settings_path = tmp.join("settings.json");
    app.layouts_path = tmp.join("layouts.json");
    app.keymap_path = tmp.join("keymap.toml");
    app.appearance_path = tmp.join("appearance.json");
    let (preset, mode) = if theme == "paper" {
        ("paper", Mode::Light)
    } else {
        ("cantus_night", Mode::Dark)
    };
    app.appearance.mode = mode;
    app.appearance.assign(mode, format!("preset:{preset}"));
    app
}

fn shoot(path: &std::path::Path, what: &str, width: f32, theme: &str, scale: f32) {
    let ctx = context(theme);
    let mut app = app_for(theme);
    app.attach(&ctx);
    let tall = what != "bar";
    if what == "lufs" {
        app.settings.topbar.meter_lufs = true;
        app.keyboard_piano = true;
        let t = app.project().tracks[0].id;
        let c = app.project().tracks[0].clips.first().map(|c| c.id);
        app.selected_track = Some(t);
        app.selected_clip = c;
        app.run_action(cantus_ui::keymap::Action::TogglePatternMode);
        app.toggle_arm(t, false);
    }
    // The bar is 54 points tall at every zoom; the image is that many points times the zoom.
    let size = vec2(width / scale, if tall { (560.0 / scale).max(360.0) } else { 54.0 });
    let mut frame = 0;
    cantus_ui::raster::shoot(&ctx, size, scale, 6, Some(path), |root| {
        frame += 1;
        if frame == 2 {
            match what {
                "overflow" => transport::open_overflow(root.ctx()),
                "metronome" => transport::open_metronome_options(root.ctx()),
                "metre" => transport::open_metre_editor(root.ctx()),
                "customise" => transport::open_customise(&mut app),
                _ => {}
            }
        }
        egui::Panel::top(egui::Id::new("toolbar"))
            .frame(egui::Frame::NONE)
            .exact_size(transport::HEIGHT)
            .show(root, |ui| transport::show(&mut app, ui));
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(cantus_ui::kit::Style::current().t.panel))
            .show(root, |_| {});
    });
}

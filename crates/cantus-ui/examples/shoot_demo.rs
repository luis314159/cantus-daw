//! Round 28's showcase song, *With All Your Heart*, on screen without a screen:
//! `cargo run -p cantus-ui --example shoot_demo -- <out-dir> [case…]`.
//!
//! The song is opened as the program opens it — *File ▸ Demo songs…*, the first row — and
//! the whole window is rendered: the arrangement with its automation lanes, the mixer with
//! the desk, the topline in the piano roll, and the editors of the round's units as the song
//! sets them (Chorda with its pedal, Tonus, Fluctus, Concentus, Speculum on the master), in
//! Cantus Night and Cantus Paper, English and Spanish, at 75 %, 100 % and 200 %.

use cantus_core::{EffectKind, StripId};
use cantus_engine::EngineHandle;
use cantus_ui::app::{CantusApp, CentralView};
use cantus_ui::i18n::{self, Locale};
use cantus_ui::kit::appearance::Mode;
use cantus_ui::{raster, views, welcome_project};
use std::path::{Path, PathBuf};

/// What a case shows.
#[derive(Copy, Clone)]
enum Show {
    Window,
    Mixer,
    Topline,
    Piano,
    Effect(&'static str, EffectKind),
    Master(EffectKind),
}

fn track_named(app: &CantusApp, name: &str) -> Option<(cantus_core::TrackId, StripId)> {
    app.project()
        .tracks
        .iter()
        .find(|t| t.name == name)
        .and_then(|t| Some((t.id, t.strip?)))
}

fn setup(app: &mut CantusApp, show: Show) {
    let en = |k: &str| i18n::t(k);
    match show {
        Show::Window => {}
        Show::Mixer => app.show_central(CentralView::Mixer),
        Show::Topline => {
            if let Some((track, _)) = track_named(app, &en("demos.track.vox")) {
                let clip = app
                    .project()
                    .track(track)
                    .and_then(|t| {
                        let chorus = en("demos.section.chorus");
                        t.clips
                            .iter()
                            .find(|c| c.automation.is_none() && c.name == chorus)
                    })
                    .map(|c| c.id);
                if let Some(clip) = clip {
                    app.select_clip(track, clip);
                }
            }
            app.show_central(CentralView::PianoRoll);
        }
        Show::Piano => {
            if let Some((track, _)) = track_named(app, &en("demos.track.piano")) {
                app.select_track(track);
                app.open_inst_editor(track);
                views::inst::demo_feed(app, cantus_core::InstrumentKind::Chorda);
            }
        }
        Show::Effect(track, kind) => {
            if let Some((_, strip)) = track_named(app, &en(track)) {
                app.select_strip(strip);
                let effect = app
                    .project()
                    .effects_of(strip)
                    .and_then(|e| e.iter().find(|e| e.spec.kind == kind))
                    .map(|e| e.id);
                if let Some(effect) = effect {
                    app.open_effect_editor(strip, effect);
                    views::fx::demo_feed(app, kind);
                }
            }
        }
        Show::Master(kind) => {
            let effect = app
                .project()
                .master
                .effects
                .iter()
                .find(|e| e.spec.kind == kind)
                .map(|e| e.id);
            if let Some(effect) = effect {
                app.open_effect_editor(StripId::MASTER, effect);
                views::fx::demo_feed(app, kind);
            }
        }
    }
}

fn shoot(
    dir: &Path,
    name: &str,
    dark: bool,
    locale: Locale,
    zoom: f32,
    size: (f32, f32),
    show: Show,
) {
    i18n::set_locale(locale);
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    let tmp = std::env::temp_dir().join("cantus-shoot-demo");
    let mut app = CantusApp::from_parts(Some(welcome_project()), EngineHandle::dummy(), false);
    // Never touch the user's files.
    app.settings_path = tmp.join("settings.json");
    app.layouts_path = tmp.join("layouts.json");
    app.keymap_path = tmp.join("keymap.toml");
    app.appearance_path = tmp.join("appearance.json");
    app.appearance.mode = if dark { Mode::Dark } else { Mode::Light };
    let preset = if dark { "cantus_night" } else { "cantus_paper" };
    let mode = app.appearance.mode;
    app.appearance.assign(mode, format!("preset:{preset}"));
    app.attach(&ctx);
    // The song, opened as the File menu's picker opens it (the first row).
    views::demos::request(&mut app, 0);
    assert_eq!(app.project().name, i18n::t("demos.song.heart"));
    setup(&mut app, show);
    let (w, h) = size;
    let points = egui::vec2(w / zoom, h / zoom);
    let path = dir.join(format!("{name}.png"));
    raster::shoot(&ctx, points, zoom, 8, Some(&path), |root| app.frame(root));
    println!("{}", path.display());
}

fn main() {
    let mut args = std::env::args().skip(1);
    let dir = args
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("shots-demo"));
    let only: Vec<String> = args.collect();
    std::fs::create_dir_all(&dir).expect("create the output directory");
    let shows: [(&str, Show); 9] = [
        ("window", Show::Window),
        ("mixer", Show::Mixer),
        ("topline", Show::Topline),
        ("chorda", Show::Piano),
        (
            "tonus",
            Show::Effect("demos.track.vox", EffectKind::AutoTune),
        ),
        (
            "fluctus",
            Show::Effect("demos.track.pad", EffectKind::VolumeShaper),
        ),
        (
            "concentus",
            Show::Effect("demos.track.pad", EffectKind::Chorus),
        ),
        (
            "tremor",
            Show::Effect("demos.track.keys", EffectKind::Tremolo),
        ),
        ("speculum", Show::Master(EffectKind::Analyzer)),
    ];
    for (theme, dark) in [("night", true), ("paper", false)] {
        for (lang, locale) in [("en", Locale::En), ("es", Locale::Es)] {
            for (what, show) in shows {
                let name = format!("heart-{what}-{theme}-{lang}");
                if !only.is_empty() && !only.iter().any(|o| name.contains(o.as_str())) {
                    continue;
                }
                shoot(&dir, &name, dark, locale, 1.0, (1920.0, 1080.0), show);
            }
        }
        for (zoom, tag) in [(0.75, "z75"), (2.0, "z200")] {
            for (what, show) in [("window", Show::Window), ("chorda", Show::Piano)] {
                let name = format!("heart-{what}-{theme}-en-{tag}");
                if !only.is_empty() && !only.iter().any(|o| name.contains(o.as_str())) {
                    continue;
                }
                shoot(&dir, &name, dark, Locale::En, zoom, (1920.0, 1080.0), show);
            }
        }
    }
}

//! The preset browser (round 37, ADR 0101) over the whole window: on an Unda (the role
//! chips, a preset heard — its mini waveform and the player's), and on Fervor with a
//! search and one heard on the clip; in Cantus Night and Cantus Paper, English and Spanish, at
//! 100 % (and 200 % for the first case).
//!
//! `cargo run -p cantus-ui --example shoot_presets [-- out_dir]` (default
//! `target/shoot-presets`). Writes `presets-<case>-<theme>-<lang>-<zoom>.png`.

use cantus_core::{EffectKind, InstrumentKind, InstrumentSpec, NativeSpec};
use cantus_engine::EngineHandle;
use cantus_ui::app::CantusApp;
use cantus_ui::i18n::{self, Locale};
use cantus_ui::preset_audition::{self as aud, Target};
use cantus_ui::preset_tags::Role;
use cantus_ui::raster;
use std::path::{Path, PathBuf};
use std::time::Duration;

const SIZE: egui::Vec2 = egui::vec2(1600.0, 1000.0);

fn app() -> CantusApp {
    let mut p = aud::phrase_project(
        InstrumentSpec::Native(NativeSpec::new(InstrumentKind::Wavetable)),
        Role::Lead,
    );
    let strip = p.tracks[0].strip.expect("a strip");
    let slot = p.make_effect(EffectKind::Saturator);
    p.strip_mut(strip).unwrap().effects.push(slot);
    let tmp = std::env::temp_dir().join("cantus-shoot-presets");
    let mut app = CantusApp::from_parts(Some(p), EngineHandle::dummy(), false);
    app.settings_path = tmp.join("settings.json");
    app.layouts_path = tmp.join("layouts.json");
    app.keymap_path = tmp.join("keymap.toml");
    app.appearance_path = tmp.join("appearance.json");
    app
}

/// Unda's presets, leads only, the second one heard.
fn unda(app: &mut CantusApp) {
    let t = app.project().tracks[0].id;
    app.open_preset_browser(Target::Track(t));
    app.presets.query.toggle_role(Role::Lead);
    let shown: Vec<String> = app.presets.shown().iter().map(|e| e.id.clone()).collect();
    for id in shown.iter().take(3) {
        app.audition_preset_id(id);
        app.finish_preset_audition(Duration::from_secs(60));
    }
    if let Some(id) = shown.get(1) {
        app.audition_preset_id(id);
        app.finish_preset_audition(Duration::from_secs(60));
    }
}

/// Fervor's presets with a word typed, one heard on the user's clip.
fn fervor(app: &mut CantusApp) {
    let strip = app.project().tracks[0].strip.unwrap();
    let effect = app.project().strip(strip).unwrap().effects[0].id;
    app.open_preset_browser(Target::Effect { strip, effect });
    app.presets.query.text = "tape".into();
    let first = app.presets.shown().first().map(|e| e.id.clone());
    if let Some(id) = first {
        app.audition_preset_id(&id);
        app.finish_preset_audition(Duration::from_secs(60));
    }
}

type Setup = fn(&mut CantusApp);

fn shoot(dir: &Path, case: &str, setup: Setup, theme: &str, lang: &str, zoom: f32) {
    i18n::set_locale(if lang == "es" { Locale::Es } else { Locale::En });
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    let t = if theme == "paper" {
        cantus_ui::theme::paper()
    } else {
        cantus_ui::theme::default_theme()
    };
    cantus_ui::theme::apply(&t, &ctx);
    let mut app = app();
    // The app installs its own appearance when it attaches: Paper is its light mode.
    app.appearance.mode = if theme == "paper" {
        cantus_ui::kit::appearance::Mode::Light
    } else {
        cantus_ui::kit::appearance::Mode::Dark
    };
    app.attach(&ctx);
    setup(&mut app);
    let path = dir.join(format!(
        "presets-{case}-{theme}-{lang}-{}.png",
        (zoom * 100.0) as u32
    ));
    raster::shoot(&ctx, SIZE, zoom, 8, Some(&path), |root| app.frame(root));
    println!("{}", path.display());
}

fn main() {
    let dir = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/shoot-presets")
        });
    std::fs::create_dir_all(&dir).expect("create the output directory");
    let cases: [(&str, Setup); 2] = [("unda", unda), ("fervor", fervor)];
    for (case, setup) in cases {
        for theme in ["night", "paper"] {
            for lang in ["en", "es"] {
                shoot(&dir, case, setup, theme, lang, 1.0);
            }
        }
    }
    shoot(&dir, "unda", unda, "night", "en", 2.0);
}

//! The verse at start, rendered headless (BACKLOG §6c): the splash with its verse band and
//! the window with the start card — the motto, the longest passage of each translation —
//! in English, Spanish (RVR1960) and Spanish (RV1909), at 75 %, 100 % and 200 %; and the
//! settings window's General section.
//!
//! `cargo run -p cantus-ui --example shoot_verse [-- out_dir [splash|card|settings]]`
//!
//! Writes into `out_dir` (default `target/shoot-verse`).

use cantus_engine::EngineHandle;
use cantus_ui::boot::{self, SplashVerse};
use cantus_ui::i18n::{self, Locale};
use cantus_ui::verse::{Reference, Shown, Spanish, Translation};
use cantus_ui::views::verse::Card;
use cantus_ui::{raster, welcome_project, CantusApp};
use std::path::{Path, PathBuf};

const LANGS: [(&str, Locale, Spanish); 3] = [
    ("en", Locale::En, Spanish::Rvr1960),
    ("es", Locale::Es, Spanish::Rvr1960),
    ("es1909", Locale::Es, Spanish::Rv1909),
];

fn ctx() -> egui::Context {
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    ctx
}

/// The passages worth looking at: the motto and the longest of the translation.
fn passages(t: Translation) -> [(&'static str, Reference); 2] {
    let longest = t
        .verses()
        .iter()
        .max_by_key(|v| v.text.chars().count())
        .expect("a passage")
        .reference;
    [
        ("motto", Reference::parse("COL 3:23").unwrap()),
        ("longest", longest),
    ]
}

fn splash(dir: &Path) {
    for (lang, locale, spanish) in LANGS {
        i18n::set_locale(locale);
        let t = Translation::for_locale(locale, spanish);
        for (name, reference) in passages(t) {
            for ppp in [0.75f32, 1.0, 2.0] {
                let ctx = ctx();
                let path = dir.join(format!("splash-{lang}-{name}-{}.png", (ppp * 100.0) as u32));
                let shown = Shown { reference, spanish };
                raster::shoot(&ctx, boot::size(true), ppp, 4, Some(&path), |root| {
                    let _ = boot::paint_splash_with(root, 0.42, SplashVerse::Shown(shown));
                });
                println!("{}", path.display());
            }
        }
    }
}

fn app(ctx: &egui::Context, spanish: Spanish) -> CantusApp {
    let tmp = std::env::temp_dir().join("cantus-shoot-verse");
    let mut app = CantusApp::from_parts(Some(welcome_project()), EngineHandle::dummy(), false);
    // Never touch the user's files.
    app.settings_path = tmp.join("settings.json");
    app.layouts_path = tmp.join("layouts.json");
    app.keymap_path = tmp.join("keymap.toml");
    app.appearance_path = tmp.join("appearance.json");
    app.attach(ctx);
    app.settings.verse.spanish = spanish;
    app
}

fn card(dir: &Path) {
    for (lang, locale, spanish) in LANGS {
        i18n::set_locale(locale);
        let t = Translation::for_locale(locale, spanish);
        for (name, reference) in passages(t) {
            for zoom in [0.75f32, 1.0, 2.0] {
                let ctx = ctx();
                let mut app = app(&ctx, spanish);
                app.verse = Some(Card::new(reference));
                let (w, h) = (1280.0, 720.0);
                let path = dir.join(format!("card-{lang}-{name}-{}.png", (zoom * 100.0) as u32));
                raster::shoot(
                    &ctx,
                    egui::vec2(w / zoom, h / zoom),
                    zoom,
                    8,
                    Some(&path),
                    |root| app.frame(root),
                );
                println!("{}", path.display());
            }
        }
    }
}

fn settings(dir: &Path) {
    for (lang, locale, spanish) in LANGS.into_iter().take(2) {
        i18n::set_locale(locale);
        let ctx = ctx();
        let mut app = app(&ctx, spanish);
        app.show_settings = true;
        let path = dir.join(format!("settings-{lang}.png"));
        raster::shoot(
            &ctx,
            egui::vec2(1280.0, 900.0),
            1.0,
            8,
            Some(&path),
            |root| app.frame(root),
        );
        println!("{}", path.display());
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let dir = args.next().map(PathBuf::from).unwrap_or_else(|| {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/shoot-verse")
    });
    let only: Vec<String> = args.collect();
    std::fs::create_dir_all(&dir).expect("create the output directory");
    let want = |n: &str| only.is_empty() || only.iter().any(|o| o == n);
    if want("splash") {
        splash(&dir);
    }
    if want("card") {
        card(&dir);
    }
    if want("settings") {
        settings(&dir);
    }
}

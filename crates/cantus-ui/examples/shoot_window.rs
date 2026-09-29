//! The whole window, rendered headless (ADR 0024 §14; `research/ui/cantus-audit.md`,
//! Appendix A): the application as it opens — toolbar, workspace, status bar — in Slate
//! and in Paper, at 1280 × 720 and 1920 × 1080, plus the cases the look must survive: the
//! interface at 75 % and 200 %, Spanish, the appearance page, each editor in front, and the
//! splash.
//!
//! `cargo run -p cantus-ui --example shoot_window [-- out_dir [case…]]`
//!
//! Writes `window-<theme>-<size>[-<case>].png` into `out_dir` (default
//! `target/shoot-window`). A case name given on the command line renders only the cases
//! whose name starts with it.

use cantus_engine::EngineHandle;
use cantus_ui::app::{CantusApp, CentralView};
use cantus_ui::i18n::{self, Locale};
use cantus_ui::kit::appearance::Mode;
use cantus_ui::{raster, welcome_project};
use std::path::{Path, PathBuf};

struct Case {
    name: &'static str,
    dark: bool,
    size: (f32, f32),
    zoom: f32,
    locale: Locale,
    setup: Setup,
}

/// A case's own setup of the application.
type Setup = fn(&mut CantusApp);
/// An extra case: its name, the interface's zoom, the language, its setup.
type Extra = (&'static str, f32, Locale, Setup);

fn nothing(_: &mut CantusApp) {}
fn piano_roll(a: &mut CantusApp) {
    a.show_central(CentralView::PianoRoll);
}
fn rack(a: &mut CantusApp) {
    a.show_central(CentralView::ChannelRack);
}
fn appearance(a: &mut CantusApp) {
    a.show_theme_editor = true;
}
fn appearance_colours(a: &mut CantusApp) {
    a.show_theme_editor = true;
    a.appearance_page.tab = cantus_ui::views::appearance::Tab::Colours;
}
fn appearance_themes(a: &mut CantusApp) {
    a.show_theme_editor = true;
    a.appearance_page.tab = cantus_ui::views::appearance::Tab::Themes;
}
fn settings(a: &mut CantusApp) {
    a.show_settings = true;
}
fn no_inspector(a: &mut CantusApp) {
    a.toggle_pane(cantus_ui::views::workspace::Pane::Inspector);
}

fn cases() -> Vec<Case> {
    let mut v = Vec::new();
    for (theme, dark) in [("slate", true), ("paper", false)] {
        for size in [(1280.0, 720.0), (1920.0, 1080.0)] {
            v.push(Case {
                name: theme,
                dark,
                size,
                zoom: 1.0,
                locale: Locale::En,
                setup: nothing,
            });
        }
        let extra: [Extra; 10] = [
            ("pianoroll", 1.0, Locale::En, piano_roll),
            ("rack", 1.0, Locale::En, rack),
            ("appearance", 1.0, Locale::En, appearance),
            ("colours", 1.0, Locale::En, appearance_colours),
            ("themes", 1.0, Locale::En, appearance_themes),
            ("settings", 1.0, Locale::En, settings),
            ("noinspector", 1.0, Locale::En, no_inspector),
            ("z75", 0.75, Locale::En, nothing),
            ("z200", 2.0, Locale::En, nothing),
            ("es", 1.0, Locale::Es, nothing),
        ];
        for (name, zoom, locale, setup) in extra {
            v.push(Case {
                name: Box::leak(format!("{theme}-{name}").into_boxed_str()),
                dark,
                size: (1280.0, 720.0),
                zoom,
                locale,
                setup,
            });
        }
    }
    v
}

fn shoot(dir: &Path, case: &Case) {
    i18n::set_locale(case.locale);
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    let tmp = std::env::temp_dir().join("cantus-shoot-window");
    let mut app = CantusApp::from_parts(Some(welcome_project()), EngineHandle::dummy(), false);
    // Never touch the user's files.
    app.settings_path = tmp.join("settings.json");
    app.layouts_path = tmp.join("layouts.json");
    app.keymap_path = tmp.join("keymap.toml");
    app.appearance_path = tmp.join("appearance.json");
    app.appearance.mode = if case.dark { Mode::Dark } else { Mode::Light };
    app.attach(&ctx);
    // A strip is selected, as after a click on the desk.
    if let Some(strip) = app.project().tracks.get(1).and_then(|t| t.strip) {
        app.select_strip(strip);
    }
    (case.setup)(&mut app);
    // The physical size is the case's; the interface's zoom makes the points fewer.
    let (w, h) = case.size;
    let points = egui::vec2(w / case.zoom, h / case.zoom);
    let file = format!("window-{}-{}x{}.png", case.name, w as u32, h as u32);
    let path = dir.join(&file);
    raster::shoot(&ctx, points, case.zoom, 8, Some(&path), |root| {
        app.frame(root)
    });
    println!("{}", path.display());
}

fn main() {
    let mut args = std::env::args().skip(1);
    let dir = args.next().map(PathBuf::from).unwrap_or_else(|| {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/shoot-window")
    });
    let only: Vec<String> = args.collect();
    std::fs::create_dir_all(&dir).expect("create the output directory");
    for case in cases() {
        if only.is_empty() || only.iter().any(|o| case.name.starts_with(o.as_str())) {
            shoot(&dir, &case);
        }
    }
    if only.is_empty() || only.iter().any(|o| "splash".starts_with(o.as_str())) {
        i18n::set_locale(Locale::En);
        for ppp in [1.0f32, 2.0] {
            let ctx = egui::Context::default();
            cantus_ui::design::install_fonts(&ctx);
            cantus_ui::design::install_style(&ctx);
            let path = dir.join(format!("splash-{}x.png", ppp as u32));
            raster::shoot(&ctx, cantus_ui::boot::SIZE, ppp, 4, Some(&path), |root| {
                cantus_ui::boot::paint_splash(root, 0.42);
            });
            println!("{}", path.display());
        }
    }
}

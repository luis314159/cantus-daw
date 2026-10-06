//! The audio setup (round 20), rendered headless in the whole window, in Slate and Paper:
//! the chip at rest, warning and lost; the popover (with a problem, with its interface
//! list open); the first-run setup; the routing page; the device cards.
//!
//! `cargo run -p cantus-ui --example shoot_audio_setup [-- out_dir [case…]]`
//! writes `audio-<theme>-<case>.png` into `out_dir` (default `target/shoot-audio`).

use cantus_engine::EngineHandle;
use cantus_ui::app::CantusApp;
use cantus_ui::audio_setup::{self as setup, demo, Notice, PopList};
use cantus_ui::i18n::{self, Locale};
use cantus_ui::kit::appearance::Mode;
use cantus_ui::{raster, welcome_project};
use std::path::{Path, PathBuf};

type Setup = fn(&mut CantusApp);

fn chip(_: &mut CantusApp) {}
fn chip_warn(a: &mut CantusApp) {
    a.audio_setup.demo.as_mut().unwrap().dropouts = 3;
}
fn chip_lost(a: &mut CantusApp) {
    a.audio_setup.plug.lost = Some(demo::UMC.into());
    a.audio_setup.notice(Notice::Lost(demo::UMC.into()));
}
fn popover(a: &mut CantusApp) {
    a.audio_setup.popover = true;
}
fn popover_problem(a: &mut CantusApp) {
    let l = a.audio_setup.demo.as_mut().unwrap();
    l.input = Some(demo::REALTEK_IN.into());
    l.input_error = Some(format!(
        "{}: the input runs at 44100 Hz and cannot run at 48000 Hz",
        demo::REALTEK_IN
    ));
    a.settings.audio.input_device = Some(demo::REALTEK_IN.into());
    a.audio_setup.popover = true;
}
fn popover_list(a: &mut CantusApp) {
    a.audio_setup.popover = true;
    a.audio_setup.pop_list = Some(PopList::Interface);
}
fn first_run(a: &mut CantusApp) {
    setup::open_first_run(a);
    a.audio_setup.tone = Some(0);
}
fn routing(a: &mut CantusApp) {
    a.audio_setup.routing = true;
    let p = a.settings.audio_setup.profile_mut(demo::UMC);
    p.inputs = vec!["Guitar".into(), "Vocal mic".into()];
    p.outputs = vec![
        "Monitors L".into(),
        "Monitors R".into(),
        "Phones L".into(),
        "Phones R".into(),
    ];
    p.cue = Some(2);
    p.cue_db = -6.0;
}
fn drivers(a: &mut CantusApp) {
    a.audio_setup.drivers = true;
}
fn notices(a: &mut CantusApp) {
    a.audio_setup
        .notice(Notice::Connected("Scarlett 2i2 USB".into()));
    a.audio_setup.notice(Notice::Reconnected(demo::UMC.into()));
}

const CASES: [(&str, Setup, Locale, f32); 13] = [
    ("drivers", drivers, Locale::En, 1.0),
    ("drivers-es", drivers, Locale::Es, 1.0),
    ("chip", chip, Locale::En, 1.0),
    ("chip-warn", chip_warn, Locale::En, 1.0),
    ("chip-lost", chip_lost, Locale::En, 1.0),
    ("popover", popover, Locale::En, 1.0),
    ("popover-problem", popover_problem, Locale::En, 1.0),
    ("popover-list", popover_list, Locale::En, 1.0),
    ("first-run", first_run, Locale::En, 1.0),
    ("routing", routing, Locale::En, 1.0),
    ("notices", notices, Locale::En, 1.0),
    ("popover-es", popover, Locale::Es, 1.0),
    ("first-run-es-150", first_run, Locale::Es, 1.5),
];

fn shoot(dir: &Path, dark: bool, name: &str, f: Setup, locale: Locale, zoom: f32) {
    i18n::set_locale(locale);
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    let tmp = std::env::temp_dir().join("cantus-shoot-audio");
    let mut app = CantusApp::from_parts(Some(welcome_project()), EngineHandle::dummy(), false);
    app.settings_path = tmp.join("settings.json");
    app.layouts_path = tmp.join("layouts.json");
    app.keymap_path = tmp.join("keymap.toml");
    app.appearance_path = tmp.join("appearance.json");
    app.appearance.mode = if dark { Mode::Dark } else { Mode::Light };
    app.attach(&ctx);
    demo::setup(&mut app);
    f(&mut app);
    let theme = if dark { "slate" } else { "paper" };
    let path = dir.join(format!("audio-{theme}-{name}.png"));
    let (w, h) = (1280.0, 760.0);
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

fn main() {
    let mut args = std::env::args().skip(1);
    let dir = args.next().map(PathBuf::from).unwrap_or_else(|| {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/shoot-audio")
    });
    let only: Vec<String> = args.collect();
    std::fs::create_dir_all(&dir).expect("create the output directory");
    for dark in [true, false] {
        for (name, f, locale, zoom) in CASES {
            if only.is_empty() || only.iter().any(|o| name.starts_with(o.as_str())) {
                shoot(&dir, dark, name, f, locale, zoom);
            }
        }
    }
}

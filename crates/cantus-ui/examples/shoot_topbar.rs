//! The top bar and its dialogs as the owner judges them (round 39, `el-topbar`): the whole
//! window, in Slate Night and Slate Paper, at 1280 × 720 at 100 % and 1920 × 1080 at 125 %,
//! on the song the program opens on — beside `dir-a`'s phase B sheets.
//!
//! Cases: `window` (as it opens), `playing` (the transport lit, the meter moving), `overflow`
//! (the More panel open), `es` (Spanish, with the panel open), `settings` (Settings ▸
//! Audio), `look` (Settings ▸ Appearance), `firstrun` (the first-run audio setup), `export`;
//! and `bar`, the bar alone at every width of the widths sheet (1920 … 800).
//!
//! `cargo run -p cantus-ui --example shoot_topbar -- <out-dir> [case…]` writes
//! `window-<case>-<theme>-<w>x<h>.png` and `bar-<width>-<theme>.png`.

use cantus_engine::EngineHandle;
use cantus_ui::app::CantusApp;
use cantus_ui::i18n::{self, Locale};
use cantus_ui::kit::appearance::Mode;
use cantus_ui::raster;
use cantus_ui::views::transport;
use egui::vec2;
use std::path::{Path, PathBuf};

const CASES: [&str; 10] = [
    "window",
    "playing",
    "overflow",
    "es",
    "settings",
    "inputs",
    "look",
    "firstrun",
    "export",
    "exporting",
];
const SIZES: [((f32, f32), f32); 2] = [((1280.0, 720.0), 1.0), ((1920.0, 1080.0), 1.25)];
const THEMES: [(&str, bool); 2] = [("slate_night", true), ("slate_paper", false)];
const BAR_WIDTHS: [f32; 8] = [1920.0, 1600.0, 1440.0, 1280.0, 1152.0, 1024.0, 900.0, 800.0];

fn main() {
    let mut args = std::env::args().skip(1);
    let dir = PathBuf::from(args.next().unwrap_or_else(|| "shots".into()));
    let only: Vec<String> = args.collect();
    std::fs::create_dir_all(&dir).expect("output dir");
    let wanted = |c: &str| only.is_empty() || only.iter().any(|o| o == c);
    for (theme, dark) in THEMES {
        for case in CASES {
            if !wanted(case) {
                continue;
            }
            for (size, zoom) in SIZES {
                if case == "es" && zoom != 1.0 {
                    continue;
                }
                let path = dir.join(format!(
                    "window-{case}-{theme}-{}x{}.png",
                    size.0 as u32, size.1 as u32
                ));
                window(&path, case, theme, dark, size, zoom);
                println!("{}", path.display());
            }
        }
        if wanted("bar") {
            for w in BAR_WIDTHS {
                let path = dir.join(format!("bar-{}-{theme}.png", w as u32));
                bar(&path, w, theme, dark);
                println!("{}", path.display());
            }
        }
    }
}

fn app_for(ctx: &egui::Context, theme: &str, dark: bool) -> CantusApp {
    cantus_ui::design::install_fonts(ctx);
    cantus_ui::design::install_style(ctx);
    let project = cantus_ui::demos::startup_project();
    let mut app = CantusApp::from_parts(Some(project), EngineHandle::dummy(), false);
    let tmp = std::env::temp_dir().join("cantus-shoot-topbar");
    app.settings_path = tmp.join("settings.json");
    app.layouts_path = tmp.join("layouts.json");
    app.keymap_path = tmp.join("keymap.toml");
    app.appearance_path = tmp.join("appearance.json");
    let mode = if dark { Mode::Dark } else { Mode::Light };
    app.appearance.mode = mode;
    app.appearance.assign(mode, format!("preset:{theme}"));
    app.attach(ctx);
    app
}

fn window(path: &Path, case: &str, theme: &str, dark: bool, size: (f32, f32), zoom: f32) {
    use cantus_ui::views::workspace_settings::{open, SettingsPage};
    i18n::set_locale(if case == "es" { Locale::Es } else { Locale::En });
    let ctx = egui::Context::default();
    let mut app = app_for(&ctx, theme, dark);
    if let Some(strip) = app.project().tracks.get(5).and_then(|t| t.strip) {
        app.select_strip(strip);
    }
    match case {
        "playing" => {
            let mut live = cantus_ui::audio_setup::demo::live_on_umc();
            live.device = "Realtek Audio".into();
            live.wanted = live.device.clone();
            app.audio_setup.demo = Some(live);
            app.engine.play();
            let mut m = app.engine.meters().clone();
            m.master = [0.52, 0.47];
            app.engine.set_meters(m);
        }
        "settings" | "inputs" => {
            cantus_ui::views::settings_audio::demo_setup(&mut app);
            open(&mut app, SettingsPage::Audio);
        }
        "look" => open(&mut app, SettingsPage::Appearance),
        "firstrun" => {
            cantus_ui::audio_setup::demo::setup(&mut app);
            cantus_ui::audio_setup::open_first_run(&mut app);
        }
        "export" => cantus_ui::views::export::open(&mut app, None),
        "exporting" => {
            cantus_ui::views::export::open(&mut app, None);
            let out = std::env::temp_dir()
                .join("cantus-shoot-topbar")
                .join("export.wav");
            let _ = std::fs::create_dir_all(out.parent().unwrap());
            cantus_ui::views::export::start(&mut app, &out);
            std::thread::sleep(std::time::Duration::from_millis(150));
        }
        _ => {}
    }
    let points = vec2(size.0 / zoom, size.1 / zoom);
    let mut frame = 0;
    // `inputs`: Settings ▸ Audio scrolled to its inputs (the wheel over the page).
    let wheel = move |n: usize, _: &egui::Context| {
        if case == "inputs" && (3..6).contains(&n) {
            vec![
                egui::Event::PointerMoved(egui::pos2(points.x * 0.6, points.y * 0.5)),
                egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: egui::vec2(0.0, -400.0),
                    modifiers: egui::Modifiers::NONE,
                    phase: egui::TouchPhase::Move,
                },
            ]
        } else {
            Vec::new()
        }
    };
    raster::shoot_events(&ctx, points, zoom, 10, Some(path), wheel, |root| {
        frame += 1;
        if frame == 3 && (case == "overflow" || case == "es") {
            transport::open_overflow(&mut app);
        }
        app.frame(root)
    });
    if app.export.busy() {
        cantus_ui::views::export::stop(&mut app);
        cantus_ui::views::export::wait(&mut app);
    }
    i18n::set_locale(Locale::En);
}

/// The bar alone at `width` points, at 100 %.
fn bar(path: &Path, width: f32, theme: &str, dark: bool) {
    i18n::set_locale(Locale::En);
    let ctx = egui::Context::default();
    let mut app = app_for(&ctx, theme, dark);
    let size = vec2(width, transport::HEIGHT);
    raster::shoot(&ctx, size, 1.0, 6, Some(path), |root| {
        egui::Panel::top(egui::Id::new("toolbar"))
            .frame(egui::Frame::NONE)
            .exact_size(transport::HEIGHT)
            .show(root, |ui| transport::show(&mut app, ui));
        egui::CentralPanel::default().show(root, |_| {});
    });
}

//! The website's hero picture (`web/assets/app-mixer-xl.png`, 1912 x 1022), headless: the whole
//! window on the startup project as the program opens, with the Browser docked at the left, the
//! mixer Extra large and its cables on (the selected strip's: its sidechain, its sends, its
//! output), in Cantus Night. The site crops it (`web/src/tour.rs`: arrangement, mixer,
//! inspector), so the layout must keep those rectangles: the top 31 px stand where the
//! operating system's caption bar is on a real screen shot, and are painted here as a plain
//! band in the theme's panel colour.
//!
//! `cargo run --release -p cantus-ui --example shoot_site -- <out-dir> [night|paper]`
//!
//! Writes `app-mixer-xl.png` (or `app-mixer-xl-<theme>.png` for the other theme).

use cantus_engine::EngineHandle;
use cantus_ui::kit::appearance::Mode;
use cantus_ui::raster;
use cantus_ui::views::mixer::layout::{CableView, WidthMode};
use cantus_ui::views::workspace::Pane;
use cantus_ui::{demos, CantusApp};
use std::path::PathBuf;

const SIZE: (f32, f32) = (1912.0, 1022.0);
/// The height the caption bar of a real window takes.
const CAPTION: f32 = 31.0;
/// The browser's and the inspector's widths in `web/src/tour.rs`'s crops.
const BROWSER_PX: f32 = 277.0;
const INSPECTOR_PX: f32 = 290.0;

fn main() {
    let mut args = std::env::args().skip(1);
    let dir = PathBuf::from(args.next().unwrap_or_else(|| "shots".into()));
    let paper = args.next().is_some_and(|a| a == "paper");
    std::fs::create_dir_all(&dir).expect("output dir");
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);

    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    let (engine, mut processor) = EngineHandle::manual(48_000.0);
    let mut app = CantusApp::from_parts(Some(demos::startup_project()), engine, false);
    // Never touch the user's files.
    let tmp = std::env::temp_dir().join("cantus-shoot-site");
    app.settings_path = tmp.join("settings.json");
    app.layouts_path = tmp.join("layouts.json");
    app.keymap_path = tmp.join("keymap.toml");
    app.appearance_path = tmp.join("appearance.json");
    app.appearance.mode = if paper { Mode::Light } else { Mode::Dark };
    app.attach(&ctx);
    // A working studio: the demo interface, here an ASIO driver at 48 kHz and 256 samples.
    cantus_ui::audio_setup::demo::setup(&mut app);
    if let Some(live) = app.audio_setup.demo.as_mut() {
        live.driver = "ASIO".into();
        live.device = "ASIO4ALL v2".into();
        live.wanted = live.device.clone();
        live.block = 256;
    }
    // Named by its driver, as an unlisted interface is.
    app.audio_setup.scan = None;
    app.settings.audio.device = Some("ASIO4ALL v2".into());
    app.set_status(cantus_ui::i18n::t("status.autosaved"));
    // The desk ends cleanly: the strips nothing feeds (the empty slots) are removed.
    let empty: Vec<_> = app
        .project()
        .strips
        .iter()
        .filter(|s| s.name.is_empty() && s.effects.is_empty())
        .map(|s| s.id)
        .collect();
    for id in empty {
        app.exec(cantus_core::Command::RemoveStrip { id });
    }
    app.central = cantus_ui::app::CentralView::Mixer;
    cantus_ui::views::mixer::set_width_mode(&mut app, WidthMode::from_index(2));
    let mut prefs = app.mixer.prefs();
    prefs.cables = CableView::Selected;
    app.mixer.set_prefs(prefs);
    // The chords: sidechained from the kick, sent to the reverb; the selected strip's cables.
    if let Some(strip) = app.project().tracks.get(5).and_then(|t| t.strip) {
        app.select_strip(strip);
    }
    // Playing, a few bars in: the clock, the playhead and the meters move.
    app.sync_engine();
    app.seek(cantus_core::Ticks::beats(24));
    app.toggle_play();
    let mut audio = vec![0f32; 800 * 2];
    for _ in 0..40 {
        processor.process(&mut audio, 2);
    }
    let name = if paper { "app-mixer-xl-paper.png" } else { "app-mixer-xl.png" };
    let path = dir.join(name);
    let mut frame = 0;
    raster::shoot(&ctx, egui::vec2(SIZE.0, SIZE.1), 1.0, 14, Some(&path), |root| {
        // The browser opens once the window's width is known, so it takes its own width.
        if frame == 1 {
            app.toggle_pane(Pane::Browser);
            app.central = cantus_ui::app::CentralView::Mixer;
        }
        // ... then the browser and the inspector take the widths the site's crops expect.
        if frame == 3 {
            let main = app.workspace.dock.main_surface_mut();
            if let egui_dock::Node::Horizontal(split) = &mut main[egui_dock::NodeIndex::root()] {
                split.fraction = BROWSER_PX / SIZE.0;
            }
            app.workspace.impose_sizes(INSPECTOR_PX, 0.0);
        }
        frame += 1;
        processor.process(&mut audio, 2);
        let pal = cantus_ui::theme::p();
        egui::Panel::top(egui::Id::new("site-caption"))
            .exact_size(CAPTION)
            .resizable(false)
            .frame(egui::Frame::NONE.fill(pal.panel))
            .show(root, |_| {});
        app.frame(root);
    });
    println!("wrote {}", path.display());
}

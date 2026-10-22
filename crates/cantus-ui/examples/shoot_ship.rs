//! The owner's round-30 frictions, screenshotted without a screen:
//!
//! - `settings-<page>`: Settings with its pages' sidebar, on General, Audio and Plug-ins;
//! - `options`: the top bar's *Options* menu open;
//! - `picker-clap`: the device picker on an empty CLAP shelf (*Scan for plug-ins*, *Find
//!   more plug-ins…*);
//! - `count-in`: the count-in chip's menu (*Before recording*, *Also on play*), and the
//!   chip saying "1 bar rec";
//! - `scan-notice`: the status line after a scan found new plug-ins.
//!
//! Night and Paper, English and Spanish.
//! `cargo run -p cantus-ui --example shoot_ship -- <out-dir> [filter…]`

use cantus_engine::EngineHandle;
use cantus_ui::i18n::Locale;
use cantus_ui::kit::appearance::Mode;
use cantus_ui::picker::PickPurpose;
use cantus_ui::picker_catalog::Source;
use cantus_ui::picker_query::Shelf;
use cantus_ui::views::{self, transport, workspace_settings::SettingsPage};
use cantus_ui::{welcome_project, CantusApp};
use egui::vec2;
use std::path::{Path, PathBuf};

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

fn app_for(theme: &str, ctx: &egui::Context) -> CantusApp {
    let mut app = CantusApp::from_parts(Some(welcome_project()), EngineHandle::dummy(), false);
    let tmp = std::env::temp_dir().join("cantus-shoot-ship");
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
    app.plugins.scanner.standard_folders = false;
    app.attach(ctx);
    app
}

/// The top bar and an empty workspace under it, plus `extra` (windows over it).
fn shoot(
    path: &Path,
    ctx: &egui::Context,
    app: &mut CantusApp,
    size: egui::Vec2,
    mut first: impl FnMut(&mut CantusApp, &egui::Context),
    mut extra: impl FnMut(&mut CantusApp, &egui::Context),
) {
    let mut frame = 0;
    cantus_ui::raster::shoot(ctx, size, 1.0, 6, Some(path), |root| {
        frame += 1;
        let ctx = root.ctx().clone();
        if frame == 2 {
            first(app, &ctx);
        }
        egui::Panel::top(egui::Id::new("toolbar"))
            .frame(egui::Frame::NONE)
            .exact_size(transport::HEIGHT)
            .show(root, |ui| transport::show(app, ui));
        egui::Panel::bottom(egui::Id::new("status"))
            .frame(egui::Frame::NONE)
            .exact_size(views::workspace_status::HEIGHT)
            .show(root, |ui| views::workspace_status::show(app, ui));
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(cantus_ui::kit::Style::current().t.panel))
            .show(root, |_| {});
        extra(app, &ctx);
    });
    println!("wrote {}", path.display());
}

fn main() {
    let mut args = std::env::args().skip(1);
    let dir = PathBuf::from(args.next().unwrap_or_else(|| "shots".into()));
    let filters: Vec<String> = args.collect();
    std::fs::create_dir_all(&dir).expect("output dir");
    let whats = [
        "settings-general",
        "settings-audio",
        "settings-plugins",
        "options",
        "picker-clap",
        "count-in",
        "scan-notice",
    ];
    for what in whats {
        for theme in ["night", "paper"] {
            for lang in [Locale::En, Locale::Es] {
                let name = format!("ship-{what}-{theme}-{}", lang.code());
                if !filters.is_empty() && !filters.iter().any(|f| name.contains(f.as_str())) {
                    continue;
                }
                cantus_ui::i18n::set_locale(lang);
                let ctx = context(theme);
                let mut app = app_for(theme, &ctx);
                let path = dir.join(format!("{name}.png"));
                let size = vec2(1500.0, 940.0);
                match what {
                    w if w.starts_with("settings-") => {
                        let page = match w {
                            "settings-audio" => SettingsPage::Audio,
                            "settings-plugins" => SettingsPage::Plugins,
                            _ => SettingsPage::General,
                        };
                        if page == SettingsPage::Audio {
                            views::settings_audio::demo_setup(&mut app);
                        }
                        views::workspace_settings::open(&mut app, page);
                        shoot(
                            &path,
                            &ctx,
                            &mut app,
                            size,
                            |_, _| {},
                            |app, ctx| views::workspace_settings::window(app, ctx),
                        );
                    }
                    "options" => shoot(
                        &path,
                        &ctx,
                        &mut app,
                        vec2(1500.0, 520.0),
                        |_, ctx| transport::open_menu(ctx, 4),
                        |_, _| {},
                    ),
                    "picker-clap" => {
                        app.open_picker(PickPurpose::AddInstrument { index: None });
                        app.picker_view.set_shelf(Shelf::Source(Source::Clap));
                        shoot(
                            &path,
                            &ctx,
                            &mut app,
                            vec2(1920.0, 1080.0),
                            |_, _| {},
                            |app, ctx| views::picker::frame(app, ctx),
                        );
                    }
                    "count-in" => {
                        let mut rs = app.project().recording.clone();
                        rs.count_in_bars = 1;
                        app.set_record_settings(rs);
                        shoot(
                            &path,
                            &ctx,
                            &mut app,
                            vec2(1920.0, 420.0),
                            |_, ctx| transport::open_count_in(ctx),
                            |_, _| {},
                        );
                    }
                    _ => {
                        let names: Vec<String> = ["Odin 2", "Surge XT", "ValhallaSupermassive"]
                            .iter()
                            .map(|s| s.to_string())
                            .collect();
                        app.set_status(views::plugins::found_text(&names));
                        shoot(
                            &path,
                            &ctx,
                            &mut app,
                            vec2(1500.0, 300.0),
                            |_, _| {},
                            |_, _| {},
                        );
                    }
                }
            }
        }
    }
}

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
    // A scene named `…-200` is shot at 200 % (two pixels a point).
    let name = path.file_stem().map(|s| s.to_string_lossy().into_owned());
    let ppp = if name.is_some_and(|n| n.contains("-200-")) {
        2.0
    } else {
        1.0
    };
    cantus_ui::raster::shoot(ctx, size, ppp, 6, Some(path), |root| {
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
        "settings-cores",
        "settings-cores-200",
        "settings-picture",
        "settings-picture-200",
        "engines",
        "engines-200",
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
                            "settings-audio"
                            | "settings-cores"
                            | "settings-cores-200"
                            | "settings-picture"
                            | "settings-picture-200" => SettingsPage::Audio,
                            "settings-plugins" => SettingsPage::Plugins,
                            _ => SettingsPage::General,
                        };
                        if page == SettingsPage::Audio {
                            views::settings_audio::demo_setup(&mut app);
                        }
                        // The cores list open: each count with its engine and model.
                        if w.starts_with("settings-cores") {
                            views::settings_audio::open_cores_list(&mut app);
                        }
                        views::workspace_settings::open(&mut app, page);
                        shoot(
                            &path,
                            &ctx,
                            &mut app,
                            size,
                            |_, _| {},
                            views::workspace_settings::window,
                        );
                    }
                    w if w.starts_with("engines") => shoot(
                        &path,
                        &ctx,
                        &mut app,
                        vec2(1500.0, 620.0),
                        |_, _| {},
                        |_, ctx| engines_gallery(ctx),
                    ),
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
                            views::picker::frame,
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

/// The ten engines' illustrations large, two rows of five, with their engine and model.
fn engines_gallery(ctx: &egui::Context) {
    use cantus_ui::engines::{paint, Engine, Look, ASPECT};
    use cantus_ui::kit::text::{self, Role, Text};
    use cantus_ui::kit::{with_brush, Fill, Style};
    egui::Area::new(egui::Id::new("engines-gallery"))
        .fixed_pos(egui::pos2(40.0, 80.0))
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            let s = Style::current();
            let (rect, _) = ui.allocate_exact_size(vec2(1420.0, 470.0), egui::Sense::hover());
            with_brush(ui.painter(), |b| {
                b.rrect(rect, s.r(6.0), Fill::Solid(s.t.panel_raised));
                let look = Look::of(&s.t, s.t.well);
                for (i, e) in Engine::ALL.iter().enumerate() {
                    let (col, row) = ((i % 5) as f32, (i / 5) as f32);
                    let cell = egui::Rect::from_min_size(
                        rect.min + vec2(20.0 + col * 278.0, 20.0 + row * 220.0),
                        vec2(258.0, 200.0),
                    );
                    let plate = egui::Rect::from_min_size(cell.min, vec2(258.0, 140.0));
                    b.rrect(plate, s.r(4.0), Fill::Solid(s.t.well));
                    let h = 84.0;
                    let car = egui::Rect::from_center_size(plate.center(), vec2(h * ASPECT, h));
                    paint(b, car, *e, look);
                    let words = format!("{} · {}", e.name(), e.model());
                    let label = egui::Rect::from_min_max(
                        egui::pos2(cell.left(), plate.bottom() + 8.0),
                        cell.max,
                    );
                    let tx =
                        Text::new(&words, Role::Body, s.t.text).align(egui::Align2::CENTER_TOP);
                    let _ = text::paint(b, label, &tx, s.tk());
                }
            });
        });
}

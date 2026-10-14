//! Every kind of text field in the app, open, screenshotted without a screen (round 26,
//! third review: the one kit field, `kit::field`, ADR 0024 §15). Each field is shown as a
//! user meets it, with its name or value selected:
//!
//! - `track`, `clip`, `insp-name` (the inspector's clip name, clicked), `pad` (the channel
//!   rack), `strip` (the mixer), `layout` (the layout's name dialog): the whole window,
//!   1280 × 720 pixels;
//! - `knob`: an instrument knob's exact entry (a right-click);
//! - `fx-value`: an effect knob's typed value (a right-click), in its device frame;
//! - `band-hint` (no field): the mixer's band handle hovered while the desk is too short
//!   for the band asked, its hint's words in the hint bar;
//! - `chosen-hint` (no field): the clip browser's paint source pointed at, its words;
//! - `pinned` (no field): the inspector's pinned strip with the Devices pane open;
//! - `draw-hint` (no field): the piano roll's grid pointed at with the Draw tool.
//!
//! Each in Cantus Night and Paper, English and Spanish, at 75, 100 and 200 %.
//!
//! `cargo run --release -p cantus-ui --example shoot_fields -- <out-dir> [filter…]`
//! writes `field-<what>-<theme>-<lang>-<scale>.png`; a filter keeps the names holding it.

use cantus_core::{InstrumentSpec, Project, PulsusParams, Ticks};
use cantus_engine::EngineHandle;
use cantus_ui::i18n::Locale;
use cantus_ui::raster::shoot_events;
use cantus_ui::{starter_project, views, CantusApp};
use egui::{pos2, vec2, Event, Modifiers, PointerButton, Pos2};
use std::path::{Path, PathBuf};

const WHAT: [&str; 12] = [
    "track",
    "clip",
    "insp-name",
    "pad",
    "strip",
    "layout",
    "knob",
    "fx-value",
    "band-hint",
    "chosen-hint",
    "pinned",
    "draw-hint",
];

fn main() {
    let mut args = std::env::args().skip(1);
    let dir = PathBuf::from(args.next().unwrap_or_else(|| "shots".into()));
    let filters: Vec<String> = args.collect();
    std::fs::create_dir_all(&dir).expect("output dir");
    for what in WHAT {
        for theme in ["night", "paper"] {
            for lang in [Locale::En, Locale::Es] {
                for scale in [0.75f32, 1.0, 2.0] {
                    let name = format!(
                        "field-{what}-{theme}-{}-{}",
                        lang.code(),
                        (scale * 100.0) as u32
                    );
                    if !filters.is_empty() && !filters.iter().any(|f| name.contains(f.as_str())) {
                        continue;
                    }
                    cantus_ui::i18n::set_locale(lang);
                    let path = dir.join(format!("{name}.png"));
                    match what {
                        "knob" => knob(&path, theme, scale),
                        "fx-value" => fx_value(&path, theme, scale),
                        _ => window(&path, what, theme, scale),
                    }
                    println!("wrote {}", path.display());
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

/// The app with its saved state kept off the user's files and the picture's theme named
/// in its appearance (the window applies it on its first frame).
fn app_for(project: Project, theme: &str) -> CantusApp {
    use cantus_ui::kit::appearance::Mode;
    let mut app = CantusApp::from_parts(Some(project), EngineHandle::dummy(), false);
    let tmp = std::env::temp_dir().join("cantus-shoot-fields");
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

fn kit_project() -> Project {
    let mut p = Project::new("kit");
    let mut t = p.make_track("Drums");
    t.instrument = InstrumentSpec::Pulsus(PulsusParams::default());
    let clip = p.make_clip("Beat", Ticks::ZERO, Ticks::beats(8));
    t.clips.push(clip);
    p.tracks.push(t);
    p
}

fn click(at: Pos2, button: PointerButton, pressed: bool) -> Event {
    Event::PointerButton {
        pos: at,
        button,
        pressed,
        modifiers: Modifiers::NONE,
    }
}

/// A field in the whole window, opened as the app opens it.
fn window(path: &Path, what: &str, theme: &str, scale: f32) {
    let ctx = context(theme);
    let mut app = app_for(
        if what == "pad" {
            kit_project()
        } else {
            starter_project()
        },
        theme,
    );
    let first = {
        let t = &app.project().tracks[0];
        (t.id, t.clips.first().map(|c| c.id))
    };
    if let (t, Some(c)) = first {
        if matches!(what, "clip" | "insp-name" | "pad") {
            app.select_clip(t, c);
        }
    }
    match what {
        "pad" => app.show_central(cantus_ui::app::CentralView::ChannelRack),
        "strip" => app.central = cantus_ui::app::CentralView::Mixer,
        "pinned" => app.show_pane(views::workspace::Pane::Devices),
        "draw-hint" => {
            if let (t, Some(c)) = first {
                app.select_clip(t, c);
            }
            app.show_central(cantus_ui::app::CentralView::PianoRoll);
        }
        "chosen-hint" => {
            app.show_pane(views::workspace::Pane::Clips);
            if let (t, Some(c)) = first {
                app.clip_browser.set_brush(Some((t, c)));
            }
        }
        "band-hint" => {
            app.central = cantus_ui::app::CentralView::Mixer;
            let mut p = app.mixer.prefs();
            p.band = views::mixer::layout::BAND_MAX;
            app.mixer.set_prefs(p);
        }
        _ => {}
    }
    let size = vec2(1280.0, 720.0) / scale;
    let mut frames = 0;
    let insp = egui::Id::new("insp-clip-name");
    let tile: std::cell::Cell<Option<Pos2>> = std::cell::Cell::new(None);
    shoot_events(
        &ctx,
        size,
        scale,
        14,
        Some(path),
        |frame, ctx| {
            if what == "draw-hint" && frame >= 4 {
                let s = ctx.content_rect().size();
                return vec![Event::PointerMoved(pos2(s.x * 0.45, s.y * 0.8))];
            }
            if what == "chosen-hint" {
                return match tile.get() {
                    Some(p) if frame >= 4 => vec![Event::PointerMoved(p)],
                    _ => Vec::new(),
                };
            }
            if what == "band-hint" {
                let handle = ctx.read_response(egui::Id::new("mixer-band-size"));
                return match handle {
                    Some(r) if frame >= 4 => vec![Event::PointerMoved(r.rect.center())],
                    _ => Vec::new(),
                };
            }
            if what != "insp-name" {
                return Vec::new();
            }
            let Some(at) = ctx.read_response(insp).map(|r| r.rect.center()) else {
                return Vec::new();
            };
            match frame {
                4 => vec![Event::PointerMoved(at)],
                5 => vec![click(at, PointerButton::Primary, true)],
                6 => vec![click(at, PointerButton::Primary, false)],
                7 => vec![Event::PointerGone],
                _ => Vec::new(),
            }
        },
        |root| {
            frames += 1;
            if frames == 3 {
                let (t, c) = first;
                match what {
                    "track" => {
                        let name = app.project().tracks[0].name.clone();
                        app.arrangement.begin_rename_track(t, name);
                    }
                    "clip" if c.is_some() => app.begin_rename_clip(),
                    "pad" => app.rack.begin_rename_pad(t, 6, "High tom".into()),
                    "strip" => {
                        let st = app.project().strips[0].clone();
                        app.mixer.begin_rename(&st);
                    }
                    "layout" => app.naming_layout = Some("Mix A".into()),
                    _ => {}
                }
            }
            app.frame(root);
            if what == "chosen-hint" {
                let (t, c) = first;
                let r = app
                    .clip_browser
                    .tile_rects()
                    .iter()
                    .find(|(k, _)| Some(*k) == c.map(|c| (t, c)));
                tile.set(r.map(|(_, r)| r.center()));
            }
        },
    );
}

/// An instrument knob's exact entry, opened by a right-click on the knob.
fn knob(path: &Path, theme: &str, scale: f32) {
    let ctx = context(theme);
    let size = vec2(260.0, 150.0);
    let mut v = 0.37f32;
    let at = pos2(70.0, 70.0);
    shoot_events(
        &ctx,
        size,
        scale,
        10,
        Some(path),
        |frame, _| match frame {
            2 => vec![Event::PointerMoved(at)],
            3 => vec![click(at, PointerButton::Secondary, true)],
            4 => vec![click(at, PointerButton::Secondary, false)],
            5 => vec![Event::PointerGone],
            _ => Vec::new(),
        },
        |root| {
            let fill = root.visuals().panel_fill;
            root.painter().rect_filled(root.max_rect(), 0.0, fill);
            let r = egui::Rect::from_center_size(at, vec2(64.0, 84.0));
            root.put(
                r,
                cantus_ui::widgets::knob::Knob::new(&mut v, 0.0..=1.0, 0.5)
                    .label("Mix")
                    .unit("%"),
            );
        },
    );
}

/// An effect knob's typed value (Spatium's), opened by a right-click, in its device frame
/// at the window scale.
fn fx_value(path: &Path, theme: &str, scale: f32) {
    let ctx = context(theme);
    let kind = cantus_core::EffectKind::Space;
    let mut app = CantusApp::from_parts(
        Some(cantus_ui::welcome_project()),
        EngineHandle::dummy(),
        false,
    );
    app.demo_editor("space");
    views::fx::demo_feed(&mut app, kind);
    app.fx.pos = Some(pos2(0.0, 0.0));
    views::fx::frame::shell_of(kind).set_scale(&ctx, scale);
    let size = views::fx::frame::designed_size(kind) * scale;
    // A right-click types straight away (the editor alone has no automation menu).
    let at = knob_at() * scale;
    shoot_events(
        &ctx,
        size,
        1.0,
        10,
        Some(path),
        |frame, _| match frame {
            2 => vec![Event::PointerMoved(at)],
            3 => vec![click(at, PointerButton::Secondary, true)],
            4 => vec![click(at, PointerButton::Secondary, false)],
            5 => vec![Event::PointerGone],
            _ => Vec::new(),
        },
        |root| {
            let ctx = root.ctx().clone();
            views::fx::window(&mut app, &ctx);
        },
    );
}

/// Where Spatium's pre-delay knob sits in its frame at 100 % (`CANTUS_KNOB=x,y` moves it).
fn knob_at() -> Pos2 {
    std::env::var("CANTUS_KNOB")
        .ok()
        .and_then(|s| {
            let (x, y) = s.split_once(',')?;
            Some(pos2(x.parse().ok()?, y.parse().ok()?))
        })
        .unwrap_or(pos2(516.0, 135.0))
}

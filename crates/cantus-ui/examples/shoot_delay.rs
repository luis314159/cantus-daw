//! Delay in the inspector (round 39, ADR 0115). Delay has had its own window since round 39
//! — its pictures on the site and in the README are `shoot_skins`' now, like every other
//! unit's. It also stays in the inspector, as the other windowed effects do: a click on its
//! insert opens the window, and the mini view under it holds the quick controls (the time,
//! the feedback and the tone, the dry/wet). This draws the application on a track with Delay
//! on its *Dotted eighth* preset and keeps the inspector's part from the inserts' band to
//! the mini view's last row (`delay-inspector-{slate,paper}-{100,200}.png`), and the whole
//! window with Delay's open over it (`delay-app-{slate,paper}.png`).
//!
//! `cargo run --release -p cantus-ui --example shoot_delay -- <dir> [--es]`

use cantus_core::{EffectKind, InstrumentKind, InstrumentSpec, NativeSpec};
use cantus_engine::EngineHandle;
use cantus_ui::app::CantusApp;
use cantus_ui::i18n::{self, Locale};
use cantus_ui::preset_audition as aud;
use cantus_ui::preset_tags::Role;
use cantus_ui::raster;
use std::path::{Path, PathBuf};

const SIZE: egui::Vec2 = egui::vec2(1600.0, 1000.0);
/// The inspector's width for the picture: a card is about 1.6 times as wide as tall.
const INSPECTOR: f32 = 360.0;
/// From the insert's tile up to above the inserts' band: the band's title.
const TOP: f32 = 32.0;
/// The preset the picture shows.
const PRESET: &str = "delay.preset.dotted_eighth";

fn app(paper: bool) -> (CantusApp, cantus_core::StripId, cantus_core::EffectId) {
    let mut p = aud::phrase_project(
        InstrumentSpec::Native(NativeSpec::new(InstrumentKind::ElectricPiano)),
        Role::Keys,
    );
    let strip = p.tracks[0].strip.expect("a strip");
    let mut slot = p.make_effect(EffectKind::Delay);
    let values = cantus_ui::views::fx::factory_presets(EffectKind::Delay)
        .iter()
        .find(|(k, _)| *k == PRESET)
        .map(|(_, v)| *v)
        .expect("the preset");
    for (k, v) in values {
        slot.spec.set(k, *v);
    }
    let effect = slot.id;
    p.strip_mut(strip).unwrap().effects.push(slot);
    let tmp = std::env::temp_dir().join("cantus-shoot-delay");
    let mut app = CantusApp::from_parts(Some(p), EngineHandle::dummy(), false);
    app.settings_path = tmp.join("settings.json");
    app.layouts_path = tmp.join("layouts.json");
    app.keymap_path = tmp.join("keymap.toml");
    app.appearance_path = tmp.join("appearance.json");
    app.appearance.mode = if paper {
        cantus_ui::kit::appearance::Mode::Light
    } else {
        cantus_ui::kit::appearance::Mode::Dark
    };
    (app, strip, effect)
}

/// The rectangle a widget took last frame, in points.
fn rect_of(ctx: &egui::Context, id: egui::Id) -> Option<egui::Rect> {
    ctx.read_response(id).map(|r| r.rect)
}

/// `canvas`'s pixels inside `r` (points × `ppp`), as a PNG.
fn crop(canvas: &raster::Canvas, r: egui::Rect, ppp: f32, path: &Path) {
    let x0 = (r.left() * ppp).floor().max(0.0) as usize;
    let y0 = (r.top() * ppp).floor().max(0.0) as usize;
    let x1 = ((r.right() * ppp).ceil() as usize).min(canvas.width);
    let y1 = ((r.bottom() * ppp).ceil() as usize).min(canvas.height);
    let (w, h) = (x1 - x0, y1 - y0);
    let mut pix = tiny_skia::Pixmap::new(w as u32, h as u32).expect("a picture");
    for (i, d) in pix.pixels_mut().iter_mut().enumerate() {
        let [r, g, b, a] = canvas.pixel(x0 + i % w, y0 + i / w);
        *d = tiny_skia::ColorU8::from_rgba(r, g, b, a).premultiply();
    }
    std::fs::write(path, pix.encode_png().expect("encoded")).expect("written");
}

fn shoot(dir: &Path, paper: bool, ppp: f32) {
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    let (mut app, strip, effect) = app(paper);
    app.attach(&ctx);
    app.select_strip(strip);
    app.inspector.open_insert_view(effect);
    let mut frame = 0;
    let canvas = raster::shoot(&ctx, SIZE, ppp, 10, None, |root| {
        if frame == 2 {
            app.workspace.impose_sizes(INSPECTOR, 0.0);
        }
        frame += 1;
        app.frame(root);
    });
    let tile = rect_of(&ctx, egui::Id::new(("insp-tile", effect))).expect("Delay's insert");
    let mix = rect_of(&ctx, egui::Id::new(("insp-fx-mix", effect))).expect("its mix row");
    let area = egui::Rect::from_min_max(
        egui::pos2(tile.left() - 10.0, tile.top() - TOP),
        egui::pos2(tile.right() + 10.0, mix.bottom() + 40.0),
    );
    let theme = if paper { "paper" } else { "slate" };
    let path = dir.join(format!(
        "delay-inspector-{theme}-{}.png",
        (ppp * 100.0) as u32
    ));
    crop(&canvas, area, ppp, &path);
    println!(
        "{} ({:.0} x {:.0} pt)",
        path.display(),
        area.width(),
        area.height()
    );
}

/// The whole window with Delay's open over it, opened as a click on the insert opens it.
fn shoot_app(dir: &Path, paper: bool) {
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    let (mut app, strip, effect) = app(paper);
    app.attach(&ctx);
    app.select_strip(strip);
    app.open_effect_editor(strip, effect);
    // Left of the inspector, so the insert it opened from shows beside it.
    app.fx.pos = Some(egui::pos2(200.0, 96.0));
    cantus_ui::views::fx::demo_feed(&mut app, EffectKind::Delay);
    let theme = if paper { "paper" } else { "slate" };
    let path = dir.join(format!("delay-app-{theme}.png"));
    let mut frame = 0;
    raster::shoot(&ctx, SIZE, 1.0, 10, Some(&path), |root| {
        if frame == 2 {
            app.workspace.impose_sizes(INSPECTOR, 0.0);
        }
        frame += 1;
        app.frame(root);
    });
    println!("{}", path.display());
}

fn main() {
    let mut args = std::env::args().skip(1);
    let dir = PathBuf::from(args.next().unwrap_or_else(|| "target/shots/delay".into()));
    let es = std::env::args().any(|a| a == "--es");
    std::fs::create_dir_all(&dir).expect("the folder");
    i18n::set_locale(if es { Locale::Es } else { Locale::En });
    for paper in [false, true] {
        for ppp in [1.0, 2.0] {
            shoot(&dir, paper, ppp);
        }
        shoot_app(&dir, paper);
    }
}

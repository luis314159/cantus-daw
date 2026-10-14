//! The views where a character once had no glyph (round 26, `tests/font_coverage.rs`):
//! Modulus's graph with a back edge (and its hint, hovered), Organum's rotor ramp (Edit, slowing
//! down and spinning up), Terminus's and Culmen's delta
//! listen key, each in its device window, maximised, in Cantus Night, at 75 % and 200 %, in
//! English and Spanish.
//!
//! `cargo run -p cantus-ui --example shoot_glyphs [-- out_dir [case…]]`
//!
//! Writes `glyphs-<case>-<lang>-<zoom>.png` into `out_dir` (default `target/shoot-glyphs`).

use cantus_core::inst::fm::matrix_key;
use cantus_core::{EffectKind, InstrumentKind, InstrumentSpec, NativeSpec};
use cantus_engine::EngineHandle;
use cantus_ui::app::CantusApp;
use cantus_ui::i18n::{self, Locale};
use cantus_ui::views::workspace_device as device;
use cantus_ui::{raster, welcome_project};
use std::path::{Path, PathBuf};

const SIZE: egui::Vec2 = egui::vec2(1600.0, 1000.0);

/// Opens the case's device; returns its window key once the window exists.
type Setup = fn(&mut CantusApp);
type Key = fn(&CantusApp) -> u64;

fn instrument(a: &mut CantusApp, spec: NativeSpec) {
    let t = a.project().tracks[0].id;
    a.set_instrument(t, InstrumentSpec::Native(spec));
    a.select_track(t);
    a.open_inst_editor(t);
}
fn fm(a: &mut CantusApp) {
    let mut spec = NativeSpec::new(InstrumentKind::Fm);
    // A back edge: operator 1 read by the earlier operator 2's modulator chain.
    spec.set(matrix_key(0, 3), 1.25);
    instrument(a, spec);
}
fn organ(a: &mut CantusApp) {
    instrument(a, NativeSpec::new(InstrumentKind::Organ));
    // The rotor ramp is in Edit's rotary-speaker card.
    a.inst.view = cantus_ui::views::inst::View::Edit;
}
fn organ_fast(a: &mut CantusApp) {
    let mut spec = NativeSpec::new(InstrumentKind::Organ);
    // Spinning up: the curves rise, and the key moves to the bottom corner.
    spec.set("rot.speed", 2.0);
    instrument(a, spec);
    a.inst.view = cantus_ui::views::inst::View::Edit;
}
fn effect(a: &mut CantusApp, kind: EffectKind) {
    let s = a.project().tracks[0].strip.unwrap();
    a.add_effect(s, kind);
    let fx = a.project().strip(s).unwrap().effects.last().unwrap().id;
    a.open_effect_editor(s, fx);
}
fn limiter(a: &mut CantusApp) {
    effect(a, EffectKind::Limiter);
}
fn maximizer(a: &mut CantusApp) {
    effect(a, EffectKind::Maximizer);
}
fn inst_key(a: &CantusApp) -> u64 {
    a.instrument_window_key(a.project().tracks[0].id).unwrap()
}
fn fx_key(a: &CantusApp) -> u64 {
    let s = a.project().tracks[0].strip.unwrap();
    let fx = a.project().strip(s).unwrap().effects.last().unwrap().id;
    cantus_ui::views::fx::device_id(fx).value()
}

/// The window, in points: tall enough for Organum's Edit page to reach its rotary card.
const TALL: egui::Vec2 = egui::vec2(1600.0, 1500.0);

/// Each case: its setup, its window, the window size, and where the pointer rests (the
/// back edge's number in `fm-hover`, so the hint line says what a dashed line is).
type Case = (&'static str, Setup, Key, egui::Vec2, Option<egui::Pos2>);

fn cases() -> [Case; 6] {
    [
        ("fm", fm as Setup, inst_key as Key, SIZE, None),
        (
            "fm-hover",
            fm,
            inst_key,
            SIZE,
            Some(egui::pos2(298.0, 380.0)),
        ),
        ("organ", organ, inst_key, TALL, None),
        ("organ-fast", organ_fast, inst_key, TALL, None),
        ("limiter", limiter, fx_key, SIZE, None),
        ("maximizer", maximizer, fx_key, SIZE, None),
    ]
}

fn shoot(dir: &Path, case: &Case, lang: &str, zoom: f32) {
    let (name, setup, key, size, pointer) = *case;
    i18n::set_locale(if lang == "es" { Locale::Es } else { Locale::En });
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    let tmp = std::env::temp_dir().join("cantus-shoot-glyphs");
    let mut app = CantusApp::from_parts(Some(welcome_project()), EngineHandle::dummy(), false);
    app.settings_path = tmp.join("settings.json");
    app.layouts_path = tmp.join("layouts.json");
    app.keymap_path = tmp.join("keymap.toml");
    app.appearance_path = tmp.join("appearance.json");
    app.attach(&ctx);
    setup(&mut app);
    let path = dir.join(format!(
        "glyphs-{name}-{lang}-{}.png",
        (zoom * 100.0) as u32
    ));
    let mut frame = 0;
    // The same window in points at both zooms: 200 % is a high-density screen, not a
    // smaller window (a 1600 × 1000-point window is 3200 × 2000 pixels there).
    raster::shoot_at(
        &ctx,
        size,
        zoom,
        16,
        Some(&path),
        |i| if i >= 10 { pointer } else { None },
        |root| {
            if frame == 6 {
                device::toggle_maximise(root.ctx(), key(&app));
            }
            app.frame(root);
            frame += 1;
        },
    );
    println!("{}", path.display());
}

fn main() {
    let mut args = std::env::args().skip(1);
    let dir = args.next().map(PathBuf::from).unwrap_or_else(|| {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/shoot-glyphs")
    });
    let only: Vec<String> = args.collect();
    std::fs::create_dir_all(&dir).expect("create the output directory");
    for case in cases() {
        if !only.is_empty() && !only.iter().any(|o| o == case.0) {
            continue;
        }
        for lang in ["en", "es"] {
            for zoom in [0.75f32, 2.0] {
                shoot(&dir, &case, lang, zoom);
            }
        }
    }
}

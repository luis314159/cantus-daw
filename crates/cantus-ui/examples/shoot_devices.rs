//! Round 20, device-ux: the device windows' own buttons, the device tray and the swap
//! affordances, rendered headless for the design review.
//!
//! `cargo run -p cantus-ui --example shoot_devices [-- out_dir [case…]]`
//!
//! Writes `devices-<case>-<theme>.png` (1600 × 1000) into `out_dir` (default
//! `target/shoot-devices`): an instrument docked with its buttons, the close key under the
//! pointer, an effect floating, both minimised to the tray, an instrument maximised, the
//! track header's instrument chip, the mixer's and the rack's swap badges.

use cantus_core::{EffectKind, InstrumentKind, InstrumentSpec, NativeSpec};
use cantus_engine::EngineHandle;
use cantus_ui::app::{CantusApp, CentralView};
use cantus_ui::kit::appearance::Mode;
use cantus_ui::views::device_chrome::{self as chrome, Spot};
use cantus_ui::views::workspace_device as device;
use cantus_ui::{raster, welcome_project};
use egui::Pos2;
use std::path::{Path, PathBuf};

const SIZE: egui::Vec2 = egui::vec2(1600.0, 1000.0);

/// Where the pointer rests in a case, found from what the first frames published.
type Aim = fn(&CantusApp, &egui::Context) -> Option<Pos2>;

struct Case {
    name: &'static str,
    setup: fn(&mut CantusApp),
    /// Run after the first frames (a click that needs the window drawn).
    then: fn(&mut CantusApp, &egui::Context),
    aim: Aim,
}

fn calor(a: &mut CantusApp) {
    let t = a.project().tracks[0].id;
    a.set_instrument(t, InstrumentSpec::Native(NativeSpec::new(InstrumentKind::Analog)));
    a.select_track(t);
    a.open_inst_editor(t);
}
fn gate(a: &mut CantusApp) {
    let s = a.project().tracks[0].strip.unwrap();
    a.add_effect(s, EffectKind::Gate);
    let fx = a.project().strip(s).unwrap().effects[0].id;
    a.open_effect_editor(s, fx);
}
fn both(a: &mut CantusApp) {
    calor(a);
    gate(a);
}
fn mixer_xl(a: &mut CantusApp) {
    let s = a.project().tracks[0].strip.unwrap();
    a.add_effect(s, EffectKind::Eq);
    a.add_effect(s, EffectKind::Compressor);
    cantus_ui::views::mixer::set_width_mode(
        a,
        cantus_ui::views::mixer::layout::WidthMode::Xl,
    );
    a.show_central(CentralView::Mixer);
}
fn rack(a: &mut CantusApp) {
    a.show_central(CentralView::ChannelRack);
}
fn nothing(_: &mut CantusApp) {}
fn no_then(_: &mut CantusApp, _: &egui::Context) {}

fn inst_key(a: &CantusApp) -> u64 {
    a.instrument_window_key(a.project().tracks[0].id).unwrap()
}
fn fx_key(a: &CantusApp) -> u64 {
    let s = a.project().tracks[0].strip.unwrap();
    let fx = a.project().strip(s).unwrap().effects[0].id;
    cantus_ui::views::fx::device_id(fx).value()
}
fn minimise_both(a: &mut CantusApp, ctx: &egui::Context) {
    device::minimise(ctx, inst_key(a));
    device::minimise(ctx, fx_key(a));
}
fn maximise(a: &mut CantusApp, ctx: &egui::Context) {
    device::toggle_maximise(ctx, inst_key(a));
}

fn no_aim(_: &CantusApp, _: &egui::Context) -> Option<Pos2> {
    None
}
fn at_close(a: &CantusApp, ctx: &egui::Context) -> Option<Pos2> {
    device::buttons(ctx, inst_key(a))?.close.map(|r| r.center())
}
fn at_swap(a: &CantusApp, ctx: &egui::Context) -> Option<Pos2> {
    device::buttons(ctx, fx_key(a))?.swap.map(|r| r.center())
}
fn at_chip(a: &CantusApp, ctx: &egui::Context) -> Option<Pos2> {
    chrome::spot(ctx, Spot::Chip(a.project().tracks[1].id)).map(|r| r.center())
}
fn at_slot(a: &CantusApp, ctx: &egui::Context) -> Option<Pos2> {
    use cantus_ui::views::mixer::{self, Control};
    let s = a.project().tracks[0].strip?;
    mixer::control_rect(ctx, Some(s), Control::Slot(1)).map(|r| chrome::badge_rect(r, 1.0).center())
}
fn at_rack(a: &CantusApp, ctx: &egui::Context) -> Option<Pos2> {
    chrome::spot(ctx, Spot::RackButton(a.project().tracks[1].id))
        .map(|r| Pos2::new(r.left() + 14.0, r.center().y))
}

fn cases() -> Vec<Case> {
    let c = |name, setup, then, aim| Case { name, setup, then, aim };
    vec![
        c("inst", calor as fn(&mut CantusApp), no_then as fn(&mut CantusApp, &egui::Context), no_aim as Aim),
        c("inst-close", calor, no_then, at_close),
        c("fx-swap", gate, no_then, at_swap),
        c("tray", both, minimise_both, no_aim),
        c("max", calor, maximise, no_aim),
        c("chip", nothing, no_then, at_chip),
        c("slot", mixer_xl, no_then, at_slot),
        c("rack", rack, no_then, at_rack),
    ]
}

fn shoot(dir: &Path, case: &Case, dark: bool) {
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    let tmp = std::env::temp_dir().join("cantus-shoot-devices");
    let mut app = CantusApp::from_parts(Some(welcome_project()), EngineHandle::dummy(), false);
    app.settings_path = tmp.join("settings.json");
    app.layouts_path = tmp.join("layouts.json");
    app.keymap_path = tmp.join("keymap.toml");
    app.appearance_path = tmp.join("appearance.json");
    app.appearance.mode = if dark { Mode::Dark } else { Mode::Light };
    app.attach(&ctx);
    (case.setup)(&mut app);
    raster::shoot(&ctx, SIZE, 1.0, 6, None, |root| app.frame(root));
    (case.then)(&mut app, &ctx);
    raster::shoot(&ctx, SIZE, 1.0, 4, None, |root| app.frame(root));
    let aim = (case.aim)(&app, &ctx);
    let theme = if dark { "slate" } else { "paper" };
    let path = dir.join(format!("devices-{}-{theme}.png", case.name));
    raster::shoot_at(&ctx, SIZE, 1.0, 4, Some(&path), aim, |root| app.frame(root));
    println!("{}", path.display());
}

fn main() {
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
    let mut args = std::env::args().skip(1);
    let dir = args.next().map(PathBuf::from).unwrap_or_else(|| {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/shoot-devices")
    });
    let only: Vec<String> = args.collect();
    std::fs::create_dir_all(&dir).expect("create the output directory");
    for case in cases() {
        if only.is_empty() || only.iter().any(|o| case.name.starts_with(o.as_str())) {
            for dark in [true, false] {
                shoot(&dir, &case, dark);
            }
        }
    }
}

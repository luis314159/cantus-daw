//! r42:arcus-polish2 — the figures' detail on a slow machine (the owner, 2026-11-25: "luego
//! optimizamos o hacemos configurable"; High costs a few ms, so the fast path must hold):
//! Arcus's Play view with a realistic violinist drawn through the app, the live arms
//! recorded as drawn (`views::inst::arcus::probe`). **Light** draws the coarse arms; **Auto**
//! draws High's while the window's frames are short and Light's once they run long (a slow
//! machine: their average over 12 ms), and High's again once they are short (under 7 ms).

use cantus_core::inst::InstrumentKind;
use cantus_engine::EngineHandle;
use cantus_ui::kit::shell::Shell;
use cantus_ui::scene::bowed_hold::{Hold, Holder};
use cantus_ui::scene::figure::Quality;
use cantus_ui::views::inst::arcus::probe;
use cantus_ui::{views, welcome_project, CantusApp};
use egui::{Pos2, Rect};

fn app(q: Quality) -> (CantusApp, egui::Context) {
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    cantus_ui::theme::apply(&cantus_ui::theme::default_theme(), &ctx);
    let mut app = CantusApp::from_parts(Some(welcome_project()), EngineHandle::dummy(), false);
    app.settings.arcus_holder = Holder::of(Hold::Woman);
    app.settings.figure_quality = q;
    app.demo_editor("arcus");
    let t = app.inst.open.expect("an Arcus editor");
    app.set_inst_value(t, "inst", 2.0, false);
    views::inst::arcus::demo_feed(&mut app);
    Shell::new(("inst", t.0)).set_scale(&ctx, 1.0);
    (app, ctx)
}

/// The bow arm's triangles as the last of `n` frames drew it.
fn drawn(app: &mut CantusApp, ctx: &egui::Context, n: usize) -> usize {
    let size = views::inst::editor_size(InstrumentKind::BowedString);
    probe::record(true);
    for i in 0..n {
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
            time: Some(i as f64 / 60.0),
            predicted_dt: 1.0 / 60.0,
            ..Default::default()
        };
        // (its textures handed back: a debug build's epaint asserts on a dropped delta)
        ctx.run_ui(input, |ui| views::inst::window_in(app, ui))
            .textures_delta
            .clear();
    }
    let arms = probe::arms();
    probe::record(false);
    arms.iter()
        .find(|a| a.side == 1)
        .map_or(0, |a| a.tris.len())
}

#[test]
fn auto_draws_the_light_arms_while_the_frames_run_long() {
    views::inst::chorda::scene::hybrid::register();
    cantus_ui::scene::set_blocking(true);
    let (mut a, ctx) = app(Quality::High);
    let high = drawn(&mut a, &ctx, 4);
    let (mut a, ctx) = app(Quality::Light);
    let light = drawn(&mut a, &ctx, 4);
    assert!(
        light > 0 && light * 2 < high,
        "Light draws a coarse arm: {light} triangles against High's {high}"
    );
    let (mut a, ctx) = app(Quality::Auto);
    a.set_frame_ms_for_tests(4.0);
    assert_eq!(drawn(&mut a, &ctx, 4), high, "Auto on a fast machine: High");
    a.set_frame_ms_for_tests(20.0);
    assert_eq!(
        drawn(&mut a, &ctx, 4),
        light,
        "Auto with frames of 20 ms: Light"
    );
    // (Between the two thresholds it keeps what it was drawing.)
    a.set_frame_ms_for_tests(9.0);
    assert_eq!(
        drawn(&mut a, &ctx, 4),
        light,
        "Auto at 9 ms after long frames: Light"
    );
    a.set_frame_ms_for_tests(5.0);
    assert_eq!(
        drawn(&mut a, &ctx, 4),
        high,
        "Auto once the frames are short: High"
    );
}

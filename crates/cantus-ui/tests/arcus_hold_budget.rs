//! r42:arcus-upper — the figure's live arms in the frame budget: Arcus's Play view with a
//! musician or the mannequin holding the violin or the cello (both arms, both hands, the
//! neck and the body stamped back, every frame) against the same view with nothing holding
//! it (the bow and the finger alone), timed beside it, at 100 % and 200 %.
//!
//! `cargo test --release -p cantus-ui --test arcus_hold_budget -- --ignored --nocapture`:
//! build + tessellate over 200 frames, p50 and p95, the reference timed beside each (best of
//! three rounds); the arms may cost at most 1 ms more than the instrument alone at the
//! median, and the slow frames (p95) at most half again plus 1 ms.

use cantus_core::inst::InstrumentKind;
use cantus_engine::EngineHandle;
use cantus_ui::kit::shell::Shell;
use cantus_ui::scene::bowed_hold::{Hold, Holder};
use cantus_ui::{views, welcome_project, CantusApp};
use egui::{Pos2, Rect};
use std::time::Instant;

fn rig(inst: f32, hold: Hold, k: f32) -> (CantusApp, egui::Context) {
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    cantus_ui::theme::apply(&cantus_ui::theme::default_theme(), &ctx);
    let mut app = CantusApp::from_parts(Some(welcome_project()), EngineHandle::dummy(), false);
    app.settings.arcus_holder = Holder::of(hold);
    app.demo_editor("arcus");
    let t = app.inst.open.expect("an Arcus editor");
    app.set_inst_value(t, "inst", inst, false);
    views::inst::arcus::demo_feed(&mut app);
    Shell::new(("inst", t.0)).set_scale(&ctx, k);
    (app, ctx)
}

/// One frame: build and tessellate, in ms.
fn frame(app: &mut CantusApp, ctx: &egui::Context, size: egui::Vec2, i: usize) -> f64 {
    let input = egui::RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
        time: Some(i as f64 / 60.0),
        predicted_dt: 1.0 / 60.0,
        ..Default::default()
    };
    let t0 = Instant::now();
    let mut out = ctx.run_ui(input, |ui| views::inst::window_in(app, ui));
    out.textures_delta.clear();
    let prims = ctx.tessellate(out.shapes, out.pixels_per_point);
    std::hint::black_box(prims);
    t0.elapsed().as_secs_f64() * 1e3
}

/// p50 and p95 of the Play view's frame.
fn measure(inst: f32, hold: Hold, k: f32) -> (f64, f64) {
    let (mut app, ctx) = rig(inst, hold, k);
    let size = views::inst::editor_size(InstrumentKind::BowedString) * k;
    for i in 0..30 {
        let _ = frame(&mut app, &ctx, size, i);
    }
    let mut ms: Vec<f64> = (30..230).map(|i| frame(&mut app, &ctx, size, i)).collect();
    ms.sort_by(f64::total_cmp);
    (ms[ms.len() / 2], ms[ms.len() * 95 / 100])
}

#[test]
#[ignore = "timing: run in release with --ignored"]
fn the_figures_live_arms_draw_in_the_frame_budget() {
    views::inst::chorda::scene::hybrid::register();
    cantus_ui::scene::set_blocking(true);
    let mut bad = Vec::new();
    for (name, inst) in [("violin", 2.0f32), ("cello", 0.0)] {
        for k in [1.0f32, 2.0] {
            for hold in [Hold::Player, Hold::Mannequin] {
                // The reference beside it, then the figure, best of three rounds each.
                let mut best: Option<(f64, f64, f64, f64)> = None;
                for _ in 0..3 {
                    let (r50, r95) = measure(inst, Hold::Air, k);
                    let (f50, f95) = measure(inst, hold, k);
                    if best.is_none_or(|b| f50 - r50 < b.2 - b.0) {
                        best = Some((r50, r95, f50, f95));
                    }
                }
                let (r50, r95, f50, f95) = best.expect("three rounds");
                println!(
                    "{name} {hold:?} {}%: alone p50 {r50:.2} p95 {r95:.2} ms; held p50 {f50:.2} \
                     p95 {f95:.2} ms; the arms +{:.2} ms",
                    (k * 100.0) as u32,
                    f50 - r50
                );
                if f50 - r50 > 1.0 || f95 > r95 * 1.5 + 1.0 {
                    bad.push(format!(
                        "{name} {hold:?} {k}: +{:.2} ms, p95 {f95:.2}",
                        f50 - r50
                    ));
                }
            }
        }
    }
    assert!(bad.is_empty(), "over budget: {bad:?}");
}

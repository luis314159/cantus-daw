//! The modulation family's faces in the frame budget (round 32, ADR 0074, ADR 0024 §13):
//! each of the eight editors — its material, its display's look with the glowing traces,
//! its marks (beads, flares, notches, rings, arrows) — against the same editor in the plain
//! look, at 100 % and 200 %.
//!
//! `cargo test --release -p cantus-ui --test mod_faces_budget -- --ignored --nocapture`:
//! build + tessellate over 200 frames, p50 and p95; a face may cost at most 1 ms more than
//! the plain look at the median, and a frame stays inside twice the window budget of 4 ms
//! (as the mixer's timings are).

use cantus_engine::EngineHandle;
use cantus_ui::kit::skin;
use cantus_ui::{views, welcome_project, CantusApp};
use egui::{Pos2, Rect};
use std::time::Instant;

const UNITS: [(&str, cantus_core::EffectKind); 8] = {
    use cantus_core::EffectKind as K;
    [
        ("concentus", K::Chorus),
        ("flexus", K::Flanger),
        ("phasis", K::Phaser),
        ("tremor", K::Tremolo),
        ("vibrans", K::Vibrato),
        ("anulus", K::RingMod),
        ("translatio", K::FreqShift),
        ("fluctus", K::VolumeShaper),
    ]
};

fn rig(stem: &str, kind: cantus_core::EffectKind, k: f32) -> (CantusApp, egui::Context) {
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    cantus_ui::theme::apply(&cantus_ui::theme::slate(), &ctx);
    let mut app = CantusApp::from_parts(Some(welcome_project()), EngineHandle::dummy(), false);
    app.demo_editor(stem);
    views::fx::demo_feed(&mut app, kind);
    app.fx.pos = Some(Pos2::ZERO);
    views::fx::frame::shell_of(kind).set_scale(&ctx, k);
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
    let mut out = ctx.run_ui(input, |ui| {
        let ctx = ui.ctx().clone();
        views::fx::window(app, &ctx);
    });
    out.textures_delta.clear();
    let prims = ctx.tessellate(out.shapes, out.pixels_per_point);
    std::hint::black_box(prims);
    t0.elapsed().as_secs_f64() * 1e3
}

/// p50 and p95 of one editor's frame in the plain look (`faces` off) or its own face.
fn measure(stem: &str, kind: cantus_core::EffectKind, k: f32, faces: bool) -> (f64, f64) {
    skin::set_enabled(faces);
    let (mut app, ctx) = rig(stem, kind, k);
    let size = views::fx::frame::designed_size(kind) * k;
    for i in 0..30 {
        let _ = frame(&mut app, &ctx, size, i);
    }
    let mut ms: Vec<f64> = (30..230).map(|i| frame(&mut app, &ctx, size, i)).collect();
    skin::set_enabled(true);
    ms.sort_by(f64::total_cmp);
    (ms[ms.len() / 2], ms[ms.len() * 95 / 100])
}

#[test]
#[ignore = "timing: run in release with --ignored"]
fn every_modulation_face_draws_in_the_frame_budget() {
    let mut bad = Vec::new();
    for (stem, kind) in UNITS {
        for k in [1.0f32, 2.0] {
            let (plain50, plain95) = measure(stem, kind, k, false);
            let (face50, face95) = measure(stem, kind, k, true);
            eprintln!(
                "{stem} at {:.0} %: plain p50 {plain50:.2} / p95 {plain95:.2} ms, \
                 face p50 {face50:.2} / p95 {face95:.2} ms",
                k * 100.0
            );
            if face50 > 8.0 || face50 > plain50 + 1.0 {
                bad.push(format!("{stem} at {k}: face p50 {face50:.2} ms"));
            }
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

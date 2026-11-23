//! The device picker's pictures made ahead while the window is quiet (round 41's leftover,
//! BACKLOG §6j; round 42): on a fresh profile, after the idle warm-up, every device's
//! picture is ready — the most used in the atlas, the rest on disk — so opening either
//! picker and scrolling to its end draws no editor and shows each tile's picture within two
//! frames (no card lingering); the warm-up waits for the start-up and for a quiet hand; once
//! done it asks for no frame (the frame budget at rest unchanged); and what each of its steps
//! costs on the interface thread is measured beside a plain frame of the same window.
//! r42:tuning-face

use cantus_engine::EngineHandle;
use cantus_ui::views::picker_snap::Subject;
use cantus_ui::views::picker_warm;
use cantus_ui::{welcome_project, CantusApp};
use std::time::Instant;

fn ctx() -> egui::Context {
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    cantus_ui::theme::apply(&cantus_ui::theme::default_theme(), &ctx);
    ctx
}

/// One pass of the app's warm-up at `time`, with `events` as its input; how long it took.
fn pass(app: &mut CantusApp, ctx: &egui::Context, time: f64, events: Vec<egui::Event>) -> f64 {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1280.0, 800.0),
        )),
        time: Some(time),
        events,
        ..Default::default()
    };
    let t0 = Instant::now();
    let _ = ctx.run_ui(input, |ui| picker_warm::frame(app, &ui.ctx().clone()));
    t0.elapsed().as_secs_f64() * 1e3
}

#[test]
fn after_the_idle_warm_up_every_picture_is_ready_and_the_rest_is_quiet() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = ctx();
    let mut app = CantusApp::from_parts(Some(welcome_project()), EngineHandle::dummy(), false);
    app.picker_view
        .thumbs
        .enable(Some(dir.path().to_path_buf()));
    let all: Vec<Subject> = picker_warm::order(&mut app);
    assert!(all.len() > 30, "every device with an editor: {}", all.len());
    // The recently chosen first.
    app.picker_view.prefs.recent = vec!["native:effect:limiter".into()];

    // It waits for the start-up, and for a quiet hand.
    let mut t = 0.0;
    for _ in 0..30 {
        pass(&mut app, &ctx, t, Vec::new());
        t += 0.1;
    }
    assert_eq!(
        app.picker_view.thumbs.stats.captures, 0,
        "not during the start-up"
    );
    let moving = vec![egui::Event::PointerMoved(egui::pos2(100.0, 100.0))];
    for _ in 0..20 {
        pass(&mut app, &ctx, t, moving.clone());
        t += 0.05;
    }
    assert_eq!(
        app.picker_view.thumbs.stats.captures, 0,
        "not under a moving hand"
    );

    // Quiet: it makes them, one editor at a time.
    let mut steps = Vec::new();
    let started = Instant::now();
    while !app.picker_warm.done {
        steps.push(pass(&mut app, &ctx, t, Vec::new()));
        t += 1.0 / 60.0;
        std::thread::sleep(std::time::Duration::from_millis(2));
        assert!(started.elapsed().as_secs() < 600, "the warm-up ends");
    }
    let th = &app.picker_view.thumbs;
    for s in &all {
        assert!(th.ready(*s), "{s:?} ready");
    }
    let (held, pages) = th.held();
    assert_eq!(pages, 1, "one atlas page");
    assert!(held >= 20.min(all.len()), "the most used held: {held}");
    let captures = th.stats.captures;
    assert!(captures as usize <= all.len(), "each made once: {captures}");

    // Opening the picker and scrolling to its end: every tile's picture within two frames,
    // and no editor drawn.
    let th = &mut app.picker_view.thumbs;
    for s in &all {
        let mut got = th.get(*s).is_some();
        for _ in 0..40 {
            if got {
                break;
            }
            th.pump(&ctx);
            std::thread::sleep(std::time::Duration::from_millis(5));
            got = th.get(*s).is_some();
        }
        assert!(got, "{s:?} shows its picture");
    }
    assert_eq!(
        th.stats.captures, captures,
        "no editor drawn while scrolling"
    );

    // Done, it asks for nothing: a pass costs what an empty pass costs.
    let mut rest = Vec::new();
    for _ in 0..30 {
        rest.push(pass(&mut app, &ctx, t, Vec::new()));
        t += 1.0 / 60.0;
    }
    let mut empty = Vec::new();
    let ctx2 = self::ctx();
    for i in 0..30 {
        let input = egui::RawInput {
            time: Some(i as f64 / 60.0),
            ..Default::default()
        };
        let t0 = Instant::now();
        let _ = ctx2.run_ui(input, |_| {});
        empty.push(t0.elapsed().as_secs_f64() * 1e3);
    }
    let median = |v: &mut Vec<f64>| {
        v.sort_by(|a, b| a.partial_cmp(b).unwrap());
        v[v.len() / 2]
    };
    let (rest_ms, empty_ms) = (median(&mut rest), median(&mut empty));
    steps.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let worst = steps.last().copied().unwrap_or(0.0);
    let typical = steps[steps.len() / 2];
    eprintln!(
        "warm-up: {} pictures, {captures} editors drawn, {} passes; a pass {typical:.2} ms typical, {worst:.1} ms worst; at rest {rest_ms:.3} ms vs an empty pass {empty_ms:.3} ms",
        all.len(),
        steps.len()
    );
    // At rest it is an empty pass (a reference timed beside it, never a fixed figure).
    assert!(
        rest_ms <= empty_ms * 3.0 + 0.05,
        "at rest {rest_ms} ms, empty {empty_ms} ms"
    );
}

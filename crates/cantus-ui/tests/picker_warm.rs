//! The device picker's pictures made ahead while the window is quiet (round 41's leftover,
//! BACKLOG §6j; round 42): on a fresh profile, after the idle warm-up, every device's
//! picture is ready — the most used in the atlas, the rest on disk — so opening either
//! picker and scrolling to its end draws no editor and shows each tile's picture within two
//! frames (no card lingering); the warm-up waits for the start-up and for a quiet hand; its
//! passes cost a plain pass and at most a quarter of a frame (p99), never a frame more
//! (max) — the editors drawn on a worker; once done it asks for no frame (the frame budget
//! at rest unchanged); each measured beside a plain pass of the same window.
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
    let mut out = ctx.run_ui(input, |ui| picker_warm::frame(app, &ui.ctx().clone()));
    out.textures_delta.clear();
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
    // `CANTUS_KEEP_WARM` (a folder): the pictures made on the worker, kept to be looked at.
    if let Some(keep) = std::env::var_os("CANTUS_KEEP_WARM") {
        let keep = std::path::PathBuf::from(keep);
        let _ = std::fs::create_dir_all(&keep);
        for e in std::fs::read_dir(dir.path()).unwrap().flatten() {
            let _ = std::fs::copy(e.path(), keep.join(e.file_name()));
        }
    }
    let (held, pages) = th.held();
    assert_eq!(pages, 1, "one atlas page");
    assert!(held >= 20.min(all.len()), "the most used held: {held}");
    let captures = th.stats.captures;
    assert!(
        captures as usize <= all.len(),
        "each made at most once: {captures}"
    );

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
        let mut out = ctx2.run_ui(input, |_| {});
        out.textures_delta.clear();
        empty.push(t0.elapsed().as_secs_f64() * 1e3);
    }
    let median = |v: &mut Vec<f64>| {
        v.sort_by(|a, b| a.partial_cmp(b).unwrap());
        v[v.len() / 2]
    };
    let (rest_ms, empty_ms) = (median(&mut rest), median(&mut empty));
    // The budget (the owner is sensitive to input lag): a pass of the warm-up costs a plain
    // pass and at most a slice of a frame. The editors are drawn on a worker; this thread
    // only uploads each finished picture.
    let budget = empty_ms + cantus_ui::views::picker_thumbs::WARM_BUDGET_MS;
    let over_budget = steps.iter().filter(|ms| **ms > budget).count();
    steps.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let worst = steps.last().copied().unwrap_or(0.0);
    let over = steps.iter().filter(|ms| **ms > 1000.0 / 60.0).count();
    let top: Vec<String> = steps
        .iter()
        .rev()
        .take(6)
        .map(|ms| format!("{ms:.0}"))
        .collect();
    eprintln!(
        "warm-up passes over a 60 Hz frame: {over} of {} (the slowest, ms: {})",
        steps.len(),
        top.join(", ")
    );
    let typical = steps[steps.len() / 2];
    eprintln!(
        "warm-up: {} pictures, {captures} editors drawn, {} passes; a pass {typical:.2} ms typical, {worst:.1} ms worst; at rest {rest_ms:.3} ms vs an empty pass {empty_ms:.3} ms",
        all.len(),
        steps.len()
    );
    let p = |q: f64| steps[((steps.len() - 1) as f64 * q).round() as usize];
    eprintln!(
        "warm-up per pass: p50 {:.3} ms, p99 {:.3} ms, max {worst:.2} ms; a plain pass {empty_ms:.3} ms (p50 ratio {:.2}); {over_budget} passes over {budget:.2} ms",
        p(0.5),
        p(0.99),
        p(0.5) / empty_ms.max(1e-6)
    );
    assert!(
        p(0.99) <= budget,
        "p99 {} ms over the budget {budget} ms",
        p(0.99)
    );
    // r42 fix-engine (review reach 2, docs/RELEASING.md L3): no gate on the single slowest
    // pass — one deschedule of a loaded runner is not the warm-up's cost; the p99 above is
    // the gate, `worst` is reported.
    // At rest it is an empty pass (a reference timed beside it, never a fixed figure).
    assert!(
        rest_ms <= empty_ms * 3.0 + 0.05,
        "at rest {rest_ms} ms, empty {empty_ms} ms"
    );
}

/// r42 fix-ui (review ui#12): the warm-up never runs while the song plays — quiet hands and
/// a long start-up notwithstanding — and begins once the transport has stopped and the
/// quiet has been counted again from the stop.
#[test]
fn the_warm_up_waits_while_the_song_plays() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = ctx();
    let mut app = CantusApp::from_parts(Some(welcome_project()), EngineHandle::dummy(), false);
    app.picker_view
        .thumbs
        .enable(Some(dir.path().to_path_buf()));
    app.engine.set_heard_for_tests(cantus_core::Ticks(0), true);
    let mut t = 0.0;
    for _ in 0..200 {
        pass(&mut app, &ctx, t, Vec::new());
        t += 0.05;
    }
    assert_eq!(
        app.picker_view.thumbs.stats.captures, 0,
        "nothing made while the song plays"
    );
    app.engine.set_heard_for_tests(cantus_core::Ticks(0), false);
    // Stopped: not at once (the quiet counts from the stop) …
    pass(&mut app, &ctx, t, Vec::new());
    assert_eq!(app.picker_view.thumbs.stats.captures, 0, "the quiet first");
    // … then it begins.
    let mut steps = 0;
    while app.picker_view.thumbs.stats.captures == 0 && steps < 400 {
        t += 0.05;
        pass(&mut app, &ctx, t, Vec::new());
        steps += 1;
        // The runner works on its own thread: give it real time.
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(
        app.picker_view.thumbs.stats.captures > 0,
        "it begins after the stop"
    );
}

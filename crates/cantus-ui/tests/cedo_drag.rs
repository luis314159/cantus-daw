//! Cedo's editor answers the pointer as its sheet promises (ADR 0144 provisional, phase B),
//! driven frame by frame with real pointer events through the window the app draws: the
//! shape's points dragged on the grid, a segment bent, a point added and taken away by a
//! double-click, a segment's shape turned by a right-click; the envelope's three handles
//! dragged in time and the threshold's line up the history (Sidechain); the trigger chosen
//! on its card and on its flag; the band strip's crossover dragged and its split clicked; the
//! level gauge dragged to a deeper duck. Each gesture is one undo step. // fxb:cedo

use cantus_core::*;
use cantus_engine::EngineHandle;
use cantus_ui::raster::shoot_events;
use cantus_ui::views::fx::cedo::{edit, shape_amount};
use cantus_ui::{views, welcome_project, CantusApp};
use egui::{Event, PointerButton, Pos2};

const KIND: EffectKind = EffectKind::Ducker;

fn ctx() -> egui::Context {
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    cantus_ui::theme::apply(&cantus_ui::theme::slate(), &ctx);
    views::fx::frame::shell_of(KIND).set_scale(&ctx, 1.0);
    ctx
}

fn open() -> (CantusApp, egui::Vec2) {
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
    let mut app = CantusApp::from_parts(Some(welcome_project()), EngineHandle::dummy(), false);
    app.demo_editor("cedo");
    app.fx.pos = Some(egui::pos2(0.0, 0.0));
    let size = views::fx::frame::designed_size(KIND);
    settle(&mut app, size);
    (app, size)
}

fn window(app: &mut CantusApp, ui: &mut egui::Ui) {
    let c = ui.ctx().clone();
    views::fx::window(app, &c)
}

/// A few quiet frames in a context of their own: the window's rects are known after.
fn settle(app: &mut CantusApp, size: egui::Vec2) {
    shoot_events(
        &ctx(),
        size,
        1.0,
        6,
        None,
        |_, _| Vec::new(),
        |ui| window(app, ui),
    );
}

fn button(p: Pos2, b: PointerButton, pressed: bool) -> Event {
    Event::PointerButton {
        pos: p,
        button: b,
        pressed,
        modifiers: Default::default(),
    }
}

/// Clicks at `at` with `b`: `n` press-and-releases in one frame (two make a double-click),
/// then the window settles.
fn clicks(app: &mut CantusApp, size: egui::Vec2, at: Pos2, b: PointerButton, n: usize) {
    shoot_events(
        &ctx(),
        size,
        1.0,
        12,
        None,
        |f, _| match f {
            4 => vec![Event::PointerMoved(at)],
            5 if n == 1 => vec![button(at, b, true)],
            6 if n == 1 => vec![button(at, b, false)],
            5 => (0..n)
                .flat_map(|_| [button(at, b, true), button(at, b, false)])
                .collect(),
            _ => Vec::new(),
        },
        |ui| window(app, ui),
    );
    settle(app, size);
}

fn click(app: &mut CantusApp, size: egui::Vec2, at: Pos2) {
    clicks(app, size, at, PointerButton::Primary, 1);
}

/// A drag from `from` to `to` in twelve moves, in a context of its own (egui's pointer
/// history must never see time go back), then the window settles.
fn drag(app: &mut CantusApp, size: egui::Vec2, from: Pos2, to: Pos2) {
    let steps = 12;
    let q = 4;
    shoot_events(
        &ctx(),
        size,
        1.0,
        q + steps + 6,
        None,
        |f, _| match f {
            f if f < q => Vec::new(),
            f if f == q => vec![Event::PointerMoved(from)],
            f if f == q + 1 => vec![button(from, PointerButton::Primary, true)],
            f if f < q + steps + 2 => {
                let t = (f - q - 1) as f32 / steps as f32;
                vec![Event::PointerMoved(from + (to - from) * t)]
            }
            f if f == q + steps + 2 => vec![button(to, PointerButton::Primary, false)],
            _ => Vec::new(),
        },
        |ui| window(app, ui),
    );
    settle(app, size);
}

fn spec(app: &CantusApp) -> EffectSpec {
    let (strip, effect, _) = app.fx.open.expect("open");
    app.project().effect(strip, effect).unwrap().spec.clone()
}

fn value(app: &CantusApp, key: &str) -> f32 {
    spec(app).get(key)
}

fn set(app: &mut CantusApp, size: egui::Vec2, key: &str, value: f32) {
    let (strip, effect, _) = app.fx.open.expect("open");
    app.exec(Command::SetEffectParam {
        strip,
        effect,
        key: key.into(),
        value,
    });
    settle(app, size);
}

fn plot(app: &CantusApp) -> egui::Rect {
    app.fx.cedo.plot().expect("the plot is drawn")
}

/// The place on the plot of `u` of the cycle and amount `a`.
fn at(app: &CantusApp, u: f32, a: f32) -> Pos2 {
    let r = plot(app);
    egui::pos2(r.left() + r.width() * u, r.top() + r.height() * a)
}

#[test]
fn the_shape_s_points_drag_bend_come_and_go_each_one_undo_step() {
    let (mut app, size) = open();
    let s0 = spec(&app);
    assert_eq!(value(&app, "trig"), 1.0, "Classic pump: Tempo");
    assert!(s0.cedo.is_none(), "the classic pump, no shape of its own");
    let pump = edit::shape_of(&s0).points;
    assert_eq!(pump.len(), 2);
    // The release point dragged to half the cycle, a quarter of the depth: snapped to the
    // sixteenths' grid in place, where the pointer is in amount.
    let from = app.fx.cedo.shape_point(&s0, 1).expect("the point is drawn");
    let to = at(&app, 0.503, 0.25);
    drag(&mut app, size, from, to);
    let moved = edit::shape_of(&spec(&app)).points;
    assert_eq!(moved.len(), 2);
    assert_eq!(moved[1].x, 0.5, "on the grid");
    assert!((moved[1].y - 0.25).abs() < 0.02, "amount {:.3}", moved[1].y);
    assert_eq!(moved[0], pump[0], "the other point stays");
    app.undo();
    assert_eq!(spec(&app).cedo, None, "one drag, one undo step");

    // The first segment bent upward from its middle: less is ducked there.
    let s = spec(&app);
    let before = shape_amount(&s, 0.3);
    let from = app.fx.cedo.on_curve(&s, 0.3).unwrap();
    drag(&mut app, size, from, from - egui::vec2(0.0, 50.0));
    let after = shape_amount(&spec(&app), 0.3);
    eprintln!("bent: the amount at 0.3 {before:.3} -> {after:.3}");
    assert!(
        after < before - 0.05,
        "bulged up: {before:.3} -> {after:.3}"
    );
    assert_eq!(
        edit::shape_of(&spec(&app)).points[0].x,
        0.0,
        "no point moved"
    );
    app.undo();
    assert_eq!(spec(&app).cedo, None, "the bend undone");

    // A double-click on the empty plot adds a point there (on the grid); another on it
    // takes it away.
    let empty = at(&app, 13.0 / 16.0, 0.5);
    clicks(&mut app, size, empty, PointerButton::Primary, 2);
    let three = edit::shape_of(&spec(&app)).points;
    assert_eq!(three.len(), 3, "a point added");
    assert_eq!(three[2].x, 13.0 / 16.0);
    assert!((three[2].y - 0.5).abs() < 0.02);
    let p = app.fx.cedo.shape_point(&spec(&app), 2).unwrap();
    clicks(&mut app, size, p, PointerButton::Primary, 2);
    assert_eq!(
        edit::shape_of(&spec(&app)).points,
        pump,
        "the added point taken away"
    );
    app.undo();
    assert_eq!(
        edit::shape_of(&spec(&app)).points,
        three,
        "its removal undone"
    );
    app.undo();
    assert_eq!(spec(&app).cedo, None, "its adding undone");

    // A right-click on the first point turns its segment Curve -> S -> Hold -> Curve.
    let p = app.fx.cedo.shape_point(&spec(&app), 0).unwrap();
    let mut seen = Vec::new();
    for _ in 0..3 {
        clicks(&mut app, size, p, PointerButton::Secondary, 1);
        seen.push(edit::shape_of(&spec(&app)).points[0].s);
    }
    assert_eq!(seen, [1, 2, 0], "S, hold, curve");
    for _ in 0..3 {
        app.undo();
    }
    assert_eq!(spec(&app).cedo, None, "three turns, three undo steps");
}

#[test]
fn the_envelope_s_handles_and_the_threshold_drag_in_sidechain() {
    let (mut app, size) = open();
    set(&mut app, size, "trig", 0.0);
    let s = spec(&app);
    let (att, hold, rel) = (s.get("att"), s.get("hold"), s.get("rel"));
    assert_eq!((att, hold, rel), (1.0, 40.0, 150.0), "the defaults");
    // Each handle dragged 30 points to the right: its own time longer, by what a point was
    // worth when the drag began, the others as they were.
    for (n, key) in ["att", "hold", "rel"].into_iter().enumerate() {
        let s = spec(&app);
        let was = s.get(key);
        let (_, span) = edit::env_scale(&s);
        let per = span / plot(&app).width();
        let from = app.fx.cedo.env_handle(&s, n).expect("the handle is drawn");
        drag(&mut app, size, from, from + egui::vec2(30.0, 0.0));
        let now = value(&app, key);
        eprintln!("{key}: {was} -> {now} ms");
        assert!(
            (now - (was + 30.0 * per)).abs() < 0.2 + 0.02 * now,
            "{key}: {was} -> {now} (a point {per:.2} ms)"
        );
        app.undo();
        assert_eq!(value(&app, key), was, "{key}: one drag, one undo step");
    }
    // The threshold's line dragged up a sixth of the history: 10 dB higher.
    let s = spec(&app);
    let thr = s.get("thr");
    let from = app
        .fx
        .cedo
        .threshold_at(&s)
        .expect("the threshold is drawn");
    let hist_h = {
        // The history's height from the line's two places a decibel apart.
        let mut t = s.clone();
        t.set("thr", thr + 1.0);
        from.y - app.fx.cedo.threshold_at(&t).unwrap().y
    };
    assert!(hist_h > 0.0);
    drag(&mut app, size, from, from - egui::vec2(0.0, 10.0 * hist_h));
    let now = value(&app, "thr");
    assert!((now - (thr + 10.0)).abs() < 0.3, "threshold {thr} -> {now}");
    app.undo();
    assert_eq!(value(&app, "thr"), thr, "one drag, one undo step");
}

#[test]
fn the_trigger_is_chosen_on_its_card_and_on_its_flag_each_one_undo_step() {
    let (mut app, size) = open();
    assert_eq!(value(&app, "trig"), 1.0);
    for (i, want) in [(0usize, 0.0), (2, 2.0), (1, 1.0)] {
        let p = app.fx.cedo.trigger_choice(i).expect("the switch is drawn");
        click(&mut app, size, p);
        assert_eq!(value(&app, "trig"), want, "the switch's choice {i}");
    }
    // The flag on the bed: the next trigger each click.
    let mut seen = Vec::new();
    for _ in 0..3 {
        let f = app
            .fx
            .cedo
            .trigger_flag()
            .expect("the trigger's flag is drawn");
        click(&mut app, size, f.center());
        seen.push(value(&app, "trig"));
    }
    assert_eq!(seen, [2.0, 0.0, 1.0], "Tempo -> MIDI -> Sidechain -> Tempo");
    for want in [0.0, 2.0, 1.0, 2.0, 0.0, 1.0] {
        app.undo();
        assert_eq!(value(&app, "trig"), want, "each click one undo step");
    }
}

#[test]
fn the_band_strip_s_crossover_drags_and_its_split_switches() {
    let (mut app, size) = open();
    assert!(app.fx.cedo.band_strip().is_none(), "Whole: no strip");
    set(&mut app, size, "band", 1.0);
    let strip = app.fx.cedo.band_strip().expect("Lows: the strip is drawn");
    let s = spec(&app);
    assert_eq!(s.get("xhz"), 120.0);
    let from = app.fx.cedo.crossover_at(&s).unwrap();
    let to = from + egui::vec2(70.0, 0.0);
    drag(&mut app, size, from, to);
    let hz = value(&app, "xhz");
    let want = edit::strip_hz(strip, to.x);
    eprintln!("crossover 120 -> {hz} Hz (the pointer at {want:.0} Hz)");
    assert!((hz / want - 1.0).abs() < 0.02, "{hz} for {want:.0}");
    // The split's word: minimum phase -> linear phase -> back.
    let w = app.fx.cedo.split_word().expect("the split is written");
    click(&mut app, size, w.center());
    assert_eq!(value(&app, "xover"), 1.0, "linear phase");
    let w = app.fx.cedo.split_word().unwrap();
    click(&mut app, size, w.center());
    assert_eq!(value(&app, "xover"), 0.0, "minimum phase again");
    app.undo();
    assert_eq!(value(&app, "xover"), 1.0);
    app.undo();
    assert_eq!(value(&app, "xover"), 0.0);
    app.undo();
    assert_eq!(
        value(&app, "xhz"),
        120.0,
        "the crossover's drag one undo step"
    );
}

#[test]
fn the_gauge_dragged_down_ducks_deeper() {
    let (mut app, size) = open();
    assert_eq!(value(&app, "depth"), 9.0, "Classic pump: 9 dB");
    let g = app.fx.cedo.gauge().expect("the gauge is drawn");
    let from = g.center();
    drag(&mut app, size, from, from + egui::vec2(0.0, 40.0));
    let deeper = value(&app, "depth");
    assert!((deeper - 19.0).abs() < 0.3, "9 dB + 40 points: {deeper}");
    drag(&mut app, size, from, from - egui::vec2(0.0, 20.0));
    let less = value(&app, "depth");
    assert!(
        (less - (deeper - 5.0)).abs() < 0.3,
        "{deeper} - 20 points: {less}"
    );
    app.undo();
    assert_eq!(value(&app, "depth"), deeper);
    app.undo();
    assert_eq!(value(&app, "depth"), 9.0, "each drag one undo step");
}

/// The latency a spec costs at 48 kHz (what the host is told).
fn latency(s: &EffectSpec) -> usize {
    cantus_dsp::cedo::latency_for(&cantus_engine::fx::cedo::settings(s), 48_000.0)
}

/// fxb:cedo — the owner's floor rule (2026-11-17): in Auto the linear phase's floor follows
/// the crossover an octave under it, but the latency moves only when the crossover is let
/// go — never during the drag — and the drag and its new floor are one undo step.
#[test]
fn dragging_the_crossover_moves_the_latency_only_at_the_release() {
    let (mut app, size) = open();
    set(&mut app, size, "band", 1.0);
    set(&mut app, size, "xover", 1.0);
    let s = spec(&app);
    assert_eq!((s.get("lfloor"), s.get("lfhz")), (0.0, 60.0), "Auto, 60 Hz");
    let before = latency(&s);
    assert_eq!(before, 1_765, "the 60 Hz floor's kernel");
    let from = app.fx.cedo.crossover_at(&s).unwrap();
    let to = from + egui::vec2(90.0, 0.0);
    // The drag held at its end, the pointer still down: read there.
    let (steps, q) = (12, 4);
    let held = std::cell::RefCell::new(None);
    let calls = std::cell::Cell::new(0usize);
    shoot_events(
        &ctx(),
        size,
        1.0,
        q + steps + 6,
        None,
        |f, _| match f {
            f if f < q => Vec::new(),
            f if f == q => vec![Event::PointerMoved(from)],
            f if f == q + 1 => vec![button(from, PointerButton::Primary, true)],
            f if f < q + steps + 2 => {
                let t = (f - q - 1) as f32 / steps as f32;
                vec![Event::PointerMoved(from + (to - from) * t)]
            }
            f if f == q + steps + 2 => vec![button(to, PointerButton::Primary, false)],
            _ => Vec::new(),
        },
        |ui| {
            window(&mut app, ui);
            calls.set(calls.get() + 1);
            if calls.get() == q + steps + 2 {
                *held.borrow_mut() = Some(spec(&app));
            }
        },
    );
    settle(&mut app, size);
    let held = held.into_inner().expect("read while held");
    assert!(held.get("xhz") > 200.0, "dragged: {} Hz", held.get("xhz"));
    assert_eq!(held.get("lfhz"), 60.0, "the floor waits for the release");
    assert_eq!(latency(&held), before, "no latency change during the drag");
    let after = spec(&app);
    let x = after.get("xhz");
    assert_eq!(
        after.get("lfhz"),
        (x * 0.5).clamp(40.0, 1_000.0),
        "an octave under"
    );
    let now = latency(&after);
    eprintln!("crossover {x} Hz: the latency {before} -> {now} samples at the release");
    assert!(now < before, "a higher crossover costs less");
    // The same latency as the fixed floor at that frequency would cost.
    assert_eq!(
        now,
        cantus_dsp::cedo::band::linear_latency(x * 0.5, 48_000.0)
    );
    app.undo();
    let back = spec(&app);
    assert_eq!(
        (back.get("xhz"), back.get("lfhz")),
        (120.0, 60.0),
        "one undo step"
    );
    // A floor chosen on the card keeps its latency whatever the crossover.
    set(&mut app, size, "lfloor", 3.0);
    assert_eq!(latency(&spec(&app)), 1_325, "80 Hz: 27.6 ms");
}

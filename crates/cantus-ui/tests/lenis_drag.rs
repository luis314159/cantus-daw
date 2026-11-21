//! Lenis's editor answers the pointer as its sheet promises (ADR 0140, phase
//! B), driven frame by frame with real pointer events through the window the app draws:
//! the focus's two gates dragged along the screen, each of its six band nodes dragged in
//! frequency and amount (a band dragged while off is switched on by the same step), the
//! Depth dial turned, and the push keys (Quality, Curve, *What is removed*, Channels)
//! pressed. Each gesture is one undo step. // fxb:lenis

use cantus_core::*;
use cantus_engine::EngineHandle;
use cantus_ui::raster::shoot_events;
use cantus_ui::{views, welcome_project, CantusApp};
use egui::{Event, PointerButton, Pos2};

const KIND: EffectKind = EffectKind::ResonanceSuppressor;

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
    app.demo_editor("lenis");
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

fn button(p: Pos2, pressed: bool) -> Event {
    Event::PointerButton {
        pos: p,
        button: PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    }
}

/// A click at `at`: pressed and let go a frame apart, then the window settles.
fn click(app: &mut CantusApp, size: egui::Vec2, at: Pos2) {
    shoot_events(
        &ctx(),
        size,
        1.0,
        12,
        None,
        |f, _| match f {
            4 => vec![Event::PointerMoved(at)],
            5 => vec![button(at, true)],
            6 => vec![button(at, false)],
            _ => Vec::new(),
        },
        |ui| window(app, ui),
    );
    settle(app, size);
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
            f if f == q + 1 => vec![button(from, true)],
            f if f < q + steps + 2 => {
                let t = (f - q - 1) as f32 / steps as f32;
                vec![Event::PointerMoved(from + (to - from) * t)]
            }
            f if f == q + steps + 2 => vec![button(to, false)],
            _ => Vec::new(),
        },
        |ui| window(app, ui),
    );
    settle(app, size);
}

fn value(app: &CantusApp, key: &str) -> f32 {
    let (strip, effect, _) = app.fx.open.expect("open");
    app.project().effect(strip, effect).unwrap().spec.get(key)
}

fn band(app: &CantusApp, n: usize, what: &str) -> f32 {
    value(app, cantus_core::fx::lenis::band_key(n, what))
}

/// The frequency the screen reads at `x`, as the editor drew it last frame.
fn hz_at(app: &CantusApp, x: f32) -> f32 {
    app.fx.lenis.plot.expect("the screen is drawn").hz(x)
}

#[test]
fn the_focus_s_gates_drag_along_the_screen_each_one_undo_step() {
    let _theme_turn = theme_turn();
    let (mut app, size) = open();
    // The starting preset, Gentle: the focus from 150 Hz to the top.
    assert_eq!((value(&app, "lo"), value(&app, "hi")), (150.0, 20_000.0));
    // The low gate's tab dragged to the right: the focus starts higher.
    let tab = app.fx.lenis.gate(false).expect("the low gate is drawn");
    let from = tab.center();
    let to = from + egui::vec2(160.0, 0.0);
    drag(&mut app, size, from, to);
    let lo = value(&app, "lo");
    // The gate follows the pointer (the tab stands beside its line, so it jumps there first).
    let want = hz_at(&app, to.x);
    assert!(lo > 400.0, "the low gate moved up: {lo:.0} Hz");
    assert!(
        (lo / want - 1.0).abs() < 0.03,
        "where the pointer let go: {lo:.0} for {want:.0} Hz"
    );
    // The high gate's tab dragged to the left: the focus ends lower.
    let tab = app.fx.lenis.gate(true).expect("the high gate is drawn");
    let from = tab.center();
    drag(&mut app, size, from, from - egui::vec2(220.0, 0.0));
    let hi = value(&app, "hi");
    assert!(
        hi < 8_000.0 && hi > lo * 1.25,
        "the high gate moved down: {hi:.0} Hz"
    );
    // Each drag one undo step.
    app.undo();
    assert_eq!(value(&app, "hi"), 20_000.0, "the high gate's drag undone");
    assert_eq!(value(&app, "lo"), lo, "the low gate's drag kept");
    app.undo();
    assert_eq!(value(&app, "lo"), 150.0, "the low gate's drag undone");
}

#[test]
fn each_band_s_node_drags_in_frequency_and_amount_and_switches_on_in_the_same_step() {
    let _theme_turn = theme_turn();
    let (mut app, size) = open();
    for n in 0..cantus_core::fx::lenis::BANDS {
        assert_eq!(band(&app, n, "on"), 0.0, "band {n} starts off");
    }
    let mut before = Vec::new();
    for n in 0..cantus_core::fx::lenis::BANDS {
        let from = app.fx.lenis.node(n).expect("the node is drawn");
        let hz0 = band(&app, n, "hz");
        // Up (more of the cut) and a little to the right (higher).
        let to = from + egui::vec2(24.0, -26.0);
        drag(&mut app, size, from, to);
        assert_eq!(
            band(&app, n, "on"),
            1.0,
            "band {n}: dragged while off, now on"
        );
        let hz = band(&app, n, "hz");
        let want = hz_at(&app, to.x);
        assert!(hz > hz0, "band {n}: higher ({hz0:.0} -> {hz:.0} Hz)");
        assert!(
            (hz / want - 1.0).abs() < 0.03,
            "band {n}: where the pointer let go ({hz:.0} for {want:.0} Hz)"
        );
        assert!(
            band(&app, n, "amt") > 0.0,
            "band {n}: raised over the curve: {:+.2}",
            band(&app, n, "amt")
        );
        assert_eq!(app.fx.lenis.band, n, "the node's drag selects its band");
        before.push(hz0);
        // A second drag on a band that is on moves it again, down this time.
        let from = app.fx.lenis.node(n).unwrap();
        let amt = band(&app, n, "amt");
        drag(&mut app, size, from, from + egui::vec2(0.0, 30.0));
        assert!(band(&app, n, "amt") < amt, "band {n}: lowered");
        assert_eq!(band(&app, n, "on"), 1.0, "band {n}: still on");
        app.undo();
        assert_eq!(
            band(&app, n, "amt"),
            amt,
            "band {n}: the second drag undone"
        );
    }
    // Each first drag one undo step that also gives back the band's switch.
    for n in (0..cantus_core::fx::lenis::BANDS).rev() {
        app.undo();
        assert_eq!(band(&app, n, "on"), 0.0, "band {n}: off again");
        assert_eq!(
            band(&app, n, "hz"),
            before[n],
            "band {n}: back where it was"
        );
        assert_eq!(band(&app, n, "amt"), 0.0, "band {n}: its amount back");
    }
}

#[test]
fn the_depth_dial_turns_and_a_double_click_turns_it_to_none() {
    let _theme_turn = theme_turn();
    let (mut app, size) = open();
    assert_eq!(value(&app, "depth"), 0.5, "Gentle: half");
    let (c, rad) = app.fx.lenis.dial().expect("the dial is drawn");
    let from = c + egui::vec2(0.0, rad * 0.4);
    // Up 66 points: the dial reads 30 % more.
    drag(&mut app, size, from, from - egui::vec2(0.0, 66.0));
    let up = value(&app, "depth");
    assert!((up - 0.8).abs() < 0.03, "turned up: {up:.2}");
    drag(&mut app, size, from, from + egui::vec2(0.0, 44.0));
    let down = value(&app, "depth");
    assert!((down - (up - 0.2)).abs() < 0.03, "turned down: {down:.2}");
    app.undo();
    assert_eq!(value(&app, "depth"), up, "one turn, one undo step");
    app.undo();
    assert_eq!(value(&app, "depth"), 0.5, "back to Gentle's");
}

#[test]
fn the_push_keys_choose_each_one_undo_step() {
    let _theme_turn = theme_turn();
    let (mut app, size) = open();
    let start = ["qual", "curve", "delta", "chan"].map(|k| value(&app, k));
    assert_eq!(
        start,
        [1.0, 0.0, 0.0, 0.0],
        "Standard, Soft, delta off, Linked"
    );
    let presses: [(&str, usize, f32); 7] = [
        ("qual", 2, 2.0),
        ("qual", 0, 0.0),
        ("curve", 1, 1.0),
        ("delta", 1, 1.0),
        ("chan", 2, 2.0),
        ("chan", 1, 1.0),
        ("delta", 1, 0.0),
    ];
    let mut was = Vec::new();
    for (key, pos, want) in presses {
        let at = app
            .fx
            .lenis
            .push_key(key, pos)
            .unwrap_or_else(|| panic!("the key {key} {pos} is drawn"));
        was.push((key, value(&app, key)));
        click(&mut app, size, at.center());
        assert_eq!(value(&app, key), want, "{key} pressed at {pos}");
    }
    // A pressed key pressed again leaves its row as it is (no undo step).
    let at = app.fx.lenis.push_key("chan", 1).unwrap();
    click(&mut app, size, at.center());
    assert_eq!(value(&app, "chan"), 1.0);
    for (key, v) in was.into_iter().rev() {
        app.undo();
        assert_eq!(value(&app, key), v, "{key}: one press, one undo step");
    }
    assert_eq!(
        ["qual", "curve", "delta", "chan"].map(|k| value(&app, k)),
        start
    );
}

/// This binary's tests take turns (docs/RELEASING.md L5): `theme::apply` installs its palette
/// and the kit's tokens for the whole process, so a test applying a theme repainted another's
/// words mid-measure (beta.16's rehearsal, macOS: fx_limiter's dark demo cut "GAIN REDUCTION").
/// The app has one interface thread; only tests run several.
fn theme_turn() -> std::sync::MutexGuard<'static, ()> {
    static TURN: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
    // Every test starts in the app's own look, whatever the test before it applied.
    cantus_ui::theme::apply(
        &cantus_ui::theme::default_theme(),
        &egui::Context::default(),
    );
    turn
}

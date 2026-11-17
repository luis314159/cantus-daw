//! Purus's editor answers the pointer as its picture promises (ADR 0145 provisional, phase
//! B), driven frame by frame with real pointer events through the window the app draws:
//!
//! - each cartridge's bat toggle switches its module, each *Hear what is removed* key its
//!   delta, the hum's 50 Hz / 60 Hz keys set the mains and Gentle its mode, the room's
//!   *Set by hand* / *Estimated* keys where the decay comes from — each press one undo
//!   step, undone to where it was;
//! - the comb's needles dragged up or down move the fundamental so the needle held sits
//!   under the pointer (the fundamental's own, or the second harmonic's at half the
//!   frequency), each drag one undo step, and none while the hum module is off;
//! - the room's tail dragged sets the reduction to the depth under the pointer, and its
//!   mark where the early reflections end moves them, each drag one undo step, and none
//!   while the room module is off. // fxc:purus

use cantus_core::*;
use cantus_engine::EngineHandle;
use cantus_ui::raster::shoot_events;
use cantus_ui::views::fx::host::FxHost as _;
use cantus_ui::views::fx::purus::Act;
use cantus_ui::{views, welcome_project, CantusApp};
use egui::{Event, PointerButton, Pos2};

const KIND: EffectKind = EffectKind::Repair;

fn ctx() -> egui::Context {
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    cantus_ui::theme::apply(&cantus_ui::theme::slate(), &ctx);
    views::fx::frame::shell_of(KIND).set_scale(&ctx, 1.0);
    ctx
}

fn open(kv: &[(&str, f32)]) -> (CantusApp, egui::Vec2) {
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
    let mut app = CantusApp::from_parts(Some(welcome_project()), EngineHandle::dummy(), false);
    app.demo_editor("purus");
    let (strip, effect, _) = app.fx.open.expect("the unit is open");
    if !kv.is_empty() {
        views::fx::host::DawHost::new(&mut app, strip, effect, "purus")
            .set_many(kv, "cmd.fx_preset");
    }
    views::fx::demo_feed(&mut app, KIND);
    app.fx.pos = Some(egui::pos2(0.0, 0.0));
    let size = views::fx::frame::designed_size(KIND);
    settle(&mut app, size);
    (app, size)
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
        |ui| {
            let c = ui.ctx().clone();
            views::fx::window(app, &c)
        },
    );
}

fn press(p: Pos2, pressed: bool) -> Event {
    Event::PointerButton {
        pos: p,
        button: PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    }
}

/// Press at `from`, move to `to` in steps, let go — in a settled context of its own.
fn drag(app: &mut CantusApp, size: egui::Vec2, from: Pos2, to: Pos2) {
    let (steps, q) = (12, 4);
    shoot_events(
        &ctx(),
        size,
        1.0,
        q + steps + 6,
        None,
        |f, _| match f {
            f if f < q => Vec::new(),
            f if f == q => vec![Event::PointerMoved(from)],
            f if f == q + 1 => vec![press(from, true)],
            f if f < q + steps + 2 => {
                let t = (f - q - 1) as f32 / steps as f32;
                vec![Event::PointerMoved(from + (to - from) * t)]
            }
            f if f == q + steps + 2 => vec![press(to, false)],
            _ => Vec::new(),
        },
        |ui| {
            let c = ui.ctx().clone();
            views::fx::window(app, &c)
        },
    );
}

/// A click at `at`: pressed and let go a frame apart.
fn click(app: &mut CantusApp, size: egui::Vec2, at: Pos2) {
    shoot_events(
        &ctx(),
        size,
        1.0,
        10,
        None,
        |f, _| match f {
            2 => vec![Event::PointerMoved(at)],
            3 => vec![press(at, true)],
            4 => vec![press(at, false)],
            _ => Vec::new(),
        },
        |ui| {
            let c = ui.ctx().clone();
            views::fx::window(app, &c)
        },
    );
}

fn value(app: &CantusApp, key: &str) -> f32 {
    let (strip, effect, _) = app.fx.open.expect("open");
    app.project().effect(strip, effect).unwrap().spec.get(key)
}

fn set(app: &mut CantusApp, key: &str, value: f32) {
    let (strip, effect, _) = app.fx.open.expect("open");
    app.exec(Command::SetEffectParam {
        strip,
        effect,
        key: key.into(),
        value,
    });
}

/// The bench's spot doing `act`.
fn spot(app: &CantusApp, act: Act) -> Pos2 {
    app.fx
        .purus
        .bench
        .as_ref()
        .expect("the bench is laid out")
        .spots
        .iter()
        .find(|s| s.act == act)
        .unwrap_or_else(|| panic!("no spot for {act:?}"))
        .rect
        .center()
}

#[test]
fn the_bench_s_toggles_and_keys_each_one_undo_step() {
    let (mut app, size) = open(&[]);
    // The starting preset: the hum module on at 50 Hz, the others off.
    assert_eq!((value(&app, "hon"), value(&app, "hhz")), (1.0, 50.0));
    let cases: [(Act, &str, f32); 11] = [
        (Act::Toggle("hon"), "hon", 0.0),
        (Act::Toggle("con"), "con", 1.0),
        (Act::Toggle("von"), "von", 1.0),
        (Act::Toggle("hdelta"), "hdelta", 1.0),
        (Act::Toggle("cdelta"), "cdelta", 1.0),
        (Act::Toggle("vdelta"), "vdelta", 1.0),
        (Act::Set("hhz", 60.0), "hhz", 60.0),
        (Act::Toggle("hgentle"), "hgentle", 1.0),
        (Act::Set("vdecay", 1.0), "vdecay", 1.0),
        (Act::Set("vdecay", 0.0), "vdecay", 0.0),
        (Act::Set("hhz", 50.0), "hhz", 50.0),
    ];
    for (act, key, want) in cases {
        // The key's other value first where it sets one (so the press moves something).
        let before = match act {
            Act::Set("vdecay", 0.0) => {
                set(&mut app, "vdecay", 1.0);
                1.0
            }
            Act::Set("hhz", 50.0) => {
                set(&mut app, "hhz", 60.0);
                60.0
            }
            _ => value(&app, key),
        };
        settle(&mut app, size);
        let at = spot(&app, act);
        click(&mut app, size, at);
        assert_eq!(value(&app, key), want, "{act:?} sets {key}");
        app.undo();
        assert_eq!(
            value(&app, key),
            before,
            "{act:?}: one press, one undo step"
        );
    }
}

#[test]
fn the_comb_s_needles_drag_the_fundamental_each_drag_one_undo_step() {
    let (mut app, size) = open(&[("hmode", 0.0)]);
    assert_eq!(value(&app, "hhz"), 50.0);
    // The fundamental's needle from 50 Hz to where 60 Hz is on the rail.
    let from = app.fx.purus.needle(1).expect("the comb is drawn");
    let to = app.fx.purus.rail_at(60.0).unwrap();
    drag(&mut app, size, from, to);
    assert!(
        (value(&app, "hhz") - 60.0).abs() < 0.11,
        "the fundamental follows its needle to 60 Hz: {}",
        value(&app, "hhz")
    );
    app.undo();
    assert_eq!(value(&app, "hhz"), 50.0, "one drag, one undo step");
    // By the second harmonic's needle (100 Hz) to 110 Hz: the fundamental 55 Hz.
    settle(&mut app, size);
    let from = app.fx.purus.needle(2).unwrap();
    let to = app.fx.purus.rail_at(110.0).unwrap();
    drag(&mut app, size, from, to);
    assert!(
        (value(&app, "hhz") - 55.0).abs() < 0.11,
        "held by its second needle: {}",
        value(&app, "hhz")
    );
    app.undo();
    assert_eq!(value(&app, "hhz"), 50.0, "one drag, one undo step");
    // With the hum module off the comb is not drawn and takes no drag.
    set(&mut app, "hon", 0.0);
    settle(&mut app, size);
    let from = app.fx.purus.rail_at(50.0).unwrap();
    let to = app.fx.purus.rail_at(60.0).unwrap();
    drag(&mut app, size, from, to);
    assert_eq!(value(&app, "hhz"), 50.0, "off: the fundamental untouched");
}

#[test]
fn the_room_s_tail_drags_the_reduction_and_the_early_mark_each_one_undo_step() {
    let (mut app, size) = open(&[("hon", 0.0), ("von", 1.0)]);
    assert_eq!((value(&app, "vamount"), value(&app, "vkeep")), (12.0, 50.0));
    let g = app.fx.purus.tail.expect("the tail is drawn");
    // Anywhere on the tail, away from the mark: the reduction to the depth under the
    // pointer (24 dB is four tenths of the way down its 60 dB).
    let x = g.x_of(g.span_s * 0.6);
    drag(
        &mut app,
        size,
        egui::pos2(x, g.y_of(-12.0)),
        egui::pos2(x, g.y_of(-24.0)),
    );
    assert_eq!(value(&app, "vamount"), 24.0, "dragged to 24 dB");
    assert_eq!(value(&app, "vkeep"), 50.0, "the mark untouched");
    app.undo();
    assert_eq!(value(&app, "vamount"), 12.0, "one drag, one undo step");
    // The mark where the early reflections end, from 50 ms to 120 ms.
    settle(&mut app, size);
    let g = app.fx.purus.tail.unwrap();
    let y = g.y_of(-50.0);
    drag(
        &mut app,
        size,
        egui::pos2(g.x_of(0.05), y),
        egui::pos2(g.x_of(0.12), y),
    );
    assert!(
        (value(&app, "vkeep") - 120.0).abs() <= 2.0,
        "the early reflections kept to 120 ms: {}",
        value(&app, "vkeep")
    );
    assert_eq!(value(&app, "vamount"), 12.0, "the reduction untouched");
    app.undo();
    assert_eq!(value(&app, "vkeep"), 50.0, "one drag, one undo step");
    // With the room module off the tail takes no drag.
    set(&mut app, "von", 0.0);
    settle(&mut app, size);
    drag(
        &mut app,
        size,
        egui::pos2(x, g.y_of(-12.0)),
        egui::pos2(x, g.y_of(-24.0)),
    );
    assert_eq!(value(&app, "vamount"), 12.0, "off: the reduction untouched");
}

//! Caelum's editor answers the pointer as its picture promises (ADR 0138, phase
//! B), driven frame by frame with real pointer events through the window the app draws:
//! the astrolabe's alidade dragged turns the first voice to the semitone under the pointer
//! and the slimmer hand the second (the angle the note in the octave, the distance from the
//! centre the turn of the spiral), each drag one undo step; a click on the plate puts the
//! nearer hand where it falls; the decay rack's dotted line follows a drag up or down (one
//! undo step) and takes none while infinite. The sill's pushbuttons are `fxa_whole`'s
//! (`caelum_s_sill_buttons_throw_the_endless_settings`). // fxa:caelum

use cantus_core::*;
use cantus_engine::EngineHandle;
use cantus_ui::raster::shoot_events;
use cantus_ui::{views, welcome_project, CantusApp};
use egui::{Event, PointerButton, Pos2};

const KIND: EffectKind = EffectKind::Shimmer;

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
    app.demo_editor("caelum");
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

fn voice(app: &CantusApp, st: f32) -> Pos2 {
    app.fx
        .caelum
        .voice_point(st)
        .expect("the astrolabe is drawn")
}

#[test]
fn the_alidade_and_the_slimmer_hand_turn_the_voices_each_drag_one_undo_step() {
    let (mut app, size) = open();
    assert_eq!((value(&app, "pa"), value(&app, "pb")), (12.0, 7.0));
    // The alidade from its jewel (+12) to the spiral's +19 (a fifth, an octave up).
    let (from, to) = (voice(&app, 12.0), voice(&app, 19.0));
    drag(&mut app, size, from, to);
    assert_eq!(
        value(&app, "pa"),
        19.0,
        "the first voice an octave and a fifth"
    );
    assert_eq!(value(&app, "pb"), 7.0, "the second untouched");
    settle(&mut app, size);
    // The slimmer hand from its jewel (+7) down to −5 (a fourth below).
    let (from, to) = (voice(&app, 7.0), voice(&app, -5.0));
    drag(&mut app, size, from, to);
    assert_eq!(value(&app, "pb"), -5.0, "the second voice a fourth down");
    assert_eq!(value(&app, "pa"), 19.0, "the first untouched");
    app.undo();
    assert_eq!(value(&app, "pb"), 7.0, "one drag, one undo step");
    assert_eq!(value(&app, "pa"), 19.0);
    app.undo();
    assert_eq!(value(&app, "pa"), 12.0, "one drag, one undo step");

    // A click on the plate puts the nearer hand where it falls: just off the alidade's line
    // at the octave's −12 (the spiral's inner turn, straight up), the first voice an
    // octave down.
    settle(&mut app, size);
    let at = voice(&app, -12.0) + egui::vec2(1.5, 0.0);
    click(&mut app, size, at);
    assert_eq!(value(&app, "pa"), -12.0, "clicked to an octave down");
    app.undo();
    assert_eq!(value(&app, "pa"), 12.0, "one click, one undo step");
}

#[test]
fn the_decay_rack_follows_a_drag_and_takes_none_while_infinite() {
    let (mut app, size) = open();
    assert_eq!(value(&app, "decay"), 6.5, "Octave shimmer's 6.5 s");
    let (rack, y) = app.fx.caelum.decay_rack(6.5).expect("the rack is drawn");
    let (_, y12) = app.fx.caelum.decay_rack(12.0).unwrap();
    let x = rack.left() + rack.width() * 0.3;
    drag(&mut app, size, egui::pos2(x, y), egui::pos2(x, y12));
    assert_eq!(value(&app, "decay"), 12.0, "dragged to 12 s");
    settle(&mut app, size);
    let (_, y2) = app.fx.caelum.decay_rack(2.0).unwrap();
    drag(&mut app, size, egui::pos2(x, y12), egui::pos2(x, y2));
    assert_eq!(value(&app, "decay"), 2.0, "and down to 2 s");
    app.undo();
    assert_eq!(value(&app, "decay"), 12.0, "one drag, one undo step");
    app.undo();
    assert_eq!(value(&app, "decay"), 6.5, "one drag, one undo step");

    // While infinite the rack shows ∞ and takes no drag.
    let (strip, effect, _) = app.fx.open.unwrap();
    app.exec(Command::SetEffectParam {
        strip,
        effect,
        key: "inf".into(),
        value: 1.0,
    });
    settle(&mut app, size);
    drag(&mut app, size, egui::pos2(x, y), egui::pos2(x, y2));
    assert_eq!(value(&app, "decay"), 6.5, "infinite: the decay untouched");
}

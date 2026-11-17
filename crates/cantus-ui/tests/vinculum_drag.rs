//! Vinculum's editor answers the pointer as its faceplate promises (ADR 0142 provisional,
//! phase B), driven frame by frame with real pointer events through the window the app
//! draws: the model's stepped switches (a click on an engraved position chooses it, a click
//! on the knob steps it, a click on the bat throws it), the Auto lamp (lights Auto, gives
//! back the release it had), the limit lamp, and the external key's lamp (its list under it:
//! its own input or a strip of the desk). Each change is one undo step. // fxb:vinculum

use cantus_core::*;
use cantus_engine::EngineHandle;
use cantus_ui::raster::shoot_events;
use cantus_ui::{views, welcome_project, CantusApp};
use egui::{Event, PointerButton, Pos2};

const KIND: EffectKind = EffectKind::BusCompressor;

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
    app.demo_editor("vinculum");
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

/// Clicks at each of `at` in turn, in one context (an open list stays open between them):
/// each pressed and let go a frame apart, then the window settles.
fn clicks(app: &mut CantusApp, size: egui::Vec2, at: &[Pos2]) {
    let n = at.len();
    shoot_events(
        &ctx(),
        size,
        1.0,
        4 + 5 * n + 4,
        None,
        |f, _| {
            if f < 4 {
                return Vec::new();
            }
            let (i, phase) = ((f - 4) / 5, (f - 4) % 5);
            match (at.get(i), phase) {
                (Some(p), 0) => vec![Event::PointerMoved(*p)],
                (Some(p), 1) => vec![press(*p, true)],
                (Some(p), 2) => vec![press(*p, false)],
                _ => Vec::new(),
            }
        },
        |ui| {
            let c = ui.ctx().clone();
            views::fx::window(app, &c)
        },
    );
}

fn click(app: &mut CantusApp, size: egui::Vec2, at: Pos2) {
    clicks(app, size, &[at]);
    settle(app, size);
}

fn value(app: &CantusApp, key: &str) -> f32 {
    let (strip, effect, _) = app.fx.open.expect("open");
    app.project().effect(strip, effect).unwrap().spec.get(key)
}

/// The centre of position `i`'s engraved word on switch `key`.
fn position(app: &CantusApp, key: &str, i: usize) -> Pos2 {
    let (_, words) = app.fx.vinculum.switch(key).expect("the switch is drawn");
    words[i].center()
}

fn knob(app: &CantusApp, key: &str) -> Pos2 {
    app.fx
        .vinculum
        .switch(key)
        .expect("the switch is drawn")
        .0
        .center()
}

fn lamp(app: &CantusApp, word: &str) -> Pos2 {
    app.fx
        .vinculum
        .lamp(word)
        .expect("the lamp is drawn")
        .center()
}

#[test]
fn the_faceplate_s_switches_answer_a_click_each_one_undo_step() {
    let (mut app, size) = open();
    // The starting preset, Glue: the VCA at 4:1, 30 ms, Auto.
    assert_eq!(
        (
            value(&app, "model"),
            value(&app, "ratio"),
            value(&app, "att"),
            value(&app, "rel")
        ),
        (0.0, 1.0, 5.0, 4.0)
    );
    assert!(app.fx.vinculum.switch("time").is_none(), "the VCA's panel");
    // A click on the ratio's "10:1" chooses it.
    let at = position(&app, "ratio", 2);
    click(&mut app, size, at);
    assert_eq!(value(&app, "ratio"), 2.0, "10:1");
    // A click on the attack's knob steps it: 30 ms round to 0.1 ms.
    let at = knob(&app, "att");
    click(&mut app, size, at);
    assert_eq!(value(&app, "att"), 0.0, "stepped round to 0.1 ms");
    // A click on the release's "0.6" chooses it.
    let at = position(&app, "rel", 2);
    click(&mut app, size, at);
    assert_eq!(value(&app, "rel"), 2.0, "0.6 s");
    // A click on the model's "Vari-mu": the panel becomes the valve's.
    let at = position(&app, "model", 2);
    click(&mut app, size, at);
    assert_eq!(value(&app, "model"), 2.0, "vari-mu");
    assert!(app.fx.vinculum.switch("ratio").is_none(), "the VCA's gone");
    let at = position(&app, "time", 3);
    click(&mut app, size, at);
    assert_eq!(value(&app, "time"), 3.0, "the fourth time position");
    // The character's bat thrown: Limit.
    let at = knob(&app, "char");
    click(&mut app, size, at);
    assert_eq!(value(&app, "char"), 1.0, "Limit");
    // Each click one undo step, back to Glue.
    let undone = [
        ("char", 0.0),
        ("time", 1.0),
        ("model", 0.0),
        ("rel", 4.0),
        ("att", 5.0),
        ("ratio", 1.0),
    ];
    for (key, was) in undone {
        app.undo();
        assert_eq!(value(&app, key), was, "{key}: one click, one undo step");
    }
}

#[test]
fn the_auto_lamp_lights_auto_and_gives_back_the_release_it_had() {
    let (mut app, size) = open();
    assert_eq!(app.fx.vinculum.lamp_lit("vinculum.d.auto"), Some(true));
    // The release at 1.2 s from its switch: the lamp goes dark.
    let at = position(&app, "rel", 3);
    click(&mut app, size, at);
    assert_eq!(value(&app, "rel"), 3.0);
    assert_eq!(app.fx.vinculum.lamp_lit("vinculum.d.auto"), Some(false));
    // The lamp lights Auto…
    let at = lamp(&app, "vinculum.d.auto");
    click(&mut app, size, at);
    assert_eq!(value(&app, "rel"), 4.0, "Auto");
    assert_eq!(app.fx.vinculum.lamp_lit("vinculum.d.auto"), Some(true));
    // …and gives back the 1.2 s it had.
    let at = lamp(&app, "vinculum.d.auto");
    click(&mut app, size, at);
    assert_eq!(value(&app, "rel"), 3.0, "the release it had");
    app.undo();
    assert_eq!(value(&app, "rel"), 4.0, "one click, one undo step");
    app.undo();
    assert_eq!(value(&app, "rel"), 3.0, "one click, one undo step");
    app.undo();
    assert_eq!(value(&app, "rel"), 4.0, "Glue's Auto");
    // With nothing to give back, Auto's lamp puts the release at its 0.3 s.
    let at = lamp(&app, "vinculum.d.auto");
    click(&mut app, size, at);
    assert_eq!(value(&app, "rel"), 1.0, "0.3 s");
}

#[test]
fn the_limit_lamp_throws_the_character() {
    let (mut app, size) = open();
    assert!(
        app.fx.vinculum.lamp("vinculum.d.limit").is_none(),
        "no limit on the VCA"
    );
    let at = position(&app, "model", 1);
    click(&mut app, size, at);
    assert_eq!(value(&app, "model"), 1.0, "the opto");
    assert!(
        app.fx.vinculum.lamp("vinculum.d.auto").is_none(),
        "no Auto on the opto"
    );
    let at = lamp(&app, "vinculum.d.limit");
    click(&mut app, size, at);
    assert_eq!(value(&app, "char"), 1.0, "Limit");
    assert_eq!(app.fx.vinculum.lamp_lit("vinculum.d.limit"), Some(true));
    let at = lamp(&app, "vinculum.d.limit");
    click(&mut app, size, at);
    assert_eq!(value(&app, "char"), 0.0, "Compress");
    app.undo();
    assert_eq!(value(&app, "char"), 1.0, "one click, one undo step");
}

#[test]
fn the_key_lamp_opens_the_key_s_list_and_routes_a_strip() {
    let (mut app, size) = open();
    let (strip, effect, _) = app.fx.open.unwrap();
    let sidechain = |app: &CantusApp| app.project().effect(strip, effect).unwrap().sidechain;
    assert_eq!(sidechain(&app), None);
    assert_eq!(app.fx.vinculum.lamp_lit("vinculum.d.key"), Some(false));
    // A click on the lamp opens its list (its own input, then the desk's strips); a click
    // on the second row routes that strip as the key.
    let at = lamp(&app, "vinculum.d.key");
    let mut chosen = None;
    for _ in 0..2 {
        clicks(&mut app, size, &[at]);
        let (list, rows) = app.fx.vinculum.key_menu().expect("the key's list is open");
        assert!(rows >= 2, "its own input and at least one strip");
        let row = egui::pos2(
            list.center().x,
            list.top() + list.height() * 1.5 / rows as f32,
        );
        clicks(&mut app, size, &[at, row]);
        chosen = sidechain(&app);
        if chosen.is_some() {
            break;
        }
    }
    let key = chosen.expect("a strip chosen from the lamp's list");
    assert_ne!(key, strip);
    assert!(
        app.fx.vinculum.key_menu().is_none(),
        "the list closed on the choice"
    );
    settle(&mut app, size);
    assert_eq!(
        app.fx.vinculum.lamp_lit("vinculum.d.key"),
        Some(true),
        "the lamp lit"
    );
    app.undo();
    assert_eq!(sidechain(&app), None, "one choice, one undo step");
    settle(&mut app, size);
    assert_eq!(app.fx.vinculum.lamp_lit("vinculum.d.key"), Some(false));
    // Open, then a click elsewhere on the faceplate closes the list and changes nothing.
    clicks(&mut app, size, &[at]);
    assert!(app.fx.vinculum.key_menu().is_some());
    let away = app.fx.vinculum.display.expect("drawn").center();
    clicks(&mut app, size, &[at, away]);
    assert!(
        app.fx.vinculum.key_menu().is_none(),
        "closed by a click elsewhere"
    );
    assert_eq!(sidechain(&app), None);
}

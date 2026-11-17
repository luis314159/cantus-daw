//! Loquax's editor answers the pointer as its picture promises (ADR 0136, phase
//! B), driven frame by frame with real pointer events through the window the app draws:
//! a stroke dragged across the bank's screen draws the band levels (`b0` … `b63`) — the
//! band it starts on at the level it was pressed at, the one it ends on at the level it
//! ends at, those between along the stroke, the others untouched — as one undo step; the ⋯
//! menu chooses the object the voice speaks into (the insert's own `obj`, one undo step)
//! and a colourway (the user's, kept in the settings); the carrier's and the roles' switches
//! are thrown by a click, one undo step each. // fxa:loquax

use cantus_core::*;
use cantus_engine::EngineHandle;
use cantus_ui::raster::shoot_events;
use cantus_ui::{views, welcome_project, CantusApp};
use egui::{Event, PointerButton, Pos2};
use std::cell::RefCell;
use std::rc::Rc;

const KIND: EffectKind = EffectKind::Vocoder;

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
    app.demo_editor("loquax");
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

/// A click at `at`: pressed and let go a frame apart (a long press is no click).
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

/// The ⋯ button clicked, then — once its menu is drawn — the row `pick` gives (from the
/// rows as drawn), in one run of frames.
fn menu_pick(app: &mut CantusApp, size: egui::Vec2, pick: impl Fn(&[egui::Rect]) -> usize) {
    let more = app
        .fx
        .loquax
        .chrome
        .more_button
        .expect("the ⋯ button is drawn");
    let rows: Rc<RefCell<Vec<egui::Rect>>> = Rc::default();
    let seen = rows.clone();
    shoot_events(
        &ctx(),
        size,
        1.0,
        16,
        None,
        |f, _| {
            let row = || {
                let r = seen.borrow();
                assert!(!r.is_empty(), "the menu is drawn");
                r[pick(&r)].center()
            };
            match f {
                2 => vec![Event::PointerMoved(more.center())],
                3 => vec![press(more.center(), true)],
                4 => vec![press(more.center(), false)],
                9 => vec![Event::PointerMoved(row())],
                10 => vec![press(row(), true)],
                11 => vec![press(row(), false)],
                _ => Vec::new(),
            }
        },
        |ui| {
            let c = ui.ctx().clone();
            views::fx::window(app, &c);
            let now = &app.fx.loquax.chrome.menu_rows;
            if !now.is_empty() {
                *rows.borrow_mut() = now.clone();
            }
        },
    );
}

fn value(app: &CantusApp, key: &str) -> f32 {
    let (strip, effect, _) = app.fx.open.expect("open");
    app.project().effect(strip, effect).unwrap().spec.get(key)
}

/// Band `i`'s column on the plot, and the height of `db` there, as the display draws them.
fn band_x(app: &CantusApp, plot: egui::Rect, i: usize) -> f32 {
    let (lo, hi, width) = (value(app, "lo"), value(app, "hi"), value(app, "width"));
    let n = value(app, "bands").round() as usize;
    let (c, _, _) = cantus_dsp::loquax::bank::layout(i, n, lo, hi, width);
    let (f0, f1) = (lo / 1.25, hi * 1.25);
    plot.min.x + plot.width() * (c / f0).log2() / (f1 / f0).log2()
}

fn level_y(plot: egui::Rect, db: f32) -> f32 {
    plot.max.y - (db + 24.0) / 36.0 * plot.height()
}

#[test]
fn a_stroke_across_the_bank_draws_the_band_levels_in_one_undo_step() {
    let (mut app, size) = open();
    assert_eq!(value(&app, "bands"), 16.0, "Robot's sixteen bands");
    let plot = app.fx.loquax.plot().expect("the bank is drawn");
    let from = egui::pos2(band_x(&app, plot, 2), level_y(plot, 6.0));
    let to = egui::pos2(band_x(&app, plot, 9), level_y(plot, -12.0));
    drag(&mut app, size, from, to);
    let levels: Vec<f32> = (0..16)
        .map(|i| value(&app, cantus_core::fx::loquax::BAND_KEYS[i]))
        .collect();
    assert!(
        (levels[2] - 6.0).abs() <= 0.5,
        "pressed at +6 dB: {levels:?}"
    );
    assert!(
        (levels[9] + 12.0).abs() <= 0.5,
        "let go at -12 dB: {levels:?}"
    );
    for w in levels[2..10].windows(2) {
        assert!(w[1] <= w[0] + 0.5, "along the stroke, falling: {levels:?}");
    }
    for i in [0, 1, 10, 15] {
        assert_eq!(levels[i], 0.0, "band {i} untouched: {levels:?}");
    }
    app.undo();
    for (i, k) in cantus_core::fx::loquax::BAND_KEYS[..16].iter().enumerate() {
        assert_eq!(value(&app, k), 0.0, "one stroke, one undo step (band {i})");
    }
}

#[test]
fn the_menu_chooses_the_object_and_the_colourway() {
    let (mut app, size) = open();
    assert_eq!(value(&app, "obj"), 0.0, "the gooseneck, the default");
    // The ⋯ menu's last rows: the object's band and its three, the colourways' band and its
    // three, a separator and Close.
    let object = |i: usize| move |r: &[egui::Rect]| r.len() - 10 + 1 + i;
    menu_pick(&mut app, size, object(2));
    assert_eq!(value(&app, "obj"), 2.0, "the desk microphone chosen");
    settle(&mut app, size);
    menu_pick(&mut app, size, object(1));
    assert_eq!(value(&app, "obj"), 1.0, "the talk-box's tube chosen");
    app.undo();
    assert_eq!(value(&app, "obj"), 2.0, "one choice, one undo step");

    settle(&mut app, size);
    menu_pick(&mut app, size, |r| r.len() - 10 + 5 + 1);
    assert_eq!(
        app.settings
            .unit_colourways
            .values()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["ink"],
        "the colourway is the user's, in the settings"
    );
    assert_eq!(
        value(&app, "obj"),
        2.0,
        "the object is the insert's, untouched"
    );
    cantus_ui::kit::skin::set_colourways(std::iter::empty());
}

#[test]
fn the_carrier_and_the_roles_are_thrown_by_a_click() {
    let (mut app, size) = open();
    assert_eq!(value(&app, "src"), 1.0, "the synthesiser");
    let seg = |key: &str, i: usize, n: usize| {
        let r = views::fx::controls::segmented_rect(key).expect("the switch is drawn");
        egui::pos2(
            r.left() + r.width() * (i as f32 + 0.5) / n as f32,
            r.center().y,
        )
    };
    let at = seg("src", 0, 3);
    click(&mut app, size, at);
    assert_eq!(value(&app, "src"), 0.0, "the sidechain");
    settle(&mut app, size);
    let at = seg("src", 2, 3);
    click(&mut app, size, at);
    assert_eq!(value(&app, "src"), 2.0, "both");
    app.undo();
    assert_eq!(value(&app, "src"), 0.0, "one click, one undo step");

    settle(&mut app, size);
    let at = seg("swap", 1, 2);
    click(&mut app, size, at);
    assert_eq!(
        value(&app, "swap"),
        1.0,
        "the sidechain speaks, this track carries"
    );
    app.undo();
    assert_eq!(value(&app, "swap"), 0.0, "one click, one undo step");
}

//! Fractus's editor answers the pointer as its picture promises (ADR 0137, phase
//! B), driven frame by frame with real pointer events through the window the app draws:
//! a block painted on a lane — pressed on a step, dragged along to hold, as tall as the
//! pointer's height (its amount) — and a Shift stroke setting the chance of the blocks it
//! passes, each one undo step; a right-click erasing a block, one undo step; the page keys
//! and a lane's name opening pages (a view, no undo step); the Frozen lamp freezing the
//! cloud and the pitch jump's Style switch, one undo step each. // fxa:fractus

use cantus_core::fx::fractus::{Block, Pattern};
use cantus_core::*;
use cantus_engine::EngineHandle;
use cantus_ui::raster::shoot_events;
use cantus_ui::views::fx::fractus::paint;
use cantus_ui::{views, welcome_project, CantusApp};
use egui::{Event, Modifiers, PointerButton, Pos2};

const KIND: EffectKind = EffectKind::Glitch;

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
    app.demo_editor("fractus");
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

fn press(p: Pos2, button: PointerButton, pressed: bool, modifiers: Modifiers) -> Event {
    Event::PointerButton {
        pos: p,
        button,
        pressed,
        modifiers,
    }
}

/// Press at `from` (with `modifiers`), move to `to` in steps, let go — in a settled context
/// of its own.
fn drag(app: &mut CantusApp, size: egui::Vec2, from: Pos2, to: Pos2, modifiers: Modifiers) {
    let (steps, q) = (12, 4);
    let b = PointerButton::Primary;
    shoot_events(
        &ctx(),
        size,
        1.0,
        q + steps + 6,
        None,
        |f, _| match f {
            f if f < q => Vec::new(),
            f if f == q => vec![Event::PointerMoved(from)],
            f if f == q + 1 => vec![press(from, b, true, modifiers)],
            f if f < q + steps + 2 => {
                let t = (f - q - 1) as f32 / steps as f32;
                vec![Event::PointerMoved(from + (to - from) * t)]
            }
            f if f == q + steps + 2 => vec![press(to, b, false, modifiers)],
            _ => Vec::new(),
        },
        |ui| {
            let c = ui.ctx().clone();
            views::fx::window(app, &c)
        },
    );
}

/// A click of `button` at `at`: pressed and let go a frame apart.
fn click_with(app: &mut CantusApp, size: egui::Vec2, at: Pos2, button: PointerButton) {
    let m = Modifiers::default();
    shoot_events(
        &ctx(),
        size,
        1.0,
        10,
        None,
        |f, _| match f {
            2 => vec![Event::PointerMoved(at)],
            3 => vec![press(at, button, true, m)],
            4 => vec![press(at, button, false, m)],
            _ => Vec::new(),
        },
        |ui| {
            let c = ui.ctx().clone();
            views::fx::window(app, &c)
        },
    );
}

fn click(app: &mut CantusApp, size: egui::Vec2, at: Pos2) {
    click_with(app, size, at, PointerButton::Primary);
}

fn spec(app: &CantusApp) -> EffectSpec {
    let (strip, effect, _) = app.fx.open.expect("open");
    app.project().effect(strip, effect).unwrap().spec.clone()
}

fn lane(app: &CantusApp, lane: usize) -> Vec<Block> {
    spec(app)
        .fractus
        .as_deref()
        .map(|p| p.lane(lane).to_vec())
        .unwrap_or_default()
}

fn laid(app: &CantusApp) -> views::fx::fractus::display::Laid {
    app.fx.fractus.stroke.laid.expect("the lanes are drawn")
}

/// Step `step` of lane `lane` at the height that reads `amount`.
fn at(app: &CantusApp, lane: usize, step: usize, amount: f32) -> Pos2 {
    let l = laid(app);
    egui::pos2(paint::step_x(&l, step), paint::amount_y(&l, lane, amount))
}

#[test]
fn a_block_is_painted_by_a_drag_its_height_the_amount_in_one_undo_step() {
    let _theme_turn = theme_turn();
    let (mut app, size) = open();
    assert_eq!(laid(&app).steps, 16, "a bar of sixteenths");
    let before: Pattern = spec(&app).fractus.as_deref().cloned().unwrap();
    assert!(lane(&app, 3).is_empty(), "the shuffle's lane is empty");
    let (from, to) = (at(&app, 3, 2, 0.75), at(&app, 3, 6, 0.75));
    drag(&mut app, size, from, to, Modifiers::default());
    assert_eq!(
        lane(&app, 3),
        vec![Block {
            amount: 0.75,
            ..Block::new(2, 5)
        }],
        "steps 2 to 6, three quarters high"
    );
    for l in [0, 1, 6, 7, 8, 9] {
        assert_eq!(lane(&app, l), before.lane(l), "lane {l} untouched");
    }
    // Dragged back over the start: the block runs from where the pointer is to the press.
    let (from, to) = (at(&app, 3, 12, 1.0), at(&app, 3, 10, 0.3));
    drag(&mut app, size, from, to, Modifiers::default());
    assert_eq!(
        lane(&app, 3),
        vec![
            Block {
                amount: 0.75,
                ..Block::new(2, 5)
            },
            Block {
                amount: 0.3,
                ..Block::new(10, 3)
            }
        ],
        "the height where it was let go"
    );
    app.undo();
    assert_eq!(lane(&app, 3).len(), 1, "one stroke, one undo step");
    app.undo();
    assert!(lane(&app, 3).is_empty(), "and the first");
    assert_eq!(spec(&app).fractus.as_deref(), Some(&before));
}

#[test]
fn a_shift_stroke_sets_the_chance_of_the_blocks_it_passes() {
    let _theme_turn = theme_turn();
    let (mut app, size) = open();
    // The filter sweep's block: steps 0..8 at 40 %.
    assert_eq!(
        lane(&app, 6),
        vec![Block {
            amount: 0.4,
            ..Block::new(0, 8)
        }]
    );
    let (from, to) = (at(&app, 6, 1, 0.5), at(&app, 6, 7, 0.5));
    drag(&mut app, size, from, to, Modifiers::SHIFT);
    assert_eq!(
        lane(&app, 6),
        vec![Block {
            amount: 0.4,
            prob: 0.5,
            ..Block::new(0, 8)
        }],
        "half the chance, the amount and the steps kept"
    );
    app.undo();
    assert_eq!(lane(&app, 6)[0].prob, 1.0, "one stroke, one undo step");
}

#[test]
fn a_right_click_erases_a_block() {
    let _theme_turn = theme_turn();
    let (mut app, size) = open();
    assert_eq!(lane(&app, 8).len(), 1, "the delay throw on step 6");
    let p = at(&app, 8, 6, 0.5);
    click_with(&mut app, size, p, PointerButton::Secondary);
    assert!(lane(&app, 8).is_empty(), "erased");
    // On an empty step nothing changes (and nothing goes in the history).
    let p = at(&app, 8, 12, 0.5);
    click_with(&mut app, size, p, PointerButton::Secondary);
    app.undo();
    assert_eq!(lane(&app, 8).len(), 1, "one undo step brings it back");
}

#[test]
fn the_page_keys_and_a_lane_s_name_open_pages_and_change_no_sound() {
    let _theme_turn = theme_turn();
    let (mut app, size) = open();
    let project = app.project().clone();
    assert_eq!(app.fx.fractus.page, 0, "the sequencer's page");
    let key = app.fx.fractus.page_keys[3]
        .expect("the page key is drawn")
        .center();
    click(&mut app, size, key);
    assert_eq!(app.fx.fractus.page, 3, "the grain cloud's page");
    settle(&mut app, size);
    let key = app.fx.fractus.page_keys[2]
        .expect("the page key is drawn")
        .center();
    click(&mut app, size, key);
    assert_eq!(app.fx.fractus.page, 2, "the colour page");
    settle(&mut app, size);
    // The repeat's name opens the slicing page; the throw's the sequencer's.
    let name = laid(&app).names[0].center();
    click(&mut app, size, name);
    assert_eq!(app.fx.fractus.page, 1, "the repeat's page");
    settle(&mut app, size);
    let name = laid(&app).names[8].center();
    click(&mut app, size, name);
    assert_eq!(app.fx.fractus.page, 0, "the throw's page");
    assert_eq!(*app.project(), project, "a page is a view: nothing to undo");
}

#[test]
fn freeze_and_the_style_switch_are_thrown_by_a_click_one_undo_step_each() {
    let _theme_turn = theme_turn();
    let (mut app, size) = open();
    assert_eq!(spec(&app).get("cfreeze"), 0.0);
    let lamp = app
        .fx
        .fractus
        .stroke
        .frozen
        .expect("the Frozen lamp")
        .center();
    click(&mut app, size, lamp);
    assert_eq!(spec(&app).get("cfreeze"), 1.0, "frozen");
    settle(&mut app, size);
    click(&mut app, size, lamp);
    assert_eq!(spec(&app).get("cfreeze"), 0.0, "thawed");
    app.undo();
    assert_eq!(spec(&app).get("cfreeze"), 1.0, "one click, one undo step");
    app.undo();
    assert_eq!(spec(&app).get("cfreeze"), 0.0);

    // The pitch jump's Style, on the colour page: the classic warble (the owner's default)
    // or clean.
    settle(&mut app, size);
    let key = app.fx.fractus.page_keys[2]
        .expect("the page key is drawn")
        .center();
    click(&mut app, size, key);
    settle(&mut app, size);
    assert_eq!(spec(&app).get("pstyle"), 0.0, "the classic warble");
    let r = views::fx::controls::segmented_rect("pstyle").expect("the Style switch is drawn");
    let clean = egui::pos2(r.left() + r.width() * 0.75, r.center().y);
    click(&mut app, size, clean);
    assert_eq!(spec(&app).get("pstyle"), 1.0, "clean");
    app.undo();
    assert_eq!(spec(&app).get("pstyle"), 0.0, "one click, one undo step");
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

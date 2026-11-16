//! Arca's and Rota's displays answer the pointer as their pictures promise (ADRs 0136 and
//! 0137, provisional), driven frame by frame with real pointer events: the microphone
//! dragged across Arca's cone from where it is to near the edge moves `micpos` there; dragged
//! in the side view to the 10 cm mark it moves `micdist` there; a click on Rota's "Fast"
//! throws the switch; Rota's right microphone dragged further round widens their angle and
//! away from the cabinet moves them back. Each gesture is one undo step. // fx3:arca

use cantus_core::*;
use cantus_engine::EngineHandle;
use cantus_ui::raster::shoot_events;
use cantus_ui::{views, welcome_project, CantusApp};
use egui::{Event, PointerButton, Pos2};

fn ctx() -> egui::Context {
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    cantus_ui::theme::apply(&cantus_ui::theme::slate(), &ctx);
    ctx
}

fn open(unit: &str, kind: EffectKind) -> (CantusApp, egui::Context, egui::Vec2) {
    let ctx = ctx();
    let mut app = CantusApp::from_parts(Some(welcome_project()), EngineHandle::dummy(), false);
    app.demo_editor(unit);
    views::fx::demo_feed(&mut app, kind);
    app.fx.pos = Some(egui::pos2(0.0, 0.0));
    views::fx::frame::shell_of(kind).set_scale(&ctx, 1.0);
    let size = views::fx::frame::designed_size(kind);
    // Settle the window: its rects are known after it has been drawn.
    shoot_events(
        &ctx,
        size,
        1.0,
        6,
        None,
        |_, _| Vec::new(),
        |ui| {
            let c = ui.ctx().clone();
            views::fx::window(&mut app, &c)
        },
    );
    (app, ctx, size)
}

/// Press at `from`, move to `to` in `steps`, let go; then a few quiet frames.
fn drag(app: &mut CantusApp, ctx: &egui::Context, size: egui::Vec2, from: Pos2, to: Pos2) {
    let steps = 12;
    shoot_events(
        ctx,
        size,
        1.0,
        steps + 6,
        None,
        |f, _| match f {
            0 => vec![Event::PointerMoved(from)],
            1 => vec![Event::PointerButton {
                pos: from,
                button: PointerButton::Primary,
                pressed: true,
                modifiers: Default::default(),
            }],
            f if f < steps + 2 => {
                let t = (f - 1) as f32 / steps as f32;
                vec![Event::PointerMoved(from + (to - from) * t)]
            }
            f if f == steps + 2 => vec![Event::PointerButton {
                pos: to,
                button: PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            }],
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

#[test]
fn arca_s_microphone_is_dragged_on_the_cone_and_in_the_side_view() {
    let (mut app, ctx, size) = open("arca", EffectKind::Amp);
    let (c, rad) = app.fx.arca.cone().expect("the cone is drawn");
    let reach = rad * 0.80;
    let pos = value(&app, "micpos");
    let from = c + egui::vec2(pos * reach, 0.0);
    let to = c + egui::vec2(0.8 * reach, 0.0);
    drag(&mut app, &ctx, size, from, to);
    let moved = value(&app, "micpos");
    assert!((moved - 0.8).abs() <= 0.02, "micpos {pos} -> {moved}");
    app.undo();
    assert_eq!(value(&app, "micpos"), pos, "one gesture, one undo step");

    // The side view: dragged to the 10 cm mark (a log scale from 1 to 30 cm).
    let side = app.fx.arca.side_view().expect("the side view is drawn");
    let (a, z) = (side.left() + 70.0, side.right() - 74.0);
    let x_of = |cm: f32| a + (z - a) * (cm.ln() / 30f32.ln());
    let y = side.center().y + 4.0 - value(&app, "micpos") * side.height() * 0.36;
    let dist = value(&app, "micdist");
    drag(
        &mut app,
        &ctx,
        size,
        egui::pos2(x_of(dist), y),
        egui::pos2(x_of(10.0), y),
    );
    let moved = value(&app, "micdist");
    assert!((moved - 10.0).abs() <= 0.3, "micdist {dist} -> {moved}");
}

#[test]
fn rota_s_switch_is_thrown_and_its_microphones_dragged() {
    let (mut app, ctx, size) = open("rota", EffectKind::Rotary);
    assert_eq!(value(&app, "speed"), 1.0, "slow");
    let fast = app.fx.rota.speed_rects()[2].center();
    shoot_events(
        &ctx,
        size,
        1.0,
        6,
        None,
        |f, _| match f {
            0 => vec![Event::PointerMoved(fast)],
            1 => vec![Event::PointerButton {
                pos: fast,
                button: PointerButton::Primary,
                pressed: true,
                modifiers: Default::default(),
            }],
            2 => vec![Event::PointerButton {
                pos: fast,
                button: PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            }],
            _ => Vec::new(),
        },
        |ui| {
            let c = ui.ctx().clone();
            views::fx::window(&mut app, &c)
        },
    );
    assert_eq!(value(&app, "speed"), 2.0, "the switch thrown to fast");

    // The right microphone dragged further round and further out.
    let [left, right] = app.fx.rota.mics();
    let centre = Pos2::new((left.x + right.x) * 0.5, left.y - 40.0);
    let (spread, dist) = (value(&app, "spread"), value(&app, "dist"));
    let out = centre + (right - centre) * 1.25 + egui::vec2(25.0, -10.0);
    drag(&mut app, &ctx, size, right, out);
    assert!(
        value(&app, "spread") > spread,
        "wider: {spread} -> {}",
        value(&app, "spread")
    );
    assert!(
        value(&app, "dist") > dist,
        "further: {dist} -> {}",
        value(&app, "dist")
    );
    app.undo();
    assert_eq!(
        (value(&app, "spread"), value(&app, "dist")),
        (spread, dist),
        "one gesture, one undo step"
    );
}

//! Chorda's knobs dragged by the pointer, the way a hand does (round 38, the owner's report
//! "some of Chorda's knobs stick or are slow"): the editor as the screenshots draw it, each
//! knob pressed, moved 80 px up two pixels a frame, let go — every knob must travel the
//! same share of its range (80 px of the knob's 200 px travel), whatever it is.

use cantus_core::*;
use cantus_engine::EngineHandle;
use cantus_ui::raster::shoot_events;
use cantus_ui::{views, welcome_project, CantusApp};

/// The knobs' centres in the Play view at 100 % (`shoot_skins`' Chorda: 1040×680).
const KNOBS: &[(&str, f32, f32)] = &[
    ("hardness", 66.0, 447.0),
    ("dynamics", 145.0, 447.0),
    ("vel curve", 222.0, 447.0),
    ("mics", 343.0, 447.0),
    ("width", 421.0, 447.0),
    ("lid", 499.0, 447.0),
    ("sympathy", 620.0, 447.0),
    ("board", 698.0, 447.0),
    ("room", 776.0, 447.0),
    ("tilt", 897.0, 447.0),
    ("gain", 975.0, 447.0),
    ("sustain", 800.0, 316.0),
    ("dampers", 878.0, 316.0),
    ("soft", 956.0, 316.0),
];

fn app() -> (CantusApp, egui::Context, egui::Vec2) {
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    cantus_ui::theme::apply(&cantus_ui::theme::slate(), &ctx);
    let mut app = CantusApp::from_parts(Some(welcome_project()), EngineHandle::dummy(), false);
    app.demo_editor("chorda");
    views::inst::demo_feed(&mut app, InstrumentKind::Chorda);
    app.fx.pos = Some(egui::pos2(0.0, 0.0));
    let t = app.inst.open.expect("Chorda's editor open");
    cantus_ui::kit::shell::Shell::new(("inst", t.0)).set_scale(&ctx, 1.0);
    (app, ctx, cantus_ui::kit::shell::INSTRUMENT)
}

fn spec(app: &CantusApp) -> NativeSpec {
    let t = app.inst.open.unwrap();
    app.project()
        .track(t)
        .and_then(|t| t.instrument.native())
        .cloned()
        .unwrap()
}

/// What moved (key, before, after, share of its range) when the knob at `at` was dragged
/// `px` up at `step` px a frame.
fn drag(at: egui::Pos2, px: f32, step: f32) -> Vec<(String, f32, f32, f32)> {
    let (mut app, ctx, size) = app();
    // Settle the window first.
    shoot_events(
        &ctx,
        size,
        1.0,
        6,
        None,
        |_, _| Vec::new(),
        |ui| views::inst::window_in(&mut app, ui),
    );
    let before = spec(&app);
    let moves = (px / step).round() as usize;
    shoot_events(
        &ctx,
        size,
        1.0,
        moves + 6,
        None,
        |f, _| {
            use egui::{Event, PointerButton};
            let y = |k: usize| at.y - step * k.min(moves) as f32;
            match f {
                0 => vec![Event::PointerMoved(at)],
                1 => vec![Event::PointerButton {
                    pos: at,
                    button: PointerButton::Primary,
                    pressed: true,
                    modifiers: Default::default(),
                }],
                f if f < moves + 2 => vec![Event::PointerMoved(egui::pos2(at.x, y(f - 1)))],
                f if f == moves + 2 => vec![Event::PointerButton {
                    pos: egui::pos2(at.x, y(moves)),
                    button: PointerButton::Primary,
                    pressed: false,
                    modifiers: Default::default(),
                }],
                _ => Vec::new(),
            }
        },
        |ui| views::inst::window_in(&mut app, ui),
    );
    let after = spec(&app);
    let mut out = Vec::new();
    for p in InstrumentKind::Chorda.params() {
        let (a, b) = (before.get(p.key), after.get(p.key));
        if a != b {
            out.push((p.key.to_string(), a, b, (b - a) / (p.max - p.min)));
        }
    }
    out
}

#[test]
#[ignore]
fn measure_every_knob_dragged() {
    for (step, what) in [(2.0, "slow, 2 px a frame"), (8.0, "quick, 8 px a frame")] {
        eprintln!("== {what}: 80 px up, 0.40 of the range expected");
        for (name, x, y) in KNOBS {
            let moved = drag(egui::pos2(*x, *y), 80.0, step);
            let line: Vec<String> = moved
                .iter()
                .map(|(k, a, b, s)| format!("{k} {a:.3} → {b:.3} ({s:+.2})"))
                .collect();
            eprintln!(
                "{name:10} {}",
                if line.is_empty() {
                    "NOTHING MOVED".into()
                } else {
                    line.join(", ")
                }
            );
        }
    }
}

#[test]
#[ignore]
fn shoot_the_edit_view() {
    let (mut app, ctx, size) = app();
    app.inst.view = views::inst::View::Edit;
    let out = std::path::Path::new("F:/projects/daw-r38/shots/chorda-edit.png");
    cantus_ui::raster::shoot(&ctx, size, 1.0, 8, Some(out), |ui| {
        views::inst::window_in(&mut app, ui)
    });
}

//! Culmen's editor: what it paints, what its gestures change (one undo step each), and
//! *Learn drive* closing the loop against the real DSP until the output reads the target.

mod support;
use cantus_core::*;
use cantus_dsp::culmen::Culmen;
use cantus_ui::views::fx::maximizer::{self, Outcome};
use cantus_ui::CantusApp;
use support::*;

/// An app with Culmen on the first strip, its editor open.
fn with_culmen() -> (CantusApp, StripId, EffectId) {
    let mut a = app(cantus_ui::starter_project());
    let s0 = a.project().tracks[0].strip.unwrap();
    a.add_effect(s0, EffectKind::Maximizer);
    let fx = a.project().strip(s0).unwrap().effects[0].id;
    a.open_effect_editor(s0, fx);
    (a, s0, fx)
}

fn spec(a: &CantusApp, s: StripId) -> EffectSpec {
    a.project().strip(s).unwrap().effects[0].spec.clone()
}

fn frame(a: &mut CantusApp, ctx: &egui::Context, events: Vec<egui::Event>) {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1400.0, 900.0),
        )),
        events,
        ..Default::default()
    };
    ctx.run_ui(input, |ui| cantus_ui::views::fx::window(a, ui.ctx()))
        .textures_delta
        .clear();
}

/// Press at `from`, move to `to` in steps, release.
fn drag(a: &mut CantusApp, from: egui::Pos2, to: egui::Pos2) {
    let ctx = ctx();
    let button = |pos, pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    frame(a, &ctx, vec![]);
    frame(a, &ctx, vec![]);
    frame(a, &ctx, vec![egui::Event::PointerMoved(from)]);
    frame(
        a,
        &ctx,
        vec![egui::Event::PointerMoved(from), button(from, true)],
    );
    for i in 1..=4 {
        let at = from + (to - from) * (i as f32 / 4.0);
        frame(a, &ctx, vec![egui::Event::PointerMoved(at)]);
    }
    frame(
        a,
        &ctx,
        vec![egui::Event::PointerMoved(to), button(to, false)],
    );
    frame(a, &ctx, vec![]);
}

/// A click (or two, for a double-click) at `at`.
fn click(a: &mut CantusApp, at: egui::Pos2, times: usize) {
    let ctx = ctx();
    let button = |pressed| egui::Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    frame(a, &ctx, vec![]);
    frame(a, &ctx, vec![egui::Event::PointerMoved(at)]);
    for _ in 0..times {
        frame(a, &ctx, vec![button(true)]);
        frame(a, &ctx, vec![button(false)]);
    }
    frame(a, &ctx, vec![]);
}

fn geometry(a: &mut CantusApp) -> maximizer::Geometry {
    let ctx = ctx();
    frame(a, &ctx, vec![]);
    frame(a, &ctx, vec![]);
    a.fx.maximizer.geometry
}

#[test]
fn the_editor_paints_every_stage_and_the_loudness() {
    let (mut a, s0, fx) = with_culmen();
    cantus_ui::views::fx::demo_setup(&mut a, s0, fx, EffectKind::Maximizer);
    cantus_ui::views::fx::demo_feed(&mut a, EffectKind::Maximizer);
    assert!(
        a.fx.maximizer.history.len() > 500,
        "the demo fed the history"
    );
    let text = paint_editors(&mut a, egui::vec2(1400.0, 900.0));
    let lower = text.to_lowercase().replace(' ', "");
    for want in [
        "Drive",
        "Ceiling",
        "Integrated",
        "Momentary",
        "PLR",
        "Low",
        "Mid",
        "High",
        "Full",
        "Learn drive",
        "Streaming −14",
        "Dynamic",
        "Linear phase",
        "Above",
        "1.5:1",
        "Brickwall",
        "Punch",
        "Gain over time",
    ] {
        let want = want.replace(' ', "").to_lowercase();
        assert!(lower.contains(&want), "missing {want:?} in {text}");
    }
    // The integrated loudness of the demo is on screen, as a number.
    let i = a.fx.maximizer.integrated().expect("measured");
    assert!((-30.0..-3.0).contains(&i), "{i}");
}

#[test]
fn dragging_the_ceiling_line_moves_the_ceiling_in_one_undo_step() {
    let (mut a, s0, _) = with_culmen();
    let g = geometry(&mut a);
    let from = egui::pos2(g.history.center().x, g.history_y(-1.0));
    let to = egui::pos2(from.x, g.history_y(-6.0));
    drag(&mut a, from, to);
    let c = spec(&a, s0).get("ceiling");
    assert!((c + 6.0).abs() < 0.3, "the ceiling went to {c}");
    a.undo();
    assert_eq!(spec(&a, s0).get("ceiling"), -1.0);
}

#[test]
fn a_split_is_dragged_on_the_strip_and_a_band_is_clicked_to_edit() {
    let (mut a, s0, _) = with_culmen();
    let g = geometry(&mut a);
    let y = g.strip.center().y;
    drag(
        &mut a,
        egui::pos2(g.strip_x(150.0), y),
        egui::pos2(g.strip_x(300.0), y),
    );
    let x1 = spec(&a, s0).get("x1");
    assert!((x1 / 300.0 - 1.0).abs() < 0.08, "the split went to {x1} Hz");
    a.undo();
    assert_eq!(spec(&a, s0).get("x1"), 150.0);
    // A click on the mid band selects it; a double-click solos it.
    let mid = egui::pos2(g.strip_x(1_000.0), y);
    click(&mut a, mid, 1);
    assert_eq!(a.fx.maximizer.selected, 1);
    click(&mut a, mid, 2);
    assert_eq!(spec(&a, s0).get("mid.solo"), 1.0);
    // The full band's column selects the full band.
    let full = a.fx.maximizer.geometry.full.center();
    click(&mut a, full, 1);
    assert_eq!(a.fx.maximizer.selected, 3);
}

#[test]
fn the_curve_is_dragged_by_its_threshold_and_its_slope() {
    let (mut a, s0, _) = with_culmen();
    // The full band is selected by default: its threshold is at −12 dB, ratio 1:1.
    let g = geometry(&mut a);
    drag(&mut a, g.curve_pos(-12.0, -12.0), g.curve_pos(-24.0, -24.0));
    let thr = spec(&a, s0).get("all.thr");
    assert!((thr + 24.0).abs() < 0.6, "the threshold went to {thr}");
    a.undo();
    assert_eq!(spec(&a, s0).get("all.thr"), -12.0);
    // Pull the side above the threshold down to half its slope: 2:1.
    let g = geometry(&mut a);
    drag(&mut a, g.curve_pos(-4.0, -4.0), g.curve_pos(-4.0, -8.0));
    let ratio = spec(&a, s0).get("all.ratio");
    assert!((ratio - 2.0).abs() < 0.25, "the ratio went to {ratio}");
    a.undo();
    assert_eq!(spec(&a, s0).get("all.ratio"), 1.0);
}

/// *Learn drive* against the real DSP: each frame the editor reads what Culmen measured,
/// and Culmen runs with what the editor set. It ends on the target, as one undo step.
#[test]
fn learning_closes_the_loop_on_the_target_in_one_undo_step() {
    let (mut a, s0, _) = with_culmen();
    let ctx = ctx();
    let sr = 48_000.0;
    let (l, r) = maximizer::demo_signal(40.0, sr);
    let mut c = Culmen::new(sr);
    c.set(&cantus_engine::fx::maximizer::settings(&spec(&a, s0)));
    c.reset();
    // Feed one frame so the editor knows the stream, then press Learn.
    let per_frame = 2_400; // 50 ms: inside the telemetry's 60 ms ring
    let mut at = 0;
    let step = |a: &mut CantusApp, c: &mut Culmen, at: &mut usize| {
        let (mut x, mut y) = (
            l[*at..*at + per_frame]
                .iter()
                .map(|v| v * 0.3)
                .collect::<Vec<f32>>(),
            r[*at..*at + per_frame]
                .iter()
                .map(|v| v * 0.3)
                .collect::<Vec<f32>>(),
        );
        *at += per_frame;
        c.set(&cantus_engine::fx::maximizer::settings(&spec(a, s0)));
        c.process(&mut x, &mut y);
        a.fx.injected = Some(c.telemetry());
        frame(a, &ctx, vec![]);
    };
    step(&mut a, &mut c, &mut at);
    let before = spec(&a, s0).get("gain");
    a.fx.maximizer.learn = Some(maximizer::Learn::new(-14.0, before));
    while at + per_frame <= l.len() && a.fx.maximizer.learn.is_some() {
        step(&mut a, &mut c, &mut at);
    }
    match a.fx.maximizer.learned {
        Some(Outcome::Done { drive, lufs }) => {
            assert!((lufs + 14.0).abs() <= 0.3, "it stopped at {lufs} LUFS");
            assert_eq!(spec(&a, s0).get("gain"), drive);
            assert!(drive > before + 3.0, "the quiet demo needed drive: {drive}");
        }
        other => panic!("learning ended as {other:?}"),
    }
    a.undo();
    assert_eq!(
        spec(&a, s0).get("gain"),
        before,
        "one undo takes it all back"
    );
}

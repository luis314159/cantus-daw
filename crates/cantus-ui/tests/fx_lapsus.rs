//! Lapsus's editor (fx side round, ADR 0132 provisional), driven by pointer events: a pad
//! of each bank chooses its pattern (one undo step), a right-click gives a slot the
//! factory's pattern back; on the timeline a point dragged, a point added and taken away
//! with double-clicks, the pencil's steps — each one undo step on `SetLapsusPattern`; the
//! pattern card's length; the factory presets in both languages and the starting preset.

mod support;
use cantus_core::fx::lapsus::{self as model, Lane};
use cantus_core::*;
use cantus_ui::CantusApp;
use support::*;

fn with_lapsus() -> (CantusApp, StripId, EffectId) {
    let mut a = app(cantus_ui::starter_project());
    let s0 = a.project().tracks[0].strip.unwrap();
    a.add_effect(s0, EffectKind::TimeShaper);
    let fx = a.project().strip(s0).unwrap().effects[0].id;
    a.open_effect_editor(s0, fx);
    (a, s0, fx)
}

fn spec(a: &CantusApp, s: StripId) -> EffectSpec {
    a.project().strip(s).unwrap().effects[0].spec.clone()
}

fn pattern(a: &CantusApp, s: StripId, lane: Lane, slot: usize) -> model::Pattern {
    model::pattern_of(spec(a, s).lapsus.as_deref(), lane, slot).clone()
}

thread_local! {
    static NOW: std::cell::Cell<f64> = const { std::cell::Cell::new(0.0) };
}

fn frame(a: &mut CantusApp, ctx: &egui::Context, events: Vec<egui::Event>) {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1600.0, 1000.0),
        )),
        events,
        time: Some(NOW.with(|t| {
            let v = t.get() + 0.02;
            t.set(v);
            v
        })),
        ..Default::default()
    };
    ctx.run_ui(input, |ui| cantus_ui::views::fx::window(a, ui.ctx()))
        .textures_delta
        .clear();
}

fn button(pos: egui::Pos2, pressed: bool, which: egui::PointerButton) -> egui::Event {
    egui::Event::PointerButton {
        pos,
        button: which,
        pressed,
        modifiers: egui::Modifiers::NONE,
    }
}

fn click_with(a: &mut CantusApp, ctx: &egui::Context, at: egui::Pos2, which: egui::PointerButton) {
    NOW.with(|t| t.set(t.get() + 1.0));
    frame(a, ctx, vec![egui::Event::PointerMoved(at)]);
    frame(a, ctx, vec![button(at, true, which)]);
    frame(a, ctx, vec![button(at, false, which)]);
    frame(a, ctx, vec![]);
}

fn click(a: &mut CantusApp, ctx: &egui::Context, at: egui::Pos2) {
    click_with(a, ctx, at, egui::PointerButton::Primary);
}

fn double_click(a: &mut CantusApp, ctx: &egui::Context, at: egui::Pos2) {
    let p = egui::PointerButton::Primary;
    NOW.with(|t| t.set(t.get() + 1.0));
    frame(a, ctx, vec![egui::Event::PointerMoved(at)]);
    for _ in 0..2 {
        frame(a, ctx, vec![button(at, true, p)]);
        frame(a, ctx, vec![button(at, false, p)]);
    }
    frame(a, ctx, vec![]);
}

fn drag(a: &mut CantusApp, ctx: &egui::Context, from: egui::Pos2, to: egui::Pos2) {
    let p = egui::PointerButton::Primary;
    NOW.with(|t| t.set(t.get() + 1.0));
    frame(a, ctx, vec![egui::Event::PointerMoved(from)]);
    frame(
        a,
        ctx,
        vec![egui::Event::PointerMoved(from), button(from, true, p)],
    );
    for i in 1..=4 {
        let at = from + (to - from) * (i as f32 / 4.0);
        frame(a, ctx, vec![egui::Event::PointerMoved(at)]);
    }
    frame(
        a,
        ctx,
        vec![egui::Event::PointerMoved(to), button(to, false, p)],
    );
    frame(a, ctx, vec![]);
}

/// The lanes and the pads, after two frames.
fn places(a: &mut CantusApp, ctx: &egui::Context) -> (egui::Rect, egui::Rect, egui::Rect) {
    frame(a, ctx, vec![]);
    frame(a, ctx, vec![]);
    let (t, l) = a.fx.lapsus.lanes.expect("the timeline was drawn");
    (t, l, a.fx.lapsus.pads.expect("the pads were drawn"))
}

/// The centre of pad `i` of the 6 × 6 grid in `pads`.
fn pad(pads: egui::Rect, i: usize) -> egui::Pos2 {
    let (row, col) = (i / 6, i % 6);
    egui::pos2(
        pads.left() + pads.width() * (col as f32 + 0.5) / 6.0,
        pads.top() + pads.height() * (row as f32 + 0.5) / 6.0,
    )
}

#[test]
fn it_starts_on_its_starting_preset_and_a_pad_of_each_bank_chooses_its_pattern() {
    let (mut a, s0, _) = with_lapsus();
    let ctx = ctx();
    assert_eq!(
        spec(&a, s0).get("tslot"),
        7.0,
        "half time ½, the starting preset"
    );
    let (_, _, pads) = places(&mut a, &ctx);
    click(&mut a, &ctx, pad(pads, 3));
    assert_eq!(spec(&a, s0).get("tslot"), 3.0);
    a.undo();
    assert_eq!(spec(&a, s0).get("tslot"), 7.0, "one step");
    // The volume bank on the pads: its pads choose the volume pattern.
    a.fx.lapsus.bank = Lane::Vol;
    let (_, _, pads) = places(&mut a, &ctx);
    click(&mut a, &ctx, pad(pads, 12));
    let s = spec(&a, s0);
    assert_eq!((s.get("tslot"), s.get("vslot")), (7.0, 12.0));
}

#[test]
fn the_timeline_edits_the_playing_pattern_one_step_at_a_time() {
    let (mut a, s0, _) = with_lapsus();
    let ctx = ctx();
    let (time, _, _) = places(&mut a, &ctx);
    let before = pattern(&a, s0, Lane::Time, 7);
    assert_eq!(before.points.len(), 2, "half time: a line from 0 to a half");
    // The half-bar pattern is drawn twice across the bar: its end point (1, 0.5) sits at
    // the half bar and the bar's end. Drag the first one down to 0.75.
    let end = egui::pos2(
        time.left() + time.width() * 0.5,
        time.top() + time.height() * 0.5,
    );
    let to = egui::pos2(end.x, time.top() + time.height() * 0.75);
    drag(&mut a, &ctx, end, to);
    let p = pattern(&a, s0, Lane::Time, 7);
    assert_eq!(p.points.len(), 2);
    assert!((p.points[1].y - 0.75).abs() < 0.02, "{:?}", p.points);
    assert!(spec(&a, s0).lapsus.is_some(), "the bank holds the slot now");
    a.undo();
    assert_eq!(
        pattern(&a, s0, Lane::Time, 7),
        before,
        "the drag was one step"
    );
    assert!(
        spec(&a, s0).lapsus.is_none(),
        "and the bank is the factory's again"
    );
    // A double-click on an empty place adds a point; on it, takes it away.
    let at = egui::pos2(
        time.left() + time.width() * 0.125,
        time.top() + time.height() * 0.6,
    );
    double_click(&mut a, &ctx, at);
    assert_eq!(pattern(&a, s0, Lane::Time, 7).points.len(), 3);
    double_click(&mut a, &ctx, at);
    assert_eq!(pattern(&a, s0, Lane::Time, 7).points.len(), 2);
    // The pencil paints held steps, a grid cell each.
    a.fx.lapsus.tool = 1;
    let from = egui::pos2(time.left() + 4.0, time.top() + time.height() * 0.25);
    let to = egui::pos2(
        time.left() + time.width() * 0.2,
        time.top() + time.height() * 0.25,
    );
    drag(&mut a, &ctx, from, to);
    let p = pattern(&a, s0, Lane::Time, 7);
    assert!(
        p.points
            .iter()
            .any(|q| q.s == model::SHAPE_HOLD && (q.y - 0.25).abs() < 0.02),
        "{:?}",
        p.points
    );
    a.undo();
    assert_eq!(
        pattern(&a, s0, Lane::Time, 7).points.len(),
        2,
        "the stroke was one step"
    );
}

#[test]
fn a_right_click_gives_a_slot_the_factorys_pattern_back() {
    let (mut a, s0, fx) = with_lapsus();
    let ctx = ctx();
    a.exec(Command::SetLapsusPattern {
        strip: s0,
        effect: fx,
        lane: Lane::Time,
        slot: 9,
        pattern: Some(Box::new(model::Pattern {
            len: 0,
            points: vec![model::Point::new(0.0, 0.3)],
            name: None,
        })),
    });
    assert!(spec(&a, s0).lapsus.is_some());
    let (_, _, pads) = places(&mut a, &ctx);
    click_with(&mut a, &ctx, pad(pads, 9), egui::PointerButton::Secondary);
    assert!(spec(&a, s0).lapsus.is_none(), "the factory's again");
    assert_eq!(
        pattern(&a, s0, Lane::Time, 9),
        model::factory(Lane::Time)[9]
    );
}

#[test]
fn every_factory_preset_names_real_keys_in_both_languages() {
    let presets = cantus_ui::preset_catalog::entries(
        cantus_ui::preset_catalog::Unit::Effect(EffectKind::TimeShaper),
        false,
    );
    assert!(presets.len() >= 12, "{} presets", presets.len());
    for lang in [cantus_ui::i18n::Locale::En, cantus_ui::i18n::Locale::Es] {
        for p in &presets {
            assert!(
                !cantus_ui::i18n::t_in(lang, &p.id).starts_with("preset."),
                "{} has a name in {lang:?}",
                p.id
            );
        }
    }
    let start = match cantus_core::starting::start_of(EffectKind::TimeShaper) {
        cantus_core::starting::Start::Preset(p) => p,
        other => panic!("{other:?}"),
    };
    assert!(presets.iter().any(|p| p.id == start.label_key));
}

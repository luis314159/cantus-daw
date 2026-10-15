//! Fluctus's editor (ADR 0055): what it paints, the curve edited with the automation
//! editor's gestures (each one undo step), the level drawn is the level played, and its
//! presets.

mod support;
use cantus_core::*;
use cantus_ui::views::fx::fluctus;
use cantus_ui::CantusApp;
use support::*;

fn with_fluctus() -> (CantusApp, StripId, EffectId) {
    let mut a = app(cantus_ui::starter_project());
    let s0 = a.project().tracks[0].strip.unwrap();
    a.add_effect(s0, EffectKind::VolumeShaper);
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

thread_local! {
    static NOW: std::cell::Cell<f64> = const { std::cell::Cell::new(0.0) };
}

fn button(pos: egui::Pos2, pressed: bool) -> egui::Event {
    egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    }
}

fn double_click(a: &mut CantusApp, ctx: &egui::Context, at: egui::Pos2) {
    // A second apart from anything before: not a third click of an earlier pair.
    NOW.with(|t| t.set(t.get() + 1.0));
    frame(a, ctx, vec![egui::Event::PointerMoved(at)]);
    for _ in 0..2 {
        frame(a, ctx, vec![button(at, true)]);
        frame(a, ctx, vec![button(at, false)]);
    }
    frame(a, ctx, vec![]);
}

fn drag(a: &mut CantusApp, ctx: &egui::Context, from: egui::Pos2, to: egui::Pos2) {
    frame(a, ctx, vec![egui::Event::PointerMoved(from)]);
    frame(
        a,
        ctx,
        vec![egui::Event::PointerMoved(from), button(from, true)],
    );
    for i in 1..=4 {
        let at = from + (to - from) * (i as f32 / 4.0);
        frame(a, ctx, vec![egui::Event::PointerMoved(at)]);
    }
    frame(
        a,
        ctx,
        vec![egui::Event::PointerMoved(to), button(to, false)],
    );
    frame(a, ctx, vec![]);
}

fn plot(a: &mut CantusApp, ctx: &egui::Context) -> egui::Rect {
    frame(a, ctx, vec![]);
    frame(a, ctx, vec![]);
    a.fx.fluctus.plot.expect("the curve was drawn")
}

fn at(r: egui::Rect, u: f32, v: f32) -> egui::Pos2 {
    egui::pos2(r.left() + r.width() * u, r.bottom() - r.height() * v)
}

#[test]
fn the_editor_paints_the_curve_and_its_cards() {
    let (mut a, s0, fx) = with_fluctus();
    fluctus::demo_setup(&mut a, s0, fx);
    cantus_ui::views::fx::demo_feed(&mut a, EffectKind::VolumeShaper);
    let text = paint_editors(&mut a, egui::vec2(1400.0, 900.0)).to_lowercase();
    for want in [
        "level over one cycle",
        "128 bpm",
        "100%",
        "0%",
        "segment from this point",
        "hold",
        "stairs",
        "tension",
        "length",
        "no latency",
    ] {
        assert!(text.contains(want), "missing {want:?} in {text}");
    }
}

#[test]
fn at_depth_zero_the_display_says_the_sound_passes() {
    let (mut a, _, _) = with_fluctus();
    let text = paint_editors(&mut a, egui::vec2(1400.0, 900.0));
    assert!(text.contains("the sound passes untouched"), "{text}");
}

#[test]
fn a_double_click_adds_a_point_and_another_takes_it_away_one_step_each() {
    let (mut a, s0, _) = with_fluctus();
    let ctx = ctx();
    let r = plot(&mut a, &ctx);
    // On the grid's three-quarter line, at a quarter's height: a free place.
    double_click(&mut a, &ctx, at(r, 0.75, 0.25));
    let sp = spec(&a, s0);
    assert_eq!(sp.get("p2.on"), 1.0, "the first free point is used");
    assert_eq!(sp.get("p2.x"), 0.75);
    assert!((sp.get("p2.y") - 0.25).abs() < 0.01);
    double_click(&mut a, &ctx, at(r, 0.75, 0.25));
    assert_eq!(spec(&a, s0).get("p2.on"), 0.0, "taken away");
    a.undo();
    assert_eq!(spec(&a, s0).get("p2.on"), 1.0, "one step back: there again");
    a.undo();
    assert_eq!(spec(&a, s0).get("p2.on"), 0.0, "and before it was added");
}

#[test]
fn dragging_a_point_moves_it_on_the_grid_in_one_step() {
    let (mut a, s0, _) = with_fluctus();
    let ctx = ctx();
    let r = plot(&mut a, &ctx);
    // p1 sits at (0.5, 1): drag it to about (0.62, 0.6).
    drag(&mut a, &ctx, at(r, 0.5, 1.0), at(r, 0.62, 0.6));
    let sp = spec(&a, s0);
    assert_eq!(sp.get("p1.x"), 0.625, "on the sixteenths");
    assert!((sp.get("p1.y") - 0.6).abs() < 0.01);
    a.undo();
    let sp = spec(&a, s0);
    assert_eq!(
        (sp.get("p1.x"), sp.get("p1.y")),
        (0.5, 1.0),
        "one undo step"
    );
}

/// The level drawn is the level played: the DSP's gain on a constant 1, unsmoothed, at
/// the song's quarter notes, against the editor's curve at the same phase.
#[test]
fn the_level_drawn_is_the_level_played() {
    let mut sp = EffectSpec::new(EffectKind::VolumeShaper);
    for (k, v) in [
        ("depth", 0.8),
        ("smooth", 0.0),
        ("p2.on", 1.0),
        ("p2.x", 0.25),
        ("p2.y", 0.7),
        ("p2.s", 1.0),
        ("p2.t", 0.5),
    ] {
        sp.set(k, v);
    }
    let fs = cantus_engine::fx::fluctus::settings(&sp);
    let mut d = cantus_dsp::fluctus::Fluctus::new(48_000.0, cantus_engine::fx::fluctus::shape_at);
    d.set(&fs);
    d.reset();
    let beat = 48_000.0 * 60.0 / 120.0;
    let n = beat as usize;
    let (mut l, mut r) = (vec![1.0f32; n], vec![1.0f32; n]);
    d.process(&mut l, &mut r);
    let curve = cantus_dsp::fluctus::Curve::new(&fs.points, cantus_engine::fx::fluctus::shape_at);
    for i in (0..n).step_by(311) {
        let phase = (i as f64 / beat) as f32;
        let drawn = fluctus::level_at(&fs, &curve, phase);
        assert!(
            (l[i] - drawn).abs() < 1e-4,
            "at {phase:.3}: {} played, {drawn} drawn",
            l[i]
        );
    }
}

#[test]
fn every_factory_preset_names_real_keys_and_is_in_both_languages() {
    for (label, values) in fluctus::FACTORY_PRESETS {
        let mut sp = EffectSpec::new(EffectKind::VolumeShaper);
        for (k, v) in *values {
            assert!(sp.set(k, *v).is_some(), "{label}: no parameter {k}");
        }
        assert!(sp.get("depth") > 0.0, "{label} does something");
        for lang in [cantus_ui::i18n::Locale::En, cantus_ui::i18n::Locale::Es] {
            cantus_ui::i18n::set_locale(lang);
            assert_ne!(cantus_ui::i18n::t(label), *label, "{label} in {lang:?}");
        }
    }
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
}

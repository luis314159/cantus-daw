//! Fervor's editor: what it paints, what its gestures change (one undo step each), the
//! harmonics it measures, and the drawn curve's editing.

mod support;
use cantus_core::*;
use cantus_dsp::fervor::{Model, SatSettings};
use cantus_ui::views::fx::saturator::{self, Harmonics};
use cantus_ui::CantusApp;
use support::*;

/// An app with Fervor on the first strip, its editor open.
fn with_fervor() -> (CantusApp, StripId, EffectId) {
    let mut a = app(cantus_ui::starter_project());
    let s0 = a.project().tracks[0].strip.unwrap();
    // The unit at its defaults: a neutral insert (ADR 0071).
    a.settings.insert_neutral = true;
    a.add_effect(s0, EffectKind::Saturator);
    let fx = a.project().strip(s0).unwrap().effects[0].id;
    a.open_effect_editor(s0, fx);
    (a, s0, fx)
}

fn spec(a: &CantusApp, s: StripId) -> EffectSpec {
    a.project().strip(s).unwrap().effects[0].spec.clone()
}

fn set(a: &mut CantusApp, s: StripId, fx: EffectId, key: &str, value: f32) {
    a.set_fx_param(s, fx, key, value, false);
}

/// Run a frame of the editor window with these events.
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

fn button(pos: egui::Pos2, pressed: bool, which: egui::PointerButton) -> egui::Event {
    egui::Event::PointerButton {
        pos,
        button: which,
        pressed,
        modifiers: egui::Modifiers::NONE,
    }
}

/// Press at `from`, move to `to` in steps, release.
fn drag(a: &mut CantusApp, ctx: &egui::Context, from: egui::Pos2, to: egui::Pos2) {
    use egui::PointerButton::Primary;
    frame(a, ctx, vec![egui::Event::PointerMoved(from)]);
    frame(
        a,
        ctx,
        vec![egui::Event::PointerMoved(from), button(from, true, Primary)],
    );
    for i in 1..=4 {
        let at = from + (to - from) * (i as f32 / 4.0);
        frame(a, ctx, vec![egui::Event::PointerMoved(at)]);
    }
    frame(
        a,
        ctx,
        vec![egui::Event::PointerMoved(to), button(to, false, Primary)],
    );
    frame(a, ctx, vec![]);
}

/// Two clicks at `at`, quickly: a double-click.
fn double_click(a: &mut CantusApp, ctx: &egui::Context, at: egui::Pos2) {
    use egui::PointerButton::Primary;
    frame(a, ctx, vec![egui::Event::PointerMoved(at)]);
    for _ in 0..2 {
        frame(a, ctx, vec![button(at, true, Primary)]);
        frame(a, ctx, vec![button(at, false, Primary)]);
    }
    frame(a, ctx, vec![]);
}

/// The square the curve is drawn in, after a couple of frames.
fn curve_rect(a: &mut CantusApp, ctx: &egui::Context) -> egui::Rect {
    frame(a, ctx, vec![]);
    frame(a, ctx, vec![]);
    a.fx.saturator
        .curve_rect
        .expect("the editor drew its curve")
}

/// A point of the curve's units on screen.
fn at(r: egui::Rect, x: f32, y: f32) -> egui::Pos2 {
    egui::pos2(
        r.center().x + x * r.width() * 0.5,
        r.center().y - y / 1.25 * r.height() * 0.5,
    )
}

#[test]
fn the_editor_paints_its_cards_and_says_what_the_model_is() {
    let (mut a, _, _) = with_fervor();
    cantus_ui::views::fx::demo_feed(&mut a, EffectKind::Saturator);
    let text = paint_editors(&mut a, egui::vec2(1400.0, 900.0));
    let lower = text.to_lowercase().replace(' ', "");
    for want in [
        "Tape",
        "Tube",
        "Overdrive",
        "Fuzz",
        "Fold",
        "Curve",
        "Soft, odd harmonics",
        "Highs saturate first",
        "Emphasis",
        "THD",
        "Aliasing",
        "Harmonics",
        "Linear",
        "No latency",
        "Match",
    ] {
        let want = want.replace(' ', "").to_lowercase();
        assert!(lower.contains(&want), "missing {want:?} in {text}");
    }
}

#[test]
fn the_character_knob_is_named_for_the_model() {
    let (mut a, s0, fx) = with_fervor();
    set(&mut a, s0, fx, "model", 1.0);
    let text = paint_editors(&mut a, egui::vec2(1400.0, 900.0)).to_lowercase();
    assert!(text.contains("asym"), "{text}");
    set(&mut a, s0, fx, "model", 4.0);
    let text = paint_editors(&mut a, egui::vec2(1400.0, 900.0)).to_lowercase();
    assert!(text.contains("knee"), "{text}");
}

#[test]
fn the_preview_says_the_settings_in_one_line() {
    let mut s = EffectSpec::new(EffectKind::Saturator);
    s.set("drive", 12.0);
    s.set("os", 2.0);
    let ctx = ctx();
    let text = paint(&ctx, egui::vec2(400.0, 120.0), 1, |ui| {
        cantus_ui::views::fx::preview(ui, &s);
    });
    assert!(text.contains("Tape"), "{text}");
    assert!(text.contains("+12.0 dB"), "{text}");
    assert!(text.contains("4x"), "{text}");
}

fn measured(s: &SatSettings, level: usize) -> Harmonics {
    saturator::harmonics_of(s, level)
}

#[test]
fn the_harmonics_are_measured_through_the_real_dsp() {
    // At its defaults: a wire, so no harmonic and nothing between them.
    let h = measured(&SatSettings::default(), 1);
    assert!(h.thd_pct < 1e-3, "{}", h.thd_pct);
    assert!(h.alias_db < -120.0, "{}", h.alias_db);
    // Tape: odd harmonics, no even ones.
    let tape = SatSettings {
        drive_db: 18.0,
        oversampling: 4,
        ..SatSettings::default()
    };
    let h = measured(&tape, 1);
    assert!(h.harm[2] > -40.0, "a third: {:?}", h.harm);
    assert!(h.harm[1] < -90.0, "no second: {:?}", h.harm);
    assert!(h.even_odd_db < -40.0, "{}", h.even_odd_db);
    // Tube: even harmonics.
    let tube = SatSettings {
        model: Model::Tube,
        ..tape.clone()
    };
    let h = measured(&tube, 1);
    assert!(h.harm[1] > -40.0, "a second: {:?}", h.harm);
    // A clip driven into a near-square at full scale: at 1× what folds back is about 40 dB
    // under the fundamental; 8× takes it down by about 20 dB more. (A near-square's
    // harmonics fall only as 1/n, and those past even 8× fold back near −60 dB — the
    // display shows exactly that, which is the point of it.)
    let clip = |os| SatSettings {
        model: Model::Clip,
        drive_db: 24.0,
        character: 0.1,
        oversampling: os,
        ..SatSettings::default()
    };
    let a1 = measured(&clip(1), 3).alias_db;
    let a8 = measured(&clip(8), 3).alias_db;
    assert!(a1 > -50.0, "1x aliases: {a1}");
    assert!(a8 < a1 - 15.0, "8x much less: {a8} against {a1}");
    // More level, more distortion.
    let quiet = measured(&tape, 0).thd_pct;
    let loud = measured(&tape, 3).thd_pct;
    assert!(loud > quiet * 2.0, "{quiet} {loud}");
}

#[test]
fn a_click_on_a_model_chooses_it_as_one_step() {
    let (mut a, s0, _) = with_fervor();
    let ctx = ctx();
    let r = curve_rect(&mut a, &ctx);
    // The model list is left of the curve: its seventh key (Curve) is the lowest.
    let before = a.bus.history().len();
    let mut chosen = None;
    for y in (0..400).step_by(6) {
        let p = egui::pos2(r.left() - 90.0, r.top() + y as f32);
        frame(&mut a, &ctx, vec![egui::Event::PointerMoved(p)]);
        frame(
            &mut a,
            &ctx,
            vec![button(p, true, egui::PointerButton::Primary)],
        );
        frame(
            &mut a,
            &ctx,
            vec![button(p, false, egui::PointerButton::Primary)],
        );
        let m = spec(&a, s0).get("model");
        if m != 0.0 {
            chosen = Some(m);
            break;
        }
    }
    assert_eq!(chosen, Some(1.0), "the key under Tape is Tube");
    assert_eq!(a.bus.history().len(), before + 1);
}

#[test]
fn the_drawn_curve_takes_points_moves_them_and_lets_them_go() {
    let (mut a, s0, fx) = with_fervor();
    set(&mut a, s0, fx, "model", 6.0);
    let ctx = ctx();
    let r = curve_rect(&mut a, &ctx);
    // Double-click on the empty right half: a point is added where the pointer is.
    double_click(&mut a, &ctx, at(r, 0.5, 0.2));
    let sp = spec(&a, s0);
    assert_eq!(sp.get("p0.on"), 1.0, "{sp:?}");
    assert!((sp.get("p0.x") - 0.5).abs() < 0.02);
    assert!((sp.get("p0.y") - 0.2).abs() < 0.02);
    // Drag it up: one undo step.
    let before = a.bus.history().len();
    drag(&mut a, &ctx, at(r, 0.5, 0.2), at(r, 0.5, 0.8));
    let sp = spec(&a, s0);
    assert!((sp.get("p0.y") - 0.8).abs() < 0.03, "{}", sp.get("p0.y"));
    assert_eq!(a.bus.history().len(), before + 1, "a drag is one step");
    // The curve the DSP runs passes through it.
    let ss = cantus_engine::fx::saturator::settings(&sp);
    assert!((ss.curve.eval(0.5) - sp.get("p0.y")).abs() < 1e-6);
    // Drag the end down: a clipped top.
    drag(&mut a, &ctx, at(r, 1.0, 1.0), at(r, 1.0, 0.6));
    assert!((spec(&a, s0).get("c.hi") - 0.6).abs() < 0.03);
    // Right-click on the point removes it, and the sparse map forgets it.
    let p = at(r, 0.5, spec(&a, s0).get("p0.y"));
    frame(&mut a, &ctx, vec![egui::Event::PointerMoved(p)]);
    frame(
        &mut a,
        &ctx,
        vec![button(p, true, egui::PointerButton::Secondary)],
    );
    frame(
        &mut a,
        &ctx,
        vec![button(p, false, egui::PointerButton::Secondary)],
    );
    let sp = spec(&a, s0);
    assert!(!sp.params.contains_key("p0.on"), "{sp:?}");
    assert!(!sp.params.contains_key("p0.x"));
}

#[test]
fn dragging_an_analytic_curve_drives_it() {
    let (mut a, s0, _) = with_fervor();
    let ctx = ctx();
    let r = curve_rect(&mut a, &ctx);
    drag(&mut a, &ctx, at(r, -0.5, -0.8), at(r, -0.5, 0.2));
    let d = spec(&a, s0).get("drive");
    assert!(d > 5.0, "dragging up drives: {d}");
}

#[test]
fn a_shape_replaces_the_curve_in_one_step_and_a_straight_line_clears_it() {
    let (mut a, s0, fx) = with_fervor();
    let before = a.bus.history().len();
    {
        let mut host = cantus_ui::views::fx::host::DawHost::new(&mut a, s0, fx, "saturator");
        saturator::apply_shape(&mut host, 6);
    }
    assert_eq!(a.bus.history().len(), before + 1);
    let sp = spec(&a, s0);
    assert_eq!(sp.get("model"), 6.0);
    assert_eq!(sp.get("c.sym"), 0.0);
    let ss = cantus_engine::fx::saturator::settings(&sp);
    assert!((ss.curve.eval(-0.5) - 0.5).abs() < 1e-6, "rectified");
    {
        let mut host = cantus_ui::views::fx::host::DawHost::new(&mut a, s0, fx, "saturator");
        saturator::apply_shape(&mut host, 0);
    }
    let sp = spec(&a, s0);
    assert_eq!(sp.params.len(), 1, "only the model is stored: {sp:?}");
    a.undo();
    a.undo();
    assert!(spec(&a, s0).params.is_empty());
}

#[test]
fn every_factory_preset_names_real_keys_and_is_in_both_languages() {
    for (label, values) in saturator::FACTORY_PRESETS {
        for (k, _) in *values {
            assert!(
                EffectKind::Saturator.param(k).is_some(),
                "{label}: no parameter {k}"
            );
        }
        for lang in [cantus_ui::i18n::Locale::En, cantus_ui::i18n::Locale::Es] {
            cantus_ui::i18n::set_thread_locale(lang);
            assert_ne!(cantus_ui::i18n::t(label), *label, "{label} in {lang:?}");
        }
    }
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
}

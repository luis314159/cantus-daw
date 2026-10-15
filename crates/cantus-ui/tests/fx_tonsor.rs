//! Tonsor's editor (ADR 0056): what it paints, a curve chosen in one step, the curve it
//! draws is the curve the DSP plays, the meter tells when nothing is clipped, and its
//! presets.

mod support;
use cantus_core::*;
use cantus_ui::views::fx::tonsor;
use cantus_ui::CantusApp;
use support::*;

fn with_tonsor() -> (CantusApp, StripId, EffectId) {
    let mut a = app(cantus_ui::starter_project());
    let s0 = a.project().tracks[0].strip.unwrap();
    a.add_effect(s0, EffectKind::Clipper);
    let fx = a.project().strip(s0).unwrap().effects[0].id;
    a.open_effect_editor(s0, fx);
    (a, s0, fx)
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

fn click(a: &mut CantusApp, ctx: &egui::Context, at: egui::Pos2) {
    use egui::PointerButton::Primary;
    let b = |pressed| egui::Event::PointerButton {
        pos: at,
        button: Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    frame(a, ctx, vec![egui::Event::PointerMoved(at)]);
    frame(a, ctx, vec![b(true)]);
    frame(a, ctx, vec![b(false)]);
    frame(a, ctx, vec![]);
}

#[test]
fn the_editor_paints_its_cards_and_its_meter() {
    let (mut a, s0, fx) = with_tonsor();
    tonsor::demo_setup(&mut a, s0, fx);
    cantus_ui::views::fx::demo_feed(&mut a, EffectKind::Clipper);
    let text = paint_editors(&mut a, egui::vec2(1400.0, 900.0));
    let lower = text.to_lowercase();
    for want in [
        "input to output",
        "clip meter",
        "ceiling",
        "knee",
        "reduction",
        "clipped",
        "overs in",
        "dbtp",
        "guard",
        "hard",
        "soft",
        "analog",
        "16x",
        "true peak",
        "gain match",
        "latency",
    ] {
        assert!(lower.contains(want), "missing {want:?} in {text}");
    }
}

/// With nothing over the ceiling the meter says the sound passes untouched.
#[test]
fn when_nothing_is_clipped_the_meter_says_so() {
    let (mut a, _, _) = with_tonsor();
    let mut d = cantus_dsp::tonsor::Tonsor::new(48_000.0);
    d.reset();
    let (mut l, mut r) = (vec![0.3f32; 4_800], vec![-0.3f32; 4_800]);
    d.process(&mut l, &mut r);
    let mut t = [0.0f32; 64];
    d.telemetry(&mut t);
    a.fx.injected = Some(t);
    let text = paint_editors(&mut a, egui::vec2(1400.0, 900.0));
    assert!(text.contains("the sound passes untouched"), "{text}");
}

#[test]
fn a_click_on_a_curve_chooses_it_as_one_step() {
    let (mut a, s0, _) = with_tonsor();
    let ctx = ctx();
    frame(&mut a, &ctx, vec![]);
    frame(&mut a, &ctx, vec![]);
    let keys = a.fx.tonsor.curve_keys.expect("the curves' keys were drawn");
    // Three keys: the first is Hard.
    let at = egui::pos2(keys.left() + keys.width() / 6.0, keys.center().y);
    click(&mut a, &ctx, at);
    let curve = a.project().strip(s0).unwrap().effects[0].spec.get("curve");
    assert_eq!(curve, 0.0, "hard chosen");
    a.undo();
    let curve = a.project().strip(s0).unwrap().effects[0].spec.get("curve");
    assert_eq!(curve, 1.0, "one undo step");
}

/// The curve drawn is the curve played: a 1 kHz sine at each level through the DSP at 1×
/// peaks where the display's curve says, within 0.05 dB.
#[test]
fn the_curve_drawn_is_the_curve_played() {
    let mut spec = EffectSpec::new(EffectKind::Clipper);
    for (k, v) in [
        ("drive", 5.0),
        ("ceil", -2.0),
        ("knee", 4.0),
        ("curve", 2.0),
        ("match", 1.0),
        ("out", 1.5),
    ] {
        spec.set(k, v);
    }
    let ts = cantus_engine::fx::tonsor::settings(&spec);
    for in_db in [-20.0f32, -9.0, -6.0, -3.0, 0.0, 3.0, 8.0] {
        let mut d = cantus_dsp::tonsor::Tonsor::new(48_000.0);
        d.set(&ts);
        d.reset();
        let a = cantus_dsp::math::db_to_gain(in_db);
        let n = 9_600;
        let mut l: Vec<f32> = (0..n)
            .map(|i| {
                a * cantus_dsp::math::sin(std::f32::consts::TAU * 1_000.0 * i as f32 / 48_000.0)
            })
            .collect();
        let mut r = l.clone();
        d.process(&mut l, &mut r);
        let peak = l[4_800..].iter().fold(0f32, |m, v| m.max(v.abs()));
        let played = 20.0 * cantus_dsp::math::log10(peak);
        let drawn = tonsor::transfer(&ts, in_db);
        assert!(
            (played - drawn).abs() < 0.05,
            "{in_db} dB in: played {played}, drawn {drawn}"
        );
    }
}

#[test]
fn every_factory_preset_names_real_keys_and_is_in_both_languages() {
    for (label, values) in tonsor::FACTORY_PRESETS {
        for (k, _) in *values {
            assert!(
                EffectKind::Clipper.param(k).is_some(),
                "{label}: no parameter {k}"
            );
        }
        for lang in [cantus_ui::i18n::Locale::En, cantus_ui::i18n::Locale::Es] {
            cantus_ui::i18n::set_locale(lang);
            assert_ne!(cantus_ui::i18n::t(label), *label, "{label} in {lang:?}");
        }
    }
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
}

/// Every preset clips something of the demo signal (or holds a ceiling under it).
#[test]
fn every_preset_moves_the_sound() {
    for (label, values) in tonsor::FACTORY_PRESETS {
        let mut spec = EffectSpec::new(EffectKind::Clipper);
        for (k, v) in *values {
            spec.set(k, *v);
        }
        let mut d = cantus_dsp::tonsor::Tonsor::new(48_000.0);
        d.set(&cantus_engine::fx::tonsor::settings(&spec));
        d.reset();
        let x: Vec<f32> = (0..24_000).map(tonsor::demo_signal).collect();
        let (mut l, mut r) = (x.clone(), x.clone());
        d.process(&mut l, &mut r);
        let lat = d.latency();
        let moved = l[lat..].iter().zip(&x).any(|(a, b)| (a - b).abs() > 1e-3);
        assert!(moved, "{label} does something");
    }
}

/// No word is cut short, in either language, with the editor fed as when it plays.
#[test]
fn no_word_is_cut_in_either_language() {
    for lang in [cantus_ui::i18n::Locale::En, cantus_ui::i18n::Locale::Es] {
        cantus_ui::i18n::set_locale(lang);
        let (mut a, s0, fx) = with_tonsor();
        tonsor::demo_setup(&mut a, s0, fx);
        cantus_ui::views::fx::demo_feed(&mut a, EffectKind::Clipper);
        let text = paint_editors(&mut a, egui::vec2(1400.0, 900.0));
        assert!(!text.contains('\u{2026}'), "{lang:?}: {text}");
    }
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
}

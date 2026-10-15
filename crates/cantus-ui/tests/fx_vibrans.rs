//! Vibrans's editor (ADR 0055): what it paints, the pitch it draws is the pitch the DSP
//! plays, and its presets.

mod support;
use cantus_core::*;
use cantus_ui::views::fx::vibrans;
use cantus_ui::CantusApp;
use support::*;

fn with_vibrans() -> (CantusApp, StripId, EffectId) {
    let mut a = app(cantus_ui::starter_project());
    let s0 = a.project().tracks[0].strip.unwrap();
    a.add_effect(s0, EffectKind::Vibrato);
    let fx = a.project().strip(s0).unwrap().effects[0].id;
    a.open_effect_editor(s0, fx);
    (a, s0, fx)
}

#[test]
fn the_editor_paints_its_cards_and_the_depth_in_cents() {
    let (mut a, s0, fx) = with_vibrans();
    vibrans::demo_setup(&mut a, s0, fx);
    cantus_ui::views::fx::demo_feed(&mut a, EffectKind::Vibrato);
    let text = paint_editors(&mut a, egui::vec2(1400.0, 900.0)).to_lowercase();
    for want in [
        "pitch over two cycles",
        "+35 ct",
        "rise",
        "after a silence",
        "400ms",
        "lfo",
        "stereo",
        "mix",
        "no latency",
    ] {
        assert!(text.contains(want), "missing {want:?} in {text}");
    }
}

#[test]
fn at_depth_zero_the_display_says_the_sound_passes() {
    let (mut a, _, _) = with_vibrans();
    let text = paint_editors(&mut a, egui::vec2(1400.0, 900.0));
    assert!(text.contains("the sound passes untouched"), "{text}");
}

/// The pitch drawn is the pitch played: a 1 kHz tone through Vibrans, its period measured
/// between zero crossings, against the drawn curve at the same moment.
#[test]
fn the_pitch_drawn_is_the_pitch_played() {
    let mut spec = EffectSpec::new(EffectKind::Vibrato);
    for (k, v) in [("depth", 50.0), ("rate", 2.0)] {
        spec.set(k, v);
    }
    let vs = cantus_engine::fx::vibrans::settings(&spec);
    let mut d = cantus_dsp::vibrans::Vibrans::new(48_000.0);
    d.set(&vs);
    d.reset();
    let n = 96_000;
    let x: Vec<f32> = (0..n)
        .map(|i| {
            let a = (std::f64::consts::TAU * 1_000.0 * i as f64 / 48_000.0)
                % std::f64::consts::TAU;
            cantus_dsp::math::sin(a) as f32 * 0.5
        })
        .collect();
    let (mut l, mut r) = (x.clone(), x);
    d.process(&mut l, &mut r);
    let mut last: Option<f64> = None;
    let mut checked = 0;
    for i in 24_000..n {
        if l[i - 1] < 0.0 && l[i] >= 0.0 {
            let t = (i - 1) as f64 + f64::from(-l[i - 1] / (l[i] - l[i - 1]));
            if let Some(p) = last {
                let cents = 1200.0 * cantus_dsp::math::log2(48_000.0 / (t - p) / 1_000.0);
                // The middle of the period, in cycles of the 2 Hz LFO.
                let c = (t + p) * 0.5 / 48_000.0 * 2.0;
                let drawn = f64::from(vibrans::pitch_at(&vs, c).0) * 50.0;
                assert!((cents - drawn).abs() < 4.0, "at {c:.3}: {cents:.1} vs {drawn:.1}");
                checked += 1;
            }
            last = Some(t);
        }
    }
    assert!(checked > 1_000);
}

#[test]
fn every_factory_preset_names_real_keys_and_is_in_both_languages() {
    for (label, values) in vibrans::FACTORY_PRESETS {
        let mut spec = EffectSpec::new(EffectKind::Vibrato);
        for (k, v) in *values {
            assert!(spec.set(k, *v).is_some(), "{label}: no parameter {k}");
        }
        assert!(spec.get("depth") > 0.0, "{label} does something");
        for lang in [cantus_ui::i18n::Locale::En, cantus_ui::i18n::Locale::Es] {
            cantus_ui::i18n::set_locale(lang);
            assert_ne!(cantus_ui::i18n::t(label), *label, "{label} in {lang:?}");
        }
    }
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
}

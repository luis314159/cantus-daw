//! Phasis's editor (ADR 0055): what it paints, the response drawn is the response the DSP
//! makes (sines through a held sweep), and its presets.

mod support;
use cantus_core::*;
use cantus_ui::views::fx::phasis;
use cantus_ui::CantusApp;
use support::*;

fn with_phasis() -> (CantusApp, StripId, EffectId) {
    let mut a = app(cantus_ui::starter_project());
    let s0 = a.project().tracks[0].strip.unwrap();
    a.add_effect(s0, EffectKind::Phaser);
    let fx = a.project().strip(s0).unwrap().effects[0].id;
    a.open_effect_editor(s0, fx);
    (a, s0, fx)
}

#[test]
fn the_editor_paints_its_sweep_and_response() {
    let (mut a, s0, fx) = with_phasis();
    phasis::demo_setup(&mut a, s0, fx);
    cantus_ui::views::fx::demo_feed(&mut a, EffectKind::Phaser);
    let text = paint_editors(&mut a, egui::vec2(1400.0, 900.0)).to_lowercase();
    for want in [
        "the stages' frequency over two cycles",
        "3.5 khz",
        "100 hz",
        "the response now",
        "stages",
        "feedback",
        "lfo",
        "no latency",
    ] {
        assert!(text.contains(want), "missing {want:?} in {text}");
    }
}

#[test]
fn at_mix_zero_the_display_says_the_sound_passes() {
    let (mut a, _, _) = with_phasis();
    let text = paint_editors(&mut a, egui::vec2(1400.0, 900.0));
    assert!(text.contains("the sound passes untouched"), "{text}");
}

#[test]
fn the_response_drawn_is_the_response_played() {
    for (stages, fb) in [(4.0f32, 0.0f32), (8.0, 0.6), (6.0, -0.5)] {
        let mut spec = EffectSpec::new(EffectKind::Phaser);
        for (k, v) in [
            ("stages", stages),
            ("mix", 0.5),
            ("fb", fb),
            ("lo", 300.0),
            ("hi", 3_000.0),
            ("rate", 0.02),
        ] {
            spec.set(k, v);
        }
        let mut ps = cantus_engine::fx::phasis::settings(&spec);
        // Held still: the sweep's middle.
        ps.lfo.rate = cantus_dsp::modfx_lfo::Rate::Hz(0.0);
        let fc = cantus_dsp::phasis::sweep_hz(300.0, 3_000.0, 0.0);
        for f in [120.0f32, 640.0, 1_333.0, 5_100.0] {
            let mut u = cantus_dsp::phasis::Phasis::new(48_000.0);
            u.set(&ps);
            u.reset();
            let n = 48_000;
            let w = std::f64::consts::TAU * f64::from(f) / 48_000.0;
            let x: Vec<f32> = (0..n)
                .map(|i| cantus_dsp::math::sin((w * i as f64) % std::f64::consts::TAU) as f32)
                .collect();
            let (mut l, mut r) = (x.clone(), x);
            u.process(&mut l, &mut r);
            let peak = l[24_000..].iter().fold(0.0f32, |m, v| m.max(v.abs()));
            let measured = 20.0 * cantus_dsp::math::log10(peak.max(1e-9));
            let drawn = phasis::response_db(&ps, fc, f);
            assert!(
                (measured - drawn).abs() < 0.3 || (drawn < -30.0 && measured < -25.0),
                "{stages} stages fb {fb} at {f} Hz: {measured:.2} measured, {drawn:.2} drawn"
            );
        }
    }
}

#[test]
fn every_factory_preset_names_real_keys_and_is_in_both_languages() {
    for (label, values) in phasis::FACTORY_PRESETS {
        let mut spec = EffectSpec::new(EffectKind::Phaser);
        for (k, v) in *values {
            assert!(spec.set(k, *v).is_some(), "{label}: no parameter {k}");
        }
        assert!(spec.get("mix") > 0.0, "{label} does something");
        for lang in [cantus_ui::i18n::Locale::En, cantus_ui::i18n::Locale::Es] {
            cantus_ui::i18n::set_locale(lang);
            assert_ne!(cantus_ui::i18n::t(label), *label, "{label} in {lang:?}");
        }
    }
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
}

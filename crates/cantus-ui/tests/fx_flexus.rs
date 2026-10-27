//! Flexus's editor (ADR 0055): what it paints, the comb drawn is the comb the DSP makes
//! (sines measured through a held sweep), the centre point drawn as silence, the presets.

mod support;
use cantus_core::*;
use cantus_ui::views::fx::flexus;
use cantus_ui::CantusApp;
use support::*;

fn with_flexus() -> (CantusApp, StripId, EffectId) {
    let mut a = app(cantus_ui::starter_project());
    let s0 = a.project().tracks[0].strip.unwrap();
    // The unit at its defaults: a neutral insert (ADR 0071).
    a.settings.insert_neutral = true;
    a.add_effect(s0, EffectKind::Flanger);
    let fx = a.project().strip(s0).unwrap().effects[0].id;
    a.open_effect_editor(s0, fx);
    (a, s0, fx)
}

#[test]
fn the_editor_paints_its_sweep_comb_and_path() {
    let (mut a, s0, fx) = with_flexus();
    flexus::demo_setup(&mut a, s0, fx);
    cantus_ui::views::fx::demo_feed(&mut a, EffectKind::Flanger);
    let text = paint_editors(&mut a, egui::vec2(1400.0, 900.0)).to_lowercase();
    for want in [
        "sweep by",
        "envelope",
        "manual",
        "the wet against the dry over two cycles",
        "dry",
        "the comb now",
        "through zero",
        "invert wet",
        "feedback",
        "latency 5.0 ms",
    ] {
        assert!(text.contains(want), "missing {want:?} in {text}");
    }
}

#[test]
fn at_mix_zero_the_display_says_the_sound_passes() {
    let (mut a, _, _) = with_flexus();
    let text = paint_editors(&mut a, egui::vec2(1400.0, 900.0));
    assert!(text.contains("the sound passes untouched"), "{text}");
}

/// A sine through Flexus with the sweep held (manual), its level after the comb settles,
/// against the comb the editor draws.
#[test]
fn the_comb_drawn_is_the_comb_played() {
    for (tz, manual, fb, invert) in [
        (false, 0.4f32, 0.0f32, false),
        (false, 0.7, 0.5, false),
        (true, 0.2, -0.4, true),
    ] {
        let mut spec = EffectSpec::new(EffectKind::Flanger);
        for (k, v) in [
            ("mode", 2.0),
            ("tz", if tz { 1.0 } else { 0.0 }),
            ("mix", 0.5),
            ("manual", manual),
            ("fb", fb),
            ("invert", if invert { 1.0 } else { 0.0 }),
        ] {
            spec.set(k, v);
        }
        let fs = cantus_engine::fx::flexus::settings(&spec);
        let (d, _) = flexus::delay_at(&fs, 0.0, 0.0);
        for f in [150.0f32, 433.0, 1_210.0, 2_900.0] {
            let mut u = cantus_dsp::flexus::Flexus::new(48_000.0);
            u.set(&fs);
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
            let drawn = flexus::comb_db(&fs, d, f);
            assert!(
                (measured - drawn).abs() < 0.6 || (drawn < -30.0 && measured < -25.0),
                "tz {tz} manual {manual} fb {fb} at {f} Hz: {measured:.2} dB measured, {drawn:.2} drawn"
            );
        }
    }
}

#[test]
fn the_centre_point_is_drawn_as_silence() {
    let mut spec = EffectSpec::new(EffectKind::Flanger);
    for (k, v) in [("mode", 2.0), ("tz", 1.0), ("mix", 0.5), ("invert", 1.0)] {
        spec.set(k, v);
    }
    let fs = cantus_engine::fx::flexus::settings(&spec);
    let (d, _) = flexus::delay_at(&fs, 0.0, 0.0);
    assert_eq!(d, 0.0, "the wet sits on the dry");
    for f in [50.0f32, 1_000.0, 3_999.0] {
        assert!(flexus::comb_db(&fs, d, f) < -100.0);
    }
}

#[test]
fn every_factory_preset_names_real_keys_and_is_in_both_languages() {
    for (label, values) in flexus::FACTORY_PRESETS {
        let mut spec = EffectSpec::new(EffectKind::Flanger);
        for (k, v) in *values {
            assert!(spec.set(k, *v).is_some(), "{label}: no parameter {k}");
        }
        assert!(spec.get("mix") > 0.0, "{label} does something");
        for lang in [cantus_ui::i18n::Locale::En, cantus_ui::i18n::Locale::Es] {
            cantus_ui::i18n::set_thread_locale(lang);
            assert_ne!(cantus_ui::i18n::t(label), *label, "{label} in {lang:?}");
        }
    }
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
}

/// The inspector's line names the LFO's rate only in the LFO mode: the envelope and the
/// hand move the sweep without it, and the line then reads "mode · mix".
#[test]
fn the_preview_names_a_rate_only_when_the_lfo_moves_the_sweep() {
    use cantus_ui::views::fx::modfx::PreviewWords;
    for (mode, rated) in [(0.0f32, true), (1.0, false), (2.0, false)] {
        let mut spec = EffectSpec::new(EffectKind::Flanger);
        spec.set("mode", mode);
        let rate = flexus::preview_rate(&spec);
        let line = PreviewWords {
            mode: "M",
            rate,
            amount: &"50%",
        }
        .to_string();
        assert_eq!(!rate.to_string().is_empty(), rated, "mode {mode}");
        assert_eq!(
            line.matches('\u{b7}').count(),
            if rated { 2 } else { 1 },
            "{line}"
        );
    }
}

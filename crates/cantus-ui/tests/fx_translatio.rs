//! Translatio's editor (ADR 0055): what it paints (the shift, the partials, the Hilbert
//! pair's measured rejection), the partials drawn are the partials played, the presets.

mod support;
use cantus_core::*;
use cantus_ui::views::fx::translatio;
use cantus_ui::CantusApp;
use support::*;

fn with_translatio() -> (CantusApp, StripId, EffectId) {
    let mut a = app(cantus_ui::starter_project());
    let s0 = a.project().tracks[0].strip.unwrap();
    a.add_effect(s0, EffectKind::FreqShift);
    let fx = a.project().strip(s0).unwrap().effects[0].id;
    a.open_effect_editor(s0, fx);
    (a, s0, fx)
}

#[test]
fn the_editor_paints_the_shift_the_partials_and_the_rejection() {
    let (mut a, s0, fx) = with_translatio();
    translatio::demo_setup(&mut a, s0, fx);
    cantus_ui::views::fx::demo_feed(&mut a, EffectKind::FreqShift);
    let text = paint_editors(&mut a, egui::vec2(1400.0, 900.0)).to_lowercase();
    for want in [
        "the shift over two lfo cycles",
        "what a 220 hz tone becomes",
        "left up · right down",
        "rejection",
        "69db",
        "up",
        "down",
        "both",
        "no latency",
    ] {
        assert!(text.contains(want), "missing {want:?} in {text}");
    }
}

#[test]
fn at_mix_zero_the_display_says_the_sound_passes() {
    let (mut a, _, _) = with_translatio();
    let text = paint_editors(&mut a, egui::vec2(1400.0, 900.0));
    assert!(text.contains("the sound passes untouched"), "{text}");
}

fn level(x: &[f32], f: f64) -> f64 {
    let n = x.len();
    let w = std::f64::consts::TAU * f / 48_000.0;
    let (mut re, mut im, mut win) = (0.0f64, 0.0f64, 0.0f64);
    for (i, v) in x.iter().enumerate() {
        let h = 0.5 - 0.5 * cantus_dsp::math::cos(std::f64::consts::TAU * i as f64 / n as f64);
        let (s, c) = cantus_dsp::math::sin_cos(w * i as f64);
        re += f64::from(*v) * h * c;
        im += f64::from(*v) * h * s;
        win += h;
    }
    2.0 * (re * re + im * im).sqrt() / win
}

/// A 220 Hz harmonic tone through the shifter: every partial drawn is where the DSP put it,
/// at its level (the feedback's passes included).
#[test]
fn the_partials_drawn_are_the_partials_played() {
    for (shift, side, fb) in [
        (37.0f32, 0.0f32, 0.0f32),
        (61.0, 1.0, 0.0),
        (29.0, 0.0, 0.5),
    ] {
        let mut spec = EffectSpec::new(EffectKind::FreqShift);
        for (k, v) in [("shift", shift), ("side", side), ("fb", fb), ("mix", 1.0)] {
            spec.set(k, v);
        }
        let ts = cantus_engine::fx::translatio::settings(&spec);
        let mut u = cantus_dsp::translatio::Translatio::new(48_000.0);
        u.set(&ts);
        u.reset();
        let n = 96_000;
        let x: Vec<f32> = (0..n)
            .map(|i| {
                (1..=6)
                    .map(|h| {
                        let w = std::f64::consts::TAU * 220.0 * h as f64 / 48_000.0;
                        cantus_dsp::math::sin((w * i as f64) % std::f64::consts::TAU) as f32
                            / h as f32
                    })
                    .sum::<f32>()
                    * 0.2
            })
            .collect();
        let (mut l, mut r) = (x.clone(), x);
        u.process(&mut l, &mut r);
        let mut parts = [(0.0f32, 0.0f32, 0u8); 64];
        let count = translatio::partials(&ts, &mut parts);
        for &(f, amp, ch) in parts[..count].iter().filter(|p| p.2 == 0) {
            let measured = level(&l[48_000..], f64::from(f)) / 0.2;
            let db = 20.0 * cantus_dsp::math::log10(measured / f64::from(amp));
            assert!(
                db.abs() < 1.0,
                "shift {shift} side {side} fb {fb}, channel {ch}, {f} Hz: {measured:.4} vs {amp:.4}"
            );
        }
    }
}

#[test]
fn every_factory_preset_names_real_keys_and_is_in_both_languages() {
    for (label, values) in translatio::FACTORY_PRESETS {
        let mut spec = EffectSpec::new(EffectKind::FreqShift);
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

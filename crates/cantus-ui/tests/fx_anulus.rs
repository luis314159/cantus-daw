//! Anulus's editor (ADR 0055): what it paints (the carrier as a note, the partials), the
//! partials drawn are the partials played, and its presets.

mod support;
use cantus_core::*;
use cantus_ui::views::fx::anulus;
use cantus_ui::CantusApp;
use support::*;

fn with_anulus() -> (CantusApp, StripId, EffectId) {
    let mut a = app(cantus_ui::starter_project());
    let s0 = a.project().tracks[0].strip.unwrap();
    a.add_effect(s0, EffectKind::RingMod);
    let fx = a.project().strip(s0).unwrap().effects[0].id;
    a.open_effect_editor(s0, fx);
    (a, s0, fx)
}

#[test]
fn the_editor_paints_the_carrier_as_a_note_and_the_partials() {
    let (mut a, s0, fx) = with_anulus();
    anulus::demo_setup(&mut a, s0, fx);
    cantus_ui::views::fx::demo_feed(&mut a, EffectKind::RingMod);
    let text = paint_editors(&mut a, egui::vec2(1400.0, 900.0)).to_lowercase();
    for want in [
        "the carrier over two lfo cycles",
        "e5 +2",
        "what a 1 khz tone becomes",
        "input",
        "sine",
        "square",
        "lfo swing",
        "no latency",
    ] {
        assert!(text.contains(want), "missing {want:?} in {text}");
    }
}

#[test]
fn notes_are_named_with_their_cents() {
    let mut s = String::new();
    anulus::note_of(440.0, &mut s);
    assert_eq!(s, "A4 +0");
    s.clear();
    anulus::note_of(261.63, &mut s);
    assert_eq!(s, "C4 +0");
    s.clear();
    anulus::note_of(450.0, &mut s);
    assert_eq!(s, "A4 +39");
}

#[test]
fn at_mix_zero_the_display_says_the_sound_passes() {
    let (mut a, _, _) = with_anulus();
    let text = paint_editors(&mut a, egui::vec2(1400.0, 900.0));
    assert!(text.contains("the sound passes untouched"), "{text}");
}

/// The amplitude of `f` Hz in `x` (Goertzel over a Hann window).
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

#[test]
fn the_partials_drawn_are_the_partials_played() {
    for wave in [0.0f32, 1.0, 2.0] {
        let mut spec = EffectSpec::new(EffectKind::RingMod);
        for (k, v) in [("freq", 300.0), ("wave", wave), ("mix", 1.0)] {
            spec.set(k, v);
        }
        let a = cantus_engine::fx::anulus::settings(&spec);
        let mut u = cantus_dsp::anulus::Anulus::new(48_000.0);
        u.set(&a);
        u.reset();
        let n = 96_000;
        let w = std::f64::consts::TAU * 1_000.0 / 48_000.0;
        let x: Vec<f32> = (0..n)
            .map(|i| cantus_dsp::math::sin((w * i as f64) % std::f64::consts::TAU) as f32)
            .collect();
        let (mut l, mut r) = (x.clone(), x);
        u.process(&mut l, &mut r);
        let mut parts = [(0.0f32, 0.0f32); 32];
        let count = anulus::partials(&a, &mut parts);
        for &(f, amp) in parts[..count].iter().take(6) {
            let measured = level(&l[48_000..], f64::from(f));
            let db = 20.0 * cantus_dsp::math::log10(measured / f64::from(amp));
            assert!(
                db.abs() < 1.0,
                "wave {wave} at {f} Hz: {measured:.4} against {amp:.4}"
            );
        }
    }
}

#[test]
fn every_factory_preset_names_real_keys_and_is_in_both_languages() {
    for (label, values) in anulus::FACTORY_PRESETS {
        let mut spec = EffectSpec::new(EffectKind::RingMod);
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

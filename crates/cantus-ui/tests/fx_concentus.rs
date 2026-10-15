//! Concentus's editor (ADR 0055): what it paints (models, the voices' delays, the mono
//! check), a model chosen in one step, the delays drawn are the delays played, and its
//! presets.

mod support;
use cantus_core::*;
use cantus_dsp::concentus::{Concentus, T_DELAY_MS};
use cantus_ui::views::fx::concentus;
use cantus_ui::CantusApp;
use support::*;

fn with_concentus() -> (CantusApp, StripId, EffectId) {
    let mut a = app(cantus_ui::starter_project());
    let s0 = a.project().tracks[0].strip.unwrap();
    a.add_effect(s0, EffectKind::Chorus);
    let fx = a.project().strip(s0).unwrap().effects[0].id;
    a.open_effect_editor(s0, fx);
    (a, s0, fx)
}

#[test]
fn the_editor_paints_its_models_voices_and_mono_check() {
    let (mut a, s0, fx) = with_concentus();
    concentus::demo_setup(&mut a, s0, fx);
    cantus_ui::views::fx::demo_feed(&mut a, EffectKind::Chorus);
    let text = paint_editors(&mut a, egui::vec2(1400.0, 900.0)).to_lowercase();
    for want in [
        "juno i",
        "juno ii",
        "juno i+ii",
        "dimension 1",
        "dimension 4",
        "free voices",
        "each voice's delay over two cycles",
        "0.86",
        "mono check",
        "correlation",
        "mono sum",
        "in mono",
        "width",
        "tone",
        "no latency",
    ] {
        assert!(text.contains(want), "missing {want:?} in {text}");
    }
}

#[test]
fn at_mix_zero_the_display_says_the_sound_passes() {
    let (mut a, _, _) = with_concentus();
    let text = paint_editors(&mut a, egui::vec2(1400.0, 900.0));
    assert!(text.contains("the sound passes untouched"), "{text}");
}

#[test]
fn a_model_chosen_is_one_undo_step() {
    let (mut a, s0, fx) = with_concentus();
    {
        let mut host = cantus_ui::views::fx::host::DawHost::new(&mut a, s0, fx, "concentus");
        cantus_ui::views::fx::host::FxHost::set(&mut host, "mode", 5.0, None);
    }
    assert_eq!(a.project().strip(s0).unwrap().effects[0].spec.get("mode"), 5.0);
    a.undo();
    assert_eq!(a.project().strip(s0).unwrap().effects[0].spec.get("mode"), 0.0);
}

/// The delays drawn are the delays played: the running unit's voice-1 delay against the
/// display's curve at the same moment, every model.
#[test]
fn the_delays_drawn_are_the_delays_played() {
    for mode in 0..8 {
        let mut spec = EffectSpec::new(EffectKind::Chorus);
        spec.set("mode", mode as f32);
        spec.set("mix", 0.5);
        let cs = cantus_engine::fx::concentus::settings(&spec);
        let mut d = Concentus::new(48_000.0);
        d.set(&cs);
        d.reset();
        let hz = cs
            .mode
            .voicing()
            .map(|v| v.rate)
            .unwrap_or(f64::from(spec.get("rate")));
        let mut t = [0.0f32; 64];
        for n in 1..=24_000usize {
            let (mut l, mut r) = ([0.2f32], [0.2f32]);
            d.process(&mut l, &mut r);
            if n % 997 == 0 {
                d.telemetry(&mut t);
                let c = hz * (n - 1) as f64 / 48_000.0;
                let drawn = concentus::delays_at(&cs, c)[0];
                assert!(
                    (t[T_DELAY_MS] - drawn).abs() < 1e-3,
                    "mode {mode} at {n}: {} ms played, {drawn} ms drawn",
                    t[T_DELAY_MS]
                );
            }
        }
    }
}

#[test]
fn every_factory_preset_names_real_keys_and_is_in_both_languages() {
    for (label, values) in concentus::FACTORY_PRESETS {
        let mut spec = EffectSpec::new(EffectKind::Chorus);
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

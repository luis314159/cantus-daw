//! Spatium's editor: it opens and paints its displays with real numbers, its presets name
//! only parameters that exist, a band made and removed is one undo step each, and the
//! measured decay comes from the audio that went through the reverb.

mod support;
use cantus_core::*;
use cantus_ui::views::fx::space::{band_used, Curve, FACTORY_PRESETS};
use support::*;

fn with_reverb() -> (cantus_ui::CantusApp, StripId, EffectId) {
    let mut a = app(cantus_ui::starter_project());
    let s0 = a.project().tracks[0].strip.unwrap();
    a.add_effect(s0, EffectKind::Space);
    let fx = a.project().strip(s0).unwrap().effects[0].id;
    (a, s0, fx)
}

fn spec(a: &cantus_ui::CantusApp, s: StripId, fx: EffectId) -> EffectSpec {
    a.project()
        .strip(s)
        .unwrap()
        .effect(fx)
        .unwrap()
        .spec
        .clone()
}

/// Painted text as the kit sets it: card titles and labels in capitals, a value's unit a
/// hair apart from its number without the space — so compare lowercased, spaces removed.
fn norm(s: &str) -> String {
    s.to_lowercase().replace([' ', '\u{a0}'], "")
}

#[test]
fn the_editor_paints_its_displays_and_numbers() {
    let (mut a, s0, fx) = with_reverb();
    a.open_effect_editor(s0, fx);
    let text = norm(&paint_editors(&mut a, egui::vec2(1400.0, 900.0)));
    for want in [
        "Decay curve",
        "Post EQ",
        "Tail",
        "Curve (s)",
        "Engine (s)",
        "Measured (s)",
        "Early reflections",
        "Ducking",
        "Church",
        "1.60 s",
    ] {
        assert!(text.contains(&norm(want)), "missing {want:?} in: {text}");
    }
}

#[test]
fn the_demo_measures_a_tail_near_the_curve() {
    let mut a = app(cantus_ui::starter_project());
    a.demo_editor("space");
    cantus_ui::views::fx::demo_feed(&mut a, EffectKind::Space);
    let (strip, effect, _) = a.fx.open.expect("the editor is open");
    let spec = spec(&a, strip, effect);
    let curve = cantus_engine::fx::space::settings(&spec).target_curve();
    let m =
        a.fx.space
            .capture
            .measured
            .clone()
            .expect("the demo's tail was measured");
    // Against what an octave band reads on an ideal decay with the same curve (the demo
    // draws a dip at 2.6 kHz, and a band averages across it).
    for (i, hz) in cantus_dsp::spatium::OCTAVES.iter().enumerate() {
        if !(250.0..=4_000.0).contains(hz) {
            continue;
        }
        let got = m.t60[i].expect("a decay per octave");
        let want = cantus_dsp::spatium::measure::expected_band_t60(|f| curve.t60(f), 48_000.0, *hz)
            .unwrap();
        assert!(
            (got / want - 1.0).abs() < 0.15,
            "{hz} Hz: measured {got} s for {want} s"
        );
    }
    let text = norm(&paint_editors(&mut a, egui::vec2(1400.0, 900.0)));
    assert!(
        text.contains("measured"),
        "the tail view names the measurement: {text}"
    );
}

#[test]
fn every_factory_preset_names_real_parameters() {
    assert!(FACTORY_PRESETS.len() >= 6);
    for (label, params) in FACTORY_PRESETS {
        assert_ne!(cantus_ui::i18n::t(label), *label, "{label} has no string");
        let mut s = EffectSpec::new(EffectKind::Space);
        for (k, v) in *params {
            assert!(s.set(k, *v).is_some(), "{label}: no parameter {k}");
        }
    }
}

#[test]
fn a_band_made_and_removed_is_one_undo_step_each() {
    let (mut a, s0, fx) = with_reverb();
    let before = spec(&a, s0, fx);
    a.set_space_params(
        s0,
        fx,
        &[
            ("d2.used".into(), 1.0),
            ("d2.freq".into(), 300.0),
            ("d2.mult".into(), 2.0),
        ],
        false,
    );
    assert!(band_used(&spec(&a, s0, fx), Curve::Decay, 2));
    let made = spec(&a, s0, fx);
    a.remove_space_band(s0, fx, Curve::Decay, 2);
    let gone = spec(&a, s0, fx);
    assert!(!band_used(&gone, Curve::Decay, 2));
    assert_eq!(
        gone, before,
        "removing a band forgets every field it stored"
    );
    a.undo();
    assert_eq!(spec(&a, s0, fx), made);
    a.undo();
    assert_eq!(spec(&a, s0, fx), before);
}

#[test]
fn the_spectrum_view_draws_the_measured_tail() {
    let mut a = app(cantus_ui::starter_project());
    a.demo_editor("space");
    cantus_ui::views::fx::demo_feed(&mut a, EffectKind::Space);
    a.fx.space.tail_spectrum = true;
    let m = a.fx.space.capture.measured.clone().expect("measured");
    assert!(m.wf_frames > 100, "a waterfall of {} frames", m.wf_frames);
    // The tail fades: late columns are quieter than early ones in every band.
    let rows = cantus_ui::views::fx::space::WF_ROWS;
    let col = |f: usize| m.waterfall[f * rows..(f + 1) * rows].iter().sum::<f32>();
    assert!(col(m.wf_frames - 1) < col(30) - 20.0 * rows as f32);
    let text = norm(&paint_editors(&mut a, egui::vec2(1400.0, 900.0)));
    assert!(
        text.contains(&norm("-60 dB")),
        "the legend names the curve's lines: {text}"
    );
}

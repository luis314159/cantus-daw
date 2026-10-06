//! Fervor (the saturator) inside the engine: the null tests before anything else, then
//! block-size independence, the latency through delay compensation, and the telemetry
//! through the plug-in contract.

use cantus_dsp::math;

mod support;
use cantus_core::*;
use cantus_dsp::fervor::telemetry;
use cantus_dsp::nulltest::assert_null;
use support::*;

fn set(p: &mut Project, effect: EffectId, key: &str, value: f32) {
    for s in &mut p.strips {
        if let Some(e) = s.effects.iter_mut().find(|e| e.id == effect) {
            e.spec.set(key, value).expect("a saturator parameter");
        }
    }
}

/// Inserted and left alone, Fervor is a wire: the render is the render without it.
#[test]
fn fervor_at_its_defaults_renders_exactly_as_no_fervor() {
    let dry = render_of(&demo_project());
    let mut p = demo_project();
    add_effect(&mut p, EffectKind::Saturator);
    assert_null(&dry, &render_of(&p), 0.0, "Fervor at its defaults");
}

/// Without drive every analytic model is a wire, whatever its character, bias and mix.
#[test]
fn without_drive_no_model_but_the_curve_touches_a_sample() {
    let dry = render_of(&demo_project());
    for model in 0..6 {
        let mut p = demo_project();
        let f = add_effect(&mut p, EffectKind::Saturator);
        set(&mut p, f, "model", model as f32);
        set(&mut p, f, "char", 0.9);
        set(&mut p, f, "bias", -0.4);
        set(&mut p, f, "mix", 0.5);
        assert_null(&dry, &render_of(&p), 0.0, &format!("model {model}"));
    }
}

fn busy(p: &mut Project, f: EffectId) {
    for (k, v) in [
        ("model", 1.0),
        ("drive", 20.0),
        ("char", 0.7),
        ("bias", 0.2),
        ("lo_cut", 80.0),
        ("hi_cut", 9_000.0),
        ("tone", 2.0),
        ("os", 3.0),
        ("bits", 10.5),
        ("mix", 0.7),
    ] {
        set(p, f, k, v);
    }
}

/// Every block size gives the same samples, 8× oversampled, crushed and blended.
#[test]
fn fervor_does_not_depend_on_the_block_size() {
    let mut p = demo_project();
    let f = add_effect(&mut p, EffectKind::Saturator);
    busy(&mut p, f);
    let reference = render_with(&p, 512);
    assert!(reference.iter().all(|x| x.is_finite()));
    let dry = render_of(&demo_project());
    let moved = reference
        .iter()
        .zip(&dry)
        .filter(|(a, b)| (*a - *b).abs() > 1e-4)
        .count();
    assert!(moved > reference.len() / 10, "it colours: {moved}");
    for block in [1usize, 7, 64, 1000, 4096] {
        assert_null(
            &reference,
            &render_with(&p, block),
            0.0,
            &format!("Fervor at block {block}"),
        );
    }
}

/// All dry at 8× linear phase: the whole render is the project delayed by Fervor's
/// latency, to the bit — delay compensation moves every other path with it, and the dry
/// path inside the unit is a plain delay.
#[test]
fn a_dry_fervor_at_8x_is_the_project_delayed_by_its_latency() {
    let dry = render_of(&demo_project());
    let mut p = demo_project();
    let f = add_effect(&mut p, EffectKind::Saturator);
    busy(&mut p, f);
    set(&mut p, f, "mix", 0.0);
    let wet = render_of(&p);
    let lat = cantus_dsp::fervor::os::latency(8, false);
    assert_eq!(lat, 57);
    // Interleaved stereo: 2 values a frame.
    let shift = 2 * lat;
    assert!(wet[..shift].iter().all(|v| *v == 0.0));
    assert_null(&dry[..dry.len() - shift], &wet[shift..], 0.0, "delayed dry");
}

/// Through the contract: the latency follows the oversampling, the settings read as
/// written, and the telemetry reports levels, the match and the factor.
#[test]
fn latency_settings_and_telemetry_through_the_contract() {
    let mut unit = cantus_engine::fx::make(EffectKind::Saturator, 48_000.0).unwrap();
    let mut spec = EffectSpec::new(EffectKind::Saturator);
    unit.configure(&spec);
    assert_eq!(unit.latency(), 0);
    for (os, lat) in [(1.0, 47), (2.0, 55), (3.0, 57)] {
        spec.set("os", os);
        unit.configure(&spec);
        assert_eq!(unit.latency(), lat);
    }
    spec.set("phase", 1.0);
    unit.configure(&spec);
    let min = unit.latency();
    assert!((1..=8).contains(&min), "minimum phase: {min}");
    spec.set("drive", 12.0);
    spec.set("p3.on", 1.0);
    spec.set("p3.x", 0.4);
    spec.set("p3.y", 0.7);
    unit.configure(&spec);
    let s = cantus_engine::fx::saturator::settings(&spec);
    assert_eq!(s.oversampling, 8);
    assert!(s.minimum_phase);
    assert_eq!(s.curve.points[3].unwrap().y, 0.7);
    assert!(s.curve.points[2].is_none());
    // 0.3 s: long enough for the drive's 20 ms glide to land on its target.
    let mut l: Vec<f32> = (0..14_400)
        .map(|i| 0.25 * math::sin(i as f32 * 0.07))
        .collect();
    let mut r = l.clone();
    for (a, b) in l.chunks_mut(480).zip(r.chunks_mut(480)) {
        unit.process(a, b, None);
    }
    let t = unit.telemetry();
    assert_eq!(t[telemetry::BLOCKS], 30.0);
    assert_eq!(t[telemetry::FACTOR], 8.0);
    assert_eq!(t[telemetry::ENGAGED], 1.0);
    assert!((t[telemetry::IN_PEAK] - (-12.04)).abs() < 0.1);
    assert!(
        t[telemetry::MATCH] < -6.0,
        "12 dB of drive is taken back out"
    );
    assert!((t[telemetry::DRIVE] - 12.0).abs() < 1e-3);
}

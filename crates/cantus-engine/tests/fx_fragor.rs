//! Fragor (the pedals) inside the engine: the null tests first, then block-size
//! independence, the latency through delay compensation, a footswitch that never moves it,
//! and the telemetry through the plug-in contract.

mod support;
use cantus_core::*;
use cantus_dsp::fragor::telemetry;
use cantus_dsp::math;
use cantus_dsp::nulltest::assert_null;
use support::*;

fn set(p: &mut Project, effect: EffectId, key: &str, value: f32) {
    for s in &mut p.strips {
        if let Some(e) = s.effects.iter_mut().find(|e| e.id == effect) {
            e.spec.set(key, value).expect("a Fragor parameter");
        }
    }
}

/// Inserted and left alone, Fragor is a wire: the render is the render without it.
#[test]
fn fragor_at_its_defaults_renders_exactly_as_no_fragor() {
    let dry = render_of(&demo_project());
    let mut p = demo_project();
    add_effect(&mut p, EffectKind::Pedal);
    assert_null(&dry, &render_of(&p), 0.0, "Fragor at its defaults");
}

/// Knobs turned, the gate off and no pedal on the board: still a wire.
#[test]
fn knobs_without_a_pedal_touch_no_sample() {
    let dry = render_of(&demo_project());
    let mut p = demo_project();
    let f = add_effect(&mut p, EffectKind::Pedal);
    for (k, v) in [
        ("ge.fuzz", 1.0),
        ("od.drive", 0.9),
        ("mono", 1.0),
        ("os", 2.0),
    ] {
        set(&mut p, f, k, v);
    }
    assert_null(&dry, &render_of(&p), 0.0, "an empty board");
}

fn busy(p: &mut Project, f: EffectId) {
    for (k, v) in [
        ("s1", 1.0),
        ("s2", 4.0),
        ("s3", 5.0),
        ("od.drive", 0.8),
        ("ge.fuzz", 0.7),
        ("ge.bias", 0.6),
        ("mf.sustain", 0.6),
        ("gate", -60.0),
        ("mix", 0.8),
        ("os", 0.0),
    ] {
        set(p, f, k, v);
    }
}

/// Every block size gives the same samples, three pedals and the gate on.
#[test]
fn fragor_does_not_depend_on_the_block_size() {
    let mut p = demo_project();
    let f = add_effect(&mut p, EffectKind::Pedal);
    busy(&mut p, f);
    let reference = render_with(&p, 512);
    assert!(reference.iter().all(|x| x.is_finite()));
    let dry = render_of(&demo_project());
    let moved = reference
        .iter()
        .zip(&dry)
        .filter(|(a, b)| (*a - *b).abs() > 1e-4)
        .count();
    assert!(moved > reference.len() / 10, "it distorts: {moved}");
    for block in [1usize, 37, 64, 4096] {
        assert_null(
            &reference,
            &render_with(&p, block),
            0.0,
            &format!("Fragor at block {block}"),
        );
    }
}

/// Pedals on the board with every footswitch off: the whole render is the project delayed
/// by Fragor's latency, to the bit — a footswitch never moves the latency, and delay
/// compensation moves every other path with it.
#[test]
fn footswitches_off_are_the_project_delayed_by_the_latency() {
    let dry = render_of(&demo_project());
    let mut p = demo_project();
    let f = add_effect(&mut p, EffectKind::Pedal);
    busy(&mut p, f);
    set(&mut p, f, "gate", -90.0);
    for k in ["od.on", "op.on", "si.on", "ge.on", "mf.on"] {
        set(&mut p, f, k, 0.0);
    }
    let wet = render_of(&p);
    let lat = cantus_dsp::fervor::os::latency(2, false);
    let shift = 2 * lat;
    assert!(wet[..shift].iter().all(|v| *v == 0.0));
    assert_null(&dry[..dry.len() - shift], &wet[shift..], 0.0, "delayed dry");
}

/// Through the contract: the latency follows the oversampling while a pedal is on the
/// board, the settings read as written, and the telemetry reports levels and the chain.
#[test]
fn latency_settings_and_telemetry_through_the_contract() {
    let mut unit = cantus_engine::fx::make(EffectKind::Pedal, 48_000.0).unwrap();
    let mut spec = EffectSpec::new(EffectKind::Pedal);
    unit.configure(&spec);
    assert_eq!(unit.latency(), 0);
    spec.set("s2", 2.0);
    for (os, f) in [(0.0, 2), (1.0, 4), (2.0, 8)] {
        spec.set("os", os);
        unit.configure(&spec);
        assert_eq!(unit.latency(), cantus_dsp::fervor::os::latency(f, false));
    }
    spec.set("os", 1.0);
    spec.set("op.dist", 0.9);
    unit.configure(&spec);
    let s = cantus_engine::fx::fragor::settings(&spec);
    assert_eq!(s.chain[1], Some(cantus_dsp::fragor::Model::OpAmp));
    assert_eq!(s.oversampling, 4);
    let mut l: Vec<f32> = (0..14_400)
        .map(|i| 0.25 * math::sin(i as f32 * 0.07))
        .collect();
    let mut r = l.clone();
    for (a, b) in l.chunks_mut(480).zip(r.chunks_mut(480)) {
        unit.process(a, b, None);
    }
    let t = unit.telemetry();
    assert_eq!(t[telemetry::BLOCKS], 30.0);
    assert_eq!(t[telemetry::FACTOR], 4.0);
    assert_eq!(t[telemetry::ENGAGED], 1.0);
    assert_eq!(t[telemetry::CHAIN + 1], 2.0);
    assert!((t[telemetry::IN_PEAK] - (-12.04)).abs() < 0.1);
    assert!(t[telemetry::PEDAL_PEAK + 1] > -40.0);
    assert!(l.iter().all(|v| v.is_finite()));
}

//! Porta (the gate) inside the engine: the null tests before anything else, then the
//! sidechain route, the latency and the telemetry through the plug-in contract.

mod support;
use cantus_core::*;
use cantus_dsp::nulltest::assert_null;
use cantus_dsp::porta::telemetry;
use support::*;

fn set(p: &mut Project, effect: EffectId, key: &str, value: f32) {
    for s in &mut p.strips {
        if let Some(e) = s.effects.iter_mut().find(|e| e.id == effect) {
            e.spec.set(key, value).expect("a gate parameter");
        }
    }
}

/// At range 0 the gate is a wire: the render is the render without it, to the bit.
#[test]
fn a_gate_with_no_range_renders_exactly_as_no_gate() {
    let dry = render_of(&demo_project());
    let mut p = demo_project();
    let g = add_effect(&mut p, EffectKind::Gate);
    set(&mut p, g, "range", 0.0);
    set(&mut p, g, "thr", -10.0);
    assert_null(&dry, &render_of(&p), 0.0, "a gate with no range");
}

/// A gate that stays open is a wire: threshold at the floor, every sample above it
/// once the lead has started (the lead starts on the first sample and holds the gate).
#[test]
fn a_gate_that_never_closes_renders_exactly_as_no_gate() {
    let dry = render_of(&demo_project());
    let mut p = demo_project();
    let g = add_effect(&mut p, EffectKind::Gate);
    set(&mut p, g, "thr", -80.0);
    set(&mut p, g, "hyst", 24.0);
    set(&mut p, g, "hold", 500.0);
    // The lead's quietest moment inside its clip is well above −104 dBFS; after the clip
    // the gate may close, and there is nothing left to close on but its own tail — so
    // compare up to the end of the riff.
    let end = (8.0 * 60.0 / 128.0 * 48_000.0) as usize * 2;
    assert_null(&dry[..end], &render_of(&p)[..end], 0.0, "an open gate");
}

/// Every block size gives the same samples: a ducker keyed from the bass strip, with
/// lookahead, the key's filters and hold.
#[test]
fn a_keyed_gate_does_not_depend_on_the_block_size() {
    let mut p = demo_project();
    let g = add_effect(&mut p, EffectKind::Gate);
    for (k, v) in [
        ("style", 5.0),
        ("thr", -30.0),
        ("range", 18.0),
        ("look", 4.0),
        ("hpf", 60.0),
        ("lpf", 2_000.0),
        ("hold", 30.0),
    ] {
        set(&mut p, g, k, v);
    }
    let bass = p.tracks[1].strip.unwrap();
    strip_of(&mut p, 0).effects[0].sidechain = Some(bass);
    let reference = render_with(&p, 512);
    assert!(reference.iter().all(|x| x.is_finite()));
    for block in [1usize, 7, 64, 1000, 4096] {
        assert_null(
            &reference,
            &render_with(&p, block),
            0.0,
            &format!("keyed gate at block {block}"),
        );
    }
}

/// Keyed from the bass, the Ducking style moves the lead only where the bass plays: the
/// render is untouched before the bass comes in (beat 2) and different after.
#[test]
fn a_ducker_keyed_from_another_strip_follows_that_strip() {
    let dry = render_of(&demo_project());
    let mut p = demo_project();
    let g = add_effect(&mut p, EffectKind::Gate);
    for (k, v) in [("style", 5.0), ("thr", -40.0), ("range", 12.0)] {
        set(&mut p, g, k, v);
    }
    let bass = p.tracks[1].strip.unwrap();
    strip_of(&mut p, 0).effects[0].sidechain = Some(bass);
    let wet = render_of(&p);
    let beat2 = (2.0 * 60.0 / 128.0 * 48_000.0) as usize * 2;
    assert_null(&dry[..beat2], &wet[..beat2], 0.0, "before the key sounds");
    let moved = dry[beat2..]
        .iter()
        .zip(&wet[beat2..])
        .map(|(a, b)| (a - b).abs())
        .fold(0.0f32, f32::max);
    assert!(moved > 0.01, "the bass should duck the lead: moved {moved}");
}

/// A closed gate with ∞ range is exact silence: gating the lead away leaves the render
/// of the bass alone, to the bit, once the gate has shut.
#[test]
fn a_shut_gate_is_exact_silence() {
    let mut alone = demo_project();
    alone.tracks[0].muted = true;
    let bass_only = render_of(&alone);
    let mut p = demo_project();
    let g = add_effect(&mut p, EffectKind::Gate);
    for (k, v) in [("thr", 0.0), ("range", 100.0), ("hold", 0.0), ("rel", 1.0)] {
        set(&mut p, g, k, v);
    }
    let from = (0.05 * 48_000.0) as usize * 2;
    assert_null(
        &bass_only[from..],
        &render_of(&p)[from..],
        0.0,
        "a shut gate",
    );
}

/// Through the contract: the latency is the lookahead's fixed 10 ms, and the telemetry
/// counts columns and hits.
#[test]
fn latency_and_telemetry_through_the_contract() {
    let mut unit = cantus_engine::fx::make(EffectKind::Gate, 48_000.0).unwrap();
    let mut spec = EffectSpec::new(EffectKind::Gate);
    assert_eq!(unit.latency(), 0, "no lookahead, no latency");
    spec.set("look", 5.0);
    unit.configure(&spec);
    assert_eq!(unit.latency(), 480, "lookahead costs a fixed 10 ms");
    // One second: a hit every 250 ms over silence.
    let mut l = vec![0.0f32; 48_000];
    for h in 0..4 {
        for (j, v) in l[h * 12_000..h * 12_000 + 2_400].iter_mut().enumerate() {
            *v = 0.5 * (j as f32 * 0.05).cos() * (1.0 - j as f32 / 2_400.0);
        }
    }
    let mut r = l.clone();
    for (a, b) in l.chunks_mut(480).zip(r.chunks_mut(480)) {
        unit.process(a, b, None);
    }
    let t = unit.telemetry();
    assert_eq!(t[telemetry::SEQ], 200.0);
    assert_eq!(t[telemetry::LATENCY], 480.0);
    assert_eq!(t[telemetry::HITS], 4.0);
    let newest = t[telemetry::EVENTS_AT] as usize;
    assert_eq!(48_000 - newest, 36_000, "the last hit, where it started");
    let settings = cantus_engine::fx::gate::settings(&spec);
    assert_eq!(settings.lookahead_ms, 5.0);
    assert_eq!(settings.threshold_db, -40.0);
}

//! Spatium in the engine: the null tests of the house rule through the whole processor,
//! the parameter model and the DSP agreeing, and ducking keyed from another strip.

use cantus_dsp::math;

mod support;
use cantus_core::*;
use cantus_dsp::nulltest::assert_null;
use cantus_engine::fx::space::{settings, tel, Unit};
use cantus_engine::FxUnit;
use support::*;

/// Set a parameter on the lead strip's reverb.
fn set(p: &mut Project, fx: EffectId, key: &str, v: f32) {
    let s = strip_of(p, 0);
    let slot = s.effects.iter_mut().find(|e| e.id == fx).unwrap();
    slot.spec.set(key, v).unwrap();
}

#[test]
fn the_parameter_defaults_are_the_dsp_defaults() {
    let spec = EffectSpec::new(EffectKind::Space);
    assert_eq!(
        settings(&spec),
        cantus_dsp::spatium::SpatiumSettings::default()
    );
}

#[test]
fn a_band_is_active_only_when_used_and_on() {
    let mut spec = EffectSpec::new(EffectKind::Space);
    spec.set("d2.mult", 2.0);
    assert!(!settings(&spec).decay_bands[2].active, "not used yet");
    spec.set("d2.used", 1.0);
    let s = settings(&spec);
    assert!(s.decay_bands[2].active);
    assert_eq!(s.decay_bands[2].mult, 2.0);
    spec.set("d2.on", 0.0);
    assert!(!settings(&spec).decay_bands[2].active, "bypassed");
    spec.set("e4.used", 1.0);
    spec.set("e4.shape", 3.0);
    assert_eq!(
        settings(&spec).post_bands[4].shape,
        cantus_dsp::EqShape::LowCut
    );
}

/// At mix 0 the reverb is a wire: the render is the same, to the bit, as with no reverb
/// at all — with a busy setting and a sidechain routed.
#[test]
fn mix_zero_renders_exactly_as_without_it() {
    let reference = render_of(&demo_project());
    let mut p = demo_project();
    let fx = add_effect(&mut p, EffectKind::Space);
    let bass = p.tracks[1].strip.unwrap();
    for (k, v) in [
        ("mix", 0.0),
        ("character", 0.9),
        ("duck", 12.0),
        ("d0.used", 1.0),
        ("d0.mult", 3.0),
        ("e0.used", 1.0),
        ("e0.gain", 6.0),
        ("freeze", 1.0),
    ] {
        set(&mut p, fx, k, v);
    }
    strip_of(&mut p, 0).effects[0].sidechain = Some(bass);
    assert_null(&reference, &render_of(&p), 0.0, "Spatium at mix 0");
}

/// Busy settings, keyed from the bass: the same samples at every block size.
#[test]
fn the_block_size_changes_nothing_in_the_engine() {
    let mut p = demo_project();
    let fx = add_effect(&mut p, EffectKind::Space);
    let bass = p.tracks[1].strip.unwrap();
    for (k, v) in [
        ("mix", 0.5),
        ("character", 0.8),
        ("duck", 9.0),
        ("duck_thr", -40.0),
        ("space", 0.3),
        ("predelay", 23.0),
        ("d1.used", 1.0),
        ("d1.shape", 2.0),
        ("d1.freq", 3_000.0),
        ("d1.mult", 0.5),
        ("e1.used", 1.0),
        ("e1.shape", 3.0),
        ("e1.freq", 200.0),
    ] {
        set(&mut p, fx, k, v);
    }
    strip_of(&mut p, 0).effects[0].sidechain = Some(bass);
    let reference = render_with(&p, 512);
    assert!(reference.iter().all(|x| x.is_finite()));
    for block in [1usize, 7, 64, 1000, 4096] {
        assert_null(
            &reference,
            &render_with(&p, block),
            0.0,
            &format!("block {block}"),
        );
    }
}

/// Keyed from the bass, the lead's reverb ducks only while the bass plays: before the bass
/// comes in (beat 2) the render is the same, to the bit, as with no ducking at all; after,
/// it is quieter.
#[test]
fn ducking_listens_to_another_strip() {
    let mut p = demo_project();
    let fx = add_effect(&mut p, EffectKind::Space);
    set(&mut p, fx, "mix", 1.0);
    let open = render_of(&p);
    set(&mut p, fx, "duck", 24.0);
    set(&mut p, fx, "duck_thr", -50.0);
    let bass = p.tracks[1].strip.unwrap();
    strip_of(&mut p, 0).effects[0].sidechain = Some(bass);
    let keyed = render_of(&p);
    // Beat 2 at 128 BPM is 0.469 s; the render is interleaved stereo.
    let before = (0.45 * 48_000.0) as usize * 2;
    assert_null(&open[..before], &keyed[..before], 0.0, "before the bass");
    // The master holds the bass too: take it out, so what is compared is the lead's
    // reverb alone.
    let mut bass_only = p.clone();
    strip_of(&mut bass_only, 0).gain_db = MIN_GAIN_DB;
    let bass_only = render_of(&bass_only);
    let energy = |x: &[f32]| {
        x[before..]
            .iter()
            .zip(&bass_only[before..])
            .map(|(v, b)| math::powi((*v - *b) as f64, 2))
            .sum::<f64>()
    };
    let drop = 10.0 * math::log10(energy(&keyed) / energy(&open));
    eprintln!("keyed from the bass: {drop:.1} dB after it comes in");
    assert!(drop < -3.0, "the keyed reverb is not quieter: {drop} dB");
}

/// The telemetry says what the engine is doing: alive, the decay it achieves per octave
/// (within 4 % of the curve at 1 kHz), the pre-delay in force.
#[test]
fn the_telemetry_reports_the_achieved_decay() {
    let mut spec = EffectSpec::new(EffectKind::Space);
    spec.set("decay", 2.5);
    spec.set("predelay", 40.0);
    let mut u = Unit::new(48_000.0);
    u.configure(&spec);
    u.reset();
    let mut l = vec![0.0; 256];
    let mut r = vec![0.0; 256];
    u.process(&mut l, &mut r, None);
    let t = u.telemetry();
    assert_eq!(t[tel::ALIVE], 1.0);
    let at_1k = t[tel::ACHIEVED + 4];
    assert!((at_1k / 2.5 - 1.0).abs() < 0.04, "{at_1k}");
    assert!((t[tel::PREDELAY_MS] - 40.0).abs() < 0.01);
}

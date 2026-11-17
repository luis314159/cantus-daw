//! Lenis, the resonance suppressor, through the whole engine (ADR 0143, provisional): at
//! its defaults the mix later by exactly the latency it reports — delay compensation
//! holding the bass back with the lead — at each of the three qualities; a quality change
//! reported to the engine as the new latency; the starting preset sounds, the same at every
//! block size and live as offline; the delta is what the processing takes away; a fork
//! carries on as the unit does. // fxb:lenis

#[path = "support/modfx.rs"]
mod modfx;
mod support;
use cantus_core::*;
use cantus_dsp::nulltest::assert_null;
use cantus_engine::EffectUnit;
use modfx::*;
use support::*;

const SR: f32 = 48_000.0;

fn differs(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).fold(0f32, |m, (x, y)| m.max((x - y).abs()))
}

/// `out` is `reference` exactly `latency` frames later (interleaved stereo).
fn assert_delayed(reference: &[f32], out: &[f32], latency: usize, what: &str) {
    let shift = 2 * latency;
    assert!(
        out[..shift].iter().all(|x| *x == 0.0),
        "{what}: sound before the latency"
    );
    assert_null(
        &reference[..reference.len() - shift],
        &out[shift..],
        0.0,
        what,
    );
}

fn unit_of(p: &mut Project, fx: EffectId) -> EffectUnit {
    let slot = strip_of(p, 0)
        .effects
        .iter()
        .find(|e| e.id == fx)
        .unwrap()
        .clone();
    EffectUnit::new(&slot, SR)
}

#[test]
fn at_its_defaults_it_is_the_dry_mix_delayed_by_its_latency_at_every_quality() {
    let reference = render_of(&demo_project());
    for (qual, want) in [(1.0, 511), (0.0, 255), (2.0, 1_023)] {
        let mut p = demo_project();
        let fx = add_effect(&mut p, EffectKind::ResonanceSuppressor);
        set(&mut p, fx, "qual", qual);
        assert_eq!(unit_of(&mut p, fx).latency(), want, "quality {qual}");
        assert_delayed(
            &reference,
            &render_of(&p),
            want,
            &format!("Lenis at its defaults, quality {qual}"),
        );
    }
}

/// A quality change is taken at the unit's next hop, which starts again from silence; the
/// latency it reports from then on is the new frame's, which the engine's compensation
/// reads each pass (ADR 0143). Through the engine: a project saved at Precise renders the
/// mix later by Precise's latency from its first sample.
#[test]
fn a_quality_change_reports_its_new_latency() {
    let mut p = demo_project();
    let fx = add_effect(&mut p, EffectKind::ResonanceSuppressor);
    set_all(&mut p, fx, &[("depth", 0.6)]);
    let mut u = unit_of(&mut p, fx);
    assert_eq!(u.latency(), 511);
    let tone: Vec<f32> = (0..4_800)
        .map(|i| 0.3 * cantus_dsp::math::sin(i as f32 * 0.39))
        .collect();
    let (mut l, mut r) = (tone.clone(), tone.clone());
    u.process(&mut l, &mut r);
    let mut slot = strip_of(&mut p, 0).effects[0].clone();
    slot.spec.set("qual", 2.0).unwrap();
    u.configure(&slot.spec);
    let (mut l, mut r) = (tone.clone(), tone.clone());
    u.process(&mut l, &mut r);
    assert_eq!(u.latency(), 1_023, "the new frame's latency, reported");
    slot.spec.set("qual", 0.0).unwrap();
    u.configure(&slot.spec);
    let (mut l, mut r) = (tone.clone(), tone);
    u.process(&mut l, &mut r);
    assert_eq!(u.latency(), 255);
}

#[test]
fn the_starting_preset_sounds_and_renders_the_same_every_way() {
    let mut p = demo_project();
    let mut slot = p.make_effect(EffectKind::ResonanceSuppressor);
    slot.spec = cantus_core::starting::starting_spec(EffectKind::ResonanceSuppressor);
    strip_of(&mut p, 0).effects.push(slot);
    let a = render_of(&p);
    let reference = render_of(&demo_project());
    let shift = 2 * 511;
    assert!(
        differs(&a[shift..], &reference[..reference.len() - shift]) > 1e-4,
        "it sounds"
    );
    let b = realtime(&p, a.len() / 2);
    assert_null(&a[..b.len()], &b, 0.0, "real time equals offline");
    for block in [1usize, 37, 64, 512] {
        assert_null(
            &a,
            &render_with(&p, block),
            0.0,
            &format!("blocks of {block}"),
        );
    }
}

/// The delta (listen to what is removed): at depth 0 nothing is removed, so the mix is the
/// other strips alone, to the bit; at a working depth the delta and the processed output
/// add up to the input delayed (within the float sum's rounding).
#[test]
fn the_delta_is_what_the_processing_takes_away() {
    // Lead only, so the sum is the lead's alone.
    let solo = lead_only();
    let mut d = solo.clone();
    let fx = add_effect(&mut d, EffectKind::ResonanceSuppressor);
    set_all(&mut d, fx, &[("delta", 1.0)]);
    let nothing = render_of(&d);
    assert!(
        nothing.iter().all(|x| *x == 0.0),
        "depth 0: nothing removed"
    );
    set_all(
        &mut d,
        fx,
        &[("depth", 0.9), ("curve", 1.0), ("select", 0.2)],
    );
    let removed = render_of(&d);
    let mut w = d.clone();
    set(&mut w, fx, "delta", 0.0);
    let kept = render_of(&w);
    let dry = render_of(&solo);
    let shift = 2 * 511;
    assert!(
        removed.iter().any(|x| x.abs() > 1e-5),
        "something is removed"
    );
    let worst = removed[shift..]
        .iter()
        .zip(&kept[shift..])
        .zip(&dry)
        .fold(0f32, |m, ((a, b), c)| m.max((a + b - c).abs()));
    assert!(worst < 1e-5, "delta + output = the input delayed ({worst})");
}

/// A fork carries on as the unit does (the detectors' averages, the rings, the overlap,
/// the frame grid copied).
#[test]
fn a_fork_carries_on_from_where_the_unit_is() {
    let mut p = demo_project();
    let mut slot = p.make_effect(EffectKind::ResonanceSuppressor);
    slot.spec = support::heavy::heavy(EffectKind::ResonanceSuppressor);
    let mut a = EffectUnit::new(&slot, SR);
    let music = |i: usize| {
        let t = i as f32 / SR;
        0.3 * cantus_dsp::math::sin(t * 3_000.0 * std::f32::consts::TAU)
            + 0.2 * cantus_dsp::math::sin(t * 220.0 * std::f32::consts::TAU)
    };
    let block = |u: &mut EffectUnit, at: usize| {
        let mut l: Vec<f32> = (at..at + 480).map(music).collect();
        let mut r: Vec<f32> = l.iter().map(|x| -0.5 * x).collect();
        u.process(&mut l, &mut r);
        (l, r)
    };
    let mut at = 0;
    for _ in 0..100 {
        block(&mut a, at);
        at += 480;
    }
    let mut b = a.fork().expect("a fork");
    for _ in 0..100 {
        let x = block(&mut a, at);
        let y = block(&mut b, at);
        assert_eq!(x, y, "at {at}");
        at += 480;
    }
}

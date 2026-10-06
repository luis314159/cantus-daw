//! Terminus through the engine: the project's parameters reach the DSP, a limiter with
//! nothing to do is a delay to the bit, the render does not depend on the block size, and
//! a mix pushed hard into it leaves the engine under the ceiling in true peak.

use cantus_dsp::math;

mod support;
use cantus_core::*;
use cantus_dsp::nulltest::assert_null;
use cantus_dsp::terminus::{latency_of, os};
use cantus_engine::fx::limiter::settings;
use support::*;

/// The lead alone, centred, at unity on its strip: what leaves the limiter is what leaves
/// the engine (up to the pan law's rounding).
fn lead_alone() -> Project {
    let mut p = demo_project();
    p.tracks.truncate(1);
    let s = strip_of(&mut p, 0);
    s.pan = 0.0;
    s.gain_db = 0.0;
    p
}

fn set(p: &mut Project, fx: EffectId, key: &str, value: f32) {
    let s = strip_of(p, 0);
    let slot = s.effects.iter_mut().find(|e| e.id == fx).unwrap();
    slot.spec.set(key, value).expect("a limiter parameter");
}

fn frames(x: &[f32], ch: usize) -> Vec<f32> {
    x.chunks_exact(2).map(|f| f[ch]).collect()
}

/// The null test: with 0 dB of gain and the ceiling at 0 dBFS over a mix that peaks lower,
/// the render with the limiter is the render without it, delayed by the latency the unit
/// reports — or not delayed at all, once the engine compensates plug-in latency.
#[test]
fn a_limiter_with_nothing_to_do_is_a_delay_to_the_bit() {
    let plain = lead_alone();
    let dry = render_of(&plain);
    let peak = dry.iter().fold(0f32, |m, v| m.max(v.abs()));
    assert!(peak < 0.9, "the test mix is too loud for this test: {peak}");
    for tp in [0.0, 1.0] {
        let mut p = plain.clone();
        let fx = add_effect(&mut p, EffectKind::Limiter);
        set(&mut p, fx, "ceiling", 0.0);
        set(&mut p, fx, "tp", tp);
        let spec = strip_of(&mut p, 0).effects[0].spec.clone();
        let lat = latency_of(&settings(&spec), 48_000.0);
        let unit = cantus_engine::EffectUnit::new(&strip_of(&mut p, 0).effects[0], 48_000.0);
        assert_eq!(unit.latency(), lat, "the unit reports the DSP's latency");
        let wet = render_of(&p);
        let n = dry.len() / 2;
        let exact = |shift: usize| {
            (0..n).all(|i| {
                let want = if i >= shift {
                    (dry[2 * (i - shift)], dry[2 * (i - shift) + 1])
                } else {
                    (0.0, 0.0)
                };
                (wet[2 * i], wet[2 * i + 1]) == want
            })
        };
        assert!(
            exact(lat) || exact(0),
            "tp={tp}: not the input delayed by {lat} samples (nor by 0)"
        );
    }
}

#[test]
fn the_render_does_not_depend_on_the_block_size() {
    let mut p = demo_project();
    let fx = add_effect(&mut p, EffectKind::Limiter);
    for (k, v) in [
        ("gain", 14.0),
        ("style", 2.0),
        ("clip", 0.4),
        ("knee", 3.0),
        ("dither", 5.0),
        ("shape", 2.0),
        ("monitor", 2.0),
        ("dc", 1.0),
    ] {
        set(&mut p, fx, k, v);
    }
    let reference = render_with(&p, 4096);
    assert!(reference.iter().all(|x| x.is_finite()));
    for block in [1usize, 7, 64, 1000] {
        assert_null(
            &reference,
            &render_with(&p, block),
            0.0,
            &format!("limiter at block size {block}"),
        );
    }
}

/// Pushed 18 dB into a −1 dBTP ceiling, the engine's output is under −1 dBTP by an
/// independent 32× reconstruction — and with true peak off, its samples are under −1 dBFS.
#[test]
fn a_mix_pushed_hard_leaves_the_engine_under_the_ceiling() {
    for tp in [1.0, 0.0] {
        let mut p = lead_alone();
        let fx = add_effect(&mut p, EffectKind::Limiter);
        set(&mut p, fx, "gain", 18.0);
        set(&mut p, fx, "ceiling", -1.0);
        set(&mut p, fx, "tp", tp);
        let out = render_of(&p);
        for ch in 0..2 {
            let x = frames(&out, ch);
            let sample = 20.0 * math::log10(x.iter().fold(0f32, |m, v| m.max(v.abs())));
            assert!(sample <= -1.0, "tp={tp}: sample peak {sample} dBFS");
            if tp > 0.5 {
                let tp_db = 20.0 * math::log10(os::reference_true_peak(&x, 0.5));
                assert!(tp_db <= -1.0, "true peak {tp_db} dBTP");
            }
        }
    }
}

/// The parameters reach the DSP as the model describes them.
#[test]
fn the_parameters_read_as_the_model_says() {
    let mut spec = EffectSpec::new(EffectKind::Limiter);
    let s = settings(&spec);
    assert_eq!(s.gain_db, 0.0);
    assert_eq!(s.ceiling_db, -1.0);
    assert!(s.true_peak, "true peak is on by default");
    assert_eq!(s.dither, 0, "dither is off by default");
    spec.set("style", 4.0);
    spec.set("monitor", 1.0);
    spec.set("dither", 5.0);
    spec.set("shape", 2.0);
    let s = settings(&spec);
    assert_eq!(s.style, cantus_dsp::terminus::LimStyle::Safe);
    assert_eq!(s.monitor, cantus_dsp::terminus::Monitor::Unity);
    assert_eq!(cantus_dsp::terminus::DITHER_BITS[s.dither], 16);
    assert_eq!(s.shaping, cantus_dsp::terminus::Shaping::Optimized);
}

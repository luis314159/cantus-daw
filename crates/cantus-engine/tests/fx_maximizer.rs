//! Culmen (the maximizer) through the engine: a new one is a delay to the bit, the render
//! does not depend on the block size, a mix driven hard stays under the true-peak ceiling,
//! and the telemetry reaches the editor through the plug-in contract.

use cantus_dsp::math;

mod support;
use cantus_core::*;
use cantus_dsp::culmen::telemetry;
use cantus_dsp::nulltest::assert_null;
use cantus_dsp::terminus::os;
use cantus_engine::fx::maximizer::settings;
use support::*;

/// The lead alone, centred, at unity on its strip.
fn lead_alone() -> Project {
    let mut p = demo_project();
    p.tracks.truncate(1);
    let s = strip_of(&mut p, 0);
    s.pan = 0.0;
    s.gain_db = 0.0;
    // Softer notes, so the lead (which Culmen hears before the fader) peaks well under the
    // default −1 dBTP ceiling: a peak that touches it is limited, rightly.
    for clip in &mut p.tracks[0].clips {
        for n in &mut clip.notes {
            n.velocity = (n.velocity / 3).max(1);
        }
    }
    p
}

fn set(p: &mut Project, fx: EffectId, key: &str, value: f32) {
    for s in &mut p.strips {
        if let Some(e) = s.effects.iter_mut().find(|e| e.id == fx) {
            e.spec.set(key, value).expect("a Culmen parameter");
        }
    }
}

fn frames(x: &[f32], ch: usize) -> Vec<f32> {
    x.chunks_exact(2).map(|f| f[ch]).collect()
}

/// Exactly the input, delayed by `shift` frames.
fn delayed_by(dry: &[f32], wet: &[f32], shift: usize) -> bool {
    let n = dry.len() / 2;
    (0..n).all(|i| {
        let want = if i >= shift {
            (dry[2 * (i - shift)], dry[2 * (i - shift) + 1])
        } else {
            (0.0, 0.0)
        };
        (wet[2 * i], wet[2 * i + 1]) == want
    })
}

/// The null test: a new Culmen (nothing stored) over a mix under its ceiling renders the
/// mix delayed by the latency the unit reports — or not delayed, where the engine
/// compensates — to the bit; so do idle bands in each crossover mode.
#[test]
fn a_new_culmen_renders_the_mix_delayed_to_the_bit() {
    let plain = lead_alone();
    let dry = render_of(&plain);
    let peak = dry.iter().fold(0f32, |m, v| m.max(v.abs()));
    assert!(
        peak < 0.85,
        "the test mix is too loud for this test: {peak}"
    );
    for mode in [0.0, 1.0, 2.0] {
        let mut p = plain.clone();
        let fx = add_effect(&mut p, EffectKind::Maximizer);
        set(&mut p, fx, "xmode", mode);
        let slot = strip_of(&mut p, 0).effects[0].clone();
        let unit = cantus_engine::EffectUnit::new(&slot, 48_000.0);
        let lat = unit.latency();
        let wet = render_of(&p);
        assert!(
            delayed_by(&dry, &wet, lat) || delayed_by(&dry, &wet, 0),
            "xmode {mode}: not the mix delayed by {lat} samples (nor by 0); first difference {:?}",
            first_difference(&dry, &wet, lat)
        );
    }
}

/// A busy Culmen — every band working, the full band, the drive, the clipper, dither,
/// the match monitor — renders the same samples at every block size.
#[test]
fn the_render_does_not_depend_on_the_block_size() {
    let mut p = demo_project();
    let fx = add_effect(&mut p, EffectKind::Maximizer);
    for (k, v) in [
        ("gain", 10.0),
        ("xmode", 1.0),
        ("lo.thr", -30.0),
        ("lo.ratio", 3.0),
        ("mid.thr_lo", -40.0),
        ("mid.ratio_lo", 2.0),
        ("hi.ratio", 0.7),
        ("all.thr", -20.0),
        ("all.ratio", 2.0),
        ("clip", 0.3),
        ("dither", 5.0),
        ("shape", 2.0),
        ("monitor", 2.0),
    ] {
        set(&mut p, fx, k, v);
    }
    let reference = render_with(&p, 512);
    assert!(reference.iter().all(|x| x.is_finite()));
    for block in [1usize, 7, 64, 1000, 4096] {
        assert_null(
            &reference,
            &render_with(&p, block),
            0.0,
            &format!("busy Culmen at block {block}"),
        );
    }
}

/// Driven 18 dB into a −1 dBTP ceiling, the mix leaves the engine under −1 dBTP by the
/// 32× reference.
#[test]
fn a_mix_driven_hard_stays_under_the_true_peak_ceiling() {
    let plain = lead_alone();
    let mut p = plain.clone();
    let fx = add_effect(&mut p, EffectKind::Maximizer);
    for (k, v) in [
        ("gain", 18.0),
        ("ceiling", -1.0),
        ("all.thr", -24.0),
        ("all.ratio", 2.0),
    ] {
        set(&mut p, fx, k, v);
    }
    let wet = render_of(&p);
    let ceiling = math::powf(10f32, -1.0 / 20.0);
    for ch in 0..2 {
        let x = frames(&wet, ch);
        let tp = os::reference_true_peak(&x, 1.0);
        assert!(
            tp <= ceiling * math::powf(10f32, 0.02 / 20.0),
            "channel {ch}: true peak {:.3} dBTP",
            20.0 * math::log10(tp)
        );
    }
    let louder = |x: &[f32]| x.iter().map(|v| (v * v) as f64).sum::<f64>();
    assert!(
        louder(&wet) > 2.0 * louder(&render_of(&plain)),
        "18 dB of drive makes it louder"
    );
}

/// The parameters reach the DSP as the model says, and the unit's telemetry counts its
/// columns and names its latency.
#[test]
fn the_unit_reads_the_spec_and_reports_its_columns() {
    let mut spec = EffectSpec::new(EffectKind::Maximizer);
    spec.set("gain", 6.0).unwrap();
    spec.set("xmode", 2.0).unwrap();
    let s = settings(&spec);
    assert_eq!(s.drive_db, 6.0);
    let slot = EffectSlot {
        id: EffectId(9),
        bypass: false,
        spec,
        mix: 1.0,
        sidechain: None,
        midi_from: None,
    };
    let mut unit = cantus_engine::EffectUnit::new(&slot, 48_000.0);
    let lat = unit.latency();
    assert!(lat > 2_000, "the linear-phase split is reported: {lat}");
    let mut l = vec![0.25f32; 4_800];
    let mut r = vec![0.25f32; 4_800];
    unit.process_keyed(&mut l, &mut r, None);
    let t = unit.telemetry().expect("Culmen reports telemetry");
    assert_eq!(t[telemetry::SEQ], 10.0, "ten columns in 100 ms");
    assert_eq!(t[telemetry::LATENCY] as usize, lat);
    assert_eq!(t[telemetry::RATE], 48_000.0);
}

/// Where `wet` first differs from `dry` delayed by `shift` frames: (frame, wet, dry).
fn first_difference(dry: &[f32], wet: &[f32], shift: usize) -> Option<(usize, f32, f32)> {
    let n = dry.len() / 2;
    (0..n).find_map(|i| {
        let want = if i >= shift {
            dry[2 * (i - shift)]
        } else {
            0.0
        };
        (wet[2 * i] != want).then_some((i, wet[2 * i], want))
    })
}

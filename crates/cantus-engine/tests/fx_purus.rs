//! Purus, the real-time repair unit, through the whole engine (ADR 0145, provisional): at
//! its defaults (every module off) the mix untouched, to the bit, no latency; each module's
//! latency reported as it is switched (the de-reverb 23.2 ms at 44.1 kHz), and the
//! engine's compensation following it; the starting preset takes a hum out; offline equals
//! real time and every block size gives the same bits; a fork carries on as the unit does;
//! the de-click's sensitivity stops at 0.7 and the Direct sound's default is one constant.
//! // fxc:purus

#[path = "support/modfx.rs"]
mod modfx;
mod support;
use cantus_core::*;
use cantus_dsp::nulltest::assert_null;
use cantus_engine::fx::purus::{settings, t};
use cantus_engine::EffectUnit;
use modfx::*;
use support::*;

const SR: f32 = 48_000.0;

fn slot_of(spec: EffectSpec) -> EffectSlot {
    let mut p = demo_project();
    let mut s = p.make_effect(spec.kind);
    s.spec = spec;
    s
}

#[test]
fn at_its_defaults_it_touches_no_sample() {
    let reference = render_of(&demo_project());
    let mut p = demo_project();
    let fx = add_effect(&mut p, EffectKind::Repair);
    let slot = strip_of(&mut p, 0)
        .effects
        .iter()
        .find(|e| e.id == fx)
        .unwrap()
        .clone();
    assert_eq!(EffectUnit::new(&slot, SR).latency(), 0);
    assert_null(&reference, &render_of(&p), 0.0, "Purus at its defaults");
}

/// Each module's latency as it is switched on, at 48 and 44.1 kHz: the notches none, the
/// gentle de-hum its 50 ms look-ahead, the de-click 767 samples, the de-reverb N − 1
/// (1 023: 21.3 ms at 48 kHz, 23.2 ms at 44.1 kHz) — the telemetry says the same.
#[test]
fn the_latency_is_the_modules_that_are_on() {
    for (rate, gentle, verb) in [
        (48_000.0f32, 2_400usize, 1_023usize),
        (44_100.0, 2_205, 1_023),
    ] {
        let lat = |kv: &[(&str, f32)]| {
            let mut s = EffectSpec::new(EffectKind::Repair);
            for (k, v) in kv {
                s.set(k, *v).unwrap();
            }
            let u = EffectUnit::new(&slot_of(s), rate);
            let tel = u.telemetry().unwrap();
            assert_eq!(tel[t::LATENCY] as usize, u.latency());
            u.latency()
        };
        assert_eq!(lat(&[("hon", 1.0)]), 0, "notches");
        assert_eq!(lat(&[("hon", 1.0), ("hgentle", 1.0)]), gentle, "gentle");
        assert_eq!(lat(&[("con", 1.0)]), 767, "de-click");
        assert_eq!(lat(&[("von", 1.0)]), verb, "de-reverb");
        assert_eq!(
            lat(&[("hon", 1.0), ("hgentle", 1.0), ("con", 1.0), ("von", 1.0)]),
            gentle + 767 + verb,
            "all three"
        );
    }
    let ms = 1_023.0 / 44_100.0 * 1_000.0;
    assert!((ms - 23.2f64).abs() < 0.05, "{ms} ms");
}

/// The engine compensates the de-reverb's latency: Purus with only the de-reverb on at mix
/// 0 on the lead is the dry project to the bit (its strip delayed and every other path
/// aligned to it: the mix is the dry mix, later by the latency, on the master).
#[test]
fn its_latency_is_compensated() {
    let mut p = demo_project();
    let fx = add_effect(&mut p, EffectKind::Repair);
    set_all(&mut p, fx, &[("von", 1.0), ("vmix", 0.0)]);
    let dry = render_of(&demo_project());
    let wet = render_of(&p);
    let lag = 2 * 1_023;
    // The render reports the latency at its start or trims it: either it is the dry mix
    // to the bit, or the dry mix later by exactly the latency.
    let same = wet.len() >= dry.len() && wet[..dry.len()] == dry[..];
    let later = wet.len() > lag
        && wet[lag..]
            .iter()
            .zip(&dry)
            .all(|(a, b)| a.to_bits() == b.to_bits());
    assert!(same || later, "compensated: the dry mix, aligned");
}

/// The starting preset (Mains 50 Hz, followed): a 50 Hz hum and its harmonics on the lead
/// come down by tens of dB; live = offline, every block size the same bits.
#[test]
fn the_starting_preset_takes_a_hum_out_and_renders_the_same_every_way() {
    let mut p = demo_project();
    let mut slot = p.make_effect(EffectKind::Repair);
    slot.spec = cantus_core::starting::starting_spec(EffectKind::Repair);
    assert_eq!(slot.spec.get("hon"), 1.0);
    assert_eq!(slot.spec.get("hmode"), 1.0);
    strip_of(&mut p, 0).effects.push(slot);
    let a = render_of(&p);
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
    // On a hum itself (the unit alone): 50 Hz and three harmonics at −20 dBFS, the
    // fundamental 0.3 Hz off its nominal (followed).
    let mut u = EffectUnit::new(
        &slot_of(cantus_core::starting::starting_spec(EffectKind::Repair)),
        SR,
    );
    let tone = |i: usize| {
        let x = i as f32 / SR;
        (1..=4)
            .map(|k| {
                0.1 / k as f32 * cantus_dsp::math::sin(x * 50.3 * k as f32 * std::f32::consts::TAU)
            })
            .sum::<f32>()
    };
    let (mut before, mut after) = (0f64, 0f64);
    for b in 0..400 {
        let mut l: Vec<f32> = (b * 480..b * 480 + 480).map(tone).collect();
        let mut r = l.clone();
        let dry = l.clone();
        u.process(&mut l, &mut r);
        if b >= 300 {
            before += dry
                .iter()
                .map(|v| f64::from(*v) * f64::from(*v))
                .sum::<f64>();
            after += l.iter().map(|v| f64::from(*v) * f64::from(*v)).sum::<f64>();
        }
    }
    let db = 10.0 * cantus_dsp::math::log10(after / before);
    assert!(db < -25.0, "the hum came down {db:.1} dB");
}

/// A fork carries on as the unit does (the de-hum's filters and tracker, the de-click's
/// rings, the de-reverb's frames and the glides copied).
#[test]
fn a_fork_carries_on_from_where_the_unit_is() {
    let slot = slot_of(support::heavy::heavy(EffectKind::Repair));
    let mut a = EffectUnit::new(&slot, SR);
    let run = |u: &mut EffectUnit, from: usize, n: usize| {
        let mut out = Vec::new();
        for b in from..from + n {
            let mut l: Vec<f32> = (b * 480..b * 480 + 480)
                .map(|i| {
                    let x = i as f32 / SR;
                    0.2 * cantus_dsp::math::sin(x * 50.0 * std::f32::consts::TAU)
                        + 0.3
                            * cantus_dsp::math::sin(x * 220.0 * std::f32::consts::TAU)
                            * if (i / 9_000) % 2 == 0 { 1.0 } else { 0.0 }
                        + if i % 7_919 == 0 { 0.6 } else { 0.0 }
                })
                .collect();
            let mut r = l.clone();
            u.process(&mut l, &mut r);
            out.extend_from_slice(&l);
        }
        out
    };
    run(&mut a, 0, 300);
    let mut b = a.fork().expect("a fork");
    assert_eq!(run(&mut a, 300, 200), run(&mut b, 300, 200));
}

/// The de-click's sensitivity stops at 0.7 (past it the repair harms: the DSP report's
/// finding), even from a hand-edited file; the Direct sound's default is one constant,
/// the DSP's own.
#[test]
fn the_sensitivity_is_capped_and_the_direct_sound_is_one_constant() {
    use cantus_core::fx::purus::{DIRECT_DB_DEFAULT, SENSITIVITY_MAX};
    let mut s = EffectSpec::new(EffectKind::Repair);
    assert_eq!(s.set("csens", 1.0), Some(SENSITIVITY_MAX));
    s.params.insert("csens".into(), 1.0);
    assert_eq!(settings(&s).click.sensitivity, SENSITIVITY_MAX);
    let d = settings(&EffectSpec::new(EffectKind::Repair));
    assert_eq!(d.verb.direct_db, DIRECT_DB_DEFAULT);
    assert_eq!(
        cantus_dsp::purus::VerbSettings::default().direct_db,
        DIRECT_DB_DEFAULT,
        "the DSP's default and the table's are the same"
    );
    assert_eq!(DIRECT_DB_DEFAULT, 6.0);
}

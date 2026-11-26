//! Chorda's model 2, round 42's voicing pass (`chorda-c17`, ADR 0098's sixth amendment): the
//! action's knock as the case rings it, and the colour a user reaches. Each part is nothing at
//! its default (the shipped voicing's pin, `tests_voicing3`, holds it to the bit) and does
//! what it says away from it; with every part on, the same bits at every block size and
//! thread count, an idle unit plays as a new one, and it comes to an exact rest.

use super::noise::Thud;
use super::tests_engine::{perform, Ev};
use super::tests_model2::piano2;
use super::voice::KNOCK_AS_IS;
use super::voicing::Voicing;
use super::*;
use crate::math;

fn voiced(v: &Voicing) -> Chorda {
    let mut c = piano2();
    c.set_voicing(v);
    c
}

/// Every round-42 knob away from its default, over the shipped voicing.
fn pass() -> Voicing {
    let mut v = Voicing::unison();
    v.knock_shape = [2_000.0, 0.007, 250.0, 3_500.0, 0.25, 0.09, 2.5, -0.4];
    v.knock_noise_reg = [0.6, 2.0];
    v.knock_treble = 0.05;
    v
}

/// Strikes over the keyboard, a key struck again while its knock rings, the top keys.
fn phrase() -> Vec<(usize, Ev)> {
    let mut ev = vec![
        (0, Ev::On(96, 0.7, 1)),
        (300, Ev::On(60, 0.5, 2)),
        (2_400, Ev::Off(96, 1)),
        (2_400, Ev::On(96, 0.9, 3)),
        (5_000, Ev::On(33, 0.8, 4)),
        (9_000, Ev::Off(60, 2)),
        (12_000, Ev::On(108, 0.6, 5)),
        (14_000, Ev::Off(108, 5)),
        (15_000, Ev::Off(33, 4)),
    ];
    ev.sort_by_key(|e| e.0);
    ev
}

fn band_energy(x: &[f32], lo: f64, hi: f64) -> f64 {
    // A crude band-pass: the difference of two one-pole low-passes, run twice.
    let k = |f: f64| math::exp(-std::f64::consts::TAU * f / 48_000.0);
    let (a, b) = (k(hi), k(lo));
    let mut s = [0.0f64; 4];
    let mut e = 0.0;
    for v in x {
        let v = *v as f64;
        s[0] = v + a * (s[0] - v);
        s[1] = v + b * (s[1] - v);
        let y = s[0] - s[1];
        s[2] = y + a * (s[2] - y);
        s[3] = y + b * (s[3] - y);
        let z = s[2] - s[3];
        e += z * z;
    }
    e
}

#[test]
fn the_knock_as_is_is_the_round_40_thud() {
    // The shape's neutral value is the model's own thud, so the unison pass (which
    // carries it) renders as it did; a shape set and set back plays as one never set.
    assert_eq!(Voicing::unison().knock_shape, KNOCK_AS_IS);
    assert_eq!(Voicing::unison().knock_noise_reg, [1.0, 1.0]);
    let ev = phrase();
    let plain = perform(&mut voiced(&Voicing::unison()), &ev, 16_000, 256, 1);
    let mut c = voiced(&pass());
    perform(
        &mut c,
        &[(0, Ev::On(96, 0.8, 9)), (200, Ev::Off(96, 9))],
        2_048,
        256,
        1,
    );
    c.set_voicing(&Voicing::unison());
    let mut idle: usize = 0;
    while !c.at_rest() {
        perform(&mut c, &[], 4_800, 512, 1);
        idle += 4_800;
        assert!(idle < 48_000 * 60, "no rest within a minute");
    }
    let to = (idle + 2_048).div_ceil(144_000) * 144_000;
    perform(&mut c, &[], to - idle - 2_048, 512, 1);
    assert!(perform(&mut c, &ev, 16_000, 256, 1) == plain);
}

#[test]
fn the_knocks_noise_sits_in_its_band_and_its_tail_rings_on() {
    // A top key's knock with its noise raised: more of the 0.5–2.8 kHz band under the
    // fundamental; with the tail, more 80–250 ms later, the first 20 ms nearly the same.
    let strike = |shape: [f64; 8]| {
        let mut v = Voicing::unison();
        v.knock_shape = shape;
        let ev = vec![(0, Ev::On(108, 0.7, 1))];
        perform(&mut voiced(&v), &ev, 14_400, 256, 1).0
    };
    let base = strike(KNOCK_AS_IS);
    let noisy = strike([3_000.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0]);
    let tail = strike([3_000.0, 0.0, 0.0, 0.0, 0.3, 0.1, 1.0, 0.0]);
    // What the shape adds: the renders less the round-40 thud's (the strings are the same).
    let e = |x: &[f32], a: usize, b: usize| {
        x[a..b]
            .iter()
            .zip(&base[a..b])
            .map(|(p, q)| {
                let d = (*p - *q) as f64;
                d * d
            })
            .sum::<f64>()
    };
    let (early, late) = ((0, 960), (4_800, 12_000));
    assert!(e(&noisy, early.0, early.1) > 0.0, "the noise added nothing");
    assert!(
        e(&tail, late.0, late.1) > 4.0 * e(&noisy, late.0, late.1),
        "the tail did not ring on: {:.3e} against {:.3e}",
        e(&tail, late.0, late.1),
        e(&noisy, late.0, late.1)
    );
    // The noise in its band: under 3 kHz more of it than over 6 kHz (the steep slopes).
    let d: Vec<f32> = noisy.iter().zip(&base).map(|(p, q)| p - q).collect();
    assert!(
        band_energy(&d[..4_800], 300.0, 3_000.0)
            > 10.0 * band_energy(&d[..4_800], 6_000.0, 20_000.0)
    );
}

#[test]
fn a_thud_reshaped_keeps_its_ringing_and_its_tail_falls() {
    let fs = 48_000.0;
    let mut a = Thud::new(fs, [72.0, 185.0], [0.06, 0.04], 300.0, 3_000.0, 0.004);
    let mut b = a.clone();
    a.strike([0.02, 0.012], 0.0, 7);
    b.strike([0.02, 0.012], 0.0, 7);
    for _ in 0..100 {
        assert_eq!(a.tick(), b.tick());
    }
    // The same shape again: nothing moves.
    b.reshape(fs, [72.0, 185.0], [0.06, 0.04], 300.0, 3_000.0, 0.004);
    for _ in 0..100 {
        assert_eq!(a.tick(), b.tick());
    }
    // A tail rings after the first stage has gone, then rests.
    let mut t = Thud::new(fs, [72.0, 185.0], [0.06, 0.04], 300.0, 3_000.0, 0.004);
    t.set_tail(fs, 0.3, 0.05);
    t.strike([0.0, 0.0], 1.0, 3);
    let x: Vec<f64> = (0..48_000).map(|_| t.tick()).collect();
    let e = |a: usize, b: usize| x[a..b].iter().map(|v| v * v).sum::<f64>();
    assert!(e(2_400, 4_800) > 0.0 && e(9_600, 12_000) < e(2_400, 4_800));
    assert!(!t.busy(), "the tail never rested");
}

#[test]
fn the_knocks_register_laws_move_their_registers_alone() {
    let mut v = Voicing::unison();
    v.knock_shape[0] = 3.0;
    assert_eq!(v.knock_shape_of(60)[0], 3.0);
    v.knock_noise_reg = [0.5, 4.0];
    assert_eq!(v.knock_shape_of(60)[0], 3.0);
    assert!((v.knock_shape_of(108)[0] - 12.0).abs() < 1e-9);
    assert!((v.knock_shape_of(21)[0] - 1.5).abs() < 1e-9);
    assert!(v.knock_shape_of(84)[0] > 3.0 && v.knock_shape_of(84)[0] < 12.0);
}

#[test]
fn the_pass_gives_the_same_bits_at_every_block_size_and_on_threads() {
    let v = pass();
    let ev = phrase();
    let frames = 16_000;
    let reference = perform(&mut voiced(&v), &ev, frames, 1, 1);
    for (block, threads) in [(7, 1), (37, 1), (64, 1), (100, 4), (300, 1), (512, 3)] {
        let got = perform(&mut voiced(&v), &ev, frames, block, threads);
        assert!(got == reference, "block {block}, {threads} threads");
    }
}

#[test]
fn an_idle_unit_with_the_pass_plays_as_a_new_one_and_comes_to_rest() {
    let v = pass();
    let ev = phrase();
    let fresh = perform(&mut voiced(&v), &ev, 16_000, 256, 1);
    let mut idle = voiced(&v);
    let short = vec![(0, Ev::On(100, 1.0, 1)), (100, Ev::Off(100, 1))];
    perform(&mut idle, &short, 512, 512, 1);
    let mut frames: usize = 0;
    while !idle.at_rest() {
        perform(&mut idle, &[], 4_800, 512, 1);
        frames += 4_800;
        assert!(frames < 48_000 * 60, "no rest within a minute");
    }
    let to = (frames + 512).div_ceil(144_000) * 144_000;
    perform(&mut idle, &[], to - frames - 512, 512, 1);
    let after = perform(&mut idle, &ev, 16_000, 256, 1);
    assert!(fresh == after, "an idle unit's attacks moved");
}

fn coloured(c: f64) -> Chorda {
    let mut p = voiced(&Voicing::default());
    let s = ChordaSettings {
        model: 2,
        colour: c,
        ..ChordaSettings::default()
    };
    p.set(&s, 0.0);
    p
}

#[test]
fn the_colour_at_zero_is_nothing_and_its_stage_follows_its_shape() {
    use super::colour::{Colour, CENTRES, SHAPE};
    let ev = phrase();
    let plain = perform(&mut voiced(&Voicing::default()), &ev, 12_000, 256, 1);
    assert!(perform(&mut coloured(0.0), &ev, 12_000, 256, 1) == plain);
    let mut st = Colour::new(48_000.0);
    assert!(!st.on());
    st.set(1.0);
    assert!(st.on());
    for (i, f) in CENTRES.iter().enumerate() {
        let g = st.gain_db(*f);
        assert!(
            (g - SHAPE[i]).abs() < 1.5,
            "{f} Hz: {g:.2} dB, the shape {:.2}",
            SHAPE[i]
        );
    }
    st.set(-1.0);
    for (i, f) in CENTRES.iter().enumerate() {
        assert!((st.gain_db(*f) + SHAPE[i]).abs() < 1.5);
    }
    if SHAPE.iter().any(|g| *g != 0.0) {
        assert!(perform(&mut coloured(1.0), &ev, 12_000, 256, 1) != plain);
    }
}

#[test]
fn a_coloured_unit_keeps_every_block_size_and_comes_to_rest() {
    let ev = phrase();
    let reference = perform(&mut coloured(0.8), &ev, 12_000, 1, 1);
    for (block, threads) in [(7, 1), (64, 1), (300, 3), (512, 1)] {
        assert!(
            perform(&mut coloured(0.8), &ev, 12_000, block, threads) == reference,
            "block {block}, {threads} threads"
        );
    }
    let mut c = coloured(-0.6);
    perform(
        &mut c,
        &[(0, Ev::On(60, 0.9, 1)), (2_000, Ev::Off(60, 1))],
        4_096,
        512,
        1,
    );
    let mut frames: usize = 0;
    while !c.at_rest() {
        perform(&mut c, &[], 4_800, 512, 1);
        frames += 4_800;
        assert!(frames < 48_000 * 60, "a coloured unit never rested");
    }
}

//! Libra's tests (ADR 0133, provisional).

use super::*;

const SR: f32 = 48_000.0;

/// ADR 0012's balance, as `cantus_core` computes it (a copy for the DSP's own tests; the
/// engine passes the real one and its tests hold it).
fn balance(p: f32) -> (f32, f32) {
    let p = p.clamp(-1.0, 1.0);
    if p == 0.0 {
        return (1.0, 1.0);
    }
    let a = (p + 1.0) * 0.25 * std::f32::consts::PI;
    let (pl, pr) = (crate::math::cos(a), crate::math::sin(a));
    let k = std::f32::consts::SQRT_2;
    if p < 0.0 {
        (1.0, (pr * k).min(1.0))
    } else {
        ((pl * k).min(1.0), 1.0)
    }
}

fn signal(n: usize) -> (Vec<f32>, Vec<f32>) {
    (0..n)
        .map(|i| {
            let t = i as f32 / SR;
            (
                0.4 * crate::math::sin(std::f32::consts::TAU * 220.0 * t) + 0.05,
                0.3 * crate::math::sin(std::f32::consts::TAU * 331.0 * t + 0.7),
            )
        })
        .unzip()
}

fn run(s: &LibraSettings, x: &(Vec<f32>, Vec<f32>), block: usize) -> (Vec<f32>, Vec<f32>) {
    let mut u = Libra::new(SR, balance);
    u.set(s);
    u.reset();
    let (mut l, mut r) = x.clone();
    for (a, b) in l.chunks_mut(block).zip(r.chunks_mut(block)) {
        u.process(a, b);
    }
    (l, r)
}

fn bits(a: &[f32], b: &[f32]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits())
}

#[test]
fn the_defaults_are_a_wire_and_zero_db_is_exact() {
    let x = signal(9_000);
    let (l, r) = run(&LibraSettings::default(), &x, 64);
    assert!(bits(&l, &x.0) && bits(&r, &x.1));
    let mut u = Libra::new(SR, balance);
    assert!(u.at_rest(), "a new unit rests");
    let (mut a, mut b) = (vec![0.0f32; 512], vec![0.0f32; 512]);
    u.process(&mut a, &mut b);
    assert!(u.at_rest());
}

#[test]
fn the_delay_is_exact_to_the_sample() {
    let x = signal(4_800);
    let s = LibraSettings {
        delay_l_ms: 1.0,
        delay_r_ms: 0.25,
        ..LibraSettings::default()
    };
    let (l, r) = run(&s, &x, 37);
    for i in 0..4_800 {
        let want_l = if i >= 48 { x.0[i - 48] } else { 0.0 };
        let want_r = if i >= 12 { x.1[i - 12] } else { 0.0 };
        assert_eq!(
            (l[i].to_bits(), r[i].to_bits()),
            (want_l.to_bits(), want_r.to_bits()),
            "{i}"
        );
    }
}

#[test]
fn the_channel_operations_and_the_polarity_do_what_they_say() {
    let x = signal(2_000);
    let at = |chan: Chan| {
        run(
            &LibraSettings {
                chan,
                ..LibraSettings::default()
            },
            &x,
            64,
        )
    };
    let (l, r) = at(Chan::Swap);
    assert!(bits(&l, &x.1) && bits(&r, &x.0));
    let (l, r) = at(Chan::LeftBoth);
    assert!(bits(&l, &x.0) && bits(&r, &x.0));
    let (l, r) = at(Chan::Side);
    assert!(l
        .iter()
        .zip(&x.0)
        .zip(&x.1)
        .all(|((y, a), b)| *y == (a - b) * 0.5));
    assert!(bits(&l, &r));
    let (l, r) = run(
        &LibraSettings {
            invert_r: true,
            ..LibraSettings::default()
        },
        &x,
        64,
    );
    assert!(bits(&l, &x.0) && r.iter().zip(&x.1).all(|(y, v)| *y == -*v));
}

#[test]
fn width_zero_is_mono_and_the_balance_never_raises_a_side() {
    let x = signal(24_000);
    let (l, r) = run(
        &LibraSettings {
            width: 0.0,
            ..LibraSettings::default()
        },
        &x,
        512,
    );
    assert!(l[4_800..].iter().zip(&r[4_800..]).all(|(a, b)| a == b));
    let (l, r) = run(
        &LibraSettings {
            pan: 1.0,
            ..LibraSettings::default()
        },
        &x,
        512,
    );
    assert!(bits(&r[4_800..], &x.1[4_800..]), "the near side untouched");
    assert!(
        l[4_800..].iter().all(|v| v.abs() < 1e-6),
        "the far side closed"
    );
}

#[test]
fn the_dc_goes_and_the_render_does_not_depend_on_the_block_size() {
    let x = signal(96_000);
    let s = LibraSettings {
        gain_db: -7.5,
        pan: -0.3,
        width: 1.4,
        chan: Chan::Swap,
        invert_l: true,
        dc: true,
        delay_l_ms: 3.0,
        ..LibraSettings::default()
    };
    let (l, _) = run(&s, &x, 512);
    // The left came from the right input (no DC); the right from the left (+0.05 DC).
    let mean = |v: &[f32]| v.iter().sum::<f32>() / v.len() as f32;
    let (_, r) = run(&s, &x, 512);
    assert!(
        mean(&r[72_000..]).abs() < 1e-3,
        "the DC is gone: {}",
        mean(&r[72_000..])
    );
    for block in [1usize, 37, 64, 1000] {
        let (bl, br) = run(&s, &x, block);
        assert!(bits(&bl, &l) && bits(&br, &r), "block {block}");
    }
}

#[test]
fn it_rests_after_a_silence_and_mute_glides() {
    let x = signal(4_800);
    let mut u = Libra::new(SR, balance);
    u.set(&LibraSettings {
        dc: true,
        delay_r_ms: 50.0,
        mute: true,
        ..LibraSettings::default()
    });
    let (mut l, mut r) = x.clone();
    u.process(&mut l, &mut r);
    assert!(l[3_000..].iter().all(|v| v.abs() < 1e-4), "muted");
    assert!(l[1].abs() > 0.0, "a glide, not a click");
    for _ in 0..200 {
        let (mut a, mut b) = (vec![0.0f32; 512], vec![0.0f32; 512]);
        u.process(&mut a, &mut b);
    }
    assert_eq!(u.rest_blocker(), None);
}

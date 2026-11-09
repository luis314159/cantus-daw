//! Model 2's voicing by register (ADR 0098, third amendment): every key's shares sum to one
//! and change smoothly, each band boosts or cuts by its gain at its centre and leaves the far
//! bands alone, a flat voicing sums the keys to the bit, and a voiced one still keeps the
//! family's rules (any block size, any thread count, an exact rest).

use super::registers::{Registers, BANDS, CENTRES};
use super::tests_engine::{perform, Ev};
use super::tests_model2::piano2;
use super::voicing::Voicing;
use super::*;
use crate::math;

const FS: f64 = 48_000.0;

#[test]
fn every_keys_shares_sum_to_one_and_glide_from_key_to_key() {
    let mut last = Registers::weights(21);
    for m in 21..=108u8 {
        let w = Registers::weights(m);
        assert!(
            (w.iter().sum::<f64>() - 1.0).abs() < 1e-12,
            "key {m}: {w:?}"
        );
        assert!(
            w.iter().filter(|x| **x > 0.0).count() <= 2,
            "key {m}: {w:?}"
        );
        for (a, b) in w.iter().zip(&last) {
            assert!(
                (a - b).abs() <= 1.0 / 30.0 + 1e-12,
                "key {m}: a step {last:?} → {w:?}"
            );
        }
        last = w;
    }
    assert_eq!(Registers::weights(21), [1.0, 0.0, 0.0]);
    assert_eq!(Registers::weights(65), [0.0, 1.0, 0.0]);
    assert_eq!(Registers::weights(108), [0.0, 0.0, 1.0]);
}

/// The level (dB) of a steady sine at `f` through register 0's bands, after its transient.
fn gain_at(gains: [f64; BANDS], f: f64) -> f64 {
    let mut r = Registers::new(FS, &[gains, [0.0; BANDS], [0.0; BANDS]]);
    let n = 48_000;
    let mut l: Vec<f64> = (0..n)
        .map(|i| math::sin(std::f64::consts::TAU * f * i as f64 / FS))
        .collect();
    let mut rt = l.clone();
    let x = l.clone();
    r.filter(0, &mut l, &mut rt);
    assert_eq!(l, rt, "both channels alike");
    let tail = n / 2..n;
    let e = |v: &[f64]| v[tail.clone()].iter().map(|s| s * s).sum::<f64>();
    10.0 * math::log10(e(&l) / e(&x))
}

#[test]
fn each_band_moves_its_centre_by_its_gain_and_leaves_the_far_bands_alone() {
    for b in 0..BANDS {
        let mut g = [0.0; BANDS];
        g[b] = 6.0;
        let at = gain_at(g, CENTRES[b]);
        assert!(
            (at - 6.0).abs() < 0.05,
            "band {b}: {at:.3} dB at its centre"
        );
        // Three octaves away the band does next to nothing.
        let far = if b < 2 {
            CENTRES[b] * 8.0
        } else {
            CENTRES[b] / 8.0
        };
        let off = gain_at(g, far);
        assert!(off.abs() < 0.5, "band {b}: {off:.3} dB at {far} Hz");
        g[b] = -6.0;
        let cut = gain_at(g, CENTRES[b]);
        assert!((cut + 6.0).abs() < 0.05, "band {b}: cut {cut:.3} dB");
    }
}

fn voiced(reg: [[f64; BANDS]; 3]) -> Chorda {
    let mut c = piano2();
    let v = Voicing {
        reg,
        ..Voicing::default()
    };
    c.set_voicing(&v);
    c
}

fn chords() -> Vec<(usize, Ev)> {
    vec![
        (0, Ev::On(30, 0.9, 1)),
        (10, Ev::On(60, 0.7, 2)),
        (20, Ev::On(96, 0.8, 3)),
        (4_000, Ev::Pedal(0.7)),
        (9_000, Ev::Off(30, 1)),
        (9_001, Ev::Off(60, 2)),
        (12_000, Ev::Pedal(0.0)),
        (12_001, Ev::Off(96, 3)),
    ]
}

#[test]
fn a_flat_voicing_sums_the_keys_to_the_bit() {
    let flat = [[0.0; BANDS]; 3];
    let mut a = piano2();
    let base = Voicing {
        reg: flat,
        ..Voicing::default()
    };
    a.set_voicing(&base);
    let mut b = voiced(flat);
    let (la, ra) = perform(&mut a, &chords(), 30_000, 256, 1);
    let (lb, rb) = perform(&mut b, &chords(), 30_000, 256, 1);
    assert_eq!(la, lb);
    assert_eq!(ra, rb);
    // And a voiced one is not flat.
    let mut g = flat;
    g[1][2] = -6.0;
    let (lc, _) = perform(&mut voiced(g), &chords(), 30_000, 256, 1);
    assert_ne!(la, lc);
}

#[test]
fn a_voiced_piano_gives_the_same_bits_at_every_block_size_and_on_threads() {
    let g = [
        [2.0, -3.0, 4.0, 0.0],
        [0.0, 0.0, -4.0, 2.0],
        [5.0, 3.0, 0.0, -2.0],
    ];
    let (l0, r0) = perform(&mut voiced(g), &chords(), 30_000, 1, 1);
    for (block, threads) in [(7, 1), (64, 1), (100, 4), (512, 3)] {
        let (l, r) = perform(&mut voiced(g), &chords(), 30_000, block, threads);
        assert_eq!(l0, l, "block {block}, {threads} threads");
        assert_eq!(r0, r, "block {block}, {threads} threads");
    }
}

#[test]
fn a_voiced_piano_comes_to_an_exact_rest() {
    let g = [
        [2.0, -3.0, 4.0, 0.0],
        [0.0, 0.0, -4.0, 2.0],
        [5.0, 3.0, 0.0, -2.0],
    ];
    let mut c = voiced(g);
    c.note_on(60, 1.0, 1);
    let mut l = vec![0.0f32; 512];
    let mut r = vec![0.0f32; 512];
    c.process(&mut l, &mut r);
    c.note_off(60, 1);
    let mut blocks = 0;
    while !c.at_rest() {
        l.fill(0.0);
        r.fill(0.0);
        c.process(&mut l, &mut r);
        blocks += 1;
        assert!(blocks < 48_000 * 60 / 512, "no rest within a minute");
    }
    l.fill(0.0);
    r.fill(0.0);
    assert!(!c.process(&mut l, &mut r));
    assert!(l.iter().chain(&r).all(|x| *x == 0.0));
}

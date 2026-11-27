//! Chorda's model 2, candidate 18 (round 42, `chorda-c18`, ADR 0098's seventh amendment): the
//! top keys' mechanics, dry. Each new part — the click's rise, the ring's own band, the treble's
//! shape of the knock — is nothing at its default (candidate 17's pin holds it to the bit),
//! does what it says away from it, and with every part on keeps every block size and thread
//! count and comes to rest (`tests_voicing4`'s `pass`, which sets them all).

use super::noise::Thud;
use super::voice::KNOCK_AS_IS;
use super::voicing::{Voicing, KNOCK_TOP_AS_IS, KNOCK_TOP_FROM};

fn energy(x: &[f64]) -> f64 {
    x.iter().map(|v| v * v).sum()
}

#[test]
fn the_new_knobs_at_their_defaults_are_nothing() {
    let v = Voicing::unison();
    assert_eq!(v.knock_treble_shape, KNOCK_TOP_AS_IS);
    assert_eq!((v.knock_bass, v.blow_bass), (1.0, 1.0));
    assert_eq!(&v.knock_shape[8..], &KNOCK_AS_IS[8..]);
    assert_eq!(&KNOCK_AS_IS[8..], &[0.0, 0.0]);
    let c17 = Voicing::c17();
    assert_eq!(c17.knock_treble_shape, KNOCK_TOP_AS_IS);
    assert_eq!((c17.knock_bass, c17.blow_bass), (1.0, 1.0));
    assert_eq!(&c17.knock_shape[8..], &[0.0, 0.0]);
    for m in 21..=108u8 {
        let mut w = c17;
        w.knock_treble_shape = KNOCK_TOP_AS_IS;
        assert_eq!(w.knock_shape_of(m), c17.knock_shape_of(m));
    }
}

#[test]
fn the_trebles_shape_moves_the_top_alone_and_reaches_its_value_at_c7() {
    let mut v = Voicing::c17();
    let base = v;
    v.knock_treble_shape = [4.0, 2.0, 0.25, 0.5, 0.3, 900.0];
    for m in 21..=KNOCK_TOP_FROM {
        assert_eq!(v.knock_shape_of(m), base.knock_shape_of(m), "key {m}");
    }
    for k in [1, 3, 4, 5, 8, 9] {
        assert_eq!(
            v.knock_shape_of(96)[k],
            v.knock_shape_of(108)[k],
            "part {k}"
        );
    }
    let (a, b) = (v.knock_shape_of(108), base.knock_shape_of(108));
    for (k, g) in [(1, 4.0), (3, 2.0), (4, 0.25), (5, 0.5)] {
        assert!(
            (a[k] - b[k] * g).abs() < 1e-9 * b[k].abs().max(1.0),
            "part {k}"
        );
    }
    assert!((a[8] - 0.3).abs() < 1e-12 && (a[9] - 900.0).abs() < 1e-6);
    for k in [0, 2, 6, 7] {
        assert_eq!(a[k], b[k], "part {k} moved");
    }
    let (m, n) = (v.knock_shape_of(90), base.knock_shape_of(90));
    assert!(m[1] > n[1] && m[1] < 4.0 * n[1]);
    assert!(m[4] < n[4] && m[4] > 0.25 * n[4]);
    assert!(m[8] > 0.0 && m[8] < 0.3 && m[9] > 900.0 && m[9] < 20_000.0);
}

#[test]
fn the_bass_knock_and_blow_scale_the_bass_alone() {
    // Through a unit: a bass key's strike with its knock and blow at a tenth is another
    // render; a middle and a top key's are the same bits.
    use super::tests_engine::{perform, Ev};
    use super::tests_model2::piano2;
    let play = |v: &Voicing, key: u8| {
        let mut c = piano2();
        c.set_voicing(v);
        perform(&mut c, &[(0, Ev::On(key, 0.8, 1))], 9_600, 256, 1)
    };
    let base = Voicing::c17();
    let mut low = base;
    low.knock_bass = 0.1;
    low.blow_bass = 0.1;
    assert!(play(&low, 24) != play(&base, 24), "the bass did not move");
    for key in [60, 72, 96] {
        assert!(play(&low, key) == play(&base, key), "key {key} moved");
    }
    assert_eq!(Voicing::bass_scale(60, 0.1), 1.0);
    assert!((Voicing::bass_scale(21, 0.1) - 0.1).abs() < 1e-12);
}

#[test]
fn the_clicks_rise_takes_its_first_milliseconds_and_keeps_its_peak() {
    let fs = 48_000.0;
    let run = |rise: f64| {
        let mut t = Thud::new(fs, [72.0, 185.0], [0.06, 0.04], 300.0, 4_000.0, 0.015);
        t.set_steep(true);
        t.set_rise(fs, 0.015, rise);
        t.strike([0.0, 0.0], 1.0, 5);
        let x: Vec<f64> = (0..48_000).map(|_| t.tick()).collect();
        assert!(!t.busy(), "the click never rested");
        x
    };
    let (at_once, rising) = (run(0.0), run(0.2));
    // Its first millisecond quieter, its peak about where the fall alone starts.
    assert!(energy(&rising[..48]) < 0.2 * energy(&at_once[..48]));
    let e = |x: &[f64]| energy(&x[240..1_440]);
    assert!(e(&rising) > 0.3 * e(&at_once) && e(&rising) < 3.0 * e(&at_once));
}

#[test]
fn the_rings_own_band_takes_its_highs_and_it_rests() {
    let fs = 48_000.0;
    let run = |hz: f64| {
        let mut t = Thud::new(fs, [72.0, 185.0], [0.06, 0.04], 90.0, 4_000.0, 0.002);
        t.set_steep(true);
        t.set_tail(fs, 0.5, 0.1);
        t.set_tail_band(fs, hz);
        t.strike([0.0, 0.0], 1.0, 11);
        let x: Vec<f64> = (0..192_000).map(|_| t.tick()).collect();
        assert!(!t.busy(), "the ring never rested");
        x
    };
    let (open, low) = (run(0.0), run(600.0));
    // After the click (20 ms on), the highs over 2.5 kHz: a one-pole difference, crude.
    let highs = |x: &[f64]| {
        let a = crate::math::exp(-std::f64::consts::TAU * 2_500.0 / fs);
        let (mut lp, mut e) = (0.0, 0.0);
        for v in &x[960..9_600] {
            lp = v + a * (lp - v);
            e += (v - lp) * (v - lp);
        }
        e
    };
    assert!(highs(&low) < 0.05 * highs(&open));
    assert!(energy(&low[960..9_600]) > 0.0);
    // Set back to none: the same as never set.
    let mut a = Thud::new(fs, [72.0, 185.0], [0.06, 0.04], 90.0, 4_000.0, 0.002);
    a.set_tail(fs, 0.5, 0.1);
    let mut b = a.clone();
    b.set_tail_band(fs, 600.0);
    b.set_tail_band(fs, 0.0);
    a.strike([0.01, 0.0], 1.0, 2);
    b.strike([0.01, 0.0], 1.0, 2);
    for _ in 0..20_000 {
        assert_eq!(a.tick(), b.tick());
    }
}

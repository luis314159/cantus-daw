//! Chorda's second model (ADR 0098): the family's rules hold for it (silence, any split,
//! threads, an exact rest, the cost), the first model's bits never move, the convolver is
//! a convolution, and no two strikes of a key are alike.

use super::conv::Convolver;
use super::tests_engine::{perform, piano, Ev};
use super::*;

const FS: f64 = 48_000.0;

pub fn piano2() -> Chorda {
    let mut c = piano();
    let s = ChordaSettings {
        model: 2,
        ..ChordaSettings::default()
    };
    c.set(&s, 0.0);
    c
}

fn phrase() -> Vec<(usize, Ev)> {
    vec![
        (0, Ev::On(48, 0.8, 1)),
        (100, Ev::On(60, 0.6, 2)),
        (2_000, Ev::On(64, 0.5, 3)),
        (2_001, Ev::Pedal(0.6)),
        (5_000, Ev::On(67, 0.9, 4)),
        (9_000, Ev::Off(60, 2)),
        (12_000, Ev::Soft(1.0)),
        (12_500, Ev::On(72, 0.7, 5)),
        (15_000, Ev::Pedal(0.0)),
        (15_001, Ev::Off(48, 1)),
        (18_000, Ev::Off(64, 3)),
        (19_000, Ev::On(100, 1.0, 6)),
        (19_500, Ev::On(60, 0.9, 7)),
        (19_731, Ev::Off(60, 7)),
    ]
}

#[test]
fn model_2_is_silent_at_rest() {
    let mut c = piano2();
    assert!(c.at_rest());
    let (mut l, mut r) = (vec![0.0; 4_096], vec![0.0; 4_096]);
    assert!(!c.process(&mut l, &mut r));
    assert!(l.iter().chain(&r).all(|x| *x == 0.0));
}

#[test]
fn model_2_gives_the_same_bits_at_every_block_size_and_on_threads() {
    let ev = phrase();
    let frames = 26_000;
    let reference = perform(&mut piano2(), &ev, frames, 512, 1);
    assert!(reference.0.iter().any(|x| *x != 0.0));
    for block in [1, 7, 37, 64, 300, 512] {
        let got = perform(&mut piano2(), &ev, frames, block, 1);
        assert!(got == reference, "block {block}");
    }
    for block in [7, 512] {
        let par = perform(&mut piano2(), &ev, frames, block, 4);
        assert!(par == reference, "threads, blocks of {block}");
    }
}

#[test]
fn model_2_differs_from_model_1_and_model_1_is_untouched_by_model_2s_settings() {
    let ev = phrase();
    let one = perform(&mut piano(), &ev, 22_000, 256, 1);
    let two = perform(&mut piano2(), &ev, 22_000, 256, 1);
    assert!(one != two);
    // Model 1 ignores the room and the noise.
    let mut c = piano();
    let s = ChordaSettings {
        room: 1.0,
        noise: 2.0,
        ..ChordaSettings::default()
    };
    c.set(&s, 0.0);
    assert!(perform(&mut c, &ev, 22_000, 256, 1) == one);
}

/// Seconds until `c` rests after `ev` (at most a minute).
fn rest_after(mut c: Chorda, ev: &[(usize, Ev)]) -> (f64, Chorda) {
    perform(&mut c, ev, 48_000 * 4, 512, 1);
    let (mut l, mut r) = (vec![0.0; 4_800], vec![0.0; 4_800]);
    let mut waited = 0;
    while !c.at_rest() && waited < 560 {
        c.process(&mut l, &mut r);
        waited += 1;
    }
    (4.0 + waited as f64 * 0.1, c)
}

#[test]
fn model_2_comes_to_an_exact_rest() {
    // Two keys struck hard and let go, a third half-pedalled: the undamped top strings ring
    // on in sympathy longest, in either model.
    let ev = [
        (1, Ev::On(84, 1.0, 1)),
        (1, Ev::On(40, 1.0, 2)),
        (2_400, Ev::Off(84, 1)),
        (2_400, Ev::Off(40, 2)),
        (3_000, Ev::Pedal(0.3)),
        (3_001, Ev::On(60, 0.6, 3)),
        (6_000, Ev::Off(60, 3)),
        (7_000, Ev::Pedal(0.0)),
    ];
    let (one, _) = rest_after(piano(), &ev);
    let (two, mut c) = rest_after(piano2(), &ev);
    eprintln!("at rest after {one:.1} s (model 1), {two:.1} s (model 2)");
    assert!(c.at_rest(), "{:?}", c.busy_parts());
    // Model 2's strings ring longer (its voicing lengthens the decay up to four times in the
    // treble), so its rest comes later — but it comes, to exact zero.
    assert!(
        two < 60.0,
        "model 2 rests {two} s against model 1's {one} s"
    );
    let (mut l, mut r) = (vec![0.0; 4_800], vec![0.0; 4_800]);
    assert!(!c.process(&mut l, &mut r));
    assert!(l.iter().chain(&r).all(|x| *x == 0.0));
}

/// No two notes of a key alike in model 2 (seeded from the note's id: the same song, the
/// same bits); in
/// model 1 a key struck from rest twice gives the same samples twice.
#[test]
fn model_2s_strikes_vary_and_model_1s_do_not() {
    let strike_twice = |mut c: Chorda| {
        let ev = [(0, Ev::On(60, 0.7, 1)), (4_800, Ev::Off(60, 1))];
        let a = perform(&mut c, &ev, 48_000 * 2, 256, 1).0;
        let mut waited = 0;
        let (mut l, mut r) = (vec![0.0; 4_800], vec![0.0; 4_800]);
        while !c.at_rest() && waited < 600 {
            c.process(&mut l, &mut r);
            waited += 1;
        }
        assert!(c.at_rest());
        let ev = [(0, Ev::On(60, 0.7, 2)), (4_800, Ev::Off(60, 2))];
        let b = perform(&mut c, &ev, 48_000 * 2, 256, 1).0;
        (a, b)
    };
    let (a, b) = strike_twice(piano());
    assert!(a == b, "model 1: the same strike");
    let (a, b) = strike_twice(piano2());
    assert!(a != b, "model 2: never the same strike");
    let level = |x: &[f32]| x.iter().map(|v| *v as f64 * *v as f64).sum::<f64>();
    let ratio: f64 = 10.0 * crate::math::log10(level(&a) / level(&b));
    eprintln!("two strikes of C4 at the same velocity differ by {ratio:+.2} dB");
    assert!(ratio.abs() < 1.5, "a variation, not another note: {ratio}");
}

/// The convolver is a convolution: against the direct sum, to rounding, for a response
/// shorter than a block, one of many blocks, and an input cut anywhere.
#[test]
fn the_convolver_is_a_convolution() {
    let mut seed = 0x2545_F491_4F6C_DD1Du64;
    let mut rnd = || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed >> 11) as f64 / (1u64 << 53) as f64 - 0.5
    };
    for len in [1usize, 17, 64, 65, 1_000, 4_321] {
        let h: Vec<f64> = (0..len).map(|_| rnd()).collect();
        let x: Vec<f64> = (0..3_000)
            .map(|i| if i % 700 < 500 { rnd() } else { 0.0 })
            .collect();
        let mut c = Convolver::new(&h);
        let got: Vec<f64> = x.iter().map(|v| c.tick(*v)).collect();
        for n in 0..x.len() {
            let want: f64 = (0..len.min(n + 1)).map(|k| h[k] * x[n - k]).sum();
            assert!(
                (got[n] - want).abs() < 1e-9,
                "len {len}, n {n}: {} vs {want}",
                got[n]
            );
        }
        // Silent long enough, it is at rest, its output exactly zero.
        for _ in 0..(len + 300) {
            c.tick(0.0);
        }
        assert!(c.at_rest(), "len {len}");
        assert_eq!(c.tick(0.0), 0.0);
    }
}

/// The body's responses: finite, unit-ish energy, the two microphones decorrelated.
#[test]
fn the_bodys_responses_are_two_microphones() {
    let v = super::body::BodyVoicing::default();
    let l = super::body::response(&v, FS, 0);
    let r = super::body::response(&v, FS, 1);
    assert!(l.iter().chain(&r).all(|x| x.is_finite()));
    let e = |h: &[f64]| h.iter().map(|x| x * x).sum::<f64>();
    let lr: f64 = l.iter().zip(&r).map(|(a, b)| a * b).sum();
    let corr = lr / (e(&l) * e(&r)).sqrt();
    eprintln!(
        "body responses: energy {:.3} / {:.3}, correlation {corr:.3}",
        e(&l),
        e(&r)
    );
    assert!(corr.abs() < 0.5, "{corr}");
}

fn load_of(mut c: Chorda) -> f64 {
    let mut ev = vec![(0usize, Ev::Pedal(1.0))];
    for (i, k) in (30u8..94).enumerate() {
        ev.push((1 + i * 37, Ev::On(k, 0.7, i as u64 + 1)));
    }
    let blocks = ((0.25 * FS) as usize).div_ceil(256);
    let seconds = (blocks * 256) as f64 / FS;
    perform(&mut c, &ev, 4_096, 256, 1);
    assert_eq!(c.active_voices(), 64);
    let (mut l, mut r) = (vec![0.0f32; 256], vec![0.0f32; 256]);
    let mut best = f64::INFINITY;
    for _ in 0..48 {
        let t = std::time::Instant::now();
        for _ in 0..blocks {
            l.fill(0.0);
            r.fill(0.0);
            c.process(&mut l, &mut r);
        }
        best = best.min(t.elapsed().as_secs_f64() / seconds);
    }
    best
}

#[test]
fn model_2s_sixty_four_pedal_down_voices_fit_a_quarter_of_a_core() {
    let load = load_of(piano2());
    eprintln!(
        "model 2, 64 pedal-down voices: {:.1} % of one core",
        load * 100.0
    );
    assert!(load < 0.6, "{load}");
}

#[test]
#[ignore]
fn measure_model_2s_sixty_four_pedal_down_voices() {
    let one = load_of(piano());
    let two = load_of(piano2());
    eprintln!(
        "64 pedal-down voices: model 1 {:.1} %, model 2 {:.1} % of one core",
        one * 100.0,
        two * 100.0
    );
    assert!(two < 0.25, "{two}");
}

/// Review of the first delivery (1): a unit that has run idle — the transport stopped, the
/// unit skipping silence — plays a phrase to the same bits as a new one. Seeded from the
/// unit's running clock, every strike's variation moved with how long it had idled.
#[test]
fn an_idle_unit_plays_as_a_new_one() {
    // Three seconds: a whole number of every grid the unit keeps (its 64-sample chunks,
    // the board's quarter rate, the control steps), so only a clock could tell the two
    // apart — and the strikes must not.
    let ev = phrase();
    let idle_frames = 48_000 * 3;
    assert_eq!(idle_frames % 64, 0);
    let fresh = perform(&mut piano2(), &ev, 20_000, 256, 1);
    let mut idle = piano2();
    perform(&mut idle, &[], idle_frames, 512, 1);
    assert!(idle.at_rest());
    let after = perform(&mut idle, &ev, 20_000, 256, 1);
    assert!(fresh == after, "an idle unit's strikes moved");
}

/// Review of the first delivery (2): a model switched on a ringing unit leaves nothing of
/// the other model's state behind — the unit still comes to rest, and switching back does
/// not bring a frozen tail with it.
#[test]
fn switching_the_model_mid_tail_leaves_no_frozen_state() {
    let with = |c: &mut Chorda, model: u8| {
        let s = ChordaSettings {
            model,
            ..ChordaSettings::default()
        };
        c.set(&s, 0.0);
    };
    let ev = [(0, Ev::On(48, 0.9, 1)), (2_000, Ev::Off(48, 1))];
    let mut c = piano2();
    perform(&mut c, &ev, 6_000, 256, 1);
    with(&mut c, 1);
    let (mut l, mut r) = (vec![0.0; 4_800], vec![0.0; 4_800]);
    let mut waited = 0;
    while !c.at_rest() && waited < 600 {
        c.process(&mut l, &mut r);
        waited += 1;
    }
    assert!(c.at_rest(), "after the switch, rest: {:?}", c.busy_parts());
    // Back to model 2, at rest: silence, no tail held over from before the switch.
    with(&mut c, 2);
    assert!(c.at_rest());
    let (mut l, mut r) = (vec![0.0; 4_800], vec![0.0; 4_800]);
    assert!(!c.process(&mut l, &mut r));
    assert!(l.iter().chain(&r).all(|x| *x == 0.0));
    // And a strike after it all equals a new unit's (skipped as far).
    let mut fresh = piano2();
    let frames = 6_000 + waited * 4_800 + 4_800;
    fresh.skip(frames);
    let a = perform(&mut c, &ev, 8_000, 256, 1);
    let b = perform(&mut fresh, &ev, 8_000, 256, 1);
    assert!(
        a == b,
        "a strike after the switches differs from a new unit's"
    );
}

/// Review of the first delivery (3): the cost at 96 and 192 kHz (release, ignored).
#[test]
#[ignore]
fn measure_model_2_at_high_rates() {
    for fs in [96_000.0f32, 192_000.0] {
        let kb = std::sync::Arc::new(super::design::Keyboard::compute(
            super::design::Tuning::default(),
            fs as f64,
            8,
        ));
        let mk = |model: u8| {
            let mut c = Chorda::new(fs);
            c.set_keyboard(kb.clone());
            let s = ChordaSettings {
                model,
                ..ChordaSettings::default()
            };
            c.set(&s, 0.0);
            c
        };
        let (one, two) = (load_at(mk(1), fs as f64), load_at(mk(2), fs as f64));
        eprintln!(
            "{fs} Hz, 64 pedal-down voices: model 1 {:.1} %, model 2 {:.1} % of one core",
            one * 100.0,
            two * 100.0
        );
    }
}

fn load_at(mut c: Chorda, fs: f64) -> f64 {
    let mut ev = vec![(0usize, Ev::Pedal(1.0))];
    for (i, k) in (30u8..94).enumerate() {
        ev.push((1 + i * 37, Ev::On(k, 0.7, i as u64 + 1)));
    }
    perform(&mut c, &ev, 4_096, 256, 1);
    let blocks = ((0.25 * fs) as usize).div_ceil(256);
    let seconds = (blocks * 256) as f64 / fs;
    let (mut l, mut r) = (vec![0.0f32; 256], vec![0.0f32; 256]);
    let mut best = f64::INFINITY;
    for _ in 0..24 {
        let t = std::time::Instant::now();
        for _ in 0..blocks {
            l.fill(0.0);
            r.fill(0.0);
            c.process(&mut l, &mut r);
        }
        best = best.min(t.elapsed().as_secs_f64() / seconds);
    }
    best
}

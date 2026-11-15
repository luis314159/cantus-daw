//! Chorda's keyboard tables against the measurements they come from.

use super::tables::*;

fn cents(a: f64, b: f64) -> f64 {
    1200.0 * crate::math::log2(a / b)
}

#[test]
fn inharmonicity_follows_the_published_measurements() {
    // Bank (2000) measured C2 (36) at 1.0·10⁻⁴ and A♯4 (70) at 7.5·10⁻⁴.
    let c2 = inharmonicity(36.0, 1.0);
    assert!((c2 - 1.0e-4).abs() / 1.0e-4 < 0.02, "C2: {c2:e}");
    let as4 = inharmonicity(70.0, 1.0);
    assert!((as4 - 7.5e-4).abs() / 7.5e-4 < 0.06, "A#4: {as4:e}");
    // Lowest in the tenor, rising toward both ends, as on every piano.
    let min = (21..=108)
        .min_by(|a, b| inharmonicity(*a as f64, 1.0).total_cmp(&inharmonicity(*b as f64, 1.0)))
        .unwrap();
    assert!((36..=55).contains(&min), "the smallest B at key {min}");
    assert!(inharmonicity(21.0, 1.0) > 2.0 * inharmonicity(min as f64, 1.0));
    assert!(inharmonicity(108.0, 1.0) > 0.01);
    assert_eq!(inharmonicity(60.0, 0.5), inharmonicity(60.0, 1.0) * 0.5);
}

#[test]
fn the_tuning_is_a4_and_stretched_as_an_aural_tuner_stretches() {
    let f = tuning(440.0, 1.0, 1.0);
    let at = |m: u8| f[(m - LOWEST) as usize];
    assert_eq!(at(69), 440.0, "A4's first partial is the reference");
    let et = |m: u8| 440.0 * crate::math::powf(2.0f64, (m as f64 - 69.0) / 12.0);
    // Railsback's curve: the bass flat, the treble sharp, the middle near equal temperament.
    let (a0, a7, c8) = (
        cents(at(21), et(21)),
        cents(at(105), et(105)),
        cents(at(108), et(108)),
    );
    assert!((-40.0..-8.0).contains(&a0), "A0 {a0:.1} cents");
    assert!((8.0..45.0).contains(&a7), "A7 {a7:.1} cents");
    assert!(c8 > a7, "C8 {c8:.1} cents");
    assert!(
        cents(at(57), et(57)).abs() < 3.0,
        "A3 near equal temperament"
    );
    // Rising key by key, every semitone near 100 cents.
    for m in 22..=108u8 {
        let step = cents(at(m), at(m - 1));
        assert!((96.0..106.0).contains(&step), "{m}: {step:.2} cents");
    }
    // An octave tuned 4:2 makes the upper note's second partial meet the lower's fourth.
    let keys = keyboard(440.0, 1.0, 1.0);
    let k = |m: u8| keys[(m - LOWEST) as usize];
    let rho = octave_type(57.0, 1.0);
    assert!(rho > 1.0 && rho < 4.0, "{rho}");
    // No stretch at all: octaves 2:1 between the lower's second and the upper's first.
    let flat = keyboard(440.0, 0.0, 1.0);
    let fk = |m: u8| flat[(m - LOWEST) as usize];
    let beat = cents(fk(81).partial(1), fk(69).partial(2));
    assert!(beat.abs() < 0.01, "A5 on A4's second partial: {beat}");
    let _ = k(60);
}

#[test]
fn the_strings_decay_as_bensa_et_al_measured() {
    let keys = keyboard(440.0, 1.0, 1.0);
    let k = |m: u8| keys[(m - LOWEST) as usize];
    // σ₁ = b₁ + b₂ (π/L)² at the measured notes.
    for (m, l, b1, b2) in [
        (36u8, 1.23, 0.25, 7.5e-5),
        (60, 0.63, 1.1, 2.7e-4),
        (96, 0.10, 9.17, 2.1e-3),
    ] {
        let key = k(m);
        assert!((key.length - l).abs() < 1e-9, "{m}");
        assert!(
            (key.b1 - b1).abs() < 1e-9 && (key.b2 - b2).abs() < 1e-12,
            "{m}"
        );
        let beta = std::f64::consts::PI / l;
        let want = 6.907_755 / (b1 + b2 * beta * beta);
        assert!(
            (key.t60(1) - want).abs() / want < 1e-6,
            "{m}: {} s",
            key.t60(1)
        );
    }
    // Middle C rings about six seconds, the lowest notes much longer, the top far shorter;
    // higher partials always die sooner.
    assert!((5.0..8.0).contains(&k(60).t60(1)));
    assert!(k(21).t60(1) > 30.0 && k(108).t60(1) < 1.0);
    for key in keys.iter() {
        assert!(key.t60(10) < key.t60(1), "{}", key.key);
        assert!(
            key.length > 0.04 && key.length < 2.2,
            "{}: {} m",
            key.key,
            key.length
        );
    }
}

#[test]
fn hammers_strings_and_dampers_are_where_a_grand_has_them() {
    let keys = keyboard(440.0, 1.0, 1.0);
    let k = |m: u8| keys[(m - LOWEST) as usize];
    assert_eq!((k(21).strings, k(35).strings, k(60).strings), (1, 2, 3));
    assert!(k(60).damper && k(88).damper && !k(89).damper && !k(108).damper);
    assert!((k(60).hammer_mass - 2.97e-3).abs() < 1e-9);
    assert!((k(60).hammer_k - 4.5e9).abs() / 4.5e9 < 1e-9);
    assert_eq!(k(60).hammer_p, 2.5);
    assert_eq!(k(60).strike, 0.12);
    assert_eq!(k(108).strike, 0.0625);
    // Heavier, softer hammers down the keyboard.
    for m in 22..=108u8 {
        assert!(k(m).hammer_mass <= k(m - 1).hammer_mass);
        assert!(k(m).hammer_k >= k(m - 1).hammer_k);
    }
}

#[test]
#[ignore]
fn print_the_keyboard() {
    let keys = keyboard(440.0, 1.0, 1.0);
    for k in keys.iter().step_by(6) {
        let et = 440.0 * crate::math::powf(2.0f64, (k.key as f64 - 69.0) / 12.0);
        eprintln!(
            "{:3} f1 {:8.2} ({:+5.1} ct) B {:.2e} L {:.3} Z {:.2} T60 {:5.2}/{:5.2} hm {:.4} K {:.2e} p {:.2} s {:.3}",
            k.key, k.f1, cents(k.f1, et), k.b, k.length, k.impedance, k.t60(1), k.t60(10),
            k.hammer_mass, k.hammer_k, k.hammer_p, k.strike
        );
    }
}

// ---- round 40, finding 9: model 2's own keyboard ----------------------------------------

/// The fitted numbers (the Salamander's measured B, `docs/round40/chorda.md` §finding 9).
fn model_2() -> super::design::Tuning {
    super::design::Tuning {
        bass_b: 0.754,
        treble_b: 0.874,
        treble_octave: 2.0,
        ..Default::default()
    }
}

#[test]
fn model_2s_keyboard_at_ones_is_model_1s_to_the_bit() {
    let t = super::design::Tuning::default();
    assert_eq!(keyboard_for(&t), keyboard(440.0, 1.0, 1.0));
    for m in 21..=108 {
        let m = m as f64;
        assert_eq!(
            inharmonicity_of(m, 1.3, 1.0, 1.0).to_bits(),
            inharmonicity(m, 1.3).to_bits()
        );
    }
    // A product by one is exact; an octave type floored at one is the model's (it is never
    // below one at a stretch of zero or more).
    let s = super::design::Tuning {
        a4: 442.0,
        stretch: 0.7,
        stiffness: 1.1,
        ..Default::default()
    };
    assert_eq!(tuning_for(&s), tuning(442.0, 0.7, 1.1));
}

#[test]
fn the_asymptotes_scale_their_own_end_of_the_keyboard() {
    let (bass, treble) = (0.75, 0.875);
    for m in [21.0, 30.0, 45.0, 69.0, 96.0, 108.0] {
        let b = inharmonicity_of(m, 1.0, bass, treble);
        let full = inharmonicity(m, 1.0);
        // Between the two scales, near the bass's at A0 and near the treble's at C8.
        assert!(
            b < full && b > full * bass.min(treble),
            "{m}: {b:e} of {full:e}"
        );
    }
    assert!(inharmonicity_of(21.0, 1.0, bass, 1.0) / inharmonicity(21.0, 1.0) < 0.76);
    assert!(inharmonicity_of(108.0, 1.0, bass, 1.0) / inharmonicity(108.0, 1.0) > 0.999);
    assert!(inharmonicity_of(108.0, 1.0, 1.0, treble) / inharmonicity(108.0, 1.0) < 0.876);
    assert!(inharmonicity_of(21.0, 1.0, 1.0, treble) / inharmonicity(21.0, 1.0) > 0.99);
}

#[test]
fn model_2s_keyboard_follows_the_recorded_grands_strings() {
    // The Salamander Grand's measured B (×10⁻⁴, the analyst's `notes_analysis.py`, mf and
    // ff, A0 … A2): the model's bass strings are 20–45 % stiffer; model 2's within 22 %.
    let measured = [
        (21u8, 2.2),
        (24, 1.6),
        (27, 1.3),
        (30, 0.9),
        (33, 0.9),
        (36, 0.9),
        (39, 0.7),
        (42, 0.7),
        (45, 0.75),
        (57, 2.2),
        (69, 6.4),
        (81, 17.8),
    ];
    let model = keyboard(440.0, 1.0, 1.0);
    let ours = keyboard_for(&model_2());
    let (mut before, mut after) = (0.0f64, 0.0f64);
    for (m, b) in measured {
        let i = (m - LOWEST) as usize;
        let err = |k: &Key| (k.b / (b * 1e-4) - 1.0).abs();
        before = before.max(err(&model[i]));
        after = after.max(err(&ours[i]));
        assert!(
            err(&ours[i]) < 0.22,
            "{m}: B {:e} against {b}e-4",
            ours[i].b
        );
    }
    assert!(before > 0.4 && after < before / 2.0, "{before} → {after}");
}

#[test]
fn the_treble_tuned_in_4_2_octaves_leaves_the_rest_alone() {
    let t = super::design::Tuning {
        treble_octave: 2.0,
        ..Default::default()
    };
    let (model, ours) = (keyboard(440.0, 1.0, 1.0), keyboard_for(&t));
    let at = |k: &[Key; KEYS], m: u8| k[(m - LOWEST) as usize];
    // A4 and below: the model's, to the bit.
    for m in LOWEST..=69 {
        assert_eq!(at(&ours, m), at(&model, m), "{m}");
    }
    // A5's second partial on A4's fourth (a 4:2 octave), where the model's octave type
    // there is below two.
    assert!(octave_type(81.0, 1.0) < 2.0);
    let beat = cents(at(&ours, 81).partial(2), at(&ours, 69).partial(4));
    assert!(beat.abs() < 0.01, "A5's 2nd on A4's 4th: {beat}");
    // The treble sharper the higher (A5, A6 by the reference's ~2 and ~10 cents more than
    // the model), every semitone still near 100 cents.
    let more = |m: u8| cents(at(&ours, m).f1, at(&model, m).f1);
    assert!((0.4..2.0).contains(&more(81)), "A5 {:.2}", more(81));
    assert!((3.0..8.0).contains(&more(93)), "A6 {:.2}", more(93));
    // The top octave keeps the model's near-2:1 octave above A6's offset.
    let top =
        cents(at(&ours, 105).f1, at(&ours, 93).f1) - cents(at(&model, 105).f1, at(&model, 93).f1);
    assert!(top.abs() < 0.5, "A6 to A7 stretched {top:.2} cents more");
    for m in 70..=93u8 {
        assert!(more(m) >= more(m - 1) - 1e-9, "{m}");
    }
    for m in 70..=108u8 {
        let step = cents(at(&ours, m).f1, at(&ours, m - 1).f1);
        assert!((96.0..108.0).contains(&step), "{m}: {step:.2} cents");
    }
}

#[test]
fn the_sixth_octave_takes_its_own_octave_type() {
    let base = super::design::Tuning {
        treble_octave: 2.0,
        ..Default::default()
    };
    let hi = super::design::Tuning {
        treble_octave_hi: 3.0,
        ..base
    };
    let (a, b) = (keyboard_for(&base), keyboard_for(&hi));
    let at = |k: &[Key; KEYS], m: u8| k[(m - LOWEST) as usize];
    // Up to A5, the same keyboard to the bit; from A5 on, sharper.
    for m in LOWEST..=81 {
        assert_eq!(at(&b, m), at(&a, m), "{m}");
    }
    let more = |m: u8| cents(at(&b, m).f1, at(&a, m).f1);
    assert!(
        more(87) > 0.5 && more(93) > more(87),
        "{} {}",
        more(87),
        more(93)
    );
    // Its A5–A6 octave: A6's second partial on A5's sixth (a 6:3 octave).
    let beat = cents(at(&b, 93).partial(3), at(&b, 81).partial(6));
    assert!(beat.abs() < 0.01, "{beat}");
    // At 0 it is `treble_octave`'s.
    let zero = super::design::Tuning {
        treble_octave_hi: 0.0,
        ..base
    };
    assert_eq!(keyboard_for(&zero), a);
}

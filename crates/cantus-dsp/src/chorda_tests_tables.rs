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

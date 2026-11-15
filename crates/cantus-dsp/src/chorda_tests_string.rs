//! A Chorda key's strings, measured on what they render: the partials where the stiff
//! string has them, the decay Bensa et al. measured, the hammer's dynamics.

use super::design::*;
use super::string::*;
use super::tables::*;
use super::testkit::*;

/// The default felt: Chaigne & Askenfelt's hammer for the key, Stulov's hysteresis.
pub fn felt_of(k: &Key) -> Felt {
    Felt {
        k: k.hammer_k,
        p: k.hammer_p,
        eps: 0.9,
        tau: 1.0e-5,
    }
}

/// Strike key `m` (one string alone unless `strings` says otherwise) at `velocity` m/s and
/// render `seconds` of its bridge force.
pub fn render_key(m: u8, fs: f64, seconds: f64, velocity: f64, strings: usize) -> (Key, Vec<f64>) {
    render_tuned(m, fs, Tuning::default(), seconds, velocity, strings)
}

/// [`render_key`] at a tuning of the instrument's.
pub fn render_tuned(
    m: u8,
    fs: f64,
    tuning: Tuning,
    seconds: f64,
    velocity: f64,
    strings: usize,
) -> (Key, Vec<f64>) {
    let keys = keyboard(tuning.a4, tuning.stretch, tuning.stiffness);
    let k = keys[(m - LOWEST) as usize];
    let d = design(k.f0(), k.b, fs);
    let mut s = KeyStrings::new(d.delay + 16, fs);
    s.tune_strings(&k, &d, [0.0, 0.0, 0.0], strings);
    let (g, a, yz) = loss(&k, fs, 0.5, 1.0, 0.0, 0.0);
    s.set_loss(g, a, yz);
    s.strike(felt_of(&k), k.hammer_mass, velocity, [1.0; MAX_STRINGS]);
    let n = (seconds * fs) as usize;
    (k, (0..n).map(|_| s.tick()).collect())
}

fn cents(a: f64, b: f64) -> f64 {
    1200.0 * crate::math::log2(a / b)
}

/// Every checked partial that sounds (within 50 dB of the loudest) on key `m`, alone,
/// within `tol` cents of the stiff-string law. Returns the worst error.
fn check_partials(m: u8, fs: f64, tol: f64) -> f64 {
    check_tuned(m, fs, Tuning::default(), tol, 1 << 16)
}

/// [`check_partials`] at a tuning, with an analysis window of `n` samples.
fn check_tuned(m: u8, fs: f64, tuning: Tuning, tol: f64, n: usize) -> f64 {
    let seconds = (n + n / 8) as f64 / fs + 0.15;
    let (k, x) = render_tuned(m, fs, tuning, seconds.max(1.0), 2.0, 1);
    let checked = checked_partials(k.f0(), k.b, fs);
    let start = (0.05 * fs) as usize;
    let levels: Vec<f64> = (1..=checked)
        .map(|i| partial_level(&x, fs, k.partial(i), 0.05))
        .collect();
    let loudest = levels.iter().cloned().fold(f64::MIN, f64::max);
    let mut worst: f64 = 0.0;
    for i in 1..=checked {
        if levels[i as usize - 1] < loudest - 50.0 {
            continue;
        }
        let want = k.partial(i);
        let tol_hz = 0.25 * k.f0();
        let got = partial_hz(&x, fs, want, tol_hz, start, n);
        let c = cents(got, want);
        assert!(
            c.abs() < tol,
            "key {m} at {fs} Hz, {tuning:?}, partial {i}: {got:.4} Hz against {want:.4} \
             ({c:+.3} cents)"
        );
        worst = worst.max(c.abs());
    }
    worst
}

#[test]
fn a_string_rings_on_the_stiff_strings_partials_within_a_cent() {
    for m in [21u8, 33, 45, 60, 72, 84, 96, 108] {
        let w = check_partials(m, 48_000.0, 1.0);
        eprintln!("key {m}: worst {w:.3} cents");
    }
}

#[test]
fn a_string_decays_as_bensa_et_al_measured() {
    // A string alone, its partials' T60 within the perceptual tolerance (−25 % / +40 %,
    // Tolonen & Järveläinen) of σ = b₁ + b₂ (kπ/L)² at the notes they measured.
    for (m, seconds, parts) in [(36u8, 12.0, 6u32), (60, 8.0, 8), (96, 2.0, 2)] {
        let (k, x) = render_key(m, 48_000.0, seconds, 2.0, 1);
        for i in 1..=parts {
            let want = k.t60(i);
            let range = (60.0f64).min(20.0 * seconds / want * 0.7 * 3.0).max(10.0);
            let got = partial_t60(&x, 48_000.0, k.partial(i), 0.2, range);
            let ratio = got / want;
            eprintln!("key {m} partial {i}: T60 {got:.3} s against {want:.3} ({ratio:.2})");
            assert!(
                (0.75..=1.40).contains(&ratio),
                "key {m}, partial {i}: {ratio:.3}"
            );
        }
    }
}

#[test]
fn a_harder_strike_is_louder_and_brighter() {
    let (k, soft) = render_key(60, 48_000.0, 0.5, 0.5, 1);
    let (_, hard) = render_key(60, 48_000.0, 0.5, 4.0, 1);
    let level = |x: &[f64]| x.iter().map(|v| v * v).sum::<f64>().sqrt();
    assert!(level(&hard) > 4.0 * level(&soft));
    // Brighter: the 8th partial against the 1st gains with the velocity (Chaigne &
    // Askenfelt: the felt stiffens as it is compressed harder).
    let ratio = |x: &[f64]| {
        partial_level(x, 48_000.0, k.partial(8), 0.02)
            - partial_level(x, 48_000.0, k.partial(1), 0.02)
    };
    assert!(
        ratio(&hard) > ratio(&soft) + 6.0,
        "{} vs {}",
        ratio(&hard),
        ratio(&soft)
    );
}

#[test]
fn the_strings_are_the_same_bits_every_time_and_silence_stays_silent() {
    let (_, a) = render_key(48, 48_000.0, 0.2, 1.5, 3);
    let (_, b) = render_key(48, 48_000.0, 0.2, 1.5, 3);
    assert_eq!(a, b);
    let keys = keyboard(440.0, 1.0, 1.0);
    let k = keys[27];
    let d = design(k.f0(), k.b, 48_000.0);
    let mut s = KeyStrings::new(d.delay + 16, 48_000.0);
    s.tune(&k, &d, [0.0; 3]);
    let (g, al, yz) = loss(&k, 48_000.0, 0.5, 1.0, 0.0, 0.0);
    s.set_loss(g, al, yz);
    assert!(
        (0..48_000).all(|_| s.tick() == 0.0),
        "nothing struck, nothing heard"
    );
}

#[test]
fn every_key_rings_within_a_cent_at_every_rate_and_tuning() {
    // ADR 0059's done-when, measured on the sound: all 88 keys, one string each, every
    // partial below 5 kHz that sounds — at 44.1, 48 and 96 kHz, and at the Baroque preset's
    // tuning (A4 415 Hz, 70 % stretch) and the stiffest strings the settings allow — and
    // the ranges' corners (sound review 2): A4 466 with the widest stretch and the stiffest
    // strings (the shortest loops, the most dispersion against the rate) at 44.1 and
    // 96 kHz, and A4 415 with no stretch and the softest strings at 44.1 kHz.
    let baroque = Tuning {
        a4: 415.0,
        stretch: 0.7,
        stiffness: 1.0,
        ..Default::default()
    };
    let stiff = Tuning {
        stiffness: 1.5,
        ..Tuning::default()
    };
    let high = Tuning {
        a4: 466.0,
        stretch: 2.0,
        stiffness: 1.5,
        ..Default::default()
    };
    let low = Tuning {
        a4: 415.0,
        stretch: 0.0,
        stiffness: 0.5,
        ..Default::default()
    };
    // Round 40's model-2 keyboard (finding 9: the bass's asymptote fitted to the recorded
    // grand, the treble tuned in 4:2 octaves).
    let model_2 = Tuning {
        bass_b: 0.754,
        treble_b: 0.874,
        treble_octave: 2.0,
        ..Tuning::default()
    };
    let runs = [
        (44_100.0, Tuning::default(), 1usize << 15),
        (48_000.0, model_2, 1 << 15),
        (48_000.0, Tuning::default(), 1 << 15),
        (96_000.0, Tuning::default(), 1 << 16),
        (48_000.0, baroque, 1 << 15),
        (48_000.0, stiff, 1 << 15),
        (44_100.0, high, 1 << 15),
        (96_000.0, high, 1 << 16),
        (44_100.0, low, 1 << 15),
    ];
    let threads = std::thread::available_parallelism()
        .map_or(4, |n| n.get())
        .min(8);
    for (fs, tuning, n) in runs {
        let worst = std::thread::scope(|sc| {
            let hs: Vec<_> = (0..threads)
                .map(|t| {
                    sc.spawn(move || {
                        (LOWEST + t as u8..=HIGHEST)
                            .step_by(threads)
                            .map(|m| check_tuned(m, fs, tuning, 1.0, n))
                            .fold(0.0f64, f64::max)
                    })
                })
                .collect();
            hs.into_iter()
                .map(|h| h.join().expect("a key's check"))
                .fold(0.0f64, f64::max)
        });
        eprintln!("{fs} Hz, {tuning:?}: worst {worst:.3} cents on 88 keys");
    }
}

/// Round 40 (candidate 7): the steeper loss is the law itself at 1, to the bit; steeper, the
/// second partial loses more, and the first keeps the law's loss wherever the filter's gain
/// may stay at or under 1 (C4, C7) — on the very top (A7) the passive filter takes a little
/// of the first partial too, never less than the law has it.
#[test]
fn the_steeper_loss_keeps_the_first_partial_and_takes_the_second() {
    let fs = 48_000.0;
    let keys = keyboard_for(&Tuning::default());
    for m in [60u8, 96, 105] {
        let k = keys[(m - LOWEST) as usize];
        assert_eq!(
            loss_steep(&k, fs, 0.5, 1.0, 0.0, 0.0, 1.0),
            loss(&k, fs, 0.5, 1.0, 0.0, 0.0)
        );
        // The filter's gain at a partial: g (1 − 2α(1 − cos ω)).
        let at = |(g, a, _): (f64, f64, f64), n: u32| {
            g * (1.0
                - 2.0 * a * (1.0 - crate::math::cos(std::f64::consts::TAU * k.partial(n) / fs)))
        };
        let (law, steep) = (
            loss(&k, fs, 0.5, 1.0, 0.0, 0.0),
            loss_steep(&k, fs, 0.5, 1.0, 0.0, 0.0, 2.0),
        );
        if m < 100 {
            assert!(
                (at(law, 1) - at(steep, 1)).abs() < 1e-12,
                "key {m}: the first partial moved"
            );
        } else {
            assert!(
                at(steep, 1) <= at(law, 1) + 1e-12,
                "key {m}: the first partial lost less"
            );
        }
        assert!(
            at(steep, 2) < at(law, 2),
            "key {m}: the second partial kept its loss"
        );
    }
}

/// Round 40 (candidate 8): a unison string's stiffness scatter, held at its first partial,
/// holds it on every key — the fraction found gives the loop back its phase there — and a
/// fraction is found for the scatter (or its mirror, or half of either) on almost every key.
/// Candidate 7's clamp left D3 and A#1 50 cents flat.
#[test]
fn a_held_stiffness_scatter_keeps_every_keys_first_partial() {
    let fs = 48_000.0;
    let keys = keyboard_for(&Tuning::default());
    let mut held = 0;
    let mut tried = 0;
    for k in keys.iter() {
        let d = design(k.f0(), k.b, fs);
        let w = std::f64::consts::TAU * k.partial(1) / fs;
        let phase = |frac: f64, e: f64| {
            let mut p = first_phase(w, frac);
            for (r, t) in d.poles[..d.sections].iter() {
                let t2 = (*t * (1.0 + e)).clamp(1e-6, std::f64::consts::PI - 1e-6);
                p += second_phase(w, *r, t2);
            }
            p
        };
        for e in [0.05, -0.05, 0.3, -0.3] {
            tried += 1;
            if let Some((k2, a)) = [e, -e, 0.5 * e, -0.5 * e]
                .iter()
                .find_map(|k2| held_frac(&d, d.frac, w, *k2).map(|a| (*k2, a)))
            {
                held += 1;
                let err = phase(a, k2) - phase(d.frac, 0.0);
                assert!(
                    err.abs() < 1e-9,
                    "key {}: the first partial's phase moved {err:e}",
                    k.key
                );
                assert!(a.abs() <= 0.99);
            }
        }
    }
    assert!(
        held * 10 >= tried * 9,
        "only {held} of {tried} scatters held"
    );
}

/// Round 40 (candidate 11): the strike point. Placed in the delay lines alone (beta.14's,
/// `strike_true` 0) it sits short of where the key's strike fraction puts it wherever the
/// dispersion sections carry part of the period — middle C's comb notched partials 12–13,
/// not 8. Placed true (1), the strike is the key's fraction of the loop's phase delay at
/// the partial its comb notches, to half a sample; the top keys keep their one sample.
#[test]
fn the_strike_point_lands_where_the_key_puts_it() {
    let fs = 48_000.0;
    let keys = keyboard_for(&Tuning::default());
    for m in (21..=108u8).step_by(3) {
        let k = keys[(m - LOWEST) as usize];
        let d = design(k.f0(), k.b, fs);
        let old = ((k.strike * d.delay.max(3) as f64 / 2.0).round() as usize).max(1);
        assert_eq!(KeyStrings::nut_samples(&k, &d, 0.0), old, "key {m}");
        let n = 1.0 / k.strike;
        let p = fs / (k.f0() * (1.0 + k.b * n * n).sqrt());
        let nut = KeyStrings::nut_samples(&k, &d, 1.0);
        if m >= 96 {
            assert_eq!(nut, old, "key {m}: the top keys keep their sample");
        } else {
            assert!(
                (2.0 * nut as f64 - k.strike * p).abs() <= 1.0,
                "key {m}: the strike at {:.3} of the string, not {:.3}",
                2.0 * nut as f64 / p,
                k.strike
            );
        }
        if m == 60 {
            assert!(2.0 * old as f64 / p < 0.095, "middle C's old strike");
        }
    }
}

/// Round 40 (candidate 11): placed true, middle C's strike notches partial 8 (the reference's
/// strike leaves partials 8–9 low) and frees partials 12–13, which the old placement notched.
#[test]
fn the_true_strike_point_notches_middle_cs_eighth_partial() {
    let fs = 48_000.0;
    let keys = keyboard_for(&Tuning::default());
    let k = keys[(60 - LOWEST) as usize];
    let d = design(k.f0(), k.b, fs);
    let render = |strike_true: f64| {
        let mut s = KeyStrings::new(d.delay + 16, fs);
        s.lay(&k, &d, [0.0; MAX_STRINGS], 1, 0, 0.0, strike_true);
        let (g, a, yz) = loss(&k, fs, 0.5, 1.0, 0.0, 0.0);
        s.set_loss(g, a, yz);
        s.strike(felt_of(&k), k.hammer_mass, 2.0, [1.0; MAX_STRINGS]);
        (0..(fs as usize)).map(|_| s.tick()).collect::<Vec<f64>>()
    };
    let (a, b) = (render(0.0), render(1.0));
    let lv = |x: &[f64], n: u32| partial_level(x, fs, k.partial(n), 0.05);
    let rel = |x: &[f64], n: u32| lv(x, n) - lv(x, 1);
    let (p8a, p8b) = (rel(&a, 8), rel(&b, 8));
    let (p12a, p12b) = (rel(&a, 12) + rel(&a, 13), rel(&b, 12) + rel(&b, 13));
    eprintln!("middle C, re partial 1: p8 {p8a:.1} -> {p8b:.1} dB; p12+p13 {p12a:.1} -> {p12b:.1}");
    assert!(p8b < p8a - 6.0);
    assert!(p12b > p12a + 6.0);
}

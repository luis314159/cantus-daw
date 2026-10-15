//! Chorda's loop designs: every key's partials where the stiff string has them.

use super::design::*;
use super::tables::*;

#[test]
fn every_key_is_designed_within_a_cent_of_its_partials_at_44_1_and_48_khz() {
    for fs in [44_100.0, 48_000.0] {
        let keys = keyboard(440.0, 1.0, 1.0);
        let mut sections = [0usize; MAX_SECTIONS + 1];
        let mut worst: (f64, u8) = (0.0, 0);
        for k in keys.iter() {
            let d = design(k.f0(), k.b, fs);
            sections[d.sections] += 1;
            if d.worst_cents > worst.0 {
                worst = (d.worst_cents, k.key);
            }
            assert!(
                d.worst_cents < 1.0,
                "{} at {fs}: {:.3} cents",
                k.key,
                d.worst_cents
            );
            // Its first resonance is the key's tuning to a hundredth of a cent.
            let f1 = d.resonance(1);
            let c = 1200.0 * crate::math::log2(f1 / k.f1);
            assert!(c.abs() < 1e-2, "{}: {c} cents", k.key);
        }
        eprintln!(
            "{fs} Hz: worst {:.3} cents at {}; sections {sections:?}",
            worst.0, worst.1
        );
    }
}

#[test]
fn a_design_is_the_same_bits_every_time() {
    let k = keyboard(440.0, 1.0, 1.0)[24];
    let a = design(k.f0(), k.b, 48_000.0);
    let b = design(k.f0(), k.b, 48_000.0);
    assert_eq!(a, b);
}

#[test]
fn a_harmonic_string_needs_almost_no_dispersion() {
    // B = 0: the allpasses only tune the fractional part of the period.
    let d = design(220.0, 0.0, 48_000.0);
    assert!(d.worst_cents < 0.5);
    for k in 1..=20 {
        let c = 1200.0 * crate::math::log2(d.resonance(k) / (220.0 * k as f64));
        assert!(c.abs() < 0.5, "partial {k}: {c}");
    }
}

#[test]
#[ignore]
fn print_a_bass_design() {
    let k = keyboard(440.0, 1.0, 1.0)[0];
    let d = design(k.f0(), k.b, 44_100.0);
    eprintln!(
        "sections {} delay {} worst {:.3}",
        d.sections, d.delay, d.worst_cents
    );
    eprintln!("poles {:?} frac {}", &d.poles[..d.sections], d.frac);
    for n in [1u32, 2, 4, 8, 12, 16, 20, 24, 28, 32] {
        let c = 1200.0 * crate::math::log2(d.resonance(n) / target_hz(k.f0(), k.b, n));
        eprintln!("partial {n}: {c:+.3} cents");
    }
}

#[test]
#[ignore]
fn time_the_designs() {
    let keys = keyboard(440.0, 1.0, 1.0);
    let t0 = std::time::Instant::now();
    for k in keys.iter().step_by(4) {
        let t = std::time::Instant::now();
        let d = design(k.f0(), k.b, 48_000.0);
        eprintln!(
            "{:3}: {:3} sections, {:.3} cents, {:6.1} ms",
            k.key,
            d.sections,
            d.worst_cents,
            t.elapsed().as_secs_f64() * 1e3
        );
    }
    eprintln!("total {:?}", t0.elapsed());
}

#[test]
fn a_keyboard_is_the_same_on_one_thread_or_eight() {
    let t = Tuning {
        a4: 442.0,
        stretch: 0.8,
        stiffness: 1.1,
    };
    let a = Keyboard::compute(t, 48_000.0, 1);
    let b = Keyboard::compute(t, 48_000.0, 8);
    assert_eq!(a, b);
    assert_eq!(a.loops.len(), KEYS);
    // A detuned unison string: the fraction's slope raises the pitch by the cents asked.
    let mut d = a.loops[40];
    let f = d.resonance(1);
    d.frac += 3.0 * d.frac_per_cent;
    let up = 1200.0 * crate::math::log2(d.resonance(1) / f);
    assert!((up - 3.0).abs() < 0.05, "{up} cents");
}

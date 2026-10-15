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
    let keys = keyboard(440.0, 1.0, 1.0);
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
    let (k, x) = render_key(m, fs, 3.0, 2.0, 1);
    let checked = checked_partials(k.f0(), k.b, fs);
    let n = 1 << 16;
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
            "key {m}, partial {i}: {got:.4} Hz against {want:.4} ({c:+.3} cents)"
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

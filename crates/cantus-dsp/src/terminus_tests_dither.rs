//! The counter-based dither (ADR 0031 R10, round 22): at rest it *is* a new ditherer, and
//! its noise has the statistics the old sequential generator's had.

use super::*;

const SEEDS: [u64; 4] = [0x9e37_79b9, 0x85eb_ca6b, 0x2545_f491, 0xc2b2_ae35];
const SHAPINGS: [Shaping; 3] = [Shaping::Flat, Shaping::Basic, Shaping::Optimized];

/// A loud, busy signal: xorshift noise over a tone, some samples exactly zero.
fn sound(n: usize, seed: u32) -> Vec<f32> {
    let mut s = seed.max(1);
    (0..n)
        .map(|i| {
            s ^= s << 13;
            s ^= s >> 17;
            s ^= s << 5;
            if s.is_multiple_of(97) {
                0.0
            } else {
                0.4 * math::sin(i as f32 * 0.03) + 0.3 * (s as f32 / u32::MAX as f32 - 0.5)
            }
        })
        .collect()
}

fn run(d: &mut Ditherer, x: &[f32], bits: u32, shaping: Shaping) -> Vec<f32> {
    x.iter().map(|v| d.process(*v, bits, shaping)).collect()
}

/// After sound and 64 zeros a ditherer equals a new one (state for state), and from there
/// it dithers the next sound to the same bits — at every shaping and word length.
#[test]
fn a_ditherer_at_rest_is_a_new_one() {
    for shaping in SHAPINGS {
        for bits in [16, 20, 24] {
            for (k, seed) in SEEDS.into_iter().enumerate() {
                let fresh = Ditherer::new(seed);
                assert!(fresh.at_rest(), "a new ditherer is at rest");
                let mut d = fresh.clone();
                let out = run(&mut d, &sound(5_000 + 777 * k, 3 + k as u32), bits, shaping);
                assert!(out.iter().any(|v| *v != 0.0));
                assert!(!d.at_rest(), "sound in, not at rest");
                let tail = run(&mut d, &[0.0; 63], bits, shaping);
                assert!(!d.at_rest(), "63 zeros are not yet rest");
                assert!(tail.iter().any(|v| *v != 0.0), "the shaped tail dithers on");
                assert_eq!(run(&mut d, &[0.0], bits, shaping), [0.0]);
                assert!(d.at_rest(), "{shaping:?} {bits}: 64 zeros bring it to rest");
                assert_eq!(d, fresh, "{shaping:?} {bits}: at rest is a new ditherer");
                // More silence: silent and still at rest.
                assert!(run(&mut d, &[0.0; 1_000], bits, shaping).iter().all(|v| v.to_bits() == 0));
                assert_eq!(d, fresh);
                // The next sound: the same bits as a new ditherer's.
                let next = sound(3_000, 99);
                let (a, b) = (run(&mut d, &next, bits, shaping), run(&mut fresh.clone(), &next, bits, shaping));
                assert!(a.iter().zip(&b).all(|(x, y)| x.to_bits() == y.to_bits()));
            }
        }
    }
}

/// Digital silence at the start stays silent (a new ditherer is at rest), and a sound that
/// stops short of 64 zeros carries on with the same stream.
#[test]
fn silence_first_is_silent_and_short_gaps_keep_the_stream() {
    let mut d = Ditherer::new(SEEDS[0]);
    assert!(run(&mut d, &[0.0; 500], 16, Shaping::Optimized).iter().all(|v| v.to_bits() == 0));
    // A gap of 10 zeros inside a sound: dithered, the counter goes on.
    let mut x = sound(1_000, 5);
    x[400..410].iter_mut().for_each(|v| *v = 0.0);
    let mut a = Ditherer::new(SEEDS[0]);
    let out = run(&mut a, &x, 16, Shaping::Flat);
    assert!(out[400..410].iter().any(|v| *v != 0.0), "a short gap is dithered");
    assert_eq!(a.n, 1_000);
}

/// The noise itself: zero mean, variance 1/6 LSB² (two uniforms' difference), triangular
/// (three quarters of it within ±½ LSB), white (lag-1 correlation nil) and independent
/// between the channels' seeds.
#[test]
fn the_noise_is_white_triangular_and_independent() {
    const N: usize = 1 << 20;
    let streams: Vec<Vec<f64>> = SEEDS
        .iter()
        .map(|&s| {
            let mut d = Ditherer::new(s);
            (0..N).map(|_| d.tpdf() as f64).collect()
        })
        .collect();
    let sd = (1.0f64 / 6.0).sqrt();
    let tol = 5.0 / (N as f64).sqrt();
    for (k, v) in streams.iter().enumerate() {
        let mean = v.iter().sum::<f64>() / N as f64;
        let var = v.iter().map(|x| (x - mean) * (x - mean)).sum::<f64>() / N as f64;
        let lag1 = v.windows(2).map(|w| (w[0] - mean) * (w[1] - mean)).sum::<f64>()
            / (N - 1) as f64
            / var;
        let inner = v.iter().filter(|x| x.abs() < 0.5).count() as f64 / N as f64;
        assert!(v.iter().all(|x| x.abs() < 1.0));
        assert!(mean.abs() < sd * tol, "seed {k}: mean {mean}");
        assert!((var - 1.0 / 6.0).abs() < 0.002, "seed {k}: variance {var}");
        assert!(lag1.abs() < tol, "seed {k}: lag-1 correlation {lag1}");
        assert!((inner - 0.75).abs() < 0.002, "seed {k}: {inner} within ±½ LSB");
    }
    for a in 0..SEEDS.len() {
        for b in a + 1..SEEDS.len() {
            let c = streams[a].iter().zip(&streams[b]).map(|(x, y)| x * y).sum::<f64>()
                / N as f64
                / (1.0 / 6.0);
            assert!(c.abs() < tol, "seeds {a} and {b}: correlation {c}");
        }
    }
}

/// The limiter itself, dithering: after its signal and silence it comes to rest, and a
/// unit at rest renders what a new one renders. (Punch: a style without the programme
/// memory, which reaches zero only with denormals flushed — the engine's contract tests
/// cover every style under FTZ.)
#[test]
fn a_dithering_limiter_comes_to_rest_as_a_new_one() {
    for shaping in SHAPINGS {
        let s = LimSettings {
            dither: 5,
            shaping,
            gain_db: 6.0,
            style: LimStyle::Punch,
            ..LimSettings::default()
        };
        let mut t = Terminus::new(48_000.0);
        t.set(&s);
        let x = sound(20_000, 11);
        let (mut l, mut r) = (x.clone(), x.clone());
        t.process(&mut l, &mut r, None);
        let (mut rested, mut ran) = (false, x.len());
        for _ in 0..2_000 {
            let (mut l, mut r) = (vec![0f32; 512], vec![0f32; 512]);
            t.process(&mut l, &mut r, None);
            ran += 512;
            if t.at_rest() {
                rested = true;
                break;
            }
        }
        assert!(rested, "{shaping:?}: no rest ({:?})", t.rest_blocker());
        // A new unit, skipped as far as this one has run (ADR 0031 §5.1's "as good as new").
        let mut fresh = Terminus::new(48_000.0);
        fresh.set(&s);
        fresh.skip(ran);
        let y = sound(9_000, 12);
        let (mut a, mut b) = ((y.clone(), y.clone()), (y.clone(), y.clone()));
        t.process(&mut a.0, &mut a.1, None);
        fresh.process(&mut b.0, &mut b.1, None);
        assert!(a.0.iter().any(|v| *v != 0.0));
        assert!(
            a.0.iter().chain(&a.1).zip(b.0.iter().chain(&b.1)).all(|(p, q)| p.to_bits() == q.to_bits()),
            "{shaping:?}: at rest, not as a new limiter"
        );
    }
}

//! Vinculum's tests: the null contract, block-size independence, exact rest, and each
//! model's static curve against an independent analysis (the same relation solved in
//! `f64` by bisection). Time constants: `vinculum_tests_time.rs`; measurements (CPU,
//! aliasing, the auto release on a drum loop): `vinculum_measure.rs`.

use super::law::{self, MuLaw, OptoLaw};
use super::*;

pub(super) const SR: f32 = 48_000.0;

pub(super) fn unit(s: &VinculumSettings) -> Vinculum {
    let mut v = Vinculum::new(SR);
    v.set(s);
    v
}

/// A deterministic test signal: two chords' worth of sines under a pumping envelope, a
/// noise burst every 250 ms, some samples of −0.0, peaking near `peak`.
pub(super) fn programme(seconds: f32, peak: f32) -> (Vec<f32>, Vec<f32>) {
    let n = (seconds * SR) as usize;
    let mut seed = 0x1234_5678u32;
    let mut l = Vec::with_capacity(n);
    let mut r = Vec::with_capacity(n);
    for i in 0..n {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let noise = (seed >> 8) as f32 / (1u32 << 24) as f32 * 2.0 - 1.0;
        let t = i as f32 / SR;
        let beat = (t * 4.0).fract();
        let env = math::exp(-beat * 12.0);
        let tone = 0.4 * math::sin(2.0 * std::f32::consts::PI * 110.0 * t)
            + 0.25 * math::sin(2.0 * std::f32::consts::PI * 277.2 * t)
            + 0.2 * math::sin(2.0 * std::f32::consts::PI * 1661.0 * t);
        let a = peak * (0.5 * tone * (0.3 + 0.7 * env) + 0.5 * noise * env * env);
        let b = peak * (0.5 * tone * (0.35 + 0.65 * env) - 0.45 * noise * env * env);
        if i % 997 == 0 {
            l.push(-0.0);
            r.push(-0.0);
        } else {
            l.push(a);
            r.push(b);
        }
    }
    (l, r)
}

pub(super) fn run(v: &mut Vinculum, l: &[f32], r: &[f32], block: usize) -> (Vec<f32>, Vec<f32>) {
    let (mut a, mut b) = (l.to_vec(), r.to_vec());
    for (x, y) in a.chunks_mut(block).zip(b.chunks_mut(block)) {
        v.process(x, y);
    }
    (a, b)
}

fn bits(x: &[f32]) -> Vec<u32> {
    x.iter().map(|v| v.to_bits()).collect()
}

/// Settings that work hard, per model.
pub(super) fn hard(model: Model) -> VinculumSettings {
    VinculumSettings {
        model,
        threshold_db: -24.0,
        ratio: 10.0,
        attack_ms: 0.3,
        release_ms: 100.0,
        mu_time: 0,
        makeup_db: 8.0,
        colour: 1.0,
        ..VinculumSettings::default()
    }
}

/// Every configuration worth holding a contract over.
pub(super) fn configurations() -> Vec<VinculumSettings> {
    let mut out = Vec::new();
    for model in Model::ALL {
        for character in [Character::Compress, Character::Limit] {
            let mut s = hard(model);
            s.character = character;
            out.push(s);
            out.push(VinculumSettings {
                sc_hpf_hz: 150.0,
                auto_release: true,
                mu_time: 5,
                mix: 0.4,
                ..s
            });
            out.push(VinculumSettings {
                mu_time: 4,
                colour: 0.0,
                threshold_db: -12.0,
                ..s
            });
        }
    }
    out
}

#[test]
fn mix_zero_touches_no_sample() {
    let (l, r) = programme(1.0, 1.5);
    for s in configurations() {
        let mut v = unit(&VinculumSettings { mix: 0.0, ..s });
        let (a, b) = run(&mut v, &l, &r, 64);
        assert_eq!(bits(&a), bits(&l), "{s:?}");
        assert_eq!(bits(&b), bits(&r), "{s:?}");
        // It still measures: the needle moves under a mix of 0.
        assert!(v.take_meter().gr_peak_db > 1.0, "{s:?}");
    }
}

#[test]
fn a_threshold_above_the_signal_touches_no_sample() {
    // Peaks of +6 dBFS against the threshold's top (+24): no model's knee is reached.
    let (l, r) = programme(1.0, 2.0);
    for s in configurations() {
        let s = VinculumSettings {
            threshold_db: THRESHOLD_MAX_DB,
            makeup_db: 0.0,
            colour: 0.0,
            ..s
        };
        let mut v = unit(&s);
        let (a, b) = run(&mut v, &l, &r, 37);
        assert_eq!(bits(&a), bits(&l), "{s:?}");
        assert_eq!(bits(&b), bits(&r), "{s:?}");
        assert_eq!(v.gain_reduction_db(), 0.0);
    }
    // And the defaults are that null.
    let mut v = Vinculum::new(SR);
    let (a, b) = run(&mut v, &l, &r, 512);
    assert_eq!(bits(&a), bits(&l));
    assert_eq!(bits(&b), bits(&r));
}

#[test]
fn the_colour_is_the_only_thing_that_touches_a_quiet_signal() {
    // Colour on, nothing compressing: the vari-mu's valve colours by design (a control,
    // default 0) — and only through the residual it adds.
    let (l, r) = programme(0.5, 0.5);
    let s = VinculumSettings {
        model: Model::VariMu,
        colour: 1.0,
        ..VinculumSettings::default()
    };
    let mut v = unit(&s);
    let (a, _) = run(&mut v, &l, &r, 64);
    let diff = a
        .iter()
        .zip(&l)
        .map(|(y, x)| (y - x).abs())
        .fold(0f32, f32::max);
    assert!(diff > 1e-5 && diff < 0.01, "colour residual {diff}");
}

#[test]
fn block_size_does_not_change_a_bit() {
    let (l, r) = programme(1.5, 1.5);
    for s in configurations() {
        let reference = run(&mut unit(&s), &l, &r, 1);
        for block in [37, 64, 512] {
            let got = run(&mut unit(&s), &l, &r, block);
            assert_eq!(bits(&got.0), bits(&reference.0), "{s:?} at {block}");
            assert_eq!(bits(&got.1), bits(&reference.1), "{s:?} at {block}");
        }
    }
}

#[test]
fn latency_is_zero_and_a_click_stays_where_it_is() {
    for s in configurations() {
        let mut v = unit(&s);
        assert_eq!(v.latency(), 0);
        let mut l = vec![0.0f32; 4096];
        l[1000] = 1.0;
        let mut r = l.clone();
        v.process(&mut l, &mut r);
        let at = l
            .iter()
            .enumerate()
            .fold(
                (0, 0f32),
                |m, (i, x)| if x.abs() > m.1 { (i, x.abs()) } else { m },
            )
            .0;
        assert_eq!(at, 1000, "{s:?}");
    }
}

#[test]
fn silence_comes_to_an_exact_rest_and_rest_is_as_new() {
    let (l, r) = programme(1.0, 1.5);
    let (tl, tr) = programme(0.5, 1.0);
    for s in configurations() {
        // The slowest position (5 s, or 15 s held) is checked in its own test below.
        let s = VinculumSettings { mu_time: 1, ..s };
        let mut v = unit(&s);
        run(&mut v, &l, &r, 64);
        assert!(!v.at_rest());
        // The valve's half-band rings for a few samples after the music; after a second
        // of silence every model's output is exact silence, and it stays so until rest.
        let mut silent = 0usize;
        let zeros = vec![0.0f32; 512];
        while !v.at_rest() {
            let (mut a, mut b) = (zeros.clone(), zeros.clone());
            v.process(&mut a, &mut b);
            if silent >= 48_000 {
                assert!(a.iter().chain(&b).all(|x| x.to_bits() == 0), "{s:?}");
            }
            silent += 512;
            assert!(silent < 120 * 48_000, "{s:?} never rests");
        }
        // Skipping holds the rest, and the rested unit is a new one.
        v.skip(48_000);
        assert!(v.at_rest());
        let twin = run(&mut unit(&s), &tl, &tr, 64);
        let got = run(&mut v, &tl, &tr, 64);
        assert_eq!(bits(&got.0), bits(&twin.0), "{s:?}");
        assert_eq!(bits(&got.1), bits(&twin.1), "{s:?}");
    }
}

#[test]
fn the_slowest_positions_rest_too() {
    let (l, r) = programme(3.0, 1.5);
    for t in [3, 5] {
        let s = VinculumSettings {
            mu_time: t,
            ..hard(Model::VariMu)
        };
        let mut v = unit(&s);
        run(&mut v, &l, &r, 512);
        let mut a = vec![0.0f32; 48_000];
        let mut b = a.clone();
        let mut secs = 0;
        while !v.at_rest() {
            a.fill(0.0);
            b.fill(0.0);
            v.process(&mut a, &mut b);
            secs += 1;
            assert!(secs < 900, "position {t} never rests");
        }
        println!("position {}: at rest {secs} s after the music", t + 1);
    }
}

/// A steady (DC) level through a unit until it settles: the gain it shows, dB.
pub(super) fn settled_gain_db(s: &VinculumSettings, level: f32, seconds: f32) -> f32 {
    let mut v = unit(s);
    let n = (seconds * SR) as usize;
    let mut l = vec![level; n];
    let mut r = l.clone();
    v.process(&mut l, &mut r);
    20.0 * math::log10(l[n - 1] / level)
}

/// Giannoulis eq. 4, written again in f64.
fn giannoulis(x: f64, t: f64, r: f64, w: f64) -> f64 {
    if 2.0 * (x - t) < -w {
        x
    } else if 2.0 * (x - t).abs() <= w {
        x + (1.0 / r - 1.0) * math::powi(x - t + w / 2.0, 2) / (2.0 * w)
    } else {
        t + (x - t) / r
    }
}

#[test]
fn the_vca_follows_the_published_gain_computer() {
    for ratio in VCA_RATIOS {
        let s = VinculumSettings {
            threshold_db: -20.0,
            ratio,
            attack_ms: 0.1,
            release_ms: 1200.0,
            ..VinculumSettings::default()
        };
        for x_db in [-40.0f32, -22.0, -21.0, -20.0, -19.0, -15.0, -6.0, 0.0, 6.0] {
            let got = settled_gain_db(&s, math::db_to_gain(x_db), 0.2);
            let want =
                giannoulis(x_db as f64, -20.0, ratio as f64, VCA_KNEE_DB as f64) - x_db as f64;
            assert!(
                (got as f64 - want).abs() < 0.01,
                "{ratio}:1 at {x_db} dB: {got} dB, the curve {want} dB"
            );
        }
    }
}

/// Bisection, f64: the `y` in `[0, x]` where `input(y) = x`, `input` increasing.
fn solve(x: f64, input: impl Fn(f64) -> f64) -> f64 {
    let (mut lo, mut hi) = (0.0, x);
    for _ in 0..200 {
        let m = 0.5 * (lo + hi);
        if input(m) < x {
            lo = m;
        } else {
            hi = m;
        }
    }
    0.5 * (lo + hi)
}

/// The opto's static output for an input `x` (linear), solved from the cell's law.
fn opto_out(law: &OptoLaw, t: f64, x: f64) -> f64 {
    solve(x, |y| {
        let d = y / t - law.u0 as f64;
        let g = if d > 0.0 {
            law.c as f64 * math::powi(d, law.p)
        } else {
            0.0
        };
        y * (1.0 + g)
    })
}

/// The vari-mu's static output, solved from the valve's law (in dB: the input is the
/// output plus the reduction the output asks for).
fn mu_out(law: &MuLaw, t: f64, x: f64) -> f64 {
    let t_db = 20.0 * math::log10(t);
    let x_db = 20.0 * math::log10(x);
    let y_db = solve_db(|y| y + law.gr_db(law.knee((y - t_db) as f32)) as f64, x_db);
    amp(y_db)
}

/// A level in dB as an amplitude.
fn amp(db: f64) -> f64 {
    math::powf(10.0, db / 20.0)
}

fn local_ratio(out: impl Fn(f64) -> f64, x_db: f64) -> f64 {
    let h = 0.05;
    let y = |d: f64| 20.0 * math::log10(out(amp(d)));
    2.0 * h / (y(x_db + h) - y(x_db - h))
}

#[test]
fn the_opto_settles_on_its_cell_law() {
    for (character, law) in [
        (Character::Compress, OptoLaw::compress()),
        (Character::Limit, OptoLaw::limit()),
    ] {
        let s = VinculumSettings {
            model: Model::Opto,
            threshold_db: -20.0,
            character,
            ..VinculumSettings::default()
        };
        let t = 0.1;
        for x_db in [-30.0f32, -26.0, -20.0, -14.0, -6.0, 0.0] {
            let got = settled_gain_db(&s, math::db_to_gain(x_db), 3.0);
            let x = amp(x_db as f64);
            let want = 20.0 * math::log10(opto_out(&law, t, x) / x);
            assert!(
                (got as f64 - want).abs() < 0.02,
                "{character:?} at {x_db}: {got} dB, the law {want} dB"
            );
        }
    }
}

#[test]
fn the_opto_ratios_are_the_cells() {
    let t = 0.1;
    let c = OptoLaw::compress();
    let l = OptoLaw::limit();
    let rc = |x| local_ratio(|v| opto_out(&c, t, v), x);
    let rl = |x| local_ratio(|v| opto_out(&l, t, v), x);
    // Compress: soft (under 1.5 : 1 at the threshold), tending to 3 : 1.
    assert!(rc(-20.0) < 1.6, "{}", rc(-20.0));
    assert!((2.5..3.0).contains(&rc(10.0)), "{}", rc(10.0));
    // Limit: firm, 10 : 1 in the end.
    for x in [-30.0, -20.0, -10.0, 0.0, 10.0] {
        println!(
            "opto at {x} dB in: compress {:.2} : 1, limit {:.2} : 1",
            rc(x),
            rl(x)
        );
    }
    assert!((6.0..14.0).contains(&rl(-5.0)), "{}", rl(-5.0));
    // About 10 : 1 at depth: 20 and 30 dB over the threshold.
    assert!((9.0..10.5).contains(&rl(0.0)), "{}", rl(0.0));
    assert!((9.5..10.5).contains(&rl(10.0)), "{}", rl(10.0));
}

#[test]
fn the_vari_mu_settles_on_its_valve_law() {
    for (character, law) in [
        (Character::Compress, MuLaw::compress()),
        (Character::Limit, MuLaw::limit()),
    ] {
        let s = VinculumSettings {
            model: Model::VariMu,
            threshold_db: -20.0,
            character,
            mu_time: 0,
            ..VinculumSettings::default()
        };
        for x_db in [-30.0f32, -21.0, -20.0, -16.0, -10.0, 0.0, 6.0] {
            let got = settled_gain_db(&s, math::db_to_gain(x_db), 2.0);
            let x = amp(x_db as f64);
            let want = 20.0 * math::log10(mu_out(&law, 0.1, x) / x);
            assert!(
                (got as f64 - want).abs() < 0.02,
                "{character:?} at {x_db}: {got} dB, the law {want} dB"
            );
        }
    }
}

#[test]
fn the_vari_mu_ratio_rises_with_level() {
    // Published: compress starts at 1.5 : 1 and steepens; limit starts at 4 : 1 and
    // reaches 20 : 1 past 12 dB of reduction. Measured on the static curve solved from
    // the law, by finite differences.
    for (law, start, at12) in [
        (MuLaw::compress(), 1.5, (2.7, 3.1)),
        (MuLaw::limit(), 4.0, (18.0, 20.01)),
    ] {
        let t = 0.1;
        let out = |x: f64| mu_out(&law, t, x);
        let gr_at = |x_db: f64| x_db - 20.0 * math::log10(out(amp(x_db)));
        // The input level where the reduction is `gr` dB.
        let input_for = |gr: f64| {
            let (mut lo, mut hi) = (-40.0, 80.0);
            for _ in 0..100 {
                let m = 0.5 * (lo + hi);
                if gr_at(m) < gr {
                    lo = m;
                } else {
                    hi = m;
                }
            }
            lo
        };
        // The law starts at the published ratio; past the knee the solved curve's local
        // ratio is the law's at its reduction.
        assert_eq!(law.r0, start);
        let past = solve_db(|x| 20.0 * math::log10(out(amp(x))), -20.0 + law.w as f64);
        for x in [past, past + 3.0, past + 10.0] {
            let r = local_ratio(out, x);
            let want = law.ratio_at(gr_at(x) as f32) as f64;
            assert!(
                (r - want).abs() < 0.02 * want,
                "{law:?} at {x} dB: {r} : 1, the law {want}"
            );
        }
        let r12 = local_ratio(out, input_for(12.0));
        assert!(
            (at12.0..at12.1).contains(&r12),
            "{law:?}: {r12} : 1 at 12 dB"
        );
        let mut last = 0.0;
        for gr in [0.5, 1.0, 3.0, 6.0, 12.0, 20.0, 30.0] {
            let r = local_ratio(out, input_for(gr));
            assert!(
                r >= last - 1e-2,
                "the ratio fell at {gr} dB: {r} after {last}"
            );
            last = r;
            println!(
                "vari-mu ({} : 1 start) at {gr} dB of reduction: {r:.2} : 1",
                law.r0
            );
        }
    }
}

/// The input level, dB, where `out_db(x)` reaches `y`, `out_db` increasing.
fn solve_db(out_db: impl Fn(f64) -> f64, y: f64) -> f64 {
    let (mut lo, mut hi) = (-60.0, 80.0);
    for _ in 0..100 {
        let m = 0.5 * (lo + hi);
        if out_db(m) < y {
            lo = m;
        } else {
            hi = m;
        }
    }
    lo
}

#[test]
fn the_limit_does_not_ring_at_its_fastest() {
    let s = VinculumSettings {
        model: Model::VariMu,
        character: Character::Limit,
        threshold_db: -30.0,
        mu_time: 0,
        ..VinculumSettings::default()
    };
    let n = 48_000;
    let mut l = vec![0.5f32; n];
    let mut r = l.clone();
    let mut v = unit(&s);
    v.process(&mut l, &mut r);
    let tail = &l[n - 4800..];
    let (lo, hi) = tail
        .iter()
        .fold((f32::MAX, f32::MIN), |m, x| (m.0.min(*x), m.1.max(*x)));
    assert!(hi - lo < 1e-5 * hi, "the settled output moves: {lo} … {hi}");
}

#[test]
fn the_valve_is_odd_and_as_strong_as_stated() {
    // A full-scale 1 kHz sine, nothing compressing, colour 1: depth 0.3 of 1 % third.
    let s = VinculumSettings {
        model: Model::VariMu,
        colour: 1.0,
        ..VinculumSettings::default()
    };
    let mut v = unit(&s);
    let n = 48_000;
    let x: Vec<f32> = (0..n)
        .map(|i| math::sin(2.0 * std::f32::consts::PI * 1000.0 * i as f32 / SR))
        .collect();
    let (a, _) = run(&mut v, &x, &x, 256);
    let tail = &a[n / 2..];
    let h = |k: f64| goertzel(tail, 1000.0 * k) / goertzel(tail, 1000.0);
    let h2 = 20.0 * math::log10(h(2.0));
    let h3 = 20.0 * math::log10(h(3.0));
    let want = 20.0 * math::log10(COLOUR_REST_DEPTH as f64 * 0.01);
    assert!((h3 - want).abs() < 0.5, "third {h3} dB, stated {want} dB");
    assert!(h2 < -110.0, "second {h2} dB: a push-pull stage is odd");
}

/// The amplitude of `hz` in `x` under a Hann window.
pub(super) fn goertzel(x: &[f32], hz: f64) -> f64 {
    let n = x.len();
    let w = 2.0 * std::f64::consts::PI * hz / SR as f64;
    let (mut re, mut im) = (0.0, 0.0);
    for (i, v) in x.iter().enumerate() {
        let win = 0.5 - 0.5 * math::cos(2.0 * std::f64::consts::PI * i as f64 / n as f64);
        let p = w * i as f64;
        re += *v as f64 * win * math::cos(p);
        im += *v as f64 * win * math::sin(p);
    }
    (re * re + im * im).sqrt() * 4.0 / n as f64
}

#[test]
fn the_half_band_is_the_published_design() {
    // Reference values from the design's formulae evaluated independently (Python, f64).
    let want = [
        0.044_296_928_346,
        0.164_569_550_328,
        0.331_371_448_984,
        0.515_803_286_191,
        0.702_645_411_568,
        0.895_142_349_285,
    ];
    let got = valve::halfband(valve::COEFS, valve::TRANSITION);
    for (g, w) in got.iter().zip(want) {
        assert!((g - w).abs() < 1e-9, "{g} vs {w}");
    }
}

#[test]
fn the_meter_reads_each_model() {
    let (l, r) = programme(1.0, 1.5);
    for model in Model::ALL {
        let mut v = unit(&hard(model));
        run(&mut v, &l, &r, 256);
        let m = v.take_meter();
        assert!(
            m.gr_peak_db >= m.gr_db && m.gr_peak_db > 3.0,
            "{model:?} {m:?}"
        );
        assert!(m.gr_db >= 0.0 && m.gr_peak_db <= law::MAX_GR_DB + 1.0);
    }
}

#[path = "vinculum_tests_time.rs"]
mod time;

#[path = "vinculum_measure.rs"]
mod measure;

#[test]
fn the_vari_mu_table_is_its_equilibrium() {
    // The tabulated equilibrium against the law's own solve, over the knee, the rising
    // ratio, the ceiling's line and the deepest reduction.
    for mu in [MuLaw::compress(), MuLaw::limit()] {
        let eq = law::MuEquilibrium::new(&mu);
        let mut worst = 0f32;
        for i in 0..=8000 {
            let x = -5.0 + i as f32 * 0.01;
            let (got, want) = (eq.at(x), mu.equilibrium(x));
            worst = worst.max((got - want).abs());
            if x <= -0.5 * mu.w {
                assert_eq!(got.to_bits(), 0, "under the knee at {x} dB");
            }
        }
        println!("vari-mu table (r0 {}): worst {worst:.2e} dB", mu.r0);
        assert!(worst < 2e-3, "{worst} dB");
    }
}

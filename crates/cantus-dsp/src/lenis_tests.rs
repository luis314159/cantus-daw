//! Lenis's contract: the nulls, the block size, the latency, the delta, the rest.

use super::*;

pub(super) const SR: f32 = 48_000.0;

pub(super) type Stereo = (Vec<f32>, Vec<f32>);

/// A deterministic uniform value in −1 … 1.
pub(super) fn hash(i: u64) -> f32 {
    let mut x = i.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    x ^= x >> 31;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 29;
    (x >> 40) as f32 / (1u64 << 23) as f32 - 1.0
}

pub(super) fn white(len: usize, seed: u64, peak: f32) -> Vec<f32> {
    (0..len)
        .map(|i| peak * hash(seed * 1_000_003 + i as u64))
        .collect()
}

/// Pink noise (Kellet's economy filter on white), about −3 dB an octave.
pub(super) fn pink(len: usize, seed: u64, gain: f32) -> Vec<f32> {
    let (mut b0, mut b1, mut b2) = (0.0f32, 0.0f32, 0.0f32);
    (0..len)
        .map(|i| {
            let w = hash(seed * 1_000_003 + i as u64);
            b0 = 0.99765 * b0 + w * 0.099_046;
            b1 = 0.96300 * b1 + w * 0.296_516_4;
            b2 = 0.57000 * b2 + w * 1.052_691_3;
            gain * 0.25 * (b0 + b1 + b2 + w * 0.1848)
        })
        .collect()
}

/// RBJ's peaking filter: `db` at `hz`, quality `q`, applied in place.
pub(super) fn peaking(x: &mut [f32], hz: f32, q: f32, db: f32) {
    let a = math::powf(10.0f64, db as f64 / 40.0);
    let w = 2.0 * std::f64::consts::PI * hz as f64 / SR as f64;
    let (sn, cs) = (math::sin(w), math::cos(w));
    let alpha = sn / (2.0 * q as f64);
    let a0 = 1.0 + alpha / a;
    let (b0, b1, b2) = (
        (1.0 + alpha * a) / a0,
        -2.0 * cs / a0,
        (1.0 - alpha * a) / a0,
    );
    let (a1, a2) = (-2.0 * cs / a0, (1.0 - alpha / a) / a0);
    let (mut x1, mut x2, mut y1, mut y2) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    for v in x.iter_mut() {
        let x0 = *v as f64;
        let y = b0 * x0 + b1 * x1 + b2 * x2 - a1 * y1 - a2 * y2;
        x2 = x1;
        x1 = x0;
        y2 = y1;
        y1 = y;
        *v = y as f32;
    }
}

/// A heavy setting: everything that can act, acting.
pub(super) fn heavy() -> LenisSettings {
    let mut s = LenisSettings {
        depth: 0.9,
        sharpness: 0.6,
        selectivity: 0.2,
        attack_ms: 2.0,
        release_ms: 60.0,
        low_hz: 60.0,
        high_hz: 16_000.0,
        mode: Mode::Hard,
        ..LenisSettings::default()
    };
    s.bands[0] = FocusBand {
        on: true,
        hz: 3_000.0,
        amount: 0.8,
        width_oct: 1.5,
    };
    s.bands[1] = FocusBand {
        on: true,
        hz: 200.0,
        amount: -0.5,
        width_oct: 1.0,
    };
    s
}

/// A harsh test signal: pink noise with two resonances and a ringing tone, a little
/// different on the right.
pub(super) fn harsh(len: usize, seed: u64) -> Stereo {
    let mut l = pink(len, seed, 0.3);
    let mut r = pink(len, seed + 7, 0.3);
    for (x, k) in [(&mut l, 0.0f32), (&mut r, 1.0)] {
        peaking(x, 3_150.0 + 40.0 * k, 12.0, 14.0);
        peaking(x, 900.0, 6.0, 9.0);
        for (i, v) in x.iter_mut().enumerate() {
            let t = i as f32 / SR;
            *v += 0.05 * math::sin(std::f32::consts::TAU * 5_200.0 * t);
        }
    }
    (l, r)
}

/// Run `x` through `u` in blocks of `block`.
pub(super) fn run(u: &mut Lenis, x: &Stereo, block: usize) -> Stereo {
    let (mut l, mut r) = (x.0.clone(), x.1.clone());
    for (a, b) in l.chunks_mut(block).zip(r.chunks_mut(block)) {
        u.process(a, b);
    }
    (l, r)
}

pub(super) fn unit(s: &LenisSettings) -> Lenis {
    let mut u = Lenis::new(SR);
    u.set(s);
    u
}

fn same(a: &Stereo, b: &Stereo) -> bool {
    a.0.len() == b.0.len()
        && a.0
            .iter()
            .chain(&a.1)
            .zip(b.0.iter().chain(&b.1))
            .all(|(p, q)| p.to_bits() == q.to_bits())
}

fn assert_delay(x: &Stereo, y: &Stereo, lat: usize, what: &str) {
    for (ch, (xi, yi)) in [(&x.0, &y.0), (&x.1, &y.1)].into_iter().enumerate() {
        for n in 0..yi.len() {
            let want = if n >= lat { xi[n - lat] } else { 0.0 };
            assert_eq!(
                yi[n].to_bits(),
                want.to_bits(),
                "{what}: channel {ch} at {n}"
            );
        }
    }
}

#[test]
fn depth_zero_and_mix_zero_are_the_input_delayed_to_the_bit() {
    let x = harsh(60_000, 3);
    let mut cases: Vec<(String, LenisSettings)> =
        vec![("defaults".into(), LenisSettings::default())];
    for q in Quality::ALL {
        for ch in [Channels::Linked, Channels::Independent, Channels::MidSide] {
            for m in [Mode::Soft, Mode::Hard] {
                let base = LenisSettings {
                    quality: q,
                    channels: ch,
                    mode: m,
                    ..heavy()
                };
                cases.push((
                    format!("depth 0 {q:?} {ch:?} {m:?}"),
                    LenisSettings {
                        depth: 0.0,
                        ..base
                    },
                ));
                cases.push((
                    format!("mix 0 {q:?} {ch:?} {m:?}"),
                    LenisSettings { mix: 0.0, ..base },
                ));
            }
        }
    }
    cases.push((
        "the focus's limits above the band".into(),
        LenisSettings {
            low_hz: 40_000.0,
            high_hz: 40_000.0,
            ..heavy()
        },
    ));
    for (what, s) in &cases {
        let mut u = unit(s);
        let y = run(&mut u, &x, 256);
        assert_eq!(u.latency(), latency_for(s, SR), "{what}");
        assert_delay(&x, &y, u.latency(), what);
    }
}

#[test]
fn the_latency_is_fixed_and_reported() {
    for (sr, q, want) in [
        (48_000.0, Quality::Fast, 255),
        (48_000.0, Quality::Standard, 511),
        (48_000.0, Quality::Precise, 1023),
        (44_100.0, Quality::Standard, 511),
        (96_000.0, Quality::Standard, 1023),
        (192_000.0, Quality::Standard, 2047),
    ] {
        let s = LenisSettings {
            quality: q,
            ..heavy()
        };
        assert_eq!(latency_for(&s, sr), want, "{sr} {q:?}");
        let mut u = Lenis::new(sr);
        u.set(&s);
        assert_eq!(u.latency(), want);
    }
    // Processed hard, the output still lines up with the input at the latency: the
    // cross-correlation peaks there.
    let x = harsh(48_000, 11);
    let mut u = unit(&heavy());
    let y = run(&mut u, &x, 128);
    let lat = u.latency();
    let corr = |lag: usize| -> f64 {
        (20_000..40_000)
            .map(|n| x.0[n - lag] as f64 * y.0[n] as f64)
            .sum()
    };
    let best = (lat - 40..lat + 40).max_by(|a, b| corr(*a).total_cmp(&corr(*b)));
    assert_eq!(best, Some(lat));
}

#[test]
fn the_output_does_not_depend_on_the_block_size() {
    let x = harsh(96_000, 5);
    let at = 512 * 60;
    let changes = [
        (
            heavy(),
            LenisSettings {
                depth: 0.3,
                sharpness: 0.2,
                mode: Mode::Soft,
                ..heavy()
            },
        ),
        (
            LenisSettings {
                channels: Channels::MidSide,
                ..heavy()
            },
            LenisSettings {
                channels: Channels::Independent,
                mix: 0.5,
                out_db: -3.0,
                delta: true,
                ..heavy()
            },
        ),
        (
            LenisSettings {
                quality: Quality::Fast,
                ..heavy()
            },
            LenisSettings {
                quality: Quality::Precise,
                ..heavy()
            },
        ),
        (LenisSettings::default(), heavy()),
    ];
    for (a, b) in &changes {
        let mut want: Option<Stereo> = None;
        for block in [1usize, 37, 64, 512] {
            let mut u = unit(a);
            let first = (x.0[..at].to_vec(), x.1[..at].to_vec());
            let rest = (x.0[at..].to_vec(), x.1[at..].to_vec());
            let mut y = run(&mut u, &first, block);
            u.set(b);
            let y2 = run(&mut u, &rest, block);
            y.0.extend(y2.0);
            y.1.extend(y2.1);
            match &want {
                None => want = Some(y),
                Some(w) => assert!(same(w, &y), "block {block}, {a:?} to {b:?}"),
            }
        }
    }
}

#[test]
fn a_setting_given_anywhere_in_a_hop_is_taken_at_the_next_hop() {
    let x = harsh(40_000, 9);
    let hop = heavy().quality.geometry(SR).hop();
    let start = 128 * hop;
    let mut want: Option<Vec<f32>> = None;
    for at in [start + 1, start + 17, start + hop - 1, start + hop] {
        let mut u = unit(&heavy());
        let (mut l, mut r) = (x.0.clone(), x.1.clone());
        u.process(&mut l[..at], &mut r[..at]);
        u.set(&LenisSettings {
            depth: 0.15,
            ..heavy()
        });
        u.process(&mut l[at..], &mut r[at..]);
        match &want {
            None => want = Some(l),
            Some(w) => assert!(
                w.iter().zip(&l).all(|(a, b)| a.to_bits() == b.to_bits()),
                "at {at}"
            ),
        }
    }
}

#[test]
fn delta_is_the_input_minus_the_output_to_the_bit() {
    let x = harsh(60_000, 13);
    for (mix, ch) in [
        (1.0, Channels::Linked),
        (0.6, Channels::Independent),
        (1.0, Channels::MidSide),
    ] {
        let s = LenisSettings {
            mix,
            channels: ch,
            ..heavy()
        };
        let y = run(&mut unit(&s), &x, 200);
        let d = run(&mut unit(&LenisSettings { delta: true, ..s }), &x, 200);
        let lat = latency_for(&s, SR);
        let mut energy = 0.0f64;
        for (xs, ys, ds) in [(&x.0, &y.0, &d.0), (&x.1, &y.1, &d.1)] {
            for n in 0..ys.len() {
                let dry = if n >= lat { xs[n - lat] } else { 0.0 };
                let want = dry - ys[n];
                assert_eq!(ds[n].to_bits(), want.to_bits(), "mix {mix} {ch:?} at {n}");
                energy += (ds[n] as f64) * (ds[n] as f64);
            }
        }
        assert!(energy > 1.0, "the delta carries what was taken ({energy})");
    }
}

#[test]
fn silence_in_is_silence_out_and_a_new_unit_is_at_rest() {
    for s in [LenisSettings::default(), heavy()] {
        let mut u = unit(&s);
        assert!(u.at_rest(), "{:?}", u.rest_blocker());
        let z = (vec![0.0f32; 10_000], vec![0.0f32; 10_000]);
        let y = run(&mut u, &z, 333);
        assert!(y.0.iter().chain(&y.1).all(|v| v.to_bits() == 0));
        assert!(u.at_rest());
    }
}

#[test]
fn it_comes_to_rest_exactly_and_its_twins_agree() {
    let music = harsh(30_000, 17);
    let after = harsh(30_000, 19);
    let block = vec![0.0f32; 64];
    let extra = 12_345usize;
    for s in [
        heavy(),
        LenisSettings {
            channels: Channels::MidSide,
            mode: Mode::Soft,
            release_ms: 2_000.0,
            ..heavy()
        },
        LenisSettings {
            quality: Quality::Precise,
            channels: Channels::Independent,
            ..heavy()
        },
    ] {
        // Music, then zeros until at rest: the same path for every twin.
        let to_rest = |u: &mut Lenis| -> usize {
            run(u, &music, 256);
            assert!(!u.at_rest());
            let mut zeros = 0usize;
            while !u.at_rest() {
                let (mut l, mut r) = (block.clone(), block.clone());
                u.process(&mut l, &mut r);
                assert!(l.iter().chain(&r).all(|v| v.abs() < 1.0));
                zeros += 64;
                assert!(
                    zeros < 40 * SR as usize,
                    "never at rest: {:?}",
                    u.rest_blocker()
                );
            }
            zeros
        };
        let mut a = unit(&s);
        let zeros = to_rest(&mut a);
        println!(
            "{:?} {:?} {:?}: at rest after {:.2} s of silence",
            s.quality,
            s.channels,
            s.mode,
            zeros as f32 / SR
        );
        let z = (vec![0.0f32; extra], vec![0.0f32; extra]);
        let y = run(&mut a, &z, 100);
        assert!(y.0.iter().chain(&y.1).all(|v| v.to_bits() == 0));
        let want = run(&mut a, &after, 100);
        // Skipped.
        let mut b = unit(&s);
        to_rest(&mut b);
        b.skip(extra);
        // Reset, then skipped.
        let mut c = unit(&s);
        to_rest(&mut c);
        c.reset();
        assert!(c.at_rest());
        c.skip(extra);
        // A new unit that heard only silence for as long.
        let mut d = unit(&s);
        let n = music.0.len() + zeros + extra;
        run(&mut d, &(vec![0.0f32; n], vec![0.0f32; n]), 512);
        for (name, u) in [("skipped", &mut b), ("reset", &mut c), ("new", &mut d)] {
            assert!(u.at_rest(), "{name}");
            let y = run(u, &after, 100);
            assert!(same(&want, &y), "{name} twin differs ({s:?})");
        }
    }
}

#[test]
fn the_meter_shows_the_resonance_where_it_is() {
    let x = harsh(48_000, 23);
    let mut u = unit(&LenisSettings {
        depth: 0.6,
        ..LenisSettings::default()
    });
    run(&mut u, &x, 256);
    let m = u.meter();
    assert!(m.frames > 300);
    let at = |hz: f32| {
        (0..DISPLAY)
            .min_by(|a, b| {
                (display_hz(*a) - hz)
                    .abs()
                    .total_cmp(&(display_hz(*b) - hz).abs())
            })
            .unwrap()
    };
    let peak = (0..DISPLAY)
        .max_by(|a, b| m.reduction_db[*a].total_cmp(&m.reduction_db[*b]))
        .unwrap();
    println!(
        "the most reduction at {:.0} Hz: {:.1} dB",
        display_hz(peak),
        m.reduction_db[peak]
    );
    assert!(m.reduction_db[peak] > 2.0);
    assert!(
        [at(3_150.0), at(5_200.0), at(900.0)]
            .iter()
            .any(|p| (*p as i32 - peak as i32).abs() <= 2),
        "the most at {:.0} Hz",
        display_hz(peak)
    );
    assert!(
        m.reduction_db[at(150.0)] < 0.5,
        "150 Hz: {}",
        m.reduction_db[at(150.0)]
    );
    assert!(m.level_db[at(1_000.0)] > -80.0);
    let most = m.reduction_db.iter().cloned().fold(0.0, f32::max);
    assert!(m.max_reduction_db >= most - 1e-3);
}

#[test]
fn the_focus_curve_is_one_where_nothing_acts_and_follows_its_bands() {
    let s = LenisSettings::default();
    assert_eq!(detect::focus_weight(&s, 1_000.0), 1.0);
    assert_eq!(detect::focus_weight(&s, 20.0), 1.0);
    assert_eq!(detect::focus_weight(&s, 0.0), 0.0);
    let h = heavy();
    assert_eq!(
        detect::focus_weight(&h, 40.0),
        0.0,
        "a third of an octave under 60 Hz"
    );
    assert!((detect::focus_weight(&h, 3_000.0) - 1.8).abs() < 1e-5);
    assert!((detect::focus_weight(&h, 200.0) - 0.5).abs() < 1e-5);
    assert_eq!(detect::focus_weight(&h, 1_000.0), 1.0);
    assert_eq!(detect::focus_weight(&h, 21_000.0), 0.0);
    assert!(detect::focus_weight(&h, 20_000.0) < 0.01);
    // The curve's shape.
    use detect::reduction_db as red;
    assert_eq!(red(3.0, 4.0, 0.5, Mode::Soft), 0.0, "under the threshold");
    assert_eq!(red(10.0, 4.0, 0.5, Mode::Hard), 6.0, "hard at 50 %: to the line");
    assert_eq!(red(10.0, 4.0, 1.0, Mode::Hard), 10.0, "never below the envelope");
    assert!((red(10.0, 4.0, 0.5, Mode::Soft) - 3.6).abs() < 1e-5, "soft: e²/(e + 4)");
    assert!(red(4.5, 4.0, 1.0, Mode::Soft) < 0.15, "the knee starts gently");
    assert_eq!(red(10.0, 4.0, 0.0, Mode::Hard), 0.0);
    assert_eq!(red(60.0, 2.0, 1.0, Mode::Hard), detect::MAX_CUT_DB);
}

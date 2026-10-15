use super::*;

const SR: f32 = 48_000.0;

/// Hits every quarter second: an instant rise and a fall over a tone, with a quiet pad
/// under them (so the body is never silent).
fn hits(n: usize, amp: f32) -> Vec<f32> {
    (0..n)
        .map(|i| {
            let t = i % 12_000;
            let env = math::exp(-(t as f32) / 1_800.0);
            amp * env * math::sin(i as f32 * 0.09)
                + 0.05 * math::sin(std::f32::consts::TAU * 220.0 * i as f32 / SR)
        })
        .collect()
}

fn noise(n: usize, seed: u32, amp: f32) -> Vec<f32> {
    let mut s = seed.wrapping_mul(2_654_435_761).max(1);
    (0..n)
        .map(|_| {
            s ^= s << 13;
            s ^= s >> 17;
            s ^= s << 5;
            amp * ((s as f32 / u32::MAX as f32) * 2.0 - 1.0)
        })
        .collect()
}

fn with(s: DiscrimenSettings) -> Discrimen {
    let mut u = Discrimen::new(SR);
    u.set(&s);
    u.reset();
    u
}

fn run(u: &mut Discrimen, l: &[f32], r: &[f32], block: usize) -> (Vec<f32>, Vec<f32>) {
    let (mut ol, mut or) = (l.to_vec(), r.to_vec());
    for (a, b) in ol.chunks_mut(block).zip(or.chunks_mut(block)) {
        u.process(a, b);
    }
    (ol, or)
}

/// The brief's null: the two parts, summed with no processing, are the input to the bit —
/// in both detection modes (the look-ahead's delayed by its latency), at any sensitivity.
#[test]
fn the_parts_sum_back_to_the_input_to_the_bit() {
    let x = hits(30_000, 0.9);
    let mut y = noise(30_000, 3, 0.7);
    y[100] = -0.0;
    y[101] = 0.0;
    for look in [false, true] {
        for sens in [0.0, 0.5, 1.0] {
            let s = DiscrimenSettings {
                look_ahead: look,
                sensitivity: sens,
                speed_ms: 4.0,
                ..DiscrimenSettings::default()
            };
            let mut u = with(s);
            let lat = u.latency();
            assert_eq!(lat, if look { latency_for(&s, SR) } else { 0 });
            let (ol, or) = run(&mut u, &x, &y, 333);
            for i in 0..x.len() {
                let (wl, wr) = if i >= lat {
                    (x[i - lat], y[i - lat])
                } else {
                    (0.0, 0.0)
                };
                assert_eq!(
                    ol[i].to_bits(),
                    wl.to_bits(),
                    "look {look} sens {sens}: L {i}"
                );
                assert_eq!(
                    or[i].to_bits(),
                    wr.to_bits(),
                    "look {look} sens {sens}: R {i}"
                );
            }
        }
    }
}

/// The split itself: `t + b == x` for any mask, every bit, through `split`.
#[test]
fn the_split_is_exact_for_any_mask() {
    let x = noise(20_000, 9, 3.0);
    for (i, &v) in x.iter().enumerate() {
        let m = (i % 101) as f32 / 100.0;
        let (t, b) = split(v, m);
        assert_eq!((t + b).to_bits(), v.to_bits(), "x {v} m {m}");
    }
    for v in [0.0f32, -0.0, 1e-38, -1e-38, f32::MIN_POSITIVE] {
        let (t, b) = split(v, 0.3);
        assert_eq!((t + b).to_bits(), v.to_bits());
    }
}

/// Listening to each part: the transient part holds the onsets, the body the rest.
#[test]
fn the_transient_part_holds_the_onsets_and_the_body_the_rest() {
    let x = hits(48_000, 0.8);
    let part = |listen: Listen, look: bool| {
        let s = DiscrimenSettings {
            listen,
            look_ahead: look,
            ..DiscrimenSettings::default()
        };
        let mut u = with(s);
        let lat = u.latency();
        let (ol, _) = run(&mut u, &x, &x, 256);
        ol[lat..].to_vec()
    };
    let energy = |v: &[f32]| v.iter().map(|a| f64::from(*a) * f64::from(*a)).sum::<f64>();
    for look in [false, true] {
        let t = part(Listen::Transient, look);
        let b = part(Listen::Body, look);
        let (mut on_t, mut on_x, mut tail_t, mut tail_x) = (0.0, 0.0, 0.0, 0.0);
        let (mut q_t, mut q_x) = (0.0, 0.0);
        for h in 1..4 {
            let s = h * 12_000;
            q_t += energy(&t[s..s + 12]);
            q_x += energy(&x[s..s + 12]);
            on_t += energy(&t[s..s + 96]);
            on_x += energy(&x[s..s + 96]);
            tail_t += energy(&t[s + 4_800..s + 9_600]);
            tail_x += energy(&x[s + 4_800..s + 9_600]);
        }
        let onset_share = on_t / on_x;
        let tail_share = tail_t / tail_x;
        eprintln!(
            "look-ahead {look}: the first 0.25 ms of each hit {:.0} % in the transient part, the first 2 ms {:.0} %, the tail {:.1} %",
            100.0 * q_t / q_x,
            100.0 * onset_share,
            100.0 * tail_share
        );
        assert!(
            onset_share > if look { 0.85 } else { 0.6 },
            "{look}: {onset_share}"
        );
        assert!(tail_share < 0.05, "{look}: {tail_share}");
        // And the body is what is left.
        assert!(energy(&b[12_000..12_096]) < energy(&x[12_000..12_096]));
    }
}

/// Each part's own gain, comp and tilt act on that part only.
#[test]
fn each_part_is_processed_on_its_own() {
    let x = hits(36_000, 0.8);
    let s = DiscrimenSettings {
        transient: Part {
            gain_db: 6.0,
            ..Part::default()
        },
        ..DiscrimenSettings::default()
    };
    let (louder, _) = run(&mut with(s), &x, &x, 256);
    let peak = |v: &[f32], a: usize, b: usize| v[a..b].iter().fold(0f32, |m, v| m.max(v.abs()));
    // The onset is louder; the late tail is not.
    assert!(peak(&louder, 12_000, 12_100) > 1.5 * peak(&x, 12_000, 12_100));
    let (a, b) = (peak(&louder, 19_000, 23_000), peak(&x, 19_000, 23_000));
    assert!((a / b - 1.0).abs() < 0.05, "{a} {b}");
    // Body compressed and tilted: something changes, nothing blows up.
    let s = DiscrimenSettings {
        body: Part {
            comp: 0.8,
            tilt_db: 4.0,
            ..Part::default()
        },
        ..DiscrimenSettings::default()
    };
    let (y, _) = run(&mut with(s), &x, &x, 256);
    assert!(y.iter().zip(&x).any(|(p, q)| (p - q).abs() > 1e-3));
    assert!(y.iter().all(|v| v.is_finite() && v.abs() < 4.0));
}

#[test]
fn the_reported_latency_is_where_an_impulse_comes_out() {
    for look in [false, true] {
        let s = DiscrimenSettings {
            look_ahead: look,
            ..DiscrimenSettings::default()
        };
        let mut u = with(s);
        let mut x = vec![0f32; 600];
        x[20] = 0.5;
        let (ol, _) = run(&mut u, &x, &x, 64);
        let at = ol.iter().position(|v| *v != 0.0).unwrap();
        assert_eq!(at, 20 + u.latency());
    }
    assert_eq!(latency_for(&DiscrimenSettings::default(), SR), 0);
}

#[test]
fn the_render_is_the_same_at_every_block_size() {
    let x = hits(30_000, 0.9);
    let y = noise(30_000, 4, 0.4);
    let s = DiscrimenSettings {
        look_ahead: true,
        sensitivity: 0.7,
        transient: Part {
            gain_db: 4.0,
            comp: 0.5,
            tilt_db: -3.0,
        },
        body: Part {
            gain_db: -2.0,
            comp: 0.3,
            tilt_db: 2.0,
        },
        out_db: -1.0,
        ..DiscrimenSettings::default()
    };
    let (rl, rr) = run(&mut with(s), &x, &y, 512);
    for block in [1usize, 7, 64, 1000] {
        let (ol, or) = run(&mut with(s), &x, &y, block);
        assert!(ol.iter().zip(&rl).all(|(a, b)| a.to_bits() == b.to_bits()));
        assert!(or.iter().zip(&rr).all(|(a, b)| a.to_bits() == b.to_bits()));
    }
}

#[test]
fn after_silence_it_rests_and_is_as_new() {
    let s = DiscrimenSettings {
        look_ahead: true,
        transient: Part {
            gain_db: 3.0,
            comp: 0.6,
            tilt_db: 2.0,
        },
        body: Part {
            gain_db: 0.0,
            comp: 0.4,
            tilt_db: -2.0,
        },
        ..DiscrimenSettings::default()
    };
    let mut u = with(s);
    let x = hits(9_000, 0.9);
    run(&mut u, &x, &x, 300);
    assert!(!u.at_rest());
    let z = vec![0f32; 64];
    let mut n = 0;
    while !u.at_rest() {
        let (ol, or) = run(&mut u, &z, &z, 64);
        n += 64;
        assert!(n < 30 * 48_000, "no rest");
        if u.at_rest() {
            assert!(ol.iter().chain(&or).all(|v| v.to_bits() == 0));
        }
    }
    u.skip(1_234);
    let mut fresh = with(s);
    fresh.skip(n + 1_234);
    let y = hits(6_000, 0.6);
    let (a, _) = run(&mut u, &y, &y, 100);
    let (b, _) = run(&mut fresh, &y, &y, 77);
    assert!(a.iter().zip(&b).all(|(p, q)| p.to_bits() == q.to_bits()));
}

/// A section that stops running leaves nothing behind: the comp and the tilt switched off
/// and on again start from new, so the unit still comes to rest.
#[test]
fn a_section_switched_off_is_emptied() {
    let busy = DiscrimenSettings {
        body: Part {
            gain_db: 0.0,
            comp: 0.7,
            tilt_db: 5.0,
        },
        ..DiscrimenSettings::default()
    };
    let mut u = with(busy);
    let x = hits(6_000, 0.9);
    run(&mut u, &x, &x, 256);
    u.set(&DiscrimenSettings::default());
    assert!(u.sections_clear(), "the body's comp and tilt were emptied");
}

/// Review 1: a NaN or an infinity passes through as itself but locks nothing — not the
/// detector, not a part's compressor or tilt — and the unit comes to rest.
#[test]
fn a_non_finite_sample_locks_nothing() {
    let s = DiscrimenSettings {
        look_ahead: true,
        transient: Part {
            gain_db: 3.0,
            comp: 0.5,
            tilt_db: 2.0,
        },
        body: Part {
            gain_db: -2.0,
            comp: 0.5,
            tilt_db: -2.0,
        },
        ..DiscrimenSettings::default()
    };
    let mut x = hits(24_000, 0.8);
    x[100] = f32::NAN;
    x[200] = f32::INFINITY;
    let mut u = with(s);
    let (y, _) = run(&mut u, &x, &x, 256);
    assert!(y[1_000..].iter().all(|v| v.is_finite()), "finite after the NaN");
    let (reference, _) = run(&mut with(s), &hits(24_000, 0.8), &hits(24_000, 0.8), 256);
    for i in 12_000..14_000 {
        assert!((y[i] - reference[i]).abs() < 1e-3, "sample {i}");
    }
    let z = vec![0f32; 64];
    let mut n = 0;
    while !u.at_rest() {
        run(&mut u, &z, &z, 64);
        n += 64;
        assert!(n < 30 * 48_000, "no rest after a NaN");
    }
}

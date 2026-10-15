use super::*;

const SR: f32 = 48_000.0;

/// Hits every quarter second: an instant rise, an exponential fall, over a tone.
fn hits(n: usize, amp: f32, hz: f32) -> Vec<f32> {
    (0..n)
        .map(|i| {
            let t = i % 12_000;
            let env = math::exp(-(t as f32) / 2_400.0);
            amp * env * math::sin(std::f32::consts::TAU * hz * i as f32 / SR)
        })
        .collect()
}

fn tone(n: usize, amp: f32, hz: f32) -> Vec<f32> {
    (0..n)
        .map(|i| amp * math::sin(std::f32::consts::TAU * hz * i as f32 / SR))
        .collect()
}

fn with(s: ImpetusSettings) -> Impetus {
    let mut u = Impetus::new(SR);
    u.set(&s);
    u.reset();
    u
}

fn run(u: &mut Impetus, x: &[f32], key: Option<&[f32]>, block: usize) -> Vec<f32> {
    let (mut l, mut r) = (x.to_vec(), x.to_vec());
    let mut at = 0;
    while at < x.len() {
        let e = (at + block).min(x.len());
        let k = key.map(|k| (&k[at..e], &k[at..e]));
        u.process(&mut l[at..e], &mut r[at..e], k);
        at = e;
    }
    l
}

fn energy(x: &[f32]) -> f64 {
    x.iter().map(|v| f64::from(*v) * f64::from(*v)).sum()
}

fn db(x: f64) -> f64 {
    10.0 * math::log10(x)
}

#[test]
fn at_zero_amounts_it_touches_no_sample_whatever_else_is_set() {
    let x = hits(24_000, 0.9, 220.0);
    let key = hits(24_000, 0.5, 90.0);
    for band in [Band::Full, Band::Highs, Band::Lows] {
        for keyed in [false, true] {
            let s = ImpetusSettings {
                band,
                speed_ms: 3.0,
                length_ms: 900.0,
                xover_hz: 1_200.0,
                ..ImpetusSettings::default()
            };
            let mut u = with(s);
            let y = run(&mut u, &x, keyed.then_some(&key[..]), 333);
            assert!(
                y.iter().zip(&x).all(|(a, b)| a.to_bits() == b.to_bits()),
                "{band:?} keyed {keyed}"
            );
        }
    }
    assert_eq!(with(ImpetusSettings::default()).latency(), 0);
}

/// Attack up makes the first milliseconds of every hit louder against the body; down makes
/// them softer.
#[test]
fn attack_shapes_the_onsets() {
    let x = hits(48_000, 0.5, 220.0);
    let ratio = |amount: f32| {
        let y = run(
            &mut with(ImpetusSettings {
                attack: amount,
                ..ImpetusSettings::default()
            }),
            &x,
            None,
            256,
        );
        // The first 5 ms of each hit against the 50 ms after.
        let (mut on, mut body) = (0.0, 0.0);
        for h in 1..4 {
            let s = h * 12_000;
            on += energy(&y[s..s + 240]) / energy(&x[s..s + 240]);
            body += energy(&y[s + 2_400..s + 4_800]) / energy(&x[s + 2_400..s + 4_800]);
        }
        db(on / body)
    };
    let up = ratio(1.0);
    let down = ratio(-1.0);
    eprintln!("attack +100 %: onset {up:.1} dB over body; -100 %: {down:.1} dB");
    assert!(up > 6.0, "{up}");
    assert!(down < -4.0, "{down}");
}

/// Sustain up lifts the tail; down shortens it.
#[test]
fn sustain_shapes_the_tails() {
    let x = hits(48_000, 0.5, 220.0);
    let tail = |amount: f32| {
        let y = run(
            &mut with(ImpetusSettings {
                sustain: amount,
                ..ImpetusSettings::default()
            }),
            &x,
            None,
            256,
        );
        let mut t = 0.0;
        for h in 1..4 {
            let s = h * 12_000;
            t += energy(&y[s + 4_800..s + 11_000]) / energy(&x[s + 4_800..s + 11_000]);
        }
        db(t / 3.0)
    };
    let up = tail(1.0);
    let down = tail(-1.0);
    eprintln!("sustain +100 %: tail {up:+.1} dB; -100 %: {down:+.1} dB");
    assert!(up > 3.0, "{up}");
    assert!(down < -3.0, "{down}");
}

/// No threshold: the same hits 12 dB quieter (a quarter) are shaped alike — the output is
/// a quarter of the loud one's, to the bit.
#[test]
fn quiet_and_loud_are_shaped_alike() {
    let x = hits(36_000, 0.8, 330.0);
    let quiet: Vec<f32> = x.iter().map(|v| v * 0.25).collect();
    let s = ImpetusSettings {
        attack: 0.7,
        sustain: -0.4,
        ..ImpetusSettings::default()
    };
    let a = run(&mut with(s), &x, None, 256);
    let b = run(&mut with(s), &quiet, None, 256);
    for (p, q) in a.iter().zip(&b) {
        assert_eq!((p * 0.25).to_bits(), q.to_bits());
    }
}

/// Split band: above a crossover only the highs are shaped — a bass under the hits is
/// left nearly alone, where the full band moves it with the hits.
#[test]
fn the_split_band_leaves_the_other_side_alone() {
    let n = 48_000;
    let hats = hits(n, 0.3, 6_000.0);
    let bass = tone(n, 0.4, 55.0);
    let x: Vec<f32> = hats.iter().zip(&bass).map(|(a, b)| a + b).collect();
    let bass_change = |band: Band| {
        let s = ImpetusSettings {
            attack: 1.0,
            band,
            xover_hz: 800.0,
            ..ImpetusSettings::default()
        };
        let y = run(&mut with(s), &x, None, 256);
        // How much the output's 55 Hz moved: the difference low-passed by a long average.
        let d: Vec<f32> = y.iter().zip(&x).map(|(a, b)| a - b).collect();
        let mut lp = 0f32;
        let lows: Vec<f32> = d
            .iter()
            .map(|v| {
                lp += (v - lp) * 0.01;
                lp
            })
            .collect();
        db(energy(&lows[4_800..]) / energy(&bass[4_800..]))
    };
    let full = bass_change(Band::Full);
    let highs = bass_change(Band::Highs);
    eprintln!("the bass moved {full:.1} dB (full band), {highs:.1} dB (highs only)");
    assert!(highs < full - 15.0, "full {full}, highs {highs}");
}

/// A key: a steady tone is shaped by another signal's hits.
#[test]
fn a_key_drives_the_detector() {
    let x = tone(36_000, 0.3, 440.0);
    let key = hits(36_000, 0.8, 100.0);
    let s = ImpetusSettings {
        attack: 1.0,
        ..ImpetusSettings::default()
    };
    let own = run(&mut with(s), &x, None, 256);
    let keyed = run(&mut with(s), &x, Some(&key), 256);
    let moved = |y: &[f32]| {
        y[12_000..12_240]
            .iter()
            .zip(&x[12_000..12_240])
            .fold(0f32, |m, (a, b)| m.max((a - b).abs()))
    };
    assert!(moved(&own) < 0.01, "a steady tone has no transient");
    assert!(moved(&keyed) > 0.1, "the key's hit is heard");
}

#[test]
fn the_render_is_the_same_at_every_block_size() {
    let x = hits(30_000, 0.7, 180.0);
    let key = hits(30_000, 0.6, 70.0);
    for (band, keyed) in [
        (Band::Full, false),
        (Band::Highs, true),
        (Band::Lows, false),
    ] {
        let s = ImpetusSettings {
            attack: 0.8,
            sustain: -0.6,
            band,
            out_db: -1.0,
            ..ImpetusSettings::default()
        };
        let k = keyed.then_some(&key[..]);
        let reference = run(&mut with(s), &x, k, 512);
        for block in [1usize, 7, 64, 1000] {
            let y = run(&mut with(s), &x, k, block);
            assert!(y
                .iter()
                .zip(&reference)
                .all(|(a, b)| a.to_bits() == b.to_bits()));
        }
    }
}

#[test]
fn after_silence_it_rests_and_is_as_new() {
    let s = ImpetusSettings {
        attack: 0.6,
        sustain: 0.5,
        band: Band::Highs,
        length_ms: 1_000.0,
        ..ImpetusSettings::default()
    };
    let mut u = with(s);
    run(&mut u, &hits(9_000, 0.9, 300.0), None, 300);
    assert!(!u.at_rest());
    let z = vec![0f32; 64];
    let mut n = 0;
    while !u.at_rest() {
        let y = run(&mut u, &z, None, 64);
        n += 64;
        assert!(n < 30 * 48_000, "no rest");
        if u.at_rest() {
            assert!(y.iter().all(|v| v.to_bits() == 0));
        }
    }
    u.skip(777);
    let mut fresh = with(s);
    fresh.skip(n + 777);
    let x = hits(6_000, 0.5, 410.0);
    let a = run(&mut u, &x, None, 100);
    let b = run(&mut fresh, &x, None, 77);
    assert!(a.iter().zip(&b).all(|(p, q)| p.to_bits() == q.to_bits()));
}

/// The editor's history: one column per 512 samples, the input's peak and the gain's
/// extremes in it.
#[test]
fn the_telemetry_carries_columns_of_input_and_gain() {
    // Just after the second hit, so the held boost is that hit's.
    let s = ImpetusSettings {
        attack: 1.0,
        ..ImpetusSettings::default()
    };
    let mut u = with(s);
    run(&mut u, &hits(13_000, 0.8, 300.0), None, 500);
    let mut t = [0f32; 64];
    u.telemetry(&mut t);
    assert_eq!(t[crate::dynfx_meter::COL_COUNT], (13_000 / 512) as f32);
    let mut cols = Vec::new();
    crate::dynfx_meter::new_columns(&t, None, |c| cols.push(c));
    assert_eq!(cols.len(), crate::dynfx_meter::COLS);
    assert!(
        t[tel::GAIN_MAX] > 6.0,
        "the boost held: {}",
        t[tel::GAIN_MAX]
    );
}

/// Review 1: a NaN or an infinity in the input passes through as itself but locks nothing:
/// the samples after it are finite and shaped as before, and the unit comes to rest.
#[test]
fn a_non_finite_sample_locks_nothing() {
    let s = ImpetusSettings {
        attack: 1.0,
        sustain: 0.5,
        band: Band::Highs,
        ..ImpetusSettings::default()
    };
    let mut x = hits(24_000, 0.6, 300.0);
    x[100] = f32::NAN;
    x[200] = f32::INFINITY;
    let mut u = with(s);
    let y = run(&mut u, &x, None, 256);
    assert!(
        y[1_000..].iter().all(|v| v.is_finite()),
        "finite after the NaN"
    );
    let reference = run(&mut with(s), &hits(24_000, 0.6, 300.0), None, 256);
    // Once the detector has forgotten the hit before, the same as without the NaN.
    for i in 12_000..14_000 {
        assert!((y[i] - reference[i]).abs() < 1e-3, "sample {i}");
    }
    let z = vec![0f32; 64];
    let mut n = 0;
    while !u.at_rest() {
        run(&mut u, &z, None, 64);
        n += 64;
        assert!(n < 30 * 48_000, "no rest after a NaN");
    }
}

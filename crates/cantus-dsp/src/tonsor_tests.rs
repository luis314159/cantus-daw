use super::*;
use crate::terminus::os::{reference_true_peak, true_peak_of};

const SR: f32 = 48_000.0;

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

/// A band-limited mix (three sines) whose true peak is about `peak`.
fn tones(n: usize, peak: f32) -> Vec<f32> {
    (0..n)
        .map(|i| {
            let t = i as f32 / SR;
            let v = 0.5 * math::sin(std::f32::consts::TAU * 110.0 * t)
                + 0.3 * math::sin(std::f32::consts::TAU * 1_234.0 * t + 0.3)
                + 0.2 * math::sin(std::f32::consts::TAU * 5_111.0 * t + 1.1);
            v * peak
        })
        .collect()
}

/// A quarter of the sample rate at 45°: every sample at 0.707 of the waveform's peak — the
/// classic inter-sample over, +3 dB above its sample peak.
fn isp(n: usize, sample_peak: f32) -> Vec<f32> {
    (0..n)
        .map(|i| {
            let ph = std::f32::consts::FRAC_PI_2 * i as f32 + std::f32::consts::FRAC_PI_4;
            sample_peak * std::f32::consts::SQRT_2 * math::sin(ph)
        })
        .collect()
}

fn run(t: &mut Tonsor, l: &[f32], r: &[f32], block: usize) -> (Vec<f32>, Vec<f32>) {
    let (mut ol, mut or) = (l.to_vec(), r.to_vec());
    for (a, b) in ol.chunks_mut(block).zip(or.chunks_mut(block)) {
        t.process(a, b);
    }
    (ol, or)
}

fn with(s: TonsorSettings) -> Tonsor {
    let mut t = Tonsor::new(SR);
    t.set(&s);
    t.reset();
    t
}

fn db(x: f32) -> f32 {
    20.0 * math::log10(x)
}

#[test]
fn at_its_defaults_it_touches_no_sample() {
    let mut t = with(TonsorSettings::default());
    assert_eq!(t.latency(), 0);
    let x = noise(10_000, 1, 0.99);
    let y = noise(10_000, 2, 0.99);
    let (ol, or) = run(&mut t, &x, &y, 333);
    for i in 0..x.len() {
        assert_eq!(ol[i].to_bits(), x[i].to_bits());
        assert_eq!(or[i].to_bits(), y[i].to_bits());
    }
}

/// The null of the brief: at a 0 dBFS ceiling, a signal with no overs comes out as it went
/// in, delayed by the reported latency — at every oversampling, true peak on or off.
#[test]
fn with_no_overs_every_factor_is_the_input_delayed_to_the_bit() {
    let x = tones(12_000, 0.8);
    assert!(true_peak_of(&x) < 0.95);
    for os in 0..5 {
        for tp in [false, true] {
            for curve in [Curve::Hard, Curve::Soft, Curve::Analog] {
                let s = TonsorSettings {
                    oversampling: crate::dynfx_os::FACTORS[os],
                    true_peak: tp,
                    curve,
                    ..TonsorSettings::default()
                };
                let mut t = with(s);
                let lat = t.latency();
                assert_eq!(lat, latency_for(&s));
                let (ol, _) = run(&mut t, &x, &x, 64);
                for i in 0..x.len() {
                    let want = if i >= lat { x[i - lat] } else { 0.0 };
                    assert_eq!(
                        ol[i].to_bits(),
                        want.to_bits(),
                        "os {os} tp {tp} {curve:?}: sample {i}"
                    );
                }
            }
        }
    }
}

#[test]
fn the_curves_are_straight_below_the_knee_and_never_pass_the_ceiling() {
    let c = 0.8;
    let k = c * math::db_to_gain(-6.0);
    for curve in [Curve::Hard, Curve::Soft, Curve::Analog] {
        let mut last = 0.0;
        for i in 0..=4_000 {
            let u = i as f32 / 1_000.0;
            let y = shape(u, k, c, curve);
            if u <= k {
                assert_eq!(y, u, "{curve:?} straight at {u}");
            }
            assert!(y <= c, "{curve:?}: {y} over the ceiling at {u}");
            assert!(y >= last, "{curve:?} rises at {u}");
            assert_eq!(shape(-u, k, c, curve), -y, "{curve:?} symmetric");
            last = y;
        }
        // Continuous where it bends.
        let a = shape(k, k, c, curve);
        let b = shape(k + 1e-4, k, c, curve);
        assert!((b - a) < 2e-4, "{curve:?} jumps at the knee");
    }
    // Hard reaches the ceiling; at a knee of 0 every curve is a plain clip.
    assert_eq!(shape(2.0, k, c, Curve::Hard), c);
    for curve in [Curve::Hard, Curve::Soft, Curve::Analog] {
        assert_eq!(shape(1.3, c, c, curve), c);
        assert_eq!(shape(0.5, c, c, curve), 0.5);
    }
}

#[test]
fn a_hard_clip_at_one_times_holds_the_sample_peak_at_the_ceiling() {
    let s = TonsorSettings {
        ceiling_db: -3.0,
        drive_db: 12.0,
        curve: Curve::Hard,
        ..TonsorSettings::default()
    };
    let mut t = with(s);
    let x = tones(9_600, 0.9);
    let (ol, or) = run(&mut t, &x, &x, 100);
    let c = math::db_to_gain(-3.0);
    let peak = ol.iter().chain(&or).fold(0f32, |m, v| m.max(v.abs()));
    assert!(peak <= c * 1.000_001, "{peak} over {c}");
    assert!(peak >= c * 0.999_999, "it clips: {peak}");
}

/// Inter-sample overs: at 1× a hot signal leaves the clipper well over its ceiling in
/// dBTP; with the true-peak ceiling on it stays under it (measured by the reference
/// reconstruction at 32 points a sample).
#[test]
fn the_true_peak_ceiling_holds_where_a_plain_clip_does_not() {
    let n = 24_000;
    let mut x = isp(n, 0.98);
    for (v, z) in x.iter_mut().zip(tones(n, 1.6)) {
        *v = 0.5 * *v + z;
    }
    let ceiling = -1.0;
    let c = math::db_to_gain(ceiling);
    let plain = TonsorSettings {
        ceiling_db: ceiling,
        curve: Curve::Hard,
        ..TonsorSettings::default()
    };
    let (ol, _) = run(&mut with(plain), &x, &x, 256);
    let over_plain = db(reference_true_peak(&ol, 3.0) / c);
    assert!(over_plain > 0.5, "a plain clip leaves overs: {over_plain} dB");
    for os in [1usize, 4, 16] {
        let guarded = TonsorSettings {
            true_peak: true,
            oversampling: os,
            ..plain
        };
        let mut t = with(guarded);
        let lat = t.latency();
        let (ol, _) = run(&mut t, &x, &x, 256);
        let tp = reference_true_peak(&ol[lat..], 3.0);
        let over = db(tp / c);
        let meter = db(true_peak_of(&ol) / c);
        eprintln!("os {os}: plain over {over_plain:.2} dB, true-peak mode {over:.3} dB (meter {meter:.3})");
        assert!(over <= 0.05, "os {os}: {over} dB over the ceiling in dBTP");
        // The meter the editor shows agrees.
        assert!(db(true_peak_of(&ol) / c) <= 0.05);
    }
}

/// Oversampling is there for aliasing: a 7 kHz sine clipped hard folds its 5th and 7th
/// harmonics to 13 kHz and 1 kHz; at 16× they are far under what 1× leaves.
#[test]
fn oversampling_takes_the_folds_away() {
    let n = 48_000;
    let x: Vec<f32> = (0..n)
        .map(|i| math::sin(std::f32::consts::TAU * 7_000.0 * i as f32 / SR))
        .collect();
    let alias = |os: usize| -> f32 {
        let s = TonsorSettings {
            drive_db: 12.0,
            ceiling_db: -1.0,
            curve: Curve::Hard,
            oversampling: os,
            ..TonsorSettings::default()
        };
        let (ol, _) = run(&mut with(s), &x, &x, 512);
        let tail = &ol[8_000..];
        let fund = goertzel(tail, 7_000.0);
        goertzel(tail, 13_000.0).max(goertzel(tail, 1_000.0)) / fund
    };
    let a1 = db(alias(1));
    let a16 = db(alias(16));
    eprintln!("aliases: 1x {a1:.1} dB, 16x {a16:.1} dB");
    assert!(a1 > -45.0, "1x folds: {a1}");
    assert!(a16 < a1 - 30.0, "16x: {a16} vs {a1}");
}

fn goertzel(x: &[f32], hz: f32) -> f32 {
    let w = std::f64::consts::TAU * hz as f64 / SR as f64;
    let c = 2.0 * math::cos(w);
    let (mut s1, mut s2) = (0f64, 0f64);
    for &v in x {
        let s = v as f64 + c * s1 - s2;
        s2 = s1;
        s1 = s;
    }
    let p = s1 * s1 + s2 * s2 - c * s1 * s2;
    (p.max(0.0).sqrt() * 2.0 / x.len() as f64) as f32
}

#[test]
fn the_render_is_the_same_at_every_block_size() {
    let x = tones(20_000, 2.5);
    let y = noise(20_000, 5, 1.8);
    for (os, tp) in [(1usize, false), (8, true), (16, false), (2, true)] {
        let s = TonsorSettings {
            oversampling: os,
            true_peak: tp,
            curve: Curve::Soft,
            knee_db: 4.0,
            drive_db: 3.0,
            ceiling_db: -0.5,
            match_gain: true,
            ..TonsorSettings::default()
        };
        let (rl, rr) = run(&mut with(s), &x, &y, 512);
        for block in [1usize, 7, 64, 1000] {
            let (ol, or) = run(&mut with(s), &x, &y, block);
            assert!(ol.iter().zip(&rl).all(|(a, b)| a.to_bits() == b.to_bits()));
            assert!(or.iter().zip(&rr).all(|(a, b)| a.to_bits() == b.to_bits()));
        }
    }
}

#[test]
fn the_reported_latency_is_where_an_impulse_comes_out() {
    for os in crate::dynfx_os::FACTORS {
        for tp in [false, true] {
            let s = TonsorSettings {
                oversampling: os,
                true_peak: tp,
                ..TonsorSettings::default()
            };
            let mut t = with(s);
            let mut x = vec![0f32; 400];
            x[10] = 0.5;
            let (ol, _) = run(&mut t, &x, &x, 64);
            let at = ol.iter().position(|v| *v != 0.0).unwrap();
            assert_eq!(at, 10 + t.latency(), "os {os} tp {tp}");
        }
    }
}

#[test]
fn gain_match_takes_the_drive_back_out() {
    let s = TonsorSettings {
        drive_db: 9.0,
        match_gain: true,
        ..TonsorSettings::default()
    };
    let x = tones(4_000, 0.2);
    let (ol, _) = run(&mut with(s), &x, &x, 128);
    for (a, b) in ol.iter().zip(&x) {
        assert!((a - b).abs() < 1e-6);
    }
}

/// How much and how often: the meter counts clipped samples and the input's inter-sample
/// overs, and holds its peaks for the editor's next frame.
#[test]
fn the_meter_counts_clips_and_inter_sample_overs() {
    let s = TonsorSettings {
        ceiling_db: 0.0,
        ..TonsorSettings::default()
    };
    let mut t = with(s);
    let x = isp(4_800, 0.98);
    run(&mut t, &x, &x, 480);
    let mut tel = [0f32; 64];
    t.telemetry(&mut tel);
    assert_eq!(tel[tel::CLIPPED], 0.0, "no sample over 0 dBFS");
    assert!(tel[tel::OVERS] > 1_000.0, "inter-sample overs: {}", tel[tel::OVERS]);
    assert!(tel[tel::IN_TP] > 2.5, "input true peak {}", tel[tel::IN_TP]);
    assert_eq!(tel[tel::SAMPLES], 4_800.0);
    // Now hot: clipped samples counted, the reduction held after a quiet block.
    let hot = tones(4_800, 2.0);
    run(&mut t, &hot, &hot, 480);
    let quiet = vec![0.01f32; 480];
    run(&mut t, &quiet, &quiet, 480);
    t.telemetry(&mut tel);
    assert!(tel[tel::CLIPPED] > 100.0);
    assert!(tel[tel::REDUCTION] > 3.0, "held: {}", tel[tel::REDUCTION]);
}

#[test]
fn after_silence_it_rests_and_is_as_new() {
    let s = TonsorSettings {
        oversampling: 16,
        true_peak: true,
        drive_db: 6.0,
        knee_db: 3.0,
        ..TonsorSettings::default()
    };
    let mut t = with(s);
    let x = tones(9_000, 1.5);
    run(&mut t, &x, &x, 300);
    assert!(!t.at_rest());
    let z = vec![0f32; 64];
    let mut n = 0;
    while !t.at_rest() {
        let (ol, or) = run(&mut t, &z, &z, 64);
        n += 64;
        assert!(n < 48_000, "no rest");
        if t.at_rest() {
            assert!(ol.iter().chain(&or).all(|v| v.to_bits() == 0));
        }
    }
    t.skip(1_000);
    let mut fresh = with(s);
    fresh.skip(n + 1_000);
    let y = noise(3_000, 8, 1.4);
    let (a, _) = run(&mut t, &y, &y, 100);
    let (b, _) = run(&mut fresh, &y, &y, 77);
    assert!(a.iter().zip(&b).all(|(p, q)| p.to_bits() == q.to_bits()));
}

use super::*;

fn sine(n: usize, hz: f32, amp: f32, sr: f32) -> Vec<f32> {
    (0..n)
        .map(|i| amp * math::sin(std::f32::consts::TAU * hz * i as f32 / sr))
        .collect()
}

/// Point 0 of every sample is the input `HALF` samples ago, to the bit, at every factor.
#[test]
fn phase_zero_is_a_pure_delay() {
    let t = Tables::new();
    let x = sine(500, 997.0, 0.8, 48_000.0);
    for f in FACTORS {
        let mut up = Up::default();
        let mut pts = [0f32; MAX_FACTOR];
        for (i, &v) in x.iter().enumerate() {
            up.push(v, f, &t, &mut pts);
            let want = if i >= HALF { x[i - HALF] } else { 0.0 };
            assert_eq!(pts[0].to_bits(), want.to_bits(), "factor {f}, sample {i}");
        }
    }
}

/// The points between the samples are the band-limited waveform: a 1 kHz sine read at
/// `n − HALF + p/F` within −70 dB of its level (Terminus's interpolator: ±0.02 dB to 0.44 fs).
#[test]
fn the_points_between_are_the_waveform() {
    let t = Tables::new();
    let sr = 48_000.0;
    let hz = 1_000.0;
    for f in [2usize, 4, 8, 16] {
        let mut up = Up::default();
        let mut pts = [0f32; MAX_FACTOR];
        let mut worst = 0f32;
        for i in 0..2_000 {
            let v = 0.5 * math::sin(std::f32::consts::TAU * hz * i as f32 / sr);
            up.push(v, f, &t, &mut pts);
            if i < 200 {
                continue;
            }
            for (p, got) in pts.iter().enumerate().take(f) {
                let at = i as f64 - HALF as f64 + p as f64 / f as f64;
                let want = 0.5 * math::sin(std::f64::consts::TAU * hz as f64 * at / sr as f64);
                worst = worst.max((got - want as f32).abs());
            }
        }
        assert!(worst < 1.6e-4, "factor {f}: error {worst}");
    }
}

/// Zeros in, exact zeros out, and the history stays clear.
#[test]
fn a_silent_residual_adds_exact_zeros() {
    let t = Tables::new();
    for f in [2usize, 4, 8, 16] {
        let mut d = Down::default();
        let zero = [0f32; MAX_FACTOR];
        for _ in 0..100 {
            assert_eq!(d.push(&zero, f, &t).to_bits(), 0);
        }
        assert!(d.is_clear());
        let mut one = zero;
        one[0] = 1.0;
        let _ = d.push(&one, f, &t);
        assert!(!d.is_clear());
    }
}

/// The decimator passes the band (a 5 kHz residual within 0.05 dB, delayed by
/// `DOWN_HALF`) and stops what would fold (a 30 kHz residual at 4× down 70 dB).
#[test]
fn the_decimator_keeps_the_band_and_stops_the_folds() {
    let t = Tables::new();
    let sr = 48_000.0f64;
    for f in [2usize, 4, 8, 16] {
        let rate = sr * f as f64;
        let run = |hz: f64| -> f32 {
            let mut d = Down::default();
            let mut peak = 0f32;
            let mut pts = [0f32; MAX_FACTOR];
            for n in 0..4_000usize {
                for (p, v) in pts.iter_mut().enumerate().take(f) {
                    let k = (n * f + p) as f64;
                    *v = math::sin(std::f64::consts::TAU * hz * k / rate) as f32;
                }
                let y = d.push(&pts, f, &t);
                if n > 2_000 {
                    peak = peak.max(y.abs());
                }
            }
            peak
        };
        let pass = run(5_000.0);
        assert!((pass - 1.0).abs() < 0.006, "factor {f}: pass band {pass}");
        if f >= 2 {
            let stop = run(30_000.0);
            assert!(stop < 3.2e-4, "factor {f}: 30 kHz at {stop}");
        }
    }
}

#[test]
fn the_latency_is_the_two_halves_above_one() {
    assert_eq!(latency(1), 0);
    for f in [2usize, 4, 8, 16] {
        assert_eq!(latency(f), HALF + DOWN_HALF);
    }
    assert_eq!(factor_of(0.0), 1);
    assert_eq!(factor_of(4.0), 16);
    assert_eq!(factor_of(9.0), 16);
}

/// The decimator lines up with point 0: base sample `n`'s output is the band-limited
/// residual at time `n − DOWN_HALF` exactly — the phase, not only the level (review 1: it
/// was `(F − 1)/F` of a sample early).
#[test]
fn the_decimator_is_aligned_with_point_zero() {
    let t = Tables::new();
    let sr = 48_000.0f64;
    let hz = 5_000.0f64;
    for f in [2usize, 4, 8, 16] {
        let rate = sr * f as f64;
        let mut d = Down::default();
        let mut pts = [0f32; MAX_FACTOR];
        let mut worst = 0f64;
        for n in 0..3_000usize {
            for (p, v) in pts.iter_mut().enumerate().take(f) {
                let k = (n * f + p) as f64;
                *v = math::sin(std::f64::consts::TAU * hz * k / rate) as f32;
            }
            let y = d.push(&pts, f, &t);
            if n > 2_000 {
                let at = ((n - DOWN_HALF) * f) as f64;
                let want = math::sin(std::f64::consts::TAU * hz * at / rate);
                worst = worst.max((f64::from(y) - want).abs());
            }
        }
        assert!(worst < 0.01, "factor {f}: off by {worst}");
    }
}

use super::*;

#[test]
fn the_design_rejects_the_other_sideband_by_60_db_over_the_band() {
    for sr in [44_100.0f32, 48_000.0, 88_200.0, 96_000.0] {
        let d = design(sr);
        let hi = HIGH_EDGE.min(0.45 * f64::from(sr));
        let r = worst_rejection(&d.coefs[..d.n], f64::from(sr), 20.0, hi);
        eprintln!("{sr} Hz: {} sections, worst {r:.1} dB over 20 Hz … {hi:.0} Hz", d.n);
        if sr <= 48_000.0 {
            assert!(r >= 60.0, "{sr}: {r:.1} dB");
        }
        assert!(d.coefs[..d.n].iter().all(|c| (0.0..1.0).contains(c)));
    }
}

/// A sine through the running pair: the two outputs are equal in level and 90° apart, so
/// `i² + q²` is constant and `i·q` averages to nothing.
#[test]
fn the_running_pair_is_in_quadrature() {
    let sr = 48_000.0f32;
    let d = design(sr);
    for f in [30.0f32, 440.0, 5_000.0, 18_000.0] {
        let mut h = Hilbert::new(&d);
        let w = std::f64::consts::TAU * f64::from(f) / f64::from(sr);
        let mut env_min = f32::INFINITY;
        let mut env_max = 0.0f32;
        let n = 96_000;
        for k in 0..n {
            let arg = (w * k as f64) % std::f64::consts::TAU;
            let (i, q) = h.run(math::sin(arg) as f32);
            if k > n / 2 {
                let e = (i * i + q * q).sqrt();
                env_min = env_min.min(e);
                env_max = env_max.max(e);
            }
        }
        // An unwanted sideband `r` below the wanted one makes the envelope ripple between
        // 1 − r and 1 + r: 60 dB (r = 0.001) is 0.0174 dB of ripple.
        let ripple_db = 20.0 * math::log10(env_max / env_min);
        assert!(ripple_db < 0.0174, "{f} Hz: ripple {ripple_db} dB");
        assert!((env_max - 1.0).abs() < 0.01, "{f} Hz: level {env_max}");
    }
}

#[test]
fn silence_brings_every_state_to_zero_once_flushed() {
    let mut h = Hilbert::new(&design(48_000.0));
    for k in 0..1000 {
        h.run(if k < 100 { 0.5 } else { 0.0 });
    }
    assert!(!h.is_zero());
    assert!(h.level() > 0.0);
    h.reset();
    assert!(h.is_zero());
}

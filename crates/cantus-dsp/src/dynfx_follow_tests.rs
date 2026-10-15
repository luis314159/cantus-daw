use super::*;

const SR: f32 = 48_000.0;

/// A deterministic noise in −1 … 1.
fn noise(n: usize, seed: u32) -> Vec<f32> {
    let mut s = seed.wrapping_mul(2_654_435_761).max(1);
    (0..n)
        .map(|_| {
            s ^= s << 13;
            s ^= s >> 17;
            s ^= s << 5;
            (s as f32 / u32::MAX as f32) * 2.0 - 1.0
        })
        .collect()
}

#[test]
fn a_step_covers_63_percent_in_one_attack_time_and_falls_in_one_release_time() {
    let mut f = EnvelopeFollower::new(SR);
    f.set(Detect::Peak, 5.0, 50.0);
    let up = (0.005 * SR) as usize;
    let mut v = 0.0;
    for _ in 0..up {
        v = f.run(1.0);
    }
    assert!(
        (v - (1.0 - crate::math::exp(-1.0f32))).abs() < 0.01,
        "attack: {v}"
    );
    // Settle at the top, then fall.
    for _ in 0..48_000 {
        f.run(1.0);
    }
    let down = (0.05 * SR) as usize;
    for _ in 0..down {
        v = f.run(0.0);
    }
    assert!((v - crate::math::exp(-1.0f32)).abs() < 0.01, "release: {v}");
}

#[test]
fn zero_milliseconds_follows_at_once() {
    let mut f = EnvelopeFollower::new(SR);
    f.set(Detect::Peak, 0.0, 0.0);
    for x in [0.3f32, -0.7, 0.1, 0.0, 0.9] {
        assert_eq!(f.run(x), x.abs());
    }
}

/// The envelope of `4·x` is four times the envelope of `x`, to the bit: a detector that
/// compares two envelopes reads the same at any level.
#[test]
fn the_envelope_scales_with_the_signal_to_the_bit() {
    let x = noise(20_000, 7);
    for detect in [Detect::Peak, Detect::Rms] {
        let mut a = EnvelopeFollower::new(SR);
        let mut b = EnvelopeFollower::new(SR);
        a.set(detect, 2.0, 80.0);
        b.set(detect, 2.0, 80.0);
        for &v in &x {
            let ea = a.run(v * 0.01);
            let eb = b.run(v * 0.04);
            assert_eq!((ea * 4.0).to_bits(), eb.to_bits(), "{detect:?}");
        }
    }
}

#[test]
fn rms_of_a_sine_reads_its_rms() {
    let mut f = EnvelopeFollower::new(SR);
    f.set(Detect::Rms, 300.0, 300.0);
    let mut v = 0.0;
    for i in 0..(4.0 * SR) as usize {
        let x = 0.5 * crate::math::sin(std::f32::consts::TAU * 1_000.0 * i as f32 / SR);
        v = f.run(x);
    }
    assert!((v - 0.5 / 2f32.sqrt()).abs() < 0.01, "{v}");
    assert!((f.db() - to_db(0.5 / 2f32.sqrt())).abs() < 0.2);
}

#[test]
fn a_pair_reads_its_louder_side() {
    let mut a = EnvelopeFollower::new(SR);
    let mut b = EnvelopeFollower::new(SR);
    a.set(Detect::Peak, 1.0, 10.0);
    b.set(Detect::Peak, 1.0, 10.0);
    for (l, r) in [(0.1f32, -0.5f32), (0.8, 0.2), (-0.3, 0.3)] {
        assert_eq!(a.run_pair(l, r), b.run(l.abs().max(r.abs())));
    }
}

/// After a burst and silence the follower comes to rest — +0.0, as new — and from there
/// renders what a new one renders.
#[test]
fn after_silence_it_is_exactly_a_new_follower() {
    let mut f = EnvelopeFollower::new(SR);
    f.set(Detect::Peak, 1.0, 200.0);
    for &v in &noise(4_800, 3) {
        f.run(v);
    }
    assert!(!f.at_rest());
    let mut n = 0;
    while !f.at_rest() {
        f.run(0.0);
        n += 1;
        assert!(n < 30 * 48_000, "no rest in 30 s");
    }
    // −120 dB from about 0 dB at 200 ms: some 2.8 s.
    assert!(n < 4 * 48_000, "rest after {n} samples");
    let mut fresh = EnvelopeFollower::new(SR);
    fresh.set(Detect::Peak, 1.0, 200.0);
    for &v in &noise(2_000, 9) {
        assert_eq!(f.run(v).to_bits(), fresh.run(v).to_bits());
    }
}

#[test]
fn decibels_of_silence_are_a_floor() {
    assert_eq!(to_db(0.0), -200.0);
    assert!((to_db(1.0)).abs() < 1e-6);
    assert!((to_db(0.5) + 6.0206).abs() < 1e-3);
}

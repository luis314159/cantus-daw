use super::*;

const SR: f32 = 48_000.0;

fn noise(n: usize, seed: u32) -> Vec<f32> {
    let mut s = seed | 1;
    (0..n)
        .map(|_| {
            s ^= s << 13;
            s ^= s >> 17;
            s ^= s << 5;
            (s as f32 / u32::MAX as f32 * 2.0 - 1.0) * 0.3
        })
        .collect()
}

fn same(a: &[f32], b: &[f32]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits())
}

fn unit(s: &FlexusSettings) -> Flexus {
    let mut f = Flexus::new(SR);
    f.set(s);
    f.reset();
    f
}

fn drive(f: &mut Flexus, l: &mut [f32], r: &mut [f32], mut split: impl FnMut() -> usize) {
    let n = l.len();
    let mut at = 0;
    while at < n {
        let b = split().clamp(1, n - at);
        f.set_transport(&Transport {
            playing: true,
            tempo_bpm: 128.0,
            samples: at as i64,
        });
        f.process(&mut l[at..at + b], &mut r[at..at + b]);
        at += b;
    }
}

fn busy(mode: Mode, tz: bool) -> FlexusSettings {
    FlexusSettings {
        mode,
        through_zero: tz,
        mix: 0.5,
        delay_ms: 0.3,
        depth_ms: 6.0,
        feedback: 0.7,
        invert: true,
        lfo: LfoSettings {
            rate: Rate::Beats(1.0),
            shape: Shape::Sine,
            ..LfoSettings::default()
        },
        spread: 0.25,
        manual: 0.3,
        attack_ms: 2.0,
        release_ms: 120.0,
        sens_db: 18.0,
        out_db: -1.0,
    }
}

/// At mix 0 the plain flanger is a wire, to the bit, with no latency; through zero it is
/// the input delayed by the centre, exactly the latency it reports. The defaults are a wire.
#[test]
fn at_mix_zero_it_is_the_input_delayed_by_its_latency() {
    let x = noise(20_000, 1);
    let delayed = |l: &[f32], lat: usize| {
        l[..lat].iter().all(|v| *v == 0.0) && same(&l[lat..], &x[..x.len() - lat])
    };
    for mode in [Mode::Lfo, Mode::Envelope, Mode::Manual] {
        for tz in [false, true] {
            let mut s = busy(mode, tz);
            s.mix = 0.0;
            s.out_db = 0.0;
            let mut u = unit(&s);
            let lat = if tz { 240 } else { 0 };
            assert_eq!(u.latency(), lat, "{mode:?} tz {tz}");
            let (mut l, mut r) = (x.clone(), x.clone());
            drive(&mut u, &mut l, &mut r, || 333);
            assert!(delayed(&l, lat) && delayed(&r, lat), "{mode:?} tz {tz}");
        }
    }
    let mut u = unit(&FlexusSettings::default());
    assert_eq!(u.latency(), 0, "the defaults");
    let (mut l, mut r) = (x.clone(), x.clone());
    u.process(&mut l, &mut r);
    assert!(same(&l, &x) && same(&r, &x), "the defaults are a wire");
}

/// The through-zero switch (a parameter a lane may automate) moves the latency, and the
/// engine re-plans compensation when it does. The dry line runs in every mode, so a switch
/// on mid-stream is at once the input delayed by the centre — no gap of silence.
#[test]
fn the_latency_follows_the_through_zero_switch() {
    let mut s = FlexusSettings::default();
    let mut u = unit(&s);
    assert_eq!(u.latency(), 0);
    let x = noise(8_000, 5);
    let (mut l, mut r) = (x.clone(), x.clone());
    let k = 3_000;
    u.process(&mut l[..k], &mut r[..k]);
    s.through_zero = true;
    u.set(&s);
    assert_eq!(u.latency(), 240);
    u.process(&mut l[k..], &mut r[k..]);
    assert!(same(&l[..k], &x[..k]), "a wire before");
    assert!(
        same(&l[k..], &x[k - 240..x.len() - 240]),
        "the centre after"
    );
    s.through_zero = false;
    u.set(&s);
    assert_eq!(u.latency(), 0);
}

/// The plain flanger is a comb, with no latency: an impulse comes back at the sweep's
/// delay (1 ms, 48 samples), then again every 49 (the delay and the one sample the last
/// wet waits to go back in), each scaled by the feedback, and nothing between.
#[test]
fn the_plain_flanger_is_its_comb() {
    let fb = 0.5f32;
    let s = FlexusSettings {
        mode: Mode::Manual,
        mix: 1.0,
        manual: 0.0,
        delay_ms: 1.0,
        depth_ms: 0.0,
        feedback: fb,
        ..FlexusSettings::default()
    };
    let n = 600;
    let mut x = vec![0.0f32; n];
    x[0] = 1.0;
    let (mut l, mut r) = (x.clone(), x.clone());
    let mut u = unit(&s);
    assert_eq!(u.latency(), 0);
    u.process(&mut l, &mut r);
    for (i, v) in l.iter().enumerate() {
        let want = if i >= 48 && (i - 48) % 49 == 0 {
            crate::math::powi(fb, ((i - 48) / 49) as i32)
        } else {
            0.0
        };
        assert!((v - want).abs() < 1e-3, "sample {i}: {v} against {want}");
    }
}

/// Through zero, the sweep held at its centre, the wet inverted at 50 %: the wet is the
/// dry, sample for sample, and they cancel to exact silence.
#[test]
fn the_centre_point_cancels_to_the_bit() {
    let s = FlexusSettings {
        mode: Mode::Manual,
        through_zero: true,
        mix: 0.5,
        invert: true,
        manual: 0.5,
        depth_ms: 8.0,
        ..FlexusSettings::default()
    };
    let x = noise(30_000, 2);
    let (mut l, mut r) = (x.clone(), x.clone());
    drive(&mut unit(&s), &mut l, &mut r, || 777);
    assert!(
        l.iter().chain(&r).all(|v| v.to_bits() == 0),
        "exact silence"
    );
    // A hair off the centre, it does not cancel.
    let mut off = s;
    off.manual = 0.52;
    let (mut l, mut r) = (x.clone(), x);
    drive(&mut unit(&off), &mut l, &mut r, || 777);
    assert!(l.iter().any(|v| v.abs() > 1e-3));
}

#[test]
fn through_zero_the_wet_passes_the_dry() {
    let mut s = busy(Mode::Lfo, true);
    s.lfo.rate = Rate::Hz(2.0);
    let mut f = unit(&s);
    let (mut before, mut after) = (false, false);
    let mut t = [0.0f32; 64];
    for _ in 0..600 {
        let (mut l, mut r) = (vec![0.1f32; 64], vec![0.1f32; 64]);
        f.process(&mut l, &mut r);
        f.telemetry(&mut t);
        before |= t[T_DELAY_MS] < -1.0;
        after |= t[T_DELAY_MS] > 1.0;
    }
    assert!(
        before && after,
        "the wet goes ahead of the dry and behind it"
    );
    assert_eq!(t[T_LATENCY], 240.0);
}

#[test]
fn feedback_either_way_stays_bounded_and_sounds_different() {
    let x = noise(48_000, 3);
    let run = |fb: f32| {
        let mut s = busy(Mode::Lfo, false);
        s.feedback = fb;
        s.invert = false;
        let (mut l, mut r) = (x.clone(), x.clone());
        drive(&mut unit(&s), &mut l, &mut r, || 512);
        l
    };
    let (up, down) = (run(0.95), run(-0.95));
    assert!(up
        .iter()
        .chain(&down)
        .all(|v| v.is_finite() && v.abs() < 20.0));
    assert!(!same(&up, &down));
}

#[test]
fn the_envelope_sweeps_further_when_louder() {
    let mut s = busy(Mode::Envelope, false);
    s.sens_db = 12.0;
    let sweep_at = |level: f32| {
        let mut f = unit(&s);
        let mut t = [0.0f32; 64];
        for _ in 0..100 {
            let (mut l, mut r) = (vec![level; 64], vec![level; 64]);
            f.process(&mut l, &mut r);
        }
        f.telemetry(&mut t);
        t[T_SWEEP]
    };
    let (quiet, loud) = (sweep_at(0.01), sweep_at(0.5));
    assert!(quiet < 0.1 && loud > 0.99, "quiet {quiet}, loud {loud}");
}

#[test]
fn every_mode_is_the_same_at_every_split() {
    let x = noise(30_000, 4);
    for mode in [Mode::Lfo, Mode::Envelope, Mode::Manual] {
        for tz in [false, true] {
            let s = busy(mode, tz);
            let (mut la, mut ra) = (x.clone(), x.clone());
            drive(&mut unit(&s), &mut la, &mut ra, || 512);
            let mut k = 0usize;
            let (mut lb, mut rb) = (x.clone(), x.clone());
            drive(&mut unit(&s), &mut lb, &mut rb, || {
                k += 1;
                [1, 7, 64, 4096, 129][k % 5]
            });
            assert!(same(&la, &lb) && same(&ra, &rb), "{mode:?} tz {tz}");
        }
    }
}

#[test]
fn it_comes_to_rest_and_skipping_is_processing_silence() {
    for mode in [Mode::Lfo, Mode::Envelope] {
        let x = noise(10_000, 8);
        let (mut l, mut r) = (x.clone(), x.clone());
        let mut s = busy(mode, true);
        s.feedback = 0.95;
        let mut a = unit(&s);
        drive(&mut a, &mut l, &mut r, || 512);
        let mut silent = 0;
        while !a.at_rest() && silent < 480_000 {
            let (mut zl, mut zr) = (vec![0.0f32; 64], vec![0.0f32; 64]);
            a.process(&mut zl, &mut zr);
            silent += 64;
        }
        assert!(a.at_rest(), "{mode:?}: {:?}", a.rest_blocker());
        let mut b = a.clone();
        let (mut zl, mut zr) = (vec![0.0f32; 999], vec![0.0f32; 999]);
        a.process(&mut zl, &mut zr);
        assert!(zl.iter().chain(&zr).all(|v| v.to_bits() == 0));
        b.skip(999);
        let y = noise(5_000, 9);
        let (mut la, mut ra) = (y.clone(), y.clone());
        let (mut lb, mut rb) = (y.clone(), y);
        a.process(&mut la, &mut ra);
        b.process(&mut lb, &mut rb);
        assert!(same(&la, &lb) && same(&ra, &rb), "{mode:?}");
    }
}

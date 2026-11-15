//! Rota's tests: the null, block size, the rotors against the published Leslie figures,
//! exact rest with the angle skipped in closed form, the switch, the drive.

use super::*;

const SR: f32 = 48_000.0;

fn tone(hz: f32, secs: f32, amp: f32) -> Vec<f32> {
    (0..(secs * SR) as usize)
        .map(|i| amp * crate::math::sin(std::f32::consts::TAU * hz * i as f32 / SR))
        .collect()
}

fn run(r: &mut Rota, x: &[f32], block: usize) -> (Vec<f32>, Vec<f32>) {
    let (mut l, mut rr) = (x.to_vec(), x.to_vec());
    for (a, b) in l.chunks_mut(block).zip(rr.chunks_mut(block)) {
        r.process(a, b);
    }
    (l, rr)
}

fn wet(speed: RotorSpeed) -> RotaSettings {
    RotaSettings {
        speed,
        mix: 1.0,
        ..RotaSettings::default()
    }
}

#[test]
fn the_defaults_are_a_null() {
    let mut r = Rota::new(SR);
    assert_eq!(r.latency(), 0);
    let x = tone(440.0, 0.5, 0.5);
    let (l, rr) = run(&mut r, &x, 64);
    assert!(l.iter().zip(&x).all(|(a, b)| a.to_bits() == b.to_bits()));
    assert!(rr.iter().zip(&x).all(|(a, b)| a.to_bits() == b.to_bits()));
    assert!(r.at_rest(), "{:?}", r.rest_blocker());
}

#[test]
fn the_block_size_changes_nothing() {
    let x = tone(330.0, 1.0, 0.4);
    let mut outs = vec![];
    for block in [1, 37, 64, 512] {
        let mut s = wet(RotorSpeed::Slow);
        s.drive = 0.5;
        let mut r = Rota::new(SR);
        r.set(&s);
        r.reset();
        let half = x.len() / 2;
        let (mut l, mut rr) = (x.clone(), x.clone());
        for (a, b) in l[..half].chunks_mut(block).zip(rr[..half].chunks_mut(block)) {
            r.process(a, b);
        }
        r.toggle();
        for (a, b) in l[half..].chunks_mut(block).zip(rr[half..].chunks_mut(block)) {
            r.process(a, b);
        }
        outs.push((l, rr));
    }
    for o in &outs[1..] {
        assert!(o.0.iter().zip(&outs[0].0).all(|(a, b)| a.to_bits() == b.to_bits()));
        assert!(o.1.iter().zip(&outs[0].1).all(|(a, b)| a.to_bits() == b.to_bits()));
    }
}

/// The time a rotor's speed takes to cover 63.2 % of a change (one time constant).
fn tau_of(r: &mut Rota, horn: bool, to: RotorSpeed) -> (f64, f64) {
    let from = if horn { r.horn.speed } else { r.drum.speed };
    let mut s = *r.settings();
    s.speed = to;
    r.set(&s);
    let target = if horn { r.horn.target } else { r.drum.target };
    let mark = from + (target - from) * (1.0 - (-1.0f64).exp());
    let silence = [0.0f32; 1];
    let mut n = 0usize;
    loop {
        let (mut a, mut b) = (silence, silence);
        r.process(&mut a, &mut b);
        n += 1;
        let v = if horn { r.horn.speed } else { r.drum.speed };
        if (target > from && v >= mark) || (target < from && v <= mark) {
            break;
        }
        assert!(n < 20 * 48_000, "never reached");
    }
    (n as f64 / SR as f64, target * 60.0)
}

#[test]
fn the_rotors_turn_as_a_leslie_does() {
    let d = RotaSettings::default();
    // The published spread (HammondWiki, a 147 and another cabinet measured; Herrera,
    // Hanson & Abel 2009): horn 40 – 49 / 400 – 424 rpm, drum 36 – 49 / 342 – 396 rpm.
    assert!((40.0..=49.0).contains(&d.horn_slow_rpm) && (400.0..=424.0).contains(&d.horn_fast_rpm));
    assert!((36.0..=49.0).contains(&d.drum_slow_rpm) && (342.0..=396.0).contains(&d.drum_fast_rpm));
    let mut r = Rota::new(SR);
    r.set(&wet(RotorSpeed::Slow));
    r.reset();
    assert!((r.horn.rpm() - 40.32).abs() < 1e-4 && (r.drum.rpm() - 36.0).abs() < 1e-4);
    let (t, rpm) = tau_of(&mut r, true, RotorSpeed::Fast);
    println!("horn up: τ {t:.3} s to {rpm:.1} rpm");
    assert!((t - 0.161).abs() < 0.002, "{t}");
    let mut r = Rota::new(SR);
    r.set(&wet(RotorSpeed::Slow));
    r.reset();
    let (t, _) = tau_of(&mut r, false, RotorSpeed::Fast);
    println!("drum up: τ {t:.3} s");
    assert!((t - 4.127).abs() < 0.01, "{t}");
    let mut r = Rota::new(SR);
    r.set(&wet(RotorSpeed::Fast));
    r.reset();
    let (t, _) = tau_of(&mut r, true, RotorSpeed::Slow);
    assert!((t - 0.321).abs() < 0.002, "horn down {t}");
    let mut r = Rota::new(SR);
    r.set(&wet(RotorSpeed::Fast));
    r.reset();
    let (t, _) = tau_of(&mut r, false, RotorSpeed::Slow);
    assert!((t - 1.371).abs() < 0.005, "drum down {t}");
    // At its speed, the horn turns 423.36 times a minute: count turns over 10 s.
    let mut r = Rota::new(SR);
    r.set(&wet(RotorSpeed::Fast));
    r.reset();
    let start = r.horn.phase;
    r.skip(480_000);
    let turns = (480_000.0 * r.horn.speed / SR as f64).floor();
    let frac = r.horn.phase.wrapping_sub(start) as f64 / TWO64;
    println!("horn: {turns} turns and {frac:.4} in 10 s");
    assert!(((turns + frac) / 10.0 * 60.0 - r.horn.rpm()).abs() < 1e-6);
}

/// Twins: one plays the silence, the other skips it in closed form; then the same note.
#[test]
fn rest_is_exact_and_the_angle_skips_in_closed_form() {
    for speed in [RotorSpeed::Slow, RotorSpeed::Fast, RotorSpeed::Stop] {
        let mk = || {
            let mut r = Rota::new(SR);
            r.set(&wet(speed));
            r.reset();
            r
        };
        let (mut a, mut b) = (mk(), mk());
        let x = tone(220.0, 0.3, 0.5);
        run(&mut a, &x, 128);
        run(&mut b, &x, 128);
        let silence = vec![0.0f32; 512];
        let mut blocks = 0;
        while !a.at_rest() {
            let (la, _) = run(&mut a, &silence, 512);
            let (lb, _) = run(&mut b, &silence, 512);
            assert_eq!(la, lb);
            blocks += 1;
            assert!(blocks < 400, "{speed:?}: {:?}", a.rest_blocker());
        }
        assert!(b.at_rest());
        for _ in 0..50 {
            let (la, _) = run(&mut a, &silence, 512);
            assert!(la.iter().all(|v| v.to_bits() == 0));
        }
        b.skip(50 * 512);
        assert_eq!(a.horn, b.horn, "{speed:?}");
        assert_eq!(a.drum, b.drum, "{speed:?}");
        let (la, ra) = run(&mut a, &x, 100);
        let (lb, rb) = run(&mut b, &x, 100);
        assert!(la.iter().zip(&lb).all(|(p, q)| p.to_bits() == q.to_bits()), "{speed:?}");
        assert!(ra.iter().zip(&rb).all(|(p, q)| p.to_bits() == q.to_bits()), "{speed:?}");
    }
}

/// A ramp is never at rest; the brake parks both rotors at its angle.
#[test]
fn a_ramp_never_rests_and_the_brake_parks() {
    let mut r = Rota::new(SR);
    r.set(&wet(RotorSpeed::Slow));
    r.reset();
    r.toggle();
    let silence = vec![0.0f32; 4_800];
    run(&mut r, &silence, 480);
    assert_eq!(r.rest_blocker(), Some("a rotor is changing speed"));
    let mut s = *r.settings();
    s.speed = RotorSpeed::Stop;
    r.set(&s);
    for _ in 0..200 {
        run(&mut r, &silence, 480);
    }
    for spin in [r.horn, r.drum] {
        assert!(spin.parked && spin.speed == 0.0);
        assert!((spin.turns() - 0.25).abs() < 1e-6, "{}", spin.turns());
    }
}

/// The drive: none at 0 (the curve is bypassed), harmonics growing with it.
#[test]
fn the_drive_adds_harmonics() {
    let x = tone(200.0, 0.5, 0.5);
    let thd = |drive: f32| {
        let mut s = wet(RotorSpeed::Stop);
        s.drive = drive;
        let mut r = Rota::new(SR);
        r.set(&s);
        r.reset();
        let mut y = x.clone();
        for c in y.chunks_mut(256) {
            for v in c.iter_mut() {
                *v = r.drive(*v);
            }
        }
        let tail = &y[12_000..];
        let n = tail.len();
        let bin = |hz: f32| {
            let (mut re, mut im) = (0.0f64, 0.0f64);
            for (i, v) in tail.iter().enumerate() {
                let (s, c) = crate::math::sin_cos(std::f64::consts::TAU * hz as f64 * i as f64 / SR as f64);
                re += *v as f64 * c;
                im += *v as f64 * s;
            }
            (re * re + im * im).sqrt() / n as f64
        };
        let f = bin(200.0);
        let h = (bin(400.0).powi(2) + bin(600.0).powi(2) + bin(800.0).powi(2)).sqrt();
        20.0 * crate::math::log10(h / f + 1e-12)
    };
    let (a, b, c) = (thd(0.0), thd(0.4), thd(0.9));
    println!("drive 0: {a:.1} dB, 0.4: {b:.1} dB, 0.9: {c:.1} dB of harmonics");
    assert!(a < -100.0 && b > -60.0 && c > b + 3.0, "{a} {b} {c}");
}

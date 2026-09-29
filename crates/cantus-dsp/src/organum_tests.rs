//! Organum's DSP, measured: tuning, drawbar steps, crosstalk, percussion, the scanner,
//! the rotors, determinism and silence.

use super::rotary::{Rotary, RotarySettings, RotorSpeed};
use super::scanner::VibMode;
use super::wheels::{cents, equal_hz, gear_hz, wheel_of, Tuning};
use super::*;

const SR: f32 = 48_000.0;

/// A plain organ: one drawbar, straight out, no vibrato, no drive, no click.
fn plain(bar: usize) -> OrganSettings {
    let mut s = OrganSettings {
        upper: [0.0; 9],
        bar_leak_db: -120.0,
        leak_db: -120.0,
        ..OrganSettings::default()
    };
    s.upper[bar] = 8.0;
    s.vib_upper = false;
    s.rotary_on = false;
    s.drive = 0.0;
    s.click_db = -120.0;
    s.release_click_db = -120.0;
    s
}

fn organ(s: &OrganSettings) -> Organum {
    let mut o = Organum::new(SR);
    o.set(s);
    o
}

fn run(o: &mut Organum, frames: usize, block: usize) -> (Vec<f32>, Vec<f32>) {
    let mut l = vec![0.0; frames];
    let mut r = vec![0.0; frames];
    let mut i = 0;
    while i < frames {
        let n = block.min(frames - i);
        o.process(&mut l[i..i + n], &mut r[i..i + n]);
        i += n;
    }
    (l, r)
}

/// Frequency from positive-going zero crossings (linearly interpolated), Hz.
fn zc_hz(x: &[f32]) -> f64 {
    let mut first = None;
    let mut last = 0.0;
    let mut count = 0usize;
    for i in 1..x.len() {
        if x[i - 1] < 0.0 && x[i] >= 0.0 {
            let t = (i - 1) as f64 + (-x[i - 1] as f64) / (x[i] as f64 - x[i - 1] as f64);
            if first.is_none() {
                first = Some(t);
            } else {
                count += 1;
            }
            last = t;
        }
    }
    count as f64 / ((last - first.unwrap()) / SR as f64)
}

/// Amplitude of the component at `hz` (Goertzel over a Hann window), linear.
fn amp_at(x: &[f32], hz: f64) -> f64 {
    let n = x.len();
    let w = std::f64::consts::TAU * hz / SR as f64;
    let (mut re, mut im, mut norm) = (0.0, 0.0, 0.0);
    for (i, v) in x.iter().enumerate() {
        let win = 0.5 - 0.5 * (std::f64::consts::TAU * i as f64 / n as f64).cos();
        re += *v as f64 * win * (w * i as f64).cos();
        im += *v as f64 * win * (w * i as f64).sin();
        norm += win;
    }
    2.0 * (re * re + im * im).sqrt() / norm
}

fn rms(x: &[f32]) -> f64 {
    (x.iter().map(|v| (*v as f64).powi(2)).sum::<f64>() / x.len() as f64).sqrt()
}

#[test]
fn every_key_sounds_its_gear_pitch_and_equal_temperament_is_exact() {
    for tuning in [Tuning::Gear, Tuning::Equal] {
        let mut s = plain(2);
        s.tuning = tuning;
        for key in [36u8, 43, 48, 57, 60, 69, 72, 81, 84, 96] {
            let mut o = organ(&s);
            o.note_on(key, 1, 0);
            let (l, _) = run(&mut o, 48_000 + 4_800, 256);
            let hz = zc_hz(&l[4_800..]);
            let w = wheel_of(key as i32);
            let want = match tuning {
                Tuning::Gear => gear_hz(w),
                Tuning::Equal => equal_hz(w, 440.0),
            };
            let err = cents(hz, want);
            assert!(
                err.abs() < 0.05,
                "{tuning:?} key {key}: {hz:.4} Hz, {err:+.4} ct"
            );
        }
    }
}

#[test]
fn drawbar_steps_are_three_db() {
    let mut last: Option<f64> = None;
    for pos in 1..=8 {
        let mut s = plain(2);
        s.upper[2] = pos as f32;
        let mut o = organ(&s);
        o.note_on(60, 1, 0);
        let (l, _) = run(&mut o, 24_000, 512);
        let db = 20.0 * rms(&l[4_800..]).log10();
        if let Some(prev) = last {
            let step = db - prev;
            assert!(
                (step - 3.0).abs() < 0.1,
                "position {pos}: step {step:.3} dB"
            );
        }
        last = Some(db);
    }
}

#[test]
fn crosstalk_is_measured_at_the_leakage_setting() {
    for leak in [-40.0f32, -30.0] {
        let mut s = plain(2);
        s.leak_db = leak;
        let mut o = organ(&s);
        o.note_on(48, 1, 0);
        let (l, _) = run(&mut o, 48_000, 256);
        let x = &l[4_800..];
        let f0 = gear_hz(wheel_of(48));
        let leak_hz = gear_hz(wheel_of(72));
        let rel = 20.0 * (amp_at(x, leak_hz) / amp_at(x, f0)).log10();
        assert!(
            (rel - leak as f64).abs() < 0.5,
            "leakage {leak}: measured {rel:.2} dB"
        );
    }
}

#[test]
fn percussion_is_single_trigger_and_steals_the_one_foot() {
    let mut s = plain(8); // only the 1′, which the percussion takes
    s.perc_on = true;
    let onset = |o: &mut Organum| {
        let (l, _) = run(o, 960, 64);
        rms(&l[240..])
    };
    let mut o = organ(&s);
    o.note_on(60, 1, 0);
    let first = onset(&mut o);
    run(&mut o, 24_000, 256); // half a second: the fast decay is 30 dB down
    o.note_on(64, 2, 0); // legato: no new attack
    let legato = onset(&mut o);
    assert!(
        legato < first * 0.1,
        "a legato note retriggered the percussion: {first} then {legato}"
    );
    o.note_off(1);
    o.note_off(2);
    run(&mut o, 4_800, 256);
    o.note_on(67, 3, 0);
    let again = onset(&mut o);
    assert!(
        again > first * 0.5,
        "after all keys were up the percussion must trigger again: {first} then {again}"
    );
    // With the percussion off, the 1′ is a drawbar again; with it on, only the
    // percussion's harmonic sounds, not the 1′'s pitch.
    let mut o = organ(&s);
    o.note_on(60, 1, 0);
    let (l, _) = run(&mut o, 9_600, 256);
    let one_foot = gear_hz(wheel_of(60 + 36));
    let third = gear_hz(wheel_of(60 + 19));
    let x = &l[2_400..];
    assert!(amp_at(x, one_foot) < amp_at(x, third) * 1e-3);
}

#[test]
fn the_scanner_turns_at_its_rate() {
    let mut s = plain(2);
    s.vib_upper = true;
    s.vib_mode = VibMode::V3;
    s.vib_rate_hz = 7.0;
    let mut o = organ(&s);
    o.note_on(72, 1, 0);
    let (l, _) = run(&mut o, 96_000, 256);
    // The instantaneous frequency, one value per cycle, and its modulation's strongest
    // component between 3 and 12 Hz.
    let mut times = Vec::new();
    let x = &l[4_800..];
    for i in 1..x.len() {
        if x[i - 1] < 0.0 && x[i] >= 0.0 {
            times.push((i - 1) as f64 + (-x[i - 1] as f64) / (x[i] as f64 - x[i - 1] as f64));
        }
    }
    let periods: Vec<(f64, f64)> = times
        .windows(2)
        .map(|w| (w[0] / SR as f64, SR as f64 / (w[1] - w[0])))
        .collect();
    let mean = periods.iter().map(|p| p.1).sum::<f64>() / periods.len() as f64;
    let mut best = (0.0f64, 0.0f64);
    let mut f = 3.0;
    while f < 12.0 {
        let (mut re, mut im) = (0.0, 0.0);
        for (t, hz) in &periods {
            re += (hz - mean) * (std::f64::consts::TAU * f * t).cos();
            im += (hz - mean) * (std::f64::consts::TAU * f * t).sin();
        }
        let m = re * re + im * im;
        if m > best.1 {
            best = (f, m);
        }
        f += 0.05;
    }
    assert!(
        (best.0 - 7.0).abs() < 0.15,
        "strongest modulation at {} Hz",
        best.0
    );
    // V3 swings the pitch by tens of cents; the dry organ does not.
    let spread = periods
        .iter()
        .map(|p| cents(p.1, mean).abs())
        .fold(0.0, f64::max);
    assert!(
        spread > 10.0 && spread < 80.0,
        "vibrato depth {spread:.1} ct"
    );
}

#[test]
fn rotors_turn_at_their_rpm_spin_up_with_their_time_constant_and_park_at_the_brake() {
    let mut r = Rotary::new(SR);
    let s = RotarySettings {
        speed: RotorSpeed::Fast,
        ..RotarySettings::default()
    };
    r.set(&s);
    r.snap_speeds();
    assert!((r.horn.rpm() - 420.0).abs() < 1e-3 && (r.drum.rpm() - 357.0).abs() < 1e-3);
    // Slow → fast: after one time constant, 63.2 % of the way (±5 %).
    let mut s = RotarySettings::default();
    r.set(&s);
    r.snap_speeds();
    s.speed = RotorSpeed::Fast;
    r.set(&s);
    for (rotor, tau, from, to) in [
        ("horn", 0.16f32, 40.0f32, 420.0f32),
        ("drum", 4.1, 36.0, 357.0),
    ] {
        let mut q = r.clone();
        for _ in 0..(tau * SR) as usize {
            q.advance();
        }
        let rpm = if rotor == "horn" {
            q.horn.rpm()
        } else {
            q.drum.rpm()
        };
        let frac = (rpm - from) / (to - from);
        println!(
            "organum: {rotor} after one time constant ({tau} s): {:.2} % of the way",
            frac * 100.0
        );
        assert!(
            (frac - 0.632).abs() < 0.632 * 0.05,
            "{rotor}: {frac:.3} of the way after τ"
        );
    }
    // Brake: parks within 2° of the brake angle.
    for brake in [0.0f32, 90.0, 237.0] {
        let mut q = r.clone();
        let s = RotarySettings {
            speed: RotorSpeed::Stop,
            brake_deg: brake,
            ..RotarySettings::default()
        };
        q.set(&s);
        for _ in 0..(8.0 * SR) as usize {
            q.advance();
        }
        for (name, rotor) in [("horn", q.horn), ("drum", q.drum)] {
            let err = ((rotor.angle * 360.0 - brake as f64 + 540.0).rem_euclid(360.0)) - 180.0;
            println!("organum: {name} parked {err:+.3} degrees from {brake}");
            assert!(
                err.abs() < 2.0 && rotor.speed == 0.0,
                "{name} parked {err:.2}° from {brake}°"
            );
        }
    }
}

#[test]
fn the_horns_amplitude_modulation_has_the_rotor_period() {
    for (speed, rpm) in [(RotorSpeed::Fast, 420.0f64), (RotorSpeed::Slow, 40.0)] {
        let mut r = Rotary::new(SR);
        let s = RotarySettings {
            speed,
            balance_db: 12.0,
            ..RotarySettings::default()
        };
        r.set(&s);
        r.snap_speeds();
        let secs = if rpm > 100.0 { 2.0 } else { 9.0 };
        let n = (secs * SR as f64) as usize;
        let hop = 96;
        let mut env = Vec::new();
        let mut acc = 0.0f64;
        for i in 0..n {
            let x = (std::f64::consts::TAU * 3_000.0 * i as f64 / SR as f64).sin() as f32;
            let (l, _) = r.tick(0.5 * x);
            acc += (l as f64).powi(2);
            if (i + 1) % hop == 0 {
                env.push(acc.sqrt());
                acc = 0.0;
            }
        }
        // The period is where the envelope's autocorrelation peaks, near 60/rpm.
        let want = 60.0 / rpm;
        let lag0 = (want * SR as f64 / hop as f64) as usize;
        let mean = env.iter().sum::<f64>() / env.len() as f64;
        let e: Vec<f64> = env.iter().map(|v| v - mean).collect();
        let corr = |lag: usize| (0..e.len() - lag).map(|i| e[i] * e[i + lag]).sum::<f64>();
        let (lo, hi) = (lag0 * 8 / 10, lag0 * 12 / 10);
        let best = (lo..=hi)
            .max_by(|a, b| corr(*a).total_cmp(&corr(*b)))
            .unwrap();
        // Refine with a parabola through the peak.
        let (a, b, c) = (corr(best - 1), corr(best), corr(best + 1));
        let off = 0.5 * (a - c) / (a - 2.0 * b + c);
        let period = (best as f64 + off) * hop as f64 / SR as f64;
        println!(
            "organum: {speed:?} horn AM period {period:.4} s against {want:.4} s ({:+.2} %)",
            (period - want) / want * 100.0
        );
        assert!(
            (period - want).abs() / want < 0.01,
            "{speed:?}: AM period {period:.4} s, the rotor's {want:.4} s"
        );
    }
}

/// A performance with everything that moves: notes, a legato, percussion, a speed change,
/// a drawbar move, releases.
fn scenario(block: usize) -> Vec<f32> {
    let mut s = OrganSettings {
        perc_on: true,
        split_upper: 60,
        split_pedal: 48,
        ..OrganSettings::default()
    };
    let mut o = organ(&s);
    let mut out = Vec::new();
    let mut go = |o: &mut Organum, frames: usize| {
        let (l, r) = run(o, frames, block);
        out.extend(l.iter().zip(&r).flat_map(|(a, b)| [*a, *b]));
    };
    o.note_on(36, 1, 0);
    o.note_on(55, 2, 0);
    o.note_on(72, 3, 0);
    go(&mut o, 7_001);
    o.note_on(76, 4, 0);
    s.rotary.speed = RotorSpeed::Fast;
    s.upper[4] = 6.0;
    o.set(&s);
    go(&mut o, 12_345);
    o.note_off(3);
    o.note_off(1);
    go(&mut o, 5_000);
    o.all_notes_off();
    go(&mut o, 30_000);
    out
}

#[test]
fn the_output_is_the_same_at_every_block_size() {
    let reference = scenario(512);
    assert!(reference.iter().any(|x| *x != 0.0));
    for block in [1usize, 7, 64, 256, 4096] {
        let other = scenario(block);
        let report = crate::nulltest::compare(&reference, &other);
        assert!(report.passes(0.0), "block {block}: {report:?}");
    }
}

#[test]
fn it_is_exactly_silent_once_the_notes_are_over() {
    let mut o = organ(&OrganSettings::default());
    o.note_on(60, 1, 0);
    o.note_on(64, 2, 0);
    run(&mut o, 4_800, 256);
    o.note_off(1);
    o.note_off(2);
    let (l, r) = run(&mut o, 48_000, 256);
    let tail = &l[24_000..];
    assert!(tail.iter().chain(&r[24_000..]).all(|x| *x == 0.0));
    let mut a = vec![0.0; 256];
    let mut b = vec![0.0; 256];
    assert!(
        !o.process(&mut a, &mut b),
        "an idle organ reports that it wrote nothing"
    );
}

#[test]
fn key_click_is_seeded_and_scales_with_its_level() {
    let hf = |click: f32, id: u64| {
        let mut s = plain(2);
        s.upper = [8.0; 9];
        s.click_db = click;
        let mut o = organ(&s);
        o.note_on(48, id, 0);
        let (l, _) = run(&mut o, 480, 480);
        // Energy far above the tones: the second difference.
        let d: Vec<f32> = l.windows(3).map(|w| w[2] - 2.0 * w[1] + w[0]).collect();
        (rms(&d), l)
    };
    let (loud, a) = hf(0.0, 7);
    let (soft, _) = hf(-60.0, 7);
    let (_, again) = hf(0.0, 7);
    assert_eq!(a, again, "the same note clicks the same way");
    assert!(loud > soft * 2.0, "click at 0 dB {loud} vs −60 dB {soft}");
    let (_, other) = hf(0.0, 8);
    assert_ne!(a, other, "another note bounces differently");
}

#[test]
fn a_split_sends_keys_to_their_division() {
    let s = OrganSettings {
        split_upper: 60,
        split_pedal: 48,
        ..OrganSettings::default()
    };
    let o = organ(&s);
    assert_eq!(o.route(40, 0).0, Manual::Pedal);
    assert_eq!(o.route(50, 0).0, Manual::Lower);
    assert_eq!(o.route(60, 0).0, Manual::Upper);
    assert_eq!(
        o.route(80, 1).0,
        Manual::Lower,
        "channel 2 is the lower manual"
    );
    assert_eq!(o.route(80, 2).0, Manual::Pedal, "channel 3 the pedals");
    let o = organ(&OrganSettings::default());
    assert_eq!(
        o.route(20, 0),
        (Manual::Upper, 44),
        "no split: all upper, folded in"
    );
}

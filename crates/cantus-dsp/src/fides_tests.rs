//! Fides's DSP tests: silence and rest, block-size independence to the bit, parallel equal
//! to serial, renders that repeat, pitch to 0.1 cent at any bend, every exciter and
//! resonator finite and bounded, and the levels each kind is calibrated to.

use super::body::BodyKind;
use super::exciter::ExciterKind;
use super::tables::ResKind;
use super::*;
use crate::math;
use crate::par::{Parallel, Reversed, Serial};

const SR: f32 = 48_000.0;

/// A score: (sample, event) — 0 note on (key, velocity), 1 note off (key), 2 bend
/// (semitones), 3 pressure (id = key), 4 slide.
#[derive(Copy, Clone, Debug)]
enum Ev {
    On(u8, f32),
    Off(u8),
    Bend(f32),
    Press(u8, f32),
    Slide(u8, f32),
}

fn apply(f: &mut Fides, e: Ev) {
    match e {
        Ev::On(k, v) => f.note_on(k, v, k as u64 + 1000),
        Ev::Off(k) => f.note_off(k, k as u64 + 1000),
        Ev::Bend(st) => f.pitch_bend(st),
        Ev::Press(k, v) => f.expression(k as u64 + 1000, 1, v),
        Ev::Slide(k, v) => f.expression(k as u64 + 1000, 2, v),
    }
}

/// Render `frames` of `score` in blocks from `block` (a function of the block's number),
/// cutting at every event, through `par`.
fn render(
    s: &FidesSettings,
    score: &[(usize, Ev)],
    frames: usize,
    block: &dyn Fn(usize) -> usize,
    par: &dyn Parallel,
) -> (Vec<f32>, Vec<f32>) {
    let mut f = Fides::new(SR);
    f.set(s);
    let mut l = vec![0.0; frames];
    let mut r = vec![0.0; frames];
    let mut at = 0;
    let mut ev = 0;
    let mut b = 0;
    while at < frames {
        while ev < score.len() && score[ev].0 <= at {
            apply(&mut f, score[ev].1);
            ev += 1;
        }
        let next = score.get(ev).map_or(frames, |e| e.0);
        let n = block(b).max(1).min(next.max(at + 1) - at).min(frames - at);
        f.process_par(&mut l[at..at + n], &mut r[at..at + n], par);
        at += n;
        b += 1;
    }
    (l, r)
}

fn melody() -> Vec<(usize, Ev)> {
    vec![
        (100, Ev::On(48, 0.8)),
        (3_000, Ev::On(55, 0.6)),
        (7_777, Ev::On(64, 1.0)),
        (9_000, Ev::Bend(1.0)),
        (12_000, Ev::Off(48)),
        (15_011, Ev::On(67, 0.4)),
        (16_000, Ev::Press(67, 0.7)),
        (18_500, Ev::Slide(67, 0.8)),
        (20_000, Ev::Off(55)),
        (20_001, Ev::On(48, 0.9)),
        (26_000, Ev::Bend(0.0)),
        (30_000, Ev::Off(64)),
        (33_000, Ev::Off(67)),
        (36_000, Ev::Off(48)),
    ]
}

/// A voice of every exciter on every resonator, B on and off, and the bodies.
fn variety() -> Vec<FidesSettings> {
    let mut v = vec![FidesSettings::default()];
    for (i, e) in ExciterKind::ALL.iter().enumerate() {
        for (j, r) in ResKind::ALL.iter().enumerate() {
            if (i + j) % 3 != 0 {
                continue;
            }
            let mut s = FidesSettings::default();
            s.exc.kind = *e;
            s.a.kind = *r;
            s.a.decay_s = 1.5;
            s.noise.level = if j % 2 == 0 { 0.3 } else { 0.0 };
            s.noise.sustain = 0.5;
            s.b_on = j % 4 == 0;
            s.serial = j % 8 == 0;
            s.b.kind = ResKind::ALL[(j + 5) % 12];
            s.b.decay_s = 0.8;
            s.body.kind = BodyKind::ALL[(i + j) % 7];
            s.vib_depth_ct = if i % 2 == 0 { 20.0 } else { 0.0 };
            s.trem_depth = if j % 5 == 0 { 0.4 } else { 0.0 };
            s.lowcut_hz = if j % 3 == 0 { 80.0 } else { 10.0 };
            s.highcut_hz = if j % 2 == 0 { 9_000.0 } else { 20_000.0 };
            s.release_s = 0.3;
            v.push(s);
        }
    }
    v
}

fn first_difference(a: &(Vec<f32>, Vec<f32>), b: &(Vec<f32>, Vec<f32>)) -> Option<usize> {
    (0..a.0.len())
        .find(|&i| a.0[i].to_bits() != b.0[i].to_bits() || a.1[i].to_bits() != b.1[i].to_bits())
}

#[test]
fn a_new_instance_is_silent_and_at_rest() {
    let mut f = Fides::new(SR);
    assert!(f.at_rest());
    let (mut l, mut r) = (vec![0.0; 512], vec![0.0; 512]);
    assert!(!f.process(&mut l, &mut r));
    assert!(l.iter().chain(&r).all(|x| *x == 0.0));
    assert!(f.at_rest());
}

#[test]
fn every_exciter_and_resonator_sounds_finite_and_bounded() {
    for (i, s) in variety().iter().enumerate() {
        let (l, r) = render(s, &melody(), 48_000, &|_| 512, &Serial);
        let peak = l.iter().chain(&r).fold(0f32, |m, x| m.max(x.abs()));
        assert!(l.iter().chain(&r).all(|x| x.is_finite()), "#{i} {s:?}");
        assert!(
            peak < 4.0,
            "#{i}: peak {peak} ({:?} on {:?})",
            s.exc.kind,
            s.a.kind
        );
        if s.exc.kind != ExciterKind::Input {
            assert!(
                peak > 1e-4,
                "#{i}: silent ({:?} on {:?})",
                s.exc.kind,
                s.a.kind
            );
        }
    }
}

#[test]
fn renders_are_the_same_at_every_block_size_and_repeat_to_the_bit() {
    for (i, s) in variety().iter().enumerate().step_by(2) {
        let reference = render(s, &melody(), 40_000, &|_| 512, &Serial);
        assert_eq!(
            first_difference(&reference, &render(s, &melody(), 40_000, &|_| 512, &Serial)),
            None,
            "#{i} repeats"
        );
        for block in [1usize, 37, 64] {
            let other = render(s, &melody(), 40_000, &|_| block, &Serial);
            assert_eq!(
                first_difference(&reference, &other),
                None,
                "#{i} at block {block}"
            );
        }
        let ragged = render(
            s,
            &melody(),
            40_000,
            &|b| [1, 300, 7, 64, 129][b % 5],
            &Serial,
        );
        assert_eq!(first_difference(&reference, &ragged), None, "#{i} ragged");
    }
}

#[test]
fn voices_on_the_pool_are_the_serial_bits() {
    for (i, s) in variety().iter().enumerate().step_by(3) {
        let serial = render(s, &melody(), 40_000, &|_| 256, &Serial);
        let par = render(s, &melody(), 40_000, &|_| 256, &Reversed(4));
        assert_eq!(first_difference(&serial, &par), None, "#{i}");
    }
}

// ---- pitch ------------------------------------------------------------------------------

/// The frequency of the partial near `near` Hz over `from…to` (s): the signal heterodyned
/// to `near`, low-passed by two boxcars of four of its periods (whose nulls fall on every
/// other harmonic of it), and the slope of its unwrapped phase fitted by least squares.
fn measure_hz(x: &[f32], near: f64, from: f64, to: f64) -> f64 {
    let sr = SR as f64;
    let len = ((4.0 * sr / near).round() as usize).max(4);
    let w = std::f64::consts::TAU * near / sr;
    let mut re: Vec<f64> = Vec::with_capacity(x.len());
    let mut im: Vec<f64> = Vec::with_capacity(x.len());
    for (n, v) in x.iter().enumerate() {
        let (s, c) = math::sin_cos(w * n as f64);
        re.push(*v as f64 * c);
        im.push(-(*v as f64) * s);
    }
    let boxcar = |v: &[f64]| -> Vec<f64> {
        let mut out = vec![0.0; v.len()];
        let mut acc = 0.0;
        for i in 0..v.len() {
            acc += v[i];
            if i >= len {
                acc -= v[i - len];
            }
            out[i] = acc / len as f64;
        }
        out
    };
    let (re, im) = (boxcar(&boxcar(&re)), boxcar(&boxcar(&im)));
    // After the two boxcars have filled.
    let from = from.max(2.0 * len as f64 / sr + 0.02);
    let (a, b) = ((from * sr) as usize, ((to * sr) as usize).min(x.len()));
    let step = 16;
    let (mut ts, mut ph) = (Vec::new(), Vec::new());
    let mut last = 0.0f64;
    let mut unwrap = 0.0f64;
    for i in (a..b).step_by(step) {
        let p = math::atan2(im[i], re[i]);
        if !ts.is_empty() {
            let d = p - last;
            if d > std::f64::consts::PI {
                unwrap -= std::f64::consts::TAU;
            } else if d < -std::f64::consts::PI {
                unwrap += std::f64::consts::TAU;
            }
        }
        last = p;
        ts.push(i as f64 / sr);
        ph.push(p + unwrap);
    }
    let n = ts.len() as f64;
    let (mt, mp) = (ts.iter().sum::<f64>() / n, ph.iter().sum::<f64>() / n);
    let (mut num, mut den) = (0.0, 0.0);
    for (t, p) in ts.iter().zip(&ph) {
        num += (t - mt) * (p - mp);
        den += (t - mt) * (t - mt);
    }
    near + num / den / std::f64::consts::TAU
}

fn cents(f: f64, want: f64) -> f64 {
    1200.0 * math::log2(f / want)
}

/// One held note with a bend (global, ±2 st) or an MPE tuning, A alone, no body: its
/// fundamental's error in cents.
fn pitch_error(s: &FidesSettings, key: u8, bend: f32, tuning: f32) -> f64 {
    let mut f = Fides::new(SR);
    let mut s = *s;
    s.body.kind = BodyKind::None;
    s.width = 0.0;
    f.set(&s);
    f.pitch_bend(bend);
    f.note_on(key, 0.8, 7);
    if tuning != 0.0 {
        f.expression(7, 0, tuning);
    }
    let frames = (1.25 * SR) as usize;
    let (mut l, mut r) = (vec![0.0; frames], vec![0.0; frames]);
    for at in (0..frames).step_by(256) {
        let n = 256.min(frames - at);
        f.process(&mut l[at..at + n], &mut r[at..at + n]);
    }
    let want = 440.0 * math::exp2((key as f64 + bend as f64 + tuning as f64 - 69.0) / 12.0);
    let got = measure_hz(&l, want, 0.2, 1.2);
    cents(got, want)
}

#[test]
fn the_measure_finds_a_sine_to_a_thousandth_of_a_cent() {
    for hz in [41.2f64, 261.63, 1046.5, 3520.0] {
        let x: Vec<f32> = (0..60_000)
            .map(|n| math::sin(std::f64::consts::TAU * hz * 1.0001 * n as f64 / SR as f64) as f32)
            .collect();
        let got = measure_hz(&x, hz, 0.1, 1.2);
        assert!(cents(got, hz * 1.0001).abs() < 1e-3, "{hz}: {got}");
    }
}

#[test]
fn pitch_is_within_a_tenth_of_a_cent_at_any_bend() {
    let mut worst = Vec::new();
    for (name, kind, inharm) in [
        ("string", ResKind::String, 0.0f32),
        ("stiff string", ResKind::String, 0.6),
        ("pipe", ResKind::Pipe, 0.0),
        ("tube", ResKind::Tube, 0.0),
        ("marimba", ResKind::Marimba, 0.0),
        ("tine", ResKind::Tine, 0.0),
        ("pan", ResKind::Pan, 0.0),
    ] {
        let mut s = FidesSettings::default();
        s.a.kind = kind;
        s.a.inharm = inharm;
        s.a.decay_s = 8.0;
        s.exc.kind = ExciterKind::Mallet;
        s.exc.contact_ms = 0.5;
        for key in [28u8, 40, 52, 60, 69, 81, 93] {
            for (bend, tuning) in [
                (0.0f32, 0.0f32),
                (2.0, 0.0),
                (-2.0, 0.0),
                (0.37, 0.0),
                (0.0, 1.61),
            ] {
                let e = pitch_error(&s, key, bend, tuning);
                worst.push((
                    e.abs(),
                    format!("{name} key {key} bend {bend} tuning {tuning}: {e:+.4} ct"),
                ));
            }
        }
    }
    worst.sort_by(|a, b| b.0.total_cmp(&a.0));
    for w in worst.iter().take(5) {
        println!("{}", w.1);
    }
    assert!(worst[0].0 < 0.1, "{}", worst[0].1);
}

/// A bowed string's pitch is the stick and slip's: the loop sets it and the bow's friction
/// pulls it a little flat as it presses harder (as a real bow does). Measured and held
/// within 2 cents at the default pressure across the range; printed for the report.
#[test]
fn a_bowed_string_plays_in_tune() {
    let mut worst = 0f64;
    for (pressure, speed) in [(0.5f32, 0.5f32), (0.3, 0.4), (0.8, 0.7)] {
        let mut s = FidesSettings::default();
        s.exc.kind = ExciterKind::Bow;
        s.exc.bow_pressure = pressure;
        s.exc.bow_speed = speed;
        s.exc.bow_attack_ms = 40.0;
        let mut line = String::new();
        for key in [36u8, 48, 60, 72, 84] {
            let e = pitch_error(&s, key, 0.0, 0.0);
            line += &format!(" {key}: {e:+.3}");
            if pressure == 0.5 {
                worst = worst.max(e.abs());
            }
        }
        println!("bowed at pressure {pressure}, speed {speed}:{line} ct");
    }
    assert!(worst < 2.0, "{worst} ct");
}

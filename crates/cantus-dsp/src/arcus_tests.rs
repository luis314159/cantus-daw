//! Arcus's DSP tests — the house's invariants: a new instance silent and at rest, every
//! technique finite, bounded and sounding, renders the same at every block size and
//! repeated, the pool's voices the serial bits, exact rest and as good as new, the tuning at
//! its default to the bit, the plucked pitch to 0.1 cent. The gate's measurements (Schelleng,
//! the attack, the bowed pitch, the vibrato, the body) are `arcus_tests_gate.rs`.

use super::tables::Instrument;
use super::*;
use crate::math;
use crate::par::{Parallel, Reversed, Serial};

pub(super) const SR: f32 = 48_000.0;

/// A score's event.
#[derive(Copy, Clone, Debug)]
pub(super) enum Ev {
    On(u8, f32),
    Off(u8),
    Bend(f32),
    Press(u8, f32),
    Slide(u8, f32),
    /// A controller (`Arcus::controller`'s `which`, value).
    Cc(u8, f32),
    /// The next note's own articulation and bow.
    Art(u8, Option<Art>, Option<BowDir>),
}

pub(super) fn id_of(k: u8) -> u64 {
    k as u64 + 1000
}

pub(super) fn apply(f: &mut Arcus, e: Ev) {
    match e {
        Ev::On(k, v) => f.note_on(k, v, id_of(k)),
        Ev::Off(k) => f.note_off(k, id_of(k)),
        Ev::Bend(st) => f.pitch_bend(st),
        Ev::Press(k, v) => f.expression(id_of(k), 1, v),
        Ev::Slide(k, v) => f.expression(id_of(k), 2, v),
        Ev::Cc(w, v) => f.controller(w, v),
        Ev::Art(k, a, b) => f.note_art(id_of(k), a, b),
    }
}

/// Render `frames` of `score` in blocks from `block`, cutting at every event, through `par`.
pub(super) fn render(
    s: &ArcusSettings,
    score: &[(usize, Ev)],
    frames: usize,
    block: &dyn Fn(usize) -> usize,
    par: &dyn Parallel,
) -> (Vec<f32>, Vec<f32>) {
    let mut f = Arcus::new(SR);
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

/// A phrase that uses every gesture: a bowed note, a slur, a re-bow, a pluck, a tremolo, a
/// staccato and a spiccato by their art, MPE's pressure and slide, a bend, a CC1 swell.
pub(super) fn phrase() -> Vec<(usize, Ev)> {
    vec![
        (100, Ev::On(48, 0.7)),
        (9_000, Ev::On(50, 0.6)),
        (9_400, Ev::Off(48)),
        (14_000, Ev::Cc(0, 0.3)),
        (16_000, Ev::Cc(0, 0.8)),
        (19_000, Ev::Off(50)),
        (19_600, Ev::On(55, 0.9)),
        (21_000, Ev::Press(55, 0.9)),
        (22_000, Ev::Slide(55, 0.8)),
        (24_000, Ev::Bend(1.0)),
        (27_000, Ev::Off(55)),
        (27_500, Ev::Bend(0.0)),
        (28_000, Ev::Art(43, Some(Art::Pizzicato), None)),
        (28_000, Ev::On(43, 0.8)),
        (31_000, Ev::Off(43)),
        (32_000, Ev::Art(57, Some(Art::Tremolo), Some(BowDir::Up))),
        (32_000, Ev::On(57, 0.6)),
        (40_000, Ev::Off(57)),
        (41_000, Ev::Art(52, Some(Art::Staccato), None)),
        (41_000, Ev::On(52, 0.8)),
        (43_000, Ev::Off(52)),
        (45_000, Ev::Art(53, Some(Art::Spiccato), None)),
        (45_000, Ev::On(53, 0.8)),
        (46_000, Ev::Off(53)),
    ]
}

/// Settings that reach every switch: each instrument, each technique, each friction.
pub(super) fn variety() -> Vec<ArcusSettings> {
    let mut v = vec![ArcusSettings::default()];
    for (i, inst) in Instrument::ALL.iter().enumerate() {
        for (j, tech) in Technique::ALL.iter().enumerate() {
            let mut s = ArcusSettings {
                instrument: *inst,
                technique: *tech,
                ..ArcusSettings::default()
            };
            s.friction = if (i + j) % 2 == 0 {
                string::Friction::Hyperbolic
            } else {
                string::Friction::Thermal
            };
            s.mode = match (i + j) % 3 {
                0 => crate::voices::PlayMode::Legato,
                1 => crate::voices::PlayMode::Mono,
                _ => crate::voices::PlayMode::Poly,
            };
            s.bow_noise_db = if j == 1 { -60.0 } else { 3.0 };
            s.porta_ms = if i == 1 { 80.0 } else { 0.0 };
            s.lowcut_hz = if j == 2 { 60.0 } else { 10.0 };
            s.highcut_hz = if i == 2 { 9_000.0 } else { 20_000.0 };
            s.body_mix = if i == 3 { 0.0 } else { 1.0 };
            v.push(s);
        }
    }
    v
}

pub(super) fn first_difference(
    a: &(Vec<f32>, Vec<f32>),
    b: &(Vec<f32>, Vec<f32>),
) -> Option<usize> {
    (0..a.0.len())
        .find(|&i| a.0[i].to_bits() != b.0[i].to_bits() || a.1[i].to_bits() != b.1[i].to_bits())
}

fn rms(x: &[f32]) -> f64 {
    (x.iter().map(|v| (*v as f64) * (*v as f64)).sum::<f64>() / x.len().max(1) as f64).sqrt()
}

#[test]
fn a_new_instance_is_silent_and_at_rest() {
    let mut f = Arcus::new(SR);
    assert!(f.at_rest());
    let (mut l, mut r) = (vec![0.0; 512], vec![0.0; 512]);
    assert!(!f.process(&mut l, &mut r));
    assert!(l.iter().chain(&r).all(|x| x.to_bits() == 0));
    assert!(f.at_rest());
}

#[test]
fn every_instrument_and_technique_sounds_finite_and_bounded() {
    for (n, s) in variety().iter().enumerate() {
        let (l, r) = render(s, &phrase(), 60_000, &|_| 256, &Serial);
        let peak = l.iter().chain(&r).fold(0f32, |m, x| m.max(x.abs()));
        assert!(
            l.iter().chain(&r).all(|x| x.is_finite()),
            "setting {n}: not finite"
        );
        assert!(
            peak > 1e-3,
            "setting {n} ({:?}): silent ({peak})",
            s.instrument
        );
        assert!(peak < 4.0, "setting {n} ({:?}): peak {peak}", s.instrument);
    }
}

#[test]
fn renders_are_the_same_at_every_block_size_and_repeat_to_the_bit() {
    for (n, s) in variety().iter().enumerate().step_by(2) {
        let base = render(s, &phrase(), 50_000, &|_| 512, &Serial);
        let again = render(s, &phrase(), 50_000, &|_| 512, &Serial);
        assert_eq!(first_difference(&base, &again), None, "setting {n}: repeat");
        for size in [1usize, 37, 64] {
            let other = render(s, &phrase(), 50_000, &|_| size, &Serial);
            assert_eq!(
                first_difference(&base, &other),
                None,
                "setting {n}, block {size}"
            );
        }
        let ragged = render(
            s,
            &phrase(),
            50_000,
            &|b| [3, 200, 31, 999, 64, 7][b % 6],
            &Serial,
        );
        assert_eq!(
            first_difference(&base, &ragged),
            None,
            "setting {n}: ragged"
        );
    }
}

#[test]
fn voices_on_the_pool_are_the_serial_bits() {
    let mut s = ArcusSettings {
        mode: crate::voices::PlayMode::Poly,
        ..ArcusSettings::default()
    };
    let chord = vec![
        (100, Ev::On(36, 0.7)),
        (110, Ev::On(43, 0.6)),
        (120, Ev::On(50, 0.8)),
        (130, Ev::On(57, 0.5)),
        (20_000, Ev::Off(36)),
        (21_000, Ev::Off(43)),
        (22_000, Ev::Off(50)),
        (23_000, Ev::Off(57)),
    ];
    for inst in [Instrument::Cello, Instrument::Bass] {
        s.instrument = inst;
        let serial = render(&s, &chord, 40_000, &|_| 256, &Serial);
        for w in [2usize, 3, 7] {
            let pool = render(&s, &chord, 40_000, &|_| 256, &Reversed(w));
            assert_eq!(first_difference(&serial, &pool), None, "{inst:?} on {w}");
        }
    }
}

#[test]
fn a_released_note_ends_in_exact_silence_and_the_instance_is_as_good_as_new() {
    for s in variety().iter().step_by(3) {
        let mut f = Arcus::new(SR);
        f.set(s);
        f.note_on(50, 0.8, 1);
        let (mut l, mut r) = (vec![0.0; 256], vec![0.0; 256]);
        for _ in 0..80 {
            l.fill(0.0);
            r.fill(0.0);
            f.process(&mut l, &mut r);
        }
        f.note_off(50, 1);
        let mut blocks = 0;
        while !f.at_rest() {
            l.fill(0.0);
            r.fill(0.0);
            f.process(&mut l, &mut r);
            blocks += 1;
            assert!(
                blocks < 48_000 * 20 / 256,
                "{:?}: never came to rest",
                s.technique
            );
        }
        l.fill(0.0);
        r.fill(0.0);
        assert!(!f.process(&mut l, &mut r));
        assert!(l.iter().chain(&r).all(|x| x.to_bits() == 0));
        // As good as new: the next note's bits are a fresh instance's skipped as far.
        let mut fresh = Arcus::new(SR);
        fresh.set(s);
        fresh.skip(((80 + blocks + 1) * 256) as usize);
        let mut a = (vec![0.0; 9_000], vec![0.0; 9_000]);
        let mut b = (vec![0.0; 9_000], vec![0.0; 9_000]);
        f.note_on(55, 0.7, 2);
        fresh.note_on(55, 0.7, 2);
        f.process(&mut a.0, &mut a.1);
        fresh.process(&mut b.0, &mut b.1);
        assert_eq!(first_difference(&a, &b), None, "{:?}", s.technique);
    }
}

#[test]
fn the_tuning_at_its_default_is_the_same_bits_and_a_table_moves_the_pitch() {
    let s = ArcusSettings::default();
    let base = render(&s, &phrase(), 40_000, &|_| 256, &Serial);
    let mut f = Arcus::new(SR);
    f.set(&s);
    f.set_tuning(&[0.0; 128]);
    let mut l = vec![0.0; 40_000];
    let mut r = vec![0.0; 40_000];
    let score = phrase();
    let mut at = 0;
    let mut ev = 0;
    while at < 40_000 {
        while ev < score.len() && score[ev].0 <= at {
            apply(&mut f, score[ev].1);
            ev += 1;
        }
        let next = score.get(ev).map_or(40_000, |e| e.0);
        let n = 256.min(next.max(at + 1) - at).min(40_000 - at);
        f.process(&mut l[at..at + n], &mut r[at..at + n]);
        at += n;
    }
    assert_eq!(first_difference(&base, &(l, r)), None);
    // A key a keyboard map leaves unmapped plays nothing.
    let mut f = Arcus::new(SR);
    let mut t = [0.0f32; 128];
    t[50] = f32::NAN;
    f.set_tuning(&t);
    f.note_on(50, 0.8, 1);
    assert!(f.at_rest());
}

#[test]
fn a_keyboard_alone_is_never_silent() {
    // The softest velocity and a controller pulled to zero still sound (pp, never niente).
    let s = ArcusSettings::default();
    for score in [
        vec![(0, Ev::On(50, 1.0 / 127.0))],
        vec![(0, Ev::Cc(0, 0.0)), (10, Ev::On(50, 0.5))],
        vec![(0, Ev::Cc(1, 0.0)), (10, Ev::On(50, 0.5))],
    ] {
        let (l, _) = render(&s, &score, 48_000, &|_| 256, &Serial);
        let level = rms(&l[24_000..]);
        assert!(level > 1e-4, "{score:?}: {level}");
    }
}

/// The frequency of the partial near `near` Hz over `from…to` (s), as Fides's tests
/// measure it (`fides_tests.rs`): the signal heterodyned to `near`, two boxcars of four of
/// its periods, the unwrapped phase's slope fitted by least squares.
pub(super) fn measure_hz(x: &[f32], near: f64, from: f64, to: f64) -> f64 {
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
    let from = from.max(2.0 * len as f64 / sr + 0.02);
    let (a, b) = ((from * sr) as usize, ((to * sr) as usize).min(x.len()));
    let (mut ts, mut ph) = (Vec::new(), Vec::new());
    let (mut last, mut unwrap) = (0.0f64, 0.0f64);
    for i in (a..b).step_by(16) {
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

pub(super) fn cents(f: f64, want: f64) -> f64 {
    1200.0 * math::log2(f / want)
}

#[test]
fn a_plucked_note_is_within_a_tenth_of_a_cent() {
    let mut worst = (0f64, String::new());
    for inst in [
        Instrument::Cello,
        Instrument::Bass,
        Instrument::Violin,
        Instrument::Viola,
    ] {
        let spec = inst.spec();
        let s = ArcusSettings {
            instrument: inst,
            technique: Technique::Pizzicato,
            body_mix: 0.0,
            pizz_decay: 3.0,
            ..ArcusSettings::default()
        };
        let lo = spec.bottom();
        for key in [lo, lo + 5, lo + 12, lo + 19, lo + 26, spec.top - 6] {
            let (l, _) = render(&s, &[(0, Ev::On(key, 0.8))], 60_000, &|_| 256, &Serial);
            let want = 440.0 * math::exp2((key as f64 - 69.0) / 12.0);
            let e = cents(measure_hz(&l, want, 0.15, 1.2), want);
            if e.abs() > worst.0 {
                worst = (e.abs(), format!("{inst:?} key {key}: {e:+.4} ct"));
            }
        }
    }
    println!("worst plucked: {}", worst.1);
    assert!(worst.0 < 0.1, "{}", worst.1);
}

/// A project's tuning reaches Arcus (ADR 0155): Werckmeister III in E flat at A415, as the
/// track's table hands it (semitones from 12-TET at A440) — the plucked pitch within 0.1 cent
/// of the table's, the bowed within the bowed gate's cent.
#[test]
fn werckmeister_iii_at_415_moves_the_measured_pitch_by_the_table() {
    // Werckmeister III's deviations from equal temperament on C, cents (the standard table).
    const W3: [f64; 12] = [
        0.0, -9.775, -7.82, -5.865, -9.775, -1.955, -11.73, -3.91, -7.82, -11.73, -3.91, -7.82,
    ];
    let shift = 12.0 * math::log2(415.0f64 / 440.0);
    let mut table = [0.0f32; 128];
    for (k, t) in table.iter_mut().enumerate() {
        *t = (W3[(k + 12 - 3) % 12] / 100.0 + shift) as f32;
    }
    let play = |tech: Technique, key: u8| {
        let s = ArcusSettings {
            technique: tech,
            vib_depth: 0.0,
            body_mix: 0.0,
            pizz_decay: 3.0,
            ..ArcusSettings::default()
        };
        let mut f = Arcus::new(SR);
        f.set(&s);
        f.set_tuning(&table);
        f.note_on(key, 0.6, 1);
        let frames = 72_000;
        let (mut l, mut r) = (vec![0.0; frames], vec![0.0; frames]);
        f.process(&mut l, &mut r);
        let want = 440.0 * math::exp2((key as f64 + table[key as usize] as f64 - 69.0) / 12.0);
        cents(measure_hz(&l, want, 0.5, 1.45), want)
    };
    for key in [43u8, 50, 55, 57, 62] {
        let p = play(Technique::Pizzicato, key);
        let b = play(Technique::Arco, key);
        println!("key {key}: plucked {p:+.3} ct, bowed {b:+.3} ct from the table's pitch");
        assert!(p.abs() < 0.1, "plucked {key}: {p}");
        assert!(b.abs() <= 1.0, "bowed {key}: {b}");
    }
}

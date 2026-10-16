//! Chorda behaves as a grand does, measured on the sound: the half pedal is continuous,
//! the strings decay in two stages, the soft pedal softens, the strings ring in sympathy
//! with the pedal down; and 64 voices with the pedal down cost less than a quarter of one
//! core at 256-frame blocks.

use super::tests_engine::{keyboard, perform, piano, Ev};
use super::*;

const FS: f64 = 48_000.0;

/// The level (dB) of `x` in windows of `win` frames.
fn envelope(x: &[f32], win: usize) -> Vec<f64> {
    x.chunks(win)
        .map(|c| {
            let ms = c.iter().map(|v| (*v as f64) * (*v as f64)).sum::<f64>() / c.len() as f64;
            10.0 * crate::math::log10(ms.max(1e-30))
        })
        .collect()
}

/// The decay rate (dB/s) of `x` between `a` and `b` seconds, by least squares on its
/// 20 ms envelope.
fn decay_db_per_s(x: &[f32], a: f64, b: f64) -> f64 {
    let win = (0.02 * FS) as usize;
    let env = envelope(x, win);
    let pts: Vec<(f64, f64)> = env
        .iter()
        .enumerate()
        .map(|(i, d)| ((i as f64 + 0.5) * win as f64 / FS, *d))
        .filter(|(t, _)| *t >= a && *t <= b)
        .collect();
    let n = pts.len() as f64;
    let (mt, md) = (
        pts.iter().map(|p| p.0).sum::<f64>() / n,
        pts.iter().map(|p| p.1).sum::<f64>() / n,
    );
    let num: f64 = pts.iter().map(|(t, d)| (t - mt) * (d - md)).sum();
    let den: f64 = pts.iter().map(|(t, _)| (t - mt) * (t - mt)).sum();
    -num / den
}

/// Middle C struck, released after half a second with the pedal held at p = 0, 0.1 … 1:
/// the decay (dB/s) that follows, from fully damped to free.
fn release_decays(sympathy: f64) -> Vec<f64> {
    (0..=10)
        .map(|step| {
            let p = step as f32 / 10.0;
            let mut c = piano();
            // The strings' decay, not the soundboard's ring: no body.
            let s = ChordaSettings {
                sympathetic: sympathy,
                body: 0.0,
                ..ChordaSettings::default()
            };
            c.set(&s, 0.0);
            let ev = [
                (0, Ev::Pedal(p)),
                (1, Ev::On(60, 0.7, 1)),
                (24_000, Ev::Off(60, 1)),
            ];
            let (l, _) = perform(&mut c, &ev, (1.2 * FS) as usize, 256, 1);
            decay_db_per_s(&l, 0.6, 1.1)
        })
        .collect()
}

#[test]
fn half_pedalling_is_continuous() {
    // The dampers alone (no sympathetic strings): the more pedal, never the more damping,
    // and no step between two states.
    let rates = release_decays(0.0);
    eprintln!("decay dB/s by pedal, dampers alone: {rates:.1?}");
    for w in rates.windows(2) {
        assert!(w[1] <= w[0] + 0.5, "more pedal never damps more: {rates:?}");
    }
    let range = rates[0] - rates[10];
    assert!(range > 30.0, "the pedal matters: {range} dB/s");
    let step = |r: &[f64]| {
        r.windows(2)
            .map(|w| (w[0] - w[1]).abs())
            .fold(0.0, f64::max)
    };
    assert!(
        step(&rates) < 0.4 * range,
        "a step of {} of {range}",
        step(&rates)
    );
    // With the strings ringing in sympathy as well: still no step.
    let full = release_decays(1.0);
    eprintln!("decay dB/s by pedal, with sympathy: {full:.1?}");
    let range = full[0] - full[10];
    assert!(
        step(&full) < 0.4 * range,
        "a step of {} of {range}",
        step(&full)
    );
}

#[test]
fn re_pedalling_leaves_no_discontinuity() {
    // Held, pedal down, key up, pedal up (the dampers fall), pedal down again before the
    // note has gone: the waveform's steepest step never jumps at the pedal's moves.
    let ev = [
        (1, Ev::On(55, 0.8, 1)),
        (12_000, Ev::Pedal(1.0)),
        (18_000, Ev::Off(55, 1)),
        (30_000, Ev::Pedal(0.0)),
        (33_000, Ev::Pedal(1.0)),
        (40_000, Ev::Pedal(0.4)),
    ];
    let (l, _) = perform(&mut piano(), &ev, 60_000, 256, 1);
    let steepest = |a: usize, b: usize| {
        l[a..b]
            .windows(2)
            .map(|w| (w[1] - w[0]).abs())
            .fold(0.0f32, f32::max)
    };
    let before = steepest(6_000, 12_000);
    for at in [12_000, 30_000, 33_000, 40_000] {
        let around = steepest(at - 64, at + 2_000);
        assert!(around <= 1.2 * before, "at {at}: {around} against {before}");
    }
}

#[test]
fn three_strings_decay_in_two_stages() {
    // Middle C held: the prompt sound falls faster than the aftersound (Weinreich).
    let ev = [(1, Ev::On(60, 0.8, 1))];
    let (l, _) = perform(&mut piano(), &ev, (8.0 * FS) as usize, 512, 1);
    let prompt = decay_db_per_s(&l, 0.2, 1.5);
    let after = decay_db_per_s(&l, 4.0, 7.5);
    eprintln!("prompt {prompt:.1} dB/s, aftersound {after:.1} dB/s");
    assert!(prompt > 1.4 * after, "{prompt} against {after}");
}

#[test]
fn the_soft_pedal_is_softer_and_darker() {
    let level_and_bright = |soft: f32| {
        let ev = [(0, Ev::Soft(soft)), (1, Ev::On(64, 0.8, 1))];
        let (l, _) = perform(&mut piano(), &ev, 24_000, 256, 1);
        let rms = l
            .iter()
            .map(|v| (*v as f64) * (*v as f64))
            .sum::<f64>()
            .sqrt();
        let diff = l
            .windows(2)
            .map(|w| ((w[1] - w[0]) as f64) * ((w[1] - w[0]) as f64))
            .sum::<f64>()
            .sqrt();
        (rms, diff / rms)
    };
    let (open, bright_open) = level_and_bright(0.0);
    let (soft, bright_soft) = level_and_bright(1.0);
    assert!(soft < 0.85 * open, "{soft} against {open}");
    assert!(
        bright_soft < bright_open,
        "{bright_soft} against {bright_open}"
    );
}

#[test]
fn the_strings_ring_in_sympathy_with_the_pedal_down() {
    // C3 struck and released: with the pedal down the other strings ring with it (the
    // difference with no sympathy is loud); with it up, only the undamped top strings do.
    let run = |pedal: f32, sympathy: f64| {
        let mut c = piano();
        let s = ChordaSettings {
            sympathetic: sympathy,
            ..ChordaSettings::default()
        };
        c.set(&s, 0.0);
        let ev = [
            (0, Ev::Pedal(pedal)),
            (1, Ev::On(48, 0.9, 1)),
            (6_000, Ev::Off(48, 1)),
        ];
        perform(&mut c, &ev, 48_000, 256, 1).0
    };
    let energy = |a: &[f32], b: &[f32]| {
        a.iter()
            .zip(b)
            .map(|(x, y)| ((x - y) as f64) * ((x - y) as f64))
            .sum::<f64>()
    };
    let total = |a: &[f32]| a.iter().map(|x| (*x as f64) * (*x as f64)).sum::<f64>();
    let down = run(1.0, 1.0);
    let down_dry = run(1.0, 0.0);
    let up = run(0.0, 1.0);
    let up_dry = run(0.0, 0.0);
    let with_pedal = 10.0 * crate::math::log10(energy(&down, &down_dry) / total(&down_dry));
    let without = 10.0 * crate::math::log10(energy(&up, &up_dry).max(1e-30) / total(&up_dry));
    eprintln!("sympathy: {with_pedal:.1} dB with the pedal, {without:.1} dB without");
    assert!(with_pedal > -40.0, "{with_pedal}");
    assert!(
        without < with_pedal - 10.0,
        "{without} against {with_pedal}"
    );
}

/// 64 voices sounding with the pedal down, 256-frame blocks, one thread: the time it takes
/// against the time it plays. ADR 0059's target is a quarter of one core.
///
/// Measured as the best of 48 stretches of a quarter second (47 blocks; 12 s of audio, as
/// the three 4-second runs before): a preempted stretch only takes longer, so on a loaded machine
/// the quickest is the instrument's own cost (fix6: under a full test run beside a Linux
/// one even the best 4-second run was preempted past the bound). Wall time, the same on
/// every system.
fn sixty_four_pedal_down_voices() -> f64 {
    let mut c = piano();
    let mut ev = vec![(0usize, Ev::Pedal(1.0))];
    for (i, k) in (30u8..94).enumerate() {
        ev.push((1 + i * 37, Ev::On(k, 0.7, i as u64 + 1)));
    }
    // A stretch: 47 blocks of 256 (12 032 samples), a quarter second rounded up to a
    // whole block; 48 of them, 12.03 s of audio.
    let blocks = ((0.25 * FS) as usize).div_ceil(256);
    assert_eq!(blocks, 47);
    let seconds = (blocks * 256) as f64 / FS;
    // Warm up: every voice struck and ringing.
    perform(&mut c, &ev, 4_096, 256, 1);
    assert_eq!(c.active_voices(), 64);
    let (mut l, mut r) = (vec![0.0f32; 256], vec![0.0f32; 256]);
    let mut best = f64::INFINITY;
    for _ in 0..48 {
        let t = std::time::Instant::now();
        for _ in 0..blocks {
            l.fill(0.0);
            r.fill(0.0);
            c.process(&mut l, &mut r);
        }
        best = best.min(t.elapsed().as_secs_f64() / seconds);
    }
    assert!(
        c.active_voices() >= 60,
        "still sounding: {}",
        c.active_voices()
    );
    best
}

#[test]
fn sixty_four_pedal_down_voices_fit_a_quarter_of_a_core() {
    let load = sixty_four_pedal_down_voices();
    eprintln!("64 pedal-down voices: {:.1} % of one core", load * 100.0);
    // A test build on a shared machine: a generous bound here; the release figure is
    // measured by the ignored test below and written in ADR 0059.
    assert!(load < 0.6, "{load}");
}

#[test]
#[ignore]
fn measure_sixty_four_pedal_down_voices() {
    let load = sixty_four_pedal_down_voices();
    eprintln!("64 pedal-down voices: {:.1} % of one core", load * 100.0);
    assert!(load < 0.25, "{load}");
    let _ = keyboard();
}

fn energy_db(x: &[f32]) -> f64 {
    10.0 * crate::math::log10(
        x.iter()
            .map(|v| (*v as f64) * (*v as f64))
            .sum::<f64>()
            .max(1e-30),
    )
}

fn with(f: impl FnOnce(&mut ChordaSettings)) -> Chorda {
    let mut c = piano();
    let mut s = ChordaSettings::default();
    f(&mut s);
    c.set(&s, 0.0);
    c
}

#[test]
fn a_struck_string_leaves_no_ghost_in_its_sympathetic_string() {
    // Pedal down, C3 rings C4's string in sympathy; C4 is struck off the grid, then both
    // keys and the pedal let go: once the voices have died the tail only falls — the
    // sympathetic string of C4 was silenced when its voice took over, never replayed.
    let ev = [
        (0, Ev::Pedal(1.0)),
        (1, Ev::On(48, 0.9, 1)),
        (9_013, Ev::On(60, 0.8, 2)),
        (20_000, Ev::Off(48, 1)),
        (20_000, Ev::Off(60, 2)),
        (20_000, Ev::Pedal(0.0)),
    ];
    let mut c = with(|s| s.body = 0.0);
    let (l, _) = perform(&mut c, &ev, (2.5 * FS) as usize, 256, 1);
    let env = envelope(&l, 480);
    let from = 30_000 / 480;
    for w in env[from..].windows(2) {
        assert!(
            w[1] <= w[0] + 3.0,
            "a burst in the tail: {:.1?}",
            &env[from..]
        );
    }
}

#[test]
fn without_dampers_the_strings_ring_in_sympathy() {
    // The dampers' grip at 0 (no dampers): with the pedal up the strings ring along as
    // with it down — the sympathetic strings' dampers follow the setting too.
    let run = |sympathy: f64| {
        let mut c = with(|s| {
            s.voice.damper = 0.0;
            s.sympathetic = sympathy;
        });
        let ev = [(1, Ev::On(48, 0.9, 1)), (6_000, Ev::Off(48, 1))];
        perform(&mut c, &ev, 48_000, 256, 1).0
    };
    let (wet, dry) = (run(1.0), run(0.0));
    let diff: Vec<f32> = wet.iter().zip(&dry).map(|(a, b)| a - b).collect();
    let rel = energy_db(&diff) - energy_db(&dry);
    eprintln!("sympathy without dampers: {rel:.1} dB");
    assert!(rel > -25.0, "{rel}");
}

#[test]
fn a_new_decay_reaches_a_ringing_note_struck_again() {
    // Pedal down, C4 rings; the decay drops to a third and C4 is struck again: its tail
    // falls faster than with the decay left alone.
    let tail = |change: bool| {
        let mut c = piano();
        let ev = [(0, Ev::Pedal(1.0)), (1, Ev::On(60, 0.7, 1))];
        perform(&mut c, &ev, 12_000, 256, 1);
        if change {
            let s = ChordaSettings {
                voice: super::voice::VoiceSettings {
                    decay: 0.3,
                    ..ChordaSettings::default().voice
                },
                ..ChordaSettings::default()
            };
            c.set(&s, 0.0);
        }
        let (l, _) = perform(&mut c, &[(0, Ev::On(60, 0.7, 2))], 96_000, 256, 1);
        energy_db(&l[72_000..])
    };
    let (same, shorter) = (tail(false), tail(true));
    eprintln!("tail energy: {same:.1} dB as it was, {shorter:.1} dB with a third of the decay");
    assert!(shorter < same - 10.0, "{shorter} against {same}");
}

#[test]
fn a_fresh_strike_finds_its_damper_already_lifted() {
    // The key's travel lifts the damper before the hammer lands: a key struck from rest has
    // its damper off the strings from the first sample, never gliding off them.
    let mut c = piano();
    c.note_on(86, 0.8, 1);
    assert_eq!(c.damper_of(86), 0.0, "struck from rest");
    perform(&mut c, &[(2_400, Ev::Off(86, 1))], 96_000, 256, 1);
    assert_eq!(c.active_voices(), 0, "the first note has died");
    assert_eq!(c.damper_of(86), 1.0, "and its damper is down");
    c.note_on(86, 0.8, 2);
    assert_eq!(c.damper_of(86), 0.0, "struck again from rest");
}

#[test]
fn a_new_keyboard_never_cuts_the_strings_ringing() {
    // Pedal down, a chord rings; a keyboard of another tuning arrives: no step in the
    // waveform where it lands, and a key struck again keeps ringing.
    let other = std::sync::Arc::new(super::design::Keyboard::compute(
        super::design::Tuning {
            a4: 442.0,
            stretch: 1.0,
            stiffness: 1.0,
        },
        FS,
        8,
    ));
    let mut c = piano();
    let ev = [
        (0, Ev::Pedal(1.0)),
        (1, Ev::On(48, 0.8, 1)),
        (1, Ev::On(55, 0.8, 2)),
        (1, Ev::On(64, 0.8, 3)),
    ];
    let (a, _) = perform(&mut c, &ev, 24_000, 256, 1);
    c.set_keyboard(other);
    let (b, _) = perform(&mut c, &[(0, Ev::On(55, 0.5, 4))], 4_800, 256, 1);
    let steepest = |x: &[f32]| {
        x.windows(2)
            .map(|w| (w[1] - w[0]).abs())
            .fold(0.0f32, f32::max)
    };
    let before = steepest(&a[18_000..]);
    let joint = (b[0] - a[a.len() - 1]).abs();
    assert!(
        joint <= 1.2 * before,
        "a step of {joint} where the keyboard landed ({before})"
    );
    let level = |x: &[f32]| energy_db(x) - 10.0 * crate::math::log10(x.len() as f64);
    assert!(
        level(&b[..480]) > level(&a[a.len() - 480..]) - 3.0,
        "the chord still rings: {} against {}",
        level(&b[..480]),
        level(&a[a.len() - 480..])
    );
}

/// Sound review 2: a keyboard that lands while the piano plays reaches every part of it.
/// The sympathetic strings the pedal woke (driven while anything sounds) fade out over a
/// few milliseconds and take the new layout; a key struck again while it still rings
/// fades its old strings and strikes the new ones. Before, both kept the old tuning for as
/// long as anything sounded.
#[test]
fn a_keyboard_landing_while_the_piano_plays_reaches_the_ringing_strings() {
    let flat = std::sync::Arc::new(super::design::Keyboard::compute(
        super::design::Tuning {
            a4: 415.0,
            stretch: 1.0,
            stiffness: 1.0,
        },
        FS,
        8,
    ));
    let mut c = piano();
    let ev = [
        (0, Ev::Pedal(1.0)),
        (1, Ev::On(48, 0.8, 1)),
        (1, Ev::On(55, 0.8, 2)),
        (1, Ev::On(64, 0.8, 3)),
    ];
    perform(&mut c, &ev, 24_000, 256, 1);
    c.set_keyboard(flat);
    assert!(
        c.sympathetic_waiting() > 0,
        "the pedal's strings wait for it"
    );
    // Still playing: a key held throughout drives the bridge; 55 struck again ringing.
    let (b, _) = perform(
        &mut c,
        &[(0, Ev::On(72, 0.6, 4)), (100, Ev::On(55, 0.6, 5))],
        24_000,
        256,
        1,
    );
    assert_eq!(
        c.sympathetic_waiting(),
        0,
        "every string took the new keyboard"
    );
    assert_eq!(
        c.voice_generation(55),
        c.generation(),
        "the key struck again plays the new keyboard"
    );
    // Nothing cut: no step anywhere in what followed.
    let steepest = b
        .windows(2)
        .map(|w| (w[1] - w[0]).abs())
        .fold(0.0f32, f32::max);
    let peak = b.iter().fold(0.0f32, |m, x| m.max(x.abs()));
    assert!(steepest < 0.25 * peak, "a step of {steepest} (peak {peak})");
}

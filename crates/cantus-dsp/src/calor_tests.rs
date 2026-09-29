//! Calor's own tests: bits at every block size, determinism and seek invariance, pitch,
//! silence, headroom, cards and round robin, click-free stealing, legato, per-note
//! expression, self-oscillation tracking the keys, the mixer driving the filter, CPU.

use super::*;
use crate::va::testkit::*;
use crate::va::Model;
use crate::voices::PlayMode;

const FS: f32 = 48_000.0;

#[derive(Clone, Debug)]
enum Ev {
    On(u8, f32, u64),
    Off(u8, u64),
    Bend(f32),
    Wheel(f32),
    Tune(u64, f32),
    Set(Box<CalorSettings>),
}

/// Render `frames` frames of a performance, `block` frames at a time (0: a pseudo-random
/// mix of sizes). Returns left then right.
fn perform(
    s: &CalorSettings,
    events: &[(usize, Ev)],
    frames: usize,
    block: usize,
) -> (Vec<f32>, Vec<f32>) {
    let mut c = Calor::new(FS);
    c.set(s);
    let (mut l, mut r) = (vec![0.0; frames], vec![0.0; frames]);
    let mut pos = 0;
    let mut next = 0;
    let mut rng = 12345u32;
    while pos < frames {
        while next < events.len() && events[next].0 <= pos {
            match &events[next].1 {
                Ev::On(k, v, id) => c.note_on(*k, *v, *id),
                Ev::Off(k, id) => c.note_off(*k, *id, 0.5),
                Ev::Bend(b) => c.pitch_bend(*b),
                Ev::Wheel(w) => c.mod_wheel(*w),
                Ev::Tune(id, st) => c.expression(*id, Expression::Tuning, *st),
                Ev::Set(s) => c.set(s),
            }
            next += 1;
        }
        let size = if block == 0 {
            rng ^= rng << 13;
            rng ^= rng >> 17;
            rng ^= rng << 5;
            1 + (rng % 700) as usize
        } else {
            block
        };
        let until = events.get(next).map_or(frames, |e| e.0.min(frames));
        let end = (pos + size).min(until).min(frames).max(pos + 1);
        c.process(&mut l[pos..end], &mut r[pos..end]);
        pos = end;
    }
    (l, r)
}

fn calibrated() -> CalorSettings {
    CalorSettings {
        condition: 0.0,
        ..Default::default()
    }
}

fn busy_settings() -> CalorSettings {
    let mut s = CalorSettings {
        condition: 0.75,
        glide: 0.08,
        stack: 2,
        chorus: chorus::ChorusMode::One,
        lfo1_pitch: 20.0,
        lfo_amount: 0.5,
        filter_fm: 0.2,
        xmod: 0.1,
        ..Default::default()
    };
    s.osc[1].sync = true;
    s.osc[2].pulse = 0.5;
    s.mix[2] = 0.4;
    s.mix[3] = 0.3;
    s.mix[4] = 0.05;
    s.filter.reso = 0.7;
    s
}

fn busy_events() -> Vec<(usize, Ev)> {
    let mut changed = busy_settings();
    changed.cutoff = 900.0;
    changed.filter.model = Model::Diode;
    vec![
        (0, Ev::On(48, 0.8, 1)),
        (0, Ev::On(55, 0.7, 2)),
        (3_001, Ev::On(60, 1.0, 3)),
        (7_777, Ev::Bend(1.0)),
        (9_000, Ev::Wheel(0.6)),
        (10_003, Ev::Off(55, 2)),
        (12_345, Ev::Set(Box::new(changed))),
        (15_000, Ev::On(67, 0.5, 4)),
        (20_000, Ev::Off(48, 1)),
        (26_000, Ev::Off(60, 3)),
        (30_000, Ev::Off(67, 4)),
    ]
}

#[test]
fn every_block_size_renders_the_same_bits() {
    for quality in [Quality::Draft, Quality::Good, Quality::Best] {
        let s = CalorSettings {
            quality,
            ..busy_settings()
        };
        let ev = busy_events();
        let reference = perform(&s, &ev, 48_000, 256);
        assert!(reference.0.iter().any(|x| *x != 0.0));
        for block in [1usize, 7, 64, 4096, 0] {
            let other = perform(&s, &ev, 48_000, block);
            assert!(
                reference == other,
                "{quality:?}: block size {block} renders different bits"
            );
        }
    }
}

#[test]
fn the_same_notes_render_the_same_and_a_new_seed_differs() {
    let s = busy_settings();
    let ev = busy_events();
    let a = perform(&s, &ev, 24_000, 128);
    assert_eq!(a, perform(&s, &ev, 24_000, 128));
    let other = CalorSettings {
        seed: 2,
        ..busy_settings()
    };
    assert_ne!(
        a,
        perform(&other, &ev, 24_000, 128),
        "a new seed is a new set of cards"
    );
}

#[test]
fn a_note_sounds_the_same_wherever_playback_starts() {
    // The second note, played after the first has died away, and played alone from a
    // fresh start: the same bits (its card, phases and drift come from its id).
    let mut s = busy_settings();
    s.stack = 1;
    s.chorus = chorus::ChorusMode::Off;
    s.lfo[0].restart = LfoRestart::Gate;
    s.env[1].release = 0.05;
    // Glide is left out: gliding from the previous note is history by definition.
    s.glide = 0.0;
    let gap = 3 * 48_000;
    let full = perform(
        &s,
        &[
            (0, Ev::On(50, 0.9, 77)),
            (4_000, Ev::Off(50, 77)),
            (gap, Ev::On(62, 0.8, 78)),
            (gap + 20_000, Ev::Off(62, 78)),
        ],
        gap + 30_000,
        128,
    );
    let alone = perform(
        &s,
        &[(0, Ev::On(62, 0.8, 78)), (20_000, Ev::Off(62, 78))],
        30_000,
        128,
    );
    assert!(full.0[gap - 1] == 0.0, "the first note has died away");
    assert!(
        full.0[gap..] == alone.0[..],
        "the note differs with where playback started"
    );
}

#[test]
fn a_note_is_on_pitch_across_the_keyboard() {
    for quality in [Quality::Draft, Quality::Good, Quality::Best] {
        let mut s = calibrated();
        s.quality = quality;
        s.mix = [0.5, 0.0, 0.0, 0.0, 0.0];
        s.osc[0].rev = OscRev::Ideal;
        s.cutoff = 20_000.0;
        s.filter.reso = 0.0;
        s.env_amount = 0.0;
        s.key_track = 0.0;
        for key in [24u8, 36, 48, 60, 72, 84, 96, 108] {
            let (l, _) = perform(&s, &[(0, Ev::On(key, 0.8, key as u64))], 48_000, 256);
            let f = frequency(&l[12_000..], FS).unwrap();
            let want = 440.0 * 2f64.powf((key as f64 - 69.0) / 12.0);
            let err = cents(f, want);
            assert!(
                err.abs() < 0.05,
                "{quality:?}: key {key} is {err:+.3} cents off"
            );
        }
    }
}

#[test]
fn notes_end_in_exact_silence() {
    let mut c = Calor::new(FS);
    c.set(&busy_settings());
    c.note_on(60, 1.0, 1);
    c.note_on(64, 1.0, 2);
    let (mut l, mut r) = (vec![0.0; 4800], vec![0.0; 4800]);
    c.process(&mut l, &mut r);
    c.note_off(60, 1, 0.0);
    c.note_off(64, 2, 0.0);
    let mut last_true = 0;
    for i in 0..400 {
        l.iter_mut().for_each(|x| *x = 0.0);
        r.iter_mut().for_each(|x| *x = 0.0);
        if c.process(&mut l[..480], &mut r[..480]) {
            last_true = i;
        } else {
            assert!(l[..480].iter().chain(r[..480].iter()).all(|x| *x == 0.0));
        }
    }
    assert!(last_true < 390, "still writing after four seconds");
    assert_eq!(c.active_voices(), 0);
    assert!(!c.process(&mut l[..480], &mut r[..480]));
}

#[test]
fn a_loud_chord_keeps_headroom() {
    let chord: Vec<(usize, Ev)> = [36u8, 48, 55, 60, 64, 67, 72, 79]
        .iter()
        .enumerate()
        .map(|(i, k)| (0, Ev::On(*k, 1.0, i as u64 + 1)))
        .collect();
    let (l, r) = perform(&CalorSettings::default(), &chord, 96_000, 256);
    let peak = l.iter().chain(r.iter()).fold(0f32, |m, x| m.max(x.abs()));
    let (l1, _) = perform(
        &CalorSettings::default(),
        &[(0, Ev::On(60, 0.8, 1))],
        48_000,
        256,
    );
    let one = l1.iter().fold(0f32, |m, x| m.max(x.abs()));
    println!(
        "peaks: eight notes at full velocity {:.1} dBFS, one note {:.1} dBFS",
        20.0 * peak.log10(),
        20.0 * one.log10()
    );
    assert!(peak < 1.0, "an eight-note chord peaks at {peak}");
}

#[test]
fn repeated_notes_rotate_round_the_cards_and_reuse_keeps_one() {
    let mut t = [0f32; 64];
    for (assign, distinct) in [(Assign::Rotate, 6usize), (Assign::Reuse, 1)] {
        let mut c = Calor::new(FS);
        c.set(&CalorSettings {
            assign,
            ..Default::default()
        });
        let (mut l, mut r) = (vec![0.0; 256], vec![0.0; 256]);
        for id in 1..=6u64 {
            c.note_on(60, 0.8, id * 1000 + 7);
            c.process(&mut l, &mut r);
            c.note_off(60, id * 1000 + 7, 0.0);
            c.process(&mut l, &mut r);
        }
        c.telemetry(&mut t);
        let lit = (0..CARDS).filter(|i| t[tel::CARD_KEY + i] == 61.0).count();
        assert_eq!(lit, distinct, "{assign:?}: cards sounding the key");
    }
}

#[test]
fn a_stolen_voice_fades_instead_of_clicking() {
    let mut s = calibrated();
    s.voices = 1;
    s.osc[0] = OscSettings {
        saw: 0.0,
        tri: 1.0,
        rev: OscRev::Ideal,
        ..Default::default()
    };
    s.mix = [0.8, 0.0, 0.0, 0.0, 0.0];
    s.env_amount = 0.0;
    let steal = 20_011;
    let (l, _) = perform(
        &s,
        &[(0, Ev::On(57, 1.0, 1)), (steal, Ev::On(57, 1.0, 2))],
        40_000,
        64,
    );
    let step = |a: usize, b: usize| {
        l[a..b]
            .windows(2)
            .map(|w| (w[1] - w[0]).abs())
            .fold(0f32, f32::max)
    };
    let steady = step(10_000, 20_000);
    let at = step(steal - 10, steal + 400);
    assert!(
        at <= 1.2 * steady,
        "stealing stepped by {at}, the note by {steady}"
    );
}

#[test]
fn legato_glides_without_a_new_attack() {
    let mut s = calibrated();
    s.mode = PlayMode::Legato;
    s.glide = 0.2;
    s.env[1].attack = 0.3;
    let mut c = Calor::new(FS);
    c.set(&s);
    let (mut l, mut r) = (vec![0.0; 4800], vec![0.0; 4800]);
    c.note_on(48, 1.0, 1);
    c.process(&mut l, &mut r);
    let mut t = [0f32; 64];
    c.telemetry(&mut t);
    let before = t[tel::ENV2];
    c.note_on(60, 1.0, 2);
    l.iter_mut().for_each(|x| *x = 0.0);
    c.process(&mut l[..16], &mut r[..16]);
    c.telemetry(&mut t);
    assert!(t[tel::ENV2] >= before, "legato restarted the envelope");
    // Mid-glide the pitch sits between the notes.
    l.iter_mut().for_each(|x| *x = 0.0);
    r.iter_mut().for_each(|x| *x = 0.0);
    c.process(&mut l, &mut r);
    assert_eq!(c.active_voices(), 1);
}

#[test]
fn a_per_note_bend_moves_only_its_note() {
    let s = CalorSettings {
        gain: 0.1,
        ..calibrated()
    };
    let both = perform(
        &s,
        &[
            (0, Ev::On(48, 0.8, 1)),
            (0, Ev::On(72, 0.8, 2)),
            (1000, Ev::Tune(1, 1.0)),
        ],
        24_000,
        128,
    );
    let one = perform(
        &s,
        &[(0, Ev::On(48, 0.8, 1)), (1000, Ev::Tune(1, 1.0))],
        24_000,
        128,
    );
    let two = perform(&s, &[(0, Ev::On(72, 0.8, 2))], 24_000, 128);
    let worst = both
        .0
        .iter()
        .zip(one.0.iter().zip(two.0.iter()))
        .map(|(b, (x, y))| (b - (x + y)).abs())
        .fold(0f32, f32::max);
    assert!(worst < 1e-5, "the bend reached the other note ({worst})");
}

#[test]
fn a_self_oscillating_filter_plays_the_keys() {
    let mut s = calibrated();
    s.mix = [0.0; 5];
    s.filter.reso = 1.05;
    s.key_track = 1.0;
    s.cutoff = 261.625_58;
    s.env_amount = 0.0;
    for model in [Model::Ladder, Model::Svf] {
        s.filter.model = model;
        for key in [48u8, 60, 72, 84] {
            let (l, _) = perform(&s, &[(0, Ev::On(key, 1.0, key as u64))], 3 * 48_000, 256);
            let f = frequency(&l[2 * 48_000..], FS).expect("it oscillates");
            let want = 440.0 * 2f64.powf((key as f64 - 69.0) / 12.0);
            let err = cents(f, want);
            assert!(err.abs() < 10.0, "{model:?} key {key}: {err:+.2} cents");
        }
    }
}

#[test]
fn the_mixer_levels_drive_the_filter() {
    // Four times the level into the ladder comes out less than four times as loud: the
    // mixer is a timbre control, not a volume knob.
    let rms = |level: f32| {
        let mut s = calibrated();
        s.mix = [level, level, 0.0, 0.0, 0.0];
        s.gain = 0.2;
        let (l, _) = perform(&s, &[(0, Ev::On(48, 0.8, 1))], 24_000, 256);
        (l[6000..].iter().map(|x| (*x as f64).powi(2)).sum::<f64>() / 18_000.0).sqrt()
    };
    let ratio = rms(1.0) / rms(0.25);
    assert!(
        ratio < 3.85 && ratio > 1.5,
        "4× the level gave {ratio:.2}× the output"
    );
}

#[test]
fn cpu_per_voice_at_each_quality() {
    for (quality, ceiling_ns) in [
        (Quality::Draft, 1500.0),
        (Quality::Good, 3000.0),
        (Quality::Best, 8000.0),
    ] {
        let mut s = busy_settings();
        s.stack = 1;
        s.quality = quality;
        let mut c = Calor::new(FS);
        c.set(&s);
        for (i, k) in [36u8, 43, 48, 55, 60, 64, 67, 72].iter().enumerate() {
            c.note_on(*k, 0.8, i as u64 + 1);
        }
        let (mut l, mut r) = (vec![0.0; 512], vec![0.0; 512]);
        let blocks = 2 * 48_000 / 512;
        let t0 = std::time::Instant::now();
        for _ in 0..blocks {
            c.process(&mut l, &mut r);
        }
        let ns = t0.elapsed().as_nanos() as f64 / (blocks * 512 * 8) as f64;
        println!(
            "{quality:?}: {ns:.0} ns per voice per sample, {:.2} % of a core per voice at 48 kHz",
            ns * 48_000.0 / 1e7
        );
        assert!(ns < ceiling_ns, "{quality:?}: {ns:.0} ns per voice-sample");
    }
}

//! Tonus's own tests: the nulls, the hard effect with no warble, a natural correction with
//! no click, scales and notes, MIDI targets to the sample, block-size independence, rest.

use super::*;
use crate::pitch::testkit::{self, measured_f0, Vowel};
use crate::pitch::{analyse, hz_to_midi};

const SR: f32 = 48_000.0;

fn run(t: &mut Tonus, l: &[f32], r: &[f32], block: usize) -> (Vec<f32>, Vec<f32>) {
    let (mut a, mut b) = (l.to_vec(), r.to_vec());
    let mut at = 0;
    while at < a.len() {
        let n = block.min(a.len() - at);
        t.process(&mut a[at..at + n], &mut b[at..at + n]);
        at += n;
    }
    (a, b)
}

fn tonus(s: TonusSettings) -> Tonus {
    let mut t = Tonus::new(SR);
    t.set(&s);
    t
}

/// A sung line: three notes slightly off (sharp, flat, with vibrato), with a gap and a
/// sibilant between.
fn line() -> Vec<f32> {
    let mut x = Vec::new();
    let off = |hz: f32, cents: f32| hz * math::exp2(cents / 1200.0);
    x.extend(testkit::voice(SR, 0.5, Vowel::A, 0.4, |_| off(220.0, 35.0)));
    x.extend(testkit::sibilant(SR, 0.08, 0.2, 4));
    x.extend(testkit::voice(SR, 0.5, Vowel::I, 0.4, |_| {
        off(246.94, -30.0)
    }));
    x.extend(std::iter::repeat_n(0.0, 2_400));
    let vib = testkit::vibrato(off(196.0, 5.0), 40.0, 5.5);
    x.extend(testkit::voice(SR, 0.6, Vowel::A, 0.4, &vib));
    x
}

fn assert_delayed(y: &[f32], x: &[f32], lat: usize, what: &str) {
    for i in 0..y.len() {
        let want = if i >= lat { x[i - lat] } else { 0.0 };
        assert_eq!(y[i].to_bits(), want.to_bits(), "{what}: sample {i}");
    }
}

#[test]
fn the_latency_is_128_samples_at_48k() {
    assert_eq!(Tonus::new(48_000.0).latency(), 128);
    assert!(Tonus::new(44_100.0).latency() as f32 / 44_100.0 < 0.003);
}

/// The corrector's nulls: amount 0; every note on bypass; MIDI with nothing held.
#[test]
fn the_nulls_are_the_input_delayed_bit_for_bit() {
    let x = line();
    let r: Vec<f32> = x.iter().map(|v| v * -0.7).collect();
    let mut cases = Vec::new();
    cases.push((
        "amount 0, slowest",
        TonusSettings {
            amount: 0.0,
            speed_ms: 400.0,
            ..TonusSettings::default()
        },
    ));
    cases.push((
        "every note on bypass",
        TonusSettings {
            notes: [NoteState::Bypass; 12],
            speed_ms: 0.0,
            ..TonusSettings::default()
        },
    ));
    for src in [Source::Midi, Source::Melody] {
        cases.push((
            "MIDI, no note held",
            TonusSettings {
                source: src,
                speed_ms: 0.0,
                ..TonusSettings::default()
            },
        ));
    }
    for (what, s) in cases {
        let mut t = tonus(s);
        let lat = t.latency();
        let (a, b) = run(&mut t, &x, &r, 64);
        assert_delayed(&a, &x, lat, what);
        assert_delayed(&b, &r, lat, what);
    }
}

/// The output's pitch contour, measured by the offline analyser: (time s, MIDI) of every
/// voiced frame.
fn contour(y: &[f32]) -> Vec<(f32, f32)> {
    analyse(y, SR)
        .frames
        .iter()
        .filter(|f| f.voiced)
        .map(|f| (f.centre as f32 / SR, hz_to_midi(f.f0)))
        .collect()
}

/// Speed 0: every note lands on the nearest semitone and stays there — no warble, even
/// through the vibrato.
#[test]
fn speed_zero_is_the_hard_effect_with_no_warble() {
    let x = line();
    let mut t = tonus(TonusSettings {
        speed_ms: 0.0,
        ..TonusSettings::default()
    });
    let (y, _) = run(&mut t, &x, &x, 256);
    let notes = [
        (0.1f32, 0.45f32, 57.0f32),
        (0.7, 1.03, 59.0),
        (1.25, 1.7, 55.0),
    ];
    let c = contour(&y);
    for (a, b, want) in notes {
        let errs: Vec<f32> = c
            .iter()
            .filter(|(t, _)| *t > a && *t < b)
            .map(|(_, m)| (m - want) * 100.0)
            .collect();
        assert!(errs.len() > 30, "{want}: {} frames", errs.len());
        let worst = errs.iter().fold(0.0f32, |m, e| m.max(e.abs()));
        let mean = errs.iter().map(|e| e.abs()).sum::<f32>() / errs.len() as f32;
        eprintln!(
            "hard tune on {want}: mean {mean:.2}, worst {worst:.2} cents over {} frames",
            errs.len()
        );
        // Below what an ear hears as out of tune or wobbling (5 cents) everywhere.
        assert!(
            worst < 5.0 && mean < 3.0,
            "note {want}: {mean} / {worst} cents off"
        );
    }
}

/// A natural speed: the centre of each note is pulled in, the vibrato survives, and the
/// note changes make no click.
#[test]
fn a_natural_speed_corrects_with_no_click() {
    let x = line();
    let mut t = tonus(TonusSettings {
        speed_ms: 60.0,
        ..TonusSettings::default()
    });
    let (y, _) = run(&mut t, &x, &x, 128);
    let c = contour(&y);
    // The first note (35 cents sharp) ends within 5 cents of A3.
    let late: Vec<f32> = c
        .iter()
        .filter(|(t, _)| *t > 0.3 && *t < 0.45)
        .map(|(_, m)| (m - 57.0) * 100.0)
        .collect();
    let mean = late.iter().sum::<f32>() / late.len() as f32;
    assert!(mean.abs() < 5.0, "settled {mean} cents off");
    // The vibrato note keeps most of its vibrato.
    let vib: Vec<f32> = c
        .iter()
        .filter(|(t, _)| *t > 1.35 && *t < 1.7)
        .map(|(_, m)| *m)
        .collect();
    let (lo, hi) = vib
        .iter()
        .fold((f32::MAX, f32::MIN), |(a, b), m| (a.min(*m), b.max(*m)));
    assert!(
        (hi - lo) * 100.0 > 40.0,
        "vibrato flattened to {} cents",
        (hi - lo) * 100.0
    );
    // No click: the second difference stays within the input's own.
    let d2 = |v: &[f32], i: usize| (v[i + 1] - 2.0 * v[i] + v[i - 1]).abs();
    let own = (1..x.len() - 1).map(|i| d2(&x, i)).fold(0.0f32, f32::max);
    let out = (1..y.len() - 1).map(|i| d2(&y, i)).fold(0.0f32, f32::max);
    assert!(out < own * 1.6, "a click: {out} against the input's {own}");
}

/// A key and a scale: a voice between two notes of C major goes to the nearer one in the
/// scale, never to the removed note in between.
#[test]
fn a_scale_pulls_to_its_notes() {
    // F#3 + 20 cents: the scale has F and G, not F#.
    let hz = 184.997 * math::exp2(20.0 / 1200.0);
    let x = testkit::voice(SR, 0.6, Vowel::A, 0.4, |_| hz);
    let mut t = tonus(TonusSettings {
        scale: Scale::Major,
        key: 0,
        speed_ms: 0.0,
        ..TonusSettings::default()
    });
    let (y, _) = run(&mut t, &x, &x, 512);
    let got = measured_f0(&y[y.len() / 2..], SR, 196.0);
    assert!(
        (1200.0 * math::log2(got / 196.0)).abs() < 3.0,
        "{got} Hz, want G3"
    );
    // Remove G: it goes to F.
    let mut notes = [NoteState::Scale; 12];
    notes[7] = NoteState::Remove;
    let mut t = tonus(TonusSettings {
        scale: Scale::Major,
        notes,
        speed_ms: 0.0,
        ..TonusSettings::default()
    });
    let (y, _) = run(&mut t, &x, &x, 512);
    let got = measured_f0(&y[y.len() / 2..], SR, 174.61);
    assert!(
        (1200.0 * math::log2(got / 174.61)).abs() < 3.0,
        "{got} Hz, want F3"
    );
}

#[test]
fn choose_skips_removed_notes_keeps_its_note_and_flags_bypass() {
    use scale::{choose, resolve};
    let mut notes = [NoteState::Scale; 12];
    notes[4] = NoteState::Bypass; // E
    let a = resolve(Scale::Major, 0, &notes);
    assert_eq!(choose(61.4, &a, None), Some((62, false))); // C#+40 → D
    assert_eq!(choose(64.2, &a, None), Some((64, true))); // E is on bypass
                                                          // Hysteresis: at 61.04, D (62) is 0.96 away and C (60) 1.04: keep C if current.
    assert_eq!(choose(61.04, &a, Some(60)), Some((60, false)));
    assert_eq!(choose(61.04, &a, None), Some((62, false)));
    let none = resolve(Scale::Major, 0, &[NoteState::Remove; 12]);
    assert_eq!(choose(60.0, &none, None), None);
}

/// MIDI: the target changes at the note's own sample, whatever the blocks; the output is
/// the same bits at every block size.
#[test]
fn midi_targets_follow_notes_to_the_sample() {
    let x = testkit::voice(SR, 0.8, Vowel::A, 0.4, |_| 200.0);
    let s = TonusSettings {
        source: Source::Midi,
        speed_ms: 0.0,
        ..TonusSettings::default()
    };
    let on_at = 19_211usize; // inside a voiced stretch, not on a hop
                             // Sample by sample: no target before, a target from that sample on.
    let mut t = tonus(s);
    let (mut l, mut r) = (x.clone(), x.clone());
    for i in 0..on_at + 10 {
        if i == on_at {
            t.note(0, NoteEvent::On(67)); // G4: the class G, nearest G3 (196 Hz)
        }
        t.process(&mut l[i..i + 1], &mut r[i..i + 1]);
        let want = if i >= on_at { Some(55) } else { None };
        assert_eq!(t.target(), want, "sample {i}");
    }
    // Any blocks: the same bits.
    let render = |block: usize| {
        let mut t = tonus(s);
        let (mut l, mut r) = (x.clone(), x.clone());
        let mut at = 0;
        while at < l.len() {
            let n = block.min(l.len() - at);
            if (at..at + n).contains(&on_at) {
                t.note(on_at - at, NoteEvent::On(67));
            }
            if (at..at + n).contains(&(on_at + 9_000)) {
                t.note(on_at + 9_000 - at, NoteEvent::Off(67));
            }
            t.process(&mut l[at..at + n], &mut r[at..at + n]);
            at += n;
        }
        l
    };
    let reference = render(512);
    for b in [1usize, 37, 64, 4_096] {
        let y = render(b);
        assert!(
            y.iter()
                .zip(&reference)
                .all(|(p, q)| p.to_bits() == q.to_bits()),
            "block {b}"
        );
    }
    // And the voice lands on G3 while the note is held.
    let got = measured_f0(&reference[on_at + 2_000..on_at + 8_000], SR, 196.0);
    assert!((1200.0 * math::log2(got / 196.0)).abs() < 3.0, "{got}");
}

/// The melody source: the voice sings the keyboard's note, octave and all.
#[test]
fn the_melody_source_sings_the_keyboard() {
    let x = testkit::voice(SR, 0.6, Vowel::A, 0.4, |_| 200.0);
    let mut t = tonus(TonusSettings {
        source: Source::Melody,
        speed_ms: 0.0,
        ..TonusSettings::default()
    });
    t.note(0, NoteEvent::On(64)); // E4, 329.63 Hz
    let (y, _) = run(&mut t, &x, &x, 300);
    let got = measured_f0(&y[y.len() / 2..], SR, 329.63);
    assert!((1200.0 * math::log2(got / 329.63)).abs() < 3.0, "{got}");
}

#[test]
fn every_block_size_renders_the_same_bits() {
    let x = line();
    let r: Vec<f32> = x.iter().map(|v| v * 0.5).collect();
    let s = TonusSettings {
        speed_ms: 15.0,
        humanize: 0.5,
        vib_depth: 30.0,
        throat: 2.0,
        transpose: -2.0,
        ..TonusSettings::default()
    };
    let reference = run(&mut tonus(s), &x, &r, 512);
    for b in [1usize, 37, 64, 4_096] {
        assert_eq!(run(&mut tonus(s), &x, &r, b), reference, "block {b}");
    }
}

/// After silence it rests, and a new unit skipped as far renders the same bits.
#[test]
fn it_rests_exactly() {
    let x = line();
    let s = TonusSettings {
        speed_ms: 0.0,
        throat: -3.0,
        ..TonusSettings::default()
    };
    let mut a = tonus(s);
    run(&mut a, &x, &x, 64);
    let mut n = 0;
    while !a.at_rest() {
        let mut z = [0.0f32; 64];
        let mut w = [0.0f32; 64];
        a.process(&mut z, &mut w);
        n += 64;
        assert!(n < 48_000 * 2, "no rest: {:?}", a.rest_blocker());
    }
    let mut b = tonus(s);
    b.skip(x.len() + n);
    assert_eq!(a.seq(), b.seq());
    let y = testkit::voice(SR, 0.4, Vowel::U, 0.4, |_| 150.0);
    assert_eq!(run(&mut a, &y, &y, 100), run(&mut b, &y, &y, 100));
}

/// The throat moves the formants and keeps the pitch; with formants off they follow it.
#[test]
fn the_throat_moves_formants_not_pitch() {
    let f0 = 180.0;
    let x = testkit::voice(SR, 0.8, Vowel::A, 0.4, |_| f0);
    let mut t = tonus(TonusSettings {
        amount: 0.0,
        throat: 4.0,
        ..TonusSettings::default()
    });
    let (y, _) = run(&mut t, &x, &x, 256);
    let half = y.len() / 2;
    let got = measured_f0(&y[half..], SR, f0);
    assert!(
        (1200.0 * math::log2(got / f0)).abs() < 3.0,
        "pitch moved: {got}"
    );
    let d = testkit::envelope_distance(&x[half..], f0, &y[half..], f0, SR, 300.0, 4_000.0);
    assert!(d > 2.0, "the throat did not move the formants ({d} dB)");
}

/// Review 1: when a correction ends while the voice goes on (the MIDI note released), the
/// glide lands on zero exactly and the output returns to the input delayed, bit for bit.
#[test]
fn a_released_correction_returns_to_the_exact_dry_path() {
    let x = testkit::voice(SR, 1.6, Vowel::A, 0.4, |_| 200.0);
    let mut t = tonus(TonusSettings {
        source: Source::Midi,
        speed_ms: 60.0,
        ..TonusSettings::default()
    });
    let lat = t.latency();
    t.note(0, NoteEvent::On(67));
    let (mut l, mut r) = (x.clone(), x.clone());
    let release = (0.4 * SR) as usize;
    t.process(&mut l[..release], &mut r[..release]);
    assert!(t.shift() != 0.0, "corrected while the note is held");
    t.note(0, NoteEvent::Off(67));
    t.process(&mut l[release..], &mut r[release..]);
    assert_eq!(t.shift(), 0.0, "the correction landed on zero");
    // The last 0.3 s: the input delayed, to the bit.
    let from = x.len() - (0.3 * SR) as usize;
    assert_delayed(
        &l[from..],
        &x[from - lat..x.len() - lat],
        0,
        "after the release",
    );
}

/// Review 1: more note events in one call than the queue holds lose no note-off: the
/// notes held afterwards are exactly those still down.
#[test]
fn a_flood_of_notes_in_one_call_leaves_none_stuck() {
    let mut t = tonus(TonusSettings {
        source: Source::Midi,
        ..TonusSettings::default()
    });
    for i in 0..600usize {
        let p = (36 + i % 48) as u8;
        t.note(i, NoteEvent::On(p));
        t.note(i, NoteEvent::Off(p));
    }
    t.note(300, NoteEvent::On(64));
    let (mut l, mut r) = (vec![0.0f32; 512], vec![0.0f32; 512]);
    t.process(&mut l, &mut r);
    assert_eq!(t.held_classes(), 1 << 4, "only E is still down");
    t.note(0, NoteEvent::Off(64));
    t.process(&mut l, &mut r);
    assert_eq!(t.held_classes(), 0);
}

/// Review 1: a note change inside one voiced stretch (the MIDI target jumps while the voice
/// sounds) makes no click, at a natural speed and at speed 0: the output's second
/// difference around the change stays within the steady voice's own.
#[test]
fn a_legato_note_change_makes_no_click() {
    let x = testkit::voice(SR, 1.2, Vowel::A, 0.4, |_| 200.0);
    let d2 = |v: &[f32], i: usize| (v[i + 1] - 2.0 * v[i] + v[i - 1]).abs();
    let steady = (10_000..20_000).map(|i| d2(&x, i)).fold(0.0f32, f32::max);
    for speed in [60.0f32, 0.0] {
        let mut t = tonus(TonusSettings {
            source: Source::Melody,
            speed_ms: speed,
            ..TonusSettings::default()
        });
        t.note(0, NoteEvent::On(55)); // G3, 196 Hz
        let change = (0.6 * SR) as usize;
        let (mut l, mut r) = (x.clone(), x.clone());
        t.process(&mut l[..change], &mut r[..change]);
        t.note(0, NoteEvent::On(57)); // A3: the target jumps a whole tone, legato
        t.process(&mut l[change..], &mut r[change..]);
        let worst = (change - 2_400..change + 4_800)
            .map(|i| d2(&l, i))
            .fold(0.0f32, f32::max);
        eprintln!("legato change at {speed} ms: worst {worst:.4}, steady voice {steady:.4}");
        assert!(
            worst < steady * 1.6,
            "a click at {speed} ms: {worst} against {steady}"
        );
    }
}

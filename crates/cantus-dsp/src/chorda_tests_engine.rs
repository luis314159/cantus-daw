//! Chorda the instrument: the family's rules (silence, block size, threads, rest) and the
//! piano's behaviour measured on the sound (half-pedalling, two-stage decay, una corda,
//! sympathy).

use super::design::{Keyboard, Tuning};
use super::*;
use crate::par::Reversed;
use std::sync::{Arc, OnceLock};

/// The reference keyboard at 48 kHz, designed once for the whole test binary.
pub fn keyboard() -> Arc<Keyboard> {
    static K: OnceLock<Arc<Keyboard>> = OnceLock::new();
    K.get_or_init(|| Arc::new(Keyboard::compute(Tuning::default(), 48_000.0, 8)))
        .clone()
}

pub fn piano() -> Chorda {
    let mut c = Chorda::new(48_000.0);
    c.set_keyboard(keyboard());
    c
}

/// What happens when: `(frame, event)`.
#[derive(Clone, Copy, Debug)]
pub enum Ev {
    On(u8, f32, u64),
    Off(u8, u64),
    Pedal(f32),
    Soft(f32),
}

/// Render `frames` frames of `events`, `block` frames at a time (events land at their own
/// frame: blocks are cut there), on `threads` (Reversed) when > 1.
pub fn perform(
    c: &mut Chorda,
    events: &[(usize, Ev)],
    frames: usize,
    block: usize,
    threads: usize,
) -> (Vec<f32>, Vec<f32>) {
    let (mut l, mut r) = (vec![0.0f32; frames], vec![0.0f32; frames]);
    let mut at = 0;
    let mut next = 0;
    while at < frames {
        while next < events.len() && events[next].0 <= at {
            match events[next].1 {
                Ev::On(k, v, id) => c.note_on(k, v, id),
                Ev::Off(k, id) => c.note_off(k, id),
                Ev::Pedal(p) => c.set_pedal(p),
                Ev::Soft(s) => c.set_soft(s),
            }
            next += 1;
        }
        let stop = events.get(next).map_or(frames, |e| e.0.min(frames));
        let end = (at + block).min(stop).max(at + 1).min(frames);
        if threads > 1 {
            c.process_par(&mut l[at..end], &mut r[at..end], &Reversed(threads));
        } else {
            c.process(&mut l[at..end], &mut r[at..end]);
        }
        at = end;
    }
    (l, r)
}

fn phrase() -> Vec<(usize, Ev)> {
    vec![
        (0, Ev::On(48, 0.8, 1)),
        (100, Ev::On(60, 0.6, 2)),
        (2_000, Ev::On(64, 0.5, 3)),
        (2_001, Ev::Pedal(0.6)),
        (5_000, Ev::On(67, 0.9, 4)),
        (9_000, Ev::Off(60, 2)),
        (12_000, Ev::Soft(1.0)),
        (12_500, Ev::On(72, 0.7, 5)),
        (15_000, Ev::Pedal(0.0)),
        (15_001, Ev::Off(48, 1)),
        (18_000, Ev::Off(64, 3)),
        (19_000, Ev::On(100, 1.0, 6)),
    ]
}

#[test]
fn silence_renders_nothing_and_is_at_rest() {
    let mut c = piano();
    assert!(c.at_rest());
    let (mut l, mut r) = (vec![0.0; 4_096], vec![0.0; 4_096]);
    assert!(!c.process(&mut l, &mut r));
    assert!(l.iter().chain(&r).all(|x| *x == 0.0));
}

#[test]
fn every_block_size_and_any_number_of_threads_give_the_same_bits() {
    let ev = phrase();
    let frames = 24_000;
    let reference = perform(&mut piano(), &ev, frames, 512, 1);
    assert!(reference.0.iter().any(|x| *x != 0.0));
    for block in [1, 37, 64, 512] {
        let got = perform(&mut piano(), &ev, frames, block, 1);
        assert!(got == reference, "block {block}");
    }
    let par = perform(&mut piano(), &ev, frames, 512, 4);
    assert!(par == reference, "threads");
}

/// Short notes that end inside a block (a damped key dies where it dies, not where a block
/// ends), an undamped top key that wakes the sympathetic strings with the pedal up, and
/// pedal moves on odd frames.
fn staccato() -> Vec<(usize, Ev)> {
    let mut ev = vec![(0, Ev::On(96, 1.0, 1)), (3, Ev::On(40, 0.9, 2))];
    for i in 0..12usize {
        let at = 211 + i * 1_013;
        let key = 50 + (i * 5 % 40) as u8;
        ev.push((at, Ev::On(key, 0.4 + 0.05 * i as f32, 10 + i as u64)));
        ev.push((at + 377, Ev::Off(key, 10 + i as u64)));
        if i % 4 == 1 {
            ev.push((at + 501, Ev::Pedal(0.3 * (i % 3) as f32)));
        }
    }
    ev.push((700, Ev::Off(40, 2)));
    ev.sort_by_key(|e| e.0);
    ev
}

/// The bench's chord: 32 keys struck together, released at 12 000.
fn chord() -> Vec<(usize, Ev)> {
    let key = |v: usize| 36 + ((v * 7) % 60) as u8;
    let mut ev: Vec<(usize, Ev)> = (0..32)
        .map(|v| (0, Ev::On(key(v), 96.0 / 127.0, 100 + v as u64)))
        .collect();
    ev.extend((0..32).map(|v| (12_000, Ev::Off(key(v), 100 + v as u64))));
    ev
}

#[test]
fn notes_and_pedals_inside_a_block_give_the_same_bits_at_any_split() {
    for (name, ev) in [("staccato", staccato()), ("chord", chord())] {
        let frames = 16_000;
        let reference = perform(&mut piano(), &ev, frames, 512, 1);
        for block in [1, 7, 37, 300] {
            let got = perform(&mut piano(), &ev, frames, block, 1);
            assert!(got == reference, "{name}, block {block}");
        }
        let par = perform(&mut piano(), &ev, frames, 7, 4);
        assert!(par == reference, "{name}, threads, blocks of 7");
    }
}

#[test]
fn a_released_note_dies_and_the_instrument_comes_to_rest() {
    let mut c = piano();
    let ev = [(0, Ev::On(84, 1.0, 1)), (2_400, Ev::Off(84, 1))];
    perform(&mut c, &ev, 48_000 * 4, 512, 1);
    assert_eq!(c.active_voices(), 0, "its damper killed it");
    // The undamped top strings ring on in sympathy for a few seconds (their upper partials
    // longest): rest comes within ten.
    let (mut l, mut r) = (vec![0.0; 4_800], vec![0.0; 4_800]);
    let mut waited = 0;
    while !c.at_rest() && waited < 100 {
        c.process(&mut l, &mut r);
        waited += 1;
    }
    eprintln!("at rest {:.1} s after the note", 4.0 + waited as f64 * 0.1);
    assert!(
        c.at_rest(),
        "and the board's tail has gone to exact zero: {:?} {:?}",
        c.busy_parts(),
        c.board_largest()
    );
}

/// One sympathetic string, freed by the pedal and driven by 50 ms of noise at the bridge:
/// what it rings with afterwards (its force), mono.
fn sympathetic_ring(key: u8) -> (Vec<f64>, super::tables::Key) {
    let kb = keyboard();
    let fs = kb.fs;
    let caps: Vec<usize> = kb
        .keys
        .iter()
        .map(|k| (1.33 * fs / k.partial(1)) as usize + 64)
        .collect();
    let mut sym = super::board::Sympathetic::new(&caps, &kb.keys, fs);
    sym.tune(&kb.keys, &kb.sympathetic);
    // Only this key's string sounds: every other is skipped as if its voice sounded.
    let others: Vec<bool> = kb.keys.iter().map(|k| k.key != key).collect();
    sym.set_dampers(1.0, &others);
    let n = (1.5 * fs) as usize;
    let mut seed = 0x1234_5678u64;
    let drive: Vec<f64> = (0..n)
        .map(|i| {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            if i < (0.05 * fs) as usize {
                ((seed >> 11) as f64 / (1u64 << 53) as f64 - 0.5) * 100.0
            } else {
                0.0
            }
        })
        .collect();
    let mut out = vec![0.0; n];
    for (d, o) in drive.chunks(64).zip(out.chunks_mut(64)) {
        sym.process(d, o);
    }
    let k = *kb.keys.iter().find(|k| k.key == key).unwrap();
    (out, k)
}

#[test]
fn a_sympathetic_string_rings_at_its_keys_own_pitch() {
    // The keys' own strings at their speaking pitch (ADR 0059 §3): never an octave below,
    // never odd partials only.
    for key in [48u8, 60, 72, 100] {
        let (x, k) = sympathetic_ring(key);
        let fs = keyboard().fs;
        let f1 = super::design::target_hz(k.f0(), k.b, 1);
        let at = |hz: f64| super::testkit::partial_level(&x, fs, hz, 0.2);
        let (first, half, second) = (
            at(f1),
            at(0.5 * f1),
            at(2.0 * k.f0() * (1.0 + 4.0 * k.b).sqrt()),
        );
        eprintln!("key {key}: f1 {first:.1} dB, f1/2 {half:.1} dB, f2 {second:.1} dB");
        assert!(
            first > half + 20.0,
            "key {key}: {first} at f1 against {half} at f1/2"
        );
        assert!(
            second > half + 20.0,
            "key {key}: the even partial is there: {second}"
        );
        let hz = super::testkit::partial_hz(&x, fs, f1, 0.03 * f1, (0.2 * fs) as usize, 16_384);
        let cents = 1200.0 * crate::math::log2(hz / f1);
        assert!(
            cents.abs() < 5.0,
            "key {key}: {hz:.2} Hz against {f1:.2} ({cents:+.1} ct)"
        );
    }
}

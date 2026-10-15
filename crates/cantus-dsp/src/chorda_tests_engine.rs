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

#[test]
fn a_released_note_dies_and_the_instrument_comes_to_rest() {
    let mut c = piano();
    let ev = [(0, Ev::On(84, 1.0, 1)), (2_400, Ev::Off(84, 1))];
    perform(&mut c, &ev, 48_000 * 4, 512, 1);
    assert_eq!(c.active_voices(), 0, "its damper killed it");
    let (mut l, mut r) = (vec![0.0; 48_000], vec![0.0; 48_000]);
    c.process(&mut l, &mut r);
    assert!(
        c.at_rest(),
        "and the board's tail has gone to exact zero: {:?} {:?}",
        c.busy_parts(),
        c.board_largest()
    );
}

//! Lapsus's factory bank: thirty-six patterns in six rows of six (ADR 0132, provisional).
//! Owned by Lapsus. Forever once shipped: a project that never edited a slot plays these.
//!
//! The time curve's value is the delay in pattern lengths (0: as it arrives). The arithmetic
//! of the classics, so each is what its name says:
//!
//! - **A tape stop** is a constant deceleration: speed `1 − u` over the stop, so the delay is
//!   `u²/2` of the stop's span — a segment bent to `u²` (tension 1/7: the automation editor's
//!   power `1 + 7·t` is 2) rising by half its width, which ends at speed zero exactly on its
//!   last point. A tape head (and a stylus) reads a level proportional to its speed, so the
//!   volume falls on a line to nothing with it. **A tape start** is the mirror: `1 − (1−u)²`.
//! - **Half time** is a line at half the 45° slope: the first half of the cycle at half speed.
//! - **A repeat** is a held step: from its point on, the delay is how far back the repeated
//!   slice began.
//! - **Reverse** is a slope of 2: the read runs back as fast as the song runs on.
//! - **A scratch** is S-curves back and forth: an S of tension 0.15 (power 2.05) across a
//!   rise of its own width runs the record at −1.05× in its middle, and back down at 3.05×.

use super::{Pattern, Point, SHAPE_HOLD, SHAPE_S, SLOTS};

/// The tension that bends a segment to `u²` (power `1 + 7·t` = 2).
const TAPE: f32 = 1.0 / 7.0;
/// An S-curve's tension for the scratches: power 2.05, close to a sine's turn.
const SCRATCH: f32 = 0.15;

const QUARTER: u8 = 0;
const HALF: u8 = 1;
const BAR: u8 = 2;

fn pat(len: u8, time: Vec<Point>, vol: Vec<Point>) -> Pattern {
    Pattern {
        len,
        time,
        vol,
        name: None,
    }
}

fn p(x: f32, y: f32) -> Point {
    Point::new(x, y)
}

/// A held step.
fn h(x: f32, y: f32) -> Point {
    Point::new(x, y).shape(SHAPE_HOLD)
}

/// An S-curve from here, at the scratches' tension.
fn s(x: f32, y: f32) -> Point {
    Point::new(x, y).shape(SHAPE_S).bend(SCRATCH)
}

/// Steps of `n` across the cycle, each held at `f(k)`.
fn steps(n: usize, f: impl Fn(usize) -> f32) -> Vec<Point> {
    (0..n).map(|k| h(k as f32 / n as f32, f(k))).collect()
}

/// The thirty-six, in slot order.
pub(super) fn factory() -> [Pattern; SLOTS] {
    let scratch = vec![s(0.0, 0.0), s(0.25, 0.25), s(0.5, 0.0), s(0.75, 0.25)];
    let bank = vec![
        // ---- Row 1: stops and starts.
        // 1 Through: nothing moves.
        pat(BAR, vec![], vec![]),
        // 2 Tape stop: the whole bar, ending still on the next bar's line.
        pat(
            BAR,
            vec![p(0.0, 0.0).bend(TAPE), p(1.0, 0.5)],
            vec![p(0.0, 1.0), p(1.0, 0.0)],
        ),
        // 3 Tape stop over the last half bar.
        pat(
            BAR,
            vec![p(0.0, 0.0), p(0.5, 0.0).bend(TAPE), p(1.0, 0.25)],
            vec![p(0.0, 1.0), p(0.5, 1.0), p(1.0, 0.0)],
        ),
        // 4 Tape stop over the last beat.
        pat(
            BAR,
            vec![p(0.0, 0.0), p(0.75, 0.0).bend(TAPE), p(1.0, 0.125)],
            vec![p(0.0, 1.0), p(0.75, 1.0), p(1.0, 0.0)],
        ),
        // 5 Tape start: the first beat spins up from still.
        pat(
            BAR,
            vec![p(0.0, 0.0).bend(-TAPE), p(0.25, 0.125), p(1.0, 0.125)],
            vec![p(0.0, 0.0), p(0.25, 1.0), p(1.0, 1.0)],
        ),
        // 6 Stop and start: up in the first beat, down in the last.
        pat(
            BAR,
            vec![
                p(0.0, 0.0).bend(-TAPE),
                p(0.25, 0.125),
                p(0.75, 0.125).bend(TAPE),
                p(1.0, 0.25),
            ],
            vec![p(0.0, 0.0), p(0.25, 1.0), p(0.75, 1.0), p(1.0, 0.0)],
        ),
        // ---- Row 2: speed.
        // 7 Half time over the bar (an octave down, the first half bar).
        pat(BAR, vec![p(0.0, 0.0), p(1.0, 0.5)], vec![]),
        // 8 Half time every half bar.
        pat(HALF, vec![p(0.0, 0.0), p(1.0, 0.5)], vec![]),
        // 9 Half time every beat.
        pat(QUARTER, vec![p(0.0, 0.0), p(1.0, 0.5)], vec![]),
        // 10 Quarter time: two octaves down.
        pat(BAR, vec![p(0.0, 0.0), p(1.0, 0.75)], vec![]),
        // 11 Slow down: from full speed to half, steadily.
        pat(BAR, vec![p(0.0, 0.0).bend(TAPE), p(1.0, 0.25)], vec![]),
        // 12 Wobble: a warped record, ±28 cents twice a bar at 120 BPM.
        pat(
            HALF,
            vec![
                Point::new(0.0, 0.0).shape(SHAPE_S).bend(SCRATCH),
                Point::new(0.5, 0.004).shape(SHAPE_S).bend(SCRATCH),
            ],
            vec![],
        ),
        // ---- Row 3: repeats.
        // 13 The last beat repeats the third.
        pat(BAR, vec![h(0.0, 0.0), h(0.75, 0.25)], vec![]),
        // 14 Every beat's first eighth, twice.
        pat(QUARTER, vec![h(0.0, 0.0), h(0.5, 0.5)], vec![]),
        // 15 Every beat's first sixteenth, four times.
        pat(QUARTER, steps(4, |k| k as f32 / 4.0), vec![]),
        // 16 Roll build: a beat, then eighths, sixteenths and thirty-seconds.
        pat(BAR, {
            let mut v = vec![h(0.0, 0.0), h(0.375, 0.125), h(0.5, 0.0)];
            v.extend((1..4).map(|k| h(0.5 + k as f32 / 16.0, k as f32 / 16.0)));
            v.push(h(0.75, 0.0));
            v.extend((1..8).map(|k| h(0.75 + k as f32 / 32.0, k as f32 / 32.0)));
            v
        }, vec![]),
        // 17 Beat repeat: the second beat again, twice, in the second half.
        pat(BAR, vec![h(0.0, 0.0), h(0.5, 0.25), h(0.75, 0.5)], vec![]),
        // 18 Gated stutter: the first sixteenth eight times, cut in thirty-seconds.
        pat(
            HALF,
            steps(8, |k| k as f32 / 8.0),
            steps(16, |k| if k % 2 == 0 { 1.0 } else { 0.0 }),
        ),
        // ---- Row 4: reverse.
        // 19 The second half bar runs the first back.
        pat(BAR, vec![p(0.0, 0.0), p(0.5, 0.0), p(1.0, 1.0)], vec![]),
        // 20 Every second beat runs the one before back.
        pat(HALF, vec![p(0.0, 0.0), p(0.5, 0.0), p(1.0, 1.0)], vec![]),
        // 21 The last beat runs the third back.
        pat(BAR, vec![p(0.0, 0.0), p(0.75, 0.0), p(1.0, 0.5)], vec![]),
        // 22 Rewind: the last eighth winds back, faster and faster (to −3×).
        pat(
            BAR,
            vec![p(0.0, 0.0), p(0.875, 0.0).bend(TAPE), p(1.0, 0.25)],
            vec![p(0.0, 1.0), p(0.875, 1.0), p(1.0, 0.3)],
        ),
        // 23 Reverse swell: the first half bar back, rising out of silence.
        pat(
            BAR,
            vec![p(0.0, 0.0), p(0.5, 0.0), p(1.0, 1.0)],
            vec![h(0.0, 1.0), p(0.5, 0.0).bend(0.4), p(1.0, 1.0)],
        ),
        // 24 Backspin: the last beat spun back hard, coming to rest on the bar.
        pat(
            BAR,
            vec![p(0.0, 0.0), p(0.75, 0.0).bend(-0.1), p(1.0, 0.75)],
            vec![p(0.0, 1.0), p(0.75, 1.0).bend(0.3), p(1.0, 0.0)],
        ),
        // ---- Row 5: scratch (a beat each).
        // 25 Baby scratch: back and forth on every eighth.
        pat(QUARTER, scratch.clone(), vec![]),
        // 26 Chirp: the fader closes at the end of every stroke.
        pat(
            QUARTER,
            scratch.clone(),
            vec![
                h(0.0, 1.0),
                h(0.2, 0.0),
                h(0.25, 1.0),
                h(0.45, 0.0),
                h(0.5, 1.0),
                h(0.7, 0.0),
                h(0.75, 1.0),
                h(0.95, 0.0),
            ],
        ),
        // 27 Transformer: the strokes cut in thirty-seconds.
        pat(
            QUARTER,
            scratch,
            steps(8, |k| if k % 2 == 0 { 1.0 } else { 0.0 }),
        ),
        // 28 Tear: the back stroke in two pushes, the record held between, then released.
        pat(
            QUARTER,
            vec![
                s(0.0, 0.0),
                p(0.125, 0.15),
                s(0.25, 0.275),
                s(0.375, 0.425),
                p(0.75, 0.0),
            ],
            vec![],
        ),
        // 29 Scribble: a fast tremble of the hand on the record.
        pat(
            QUARTER,
            (0..8).map(|k| s(k as f32 / 8.0, if k % 2 == 1 { 0.02 } else { 0.0 })).collect(),
            vec![],
        ),
        // 30 Drag: the record held back by the hand, then let go.
        pat(BAR, vec![s(0.0, 0.0), s(0.5, 0.2)], vec![]),
        // ---- Row 6: volume.
        // 31 Trance gate in sixteenths.
        pat(
            BAR,
            vec![],
            steps(16, |k| {
                const BITS: [u8; 16] = [1, 0, 1, 1, 0, 1, 1, 0, 1, 1, 0, 1, 1, 0, 1, 0];
                f32::from(BITS[k])
            }),
        ),
        // 32 Off the beat: only the second eighth of every beat.
        pat(QUARTER, vec![], vec![h(0.0, 0.0), h(0.5, 1.0)]),
        // 33 Pump: the sidechain's duck on every beat, without a key.
        pat(
            QUARTER,
            vec![],
            vec![p(0.0, 0.0).bend(-0.6), h(0.5, 1.0)],
        ),
        // 34 Fade in over the bar, again every bar.
        pat(BAR, vec![], vec![p(0.0, 0.0).bend(0.3), p(1.0, 1.0)]),
        // 35 Chop: half time, cut in sixteenths.
        pat(
            HALF,
            vec![p(0.0, 0.0), p(1.0, 0.5)],
            steps(8, |k| if k % 2 == 0 { 1.0 } else { 0.0 }),
        ),
        // 36 Echo repeat: the bar's first sixteenth sixteen times, each quieter.
        pat(
            BAR,
            steps(16, |k| k as f32 / 16.0),
            steps(16, |k| (0..k).fold(1.0f32, |a, _| a * 0.88)),
        ),
    ];
    match bank.try_into() {
        Ok(b) => b,
        Err(v) => panic!("the factory bank has {} patterns, not 36", v.len()),
    }
}

//! Lapsus's factory banks (ADR 0132, provisional): thirty-six **time** patterns and
//! thirty-six **volume** patterns, each bank in six rows of six. Owned by Lapsus. Forever
//! once shipped: a project that never edited a slot plays these.
//!
//! A time curve's value is the delay in pattern lengths (0: as it arrives). The arithmetic
//! of the classics, so each is what its name says:
//!
//! - **A tape stop** is a constant deceleration: speed `1 − u` over the stop, so the delay is
//!   `u²/2` of the stop's span — a segment bent to `u²` (tension 1/7: the automation editor's
//!   power `1 + 7·t` is 2) rising by half its width, which ends at speed zero exactly on its
//!   last point. **A tape start** is the mirror: `1 − (1−u)²`. A tape head (and a stylus)
//!   reads a level proportional to its speed: the volume bank's *Tape stop fade* and *Fade
//!   the last beat* fall on a line to nothing with them (the presets pair them).
//! - **Half time** is a line at half the 45° slope: the first half of the cycle at half speed.
//! - **A repeat** is a held step: from its point on, the delay is how far back the repeated
//!   slice began.
//! - **Reverse** is a slope of 2: the read runs back as fast as the song runs on.
//! - **A scratch** is S-curves back and forth: an S of tension 0.15 (power 2.05) across a
//!   rise of its own width runs the record at −1.05× in its middle, and back down at 3.05×.

use super::{Pattern, Point, SHAPE_HOLD, SHAPE_S, SLOTS};

/// The tension that bends a segment to `u²` (power `1 + 7·t` = 2).
const TAPE: f32 = 1.0 / 7.0;
/// An S-curve's tension for the scratches and the tremolos: power 2.05, close to a sine's
/// turn.
const S_T: f32 = 0.15;

const QUARTER: u8 = 0;
const HALF: u8 = 1;
const BAR: u8 = 2;

fn pat(len: u8, points: Vec<Point>) -> Pattern {
    Pattern {
        len,
        points,
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

/// An S-curve from here.
fn s(x: f32, y: f32) -> Point {
    Point::new(x, y).shape(SHAPE_S).bend(S_T)
}

/// Steps of `n` across the cycle, each held at `f(k)`.
fn steps(n: usize, f: impl Fn(usize) -> f32) -> Vec<Point> {
    (0..n).map(|k| h(k as f32 / n as f32, f(k))).collect()
}

/// Steps of sixteenths from a pattern of bits.
fn bits(b: &[u8]) -> Vec<Point> {
    steps(b.len(), |k| f32::from(b[k]))
}

fn done(v: Vec<Pattern>) -> [Pattern; SLOTS] {
    match v.try_into() {
        Ok(b) => b,
        Err(v) => panic!("a factory bank has {} patterns, not 36", v.len()),
    }
}

/// The time bank, in slot order.
pub(super) fn time() -> [Pattern; SLOTS] {
    let scratch = vec![s(0.0, 0.0), s(0.25, 0.25), s(0.5, 0.0), s(0.75, 0.25)];
    done(vec![
        // ---- Row 1: stops and starts.
        // 1 Through: nothing moves.
        pat(BAR, vec![]),
        // 2 Tape stop: the whole bar, ending still on the next bar's line.
        pat(BAR, vec![p(0.0, 0.0).bend(TAPE), p(1.0, 0.5)]),
        // 3 Tape stop over the last half bar.
        pat(BAR, vec![p(0.0, 0.0), p(0.5, 0.0).bend(TAPE), p(1.0, 0.25)]),
        // 4 Tape stop over the last beat.
        pat(
            BAR,
            vec![p(0.0, 0.0), p(0.75, 0.0).bend(TAPE), p(1.0, 0.125)],
        ),
        // 5 Tape start: the first beat spins up from still.
        pat(
            BAR,
            vec![p(0.0, 0.0).bend(-TAPE), p(0.25, 0.125), p(1.0, 0.125)],
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
        ),
        // ---- Row 2: speed.
        // 7 Half time over the bar (an octave down, the first half bar).
        pat(BAR, vec![p(0.0, 0.0), p(1.0, 0.5)]),
        // 8 Half time every half bar.
        pat(HALF, vec![p(0.0, 0.0), p(1.0, 0.5)]),
        // 9 Half time every beat.
        pat(QUARTER, vec![p(0.0, 0.0), p(1.0, 0.5)]),
        // 10 Quarter time: two octaves down.
        pat(BAR, vec![p(0.0, 0.0), p(1.0, 0.75)]),
        // 11 Slow down: from full speed to half, steadily.
        pat(BAR, vec![p(0.0, 0.0).bend(TAPE), p(1.0, 0.25)]),
        // 12 Wobble: a warped record, ±28 cents twice a bar at 120 BPM.
        pat(HALF, vec![s(0.0, 0.0), s(0.5, 0.004)]),
        // ---- Row 3: repeats.
        // 13 The last beat repeats the third.
        pat(BAR, vec![h(0.0, 0.0), h(0.75, 0.25)]),
        // 14 Every beat's first eighth, twice.
        pat(QUARTER, vec![h(0.0, 0.0), h(0.5, 0.5)]),
        // 15 Every beat's first sixteenth, four times.
        pat(QUARTER, steps(4, |k| k as f32 / 4.0)),
        // 16 Every beat's first thirty-second, eight times.
        pat(QUARTER, steps(8, |k| k as f32 / 8.0)),
        // 17 Roll build: a beat, then eighths, sixteenths and thirty-seconds.
        pat(BAR, {
            let mut v = vec![h(0.0, 0.0), h(0.375, 0.125), h(0.5, 0.0)];
            v.extend((1..4).map(|k| h(0.5 + k as f32 / 16.0, k as f32 / 16.0)));
            v.push(h(0.75, 0.0));
            v.extend((1..8).map(|k| h(0.75 + k as f32 / 32.0, k as f32 / 32.0)));
            v
        }),
        // 18 Beat repeat: the second beat again, twice, in the second half.
        pat(BAR, vec![h(0.0, 0.0), h(0.5, 0.25), h(0.75, 0.5)]),
        // ---- Row 4: reverse.
        // 19 The second half bar runs the first back.
        pat(BAR, vec![p(0.0, 0.0), p(0.5, 0.0), p(1.0, 1.0)]),
        // 20 Every second beat runs the one before back.
        pat(HALF, vec![p(0.0, 0.0), p(0.5, 0.0), p(1.0, 1.0)]),
        // 21 The last beat runs the third back.
        pat(BAR, vec![p(0.0, 0.0), p(0.75, 0.0), p(1.0, 0.5)]),
        // 22 Every beat's second eighth runs its first back.
        pat(QUARTER, vec![p(0.0, 0.0), p(0.5, 0.0), p(1.0, 1.0)]),
        // 23 Rewind: the last eighth winds back, faster and faster (to −3×).
        pat(
            BAR,
            vec![p(0.0, 0.0), p(0.875, 0.0).bend(TAPE), p(1.0, 0.25)],
        ),
        // 24 Backspin: the last beat spun back hard.
        pat(
            BAR,
            vec![p(0.0, 0.0), p(0.75, 0.0).bend(-0.1), p(1.0, 0.75)],
        ),
        // ---- Row 5: scratch.
        // 25 Baby scratch: back and forth on every eighth.
        pat(QUARTER, scratch.clone()),
        // 26 Slow scratch: back and forth on every quarter.
        pat(HALF, scratch),
        // 27 Tear: the back stroke in two pushes, the record held between, then released.
        pat(
            QUARTER,
            vec![
                s(0.0, 0.0),
                p(0.125, 0.15),
                s(0.25, 0.275),
                s(0.375, 0.425),
                p(0.75, 0.0),
            ],
        ),
        // 28 Scribble: a fast tremble of the hand on the record.
        pat(
            QUARTER,
            (0..8)
                .map(|k| s(k as f32 / 8.0, if k % 2 == 1 { 0.02 } else { 0.0 }))
                .collect(),
        ),
        // 29 Drag: the record held back by the hand, then let go.
        pat(BAR, vec![s(0.0, 0.0), s(0.5, 0.2)]),
        // 30 Scratch back: three beats as they come, one stroke back and forth in the last.
        pat(BAR, vec![p(0.0, 0.0), s(0.75, 0.0), s(0.875, 0.1)]),
        // ---- Row 6: juggles.
        // 31 The first half bar twice.
        pat(BAR, vec![h(0.0, 0.0), h(0.5, 0.5)]),
        // 32 Beat juggle: one, one, three, three.
        pat(
            BAR,
            vec![h(0.0, 0.0), h(0.25, 0.25), h(0.5, 0.0), h(0.75, 0.25)],
        ),
        // 33 Dotted repeat: the first dotted eighth, over and over.
        pat(
            BAR,
            (0..6)
                .map(|k| h(k as f32 * 0.1875, k as f32 * 0.1875))
                .collect(),
        ),
        // 34 Triplet stutter: every beat's first triplet eighth, three times.
        pat(QUARTER, steps(3, |k| k as f32 / 3.0)),
        // 35 Every half bar's first sixteenth, eight times.
        pat(HALF, steps(8, |k| k as f32 / 8.0)),
        // 36 Echo repeat: the bar's first sixteenth sixteen times.
        pat(BAR, steps(16, |k| k as f32 / 16.0)),
    ])
}

/// The volume bank, in slot order.
pub(super) fn vol() -> [Pattern; SLOTS] {
    done(vec![
        // ---- Row 1: gates.
        // 1 Through: full level.
        pat(BAR, vec![]),
        // 2 On the beat: every beat's first eighth.
        pat(QUARTER, vec![h(0.0, 1.0), h(0.5, 0.0)]),
        // 3 Off the beat: every beat's second eighth.
        pat(QUARTER, vec![h(0.0, 0.0), h(0.5, 1.0)]),
        // 4 Gate in sixteenths.
        pat(QUARTER, bits(&[1, 0, 1, 0])),
        // 5 Gate in thirty-seconds.
        pat(QUARTER, bits(&[1, 0, 1, 0, 1, 0, 1, 0])),
        // 6 Trance gate.
        pat(BAR, bits(&[1, 0, 1, 1, 0, 1, 1, 0, 1, 1, 0, 1, 1, 0, 1, 0])),
        // ---- Row 2: rhythms.
        // 7 Trance gate 2.
        pat(BAR, bits(&[1, 1, 0, 1, 1, 0, 1, 0, 1, 1, 0, 1, 1, 0, 1, 1])),
        // 8 Trance gate 3.
        pat(BAR, bits(&[1, 0, 0, 1, 1, 0, 1, 0, 0, 1, 1, 0, 1, 0, 1, 0])),
        // 9 Trance gate 4: in pairs.
        pat(HALF, bits(&[1, 1, 0, 1, 1, 0, 1, 0])),
        // 10 Triplets: the first and third of every beat's three.
        pat(QUARTER, steps(6, |k| if k % 2 == 0 { 1.0 } else { 0.0 })),
        // 11 Tresillo: three, three, two.
        pat(HALF, bits(&[1, 0, 0, 1, 0, 0, 1, 0])),
        // 12 Gallop: one, and-a.
        pat(QUARTER, bits(&[1, 0, 1, 1])),
        // ---- Row 3: pumps.
        // 13 Pump: the sidechain's duck on every beat, without a key.
        pat(QUARTER, vec![p(0.0, 0.0).bend(-0.6), h(0.5, 1.0)]),
        // 14 Pump on the half notes.
        pat(HALF, vec![p(0.0, 0.0).bend(-0.6), h(0.5, 1.0)]),
        // 15 Hard pump: deep and quick.
        pat(QUARTER, vec![p(0.0, 0.0).bend(-0.85), h(0.35, 1.0)]),
        // 16 Soft pump: a gentle dip.
        pat(QUARTER, vec![p(0.0, 0.45).bend(-0.5), h(0.6, 1.0)]),
        // 17 Duck on every eighth.
        pat(
            QUARTER,
            vec![
                p(0.0, 0.0).bend(-0.6),
                h(0.25, 1.0),
                p(0.5, 0.0).bend(-0.6),
                h(0.75, 1.0),
            ],
        ),
        // 18 Swell: every beat rising out of quiet.
        pat(QUARTER, vec![p(0.0, 0.15).bend(0.5), p(1.0, 1.0)]),
        // ---- Row 4: fades.
        // 19 Fade in over the bar, again every bar.
        pat(BAR, vec![p(0.0, 0.0).bend(0.3), p(1.0, 1.0)]),
        // 20 Fade out over the bar.
        pat(BAR, vec![p(0.0, 1.0).bend(-0.3), p(1.0, 0.0)]),
        // 21 Fade in and out.
        pat(BAR, vec![s(0.0, 0.0), s(0.5, 1.0)]),
        // 22 Fade the last beat: falls with a tape stop over it.
        pat(BAR, vec![p(0.0, 1.0), p(0.75, 1.0), p(1.0, 0.0)]),
        // 23 Fade the last half bar.
        pat(BAR, vec![p(0.0, 1.0), p(0.5, 1.0), p(1.0, 0.0)]),
        // 24 Tape stop fade: the whole bar down, with the whole-bar tape stop.
        pat(BAR, vec![p(0.0, 1.0), p(1.0, 0.0)]),
        // ---- Row 5: tremolos.
        // 25 Tremolo on the eighths.
        pat(QUARTER, vec![s(0.0, 1.0), s(0.5, 0.2)]),
        // 26 Tremolo on the sixteenths.
        pat(
            QUARTER,
            vec![s(0.0, 1.0), s(0.25, 0.2), s(0.5, 1.0), s(0.75, 0.2)],
        ),
        // 27 Slow tremolo: twice a bar.
        pat(
            BAR,
            vec![s(0.0, 1.0), s(0.25, 0.3), s(0.5, 1.0), s(0.75, 0.3)],
        ),
        // 28 Saw down on every beat.
        pat(QUARTER, vec![p(0.0, 1.0), p(1.0, 0.1)]),
        // 29 Saw up on every beat.
        pat(QUARTER, vec![p(0.0, 0.1), p(1.0, 1.0)]),
        // 30 Echo fade: each sixteenth quieter than the last.
        pat(BAR, steps(16, |k| (0..k).fold(1.0f32, |a, _| a * 0.88))),
        // ---- Row 6: cuts.
        // 31 Cut the last beat.
        pat(BAR, vec![h(0.0, 1.0), h(0.75, 0.0)]),
        // 32 Cut two and four.
        pat(BAR, bits(&[1, 0, 1, 0])),
        // 33 Cut the last eighth of every half bar.
        pat(HALF, vec![h(0.0, 1.0), h(0.75, 0.0)]),
        // 34 Transformer: a scratcher's fader in thirty-seconds, every beat.
        pat(QUARTER, bits(&[1, 0, 0, 1, 1, 0, 1, 0])),
        // 35 Chop: sixteenths on and off over the half bar, the third one held.
        pat(HALF, bits(&[1, 0, 1, 0, 1, 1, 1, 0])),
        // 36 Mute.
        pat(BAR, vec![h(0.0, 0.0)]),
    ])
}

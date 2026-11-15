//! Delay's time synced to the tempo (round 40, ADR 0119): the note values, the line each
//! needs, and the read that does not depend on the line's length.
//!
//! **The note values** are forever (a project, an automation lane and the exported
//! plug-in's state hold the index): `note = 3 · base + feel`, the base from 1/64 (0) to
//! 4/1 (8) — four whole notes, four bars of 4/4 — and the feel straight (0), dotted (1)
//! or triplet (2). The default, index 10, is a dotted eighth: 375 ms at 120 BPM, the free
//! time's default, so switching the sync on at 120 BPM moves nothing.
//!
//! **The line** grows with what the note needs at the slowest tempo a song can have
//! ([`FLOOR_BPM`]), so a tempo change or the tempo track never finds it short: the note at
//! 20 BPM, in whole seconds, at least the free line's two and at most [`MAX_LINE_SECONDS`].
//! It is built off the audio thread (ADR 0022's path, `MemoryNeed::DelayLine`) and the
//! unit switches to it when it arrives, its history copied in; a synced time longer than
//! the line holds (a dotted 4/1 under 30 BPM, or while a longer line is on its way) is
//! held at the line's end. With the sync off the line is the free line, exactly as it
//! always was (the null, to the bit).
//!
//! **The time follows the tempo** the engine tells the unit before each pass (the tempo of
//! the pass: under a ramp, one value per step of `RAMP_STEP` ticks, each starting a pass,
//! so the same samples hear the same tempo at every block size). The read head ([`Head`],
//! in `f64`, so a long line's head moves by fractions of a sample, never in steps) goes
//! straight to each step's time over the step inside a ramp, glides a small move over
//! 50 ms and crossfades a jump over 50 ms: no click, no zipper, no sweep. While the lines hold only silence a new tempo is taken at once: nothing can be heard
//! moving, and a unit at rest stays at rest exactly (ADR 0031 §5.1).

use crate::modfx_clock::DEFAULT_TEMPO;

/// The note values: 9 bases × straight, dotted, triplet.
pub const NOTES: usize = 27;
/// A dotted eighth: 375 ms at 120 BPM (the free time's default).
pub const DEFAULT_NOTE: usize = 10;
/// The slowest tempo a song can have (`cantus_core::time::MIN_BPM`): the line holds the
/// note at this tempo.
pub const FLOOR_BPM: f64 = 20.0;
/// The longest line, in seconds: 4/1 at 20 BPM.
pub const MAX_LINE_SECONDS: f32 = 48.0;

/// The base of note `i`, in quarter notes: 1/64 is a sixteenth of a beat, 4/1 sixteen.
fn base_beats(i: usize) -> f64 {
    let b = (i / 3).min(8) as i32;
    // 2^b / 16, exactly.
    f64::from(1u32 << b) / 16.0
}

/// Note `i` in quarter notes (straight, dotted ×1.5, triplet ×2/3).
pub fn beats(i: usize) -> f64 {
    let i = i.min(NOTES - 1);
    let b = base_beats(i);
    match i % 3 {
        0 => b,
        1 => b * 1.5,
        _ => b * 2.0 / 3.0,
    }
}

/// Note index from a parameter's value (stepped, clamped).
pub fn note_index(v: f32) -> usize {
    if v.is_finite() {
        (v.round().max(0.0) as usize).min(NOTES - 1)
    } else {
        DEFAULT_NOTE
    }
}

/// A tempo as the unit takes it: the given one, or 120 when it is not a tempo.
pub fn tempo_or_default(bpm: f64) -> f64 {
    if bpm.is_finite() && bpm > 0.0 {
        bpm
    } else {
        DEFAULT_TEMPO
    }
}

/// Note `i` at `bpm`, in milliseconds (what the window shows).
pub fn time_ms(i: usize, bpm: f64) -> f64 {
    beats(i) * 60_000.0 / tempo_or_default(bpm)
}

/// Note `i` at `bpm`, in samples at `sample_rate` (unclamped).
pub fn time_samples(i: usize, bpm: f64, sample_rate: f32) -> f64 {
    beats(i) * 60.0 / tempo_or_default(bpm) * f64::from(sample_rate)
}

/// Seconds of line the settings need: the free line's when the sync is off; the note at
/// [`FLOOR_BPM`], in whole seconds, between the free line's and [`MAX_LINE_SECONDS`], when
/// it is on.
pub fn line_seconds(sync: bool, note: usize) -> f32 {
    let free = super::MAX_DELAY_SECONDS;
    if !sync {
        return free;
    }
    let s = (beats(note) * 60.0 / FLOOR_BPM).ceil() as f32;
    s.clamp(free, MAX_LINE_SECONDS)
}

/// The cells of a line of `seconds` at `sample_rate` (two more than the longest delay, as
/// the free line always had).
pub fn line_len(seconds: f32, sample_rate: f32) -> usize {
    (seconds * sample_rate) as usize + 2
}

/// A delay line pair, built off the audio thread and handed to the unit.
#[derive(Clone, Debug)]
pub struct Lines {
    pub l: Vec<f32>,
    pub r: Vec<f32>,
}

impl Lines {
    /// Silent lines of `len` cells. Allocates: never on the audio thread.
    pub fn new(len: usize) -> Lines {
        Lines {
            l: vec![0.0; len],
            r: vec![0.0; len],
        }
    }

    pub fn len(&self) -> usize {
        self.l.len()
    }

    pub fn is_empty(&self) -> bool {
        self.l.is_empty()
    }
}

/// The sample `d` samples before the next write (`1 ≤ d ≤ len − 2`), linearly between the
/// two around it — worked out from the whole and the fraction of `d` alone, so the same
/// history reads the same whatever the line's length or where its write position is.
#[inline]
pub fn read(buf: &[f32], write: usize, d: f64) -> f32 {
    read_masked::<false>(buf, write, d, 0)
}

/// [`read`], the cells a reset left uncleared (from `write` up to `stale`) read as the
/// zeros they stand for when `MASK` (fix-a, A-M3) — a copy of its own, so the unmasked
/// read is the instructions it was.
#[inline(always)]
pub(crate) fn read_masked<const MASK: bool>(
    buf: &[f32],
    write: usize,
    d: f64,
    stale: usize,
) -> f32 {
    let len = buf.len();
    let whole = d.floor();
    let f = (d - whole) as f32;
    let k = (whole as usize).min(len - 2);
    let a = cell::<MASK>(buf, (write + len - k) % len, write, stale);
    let b = cell::<MASK>(buf, (write + len - k - 1) % len, write, stale);
    a + (b - a) * f
}

/// Cell `i` of a line whose next write is at `write`: with `MASK`, a cell from `write` up
/// to `stale` — not written since a reset, not cleared yet — is a zero.
#[inline(always)]
pub(crate) fn cell<const MASK: bool>(buf: &[f32], i: usize, write: usize, stale: usize) -> f32 {
    if MASK && i >= write && i < stale {
        0.0
    } else {
        buf[i]
    }
}

/// How far a move may bend the pitch and still glide (3 %): a larger one crossfades.
pub const BEND: f64 = 0.03;

/// The synced read head (ADR 0119 §3), in samples, in `f64`. It moves three ways:
///
/// - **inside a tempo ramp**, straight to each step's time over the step (a sixteenth of a
///   beat): the time is the ramp's, piece by piece, and the pitch bends as a tape's would
///   whose speed follows the ramp;
/// - **a small move** (a host's tempo creeping, a bend under [`BEND`] over the fade),
///   straight there over the fade's length;
/// - **a jump** (a tempo marker, a new note value), a crossfade from the old head to the
///   new over the fade's length: no click and no sweep. A move that comes during a
///   crossfade waits for its end.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Head {
    /// The head's time now (during a crossfade, the new head's).
    pub current: f64,
    /// Where it is going.
    pub target: f64,
    /// Samples per sample of a straight move.
    step: f64,
    /// Samples left of the move or the crossfade.
    left: u32,
    /// The crossfade's old head, and its length (0: not fading).
    from: f64,
    fade: u32,
    /// A move asked for during a crossfade.
    pending: Option<f64>,
}

impl Head {
    pub fn new(value: f64) -> Head {
        Head {
            current: value,
            target: value,
            step: 0.0,
            left: 0,
            from: value,
            fade: 0,
            pending: None,
        }
    }

    /// There at once.
    pub fn snap(&mut self, v: f64) {
        *self = Head::new(v);
    }

    /// Go to `t`: over `ramp` samples straight when it is a ramp's step; otherwise straight
    /// over `fade` when the bend is small, crossfading over `fade` when it is not.
    pub fn go(&mut self, t: f64, ramp: Option<u32>, fade: u32) {
        if self.fade > 0 {
            self.pending = Some(t);
            return;
        }
        let fade = fade.max(1);
        match ramp {
            Some(n) => self.straight(t, n.max(1)),
            None if (t - self.current).abs() <= BEND * f64::from(fade) => self.straight(t, fade),
            None => {
                self.from = self.current;
                self.current = t;
                self.target = t;
                self.step = 0.0;
                self.left = fade;
                self.fade = fade;
            }
        }
    }

    fn straight(&mut self, t: f64, n: u32) {
        self.target = t;
        if t == self.current {
            self.left = 0;
            return;
        }
        self.step = (t - self.current) / f64::from(n);
        self.left = n;
    }

    /// The next sample's heads: the old one, the new one, and how much of the new is heard
    /// (1 except in a crossfade, where the old is the rest).
    #[inline]
    pub fn next(&mut self, fade: u32) -> (f64, f64, f32) {
        if self.fade > 0 {
            let g = (self.fade - self.left + 1) as f32 / self.fade as f32;
            let out = (self.from, self.current, g);
            self.left -= 1;
            if self.left == 0 {
                self.fade = 0;
                self.from = self.current;
                if let Some(t) = self.pending.take() {
                    self.go(t, None, fade);
                }
            }
            return out;
        }
        if self.left > 0 {
            self.left -= 1;
            self.current = if self.left == 0 {
                self.target
            } else {
                self.current + self.step
            };
        }
        (self.current, self.current, 1.0)
    }

    /// Nothing moving and nothing waiting.
    pub fn is_settled(&self) -> bool {
        self.left == 0 && self.fade == 0 && self.pending.is_none() && self.current == self.target
    }

    /// The time it is going to (a move waiting, else its target).
    pub fn goal(&self) -> f64 {
        self.pending.unwrap_or(self.target)
    }
}

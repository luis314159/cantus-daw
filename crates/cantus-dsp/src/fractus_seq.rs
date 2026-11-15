//! Fractus's sequencer: the steps' lines on the song, the blocks as events, the rolls, and
//! the events held notes make. Everything here is a function of the song sample, the
//! settings and the pattern; the struct only caches it.

use super::{Ev, FractusSettings, Pattern, StepKind, Trigger, LANES};
use crate::modfx_clock::hash64;

/// The line of absolute step `k` (from the song's start), song samples: the engine's own
/// expression for a beat's sample, the odd steps late by the swing.
#[inline]
pub(crate) fn line(k: i64, step: f64, swing: f32, tempo: f64, sr: f64) -> i64 {
    let sw = if k.rem_euclid(2) == 1 {
        f64::from(swing) * 0.5 * step
    } else {
        0.0
    };
    ((k as f64 * step + sw) * 60.0 / tempo * sr).round() as i64
}

/// The grid as one sample's maths needs it.
#[derive(Copy, Clone, Debug, PartialEq)]
pub(crate) struct Grid {
    pub step: f64,
    pub swing: f32,
    pub tempo: f64,
    pub sr: f64,
    pub steps: usize,
}

impl Grid {
    pub fn of(s: &FractusSettings, tempo: f64, sr: f64) -> Grid {
        Grid {
            step: s.step,
            swing: s.swing,
            tempo,
            sr,
            steps: s.steps(),
        }
    }

    #[inline]
    pub fn line(&self, k: i64) -> i64 {
        line(k, self.step, self.swing, self.tempo, self.sr)
    }

    /// Samples in a quarter note.
    #[inline]
    pub fn spb(&self) -> f64 {
        60.0 / self.tempo * self.sr
    }

    /// The absolute step holding song sample `so`.
    pub fn step_of(&self, so: i64) -> i64 {
        let len = self.step * self.spb();
        let mut k = (so as f64 / len).floor() as i64;
        while self.line(k) > so {
            k -= 1;
        }
        while self.line(k + 1) <= so {
            k += 1;
        }
        k
    }

    /// The first line at or after `so`.
    pub fn next_line(&self, so: i64) -> i64 {
        let k = self.step_of(so);
        let l = self.line(k);
        if l == so {
            l
        } else {
            self.line(k + 1)
        }
    }
}

/// A value in 0 … 1 from a hash.
#[inline]
pub(crate) fn unit(h: u64) -> f32 {
    (h >> 40) as f32 / (1u64 << 24) as f32
}

/// The hash of a seed, a lane, a count and a purpose: every roll in Fractus is one.
#[inline]
pub(crate) fn roll(seed: u32, lane: usize, n: u64, purpose: u64) -> u64 {
    hash64(
        u64::from(seed) ^ ((lane as u64) << 52) ^ (purpose << 60) ^ hash64(n ^ (purpose << 32)),
    )
}

#[derive(Copy, Clone, Debug)]
pub(crate) struct Seq {
    valid: bool,
    /// The cached step: its absolute index, its line and the next.
    k: i64,
    b0: i64,
    b1: i64,
    evs: [Option<Ev>; LANES],
    notes: [Option<Ev>; LANES],
    /// Bumped at every note event, so two notes on one sample still differ.
    note_count: u64,
}

impl Seq {
    pub fn new() -> Seq {
        Seq {
            valid: false,
            k: 0,
            b0: 0,
            b1: 0,
            evs: [None; LANES],
            notes: [None; LANES],
            note_count: 0,
        }
    }

    pub fn reset(&mut self) {
        *self = Seq::new();
    }

    pub fn invalidate(&mut self) {
        self.valid = false;
    }

    /// Every note event ends now (the trigger no longer takes notes).
    pub fn notes_off_now(&mut self) {
        self.notes = [None; LANES];
    }

    pub fn notes_held(&self) -> bool {
        self.notes.iter().any(Option::is_some)
    }

    /// Lanes a held note plays now, a bit each.
    pub fn note_bits(&self, so: i64) -> u32 {
        let mut b = 0;
        for (i, n) in self.notes.iter().enumerate() {
            if n.is_some_and(|e| so >= e.start && so < e.end) {
                b |= 1 << i;
            }
        }
        b
    }

    /// Make the cache hold the step of `so`.
    #[inline]
    pub fn update(&mut self, so: i64, set: &FractusSettings, pat: &Pattern, g: &Grid) {
        if self.valid && so >= self.b0 && so < self.b1 {
            return;
        }
        let k = g.step_of(so);
        self.k = k;
        self.b0 = g.line(k);
        self.b1 = g.line(k + 1);
        self.valid = true;
        for lane in 0..LANES {
            self.evs[lane] = block_at(pat, lane, k, set.seed, g);
        }
    }

    /// The event lane `lane` plays at `so` (after [`Seq::update`] for `so`): a held note's,
    /// or the steps'.
    #[inline]
    pub fn event(&mut self, lane: usize, so: i64, trigger: Trigger) -> Option<Ev> {
        if trigger.notes() {
            if let Some(e) = self.notes[lane] {
                if so >= e.end {
                    self.notes[lane] = None;
                } else if so >= e.start {
                    return Some(e);
                } else if trigger == Trigger::Notes {
                    return None;
                }
            }
        }
        if trigger.steps() {
            self.evs[lane]
        } else {
            None
        }
    }

    /// A note starts lane `lane`'s module at `so` (or the next line, quantised).
    pub fn note_on(&mut self, lane: usize, so: i64, vel: u8, quantise: bool, g: &Grid) {
        let start = if quantise { g.next_line(so) } else { so };
        self.note_count += 1;
        self.notes[lane] = Some(Ev {
            start,
            end: i64::MAX,
            amount: f32::from(vel.clamp(1, 127)) / 127.0,
            seed: hash64(start as u64 ^ (self.note_count << 40) ^ 0x6E07E),
        });
    }

    /// Lane `lane`'s note is released at `so`: its event ends `xf` later (or on the line
    /// after that, quantised), so its fade has room.
    pub fn note_off(&mut self, lane: usize, so: i64, xf: u32, quantise: bool, g: &Grid) {
        if let Some(e) = &mut self.notes[lane] {
            if e.end != i64::MAX {
                return;
            }
            let at = so.max(e.start) + i64::from(xf);
            e.end = if quantise { g.next_line(at) } else { at };
            if e.end <= e.start {
                self.notes[lane] = None;
            }
        }
    }

    /// Every note is released at `so`.
    pub fn all_off(&mut self, so: i64, xf: u32) {
        for n in &mut self.notes {
            if let Some(e) = n {
                if so < e.start {
                    *n = None;
                } else if e.end == i64::MAX {
                    e.end = so + i64::from(xf);
                }
            }
        }
    }
}

/// The block lane `lane` plays through absolute step `k`, if it plays: from its start's
/// line to the line after its last hold, within the cycle, its roll passed.
fn block_at(pat: &Pattern, lane: usize, k: i64, seed: u32, g: &Grid) -> Option<Ev> {
    let n = g.steps as i64;
    let (c, i) = (k.div_euclid(n), k.rem_euclid(n) as usize);
    let l = &pat.lanes[lane];
    if l[i].kind == StepKind::Off {
        return None;
    }
    let mut a = i;
    while a > 0 && l[a].kind == StepKind::Hold {
        a -= 1;
    }
    let (a, amount, prob) = match l[a].kind {
        StepKind::Start => (a, l[a].amount, l[a].prob),
        // A hold after an empty step (or on the cycle's first) starts its own block.
        StepKind::Off => (a + 1, 1.0, 1.0),
        StepKind::Hold => (a, 1.0, 1.0),
    };
    let mut b = i + 1;
    while b < g.steps && l[b].kind == StepKind::Hold {
        b += 1;
    }
    let ka = c * n + a as i64;
    let plays = if prob >= 1.0 {
        true
    } else if prob <= 0.0 {
        false
    } else {
        unit(roll(seed, lane, ka as u64, 1)) < prob
    };
    if !plays || amount <= 0.0 {
        return None;
    }
    Some(Ev {
        start: g.line(ka),
        end: g.line(c * n + b as i64),
        amount: amount.min(1.0),
        seed: ka as u64,
    })
}

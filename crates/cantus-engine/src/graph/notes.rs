//! Notes into effects, by outbox (ADR 0022, ADR 0031 §1.5).
//!
//! A track writes the notes it plays into its own **outbox**, each with the absolute
//! engine sample it was played at — the track node during a pass, the control phase for
//! live notes and all-notes-off. An insert that hears the track is a **listener**: it keeps
//! a cursor into the outbox and its delay (the latency ahead of its slot, so note and sound
//! meet), and before each `process` call it takes every note due inside that call, at
//! `due − pass start`. The track is the listener's predecessor, so in a parallel schedule
//! the outbox is complete before anyone reads it; nobody writes where anyone else does.
//!
//! **What the shared queue did, the same.** Notes reach an effect in the order they were
//! played, each at `played + delay`, delivered before the call that contains that sample
//! (or at its first sample, if it is already late) — also to a bypassed effect, so it knows
//! what is held when it comes back. What is past due and was not delivered (its effect did
//! not run: a switched-off master chain) is dropped at the end of the block, as the queue
//! pruned it. A full outbox drops the note and counts it (`EngineStats::dropped_events`).

use crate::fx::FxNote;
use cantus_core::EffectId;

/// Notes a track can have on their way at once (per track, so a busy track cannot starve
/// another's listeners).
pub const OUTBOX: usize = 256;

/// One track's notes on their way to the effects that hear it: a ring of `(sample, note)`
/// in the order they were played, numbered by sequence.
#[derive(Debug)]
pub(crate) struct Outbox {
    notes: Box<[(u64, FxNote)]>,
    /// Sequence number of the next note written.
    written: u64,
    /// The oldest note some listener may still need (the slowest cursor, refreshed at the
    /// end of every block): the ring never overwrites from here on.
    floor: u64,
}

impl Outbox {
    pub(crate) fn new() -> Self {
        Outbox {
            notes: vec![(0, FxNote::AllOff); OUTBOX].into_boxed_slice(),
            written: 0,
            floor: 0,
        }
    }

    /// Write a note played at absolute sample `at`; `false` (dropped) when the ring is
    /// full of notes not yet delivered.
    #[inline]
    pub(crate) fn push(&mut self, at: u64, note: FxNote) -> bool {
        if self.written - self.floor >= OUTBOX as u64 {
            return false;
        }
        self.notes[(self.written % OUTBOX as u64) as usize] = (at, note);
        self.written += 1;
        true
    }

    #[inline]
    fn get(&self, seq: u64) -> (u64, FxNote) {
        self.notes[(seq % OUTBOX as u64) as usize]
    }

    /// Sequence number of the next note written: where a new listener starts.
    pub(crate) fn written(&self) -> u64 {
        self.written
    }

    /// No listener needs anything before `seq` any more.
    pub(crate) fn set_floor(&mut self, seq: u64) {
        self.floor = seq.clamp(self.floor, self.written);
    }
}

/// An insert hearing a track's notes.
#[derive(Copy, Clone, Debug)]
pub(crate) struct Listener {
    /// The track's slot.
    pub track: usize,
    pub effect: EffectId,
    /// How late the audio reaching the insert is: its notes wait as long.
    pub delay: u64,
    /// The next note of the track's outbox it has not taken.
    pub cursor: u64,
}

impl Listener {
    /// Hand `f` every note due before absolute sample `end`, at its offset from `start`
    /// (0 when it is already late), in the order played.
    #[inline]
    pub(crate) fn deliver(
        &mut self,
        outbox: &Outbox,
        start: u64,
        end: u64,
        mut f: impl FnMut(usize, FxNote),
    ) {
        while self.cursor < outbox.written {
            let (at, note) = outbox.get(self.cursor);
            let due = at + self.delay;
            if due >= end {
                break;
            }
            f(due.saturating_sub(start) as usize, note);
            self.cursor += 1;
        }
    }

    /// Drop what is past due at `now` and was not delivered.
    pub(crate) fn prune(&mut self, outbox: &Outbox, now: u64) {
        // A note older than the ring's floor was overwritten; it is long past due anyway.
        self.cursor = self.cursor.max(outbox.floor);
        while self.cursor < outbox.written {
            let (at, _) = outbox.get(self.cursor);
            if at + self.delay >= now {
                break;
            }
            self.cursor += 1;
        }
    }
}

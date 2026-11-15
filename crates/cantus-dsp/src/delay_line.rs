//! Delay's line looked after a bounded stretch per call (fix-a, A-M3; ADR 0119 §4 amended).
//!
//! A synced Delay's line may be 48 s — 2.3 M cells a side. Copying its history into a new
//! line, or clearing it after a reset, in one audio callback is tens of milliseconds: an
//! xrun. So neither is done whole:
//!
//! - **A new line** ([`StereoDelay::supply`]) is taken at once only when the old one holds
//!   silence. Otherwise it waits beside the old one ([`Move`]) while each call copies, before
//!   it runs, the frames it is about to process plus [`MAINTAIN_CELLS`] more of the history,
//!   oldest first, in runs of `memcpy` (absolute sample `s` lives at `s mod len` in either
//!   line). The unit keeps playing on the old line meanwhile (a synced time it cannot hold
//!   held at its end, as while the line was on its way). When the copy has caught up with
//!   the write position the lines are swapped before that call runs: the new line holds the
//!   history an immediate copy would, so **a line that grows mid-song still changes no
//!   sample**. The old line waits in `spent` for the engine to take it off the audio thread
//!   ([`StereoDelay::take_spent`]).
//! - **A reset** of a line that held sound leaves it as it is and marks the cells not
//!   written since (`stale`: from the write position up); the reads of a call made while
//!   any are left read them as zeros (`sync::cell`, a copy of the loops of its own), and each
//!   call clears the frames it is about to process plus [`MAINTAIN_CELLS`] more from the top.
//!   The output is a cleared line's, to the bit.
//!
//! Each call's work is at most `2 × (frames + MAINTAIN_CELLS)` cells for each of the two
//! ([`StereoDelay::work`] says how much the last call did).

use super::{Lines, StereoDelay};
use crate::rest::ZeroRun;

/// Cells a side copied or cleared per call beyond the call's own frames: 128 KB a side, a
/// few tens of microseconds; a 48 s line is moved or cleared in some 70 calls.
pub const MAINTAIN_CELLS: usize = 32_768;

/// A line being given the history of the one playing.
#[derive(Clone, Debug)]
pub(super) struct Move {
    pub lines: Box<Lines>,
    /// The next absolute sample to copy.
    next: u64,
    /// Anything copied yet (else the new line is all zeros).
    copied: bool,
}

/// Copy absolute samples `[from, to)` from `src` into `dst` (each at `s mod` its length),
/// in contiguous runs.
fn copy_run(src: &[f32], dst: &mut [f32], from: u64, to: u64) {
    let (a, b) = (src.len() as u64, dst.len() as u64);
    let mut s = from;
    while s < to {
        let i = (s % a) as usize;
        let j = (s % b) as usize;
        let run = (to - s).min(a - i as u64).min(b - j as u64) as usize;
        dst[j..j + run].copy_from_slice(&src[i..i + run]);
        s += run as u64;
    }
}

impl StereoDelay {
    /// Take a line built for [`Self::need`]. When the line playing holds only silence it is
    /// swapped in at once and the old one comes back; otherwise the history is moved into
    /// it over the next calls and the old one waits for [`Self::take_spent`]. What comes
    /// back now — the old line, a line on its way that this one replaces, or the offered one
    /// when it is not the length wanted (or a spent line still waits to be taken) — is to
    /// be freed off the audio thread. No allocation; O(1).
    pub fn supply(&mut self, lines: Box<Lines>) -> Option<Box<Lines>> {
        if lines.len() != self.wanted_len() || lines.r.len() != lines.l.len() {
            return Some(lines);
        }
        if self.moving.is_none() && self.silent_lines() {
            return Some(self.swap_in(lines, false));
        }
        if self.spent.is_some() {
            // The line it would let go of has nowhere to wait: asked for again later.
            return Some(lines);
        }
        let start = self
            .n
            .saturating_sub(self.buf_l.len().min(lines.len()) as u64);
        let replaced = self.moving.replace(Move {
            lines,
            next: start,
            copied: false,
        });
        replaced.map(|m| m.lines)
    }

    /// The line the unit let go of, once a move is done (or a reset took the new one): to be
    /// freed off the audio thread.
    pub fn take_spent(&mut self) -> Option<Box<Lines>> {
        self.spent.take()
    }

    /// A line is being moved or cleared, or one waits to be taken.
    pub fn busy(&self) -> bool {
        self.moving.is_some() || self.stale > 0 || self.spent.is_some()
    }

    /// Cells copied or cleared by the last call, both sides.
    pub fn work(&self) -> usize {
        self.work
    }

    /// Swap `lines` in for the line playing; the old one is returned. Its history is the
    /// new line's when `copied` (the newest `min(old, new)` cells are counted again for the
    /// rest); otherwise the new line holds only zeros.
    fn swap_in(&mut self, mut lines: Box<Lines>, copied: bool) -> Box<Lines> {
        let (old, new) = (self.buf_l.len(), lines.len());
        std::mem::swap(&mut self.buf_l, &mut lines.l);
        std::mem::swap(&mut self.buf_r, &mut lines.r);
        let write = (self.n % new as u64) as usize;
        self.write = write;
        self.stale = 0;
        if copied {
            let keep = old.min(new);
            for (q, b) in self.quiet.iter_mut().zip([&self.buf_l, &self.buf_r]) {
                q.dirty();
                q.wrote(b, write, keep);
            }
        } else {
            self.quiet = [ZeroRun::FULL; 2];
        }
        // The times held to the old line's end may reach further now.
        if self.settings.sync && !self.arm {
            let target = self.synced_target(self.settings.note);
            if target != self.synced.goal() {
                self.synced.go(target, None, self.fade);
            }
        }
        lines
    }

    /// Before a call of `frames`: a stretch of the move and of the clearing.
    pub(super) fn maintain(&mut self, frames: usize) {
        self.work = 0;
        if self.moving.is_some() {
            self.move_some(frames);
        }
        if self.stale > 0 {
            self.clear_some(frames);
        }
    }

    fn move_some(&mut self, frames: usize) {
        let silent = self.silent_lines();
        let n = self.n;
        let Some(m) = self.moving.as_mut() else {
            return;
        };
        if !(silent && !m.copied) {
            let keep = self.buf_l.len().min(m.lines.len()) as u64;
            // What the old line no longer holds is not copied (never, at this budget).
            m.next = m.next.max(n.saturating_sub(keep));
            let end = n.min(m.next + (frames + MAINTAIN_CELLS) as u64);
            if end > m.next {
                copy_run(&self.buf_l, &mut m.lines.l, m.next, end);
                copy_run(&self.buf_r, &mut m.lines.r, m.next, end);
                self.work += 2 * (end - m.next) as usize;
                m.next = end;
                m.copied = true;
            }
            if m.next < n {
                return;
            }
        }
        if self.spent.is_some() {
            return;
        }
        if let Some(m) = self.moving.take() {
            let copied = m.copied;
            let old = self.swap_in(m.lines, copied);
            self.spent = Some(old);
        }
    }

    fn clear_some(&mut self, frames: usize) {
        let n = self.n as usize;
        let hi = self.stale.min(self.buf_l.len());
        let lo = hi.saturating_sub(frames + MAINTAIN_CELLS).max(n.min(hi));
        self.buf_l[lo..hi].fill(0.0);
        self.buf_r[lo..hi].fill(0.0);
        self.work += 2 * (hi - lo);
        // Left above the frames this call writes, if any: `stale − n > frames`.
        self.stale = if lo <= n { 0 } else { lo };
    }

    /// Skipped frames write nothing: the stale cells they pass are cleared (they are what
    /// the call would have written zeros into). At most `frames` a side.
    pub(super) fn skip_stale(&mut self, frames: usize) {
        if self.stale == 0 {
            return;
        }
        let n = self.n as usize;
        let hi = (n + frames).min(self.stale);
        if hi > n {
            self.buf_l[n..hi].fill(0.0);
            self.buf_r[n..hi].fill(0.0);
            self.work += 2 * (hi - n);
        }
        self.end_stale_at(n + frames);
    }

    /// After a call: no cell is stale once the writes reach the top of the stale ones.
    pub(super) fn end_stale(&mut self) {
        self.end_stale_at(self.n as usize);
    }

    fn end_stale_at(&mut self, written: usize) {
        if self.stale > 0 && written >= self.stale {
            self.stale = 0;
        }
    }

    /// A reset while a line is on its way takes it at once (its copy is no history now) and
    /// lets the old one go; `Some(copied)` — whether the new line holds anything — when it
    /// did.
    pub(super) fn finish_move_for_reset(&mut self) -> Option<bool> {
        if self.spent.is_some() {
            // Cannot happen (a move starts only with nothing waiting): played on.
            return None;
        }
        let m = self.moving.take()?;
        let copied = m.copied;
        let old = self.swap_in(m.lines, copied);
        self.spent = Some(old);
        Some(copied)
    }
}

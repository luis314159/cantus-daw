//! A streamed voice's window (ADR 0050 §4): before the voice renders, the frames it can
//! touch — the interpolator's taps, the decimator's history and a fade's worth more — are
//! copied from the head or the pool into its own buffer, so the resampler reads plain slices
//! and takes exactly the paths it takes on a sample held in memory.
//!
//! **Any ratio.** When a run's reads would not fit the window (Draft far above the root, a
//! 96 kHz file, past the top mip level), the run is cut shorter — down to one output frame —
//! and when even one frame's reads are wider than the window, a read the window misses is
//! copied from the pool on the spot ([`Playhead::pool_read`]): the same numbers on the same
//! paths, so a wide span is never a late read. Only a block that is not in the pool is.

use super::*;
use std::time::Instant;

/// What [`Playhead::ensure`] found.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Reach {
    /// Render this many output frames (at least 1, at most what was asked).
    Ready(usize),
    /// A block is not in the pool (real time, or offline past the pool's patience): this
    /// many output frames can still be rendered from what the voice holds — fade over them.
    Late(usize),
    /// A voice decided at its start to play its head only (a control step reads more than
    /// the reader keeps ready: second review #17) reaches the head's end: this many output frames are
    /// left — fade over them. Not late: the same in real time and offline.
    End(usize),
}

impl Playhead {
    /// Give the voice a window of `frames` frames and its cell on `board` (allocates:
    /// with the instrument, never on the audio thread). `fade`: output frames the window
    /// keeps beyond what is rendered, so a late block can be faded out over them.
    pub fn attach_window(&mut self, frames: usize, fade: usize, board: Arc<Board>, cell: u16) {
        self.win = vec![0.0; frames * 2];
        self.win_cap = frames;
        self.fade = fade;
        self.board = Some((board, cell));
    }

    /// Offline (wait for blocks) or real time (fade on a late one).
    pub fn set_blocking(&mut self, on: bool) {
        self.blocking = on;
    }

    pub fn streamed(&self) -> bool {
        self.src.stream.is_some()
    }

    /// Whether a read missed a block since the last call (a block evicted under a wide
    /// read): the caller counts it as late.
    pub fn take_missed(&self) -> bool {
        self.missed.replace(false)
    }

    /// The voice reads nothing from disk any more: its cell says so.
    pub fn idle_cursor(&mut self) {
        if self.published != 0 {
            self.published = 0;
            if let Some((b, i)) = &self.board {
                b.set(*i as usize, 0);
            }
        }
    }

    /// Times the voice woke the stream reader since the last call.
    pub fn take_wakes(&mut self) -> u32 {
        std::mem::take(&mut self.wakes)
    }

    /// The local ratio above which a voice plays its head only (its instrument's rate:
    /// [`crate::exemplar::head_only_above`]).
    pub fn set_head_only_above(&mut self, ratio: f64) {
        self.head_only_above = ratio;
    }

    /// The voice has ended: its cell says it reads nothing, and it lets go of its stream
    /// (second review #2: an ended voice never keeps a stream from being released). The
    /// drop is never the last one — the hub and the sample cache hold the stream — so it
    /// frees nothing here, on the audio thread.
    pub fn let_go(&mut self) {
        self.idle_cursor();
        if self.src.stream.is_some() {
            self.src = Source::default();
            self.win_len = 0;
        }
    }

    /// `n` frames from frame `b` of the current level, interleaved: from the level in memory
    /// (a whole sample, or a streamed one's head), else from the window.
    #[inline]
    pub(super) fn slice(&self, b: usize, n: usize) -> Option<&[f32]> {
        let ch = if self.src.channels == 1 { 1 } else { 2 };
        let d = self.src.levels[self.level].as_deref().unwrap_or(&[]);
        let (lo, hi) = (b * ch, (b + n) * ch);
        if hi <= d.len() {
            return Some(&d[lo..hi]);
        }
        if self.win_len > 0
            && self.win_level == self.level
            && b as i64 >= self.win_base
            && (b + n) as i64 <= self.win_base + self.win_len as i64
        {
            let o = (b - self.win_base as usize) * ch;
            return Some(&self.win[o..o + n * ch]);
        }
        None
    }

    /// `n` frames from frame `b` copied into `out` from wherever they are — head, window or
    /// the pool. `false` (and the miss remembered) when a block is not there.
    pub(super) fn pool_read(&self, b: usize, n: usize, out: &mut [f32]) -> bool {
        let Some(st) = self.src.stream.as_ref() else {
            return false;
        };
        let ch = st.channels;
        let out = &mut out[..n * ch];
        if let Some(d) = self.slice(b, n) {
            out.copy_from_slice(d);
            return true;
        }
        let head = st.head_frames(self.level);
        let mut done = 0;
        if b < head {
            let h = st.heads[self.level].as_deref().unwrap_or(&[]);
            done = (head - b).min(n);
            out[..done * ch].copy_from_slice(&h[b * ch..(b + done) * ch]);
        }
        let got = st.copy(self.level, b + done, &mut out[done * ch..]);
        if done + got < n {
            self.missed.set(true);
            return false;
        }
        true
    }

    /// The frames of the current level the next `out` output frames can read, `[lo, hi)`:
    /// the decimator's history (as `prime` reads it) behind, the taps ahead.
    fn span(&self, out: usize, ratio: f64, reader: &Reader) -> (i64, i64) {
        let scale = (1u64 << self.level) as f64;
        let (lo, hi) = match self.mode {
            Mode::Exact => (self.pos.floor() - 1.0, self.pos + out as f64 + 2.0),
            Mode::Linear => (self.pos.floor() - 1.0, self.pos + out as f64 * ratio + 3.0),
            Mode::Resampled => {
                let s = ratio / scale / 2.0;
                let pk = self.pos / scale;
                let taps = reader.kernel.taps as f64;
                let m = reader.dec.odd.len() as f64;
                let d2 = (reader.dec.len / 2) as f64;
                (
                    pk + (d2 - 4.0 * m - 2.0) * s - taps - 2.0,
                    pk + out as f64 * ratio / scale + (d2 + 1.0) * s + taps + 3.0,
                )
            }
        };
        let frames = self.lvl.frames;
        (
            (lo.floor() as i64).clamp(0, frames),
            (hi.ceil() as i64).clamp(0, frames),
        )
    }

    /// The most output frames (≤ `most`) whose reads end by frame `end`.
    fn frames_before(&self, end: i64, most: usize, ratio: f64, reader: &Reader) -> usize {
        if self.span(most, ratio, reader).1 <= end {
            return most;
        }
        let (mut lo, mut hi) = (0usize, most);
        while lo < hi {
            let mid = (lo + hi).div_ceil(2);
            if self.span(mid, ratio, reader).1 <= end {
                lo = mid;
            } else {
                hi = mid - 1;
            }
        }
        lo
    }

    /// Get the next `want` output frames' reads ready (and a fade's worth more), saying
    /// where the voice reads. Offline a missing block is waited for (at most the pool's
    /// patience, and not while the voice already fades); in real time it is [`Reach::Late`].
    /// Sample memory and nothing else: no lock, no allocation, no I/O.
    pub fn ensure(&mut self, ratio: f64, reader: &Reader, want: usize, fading: bool) -> Reach {
        let want = want.max(1);
        if self.src.stream.is_none() || self.done {
            return Reach::Ready(want);
        }
        let most = want + self.fade;
        let (lo, hi) = self.span(most, ratio, reader);
        if self.head_only.is_none() {
            // Read faster than the reader keeps up with in real time (a millisecond of its
            // reads over a quarter of what it keeps ready: third review #8): it is not
            // streamed at all — offline neither, so both play the same.
            let local = ratio / (1u64 << self.level) as f64;
            self.head_only = Some(local > self.head_only_above);
        }
        if self.head_only == Some(true) {
            // Its head, read in place; nothing asked of the reader.
            let head = self.head_end();
            if hi <= head {
                return Reach::Ready(want);
            }
            let n = self.frames_before(head, want, ratio, reader);
            // Nothing of the head left: it ends here, reading nothing past it.
            self.done |= n == 0;
            return Reach::End(n);
        }
        // Near the hysteresis band's edges (`choose_level`): the next level may come.
        let local = ratio / (1u64 << self.level) as f64;
        let near = (local > 1.6, local < 1.1 && self.level > 0);
        self.publish(lo, near, ratio);
        // Covered already (the usual case): nothing to copy.
        if hi <= self.covered_end(lo) {
            return Reach::Ready(want);
        }
        let started = if self.blocking && !fading {
            Some(Instant::now())
        } else {
            None
        };
        loop {
            let (end, missing) = self.fill(lo, hi);
            let n = self.frames_before(end, most, ratio, reader);
            if n >= most || (!missing && n > 0) {
                return Reach::Ready(want.min(n));
            }
            if !missing {
                // Even one frame's reads are wider than the window (a bend that widened
                // them after the start): they come from the pool as they are read — as
                // many frames as the blocks there cover, down to one (second review #17).
                let fits = |n: usize| {
                    let (l, h) = self.span(n, ratio, reader);
                    self.resident(l, h)
                };
                if fits(1) {
                    let (mut a, mut b) = (1usize, want);
                    while a < b {
                        let mid = (a + b).div_ceil(2);
                        if fits(mid) {
                            a = mid;
                        } else {
                            b = mid - 1;
                        }
                    }
                    return Reach::Ready(a);
                }
            }
            let patience = self
                .src
                .stream
                .as_ref()
                .map_or(std::time::Duration::ZERO, |s| s.pool.patience());
            let failed = self.src.stream.as_ref().is_some_and(|s| s.failed());
            match started {
                Some(t) if !failed && t.elapsed() < patience => {
                    if let Some(s) = &self.src.stream {
                        s.pool.idle();
                    }
                }
                _ => return Reach::Late(n),
            }
        }
    }

    /// How far the frames a voice can read run on unbroken from `lo` — the head, then the
    /// window (`lo` itself when neither holds it).
    fn covered_end(&self, lo: i64) -> i64 {
        let head = self.head_end();
        let w = self.win_len > 0 && self.win_level == self.level;
        let (wb, we) = (self.win_base, self.win_base + self.win_len as i64);
        if lo < head {
            if w && wb <= head {
                we.max(head)
            } else {
                head
            }
        } else if w && lo >= wb && lo <= we {
            we
        } else {
            lo
        }
    }

    /// Whether every block of `[lo, hi)` beyond the head is in the pool.
    fn resident(&self, lo: i64, hi: i64) -> bool {
        let Some(st) = self.src.stream.as_ref() else {
            return true;
        };
        let k = self.level;
        let from = lo.max(st.head_frames(k) as i64);
        if from >= hi {
            return true;
        }
        (from as usize / BLOCK..=(hi as usize - 1) / BLOCK).all(|b| st.resident(k, b))
    }

    fn publish(&mut self, lo: i64, near: (bool, bool), ratio: f64) {
        let Some(st) = self.src.stream.as_ref() else {
            return;
        };
        if let Some((b, i)) = &self.board {
            let c = Cursor {
                stream: st.id,
                level: self.level,
                block: lo.max(0) as usize / BLOCK,
                near_up: near.0,
                near_down: near.1,
                ahead: Cursor::ahead_for(ratio / (1u64 << self.level) as f64),
            }
            .pack();
            if self.published != c {
                let before = Cursor::unpack(self.published);
                self.published = c;
                b.set(*i as usize, c);
                // The reader is woken only when it is needed sooner than its next look
                // (fourth review #1): a fast voice, a new level, or a block the voice will
                // reach that is not in the pool yet. A voice crossing blocks the reader
                // already keeps ready wakes nothing.
                let k = self.level;
                let block = lo.max(0) as usize / BLOCK;
                let ahead = Cursor::ahead_for(ratio / (1u64 << k) as f64);
                let head = st.head_frames(k) / BLOCK;
                // The blocks the previous cursor already asked for (the reader keeps the
                // cursor's block and READ_AHEAD + ahead more): the one newly wanted at the
                // far end is a look or more away, never a reason to wake (last review #2).
                let missing = (block..block + crate::exemplar::stream::READ_AHEAD + ahead)
                    .filter(|b| *b >= head && *b < st.blocks(k))
                    .any(|b| !st.resident(k, b));
                let new_level = before.is_some_and(|p| p.level != k);
                if ahead > 1 || new_level || missing {
                    st.pool.wake();
                    self.wakes += 1;
                }
            }
        }
    }

    /// Bring as much of `[lo, hi)` (and on, to the window's end) of the current level into
    /// the window as the head and the pool allow. Returns where what the voice can read now
    /// ends, and whether a missing block stopped it short of `hi`.
    fn fill(&mut self, lo: i64, hi: i64) -> (i64, bool) {
        let Playhead {
            src,
            win,
            win_cap,
            win_base,
            win_len,
            win_level,
            level,
            ..
        } = self;
        let Some(st) = src.stream.as_ref() else {
            return (i64::MAX, false);
        };
        let k = *level;
        let ch = st.channels;
        let head = st.head_frames(k) as i64;
        if hi <= head || *win_cap == 0 {
            return (head, false);
        }
        let end = *win_base + *win_len as i64;
        // The window starts where the reads start, or at the head's end, whichever is
        // later (the head is read in place).
        let start = lo.max(0);
        if *win_len == 0 || *win_level != k || start < *win_base || start > end {
            *win_level = k;
            *win_base = start;
            *win_len = 0;
        } else if hi > *win_base + *win_cap as i64 && start > *win_base {
            // Keep what is still ahead at the front.
            let keep = (end - start) as usize;
            let from = (start - *win_base) as usize * ch;
            win.copy_within(from..from + keep * ch, 0);
            *win_base = start;
            *win_len = keep;
        }
        let total = st.frames[k] as i64;
        let target = (*win_base + *win_cap as i64).min(total);
        let mut missing = false;
        while *win_base + (*win_len as i64) < target {
            let f = *win_base + *win_len as i64;
            let at = *win_len * ch;
            let room = (target - f) as usize;
            let n = if f < head {
                let h = st.heads[k].as_deref().unwrap_or(&[]);
                let n = room.min((head - f) as usize);
                let from = f as usize * ch;
                win[at..at + n * ch].copy_from_slice(&h[from..from + n * ch]);
                n
            } else {
                st.copy(k, f as usize, &mut win[at..at + room * ch])
            };
            if n == 0 {
                missing = true;
                break;
            }
            *win_len += n;
        }
        let reach = self.covered_end(lo);
        (reach, missing && reach < hi)
    }

    fn head_end(&self) -> i64 {
        self.src
            .stream
            .as_ref()
            .map_or(0, |s| s.head_frames(self.level) as i64)
    }
}

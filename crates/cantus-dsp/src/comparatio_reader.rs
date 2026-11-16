//! Where the reference plays: its position against the song's, and a reader that never
//! clicks.
//!
//! The engine holds the reference decoded in memory at the session's rate (Phase B) and
//! hands Comparatio its two channels each pass; this reader chooses the sample to play.
//!
//! ## The three ways to follow the song ([`Sync`])
//!
//! - **Free** — the reference has its own playhead, which runs while the song plays and
//!   stands while it stops; seeking the song does not move it. [`Reader::cue`] puts it
//!   anywhere.
//! - **Locked** — the reference sample is the song sample plus an offset: a seek, a loop's
//!   wrap or a jump in the song moves the reference with it.
//! - **Loop** — a region of the reference `[start, end)` played round and round while the
//!   song plays, its playhead free.
//!
//! The reference sounds only while the song plays; outside its length it is silence.
//!
//! ## No click, ever
//!
//! The reader holds a **voice** — silence, or a position that moves on one sample per
//! sample. Whenever the position wanted is not where the voice would go next (a seek, a
//! loop's wrap, a cue, a new offset, the song starting or stopping), the old voice goes on
//! where it would have gone and fades out while the new one fades in, equal power, over
//! [`DECLICK_MS`]. Both read the recording itself, which is in memory: at a loop's end the
//! fading voice plays what the recording has after the region, as a tape loop's splice
//! would. A new jump during a fade drops the older fading voice (a loop shorter than two
//! fades is lengthened to two).

use crate::math;
use crate::modfx_clock::Clock;

/// The fade at every jump, ms.
pub const DECLICK_MS: f32 = 5.0;

/// How the reference follows the song.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum Sync {
    /// Its own playhead, running while the song plays.
    #[default]
    Free,
    /// The song's position plus an offset, samples (positive: the reference is ahead).
    Locked { offset: i64 },
    /// A region of the reference, round and round, samples.
    Loop { start: i64, end: i64 },
}

/// The reference's two channels, as the engine holds them, and its identity (a new
/// reference is a new `id`).
#[derive(Copy, Clone, Debug)]
pub struct RefAudio<'a> {
    pub left: &'a [f32],
    pub right: &'a [f32],
    pub id: u64,
}

impl RefAudio<'_> {
    pub fn len(&self) -> i64 {
        self.left.len().min(self.right.len()) as i64
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    #[inline]
    fn at(&self, pos: i64) -> (f32, f32) {
        if pos >= 0 && pos < self.len() {
            (self.left[pos as usize], self.right[pos as usize])
        } else {
            (0.0, 0.0)
        }
    }
}

/// Silence, or the next sample to read.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Voice {
    Silent,
    At(i64),
}

impl Voice {
    fn next(self) -> Voice {
        match self {
            Voice::Silent => Voice::Silent,
            Voice::At(p) => Voice::At(p + 1),
        }
    }
}

/// The reader.
#[derive(Clone, Debug)]
pub struct Reader {
    sync: Sync,
    /// The free (and loop) playhead: the next sample it plays.
    head: i64,
    /// What sounds, and what fades out.
    main: Voice,
    ghost: Voice,
    /// Samples into the fade; `fade_len` when there is none.
    fade_k: usize,
    fade_len: usize,
    /// The reference read last, by id.
    id: Option<u64>,
}

impl Reader {
    pub fn new(sample_rate: f32) -> Self {
        let fade_len = ((DECLICK_MS / 1_000.0 * sample_rate).round() as usize).max(1);
        Reader {
            sync: Sync::Free,
            head: 0,
            main: Voice::Silent,
            ghost: Voice::Silent,
            fade_k: fade_len,
            fade_len,
            id: None,
        }
    }

    pub fn set_sync(&mut self, sync: Sync) {
        let sync = match sync {
            Sync::Loop { start, end } => {
                let start = start.max(0);
                Sync::Loop {
                    start,
                    end: end.max(start + 2 * self.fade_len as i64),
                }
            }
            s => s,
        };
        if let (Sync::Loop { start, end }, false) = (sync, self.sync == sync) {
            if self.head < start || self.head >= end {
                self.head = start;
            }
        }
        self.sync = sync;
    }

    pub fn sync(&self) -> Sync {
        self.sync
    }

    /// Put the free (or loop) playhead at `pos`.
    pub fn cue(&mut self, pos: i64) {
        self.head = match self.sync {
            Sync::Loop { start, end } => pos.clamp(start, end - 1),
            _ => pos.max(0),
        };
    }

    /// Where the reference is now (the next sample it plays), or `None` while silent.
    pub fn position(&self) -> Option<i64> {
        match self.main {
            Voice::At(p) => Some(p),
            Voice::Silent => None,
        }
    }

    /// The free (and loop) playhead.
    pub fn head(&self) -> i64 {
        self.head
    }

    /// The voice wanted `i` samples into the call; advances the free playhead.
    #[inline]
    fn wanted(&mut self, clock: &Clock, i: usize) -> Voice {
        if !clock.playing() {
            return Voice::Silent;
        }
        match self.sync {
            Sync::Locked { offset } => Voice::At(clock.song(i) + offset),
            Sync::Free => {
                let p = self.head;
                self.head += 1;
                Voice::At(p)
            }
            Sync::Loop { start, end } => {
                let p = self.head;
                self.head = if p + 1 >= end { start } else { p + 1 };
                Voice::At(p)
            }
        }
    }

    /// The next `left.len()` samples of the reference, from `clock` (already set for this
    /// call) `at` samples into the call.
    pub fn read(
        &mut self,
        audio: Option<RefAudio>,
        clock: &Clock,
        at: usize,
        left: &mut [f32],
        right: &mut [f32],
    ) {
        let id = audio.map(|a| a.id);
        if id != self.id {
            // Another reference: what was playing is gone; the new one fades in.
            self.id = id;
            self.ghost = Voice::Silent;
            self.main = Voice::Silent;
            self.fade_k = self.fade_len;
        }
        for (i, (l, r)) in left.iter_mut().zip(right.iter_mut()).enumerate() {
            let want = self.wanted(clock, at + i);
            // `main` holds, between samples, where the voice goes next.
            if want != self.main {
                self.ghost = self.main;
                self.fade_k = 0;
            }
            let (ml, mr) = Self::sample(audio, want);
            if self.fade_k < self.fade_len {
                let x = (self.fade_k as f32 + 0.5) / self.fade_len as f32;
                let a = x * std::f32::consts::FRAC_PI_2;
                let (gi, go) = (math::sin(a), math::cos(a));
                let (gl, gr) = Self::sample(audio, self.ghost);
                *l = ml * gi + gl * go;
                *r = mr * gi + gr * go;
                self.ghost = self.ghost.next();
                self.fade_k += 1;
                if self.fade_k == self.fade_len {
                    self.ghost = Voice::Silent;
                }
            } else {
                *l = ml;
                *r = mr;
            }
            self.main = want.next();
        }
    }

    #[inline]
    fn sample(audio: Option<RefAudio>, v: Voice) -> (f32, f32) {
        match (audio, v) {
            (Some(a), Voice::At(p)) => a.at(p),
            _ => (0.0, 0.0),
        }
    }

    /// Nothing sounds and nothing fades: the reference is silent and stays so while the
    /// song is stopped (or plays outside the reference, which [`Self::silent_ahead`] says).
    pub fn is_silent(&self) -> bool {
        self.main == Voice::Silent && self.fade_k >= self.fade_len
    }

    /// No fade is running.
    pub fn fade_done(&self) -> bool {
        self.fade_k >= self.fade_len
    }

    /// The next `n` samples are certainly silence, the voice outside the recording and
    /// moving on without a jump (a locked reference after its end), no fade running.
    pub fn silent_ahead(&self, audio: Option<RefAudio>, clock: &Clock, n: usize) -> bool {
        if self.fade_k < self.fade_len {
            return false;
        }
        let Some(a) = audio else {
            return true;
        };
        if !clock.playing() {
            return true;
        }
        match self.sync {
            Sync::Locked { offset } => {
                let p0 = clock.song(0) + offset;
                let p1 = p0 + n as i64;
                (p1 <= 0 || p0 >= a.len()) && self.main == Voice::At(p0)
            }
            _ => false,
        }
    }

    /// `n` samples skipped while [`Self::silent_ahead`] (or the song stopped): exactly
    /// what reading them would leave.
    pub fn skip(&mut self, clock: &Clock, n: usize) {
        if !clock.playing() {
            return;
        }
        match self.sync {
            Sync::Locked { offset } => self.main = Voice::At(clock.song(n) + offset),
            Sync::Free | Sync::Loop { .. } => {
                for i in 0..n {
                    let w = self.wanted(clock, i);
                    self.main = w.next();
                }
            }
        }
    }

    pub fn reset(&mut self) {
        self.head = match self.sync {
            Sync::Loop { start, .. } => start,
            _ => 0,
        };
        self.main = Voice::Silent;
        self.ghost = Voice::Silent;
        self.fade_k = self.fade_len;
        self.id = None;
    }
}

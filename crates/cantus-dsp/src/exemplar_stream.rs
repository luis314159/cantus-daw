//! Streaming a sample from disk (ADR 0050): the shared block pool, a streamed sample's block
//! tables and the boards voices publish what they read on. Plain data and atomics: the
//! reader thread that fills the pool lives in `cantus_engine::inst::stream`.
//!
//! **The pool** is fixed slots of [`BLOCK`] frames, added in chunks by the one writer (the
//! reader thread) and never freed. A slot's samples are `AtomicU32`s holding `f32` bits,
//! behind a **sequence number** — a seqlock: the writer makes it odd, writes after a release
//! fence and publishes an even number with a release store; a reader copies between two
//! loads of the number and keeps the copy only when both equal the number the block table
//! promised. A torn or recycled block is detected, never used. Everything is safe Rust.
//!
//! **A streamed sample** keeps the head of each mip level in memory (`heads`) and, per level,
//! one `AtomicU64` per block: 0, or the slot and the sequence number it was published with.
//! Every voice of a sample reads the same blocks.

use super::resample::MAX_LEVELS;
use std::sync::atomic::{fence, AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};

/// Frames in a block (per level).
pub const BLOCK: usize = 4_096;
/// Frames of level 0 a streamed sample keeps in memory (rounded up to whole blocks, plus the
/// latest start a zone gives it): the reader's worst-case latency at twice the rate, with
/// room to spare (ADR 0050 §3, `docs/round26/sampler.md`).
pub const HEAD_FRAMES: usize = 16_384;
/// Blocks the stream reader keeps ready ahead of each voice, after the one it reads. A
/// voice whose control step (32 output frames) reads more than that — over 384 frames of
/// its level a frame — plays its head only (second review #17).
pub const READ_AHEAD: usize = 3;
/// Slots in a chunk of the pool, and chunks at most (64 × 256 × 32 KiB = 512 MiB).
pub const CHUNK: usize = 256;
pub const MAX_CHUNKS: usize = 64;

/// One block's samples, interleaved (two channels at most), and its sequence number.
#[derive(Debug)]
pub struct Slot {
    seq: AtomicU32,
    data: Box<[AtomicU32]>,
}

/// The shared block pool. One writer (the reader thread) grows it and writes its slots;
/// any thread copies out of it.
#[derive(Debug)]
pub struct Pool {
    chunks: Box<[OnceLock<Box<[Slot]>>]>,
    /// Chunks published, in order (a reader sees a prefix).
    grown: AtomicU32,
    /// What a voice that must wait (offline) does between two looks: wake the reader and
    /// sleep a little. A plain function, so the pool stays free of threads.
    idle: fn(),
    /// Tell the reader a voice moved to a new block (third review #8): allocation- and
    /// lock-free, never waits — a voice reading fast is seen at once, not at the reader's
    /// next look (a millisecond asked, a timer tick of up to 16 ms given on Windows).
    wake: fn(),
    /// How long an offline voice waits for one block before it gives up (the file has gone,
    /// the reader is stuck): it then fades out as a late voice does in real time.
    patience: std::time::Duration,
}

fn new_chunk() -> Box<[Slot]> {
    (0..CHUNK)
        .map(|_| Slot {
            seq: AtomicU32::new(0),
            data: (0..BLOCK * 2).map(|_| AtomicU32::new(0)).collect(),
        })
        .collect()
}

impl Pool {
    /// A pool of `chunks` chunks to begin with (allocates: off the audio thread).
    pub fn new(chunks: usize, idle: fn()) -> Pool {
        let p = Pool {
            chunks: (0..MAX_CHUNKS).map(|_| OnceLock::new()).collect(),
            grown: AtomicU32::new(0),
            idle,
            wake: || {},
            patience: std::time::Duration::from_secs(10),
        };
        for _ in 0..chunks.clamp(1, MAX_CHUNKS) {
            p.grow();
        }
        p
    }

    /// Slots there are now.
    pub fn slots(&self) -> usize {
        self.grown.load(Ordering::Acquire) as usize * CHUNK
    }

    /// Add a chunk (the writer only; allocates). `false` at the cap.
    pub fn grow(&self) -> bool {
        let n = self.grown.load(Ordering::Acquire) as usize;
        if n >= MAX_CHUNKS {
            return false;
        }
        let _ = self.chunks[n].set(new_chunk());
        self.grown.store(n as u32 + 1, Ordering::Release);
        true
    }

    fn slot(&self, i: u32) -> Option<&Slot> {
        let (c, s) = (i as usize / CHUNK, i as usize % CHUNK);
        if c >= self.grown.load(Ordering::Acquire) as usize {
            return None;
        }
        self.chunks[c].get().map(|chunk| &chunk[s])
    }

    /// Write `samples` (interleaved, at most a block) into slot `i`; returns the even
    /// sequence number to publish it with. The writer only.
    pub fn write(&self, i: u32, samples: &[f32]) -> Option<u32> {
        let s = self.slot(i)?;
        let seq = s.seq.load(Ordering::Relaxed);
        let odd = seq.wrapping_add(1) | 1;
        s.seq.store(odd, Ordering::Relaxed);
        fence(Ordering::Release);
        for (d, v) in s.data.iter().zip(samples) {
            d.store(v.to_bits(), Ordering::Relaxed);
        }
        let even = odd.wrapping_add(1);
        s.seq.store(even, Ordering::Release);
        Some(even)
    }

    /// A pool whose voices call `wake` when they move to a new block.
    pub fn with_wake(mut self, wake: fn()) -> Pool {
        self.wake = wake;
        self
    }

    /// A voice moved to a new block: the reader looks now.
    #[inline]
    pub fn wake(&self) {
        (self.wake)()
    }

    /// A pool whose offline voices wait at most `patience` for a block.
    pub fn with_patience(mut self, patience: std::time::Duration) -> Pool {
        self.patience = patience;
        self
    }

    pub fn patience(&self) -> std::time::Duration {
        self.patience
    }

    /// Wait a little for the reader (offline voices only).
    pub fn idle(&self) {
        (self.idle)()
    }
}

/// A table entry: 0 is "not here".
fn entry(slot: u32, seq: u32) -> u64 {
    (slot as u64 + 1) | ((seq as u64) << 32)
}

/// A sample streamed from disk: its heads in memory, its blocks in the pool.
#[derive(Debug)]
pub struct Stream {
    /// Names it to the reader (from 1).
    pub id: u32,
    /// 1 or 2.
    pub channels: usize,
    pub rate: f32,
    /// Frames of each level present (0 above the last).
    pub frames: [usize; MAX_LEVELS],
    /// The first frames of each level present, interleaved.
    pub heads: [Option<Arc<[f32]>>; MAX_LEVELS],
    tables: [Box<[AtomicU64]>; MAX_LEVELS],
    pub pool: Arc<Pool>,
    failed: AtomicBool,
}

impl Stream {
    /// A stream of `frames0` frames whose levels `0..heads.len()` keep `heads` in memory
    /// (allocates its tables: off the audio thread).
    pub fn new(
        id: u32,
        channels: usize,
        rate: f32,
        frames0: usize,
        heads: Vec<Arc<[f32]>>,
        pool: Arc<Pool>,
    ) -> Stream {
        let channels = channels.clamp(1, 2);
        let mut frames = [0usize; MAX_LEVELS];
        let mut f = frames0;
        for slot in frames.iter_mut().take(heads.len().min(MAX_LEVELS)) {
            *slot = f;
            f = f.div_ceil(2);
        }
        let tables = std::array::from_fn(|k| {
            (0..frames[k].div_ceil(BLOCK))
                .map(|_| AtomicU64::new(0))
                .collect::<Box<[AtomicU64]>>()
        });
        let mut hs: [Option<Arc<[f32]>>; MAX_LEVELS] = Default::default();
        for (k, h) in heads.into_iter().enumerate().take(MAX_LEVELS) {
            hs[k] = Some(h);
        }
        Stream {
            id,
            channels,
            rate,
            frames,
            heads: hs,
            tables,
            pool,
            failed: AtomicBool::new(false),
        }
    }

    /// Levels present.
    pub fn levels(&self) -> usize {
        self.heads.iter().take_while(|h| h.is_some()).count()
    }

    /// Frames of level `k` held in memory.
    pub fn head_frames(&self, k: usize) -> usize {
        self.heads[k]
            .as_ref()
            .map_or(0, |h| h.len() / self.channels)
    }

    /// Blocks of level `k`.
    pub fn blocks(&self, k: usize) -> usize {
        self.tables[k].len()
    }

    /// Whether block `b` of level `k` is in the pool (the writer's view).
    pub fn resident(&self, k: usize, b: usize) -> bool {
        self.tables[k]
            .get(b)
            .is_some_and(|e| e.load(Ordering::Acquire) != 0)
    }

    /// Say block `b` of level `k` is in slot `slot`, written with `seq`. The writer only.
    pub fn publish(&self, k: usize, b: usize, slot: u32, seq: u32) {
        if let Some(e) = self.tables[k].get(b) {
            e.store(entry(slot, seq), Ordering::Release);
        }
    }

    /// Say block `b` of level `k` is no longer in the pool. The writer only, before it
    /// writes the slot again.
    pub fn evict(&self, k: usize, b: usize) {
        if let Some(e) = self.tables[k].get(b) {
            e.store(0, Ordering::Release);
        }
    }

    /// The file can no longer be read: waiting voices stop waiting.
    pub fn set_failed(&self) {
        self.failed.store(true, Ordering::Release);
    }

    pub fn failed(&self) -> bool {
        self.failed.load(Ordering::Acquire)
    }

    /// Copy frames of level `k` from frame `from` into `dst` (interleaved, `dst.len()` a
    /// whole number of frames), out of the pool, as far as the blocks are there and whole.
    /// Returns the frames copied. Allocation- and lock-free: any thread.
    pub fn copy(&self, k: usize, from: usize, dst: &mut [f32]) -> usize {
        let ch = self.channels;
        let want = (dst.len() / ch).min(self.frames[k].saturating_sub(from));
        let mut done = 0;
        while done < want {
            let f = from + done;
            let b = f / BLOCK;
            let Some(e) = self.tables[k].get(b).map(|e| e.load(Ordering::Acquire)) else {
                break;
            };
            if e == 0 {
                break;
            }
            let (slot, seq) = ((e & 0xFFFF_FFFF) as u32 - 1, (e >> 32) as u32);
            let Some(s) = self.pool.slot(slot) else {
                break;
            };
            if s.seq.load(Ordering::Acquire) != seq {
                break;
            }
            let off = f - b * BLOCK;
            let n = (BLOCK - off).min(want - done);
            let src = &s.data[off * ch..(off + n) * ch];
            for (d, v) in dst[done * ch..(done + n) * ch].iter_mut().zip(src) {
                *d = f32::from_bits(v.load(Ordering::Relaxed));
            }
            fence(Ordering::Acquire);
            if s.seq.load(Ordering::Relaxed) != seq {
                break;
            }
            done += n;
        }
        done
    }
}

/// Where each layer of an instrument is reading: one cell per layer, 0 when it streams
/// nothing. The reader scans every board.
#[derive(Debug)]
pub struct Board {
    cells: Box<[AtomicU64]>,
    /// The cells are [`StartPin`]s (an instrument's zones' starts), not [`Cursor`]s.
    pins: bool,
}

impl Board {
    pub fn new(layers: usize) -> Board {
        Board {
            cells: (0..layers).map(|_| AtomicU64::new(0)).collect(),
            pins: false,
        }
    }

    /// A board of [`StartPin`]s, one cell per zone.
    pub fn new_pins(zones: usize) -> Board {
        Board {
            pins: true,
            ..Board::new(zones)
        }
    }

    pub fn is_pins(&self) -> bool {
        self.pins
    }

    pub fn len(&self) -> usize {
        self.cells.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    /// Layer `i` reads block `block` of level `level` of stream `id` (0: nothing).
    pub fn set(&self, i: usize, cursor: u64) {
        if let Some(c) = self.cells.get(i) {
            c.store(cursor, Ordering::Relaxed);
        }
    }

    pub fn get(&self, i: usize) -> u64 {
        self.cells.get(i).map_or(0, |c| c.load(Ordering::Relaxed))
    }
}

/// What a voice reads, as its board cell says it.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Cursor {
    pub stream: u32,
    pub level: usize,
    pub block: usize,
    /// Its pitch is near the next level up or down (the resampler's hysteresis band):
    /// the reader keeps that level's block at the same place ready too.
    pub near_up: bool,
    pub near_down: bool,
    /// Blocks it wants ready beyond the reader's usual [`READ_AHEAD`] (0…15): a voice read
    /// fast runs through more than a block in one audio callback (third review #8).
    pub ahead: usize,
}

impl Cursor {
    /// The cell's value (0 never: stream ids start at 1).
    pub fn pack(&self) -> u64 {
        ((self.stream as u64) << 32)
            | ((self.level as u64 & 0xF) << 28)
            | ((self.near_up as u64) << 27)
            | ((self.near_down as u64) << 26)
            | ((self.ahead.min(15) as u64) << 22)
            | (self.block as u64 & 0x003F_FFFF)
    }

    /// `None` when the cell is idle.
    pub fn unpack(c: u64) -> Option<Cursor> {
        let stream = (c >> 32) as u32;
        (stream != 0).then_some(Cursor {
            stream,
            level: ((c >> 28) & 0xF) as usize,
            near_up: (c >> 27) & 1 == 1,
            near_down: (c >> 26) & 1 == 1,
            ahead: ((c >> 22) & 0xF) as usize,
            block: (c & 0x003F_FFFF) as usize,
        })
    }

    /// Blocks beyond [`READ_AHEAD`] a voice reading `local` frames of its level a frame
    /// wants ready: what one audio callback of up to [`MAX_CALLBACK`] frames runs through.
    pub fn ahead_for(local: f64) -> usize {
        ((local.max(0.0) * MAX_CALLBACK as f64) / BLOCK as f64)
            .ceil()
            .min(15.0) as usize
    }
}

/// The longest audio callback a fast voice is kept ready for, frames.
pub const MAX_CALLBACK: usize = 1_024;

/// Blocks a zone may start in beyond its sample's head (a zone's start, the velocity's):
/// the reader keeps them in the pool while the zone exists, so a note that starts there
/// finds its first blocks (review 13: no stream per start). `block`/`count` are level 0's;
/// the same span is kept at every level `1..levels` the zone's range can start at (a note
/// pitched up an octave or more reads a mip level from its first sample) — at level *k*
/// `count` blocks from `block >> k`, the same time ahead as level 0's. A zone pins at most
/// `count × levels` blocks (`count` ≤ 255, `levels` ≤ 8; a zone that starts one block past
/// its head and is never pitched an octave up pins 3).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct StartPin {
    pub stream: u32,
    pub block: usize,
    pub count: usize,
    pub levels: usize,
}

impl StartPin {
    pub fn pack(&self) -> u64 {
        ((self.stream as u64) << 32)
            | ((self.block as u64 & 0x000F_FFFF) << 12)
            | ((self.count.clamp(1, 255) as u64) << 4)
            | (self.levels.clamp(1, 15) as u64)
    }

    pub fn unpack(c: u64) -> Option<StartPin> {
        let stream = (c >> 32) as u32;
        (stream != 0).then_some(StartPin {
            stream,
            block: ((c >> 12) & 0x000F_FFFF) as usize,
            count: ((c >> 4) & 0xFF) as usize,
            levels: (c & 0xF) as usize,
        })
    }

    /// Blocks of level `k` to keep ready.
    pub fn blocks(&self, k: usize) -> std::ops::Range<usize> {
        (self.block >> k)..(self.block >> k) + self.count.max(1)
    }
}

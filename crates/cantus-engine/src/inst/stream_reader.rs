//! The stream reader thread (ADR 0050 §4): it scans every board for what voices read and
//! which starts zones keep, reads the blocks wanted (nearest first) into the pool and
//! evicts the least recently wanted. It keeps its own copies of the streams and boards,
//! refreshed only when they change, and sleeps when nothing streams (review 2, 21): a
//! millisecond between looks while voices read, 20 ms while streams exist but nothing
//! reads, and not at all — parked until a stream is made — when there is none.

use super::{hub, recover, Backing, Entry, Fault};
use cantus_dsp::exemplar::resample::{halve_reach, halve_span};
use cantus_dsp::exemplar::stream::{Board, Cursor, Pool, StartPin, BLOCK};
use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Weak};
use std::time::Duration;

/// Blocks the reader keeps ready ahead of each voice, after the one it reads.
pub const LOOKAHEAD: usize = cantus_dsp::exemplar::stream::READ_AHEAD;
/// Level-0 frames the reader has read (from a cache file or a decode) since the start: what
/// a start's chain costs (fourth review #6; tests).
static LEVEL0_READ: AtomicU64 = AtomicU64::new(0);

pub fn level0_frames_read() -> u64 {
    LEVEL0_READ.load(Ordering::Relaxed)
}

/// Start blocks (zones' starts kept ready) read in one look before the cursors are looked
/// at again (third review #7).
const PIN_READS_PER_LOOK: usize = 2;
/// Between looks while voices read.
const BUSY: Duration = Duration::from_millis(1);
/// Between looks while streams exist but nothing reads (a voice that starts is seen within
/// this: well inside a head).
const QUIET: Duration = Duration::from_millis(20);

/// A pause before every block read, µs: the stress test's slow disk (0: none).
static READ_DELAY_US: AtomicU64 = AtomicU64::new(0);

/// Make every block read take `d` longer — a slow disk, for the stress test.
pub fn set_read_delay(d: Duration) {
    READ_DELAY_US.store(d.as_micros() as u64, Ordering::Relaxed);
}

/// Who holds a slot and when it was last wanted.
#[derive(Clone, Copy, Debug)]
struct Owner {
    stream: u32,
    level: u8,
    block: u32,
    wanted: u64,
}

/// What one look found.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Look {
    /// Blocks were read.
    Read,
    /// Something is wanted, all of it in the pool already.
    Wanted,
    /// Nothing reads.
    Quiet,
}

struct Reader {
    owners: Vec<Option<Owner>>,
    tick: u64,
    generation: u64,
    entries: Vec<Option<Arc<Entry>>>,
    /// Never a strong copy between looks: a board is the instrument's alone (second
    /// review #1).
    boards: Vec<Weak<Board>>,
}

pub(super) fn run() {
    let mut r = Reader {
        owners: Vec::new(),
        tick: 0,
        generation: u64::MAX,
        entries: Vec::new(),
        boards: Vec::new(),
    };
    loop {
        r.refresh();
        match r.look() {
            Look::Read => {}
            Look::Wanted => std::thread::park_timeout(BUSY),
            Look::Quiet if r.entries.iter().all(Option::is_none) => std::thread::park(),
            Look::Quiet => std::thread::park_timeout(QUIET),
        }
    }
}

impl Reader {
    /// The hub's streams and boards, copied only when they changed.
    fn refresh(&mut self) {
        let hub = hub();
        let g = hub.generation.load(Ordering::Acquire);
        if g == self.generation {
            return;
        }
        let Ok(mut i) = hub.inner.lock() else {
            return;
        };
        i.boards.retain(|b| b.strong_count() > 0);
        self.entries.clone_from(&i.entries);
        self.boards.clone_from(&i.boards);
        self.generation = g;
        // Slots of released streams are free again (their files close with them).
        for o in self.owners.iter_mut() {
            if o.is_some_and(|o| {
                self.entries
                    .get(o.stream as usize - 1)
                    .is_none_or(Option::is_none)
            }) {
                *o = None;
            }
        }
    }

    fn entry(&self, id: u32) -> Option<&Arc<Entry>> {
        self.entries.get(id as usize - 1)?.as_ref()
    }

    /// One look at every board: what is wanted, nearest first, read.
    fn look(&mut self) -> Look {
        let Some(pool) = hub().pool.get().cloned() else {
            return Look::Quiet;
        };
        self.tick += 1;
        let mut wants: Vec<(usize, u32, usize, usize)> = Vec::new();
        // Start pins whose mip levels are built in one chain (fourth review #6).
        let mut chains: Vec<StartPin> = Vec::new();
        for b in &self.boards {
            // Held only while it is read.
            let Some(b) = b.upgrade() else {
                continue;
            };
            for i in 0..b.len() {
                let cell = b.get(i);
                if cell == 0 {
                    continue;
                }
                if b.is_pins() {
                    let Some(p) = StartPin::unpack(cell) else {
                        continue;
                    };
                    let Some(e) = self.entry(p.stream) else {
                        continue;
                    };
                    for k in 0..p.levels.min(e.stream.levels()) {
                        let head = e.stream.head_frames(k) / BLOCK;
                        for b in p.blocks(k).filter(|b| *b >= head) {
                            wants.push((LOOKAHEAD + 2, p.stream, k, b));
                        }
                    }
                    if p.levels > 1 {
                        chains.push(p);
                    }
                    continue;
                }
                let Some(c) = Cursor::unpack(cell) else {
                    continue;
                };
                let Some(e) = self.entry(c.stream) else {
                    continue;
                };
                let st = &e.stream;
                let first = |k: usize, blk: usize| blk.max(st.head_frames(k) / BLOCK);
                let start = first(c.level, c.block);
                for d in 0..=LOOKAHEAD + c.ahead {
                    wants.push((d.min(LOOKAHEAD), c.stream, c.level, start + d));
                }
                // The neighbouring mip level at the same place when the voice's pitch is
                // near it: a pitch change that crosses an octave finds its block there.
                let at = c.block * BLOCK;
                if c.near_up && c.level + 1 < st.levels() {
                    let b = first(c.level + 1, at / 2 / BLOCK);
                    wants.push((LOOKAHEAD + 1, c.stream, c.level + 1, b));
                }
                if c.near_down && c.level > 0 {
                    let b = first(c.level - 1, at * 2 / BLOCK);
                    wants.push((LOOKAHEAD + 1, c.stream, c.level - 1, b));
                }
            }
        }
        if wants.is_empty() {
            return Look::Quiet;
        }
        wants.sort_unstable();
        wants.dedup_by_key(|w| (w.1, w.2, w.3));
        let keep: HashSet<(u32, usize, usize)> = wants.iter().map(|w| (w.1, w.2, w.3)).collect();
        // Mark what is there as wanted, so nothing wanted is evicted.
        let tick = self.tick;
        for o in self.owners.iter_mut().flatten() {
            if keep.contains(&(o.stream, o.level as usize, o.block as usize)) {
                o.wanted = tick;
            }
        }
        let mut read = false;
        let mut pin_reads = 0;
        for &(priority, id, level, block) in &wants {
            // Playing voices first (third review #7): a few start blocks a look, then the
            // cursors are looked at again before any more.
            if priority > LOOKAHEAD + 1 && pin_reads >= PIN_READS_PER_LOOK {
                return Look::Read;
            }
            // A start's mip levels come from its chain, below.
            if priority > LOOKAHEAD + 1 && level > 0 {
                continue;
            }
            let Some(e) = self.entry(id).cloned() else {
                continue;
            };
            let st = &e.stream;
            if level >= st.levels()
                || block >= st.blocks(level)
                || st.resident(level, block)
                || st.failed()
                || e.recovering.load(Ordering::Acquire)
            {
                continue;
            }
            let Some(slot) = self.claim(&pool) else {
                break;
            };
            match self.load(&e, level, block) {
                Ok(samples) => {
                    if let Some(seq) = pool.write(slot as u32, &samples) {
                        st.publish(level, block, slot as u32, seq);
                        self.owners[slot] = Some(Owner {
                            stream: id,
                            level: level as u8,
                            block: block as u32,
                            wanted: tick,
                        });
                        read = true;
                        if priority > LOOKAHEAD + 1 {
                            pin_reads += 1;
                        }
                    }
                }
                Err(fault) => {
                    // A damaged, vanished or unreadable cache file: the source is decoded
                    // again beside the reader and the stream plays on from it (second
                    // review #5); a voice in real time that reaches the block first fades.
                    recover(&e, &fault);
                }
            }
        }
        // One start's levels a look, built from one read of the level-0 span its deepest
        // level covers and halved level by level (fourth review #6), after the cursors.
        if pin_reads < PIN_READS_PER_LOOK {
            for p in &chains {
                if self.chain(&pool, p) {
                    return Look::Read;
                }
            }
        }
        if read {
            Look::Read
        } else {
            Look::Wanted
        }
    }

    /// Build the blocks `p` keeps ready at levels 1 and up that are not in the pool: the
    /// level-0 frames the deepest of them reaches read once, each level halved from the one
    /// below over just the span the levels above need — the very numbers `frames` gives a
    /// block alone. Returns whether it built anything.
    fn chain(&mut self, pool: &Pool, p: &StartPin) -> bool {
        let Some(e) = self.entry(p.stream).cloned() else {
            return false;
        };
        let st = &e.stream;
        if st.failed() || e.recovering.load(Ordering::Acquire) {
            return false;
        }
        let top = p.levels.min(st.levels());
        let pinned = |k: usize| {
            let head = st.head_frames(k) / BLOCK;
            p.blocks(k).filter(move |b| *b >= head && *b < st.blocks(k))
        };
        if !(1..top).any(|k| pinned(k).any(|b| !st.resident(k, b))) {
            return false;
        }
        // The span each level needs, from the top down.
        let reach = halve_reach();
        let mut need = vec![(0usize, 0usize); top];
        let mut above: Option<(usize, usize)> = None;
        for k in (0..top).rev() {
            let frames = st.frames[k];
            let mut span = above.map(|(lo, hi)| {
                (
                    (2 * lo).saturating_sub(reach),
                    (2 * hi + reach + 1).min(frames),
                )
            });
            if k > 0 {
                if let (Some(a), Some(b)) = (pinned(k).next(), pinned(k).next_back()) {
                    let own = (a * BLOCK, ((b + 1) * BLOCK).min(frames));
                    span = Some(match span {
                        Some((lo, hi)) => (lo.min(own.0), hi.max(own.1)),
                        None => own,
                    });
                }
            }
            let Some(s) = span else {
                continue;
            };
            need[k] = s;
            above = Some(s);
        }
        let (lo0, hi0) = need[0];
        let mut below = match self.frames(&e, 0, lo0, hi0) {
            Ok(b) => b,
            Err(fault) => {
                // A block of the span that fails is recovered as any other (last review
                // #1): the stream recovering, its chain waits for it — never a span read
                // again look after look.
                recover(&e, &fault);
                return false;
            }
        };
        let mut below_lo = lo0;
        let ch = st.channels;
        let mut built = false;
        for (k, &(lo, hi)) in need.iter().enumerate().take(top).skip(1) {
            if hi <= lo {
                break;
            }
            let mut out = vec![0.0f32; (hi - lo) * ch];
            halve_span(
                &below,
                below_lo as i64,
                st.frames[k - 1] as i64,
                ch,
                lo,
                &mut out,
            );
            for b in pinned(k)
                .filter(|b| !st.resident(k, *b))
                .collect::<Vec<_>>()
            {
                let (bl, bh) = (b * BLOCK, ((b + 1) * BLOCK).min(st.frames[k]));
                if bl < lo || bh > hi {
                    continue;
                }
                let Some(slot) = self.claim(pool) else {
                    return built;
                };
                if let Some(seq) = pool.write(slot as u32, &out[(bl - lo) * ch..(bh - lo) * ch]) {
                    st.publish(k, b, slot as u32, seq);
                    self.owners[slot] = Some(Owner {
                        stream: p.stream,
                        level: k as u8,
                        block: b as u32,
                        wanted: self.tick,
                    });
                    built = true;
                }
            }
            below = out;
            below_lo = lo;
        }
        built
    }

    /// A slot to write: a free one, else the least recently wanted one nobody wants now
    /// (its block evicted first), else a new chunk.
    fn claim(&mut self, pool: &Pool) -> Option<usize> {
        if self.owners.len() < pool.slots() {
            self.owners.resize(pool.slots(), None);
        }
        if let Some(i) = self.owners.iter().position(|o| o.is_none()) {
            return Some(i);
        }
        let victim = self
            .owners
            .iter()
            .enumerate()
            .filter_map(|(i, o)| o.map(|o| (i, o)))
            .filter(|(_, o)| o.wanted != self.tick)
            .min_by_key(|(_, o)| o.wanted);
        if let Some((i, o)) = victim {
            if let Some(e) = self.entry(o.stream) {
                e.stream.evict(o.level as usize, o.block as usize);
            }
            self.owners[i] = None;
            return Some(i);
        }
        if pool.grow() {
            self.owners.resize(pool.slots(), None);
            return self.owners.iter().position(|o| o.is_none());
        }
        None
    }

    /// Close the handle of the stream read least recently (not `keep`'s): whether one was.
    /// Another stream's backing is only tried, never waited for.
    fn close_least_recent(&self, keep: u32) -> bool {
        let mut best: Option<(u64, &Arc<Entry>)> = None;
        for e in self.entries.iter().flatten() {
            if e.stream.id == keep {
                continue;
            }
            if let Ok(b) = e.backing.try_lock() {
                if let Backing::File(o) = &*b {
                    if o.f.is_some() && best.is_none_or(|(u, _)| o.used < u) {
                        best = Some((o.used, e));
                    }
                }
            }
        }
        let Some((_, e)) = best else {
            return false;
        };
        if let Ok(mut b) = e.backing.try_lock() {
            if let Backing::File(o) = &mut *b {
                o.close();
                return true;
            }
        }
        false
    }

    /// Block `block` of level `level`, interleaved.
    fn load(&mut self, e: &Entry, level: usize, block: usize) -> Result<Vec<f32>, Fault> {
        let delay = READ_DELAY_US.load(Ordering::Relaxed);
        if delay > 0 {
            std::thread::sleep(Duration::from_micros(delay));
        }
        let lo = block * BLOCK;
        let hi = ((block + 1) * BLOCK).min(e.stream.frames[level]);
        self.frames(e, level, lo, hi)
    }

    /// Frames `lo..hi` of level `k`: read (every block's checksum checked) for level 0,
    /// computed from the level below otherwise (`halve_span`: the in-memory mip-map's
    /// numbers).
    fn frames(&mut self, e: &Entry, k: usize, lo: usize, hi: usize) -> Result<Vec<f32>, Fault> {
        let st = &e.stream;
        let ch = st.channels;
        if k == 0 {
            LEVEL0_READ.fetch_add((hi - lo) as u64, Ordering::Relaxed);
            let mut backing = e.backing.lock().unwrap_or_else(|p| p.into_inner());
            // Its own handle, open (and locked) while the budget allows (third review #1).
            let o = match &mut *backing {
                // A first load whose cache file is still being written: the decode itself.
                Backing::Memory(m) => return Ok(m[lo * ch..hi * ch].to_vec()),
                Backing::File(o) => o,
            };
            if o.f.is_none() {
                if !super::handles::take() {
                    // The least recently read stream lets go of its handle for this one.
                    if !self.close_least_recent(st.id) || !super::handles::take() {
                        super::handles::force();
                    }
                }
                match super::stream_cache::open_locked(&o.c.path) {
                    Some(f) => o.f = Some(f),
                    None => {
                        super::handles::give();
                        // Gone since (another Cantus's sweep): recovered like a damaged one.
                        return Err(Fault::Gone);
                    }
                }
            }
            o.used = self.tick;
            let (b0, b1) = (lo / BLOCK, hi.div_ceil(BLOCK));
            if injected_io(&o.c.path) {
                return Err(Fault::Io("an injected read error (a test)".into(), b0));
            }
            let mut buf = vec![0.0f32; (b1 - b0) * BLOCK * ch];
            let f = o.f.as_mut().expect("opened above");
            if let Err(err) = o.c.read_blocks(f, b0, b1 - b0, &mut buf) {
                return Err(match err {
                    super::stream_cache::ReadError::Damaged => Fault::Damaged,
                    super::stream_cache::ReadError::Io(e) => {
                        // A read of many blocks: the one that fails, found block by block
                        // (only on this path), so it is that block that is read again.
                        let mut one = vec![0.0f32; BLOCK * ch];
                        let bad = (b0..b1)
                            .find(|b| o.c.read_blocks(f, *b, 1, &mut one).is_err())
                            .unwrap_or(b0);
                        io_fault(&e, bad)
                    }
                });
            }
            let from = (lo - b0 * BLOCK) * ch;
            return Ok(buf[from..from + (hi - lo) * ch].to_vec());
        }
        let reach = halve_reach();
        let below = st.frames[k - 1];
        let slo = (2 * lo).saturating_sub(reach);
        let shi = (2 * hi + reach + 1).min(below);
        // The level below from its head and the pool when it is all there (read a moment
        // ago: the levels are read in order), else from the file — never every level down
        // to the file for each block (third review #7).
        let src = match held(st, k - 1, slo, shi) {
            Some(s) => s,
            None => self.frames(e, k - 1, slo, shi)?,
        };
        let mut out = vec![0.0f32; (hi - lo) * ch];
        halve_span(&src, slo as i64, below as i64, ch, lo, &mut out);
        Ok(out)
    }
}

/// A read error a test asks for, once, on the next block read from `path` (round 29,
/// review 2: a frozen render's passing read error). Off, one relaxed load per read.
#[doc(hidden)]
pub fn inject_io_fault(path: &std::path::Path) {
    inject_io_faults(path, 1);
}

/// `n` read errors in a row on `path`, the reads that try it again included (review 3: a
/// lock that outlasts the first try).
#[doc(hidden)]
pub fn inject_io_faults(path: &std::path::Path, n: u32) {
    if let Ok(mut v) = INJECT_PATHS.lock() {
        v.retain(|(p, _)| p != path);
        if n > 0 {
            v.push((path.to_path_buf(), n));
        }
        INJECT.store(!v.is_empty(), Ordering::Release);
    }
}

static INJECT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// Per file, so tests running side by side never take each other's.
static INJECT_PATHS: std::sync::Mutex<Vec<(std::path::PathBuf, u32)>> =
    std::sync::Mutex::new(Vec::new());

pub(super) fn injected_io(path: &std::path::Path) -> bool {
    if !INJECT.load(Ordering::Relaxed) {
        return false;
    }
    let Ok(mut v) = INJECT_PATHS.lock() else {
        return false;
    };
    let Some(i) = v.iter().position(|(p, _)| p == path) else {
        return false;
    };
    v[i].1 -= 1;
    if v[i].1 == 0 {
        v.swap_remove(i);
    }
    INJECT.store(!v.is_empty(), Ordering::Release);
    true
}

/// A file error as the reader's fault: a file that is not there any more, or another.
fn io_fault(e: &std::io::Error, block: usize) -> Fault {
    match e.kind() {
        std::io::ErrorKind::NotFound => Fault::Gone,
        // A file cut short (a crash): not what was written.
        std::io::ErrorKind::UnexpectedEof => Fault::Damaged,
        _ => Fault::Io(e.to_string(), block),
    }
}

/// Frames `lo..hi` of level `k` from the stream's head and the pool, when every one of them
/// is there.
fn held(
    st: &cantus_dsp::exemplar::stream::Stream,
    k: usize,
    lo: usize,
    hi: usize,
) -> Option<Vec<f32>> {
    let ch = st.channels;
    let mut out = vec![0.0f32; (hi - lo) * ch];
    let head = st.head_frames(k).min(hi);
    let mut at = lo;
    if at < head {
        let h = st.heads[k].as_deref()?;
        out[..(head - lo) * ch].copy_from_slice(&h[lo * ch..head * ch]);
        at = head;
    }
    while at < hi {
        let n = st.copy(k, at, &mut out[(at - lo) * ch..]);
        if n == 0 {
            return None;
        }
        at += n;
    }
    Some(out)
}

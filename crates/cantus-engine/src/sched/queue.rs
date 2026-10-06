//! The two ready queues of R2, behind one trait.
//!
//! - [`Vyukov`]: two bounded multi-producer multi-consumer array queues (Dmitry Vyukov's
//!   algorithm), *critical* and *normal*, popped in that order — ADR 0031 §2.3 as the
//!   architect wrote it, and the prototype's `queue.rs` with its claim/publish window made
//!   visible.
//! - [`ChaseLev`]: one fixed-capacity Chase–Lev deque per thread (C11 orderings of Lê,
//!   Pop, Cohen and Zappa Nardelli, PPoPP 2013) and one **mailbox** per thread for the
//!   block's sources, seeded by each source's *home* (the thread that ran it last block —
//!   Acar, Blelloch and Blumofe, SPAA 2000). Owner push and pop are wait-free; a thief
//!   preempted mid-steal blocks no one; a mailbox claim is one `fetch_add`.
//!
//! Every cell is an atomic, so all of it is safe Rust. Capacities are fixed at
//! construction to the most tasks a block may have; since a task is made ready at most
//! once per block, no queue can fill (a full push is retried by the pool and counted).
//! Indices grow monotonically across blocks (64-bit, so they never wrap) and nothing is
//! reset between blocks: at the end of a block every queue is empty.

use super::Pad;
use std::sync::atomic::{fence, AtomicI64, AtomicU32, AtomicU64, AtomicU8, AtomicUsize, Ordering::*};

/// Per-task scheduling data the queues read: each task's priority (higher = more
/// critical) and home slot. Written by the callback while it arms a block (priorities) and
/// by whichever slot runs a task (home); read by every slot during the block.
pub struct Meta {
    prio: Box<[AtomicU32]>,
    home: Box<[AtomicU8]>,
    /// A task is *critical* when its priority is above this (the top quarter of the ranks).
    crit_above: AtomicU32,
}

impl Meta {
    /// Room for `tasks` tasks; homes start round-robin over `slots`.
    pub fn new(tasks: usize, slots: usize) -> Self {
        Meta {
            prio: (0..tasks).map(|_| AtomicU32::new(0)).collect(),
            home: (0..tasks).map(|t| AtomicU8::new((t % slots.max(1)) as u8)).collect(),
            crit_above: AtomicU32::new(u32::MAX),
        }
    }

    /// Set the priorities from a ranks array (critical path first): the first of `n` gets
    /// `n`, the last `1`; the top quarter (at least one) is critical. Callback only, while
    /// arming.
    pub fn set_ranks(&self, ranks: &[u32]) {
        let n = ranks.len();
        for (pos, &t) in ranks.iter().enumerate() {
            if let Some(p) = self.prio.get(t as usize) {
                p.store((n - pos) as u32, Relaxed);
            }
        }
        self.crit_above.store((n - n.div_ceil(4)) as u32, Relaxed);
    }

    /// The task's priority (higher runs first).
    #[inline]
    pub fn prio(&self, task: u32) -> u32 {
        self.prio[task as usize].load(Relaxed)
    }

    /// Whether the task is in the top quarter of the ranks.
    #[inline]
    pub fn critical(&self, task: u32) -> bool {
        self.prio(task) > self.crit_above.load(Relaxed)
    }

    /// The slot that ran the task last.
    #[inline]
    pub fn home(&self, task: u32) -> usize {
        self.home[task as usize].load(Relaxed) as usize
    }

    /// Record that `slot` runs the task (its home next block).
    #[inline]
    pub fn set_home(&self, task: u32, slot: usize) {
        self.home[task as usize].store(slot as u8, Relaxed);
    }
}

/// A ready queue for one pool: every slot (0 = the callback, `1..=W` the workers) pushes
/// the tasks it makes ready and pops tasks to run.
pub trait ReadyQueue: Send + Sync + 'static {
    /// A short name for reports.
    const NAME: &'static str;

    /// Allocate everything: room for `tasks` tasks per block, `slots` threads.
    fn new(tasks: usize, slots: usize) -> Self;

    /// Make the block's sources ready. Callback only, while arming (nothing in flight);
    /// `sources` are critical path first.
    fn seed(&self, sources: &[u32], meta: &Meta);

    /// Slot `slot` makes `task` ready. `between` runs between the claim and the publish
    /// (the schedule fuzz's stall site). Returns `false` if the queue is full (the caller
    /// retries; it cannot happen with a correct capacity).
    fn push(&self, slot: usize, task: u32, meta: &Meta, between: impl FnMut()) -> bool;

    /// Slot `slot` takes a task to run: its own work first, then anyone's. `mid` runs
    /// between a read and its claim. `None` when nothing is ready *right now*.
    fn pop(&self, slot: usize, meta: &Meta, mid: impl FnMut()) -> Option<u32>;
}

// ---------------------------------------------------------------------------------------
// Vyukov's bounded MPMC queue
// ---------------------------------------------------------------------------------------

struct Cell {
    seq: AtomicUsize,
    val: AtomicU32,
}

/// Dmitry Vyukov's bounded MPMC queue of `u32`s. Not lock-free "in the official meaning":
/// a producer stopped between its `enq` CAS and its `seq` store makes every consumer see
/// the queue *empty* at that cell, whatever was pushed after it (R2).
pub struct Mpmc {
    cells: Box<[Cell]>,
    mask: usize,
    enq: Pad<AtomicUsize>,
    deq: Pad<AtomicUsize>,
}

impl Mpmc {
    /// Room for at least `capacity` values.
    pub fn new(capacity: usize) -> Self {
        let cap = capacity.next_power_of_two().max(2);
        Mpmc {
            cells: (0..cap)
                .map(|i| Cell {
                    seq: AtomicUsize::new(i),
                    val: AtomicU32::new(0),
                })
                .collect(),
            mask: cap - 1,
            enq: Pad(AtomicUsize::new(0)),
            deq: Pad(AtomicUsize::new(0)),
        }
    }

    /// Push `v`; `between` runs after the cell is claimed and before it is published.
    pub fn push_with(&self, v: u32, mut between: impl FnMut()) -> bool {
        let mut pos = self.enq.load(Relaxed);
        loop {
            let cell = &self.cells[pos & self.mask];
            let seq = cell.seq.load(Acquire);
            let dif = seq as isize - pos as isize;
            if dif == 0 {
                match self.enq.compare_exchange_weak(pos, pos + 1, Relaxed, Relaxed) {
                    Ok(_) => {
                        cell.val.store(v, Relaxed);
                        between();
                        cell.seq.store(pos + 1, Release);
                        return true;
                    }
                    Err(p) => pos = p,
                }
            } else if dif < 0 {
                return false;
            } else {
                pos = self.enq.load(Relaxed);
            }
        }
    }

    /// Pop the oldest published value; `mid` runs after the claim, before the cell is
    /// released to producers.
    pub fn pop_with(&self, mut mid: impl FnMut()) -> Option<u32> {
        let mut pos = self.deq.load(Relaxed);
        loop {
            let cell = &self.cells[pos & self.mask];
            let seq = cell.seq.load(Acquire);
            let dif = seq as isize - (pos + 1) as isize;
            if dif == 0 {
                match self.deq.compare_exchange_weak(pos, pos + 1, Relaxed, Relaxed) {
                    Ok(_) => {
                        let v = cell.val.load(Relaxed);
                        mid();
                        cell.seq.store(pos + self.mask + 1, Release);
                        return Some(v);
                    }
                    Err(p) => pos = p,
                }
            } else if dif < 0 {
                return None;
            } else {
                pos = self.deq.load(Relaxed);
            }
        }
    }
}

/// ADR 0031 §2.3: two shared MPMC queues, critical and normal, popped in that order.
pub struct Vyukov {
    critical: Mpmc,
    normal: Mpmc,
}

impl Vyukov {
    fn lane(&self, task: u32, meta: &Meta) -> &Mpmc {
        if meta.critical(task) {
            &self.critical
        } else {
            &self.normal
        }
    }
}

impl ReadyQueue for Vyukov {
    const NAME: &'static str = "vyukov";

    fn new(tasks: usize, _slots: usize) -> Self {
        Vyukov {
            critical: Mpmc::new(tasks),
            normal: Mpmc::new(tasks),
        }
    }

    fn seed(&self, sources: &[u32], meta: &Meta) {
        for &t in sources {
            while !self.lane(t, meta).push_with(t, || ()) {
                std::hint::spin_loop();
            }
        }
    }

    #[inline]
    fn push(&self, _slot: usize, task: u32, meta: &Meta, between: impl FnMut()) -> bool {
        self.lane(task, meta).push_with(task, between)
    }

    #[inline]
    fn pop(&self, _slot: usize, _meta: &Meta, mut mid: impl FnMut()) -> Option<u32> {
        self.critical
            .pop_with(&mut mid)
            .or_else(|| self.normal.pop_with(&mut mid))
    }
}

// ---------------------------------------------------------------------------------------
// Chase–Lev deque with a fixed buffer
// ---------------------------------------------------------------------------------------

/// What a steal found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Steal {
    /// Nothing to take.
    Empty,
    /// Lost a race with the owner or another thief: try again (someone made progress).
    Retry,
    /// Took this task.
    Took(u32),
}

/// A Chase–Lev work-stealing deque of `u32`s with a **fixed** buffer: it never grows,
/// never shrinks and is never replaced, so there is nothing to reclaim (the objections to
/// crossbeam-deque in ADR 0031 §2.3 do not arise). Orderings from Lê et al., PPoPP 2013.
/// The owner pushes and pops at the bottom (LIFO); thieves take from the top (FIFO).
pub struct Deque {
    top: Pad<AtomicI64>,
    bottom: Pad<AtomicI64>,
    buf: Box<[AtomicU32]>,
    mask: i64,
}

impl Deque {
    /// Room for at least `capacity` tasks at once.
    pub fn new(capacity: usize) -> Self {
        let cap = capacity.next_power_of_two().max(2);
        Deque {
            top: Pad(AtomicI64::new(0)),
            bottom: Pad(AtomicI64::new(0)),
            buf: (0..cap).map(|_| AtomicU32::new(0)).collect(),
            mask: cap as i64 - 1,
        }
    }

    #[inline]
    fn cell(&self, i: i64) -> &AtomicU32 {
        &self.buf[(i & self.mask) as usize]
    }

    /// Owner only. `between` runs after the cell is written and before the new bottom is
    /// published: only this one task is invisible meanwhile.
    pub fn push_with(&self, v: u32, mut between: impl FnMut()) -> bool {
        let b = self.bottom.load(Relaxed);
        let t = self.top.load(Acquire);
        if b - t > self.mask {
            return false;
        }
        self.cell(b).store(v, Relaxed);
        between();
        fence(Release);
        self.bottom.store(b + 1, Relaxed);
        true
    }

    /// Owner only: the newest task.
    pub fn pop(&self) -> Option<u32> {
        let b = self.bottom.load(Relaxed) - 1;
        self.bottom.store(b, Relaxed);
        fence(SeqCst);
        let t = self.top.load(Relaxed);
        if t > b {
            self.bottom.store(b + 1, Relaxed);
            return None;
        }
        let v = self.cell(b).load(Relaxed);
        if t < b {
            return Some(v);
        }
        // The last one: race the thieves for it.
        let won = self.top.compare_exchange(t, t + 1, SeqCst, Relaxed).is_ok();
        self.bottom.store(b + 1, Relaxed);
        won.then_some(v)
    }

    /// Anyone: the oldest task. `mid` runs between the read and the claim.
    pub fn steal_with(&self, mut mid: impl FnMut()) -> Steal {
        let t = self.top.load(Acquire);
        fence(SeqCst);
        let b = self.bottom.load(Acquire);
        if t >= b {
            return Steal::Empty;
        }
        let v = self.cell(t).load(Relaxed);
        mid();
        if self.top.compare_exchange(t, t + 1, SeqCst, Relaxed).is_ok() {
            Steal::Took(v)
        } else {
            Steal::Retry
        }
    }

    /// Whether the deque looks empty (a hint, for idle scans).
    #[inline]
    pub fn looks_empty(&self) -> bool {
        self.top.load(Relaxed) >= self.bottom.load(Relaxed)
    }
}

/// A slot's mailbox: the block's sources whose home is this slot, critical path first.
/// Written by the callback while arming; claimed by anyone — the owner first, thieves when
/// the owner is late or busy — with one `fetch_add` on a word holding `(len << 32) | next`,
/// so a claim has no window in which a stalled thread can hide work.
struct Mailbox {
    word: Pad<AtomicU64>,
    /// How many items the callback has written this arming (callback only).
    fill: AtomicU32,
    items: Box<[AtomicU32]>,
}

impl Mailbox {
    fn new(capacity: usize) -> Self {
        Mailbox {
            word: Pad(AtomicU64::new(0)),
            fill: AtomicU32::new(0),
            items: (0..capacity.max(1)).map(|_| AtomicU32::new(0)).collect(),
        }
    }

    #[inline]
    fn claim(&self) -> Option<u32> {
        let w = self.word.load(Relaxed);
        if (w as u32) >= (w >> 32) as u32 {
            return None;
        }
        let old = self.word.fetch_add(1, Acquire);
        let (next, len) = (old as u32, (old >> 32) as u32);
        (next < len).then(|| self.items[next as usize].load(Relaxed))
    }
}

/// R2: one fixed Chase–Lev deque and one mailbox per slot (the callback's included).
pub struct ChaseLev {
    deques: Box<[Deque]>,
    mail: Box<[Mailbox]>,
}

impl ChaseLev {
    fn others(&self, slot: usize) -> impl Iterator<Item = usize> {
        let n = self.deques.len();
        (1..n).map(move |k| (slot + k) % n)
    }
}

impl ReadyQueue for ChaseLev {
    const NAME: &'static str = "chase-lev";

    fn new(tasks: usize, slots: usize) -> Self {
        let slots = slots.max(1);
        ChaseLev {
            deques: (0..slots).map(|_| Deque::new(tasks)).collect(),
            mail: (0..slots).map(|_| Mailbox::new(tasks)).collect(),
        }
    }

    fn seed(&self, sources: &[u32], meta: &Meta) {
        let n = self.mail.len();
        for m in self.mail.iter() {
            m.fill.store(0, Relaxed);
        }
        for &t in sources {
            let h = meta.home(t);
            let m = &self.mail[if h < n { h } else { t as usize % n }];
            let i = m.fill.load(Relaxed) as usize;
            if let Some(cell) = m.items.get(i) {
                cell.store(t, Relaxed);
                m.fill.store(i as u32 + 1, Relaxed);
            }
        }
        for m in self.mail.iter() {
            // Publishes the items written above (Release); everything claimed last block
            // is done, so no claim of the old word can still read an item.
            m.word.store(u64::from(m.fill.load(Relaxed)) << 32, Release);
        }
    }

    #[inline]
    fn push(&self, slot: usize, task: u32, _meta: &Meta, between: impl FnMut()) -> bool {
        self.deques[slot].push_with(task, between)
    }

    fn pop(&self, slot: usize, _meta: &Meta, mut mid: impl FnMut()) -> Option<u32> {
        if let Some(t) = self.deques[slot].pop() {
            return Some(t);
        }
        if let Some(t) = self.mail[slot].claim() {
            return Some(t);
        }
        // The others' sources first (the most critical work, and nobody may be doing it:
        // its owner may be late), then their deques, oldest first.
        for v in self.others(slot) {
            if let Some(t) = self.mail[v].claim() {
                return Some(t);
            }
        }
        for v in self.others(slot) {
            let d = &self.deques[v];
            loop {
                match d.steal_with(&mut mid) {
                    Steal::Took(t) => return Some(t),
                    Steal::Empty => break,
                    Steal::Retry => std::hint::spin_loop(),
                }
            }
        }
        None
    }
}

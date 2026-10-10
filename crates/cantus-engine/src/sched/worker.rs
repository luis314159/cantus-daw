//! Every slot's side of the pool: what the threads share, a worker's life (start, wait,
//! help, park), and the drain loop the callback runs too.

use super::{atoms32, padbools, pads32, pads64, Config, Idle, Remaining, StartHook};
use crate::sched::queue::{Meta, ReadyQueue};
use crate::sched::{buggify, Chaos, Graph, Pad, Site};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering::*};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

/// How many successors a finishing task sorts by priority before pushing; more than this
/// (rare: a track feeding many sends) are pushed as they come.
const READY_SORT: usize = 16;

/// What every thread of the pool shares.
pub(crate) struct Shared<G, Q, C> {
    pub(crate) graph: Arc<G>,
    pub(crate) queue: Q,
    pub(crate) chaos: C,
    pub(crate) meta: Meta,
    pub(crate) pending: Box<[Pad<AtomicU32>]>,
    pub(crate) remaining: Remaining,
    pub(crate) epoch: Pad<AtomicU64>,
    pub(crate) stop: AtomicBool,
    /// The topology's successor lists (`Pool::set_topology`): task `t`'s are
    /// `succs[succ_start[t]..succ_start[t + 1]]`.
    pub(crate) succ_start: Box<[AtomicU32]>,
    pub(crate) succs: Box<[AtomicU32]>,
    /// Bit `s` set: slot `s` has started and joined (bit 0, the callback, is implied).
    pub(crate) available: AtomicU64,
    /// The device period, ns; 0 until known (the workers wait for it).
    pub(crate) period_ns: AtomicU64,
    /// When the current block was armed, ns since `origin`.
    pub(crate) armed_ns: AtomicU64,
    pub(crate) origin: Instant,
    /// Per slot: parked (workers only), busy ns this block, tasks run, continuations.
    pub(crate) parked: Box<[Pad<AtomicBool>]>,
    pub(crate) busy: Box<[Pad<AtomicU64>]>,
    pub(crate) runs: Box<[Pad<AtomicU64>]>,
    pub(crate) conts: Box<[Pad<AtomicU64>]>,
    pub(crate) pushes: AtomicU64,
    pub(crate) full: AtomicU64,
    idle: Idle,
}

impl<G: Graph, Q: ReadyQueue, C: Chaos> Shared<G, Q, C> {
    pub(crate) fn new(
        graph: Arc<G>,
        queue: Q,
        chaos: C,
        tasks: usize,
        edges: usize,
        config: &Config,
        slots: usize,
    ) -> Self {
        Shared {
            graph,
            queue,
            chaos,
            meta: Meta::new(tasks, slots),
            pending: pads32(tasks),
            remaining: Pad(AtomicUsize::new(0)),
            epoch: Pad(AtomicU64::new(0)),
            stop: AtomicBool::new(false),
            succ_start: atoms32(tasks + 1),
            succs: atoms32(edges),
            available: AtomicU64::new(0),
            period_ns: AtomicU64::new(config.period.as_nanos() as u64),
            armed_ns: AtomicU64::new(0),
            origin: Instant::now(),
            parked: padbools(slots),
            busy: pads64(slots),
            runs: pads64(slots),
            conts: pads64(slots),
            pushes: AtomicU64::new(0),
            full: AtomicU64::new(0),
            idle: config.idle,
        }
    }

    /// Where task `t`'s successors are in `succs`.
    #[inline]
    pub(crate) fn succ_range(&self, t: u32) -> (usize, usize) {
        (
            self.succ_start[t as usize].load(Relaxed) as usize,
            self.succ_start[t as usize + 1].load(Relaxed) as usize,
        )
    }

    /// Run ready tasks until the block has none left — and, while nothing is ready, the
    /// jobs running tasks forked ([`Graph::help`]).
    pub(crate) fn drain(&self, slot: usize) {
        self.graph.presence(slot, true);
        self.drain_tasks(slot);
        self.graph.presence(slot, false);
    }

    fn drain_tasks(&self, slot: usize) {
        let mut cont: Option<u32> = None;
        let mut polls = 0u32;
        let mut since: Option<Instant> = None;
        loop {
            let task = match cont.take() {
                Some(t) => {
                    self.conts[slot].fetch_add(1, Relaxed);
                    t
                }
                None => {
                    let popped = self.queue.pop(slot, &self.meta, || {
                        buggify!(&self.chaos, Site::Steal, slot)
                    });
                    match popped {
                        Some(t) => t,
                        None => {
                            since = None;
                            if self.remaining.load(Acquire) == 0 {
                                return;
                            }
                            // A running task's forked jobs (ADR 0038): work, not waiting.
                            let helped = self.graph.help(slot);
                            if helped > 0 {
                                self.busy[slot].fetch_add(helped, Relaxed);
                                polls = 0;
                                continue;
                            }
                            // R5: a few bare polls, then PAUSE between polls. No clock:
                            // what this waits for is a task already running.
                            polls = polls.saturating_add(1);
                            if polls > 16 {
                                std::hint::spin_loop();
                            }
                            continue;
                        }
                    }
                }
            };
            polls = 0;
            let start = since.unwrap_or_else(Instant::now);
            buggify!(&self.chaos, Site::Run, slot);
            self.meta.set_home(task, slot);
            self.graph.run(task, slot);
            let end = Instant::now();
            // One writer per slot; the callback reads it after `remaining` reaches zero,
            // which this slot's `finish` below orders after this add.
            self.busy[slot].fetch_add(end.duration_since(start).as_nanos() as u64, Relaxed);
            self.runs[slot].fetch_add(1, Relaxed);
            since = Some(end);
            buggify!(&self.chaos, Site::Finish, slot);
            cont = self.finish(task, slot);
        }
    }

    /// Release `task`'s successors: the most critical one that became ready is returned as
    /// the continuation, the others pushed least critical first (so the owner's LIFO end
    /// pops the more critical first). `remaining` drops last, so it cannot reach zero
    /// while a successor is ready.
    fn finish(&self, task: u32, slot: usize) -> Option<u32> {
        let mut ready = [0u32; READY_SORT];
        let mut k = 0usize;
        let (a, b) = self.succ_range(task);
        for i in a..b {
            let s = self.succs[i].load(Relaxed);
            if self.pending[s as usize].fetch_sub(1, AcqRel) != 1 {
                continue;
            }
            if k == READY_SORT {
                self.push(slot, s);
                continue;
            }
            // Insertion sort, ascending priority.
            let p = self.meta.prio(s);
            let mut i = k;
            while i > 0 && self.meta.prio(ready[i - 1]) > p {
                ready[i] = ready[i - 1];
                i -= 1;
            }
            ready[i] = s;
            k += 1;
        }
        let cont = if k > 0 {
            k -= 1;
            Some(ready[k])
        } else {
            None
        };
        for &s in &ready[..k] {
            self.push(slot, s);
        }
        self.remaining.fetch_sub(1, AcqRel);
        cont
    }

    #[inline]
    fn push(&self, slot: usize, task: u32) {
        self.pushes.fetch_add(1, Relaxed);
        while !self.queue.push(slot, task, &self.meta, || {
            buggify!(&self.chaos, Site::Claimed, slot)
        }) {
            self.full.fetch_add(1, Relaxed);
            std::hint::spin_loop();
        }
    }

    /// A worker thread's whole life: wait for the period, run the start hook around
    /// [`Self::serve`] (it is available only from inside it).
    pub(crate) fn life(&self, slot: usize, hook: Option<StartHook>) {
        // The period may not be known yet (a pool built before its stream's first
        // block): wait parked for it.
        let period = loop {
            if self.stop.load(SeqCst) {
                return;
            }
            let ns = self.period_ns.load(SeqCst);
            if ns != 0 {
                break Duration::from_nanos(ns);
            }
            thread::park();
        };
        let mut serve = || self.serve(slot);
        match hook {
            Some(h) => h(slot, period, &mut serve),
            None => serve(),
        }
    }

    /// The pool's side of a worker's life: announce it, then wait for a block, help with
    /// it, repeat, until the pool stops.
    fn serve(&self, slot: usize) {
        let mut seen = self.epoch.load(Acquire);
        self.available.fetch_or(1 << slot, SeqCst);
        let mut last = Instant::now();
        loop {
            buggify!(&self.chaos, Site::Idle, slot);
            if !self.wait(slot, &mut seen, &mut last) {
                self.available.fetch_and(!(1 << slot), SeqCst);
                return;
            }
            self.drain(slot);
            last = Instant::now();
        }
    }

    /// Wait for the next epoch by the idle policy. `false` when the pool stops.
    fn wait(&self, slot: usize, seen: &mut u64, last: &mut Instant) -> bool {
        let mut polls = 0u32;
        loop {
            let e = self.epoch.load(Acquire);
            if e != *seen {
                *seen = e;
                return true;
            }
            if self.stop.load(Relaxed) {
                return false;
            }
            polls = polls.wrapping_add(1);
            // R5: bare polls, then PAUSE; the clock only every 256 polls (PAUSE is
            // ~140 cycles on Skylake and later, so the budget is kept in time).
            if polls < 64 || !polls.is_multiple_of(256) {
                std::hint::spin_loop();
                continue;
            }
            let park = match self.idle {
                Idle::Spin {
                    periods,
                    park_after,
                } => {
                    let period = Duration::from_nanos(self.period_ns.load(Relaxed));
                    let idle = last.elapsed();
                    if idle < period.mul_f64(periods) {
                        continue;
                    }
                    if idle < park_after.max(period.mul_f64(periods)) {
                        thread::yield_now();
                        continue;
                    }
                    true
                }
                Idle::Bounded { share, min_spin } => {
                    // Busy at most `share` of a period from the arm on — unless the block's
                    // own work ran longer — and always a short grace after the last task,
                    // so blocks that come back to back never park anyone.
                    let now = Instant::now();
                    let period = self.period_ns.load(Relaxed) as f64;
                    let armed = self.armed_ns.load(Relaxed);
                    let deadline = armed + (period * share) as u64;
                    let at = now.saturating_duration_since(self.origin).as_nanos() as u64;
                    at >= deadline && now.saturating_duration_since(*last) >= min_spin
                }
            };
            if !park {
                continue;
            }
            // Park (Dekker with the callback's arm: the flag, then the epoch again).
            self.parked[slot].store(true, SeqCst);
            if self.epoch.load(SeqCst) != *seen || self.stop.load(SeqCst) {
                self.parked[slot].store(false, SeqCst);
                continue;
            }
            thread::park();
            // Woken, or spuriously: the waker cleared the flag; clear it here too for the
            // spurious case, and start a fresh spin budget.
            self.parked[slot].store(false, SeqCst);
            *last = Instant::now();
            polls = 0;
        }
    }
}

//! The pool: W live workers plus the callback (ADR 0031 §2.1–§2.5).
//!
//! Slot 0 is the callback — the thread that calls [`Pool::run_block`] — and slots `1..=W`
//! are the workers, plain `std::thread`s. A worker runs [`Config::on_start`] first, with the
//! device period: that hook is where `cantus-rt`'s promotion and placement plug in, and a
//! worker **counts as available only once it has returned into the pool** (rt's round-21
//! lesson: the first block must never wait for a worker the OS has not yet scheduled to
//! promote itself). Until the period is known (a pool built before its stream's first
//! block) the workers wait parked, and the callback's first block publishes it.
//!
//! **Topology.** The pool keeps its own copy of the task graph's shape
//! ([`Pool::set_topology`]): the callback's arrays (predecessor counts, ranks) and the
//! successor lists every slot reads in `finish`, as atomics. The graph changes it between
//! blocks only, on the callback — so [`Graph`] itself is just `run`.
//!
//! **Idle** ([`Idle`]): [`Idle::Spin`] is round 21's discipline (spin 1.5 periods, yield,
//! park after ~100 ms of silence: a spinner, for `Waiting::Spins`); [`Idle::Bounded`] is
//! round 22's for workers in the real-time class (`Waiting::Bounded`): spin until a share of
//! the period **after the block was armed**, then park — so a worker is never busy more than
//! that share of a period unless the block's own work keeps it so, and MMCSS never throttles
//! it (rt's measured finding: one spinning MMCSS thread throttles the callback too). A
//! parked worker is woken by the callback's next arm: an idle → busy transition, counted.

use super::queue::ReadyQueue;
use super::{Calm, Chaos, Graph, Pad, Topology};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering::*};
use std::sync::Arc;
use std::thread::{self, JoinHandle, Thread};
use std::time::{Duration, Instant};

#[path = "worker.rs"]
mod worker;
pub(crate) use worker::Shared;

/// The default number of live workers: physical performance cores minus two — one for the
/// callback, one left to the OS, the UI and the driver (ADR 0031 §2.1) — at least 1, at
/// most 14.
pub fn default_workers(performance_cores: usize) -> usize {
    performance_cores.saturating_sub(2).clamp(1, 14)
}

/// The most slots a pool may have (the availability mask is one word).
pub const MAX_SLOTS: usize = 64;

/// Serial or parallel, chosen per block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// The adaptive rule of §2.5: serial below 25 % load for 32 blocks, parallel again
    /// above 35 %; the topology's [`Topology::serial_hint`] forces serial.
    Auto,
    /// Always the callback alone.
    Serial,
    /// Always the pool (when it has available workers).
    Parallel,
}

/// How a worker waits between blocks.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Idle {
    /// Spin `periods` device periods after its last task (bare polls, then `PAUSE`, the
    /// clock every 256 polls), then yield, then park after `park_after` without a block.
    /// For workers outside any throttled real-time class (`Waiting::Spins`).
    Spin { periods: f64, park_after: Duration },
    /// Spin until `share` of a period after the block was armed — and at least `min_spin`
    /// after the worker's last task, so back-to-back blocks never park it — then park until
    /// the next arm wakes it. For workers in the real-time class (`Waiting::Bounded`).
    Bounded { share: f64, min_spin: Duration },
}

impl Idle {
    /// Round 22's default for real-time workers: half a period, then park.
    pub const BOUNDED: Idle = Idle::Bounded {
        share: 0.5,
        min_spin: Duration::from_micros(50),
    };
}

/// The start hook: called on each worker thread with its slot and the device period. It
/// sets the thread up (priority, placement, the denormal guard), calls `serve` — the
/// worker's whole life in the pool — and tears down after it returns.
pub type StartHook = Arc<dyn Fn(usize, Duration, &mut dyn FnMut()) + Send + Sync>;

/// How the pool is built.
#[derive(Clone)]
pub struct Config {
    /// Live workers besides the callback (`W`); `0` is the serial engine.
    pub workers: usize,
    /// The most tasks a block may have; every buffer is sized for it at construction.
    pub max_tasks: usize,
    /// The most dependency edges a topology may have (0: eight per task).
    pub max_edges: usize,
    /// The device period; `Duration::ZERO` when not known yet (the workers wait for the
    /// first block's [`Pool::set_period`]).
    pub period: Duration,
    /// How workers wait between blocks.
    pub idle: Idle,
    /// Serial or parallel.
    pub mode: Mode,
    /// Called on each worker thread at start (`cantus-rt`'s promotion plugs in here).
    pub on_start: Option<StartHook>,
}

impl Config {
    /// `workers` live workers for graphs of up to `max_tasks` tasks, a 128-frame period at
    /// 48 kHz, round 21's spin-then-park-after-100-ms idle, the adaptive mode.
    pub fn new(workers: usize, max_tasks: usize) -> Self {
        Config {
            workers,
            max_tasks,
            max_edges: 0,
            period: Duration::from_nanos(2_666_667),
            idle: Idle::Spin {
                periods: 1.5,
                park_after: Duration::from_millis(100),
            },
            mode: Mode::Auto,
            on_start: None,
        }
    }
}

/// What one block did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BlockReport {
    /// Wall time of the block on the callback (arming to the last task), ns.
    pub wall_ns: u64,
    /// Time spent running tasks, summed over every slot, ns (the block's *work*).
    pub work_ns: u64,
    /// The callback ran the block alone.
    pub serial: bool,
    /// Parked workers woken at this block (an idle → busy transition).
    pub woke: u32,
    /// The graph had more tasks than the pool was built for: nothing ran (the graph
    /// builder answers with a bigger pool, the way `PdcNeed` is answered).
    pub too_big: bool,
}

/// Counters since the pool was built.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PoolStats {
    /// Blocks run by the pool.
    pub parallel_blocks: u64,
    /// Blocks run by the callback alone.
    pub serial_blocks: u64,
    /// Non-blocking wakes of parked workers (amended ADR 0005 rule 3: only at idle → busy).
    pub wakes: u64,
    /// Tasks pushed by a slot (made ready beside a continuation; the sources are seeded,
    /// not pushed).
    pub pushes: u64,
    /// Pushes that found a queue full and were retried (zero with a correct capacity).
    pub full_pushes: u64,
    /// Workers that have started and joined the pool.
    pub available: u32,
}

/// The real-time pool. Owned by the callback thread: [`Pool::run_block`] takes `&mut self`.
pub struct Pool<G: Graph, Q: ReadyQueue, C: Chaos = Calm> {
    sh: Arc<Shared<G, Q, C>>,
    threads: Vec<Thread>,
    handles: Vec<JoinHandle<()>>,
    /// The topology's callback half: predecessor counts and ranks, and whether it fits.
    len: usize,
    preds: Vec<u32>,
    ranks: Vec<u32>,
    serial_hint: bool,
    too_big: bool,
    /// Callback-owned scratch, sized at construction.
    sources: Vec<u32>,
    serial_pending: Vec<u32>,
    serial_stack: Vec<u32>,
    mode: Mode,
    serial: bool,
    quiet_blocks: u32,
    period_ns: u64,
    block: u64,
    stats: PoolStats,
}

impl<G: Graph, Q: ReadyQueue> Pool<G, Q, Calm> {
    /// A pool with no chaos (production) and an empty topology.
    pub fn new(graph: Arc<G>, config: Config) -> std::io::Result<Self> {
        Self::with_chaos(graph, config, Calm)
    }
}

impl<G: Graph + Topology, Q: ReadyQueue> Pool<G, Q, Calm> {
    /// A pool running a graph that is its own topology (the mock graphs).
    pub fn for_graph(graph: Arc<G>, config: Config) -> std::io::Result<Self> {
        Self::for_graph_with_chaos(graph, config, Calm)
    }
}

impl<G: Graph + Topology, Q: ReadyQueue, C: Chaos> Pool<G, Q, C> {
    /// [`Pool::for_graph`] with a chaos hook.
    pub fn for_graph_with_chaos(graph: Arc<G>, config: Config, chaos: C) -> std::io::Result<Self> {
        let mut pool = Self::with_chaos(graph.clone(), config, chaos)?;
        pool.set_topology(&*graph);
        Ok(pool)
    }
}

impl<G: Graph, Q: ReadyQueue, C: Chaos> Pool<G, Q, C> {
    /// Build the pool and start its workers. Everything the pool will ever need is
    /// allocated here; nothing after.
    pub fn with_chaos(graph: Arc<G>, config: Config, chaos: C) -> std::io::Result<Self> {
        let n = config.max_tasks.max(1);
        let edges = if config.max_edges == 0 {
            n * 8
        } else {
            config.max_edges
        };
        let workers = config.workers.min(MAX_SLOTS - 1);
        let slots = workers + 1;
        let sh = Arc::new(Shared::new(graph, Q::new(n, slots), chaos, n, edges, &config, slots));
        let mut handles = Vec::with_capacity(workers);
        for slot in 1..slots {
            let s = sh.clone();
            let hook = config.on_start.clone();
            let h = thread::Builder::new()
                .name(format!("cantus-pool-{slot}"))
                .spawn(move || s.life(slot, hook));
            match h {
                Ok(h) => handles.push(h),
                Err(e) => {
                    sh.stop.store(true, SeqCst);
                    for h in &handles {
                        h.thread().unpark();
                    }
                    for h in handles {
                        let _ = h.join();
                    }
                    return Err(e);
                }
            }
        }
        let threads = handles.iter().map(|h| h.thread().clone()).collect();
        Ok(Pool {
            sh,
            threads,
            handles,
            len: 0,
            preds: Vec::with_capacity(n),
            ranks: Vec::with_capacity(n),
            serial_hint: false,
            too_big: false,
            sources: Vec::with_capacity(n),
            serial_pending: vec![0; n],
            serial_stack: Vec::with_capacity(n),
            mode: config.mode,
            serial: false,
            quiet_blocks: 0,
            period_ns: config.period.as_nanos().max(1) as u64,
            block: 0,
            stats: PoolStats::default(),
        })
    }

    /// The graph the pool runs.
    pub fn graph(&self) -> &Arc<G> {
        &self.sh.graph
    }

    /// The chaos hook (tests read what it recorded).
    pub fn chaos(&self) -> &C {
        &self.sh.chaos
    }

    /// Live workers (`W`), started or not.
    pub fn workers(&self) -> usize {
        self.threads.len()
    }

    /// Workers that have started and joined the pool (promoted, when a hook promotes).
    pub fn available(&self) -> usize {
        self.sh.available.load(Acquire).count_ones() as usize
    }

    /// Counters since construction.
    pub fn stats(&self) -> PoolStats {
        PoolStats {
            pushes: self.sh.pushes.load(Relaxed),
            full_pushes: self.sh.full.load(Relaxed),
            available: self.available() as u32,
            ..self.stats
        }
    }

    /// Tasks run and continuations taken by `slot` since construction.
    pub fn slot_counts(&self, slot: usize) -> (u64, u64) {
        (
            self.sh.runs[slot].load(Relaxed),
            self.sh.conts[slot].load(Relaxed),
        )
    }

    /// Whether worker `slot` (1..=W) is parked right now.
    pub fn is_parked(&self, slot: usize) -> bool {
        self.sh.parked.get(slot).is_some_and(|p| p.load(SeqCst))
    }

    /// Change the mode; takes effect at the next block.
    pub fn set_mode(&mut self, mode: Mode) {
        self.mode = mode;
    }

    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// Whether the adaptive rule has the pool serial now.
    pub fn adaptive_serial(&self) -> bool {
        self.serial
    }

    /// The device period (the spin budget and the adaptive rule's load are measured
    /// against it). The first call on a pool built without one releases the workers to
    /// start (each start wakes a parked worker: counted). Callback only; allocation-free.
    pub fn set_period(&mut self, period: Duration) {
        let ns = period.as_nanos().max(1) as u64;
        if ns == self.period_ns && self.sh.period_ns.load(Relaxed) != 0 {
            return;
        }
        self.period_ns = ns;
        let first = self.sh.period_ns.swap(ns, SeqCst) == 0;
        if first {
            for th in &self.threads {
                th.unpark();
                self.stats.wakes += 1;
            }
        }
    }

    /// Install a new task graph shape: every task's predecessor count, successors and rank.
    /// Callback only, between blocks; allocation-free. A topology larger than the pool
    /// was built for is refused (every later block reports `too_big`) until a smaller one
    /// comes.
    pub fn set_topology(&mut self, t: &impl Topology) -> bool {
        let n = t.len();
        let mut edges = 0usize;
        for task in 0..n as u32 {
            edges += t.succs(task).len();
        }
        if n > self.serial_pending.len() || edges > self.sh.succs.len() {
            self.too_big = true;
            self.len = 0;
            return false;
        }
        self.too_big = false;
        self.len = n;
        self.preds.clear();
        self.preds.extend((0..n as u32).map(|task| t.preds(task)));
        self.ranks.clear();
        self.ranks
            .extend(t.ranks().iter().copied().filter(|&r| (r as usize) < n));
        self.serial_hint = t.serial_hint();
        let mut at = 0u32;
        for task in 0..n {
            self.sh.succ_start[task].store(at, Relaxed);
            for &s in t.succs(task as u32) {
                self.sh.succs[at as usize].store(s, Relaxed);
                at += 1;
            }
        }
        self.sh.succ_start[n].store(at, Relaxed);
        // Published to the workers by the next arm's epoch (SeqCst).
        true
    }

    /// Tasks in the current topology.
    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

// ---------------------------------------------------------------------------------------
// The callback's side
// ---------------------------------------------------------------------------------------

impl<G: Graph, Q: ReadyQueue, C: Chaos> Pool<G, Q, C> {
    /// Run one block of the graph and apply the adaptive rule to its work: every task
    /// exactly once, each after its predecessors. Called by the audio callback; never
    /// blocks, never allocates, never waits on a worker waking (it helps, and spins only on
    /// tasks already running).
    pub fn run_block(&mut self) -> BlockReport {
        let r = self.run_pass();
        self.adapt(r.work_ns, self.period_ns);
        r
    }

    /// One run of the graph, serial or parallel as the mode and the adaptive state say,
    /// **without** updating the adaptive rule — for a caller that runs the graph several
    /// times per audio block (passes) and judges the block as a whole ([`Pool::adapt`]).
    pub fn run_pass(&mut self) -> BlockReport {
        self.run_pass_as(None)
    }

    /// [`Pool::run_pass`], forced serial when `serial` says so (a stem render, which sums
    /// in an order only one thread keeps).
    pub fn run_pass_as(&mut self, force_serial: Option<bool>) -> BlockReport {
        let t0 = Instant::now();
        if self.too_big {
            return BlockReport {
                too_big: true,
                ..BlockReport::default()
            };
        }
        let n = self.len;
        let avail = self.sh.available.load(Acquire);
        let serial = force_serial.unwrap_or(false)
            || avail == 0
            || match self.mode {
                Mode::Serial => true,
                Mode::Parallel => false,
                Mode::Auto => self.serial || self.serial_hint,
            };
        self.block += 1;
        let report = if serial {
            self.run_serial(n);
            let wall = t0.elapsed().as_nanos() as u64;
            BlockReport {
                wall_ns: wall,
                work_ns: wall,
                serial: true,
                ..BlockReport::default()
            }
        } else {
            let woke = self.arm(n, avail, t0);
            self.sh.drain(0);
            let work = self.sh.busy.iter().map(|b| b.swap(0, Relaxed)).sum();
            BlockReport {
                wall_ns: t0.elapsed().as_nanos() as u64,
                work_ns: work,
                woke,
                ..BlockReport::default()
            }
        };
        if serial {
            self.stats.serial_blocks += 1;
        } else {
            self.stats.parallel_blocks += 1;
        }
        self.stats.wakes += u64::from(report.woke);
        report
    }

    /// §2.5 with hysteresis, for a block whose work was `work_ns` against a period of
    /// `period_ns`: 32 blocks in a row under 25 % → serial; one block over 35 % → parallel
    /// at the next block.
    pub fn adapt(&mut self, work_ns: u64, period_ns: u64) {
        let load = work_ns as f64 / period_ns.max(1) as f64;
        if self.serial {
            if load > 0.35 {
                self.serial = false;
                self.quiet_blocks = 0;
            }
        } else if load < 0.25 {
            self.quiet_blocks += 1;
            if self.quiet_blocks >= 32 {
                self.serial = true;
            }
        } else {
            self.quiet_blocks = 0;
        }
    }

    /// The callback alone: a topological walk with its own counters and stack (nothing
    /// shared with anyone, no queue). Homes stay as the last parallel block set them.
    fn run_serial(&mut self, n: usize) {
        let g = &*self.sh.graph;
        self.serial_stack.clear();
        self.serial_pending[..n].copy_from_slice(&self.preds[..n]);
        // Sources pushed least critical first, so the most critical pops first.
        for &t in self.ranks.iter().rev() {
            if self.serial_pending[t as usize] == 0 {
                self.serial_stack.push(t);
            }
        }
        if self.serial_stack.len() != self.preds.iter().filter(|p| **p == 0).count() {
            // `ranks` broke its contract (not a permutation): index order.
            self.serial_stack.clear();
            self.serial_stack
                .extend((0..n as u32).rev().filter(|&t| self.preds[t as usize] == 0));
        }
        while let Some(t) = self.serial_stack.pop() {
            g.run(t, 0);
            let (a, b) = self.sh.succ_range(t);
            for i in a..b {
                let s = self.sh.succs[i].load(Relaxed);
                let p = &mut self.serial_pending[s as usize];
                *p -= 1;
                if *p == 0 && self.serial_stack.len() < self.serial_stack.capacity() {
                    self.serial_stack.push(s);
                }
            }
        }
    }

    /// Arm a parallel block (§2.2 step 2) and wake parked workers. Returns the wakes.
    fn arm(&mut self, n: usize, avail: u64, t0: Instant) -> u32 {
        let sh = &*self.sh;
        sh.meta.set_ranks(&self.ranks);
        let mut zero = 0usize;
        for t in 0..n {
            let p = self.preds[t];
            sh.pending[t].store(p, Relaxed);
            zero += usize::from(p == 0);
        }
        // Every task without a predecessor is a source, not only the tracks: a bus
        // nothing feeds has none either (the prototype hung on one).
        self.sources.clear();
        for &t in &self.ranks {
            if self.preds[t as usize] == 0 && self.sources.len() < self.sources.capacity() {
                self.sources.push(t);
            }
        }
        if self.sources.len() != zero {
            // `ranks` broke its contract (not a permutation): index order rather than a
            // hung audio thread.
            self.sources.clear();
            self.sources
                .extend((0..n as u32).filter(|&t| self.preds[t as usize] == 0));
        }
        sh.remaining.store(n, Relaxed);
        // Only the callback and the workers that have joined are seeded.
        sh.queue.seed(&self.sources, &sh.meta, avail | 1);
        sh.chaos.arm(self.block);
        sh.armed_ns
            .store(t0.saturating_duration_since(sh.origin).as_nanos() as u64, Relaxed);
        // Publishes everything above (SeqCst: also orders it before the `parked` loads
        // below, the callback's half of the park protocol).
        sh.epoch.fetch_add(1, SeqCst);
        let mut woke = 0;
        for (w, th) in self.threads.iter().enumerate() {
            let slot = w + 1;
            if avail & (1 << slot) == 0 {
                continue; // Not started yet: it joins when it is ready.
            }
            let parked = &sh.parked[slot];
            if parked.load(SeqCst) && parked.swap(false, SeqCst) {
                // The one system call the callback may make (amended ADR 0005 rule 3):
                // non-blocking, at an idle-to-busy transition only, never waited on.
                th.unpark();
                woke += 1;
            }
        }
        woke
    }
}

impl<G: Graph, Q: ReadyQueue, C: Chaos> Drop for Pool<G, Q, C> {
    fn drop(&mut self) {
        self.sh.stop.store(true, SeqCst);
        for t in &self.threads {
            t.unpark();
        }
        for h in self.handles.drain(..) {
            let _ = h.join();
        }
    }
}

/// Padded atomics, `k` of them.
pub(crate) fn pads32(k: usize) -> Box<[Pad<AtomicU32>]> {
    (0..k).map(|_| Pad(AtomicU32::new(0))).collect()
}

pub(crate) fn pads64(k: usize) -> Box<[Pad<AtomicU64>]> {
    (0..k).map(|_| Pad(AtomicU64::new(0))).collect()
}

pub(crate) fn padbools(k: usize) -> Box<[Pad<AtomicBool>]> {
    (0..k).map(|_| Pad(AtomicBool::new(false))).collect()
}

pub(crate) fn atoms32(k: usize) -> Box<[AtomicU32]> {
    (0..k).map(|_| AtomicU32::new(0)).collect()
}

pub(crate) type Remaining = Pad<AtomicUsize>;

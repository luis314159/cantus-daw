//! The pool: W live workers plus the callback (ADR 0031 §2.1–§2.5).
//!
//! Slot 0 is the callback — the thread that calls [`Pool::run_block`] — and slots `1..=W`
//! are the workers, plain `std::thread`s that call [`Config::on_start`] first: that hook
//! is where `cantus-rt`'s promotion (MMCSS "Pro Audio", CPU sets, time constraint and
//! workgroup) plugs in next wave.

use super::queue::{Meta, ReadyQueue};
use super::{buggify, Calm, Chaos, Graph, Pad, Site};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering::*};
use std::sync::Arc;
use std::thread::{self, JoinHandle, Thread};
use std::time::{Duration, Instant};

/// How many successors a finishing task sorts by priority before pushing; more than this
/// (rare: a track feeding many sends) are pushed as they come.
const READY_SORT: usize = 16;

/// The default number of live workers: physical performance cores minus two — one for the
/// callback, one left to the OS, the UI and the driver (ADR 0031 §2.1) — at least 1, at
/// most 14.
pub fn default_workers(performance_cores: usize) -> usize {
    performance_cores.saturating_sub(2).clamp(1, 14)
}

/// Serial or parallel, chosen per block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// The adaptive rule of §2.5: serial below 25 % load for 32 blocks, parallel again
    /// above 35 %; the graph's [`Graph::serial_hint`] forces serial.
    Auto,
    /// Always the callback alone.
    Serial,
    /// Always the pool (when it has workers).
    Parallel,
}

/// The rt promotion hook: called once on each worker thread, with its slot, before it
/// does anything else.
pub type StartHook = Arc<dyn Fn(usize) + Send + Sync>;

/// How the pool is built.
#[derive(Clone)]
pub struct Config {
    /// Live workers besides the callback (`W`); `0` is the serial engine.
    pub workers: usize,
    /// The most tasks a block may have; every buffer is sized for it at construction.
    pub max_tasks: usize,
    /// The device period. A worker spins for 1.5 of these after a block (§2.4), so it is
    /// spinning when the next one comes.
    pub period: Duration,
    /// Silence after which a worker parks (§2.4: ~100 ms).
    pub park_after: Duration,
    /// Serial or parallel.
    pub mode: Mode,
    /// Called on each worker thread at start (`cantus-rt`'s promotion plugs in here).
    pub on_start: Option<StartHook>,
}

impl Config {
    /// `workers` live workers for graphs of up to `max_tasks` tasks, a 128-frame period at
    /// 48 kHz, parking after 100 ms, the adaptive mode.
    pub fn new(workers: usize, max_tasks: usize) -> Self {
        Config {
            workers,
            max_tasks,
            period: Duration::from_nanos(2_666_667),
            park_after: Duration::from_millis(100),
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
}

/// What every thread of the pool shares.
pub(crate) struct Shared<G, Q, C> {
    graph: Arc<G>,
    queue: Q,
    chaos: C,
    meta: Meta,
    pending: Box<[Pad<AtomicU32>]>,
    remaining: Pad<AtomicUsize>,
    epoch: Pad<AtomicU64>,
    stop: AtomicBool,
    /// Per slot: parked (workers only), busy ns this block, tasks run, continuations.
    parked: Box<[Pad<AtomicBool>]>,
    busy: Box<[Pad<AtomicU64>]>,
    runs: Box<[Pad<AtomicU64>]>,
    conts: Box<[Pad<AtomicU64>]>,
    pushes: AtomicU64,
    full: AtomicU64,
    spin_budget: Duration,
    park_after: Duration,
}

/// The real-time pool. Owned by the callback thread: [`Pool::run_block`] takes `&mut self`.
pub struct Pool<G: Graph, Q: ReadyQueue, C: Chaos = Calm> {
    sh: Arc<Shared<G, Q, C>>,
    threads: Vec<Thread>,
    handles: Vec<JoinHandle<()>>,
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
    /// A pool with no chaos (production).
    pub fn new(graph: Arc<G>, config: Config) -> std::io::Result<Self> {
        Self::with_chaos(graph, config, Calm)
    }
}

impl<G: Graph, Q: ReadyQueue, C: Chaos> Pool<G, Q, C> {
    /// Build the pool and start its workers. Everything the pool will ever need is
    /// allocated here; nothing after.
    pub fn with_chaos(graph: Arc<G>, config: Config, chaos: C) -> std::io::Result<Self> {
        let n = config.max_tasks.max(1);
        let slots = config.workers + 1;
        let pads32 = |k: usize| (0..k).map(|_| Pad(AtomicU32::new(0))).collect();
        let pads64 = |k: usize| (0..k).map(|_| Pad(AtomicU64::new(0))).collect::<Box<[_]>>();
        let sh = Arc::new(Shared {
            graph,
            queue: Q::new(n, slots),
            chaos,
            meta: Meta::new(n, slots),
            pending: pads32(n),
            remaining: Pad(AtomicUsize::new(0)),
            epoch: Pad(AtomicU64::new(0)),
            stop: AtomicBool::new(false),
            parked: (0..slots).map(|_| Pad(AtomicBool::new(false))).collect(),
            busy: pads64(slots),
            runs: pads64(slots),
            conts: pads64(slots),
            pushes: AtomicU64::new(0),
            full: AtomicU64::new(0),
            spin_budget: config.period.mul_f64(1.5),
            park_after: config.park_after.max(config.period.mul_f64(1.5)),
        });
        let mut handles = Vec::with_capacity(config.workers);
        for slot in 1..slots {
            let s = sh.clone();
            let hook = config.on_start.clone();
            let h = thread::Builder::new()
                .name(format!("cantus-sched-{slot}"))
                .spawn(move || {
                    if let Some(hook) = hook {
                        hook(slot);
                    }
                    s.worker(slot);
                });
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

    /// Live workers (`W`).
    pub fn workers(&self) -> usize {
        self.threads.len()
    }

    /// Counters since construction.
    pub fn stats(&self) -> PoolStats {
        PoolStats {
            pushes: self.sh.pushes.load(Relaxed),
            full_pushes: self.sh.full.load(Relaxed),
            ..self.stats
        }
    }

    /// Tasks run and continuations taken by `slot` since construction.
    pub fn slot_counts(&self, slot: usize) -> (u64, u64) {
        (self.sh.runs[slot].load(Relaxed), self.sh.conts[slot].load(Relaxed))
    }

    /// Whether worker `slot` (1..=W) is parked right now.
    pub fn is_parked(&self, slot: usize) -> bool {
        self.sh.parked.get(slot).is_some_and(|p| p.load(SeqCst))
    }

    /// Change the mode; takes effect at the next block.
    pub fn set_mode(&mut self, mode: Mode) {
        self.mode = mode;
    }
}

// ---------------------------------------------------------------------------------------
// The callback's side
// ---------------------------------------------------------------------------------------

impl<G: Graph, Q: ReadyQueue, C: Chaos> Pool<G, Q, C> {
    /// Run one block of the graph: every task exactly once, each after its predecessors.
    /// Called by the audio callback; never blocks, never allocates, never waits on a
    /// worker waking (it helps, and spins only on tasks already running).
    pub fn run_block(&mut self) -> BlockReport {
        let t0 = Instant::now();
        let n = self.sh.graph.len();
        if n > self.serial_pending.len() {
            return BlockReport {
                too_big: true,
                ..BlockReport::default()
            };
        }
        let serial = match self.mode {
            _ if self.threads.is_empty() => true,
            Mode::Serial => true,
            Mode::Parallel => false,
            Mode::Auto => self.serial || self.sh.graph.serial_hint(),
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
            let woke = self.arm(n);
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
        self.adapt(report.work_ns);
        report
    }

    /// §2.5 with hysteresis: 32 blocks in a row under 25 % of the period → serial; one block
    /// over 35 % → parallel at the next block.
    fn adapt(&mut self, work_ns: u64) {
        let load = work_ns as f64 / self.period_ns as f64;
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
        for t in 0..n as u32 {
            self.serial_pending[t as usize] = g.preds(t);
        }
        // Sources pushed least critical first, so the most critical pops first.
        for &t in g.ranks().iter().rev() {
            if (t as usize) < n && self.serial_pending[t as usize] == 0 {
                self.serial_stack.push(t);
            }
        }
        while let Some(t) = self.serial_stack.pop() {
            g.run(t, 0);
            for &s in g.succs(t) {
                let p = &mut self.serial_pending[s as usize];
                *p -= 1;
                if *p == 0 && self.serial_stack.len() < self.serial_stack.capacity() {
                    self.serial_stack.push(s);
                }
            }
        }
    }

    /// Arm a parallel block (§2.2 step 2) and wake parked workers. Returns the wakes.
    fn arm(&mut self, n: usize) -> u32 {
        let sh = &*self.sh;
        let g = &*sh.graph;
        let ranks = g.ranks();
        sh.meta.set_ranks(ranks);
        let mut zero = 0usize;
        for t in 0..n {
            let p = g.preds(t as u32);
            sh.pending[t].store(p, Relaxed);
            zero += usize::from(p == 0);
        }
        // Every task without a predecessor is a source, not only the tracks: a bus
        // nothing feeds has none either (the prototype hung on one).
        self.sources.clear();
        for &t in ranks {
            if (t as usize) < n && g.preds(t) == 0 && self.sources.len() < self.sources.capacity()
            {
                self.sources.push(t);
            }
        }
        if self.sources.len() != zero {
            // `ranks` broke its contract (not a permutation): index order rather than a
            // hung audio thread.
            self.sources.clear();
            self.sources.extend((0..n as u32).filter(|&t| g.preds(t) == 0));
        }
        sh.remaining.store(n, Relaxed);
        sh.queue.seed(&self.sources, &sh.meta);
        sh.chaos.arm(self.block);
        // Publishes everything above (SeqCst: also orders it before the `parked` loads
        // below, the callback's half of the park protocol).
        sh.epoch.fetch_add(1, SeqCst);
        let mut woke = 0;
        for (w, th) in self.threads.iter().enumerate() {
            let parked = &sh.parked[w + 1];
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

// ---------------------------------------------------------------------------------------
// Every slot's side
// ---------------------------------------------------------------------------------------

impl<G: Graph, Q: ReadyQueue, C: Chaos> Shared<G, Q, C> {
    /// Run ready tasks until the block has none left.
    fn drain(&self, slot: usize) {
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
        for &s in self.graph.succs(task) {
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

    /// A worker's life: wait for a block (spin for a time budget, then yield, then park
    /// after the silence), help with it, repeat.
    fn worker(&self, slot: usize) {
        let mut seen = self.epoch.load(Acquire);
        let mut last = Instant::now();
        loop {
            buggify!(&self.chaos, Site::Idle, slot);
            let mut polls = 0u32;
            loop {
                let e = self.epoch.load(Acquire);
                if e != seen {
                    seen = e;
                    break;
                }
                if self.stop.load(Relaxed) {
                    return;
                }
                polls = polls.wrapping_add(1);
                // R5: bare polls, then PAUSE; the clock only every 256 polls (PAUSE is
                // ~140 cycles on Skylake and later, so the budget is kept in time).
                if polls < 64 || polls % 256 != 0 {
                    std::hint::spin_loop();
                    continue;
                }
                let idle = last.elapsed();
                if idle < self.spin_budget {
                    continue;
                }
                if idle < self.park_after {
                    thread::yield_now();
                    continue;
                }
                // Park (Dekker with the callback's arm: the flag, then the epoch again).
                self.parked[slot].store(true, SeqCst);
                if self.epoch.load(SeqCst) != seen || self.stop.load(SeqCst) {
                    self.parked[slot].store(false, SeqCst);
                    continue;
                }
                thread::park();
                // Woken, or spuriously: the waker cleared the flag; clear it here too for
                // the spurious case, and start a fresh spin budget.
                self.parked[slot].store(false, SeqCst);
                last = Instant::now();
            }
            self.drain(slot);
            last = Instant::now();
        }
    }
}

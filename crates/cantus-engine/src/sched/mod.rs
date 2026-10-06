//! # The real-time multicore scheduler (ADR 0031 §2; amendments R2, R5, R13)
//!
//! A pool of live workers plus the driver's callback thread runs a static task graph once
//! per audio block. The graph is anything that implements [`Graph`]: a number of tasks,
//! each task's predecessor count and successors, a *ranks* array (every task once,
//! critical path first) and a way to run a task. This wave the scheduler runs against mock
//! graphs only (`tests/sched.rs`, [`fuzz`]); the next wave connects the engine's compiled
//! graph to it.
//!
//! **The block protocol** ([`Pool::run_block`], ADR 0031 §2.2): the callback *arms* the
//! block — every pending count reset from the graph, every task without a predecessor made
//! ready, critical path first, an epoch published — then **helps**: it is a worker like the
//! others for the block. A task that finishes decrements its successors' counts and makes
//! those that reach zero ready, keeping the most critical one in its **continuation slot**
//! (run next, its inputs hot in that core's cache). When nothing is ready and tasks remain,
//! the callback spins: what it waits for is tasks *already running*, never a worker waking.
//!
//! **Two ready queues behind one trait** ([`ReadyQueue`], R2): [`Vyukov`] — two bounded
//! MPMC array queues, *critical* (the top quarter of the ranks) and *normal*, the
//! architect's design — and [`ChaseLev`] — one fixed-capacity Chase–Lev deque per thread
//! (the callback's included) plus a per-thread *mailbox* the sources are seeded into by
//! their **home** (the thread that ran them last block; Acar, Blelloch and Blumofe's
//! affinity). A task is pushed at most once per block, so no buffer ever grows. The
//! difference that matters is progress: a thread preempted between claiming a Vyukov cell
//! and publishing it hides every later push from every consumer; a preempted deque owner
//! hides only the one task it was pushing. `docs/round21/sched.md` measures both.
//!
//! **Idle** ([`pool`], ADR 0031 §2.4 and amended ADR 0005 rule 3): a worker that finishes a
//! block spins for a *time* budget (1.5 device periods; R5's progressive backoff, the clock
//! read every few hundred polls), then yields, and **parks** only after ~100 ms without a
//! block. The callback wakes a parked worker with one non-blocking `unpark` at the first
//! block after the silence, never waits for it, and never makes a system call on a block
//! whose workers are spinning; each wake is counted ([`PoolStats::wakes`]).
//!
//! **Serial when parallel does not pay** (§2.5): below 25 % load for 32 blocks the callback
//! runs the graph alone (the workers spin out their budget and park); above 35 % it is
//! parallel again at the next block. Both give the same bits — the graph's canonical gather
//! order (§1.3) makes the schedule irrelevant to the output.
//!
//! **Rules.** Safe Rust only (`#![forbid(unsafe_code)]` holds: every shared cell is an
//! atomic); nothing allocates, locks or logs after [`Pool::new`]; the only system calls in
//! steady state are none (the idle → busy wake is the counted exception; `yield` and `park`
//! happen on workers with nothing to do, never on the callback).
//!
//! **Testing hooks** ([`Chaos`], `buggify!`): named sites — between a push's claim and its
//! publish, before a task runs, after it, inside a steal, before a worker waits — call the
//! pool's chaos hook when the crate is built with the `fuzz` feature, and compile to nothing
//! without it. [`fuzz`] holds the PCT-shaped schedule fuzz (R13) that uses them.

pub mod fuzz;
pub mod pool;
pub mod queue;

pub use pool::{default_workers, BlockReport, Config, Mode, Pool, PoolStats};
pub use queue::{ChaseLev, Meta, ReadyQueue, Vyukov};

use std::ops::Deref;

/// What the scheduler runs. Implemented by the engine's compiled graph (next wave) and by
/// the mock graphs of the tests.
///
/// **Contract.** The topology (`len`, `preds`, `succs`, `ranks`) changes only *between*
/// blocks, on the callback thread (through the implementor's own interior mutability);
/// during a block every thread reads the same topology. `run` is called exactly once per
/// task per block, on any thread of the pool, after every predecessor's `run` has
/// returned (and with a happens-before edge from it). `run` must not block, allocate or
/// panic (ADR 0005).
pub trait Graph: Send + Sync + 'static {
    /// How many tasks the graph has this block (tasks are `0..len`).
    fn len(&self) -> usize;
    /// Whether the graph has no task (then a block does nothing).
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// How many predecessors `task` has (its dependency count).
    fn preds(&self, task: u32) -> u32;
    /// The tasks that depend on `task`.
    fn succs(&self, task: u32) -> &[u32];
    /// Every task exactly once, **critical path first** (HEFT's upward rank, ADR 0031 §2.3).
    /// It need not be a topological order; the scheduler never relies on it for
    /// correctness, only for priority.
    fn ranks(&self) -> &[u32];
    /// Run `task` on pool slot `slot` (0 is the callback, `1..=W` the workers — per-slot
    /// scratch belongs to the slot).
    fn run(&self, task: u32, slot: usize);
    /// `true` when the graph knows parallel will not pay this block (one long chain: the
    /// critical path above 80 % of the total cost, §2.5). The pool then runs it serially.
    fn serial_hint(&self) -> bool {
        false
    }
}

/// The named points where the schedule fuzz may interfere ([`fuzz::Pct`]). Only reached
/// with the `fuzz` feature (`buggify!`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Site {
    /// Between a push's claim (a Vyukov cell won, a deque cell written) and its publish.
    Claimed,
    /// Between a pop's or steal's read and its claim (a thief "preempted mid-steal").
    Steal,
    /// Just before a task runs (a worker preempted with a task in hand).
    Run,
    /// After a task ran, before its successors are released.
    Finish,
    /// A worker about to wait for the next block (a late worker).
    Idle,
}

/// A hook the pool calls at every [`Site`] (with the `fuzz` feature) and when it arms a
/// block. Production uses [`Calm`], which does nothing and compiles away.
pub trait Chaos: Send + Sync + 'static {
    /// The callback armed block `block` (before the epoch is published).
    fn arm(&self, _block: u64) {}
    /// Thread `slot` reached `site`.
    fn at(&self, _site: Site, _slot: usize) {}
}

/// No chaos: the production hook.
#[derive(Clone, Copy, Debug, Default)]
pub struct Calm;

impl Chaos for Calm {}

/// `buggify!(chaos, site, slot)`: call the chaos hook at a named site — only when the crate
/// is built with the `fuzz` feature; otherwise nothing is evaluated (FoundationDB's
/// BUGGIFY, R13).
macro_rules! buggify {
    ($chaos:expr, $site:expr, $slot:expr) => {{
        #[cfg(feature = "fuzz")]
        {
            $crate::sched::Chaos::at($chaos, $site, $slot);
        }
    }};
}
pub(crate) use buggify;

/// A value alone on its own pair of cache lines: 128 bytes, because Intel's spatial
/// prefetcher fetches 64-byte lines in pairs (R11; crossbeam's `CachePadded` is 128 on
/// x86-64 for the same reason). `cantus-rt`'s `pad.rs` is the shared home of this type once
/// it lands.
#[derive(Default, Debug)]
#[repr(align(128))]
pub struct Pad<T>(pub T);

impl<T> Deref for Pad<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.0
    }
}

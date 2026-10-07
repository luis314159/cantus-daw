//! The schedule fuzz (ADR 0031 "What the tests hold", amendment R13): PCT-shaped episodes
//! with replay tokens and FoundationDB's "unseed" check, and the chaos hooks the tests and
//! the bench use.
//!
//! **An episode** comes entirely from one seed: a small random DAG (short episodes keep
//! PCT's bound useful — it degrades as k^(d−1) with the number of steps k), **random task
//! priorities** (the ranks array is a random permutation, unrelated to the topology), a
//! worker count, a few blocks, and **two or three change points** — (block, step, stall) —
//! at which whichever thread reaches the scheduler's `step`-th site in that block is
//! "preempted": it stalls there, mid-push, mid-steal, with a task in hand, or between a
//! task and its successors (Burckhardt, Kothari, Musuvathi and Nagarakatte, ASPLOS 2010:
//! random priorities, d − 1 change points). The sites fire only with the `fuzz` feature;
//! without it an episode still checks random graphs under random priorities and worker
//! counts, without the stalls.
//!
//! **The oracle** ([`FuzzGraph`]): every task runs exactly once per block, starts only
//! after each predecessor *finished* in the same block, and the graph's output — each task
//! mixes its inputs in canonical (index) order — hashes the same as the serial run.
//!
//! **Replay** ([`Token`]): a failing episode prints `sched1-<seed>`; `Token::parse` and
//! [`Episode::from_token`] rebuild it exactly. **Unseed**: each seed runs twice; the final
//! state of the episode's generator and the output hash must match (a different state means
//! something outside the seed — the clock, the schedule — leaked into the episode's
//! randomness).

use super::pool::{Config, Mode, Pool};
use super::queue::ReadyQueue;
use super::{Chaos, Graph, Site, Topology};
use std::fmt;
use std::sync::atomic::{AtomicU32, AtomicU64, AtomicUsize, Ordering::*};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// SplitMix64: tiny, fast, and the whole state is one `u64` (so it can be compared).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rng(u64);

impl Rng {
    /// A generator from a seed.
    pub fn new(seed: u64) -> Self {
        Rng(seed)
    }

    /// The whole state.
    pub fn state(&self) -> u64 {
        self.0
    }

    /// The next 64 random bits.
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        mix(self.0)
    }

    /// Uniform in `0..n` (`n > 0`).
    pub fn below(&mut self, n: u64) -> u64 {
        ((u128::from(self.next_u64()) * u128::from(n.max(1))) >> 64) as u64
    }

    /// Uniform in `lo..=hi`.
    pub fn range(&mut self, lo: u64, hi: u64) -> u64 {
        lo + self.below(hi - lo + 1)
    }

    /// `true` with probability `num / den`.
    pub fn chance(&mut self, num: u64, den: u64) -> bool {
        self.below(den) < num
    }
}

/// SplitMix64's finaliser: a good 64-bit mix.
pub fn mix(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// A replay token: everything an episode is, in one string (`sched1-<seed in hex>`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Token(pub u64);

impl Token {
    /// Parse `sched1-<hex>`.
    pub fn parse(s: &str) -> Option<Token> {
        let hex = s.trim().strip_prefix("sched1-")?;
        u64::from_str_radix(hex, 16).ok().map(Token)
    }
}

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "sched1-{:016x}", self.0)
    }
}

/// A PCT change point: in block `block`, whichever thread reaches the `step`-th site stalls
/// for `stall`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChangePoint {
    pub block: u64,
    pub step: u64,
    pub stall: Duration,
}

/// One fuzz episode, all from one seed.
#[derive(Clone, Debug)]
pub struct Episode {
    pub token: Token,
    /// Each task's predecessors, in canonical (index) order. Task indices are a
    /// topological order of the DAG.
    pub preds: Vec<Vec<u32>>,
    /// Each task's busy-wait cost.
    pub cost: Vec<Duration>,
    /// The random priorities: a permutation of the tasks.
    pub ranks: Vec<u32>,
    pub workers: usize,
    pub blocks: u64,
    pub changes: Vec<ChangePoint>,
    /// The generator's state once the episode is built (part of the unseed check).
    pub rng: Rng,
}

impl Episode {
    /// The episode a token stands for.
    pub fn from_token(token: Token) -> Episode {
        let mut rng = Rng::new(token.0);
        let n = rng.range(1, 40) as usize;
        let mut preds = Vec::with_capacity(n);
        for t in 0..n {
            let mut p: Vec<u32> = Vec::new();
            if t > 0 && !rng.chance(1, 4) {
                for _ in 0..rng.range(1, 3.min(t as u64)) {
                    let q = rng.below(t as u64) as u32;
                    if !p.contains(&q) {
                        p.push(q);
                    }
                }
            }
            p.sort_unstable();
            preds.push(p);
        }
        let cost = (0..n)
            .map(|_| {
                Duration::from_micros(if rng.chance(1, 3) {
                    0
                } else {
                    rng.range(1, 40)
                })
            })
            .collect();
        let mut ranks: Vec<u32> = (0..n as u32).collect();
        for i in (1..n).rev() {
            ranks.swap(i, rng.below(i as u64 + 1) as usize);
        }
        let workers = rng.range(1, 3) as usize;
        let blocks = rng.range(2, 5);
        let steps = 4 * n as u64 + 4;
        let changes = (0..rng.range(2, 3))
            .map(|_| ChangePoint {
                block: rng.range(1, blocks),
                step: rng.below(steps),
                stall: Duration::from_micros(rng.range(50, 600)),
            })
            .collect();
        Episode {
            token,
            preds,
            cost,
            ranks,
            workers,
            blocks,
            changes,
            rng,
        }
    }
}

/// Busy-wait for `d` (a stall that keeps its core, like a preempted thread's victim sees).
pub fn spin_for(d: Duration) {
    if d.is_zero() {
        return;
    }
    let until = Instant::now() + d;
    while Instant::now() < until {
        std::hint::spin_loop();
    }
}

/// The PCT-shaped chaos: at each change point's step of its block, the thread that reaches
/// the scheduler's site stalls.
pub struct Pct {
    changes: Vec<ChangePoint>,
    block: AtomicU64,
    step: AtomicU64,
    /// How many change points fired, and at which sites (indexed like [`site_index`]).
    pub fired: AtomicU32,
    pub fired_at: [AtomicU32; 5],
}

/// A site's index in [`Pct::fired_at`].
pub fn site_index(site: Site) -> usize {
    match site {
        Site::Claimed => 0,
        Site::Steal => 1,
        Site::Run => 2,
        Site::Finish => 3,
        Site::Idle => 4,
    }
}

impl Pct {
    pub fn new(changes: Vec<ChangePoint>) -> Self {
        Pct {
            changes,
            block: AtomicU64::new(0),
            step: AtomicU64::new(0),
            fired: AtomicU32::new(0),
            fired_at: Default::default(),
        }
    }
}

impl Chaos for Pct {
    fn arm(&self, block: u64) {
        self.step.store(0, Relaxed);
        self.block.store(block, Relaxed);
    }

    fn at(&self, site: Site, _slot: usize) {
        if site == Site::Idle {
            return;
        }
        let s = self.step.fetch_add(1, Relaxed);
        let b = self.block.load(Relaxed);
        for cp in &self.changes {
            if cp.block == b && cp.step == s {
                self.fired.fetch_add(1, Relaxed);
                self.fired_at[site_index(site)].fetch_add(1, Relaxed);
                spin_for(cp.stall);
            }
        }
    }
}

/// A deterministic stall: in every block, the `nth` time a *worker* (or any slot, with
/// `workers_only = false`) reaches `site`, it stalls for `dur`. With `spread > 0` the
/// `nth` of each block is `mix(block) % spread` instead. The tests' and the bench's
/// "stalled pusher" is `site = Site::Claimed`.
pub struct Stall {
    pub site: Site,
    pub nth: u64,
    pub spread: u64,
    pub dur: Duration,
    pub workers_only: bool,
    count: AtomicU64,
    target: AtomicU64,
    /// How many stalls happened.
    pub fired: AtomicU64,
}

impl Stall {
    pub fn new(site: Site, nth: u64, dur: Duration) -> Self {
        Stall {
            site,
            nth,
            spread: 0,
            dur,
            workers_only: true,
            count: AtomicU64::new(0),
            target: AtomicU64::new(nth),
            fired: AtomicU64::new(0),
        }
    }
}

impl Chaos for Stall {
    fn arm(&self, block: u64) {
        let nth = if self.spread > 0 {
            mix(block) % self.spread
        } else {
            self.nth
        };
        self.target.store(nth, Relaxed);
        self.count.store(0, Relaxed);
    }

    fn at(&self, site: Site, slot: usize) {
        if site != self.site || (self.workers_only && slot == 0) {
            return;
        }
        if self.count.fetch_add(1, Relaxed) == self.target.load(Relaxed) {
            self.fired.fetch_add(1, Relaxed);
            spin_for(self.dur);
        }
    }
}

/// The checking graph: every task mixes its inputs in canonical order, and records
/// whether it ran twice, ran before a predecessor finished, or did not run.
pub struct FuzzGraph {
    preds: Vec<Vec<u32>>,
    succs: Vec<Vec<u32>>,
    cost: Vec<Duration>,
    ranks: Vec<u32>,
    block: AtomicU64,
    /// Cost multiplier for this block (0, 1 or 2), drawn from the episode's generator.
    jitter: AtomicU32,
    started: Vec<AtomicU64>,
    done: Vec<AtomicU64>,
    value: Vec<AtomicU64>,
    pub double: AtomicUsize,
    pub early: AtomicUsize,
}

impl FuzzGraph {
    pub fn new(preds: Vec<Vec<u32>>, cost: Vec<Duration>, ranks: Vec<u32>) -> Self {
        let n = preds.len();
        let mut succs = vec![Vec::new(); n];
        for (t, ps) in preds.iter().enumerate() {
            for &p in ps {
                succs[p as usize].push(t as u32);
            }
        }
        let atoms = || (0..n).map(|_| AtomicU64::new(0)).collect::<Vec<_>>();
        FuzzGraph {
            preds,
            succs,
            cost,
            ranks,
            block: AtomicU64::new(0),
            jitter: AtomicU32::new(1),
            started: atoms(),
            done: atoms(),
            value: atoms(),
            double: AtomicUsize::new(0),
            early: AtomicUsize::new(0),
        }
    }

    /// Before block `b` (callback thread, between blocks).
    pub fn begin(&self, b: u64, jitter: u32) {
        self.block.store(b, Relaxed);
        self.jitter.store(jitter, Relaxed);
    }

    /// After block `b`: how many tasks did not run, and the hash of every output.
    pub fn end(&self, b: u64) -> (usize, u64) {
        let missed = self.done.iter().filter(|d| d.load(Acquire) != b).count();
        let hash = self.value.iter().fold(b, |h, v| mix(h ^ v.load(Relaxed)));
        (missed, hash)
    }
}

impl Topology for FuzzGraph {
    fn len(&self) -> usize {
        self.preds.len()
    }
    fn preds(&self, task: u32) -> u32 {
        self.preds[task as usize].len() as u32
    }
    fn succs(&self, task: u32) -> &[u32] {
        &self.succs[task as usize]
    }
    fn ranks(&self) -> &[u32] {
        &self.ranks
    }
}

impl Graph for FuzzGraph {
    fn run(&self, task: u32, _slot: usize) {
        let t = task as usize;
        let b = self.block.load(Relaxed);
        if self.started[t].swap(b, AcqRel) == b {
            self.double.fetch_add(1, Relaxed);
        }
        let mut v = mix(task as u64 ^ (b << 32));
        for &p in &self.preds[t] {
            if self.done[p as usize].load(Acquire) != b {
                self.early.fetch_add(1, Relaxed);
            }
            v = mix(v ^ self.value[p as usize].load(Relaxed));
        }
        spin_for(self.cost[t] * self.jitter.load(Relaxed));
        self.value[t].store(v, Relaxed);
        self.done[t].store(b, Release);
    }
}

/// What an episode did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Outcome {
    pub token: Token,
    pub queue: &'static str,
    /// The episode generator's final state (the unseed check).
    pub rng: u64,
    /// The parallel run's output hash, and the serial run's.
    pub hash: u64,
    pub serial_hash: u64,
    /// Tasks run twice, run before a predecessor finished, not run.
    pub double: usize,
    pub early: usize,
    pub missed: usize,
    /// Change points that fired, and how many at `Site::Claimed`.
    pub fired: u32,
    pub fired_claimed: u32,
}

impl Outcome {
    /// `Ok` when the oracle holds; otherwise what failed, with the replay token.
    pub fn verdict(&self) -> Result<(), String> {
        if self.double + self.early + self.missed > 0 || self.hash != self.serial_hash {
            return Err(format!(
                "{} ({}): {} ran twice, {} ran early, {} never ran, hash {:016x} vs serial {:016x}",
                self.token,
                self.queue,
                self.double,
                self.early,
                self.missed,
                self.hash,
                self.serial_hash
            ));
        }
        Ok(())
    }
}

fn run_blocks<Q: ReadyQueue, C: Chaos>(
    ep: &Episode,
    workers: usize,
    chaos: C,
) -> (Pool<FuzzGraph, Q, C>, Rng, u64, usize) {
    let g = Arc::new(FuzzGraph::new(
        ep.preds.clone(),
        ep.cost.clone(),
        ep.ranks.clone(),
    ));
    let mut cfg = Config::new(workers, ep.preds.len());
    cfg.mode = Mode::Parallel;
    let mut pool = Pool::<FuzzGraph, Q, C>::for_graph_with_chaos(g.clone(), cfg, chaos)
        .expect("the fuzz could not start its workers");
    let mut rng = ep.rng.clone();
    let (mut hash, mut missed) = (0u64, 0usize);
    for b in 1..=ep.blocks {
        g.begin(b, rng.below(3) as u32);
        pool.run_block();
        let (m, h) = g.end(b);
        missed += m;
        hash = mix(hash ^ h);
    }
    (pool, rng, hash, missed)
}

/// Run one episode with queue `Q`: the parallel run under the episode's change points,
/// then the same graph serially for the reference hash.
pub fn run_episode<Q: ReadyQueue>(ep: &Episode) -> Outcome {
    let (pool, rng, hash, missed) =
        run_blocks::<Q, Pct>(ep, ep.workers, Pct::new(ep.changes.clone()));
    let g = pool.graph().clone();
    let (fired, fired_claimed) = (
        pool.chaos().fired.load(Relaxed),
        pool.chaos().fired_at[site_index(Site::Claimed)].load(Relaxed),
    );
    drop(pool);
    let (_, _, serial_hash, _) = run_blocks::<Q, super::Calm>(ep, 0, super::Calm);
    Outcome {
        token: ep.token,
        queue: Q::NAME,
        rng: rng.state(),
        hash,
        serial_hash,
        double: g.double.load(Relaxed),
        early: g.early.load(Relaxed),
        missed,
        fired,
        fired_claimed,
    }
}

/// Run the episode a token stands for **twice** under a watchdog: both runs must pass the
/// oracle, and the generator's final state and the hash must be the same both times
/// (FoundationDB's unseed check). A run that makes no progress within `timeout` is a
/// deadlock or a lost task; the error names the token either way.
pub fn check<Q: ReadyQueue>(token: Token, timeout: Duration) -> Result<Outcome, String> {
    let once = || -> Result<Outcome, String> {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::Builder::new()
            .name(format!("fuzz-{token}"))
            .spawn(move || {
                let _ = tx.send(run_episode::<Q>(&Episode::from_token(token)));
            })
            .map_err(|e| format!("{token}: could not start the episode: {e}"))?;
        match rx.recv_timeout(timeout) {
            Ok(o) => o.verdict().map(|()| o),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => Err(format!(
                "{token} ({}): no progress in {timeout:?}: a deadlock or a lost task",
                Q::NAME
            )),
            Err(_) => Err(format!("{token} ({}): the episode panicked", Q::NAME)),
        }
    };
    let a = once()?;
    let b = once()?;
    if (a.rng, a.hash) != (b.rng, b.hash) {
        return Err(format!(
            "{token} ({}): not deterministic: generator {:016x} vs {:016x}, hash {:016x} vs {:016x}",
            Q::NAME,
            a.rng,
            b.rng,
            a.hash,
            b.hash
        ));
    }
    Ok(a)
}

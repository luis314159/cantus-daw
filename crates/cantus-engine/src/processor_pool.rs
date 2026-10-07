//! The processor's pool (ADR 0031 §2.1, §2.6): how many live workers, how they start
//! (placement, `cantus_rt::promote_worker` with a bounded spin, the denormal guard), the
//! task map and clusters it runs, the costs those come from, and the late block's
//! critical path for the blame.

use super::ctx::Job;
use super::*;
use crate::profile::Node;
use crate::sched::{Chaos, Config, DynChaos, Idle, Mode, Pool};
use std::sync::atomic::Ordering::Relaxed;
use std::sync::OnceLock;

/// The most live workers a pool may have (ADR 0031 §2.1), and the slots every graph is
/// built with (the callback's included).
pub const MAX_WORKERS: usize = 14;
pub(crate) const SLOTS: usize = MAX_WORKERS + 1;

/// How many live workers a processor runs, and how (`Processor::set_workers`).
#[derive(Clone)]
pub struct Workers {
    /// Live workers besides the callback; 0 is the serial engine.
    pub count: usize,
    /// Serial or parallel per block (§2.5's adaptive rule, or forced).
    pub mode: Mode,
    /// Promote the workers (`cantus_rt::promote_worker`, MMCSS "Pro Audio" on Windows,
    /// with a bounded spin) and place them on performance cores. The application's pool
    /// does; tests and the offline paths never touch thread priority.
    pub promote: bool,
    /// How the workers wait between blocks: `false` (the default) spins at most half a
    /// period after the block was armed, then parks — safe for MMCSS "Pro Audio", which a
    /// promoted worker then gets (`Waiting::Bounded`); `true` spins 1.5 periods and parks
    /// only after 100 ms of silence — a promoted worker then gets the highest priority no
    /// OS throttles instead (`Waiting::Spins`: Windows TIME_CRITICAL, no MMCSS).
    pub spin: bool,
    /// For bounded workers: the share of a period after the block's arm they spin before
    /// parking (at most 0.65, what MMCSS allows with margin). 0.1 by default: round 22's
    /// paced runs missed fewer periods at 0.1 than at 0.5 in every pair — a laptop's power
    /// budget, its turbo clock, is better spent on the threads doing the work.
    pub spin_share: f32,
    /// The schedule fuzz's hook (`sched::fuzz`), and a seed that shuffles the tasks'
    /// priorities and switches clustering — a test's way of trying other schedules.
    pub chaos: Option<Arc<dyn Chaos>>,
    pub shuffle: Option<u64>,
}

impl Workers {
    /// No pool: the callback runs every node.
    pub const SERIAL: Workers = Workers {
        count: 0,
        mode: Mode::Serial,
        promote: false,
        spin: false,
        spin_share: 0.1,
        chaos: None,
        shuffle: None,
    };

    /// `count` workers, promoted and placed, with the adaptive rule (the application's).
    pub fn live(count: usize) -> Workers {
        Workers {
            count: count.min(MAX_WORKERS),
            mode: Mode::Auto,
            promote: true,
            spin: false,
            spin_share: 0.1,
            chaos: None,
            shuffle: None,
        }
    }

    /// The default count for this machine: performance cores minus two (ADR 0031 §2.1).
    pub fn auto() -> Workers {
        Workers::live(default_workers())
    }

    /// `count` workers that always run the pool, unpromoted: the tests' and the bench's.
    pub fn parallel(count: usize) -> Workers {
        Workers {
            count: count.min(MAX_WORKERS),
            mode: Mode::Parallel,
            promote: false,
            spin: false,
            spin_share: 0.1,
            chaos: None,
            shuffle: None,
        }
    }

    /// From `CANTUS_WORKERS` — `0` (or unset): serial; `N`: N workers always parallel;
    /// `N:auto`: N workers with the adaptive rule; `auto`: the machine's default — so every
    /// engine test and the bench run at any `W` without a change (`Processor::new`).
    /// `CANTUS_WORKERS_PROMOTE=1` promotes them as the application does;
    /// `CANTUS_WORKERS_SPIN=1` makes them spinners (see [`Workers::spin`]).
    pub fn from_env() -> Workers {
        let Ok(v) = std::env::var("CANTUS_WORKERS") else {
            return Workers::SERIAL;
        };
        let v = v.trim();
        let mut w = match v.split_once(':') {
            _ if v == "auto" => Workers::auto(),
            Some((n, "auto")) => Workers {
                mode: Mode::Auto,
                ..Workers::parallel(n.trim().parse().unwrap_or(0))
            },
            _ => Workers::parallel(v.parse().unwrap_or(0)),
        };
        w.promote = std::env::var("CANTUS_WORKERS_PROMOTE").is_ok_and(|p| p == "1");
        w.spin = std::env::var("CANTUS_WORKERS_SPIN").is_ok_and(|p| p == "1");
        if let Some(share) = std::env::var("CANTUS_WORKERS_SHARE")
            .ok()
            .and_then(|v| v.parse().ok())
        {
            w.spin_share = share;
        }
        if w.count == 0 {
            return Workers::SERIAL;
        }
        w
    }

    pub fn count(&self) -> usize {
        self.count
    }
}

impl std::fmt::Debug for Workers {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Workers")
            .field("count", &self.count)
            .field("mode", &self.mode)
            .field("promote", &self.promote)
            .field("spin", &self.spin)
            .field("spin_share", &self.spin_share)
            .field("chaos", &self.chaos.is_some())
            .field("shuffle", &self.shuffle)
            .finish()
    }
}

/// The machine's default number of live workers (ADR 0031 §2.1: the live cache domain's
/// performance cores minus two, 1–14), read from the OS once.
pub fn default_workers() -> usize {
    static N: OnceLock<usize> = OnceLock::new();
    *N.get_or_init(|| cantus_rt::Topology::detect().default_live_workers())
}

/// A function every pool worker calls on its own thread with `true` when it starts
/// serving and `false` when it stops: for tests that watch the workers
/// (`tests/rt_memory.rs` arms its allocation counter there). Set once per process.
static WORKER_HOOK: OnceLock<fn(usize, bool)> = OnceLock::new();

/// Install the worker-start hook (see [`WORKER_HOOK`]); later calls are ignored.
pub fn on_worker_start(f: fn(usize, bool)) {
    let _ = WORKER_HOOK.set(f);
}

/// The late block's critical path: the chain of nodes that finished last, back from the
/// master (ADR 0031 §2.9). Only these are blamed for an overrun of a parallel block.
pub(super) struct CriticalPath {
    tracks: [bool; MAX_TRACKS],
    strips: [bool; MAX_STRIPS],
}

impl CriticalPath {
    pub fn holds(&self, n: Node) -> bool {
        match n {
            Node::Engine | Node::MasterInsert(_) => true,
            Node::Track(s) => self.tracks.get(s).copied().unwrap_or(false),
            Node::Strip(s) | Node::Insert(s, _) => self.strips.get(s).copied().unwrap_or(false),
        }
    }
}

impl Processor {
    /// Run with `workers` live workers from now on: the old pool's threads are stopped
    /// and joined, the new ones started (they wait for the first block's period, then
    /// promote themselves and join). Off the audio thread: it allocates and spawns.
    pub fn set_workers(&mut self, workers: Workers) {
        self.pool = None;
        self.map_dirty = true;
        self.shuffle = workers.shuffle;
        if workers.count == 0 {
            return;
        }
        let count = workers.count.min(MAX_WORKERS);
        let mut cfg = Config::new(count, crate::graph::MAX_NODES);
        cfg.max_edges = crate::graph::MAX_NODES * 40;
        cfg.period = std::time::Duration::ZERO;
        cfg.idle = if workers.spin {
            Idle::Spin {
                periods: 1.5,
                park_after: std::time::Duration::from_millis(100),
            }
        } else {
            Idle::Bounded {
                share: f64::from(workers.spin_share.clamp(0.0, 0.65)),
                min_spin: std::time::Duration::from_micros(50),
            }
        };
        let waiting = if workers.spin {
            cantus_rt::thread::Waiting::Spins
        } else {
            cantus_rt::thread::Waiting::Bounded {
                busy_share: workers.spin_share.clamp(0.5, 0.65),
            }
        };
        cfg.mode = workers.mode;
        let placement = if workers.promote {
            cantus_rt::Topology::detect().placement(count).live
        } else {
            Vec::new()
        };
        let promote = workers.promote;
        cfg.on_start = Some(Arc::new(move |slot, period, serve: &mut dyn FnMut()| {
            // Flush denormals for the worker's whole life (ADR 0031 §5.2), its stack
            // touched once (R11), its core a performance core (R5), and real-time priority
            // safe for a thread that spins at most half a period (rt's finding).
            let _ftz = cantus_rt::DenormalGuard::new();
            cantus_rt::memory::prefault_stack();
            if let Some(cpu) = placement.get(slot - 1) {
                let _ = cantus_rt::thread::set_current_thread_cpus(&[*cpu]);
            }
            let _promotion = promote.then(|| {
                cantus_rt::thread::promote_worker(
                    cantus_rt::thread::WorkerRole::Live,
                    period,
                    waiting,
                )
            });
            if let Some(hook) = WORKER_HOOK.get() {
                hook(slot, true);
            }
            serve();
            if let Some(hook) = WORKER_HOOK.get() {
                hook(slot, false);
            }
        }));
        match Pool::with_chaos(self.nodes.clone(), cfg, DynChaos(workers.chaos)) {
            Ok(p) => {
                self.pool_wakes = 0;
                self.pool = Some(p);
            }
            Err(e) => log::warn!("audio pool: could not start {count} workers: {e}"),
        }
    }

    /// Live workers this processor runs (`W`), started or not.
    pub fn workers(&self) -> usize {
        self.pool.as_ref().map_or(0, |p| p.workers())
    }

    /// Resolve the plan to slots and lines, cluster it into tasks and hand their shape to
    /// the pool. Allocation-free; on the callback, between passes.
    pub(super) fn rebuild_map(&mut self) {
        let nodes = self.nodes.clone();
        let Some(mut frame) = nodes.frame.try_borrow_mut() else {
            nodes.conflict();
            return;
        };
        let map = &mut frame.map;
        map.resolve(
            &self.plan,
            &self.track_to_slot,
            &self.strip_to_slot,
            |s, k| nodes.strip(s).and_then(|st| st.send_line[k]),
        );
        self.map_dirty = false;
        self.clustered_at = self.block;
        let Some(pool) = self.pool.as_mut() else {
            map.one_task_per_node();
            return;
        };
        self.node_cost.clear();
        for job in &map.jobs {
            self.node_cost.push(match *job {
                Job::Track { slot, .. } => f64::from(self.track_cost[slot as usize]),
                Job::Strip { slot } => f64::from(self.strip_cost[slot as usize]),
                Job::Master => 0.0,
            });
        }
        // A test's other schedules: clustering on or off, priorities shuffled.
        let (cluster, shuffle) = match self.shuffle {
            Some(seed) => {
                let r = crate::sched::fuzz::mix(seed ^ self.block);
                (r & 1 == 0, Some(r))
            }
            None => (true, None),
        };
        let cost = &self.node_cost;
        self.topo.build(&self.plan, map, |n| cost[n], cluster);
        if let Some(r) = shuffle {
            self.topo.shuffle_ranks(r);
        }
        pool.set_topology(&self.topo);
    }

    /// Each node's cost this block, from the profiler (smoothed, per slot: slots keep their
    /// node across plans); and a new clustering when it is due.
    pub(super) fn measure_costs(&mut self) {
        if self.pool.is_none() || !self.prof.is_on() {
            return;
        }
        let nodes = self.nodes.clone();
        if let Some(frame) = nodes.read(&nodes.frame) {
            let ema = |c: &mut f32, ns: f32| {
                *c = if *c == 0.0 {
                    ns.max(1.0)
                } else {
                    *c + (ns - *c) * 0.1
                };
            };
            for job in &frame.map.jobs {
                match *job {
                    Job::Track { slot, .. } => {
                        let s = slot as usize;
                        let ns = self.prof.block_ns(Node::Track(s)) as f32;
                        ema(&mut self.track_cost[s], ns);
                    }
                    Job::Strip { slot } => {
                        let s = slot as usize;
                        let mut ns = self.prof.block_ns(Node::Strip(s));
                        for i in 0..MAX_EFFECTS_PER_TRACK {
                            ns += self.prof.block_ns(Node::Insert(s, i));
                        }
                        ema(&mut self.strip_cost[s], ns as f32);
                    }
                    Job::Master => {}
                }
            }
        }
        // Soon after a plan (its first costs), then every half second of audio.
        let frames = self.stats.block_frames.max(1) as f64;
        let every = if self.clustered_costs {
            (self.sample_rate * 0.5 / frames).max(8.0) as u64
        } else {
            8
        };
        if self.block.saturating_sub(self.clustered_at) >= every {
            self.map_dirty = true;
            self.clustered_costs = true;
        }
    }

    /// The chain of nodes that finished last, back from the master.
    pub(super) fn critical_path(&self) -> CriticalPath {
        let mut cp = CriticalPath {
            tracks: [false; MAX_TRACKS],
            strips: [false; MAX_STRIPS],
        };
        let nodes = &*self.nodes;
        let Some(frame) = nodes.read(&nodes.frame) else {
            return cp;
        };
        let mut cur = self.plan.master();
        for _ in 0..crate::graph::MAX_NODES {
            let mut best: Option<(usize, u64)> = None;
            for &p in self.topo.preds_of(cur) {
                let p = p as usize;
                let end = nodes.timing.get(p).map_or(0, |t| t.end.load(Relaxed));
                if best.is_none_or(|(_, e)| end > e) {
                    best = Some((p, end));
                }
            }
            let Some((p, _)) = best else {
                break;
            };
            match frame.map.jobs.get(p) {
                Some(Job::Track { slot, .. }) => cp.tracks[*slot as usize] = true,
                Some(Job::Strip { slot }) => cp.strips[*slot as usize] = true,
                _ => {}
            }
            cur = p;
        }
        cp
    }
}

//! Tasks from nodes (ADR 0031 §2.3, §2.5): clustering, critical-path ranks and the serial
//! hint — the shape the pool runs, built on the callback, allocation-free, whenever the
//! plan changes and every half second of audio as the measured costs move. Changing it
//! changes no sample: a task runs its nodes in plan order, and every gather is canonical.
//!
//! **Clustering.** Dispatch costs ~0.3–0.5 µs a task (round 21), so nodes the profiler
//! measures under [`LIGHT`] are fused: a light node into its only successor that is not
//! the master (it must run before it anyway — a light track into the bus it feeds, a light
//! strip into the bus or return it reaches), and the light nodes that feed only the master
//! into bins of at least [`TARGET`] (nothing waits for them but the master, so binning
//! them can only delay the master by the bin's own cost). Both keep the task graph acyclic;
//! a Kahn sort checks it anyway, and a cycle (a bug) falls back to one task per node.
//!
//! **Ranks.** HEFT's upward rank: a task's cost plus its most expensive path to the end.
//! The pool seeds and pushes by it, so the critical path goes first.
//!
//! **The serial hint.** When the critical path is over 80 % of all the work, parallel
//! cannot pay (§2.5): the pool runs the block alone — unless a node on it can split its own
//! work over the pool (an instrument's voices, ADR 0038; [`TaskTopo::divisible`]): then one
//! long node is exactly what the other threads are for.

use super::ctx::{Job, TaskMap};
use crate::graph::plan::{GraphPlan, MAX_NODES};

/// A node under this cost (ns per block) is light: fused rather than dispatched alone.
pub const LIGHT: f64 = 10_000.0;
/// A bin of light master-feeding nodes is closed at this cost (ns per block).
pub const TARGET: f64 = 20_000.0;
const NONE: u16 = u16::MAX;

/// The task graph the pool runs (`sched::Topology`), and the node-level reverse edges the
/// profiler's critical path walks.
pub(crate) struct TaskTopo {
    /// Per plan node: its predecessors (`preds[pred_start[n]..pred_start[n + 1]]`).
    pub pred_start: Vec<u32>,
    pub preds: Vec<u16>,
    /// Per plan node, its task (`NONE` for the master).
    task_of: Vec<u16>,
    /// The tasks: predecessor counts, successor lists, ranks.
    len: usize,
    tpreds: Vec<u32>,
    tsucc_start: Vec<u32>,
    tsuccs: Vec<u32>,
    ranks: Vec<u32>,
    serial_hint: bool,
    /// Some node splits its own work over the pool (ADR 0038): never hint serial. Set by
    /// the processor before [`TaskTopo::build`].
    pub divisible: bool,
    /// The critical path's share of the work, and the work (ns), as last built.
    pub critical_share: f64,
    pub work_ns: f64,
    // Scratch.
    parent: Vec<u16>,
    cost: Vec<f64>,
    tcost: Vec<f64>,
    rank: Vec<f64>,
    order: Vec<u32>,
    indeg: Vec<u32>,
    stamp: Vec<u32>,
    fill: Vec<u32>,
    edges: Vec<(u32, u32)>,
}

impl TaskTopo {
    /// Room for the largest plan. Allocates: built with the processor.
    pub fn new() -> Self {
        let n = MAX_NODES;
        let e = n * 40;
        TaskTopo {
            pred_start: Vec::with_capacity(n + 1),
            preds: Vec::with_capacity(e),
            task_of: Vec::with_capacity(n),
            len: 0,
            tpreds: Vec::with_capacity(n),
            tsucc_start: Vec::with_capacity(n + 1),
            tsuccs: Vec::with_capacity(e),
            ranks: Vec::with_capacity(n),
            serial_hint: false,
            divisible: false,
            critical_share: 0.0,
            work_ns: 0.0,
            parent: Vec::with_capacity(n),
            cost: Vec::with_capacity(n),
            tcost: Vec::with_capacity(n),
            rank: Vec::with_capacity(n),
            order: Vec::with_capacity(n),
            indeg: Vec::with_capacity(n),
            stamp: Vec::with_capacity(n),
            fill: Vec::with_capacity(n + 1),
            edges: Vec::with_capacity(e),
        }
    }

    fn find(&mut self, mut v: u16) -> u16 {
        while self.parent[v as usize] != v {
            let p = self.parent[v as usize];
            self.parent[v as usize] = self.parent[p as usize];
            v = p;
        }
        v
    }

    /// Build the tasks of `plan` (whose jobs `map` already resolved) from each node's
    /// measured cost (`cost(n)`, ns per block; 0 = not measured, treated as heavy), and
    /// write each task's nodes into `map`. `cluster: false` makes one task per node.
    pub fn build(
        &mut self,
        plan: &GraphPlan,
        map: &mut TaskMap,
        cost: impl Fn(usize) -> f64,
        cluster: bool,
    ) {
        let count = plan.nodes().len();
        let master = plan.master();
        // Reverse edges, by a counting sort.
        self.pred_start.clear();
        self.pred_start.resize(count + 1, 0);
        for u in 0..count {
            for &v in plan.successors(u) {
                self.pred_start[v as usize + 1] += 1;
            }
        }
        for n in 0..count {
            self.pred_start[n + 1] += self.pred_start[n];
        }
        self.fill.clear();
        self.fill.extend_from_slice(&self.pred_start[..count]);
        self.preds.clear();
        self.preds.resize(self.pred_start[count] as usize, 0);
        for u in 0..count {
            for &v in plan.successors(u) {
                let at = &mut self.fill[v as usize];
                self.preds[*at as usize] = u as u16;
                *at += 1;
            }
        }
        self.cost.clear();
        self.cost.extend((0..count).map(&cost));
        // Clusters (union-find, the root being the cluster's id).
        self.parent.clear();
        self.parent.extend(0..count as u16);
        if cluster {
            // A light node into its only successor that is not the master.
            for v in (0..count).rev() {
                if v == master || !(self.cost[v] > 0.0 && self.cost[v] < LIGHT) {
                    continue;
                }
                let mut only = NONE;
                let mut k = 0;
                for &s in plan.successors(v) {
                    if s as usize != master {
                        only = s as u16;
                        k += 1;
                    }
                }
                if k == 1 {
                    let (a, b) = (self.find(v as u16), self.find(only));
                    if a != b {
                        self.parent[a as usize] = b;
                    }
                }
            }
            // The light nodes nothing but the master waits for, into bins (in plan order).
            let mut bin = NONE;
            let mut bin_cost = 0.0;
            for v in 0..count {
                if v == master || !(self.cost[v] > 0.0 && self.cost[v] < LIGHT) {
                    continue;
                }
                if plan.successors(v).iter().any(|&s| s as usize != master) {
                    continue;
                }
                if self.find(v as u16) != v as u16 {
                    continue; // Already the end of something fused into it.
                }
                if bin == NONE || bin_cost >= TARGET {
                    bin = v as u16;
                    bin_cost = self.cost[v];
                } else {
                    let b = self.find(bin);
                    self.parent[v] = b;
                    bin_cost += self.cost[v];
                }
            }
        }
        if !self.number_tasks(plan, count, master) {
            // A cycle (a bug in the rules above): one task per node is always acyclic.
            self.parent.clear();
            self.parent.extend(0..count as u16);
            let ok = self.number_tasks(plan, count, master);
            debug_assert!(ok);
        }
        self.write_map(map, count);
    }

    /// Number the clusters as tasks, build their edges, sort them (Kahn) and rank them.
    /// `false` when the task graph has a cycle.
    fn number_tasks(&mut self, plan: &GraphPlan, count: usize, master: usize) -> bool {
        self.task_of.clear();
        self.task_of.resize(count, NONE);
        self.len = 0;
        for v in 0..count {
            if v == master {
                continue;
            }
            let r = self.find(v as u16) as usize;
            if self.task_of[r] == NONE {
                self.task_of[r] = self.len as u16;
                self.len += 1;
            }
            self.task_of[v] = self.task_of[r];
        }
        let n = self.len;
        self.tcost.clear();
        self.tcost.resize(n, 0.0);
        for v in 0..count {
            if v != master {
                self.tcost[self.task_of[v] as usize] += self.cost[v].max(1.0);
            }
        }
        // Task edges, each once.
        self.edges.clear();
        self.stamp.clear();
        self.stamp.resize(n, u32::MAX);
        for u in 0..count {
            if u == master {
                continue;
            }
            let tu = self.task_of[u] as u32;
            for &v in plan.successors(u) {
                if v as usize == master {
                    continue;
                }
                let tv = self.task_of[v as usize] as u32;
                if tu != tv {
                    self.edges.push((tu, tv));
                }
            }
        }
        self.edges.sort_unstable();
        self.edges.dedup();
        self.tsucc_start.clear();
        self.tsucc_start.resize(n + 1, 0);
        self.tpreds.clear();
        self.tpreds.resize(n, 0);
        for &(a, b) in &self.edges {
            self.tsucc_start[a as usize + 1] += 1;
            self.tpreds[b as usize] += 1;
        }
        for t in 0..n {
            self.tsucc_start[t + 1] += self.tsucc_start[t];
        }
        self.tsuccs.clear();
        self.tsuccs.extend(self.edges.iter().map(|e| e.1));
        // Kahn: a topological order, or a cycle.
        self.indeg.clear();
        self.indeg.extend_from_slice(&self.tpreds);
        self.order.clear();
        self.order
            .extend((0..n as u32).filter(|&t| self.tpreds[t as usize] == 0));
        let mut i = 0;
        while i < self.order.len() {
            let t = self.order[i] as usize;
            for k in self.tsucc_start[t]..self.tsucc_start[t + 1] {
                let s = self.tsuccs[k as usize] as usize;
                self.indeg[s] -= 1;
                if self.indeg[s] == 0 {
                    self.order.push(s as u32);
                }
            }
            i += 1;
        }
        if self.order.len() != n {
            return false;
        }
        // Upward ranks, in reverse topological order.
        self.rank.clear();
        self.rank.resize(n, 0.0);
        for &t in self.order.iter().rev() {
            let t = t as usize;
            let mut best = 0.0f64;
            for k in self.tsucc_start[t]..self.tsucc_start[t + 1] {
                best = best.max(self.rank[self.tsuccs[k as usize] as usize]);
            }
            self.rank[t] = self.tcost[t] + best;
        }
        self.ranks.clear();
        self.ranks.extend(0..n as u32);
        let rank = &self.rank;
        self.ranks.sort_unstable_by(|a, b| {
            rank[*b as usize]
                .total_cmp(&rank[*a as usize])
                .then(a.cmp(b))
        });
        self.work_ns = self.tcost.iter().sum();
        let critical = self.rank.iter().copied().fold(0.0, f64::max);
        self.critical_share = if self.work_ns > 0.0 {
            critical / self.work_ns
        } else {
            1.0
        };
        self.serial_hint = (n <= 1 || self.critical_share > 0.8) && !self.divisible;
        true
    }

    /// Each task's nodes, in plan order.
    fn write_map(&mut self, map: &mut TaskMap, count: usize) {
        let n = self.len;
        map.task_start.clear();
        map.task_start.resize(n + 1, 0);
        for v in 0..count {
            if map.jobs.get(v).is_some_and(|j| *j != Job::Master) && self.task_of[v] != NONE {
                map.task_start[self.task_of[v] as usize + 1] += 1;
            }
        }
        for t in 0..n {
            map.task_start[t + 1] += map.task_start[t];
        }
        self.fill.clear();
        self.fill.extend_from_slice(&map.task_start[..n]);
        map.task_nodes.clear();
        map.task_nodes.resize(map.task_start[n] as usize, 0);
        for v in 0..count {
            if map.jobs.get(v).is_some_and(|j| *j != Job::Master) && self.task_of[v] != NONE {
                let at = &mut self.fill[self.task_of[v] as usize];
                map.task_nodes[*at as usize] = v as u16;
                *at += 1;
            }
        }
    }

    /// Shuffle the ranks by `seed` (the schedule fuzz: priorities unrelated to the
    /// costs); the serial hint is dropped so the pool keeps running in parallel.
    pub fn shuffle_ranks(&mut self, seed: u64) {
        let mut r = crate::sched::fuzz::Rng::new(seed);
        for i in (1..self.ranks.len()).rev() {
            let j = r.below(i as u64 + 1) as usize;
            self.ranks.swap(i, j);
        }
        self.serial_hint = false;
    }

    /// Plan node `n`'s predecessors.
    pub fn preds_of(&self, n: usize) -> &[u16] {
        match (self.pred_start.get(n), self.pred_start.get(n + 1)) {
            (Some(&a), Some(&b)) => &self.preds[a as usize..b as usize],
            _ => &[],
        }
    }
}

impl crate::sched::Topology for TaskTopo {
    fn len(&self) -> usize {
        self.len
    }
    fn preds(&self, task: u32) -> u32 {
        self.tpreds[task as usize]
    }
    fn succs(&self, task: u32) -> &[u32] {
        let t = task as usize;
        &self.tsuccs[self.tsucc_start[t] as usize..self.tsucc_start[t + 1] as usize]
    }
    fn ranks(&self) -> &[u32] {
        &self.ranks
    }
    fn serial_hint(&self) -> bool {
        self.serial_hint
    }
}

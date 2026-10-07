//! What every node of a pass reads and nobody writes during it (ADR 0031 §1.1, §2.2): the
//! pass's context and the task map — one [`Frame`], written by the callback between passes
//! and read by every task through one shared borrow.

use super::*;
use crate::graph::plan::Input;
use crate::graph::NodeRef;

/// The pass being run: where the transport is, which slice of the block, and the switches
/// nodes read. Written by the callback before each pass.
pub(crate) struct PassCtx {
    pub snapshot: Option<Arc<Snapshot>>,
    /// The transport at the pass's first sample.
    pub transport: Transport,
    pub sample_rate: f64,
    /// Samples processed before the current block (the clock notes to effects are timed on).
    pub clock: u64,
    /// The pass is `[p0, p0 + n)` of the block.
    pub p0: usize,
    pub n: usize,
    /// Where the song is at the pass's first sample, for the strips' units.
    pub position: crate::fx::SongPosition,
    pub law: PanLaw,
    pub recording: bool,
    /// The track whose instrument reports telemetry, and the effect the analyser watches.
    pub probed: Option<TrackId>,
    pub analysed: Option<EffectId>,
    /// The strip slot the mix recorder takes, and its line.
    pub capture_slot: Option<usize>,
    pub capture_line: Option<usize>,
    /// Automation's last control point (ramps count from it), when a runtime is installed.
    pub auto_at: Option<u64>,
    /// A number no other pass had: the stamp of what a node wrote this pass.
    pub pass: u64,
    /// The block's number: the stamp of the peaks a node keeps across its passes.
    pub block: u64,
    /// The per-node profiler and node timing are on.
    pub profiling: bool,
    /// A stem tap is attached (the pass runs serially).
    pub tap: bool,
}

impl PassCtx {
    pub fn new(sample_rate: f64) -> Self {
        PassCtx {
            snapshot: None,
            transport: Transport::new(sample_rate),
            sample_rate,
            clock: 0,
            p0: 0,
            n: 0,
            position: crate::fx::SongPosition::default(),
            law: PanLaw::default(),
            recording: false,
            probed: None,
            analysed: None,
            capture_slot: None,
            capture_line: None,
            auto_at: None,
            pass: 0,
            block: 0,
            profiling: true,
            tap: false,
        }
    }

    /// Samples since the last control point, for a stretch starting at transport sample
    /// `now`; `None` without a runtime (nothing ramps).
    #[inline]
    pub fn k0(&self, now: u64) -> Option<u64> {
        self.auto_at.map(|at| now.saturating_sub(at))
    }
}

/// A node as a task runs it, resolved to its slot.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum Job {
    /// Snapshot track `ti`, in track slot `slot`.
    Track { ti: u16, slot: u16 },
    /// The strip in strip slot `slot`.
    Strip { slot: u16 },
    /// The master (run by the callback after the pool's part of the pass).
    Master,
}

/// One input of a gather, resolved to where its producer leaves it.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum In {
    /// A track slot's output.
    Track(u16),
    /// A strip slot's output edge.
    Out(u16),
    /// A delayed send: strip slot, its line — the line's output.
    Line(u16, u16),
    /// An undelayed send: strip slot, send `k` — the strip's post-chain pair times the
    /// fader and the send's level.
    Send(u16, u8),
}

/// The plan, resolved to slots and lines, and the tasks (clusters of nodes): what a task
/// needs to run its nodes. Rebuilt by the callback when the plan, a slot or a line changes.
pub(crate) struct TaskMap {
    /// Per plan node.
    pub jobs: Vec<Job>,
    /// Per plan node, its gather's inputs: `inputs[in_start[n]..in_start[n + 1]]`.
    pub in_start: Vec<u32>,
    pub inputs: Vec<In>,
    /// Per task, its nodes in plan order: `task_nodes[task_start[t]..task_start[t + 1]]`.
    pub task_start: Vec<u32>,
    pub task_nodes: Vec<u16>,
}

impl TaskMap {
    pub fn new() -> Self {
        use crate::graph::plan::MAX_NODES;
        TaskMap {
            jobs: Vec::with_capacity(MAX_NODES),
            in_start: Vec::with_capacity(MAX_NODES + 1),
            inputs: Vec::with_capacity(MAX_TRACKS + MAX_STRIPS * (1 + MAX_SENDS_PER_STRIP)),
            task_start: Vec::with_capacity(MAX_NODES + 1),
            task_nodes: Vec::with_capacity(MAX_NODES),
        }
    }

    /// Node `n`'s resolved inputs, in canonical order.
    #[inline]
    pub fn inputs(&self, n: usize) -> &[In] {
        &self.inputs[self.in_start[n] as usize..self.in_start[n + 1] as usize]
    }

    /// Task `t`'s nodes.
    #[inline]
    pub fn task(&self, t: usize) -> &[u16] {
        &self.task_nodes[self.task_start[t] as usize..self.task_start[t + 1] as usize]
    }

    /// Resolve `plan` to slots and lines. Allocation-free (every vector holds the largest
    /// plan). Tasks are left as they were: [`TaskMap::one_task_per_node`] or the
    /// clustering sets them.
    pub fn resolve(
        &mut self,
        plan: &GraphPlan,
        track_to_slot: &[usize],
        strip_to_slot: &[usize],
        send_line_of: impl Fn(usize, usize) -> Option<usize>,
    ) {
        self.jobs.clear();
        for r in plan.nodes() {
            self.jobs.push(match *r {
                NodeRef::Track(ti) => Job::Track {
                    ti,
                    slot: track_to_slot.get(ti as usize).copied().unwrap_or(0) as u16,
                },
                NodeRef::Strip(si) => Job::Strip {
                    slot: strip_to_slot.get(si as usize).copied().unwrap_or(0) as u16,
                },
                NodeRef::Master => Job::Master,
            });
        }
        self.in_start.clear();
        self.inputs.clear();
        self.in_start.push(0);
        for n in 0..plan.nodes().len() {
            for i in plan.inputs(n) {
                let r = match *i {
                    Input::Track(ti) => {
                        track_to_slot.get(ti as usize).map(|s| In::Track(*s as u16))
                    }
                    Input::Out(si) => strip_to_slot.get(si as usize).map(|s| In::Out(*s as u16)),
                    Input::Send(si, k) => {
                        strip_to_slot
                            .get(si as usize)
                            .map(|&s| match send_line_of(s, k as usize) {
                                Some(li) => In::Line(s as u16, li as u16),
                                None => In::Send(s as u16, k),
                            })
                    }
                };
                if let Some(r) = r {
                    self.inputs.push(r);
                }
            }
            self.in_start.push(self.inputs.len() as u32);
        }
    }

    /// Every node but the master its own task, in plan order: the serial engine's shape,
    /// and the pool's before the first costs are measured.
    pub fn one_task_per_node(&mut self) {
        self.task_start.clear();
        self.task_nodes.clear();
        self.task_start.push(0);
        for (n, j) in self.jobs.iter().enumerate() {
            if *j != Job::Master {
                self.task_nodes.push(n as u16);
                self.task_start.push(self.task_nodes.len() as u32);
            }
        }
    }

    /// Tasks in the map.
    pub fn tasks(&self) -> usize {
        self.task_start.len().saturating_sub(1)
    }
}

/// Everything the tasks of a pass read: the context and the map.
pub(crate) struct Frame {
    pub ctx: PassCtx,
    pub map: TaskMap,
}

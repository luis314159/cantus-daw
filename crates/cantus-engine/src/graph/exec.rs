//! The engine's graph as the pool runs it (ADR 0031 §2.2, §2.7, Phase 1): every node's
//! state, its outputs and every worker's scratch behind try-only cells
//! (`cantus_rt::cell`), so any thread of the pool may run any node, and the callback reaches
//! all of it between passes.
//!
//! **Who holds what.** A node's *state* (its units, its settings, its lines' assignment) is
//! a [`NodeCell`]: one holder at a time — the task running the node, or the callback's
//! control phase between passes. A node's *outputs* (a track's leveled pair, a strip's
//! output edge and post-chain pair, its key, a delayed send's line output, a track's note
//! outbox) are [`ShareCell`]s: the producer writes them while it runs, any number of
//! consumers read them after it finished. The plan's edges make every borrow succeed; a
//! borrow that fails is a scheduler bug, counted in [`Nodes::conflicts`]
//! (`EngineStats::schedule_conflicts`) — the node or the contribution is skipped as silence,
//! and nothing waits or panics (ADR 0005).
//!
//! **What a pass reads** is one [`Frame`] (`ctx.rs`), written by the callback before the
//! pass and borrowed shared once per task. **What a pass sends out** — a hosted plug-in's
//! parameter gestures, the profiler's time — goes into the running slot's [`Scratch`] and
//! the callback forwards it after the pass; meters go into atomics the callback reads.

use super::ctx::{Frame, Job, PassCtx, TaskMap};
use super::*;
use crate::graph::notes::{Listener, Outbox};
use crate::pdc::{DelayLine, Lines};
use cantus_rt::cell::{NodeCell, NodeMut, ShareCell};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering::Relaxed};

/// Parameter gestures a slot can carry out of one pass (a hosted plug-in's window being
/// played with: a few per pass at most).
pub(crate) const OUTGOING: usize = 512;

/// A hosted unit's gesture, on its way to the UI after the pass.
#[derive(Copy, Clone, Debug)]
pub(crate) enum Outgoing {
    Inst {
        track: TrackId,
        event: crate::fx::FxParamEvent,
    },
    Fx {
        effect: EffectId,
        event: crate::fx::FxParamEvent,
    },
}

/// Queue a gesture into a slot's outgoing events; `false` (dropped) when full.
pub(crate) trait PushEvent {
    fn push_event(&mut self, o: Outgoing) -> bool;
}

impl PushEvent for Vec<Outgoing> {
    #[inline]
    fn push_event(&mut self, o: Outgoing) -> bool {
        if self.len() < self.capacity() {
            self.push(o);
            true
        } else {
            false
        }
    }
}

/// One slot's working memory: every node it runs works in these buffers.
pub(crate) struct Scratch {
    pub mono: Vec<f32>,
    pub buf_l: Vec<f32>,
    pub buf_r: Vec<f32>,
    pub dry_l: Vec<f32>,
    pub dry_r: Vec<f32>,
    pub edge_l: Vec<f32>,
    pub edge_r: Vec<f32>,
    /// What the slot's nodes sent out this pass (forwarded by the callback).
    pub events: Vec<Outgoing>,
    /// Where the slot's time went this block (folded into the processor's profiler).
    pub prof: crate::profile::SlotProfile,
}

impl Scratch {
    pub fn new() -> Self {
        let buf = || vec![0.0; PASS_FRAMES];
        Scratch {
            mono: buf(),
            buf_l: buf(),
            buf_r: buf(),
            dry_l: buf(),
            dry_r: buf(),
            edge_l: buf(),
            edge_r: buf(),
            events: Vec::with_capacity(OUTGOING),
            prof: crate::profile::SlotProfile::new(),
        }
    }

    fn prefault(&mut self) {
        for v in [
            &mut self.mono,
            &mut self.buf_l,
            &mut self.buf_r,
            &mut self.dry_l,
            &mut self.dry_r,
            &mut self.edge_l,
            &mut self.edge_r,
        ] {
            touch(v);
        }
    }
}

/// Written, made observable, written back: neither store can be elided (R11).
pub(crate) fn touch(v: &mut [f32]) {
    v.fill(f32::MIN_POSITIVE);
    std::hint::black_box(&mut *v);
    v.fill(0.0);
}

/// A track slot's cells.
pub(crate) struct TrackCells {
    pub state: NodeCell<TrackSlot>,
    pub out: ShareCell<TrackOut>,
    pub outbox: ShareCell<Outbox>,
}

/// A strip slot's cells.
pub(crate) struct StripCells {
    pub state: NodeCell<StripSlot>,
    pub out: ShareCell<StripOut>,
    pub key: ShareCell<KeyBuf>,
}

/// What leaves a strip's fader, kept for the inserts keyed from it; valid in the pass
/// whose stamp it carries.
pub(crate) struct KeyBuf {
    pub l: Vec<f32>,
    pub r: Vec<f32>,
    pub pass: u64,
}

/// The analysed effect's input and its readings, and the analyser's ring.
pub(crate) struct Analysis {
    pub ring: Option<rtrb::Producer<f32>>,
    pub pre: Vec<f32>,
    pub eq_deltas: Option<[f32; METERED_EQ_BANDS]>,
    pub comp: Option<[f32; 3]>,
    pub fx: Option<crate::fx::Telemetry>,
}

/// The mix recorder's ring (a strip's take, or the master's), and what it lost.
pub(crate) struct Capture {
    pub ring: Option<rtrb::Producer<f32>>,
    pub overruns: u64,
    /// The captured strip ran this pass.
    pub ran: bool,
}

impl Capture {
    /// One stereo frame into the recorder's ring; a full ring drops it and counts it.
    #[inline]
    pub fn frame(&mut self, l: f32, r: f32) {
        if let Some(c) = self.ring.as_mut() {
            if c.slots() >= 2 {
                let _ = c.push(l);
                let _ = c.push(r);
            } else {
                self.overruns += 1;
            }
        }
    }
}

/// A plan node's last run: when it started and ended (ns since [`Nodes::origin`]), and on
/// which slot — the profiler's critical path and the clustering's costs.
#[derive(Default)]
pub(crate) struct NodeTime {
    pub start: AtomicU64,
    pub end: AtomicU64,
}

/// Every node of the engine, as the pool shares it. See the module's docs.
pub(crate) struct Nodes {
    pub tracks: Box<[TrackCells]>,
    pub strips: Box<[StripCells]>,
    /// Delay compensation's lines (shared with the planner, `crate::pdc`), and each line's
    /// output for a delayed send's consumer.
    pub lines: Arc<Lines>,
    pub edges: Box<[ShareCell<[Vec<f32>; 2]>]>,
    /// The inserts hearing a track's notes (ADR 0022), by place.
    pub listeners: Box<[NodeCell<Listener>]>,
    pub frame: ShareCell<Frame>,
    /// The block's input, deinterleaved, for the tracks that monitor it.
    pub input: ShareCell<crate::processor::rec::InputBlock>,
    pub scratch: Box<[cantus_rt::CachePadded<NodeCell<Scratch>>]>,
    pub analysis: NodeCell<Analysis>,
    pub probe: NodeCell<Option<crate::inst::Telemetry>>,
    pub capture: NodeCell<Capture>,
    pub tap: NodeCell<Option<Box<crate::stems::StemTap>>>,
    /// Per slot, published by the node at the end of each pass: its peak this block (f32
    /// bits), and a track's voices or a strip's deepest reduction.
    pub track_peak: Box<[AtomicU32]>,
    pub track_voices: Box<[AtomicU32]>,
    pub strip_peak: Box<[AtomicU32]>,
    pub strip_gr: Box<[AtomicU32]>,
    /// Per plan node.
    pub timing: Box<[NodeTime]>,
    /// Borrows that failed (a scheduler bug if ever non-zero) and notes or events dropped.
    pub conflicts: AtomicU64,
    pub dropped: AtomicU64,
    pub origin: Instant,
}

impl Nodes {
    /// Every cell, for `slots` pool slots (the callback's included), allocated and touched
    /// here, off the audio thread.
    pub fn new(sample_rate: f64, slots: usize, lines: Arc<Lines>) -> Self {
        let sr = sample_rate as f32;
        let atoms = |n: usize| (0..n).map(|_| AtomicU32::new(0)).collect::<Box<[_]>>();
        let pair = || [vec![0.0; PASS_FRAMES], vec![0.0; PASS_FRAMES]];
        let mut n = Nodes {
            tracks: (0..MAX_TRACKS)
                .map(|_| TrackCells {
                    state: NodeCell::new(TrackSlot::new(sr)),
                    out: ShareCell::new(TrackOut::new()),
                    outbox: ShareCell::new(Outbox::new()),
                })
                .collect(),
            strips: (0..MAX_STRIPS)
                .map(|_| StripCells {
                    state: NodeCell::new(StripSlot::new()),
                    out: ShareCell::new(StripOut::new()),
                    key: ShareCell::new(KeyBuf {
                        l: vec![0.0; PASS_FRAMES],
                        r: vec![0.0; PASS_FRAMES],
                        pass: 0,
                    }),
                })
                .collect(),
            edges: (0..crate::pdc::POOL)
                .map(|_| ShareCell::new(pair()))
                .collect(),
            lines,
            listeners: (0..MAX_LISTENERS)
                .map(|_| {
                    NodeCell::new(Listener {
                        track: 0,
                        effect: EffectId(0),
                        delay: 0,
                        cursor: 0,
                    })
                })
                .collect(),
            frame: ShareCell::new(Frame {
                ctx: PassCtx::new(sample_rate),
                map: TaskMap::new(),
            }),
            input: ShareCell::new(crate::processor::rec::InputBlock::new()),
            scratch: (0..slots.max(1))
                .map(|_| cantus_rt::CachePadded::new(NodeCell::new(Scratch::new())))
                .collect(),
            analysis: NodeCell::new(Analysis {
                ring: None,
                pre: vec![0.0; PASS_FRAMES],
                eq_deltas: None,
                comp: None,
                fx: None,
            }),
            probe: NodeCell::new(None),
            capture: NodeCell::new(Capture {
                ring: None,
                overruns: 0,
                ran: false,
            }),
            tap: NodeCell::new(None),
            track_peak: atoms(MAX_TRACKS),
            track_voices: atoms(MAX_TRACKS),
            strip_peak: atoms(MAX_STRIPS),
            strip_gr: atoms(MAX_STRIPS),
            timing: (0..crate::graph::MAX_NODES)
                .map(|_| NodeTime::default())
                .collect(),
            conflicts: AtomicU64::new(0),
            dropped: AtomicU64::new(0),
            origin: Instant::now(),
        };
        n.prefault();
        n
    }

    /// Touch every buffer once on the thread that builds the graph, so the audio thread
    /// never takes their first page faults (ADR 0031 R11).
    fn prefault(&mut self) {
        for t in self.tracks.iter_mut() {
            let o = t.out.get_mut();
            touch(&mut o.l);
            touch(&mut o.r);
        }
        for s in self.strips.iter_mut() {
            let o = s.out.get_mut();
            for v in [&mut o.post_l, &mut o.post_r, &mut o.out_l, &mut o.out_r] {
                touch(v);
            }
            let k = s.key.get_mut();
            touch(&mut k.l);
            touch(&mut k.r);
        }
        for e in self.edges.iter_mut() {
            let [l, r] = e.get_mut();
            touch(l);
            touch(r);
        }
        for s in self.scratch.iter_mut() {
            s.get_mut().prefault();
        }
        touch(&mut self.analysis.get_mut().pre);
        self.input.get_mut().prefault();
    }

    /// Count a borrow that failed: a scheduler bug, never a wait.
    #[cold]
    pub fn conflict(&self) {
        self.conflicts.fetch_add(1, Relaxed);
    }

    /// A note or an event that found its queue full.
    #[inline]
    pub fn drop_event(&self) {
        self.dropped.fetch_add(1, Relaxed);
    }

    /// A producer's output, shared (counted when it fails).
    #[inline]
    pub fn read<'a, T>(&self, c: &'a ShareCell<T>) -> Option<cantus_rt::cell::ShareRef<'a, T>> {
        let r = c.try_borrow();
        if r.is_none() {
            self.conflict();
        }
        r
    }

    /// Track slot `slot`'s state, exclusively (counted when it fails).
    #[inline]
    pub fn track(&self, slot: usize) -> Option<NodeMut<'_, TrackSlot>> {
        let r = self.tracks.get(slot)?.state.try_borrow_mut();
        if r.is_none() {
            self.conflict();
        }
        r
    }

    /// Strip slot `slot`'s state, exclusively (counted when it fails).
    #[inline]
    pub fn strip(&self, slot: usize) -> Option<NodeMut<'_, StripSlot>> {
        let r = self.strips.get(slot)?.state.try_borrow_mut();
        if r.is_none() {
            self.conflict();
        }
        r
    }

    /// Delay line `i`, exclusively (counted when it fails).
    #[inline]
    pub fn line(&self, i: usize) -> Option<NodeMut<'_, DelayLine>> {
        let r = self.lines.cell(i)?.try_borrow_mut();
        if r.is_none() {
            self.conflict();
        }
        r
    }

    /// Whether line `i` still holds signal (a line nobody holds; `false` if someone does).
    #[inline]
    pub fn line_ringing(&self, i: Option<usize>) -> bool {
        i.and_then(|i| self.line(i)).is_some_and(|l| l.ringing())
    }

    /// Listener `i`, exclusively.
    #[inline]
    pub fn listener(&self, i: usize) -> Option<NodeMut<'_, Listener>> {
        let r = self.listeners.get(i)?.try_borrow_mut();
        if r.is_none() {
            self.conflict();
        }
        r
    }

    /// The nanoseconds since the graph was built: the node timing's clock.
    #[inline]
    pub fn now_ns(&self, at: Instant) -> u64 {
        at.saturating_duration_since(self.origin).as_nanos() as u64
    }

    /// Run plan node `n` on slot `slot` (the scratch and the frame borrowed by the caller).
    #[inline]
    pub fn run_node(&self, frame: &Frame, n: usize, sc: &mut Scratch) {
        let ctx = &frame.ctx;
        let started = ctx.profiling.then(Instant::now);
        match frame.map.jobs.get(n) {
            Some(&Job::Track { ti, slot }) => {
                crate::processor::track::run_track(self, ctx, ti as usize, slot as usize, sc)
            }
            Some(&Job::Strip { slot }) => {
                crate::processor::strip::run_strip(self, frame, n, slot as usize, sc)
            }
            Some(Job::Master) | None => {}
        }
        if let Some(t0) = started {
            let t1 = Instant::now();
            sc.prof.stop(t1);
            if let Some(t) = self.timing.get(n) {
                t.start.store(self.now_ns(t0), Relaxed);
                t.end.store(self.now_ns(t1), Relaxed);
            }
        }
    }

    /// Run task `task` (a cluster of nodes, in plan order) on slot `slot`.
    pub fn run_task(&self, task: usize, slot: usize) {
        let Some(mut sc) = self.scratch.get(slot).and_then(|c| c.try_borrow_mut()) else {
            self.conflict();
            return;
        };
        let Some(frame) = self.frame.try_borrow() else {
            self.conflict();
            return;
        };
        if task >= frame.map.tasks() {
            return;
        }
        sc.prof.set_on(frame.ctx.profiling);
        for &n in frame.map.task(task) {
            self.run_node(&frame, n as usize, &mut sc);
        }
    }
}

impl crate::sched::Graph for Nodes {
    #[inline]
    fn run(&self, task: u32, slot: usize) {
        self.run_task(task as usize, slot);
    }
}

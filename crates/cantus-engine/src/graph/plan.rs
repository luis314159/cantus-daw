//! The graph's plan (ADR 0031 §1.2): which nodes there are, in what order the serial
//! engine runs them, what each one gathers and from whom, and who waits for whom.
//!
//! **Compiled on the UI thread**, when the routing changes: [`compile`] allocates (once,
//! at the largest size the engine holds, so the audio thread can later compile into the
//! same plan without allocating) and is sent beside its snapshot
//! (`EngineMessage::SetSnapshotPlanned`). Its [`GraphPlan::key`] is [`topology_key`], a
//! hash of exactly what the plan is computed from — the tracks' and strips' ids, where each
//! track and strip goes, every send's target, every insert's key and note source, the
//! processing order — and of nothing else, so a knob turn, a fader move or a new note
//! compiles no plan. A snapshot that arrives without a plan, or with a stale one (a test
//! that sends snapshots by hand, a message the queue could not take), is compiled into the
//! plan the processor already holds, in place: the same code, allocation-free.
//!
//! **Snapshot indices, not slots.** The plan speaks of `Snapshot::tracks[i]` and
//! `Snapshot::strips[i]`; the processor maps them to its slots when it installs the
//! snapshot, which is the one thing only the audio thread can do (slots keep voices and
//! tails ringing across snapshots).
//!
//! **What the old engine did, exactly.** The plan reproduces the serial engine's routing
//! rules, including the ones a sane project never meets: tracks and strips past the
//! engine's limits are not played (and are counted, [`GraphPlan::refused_tracks`]); a send
//! into its own strip, or past the first [`MAX_SENDS_PER_STRIP`] accepted, is dropped; a
//! strip that is not in `strip_order` is never processed, so what feeds it is lost; and a
//! contribution into a strip that comes *later* than it in the order (a cycle a hand-edited
//! file could hold) is lost too, because the serial engine added it after the strip had
//! already been mixed.

use crate::messages::{MAX_EFFECTS_PER_TRACK, MAX_SENDS_PER_STRIP, MAX_STRIPS, MAX_TRACKS};
use cantus_core::Snapshot;

#[path = "plan_compile.rs"]
mod compile_impl;

/// Nodes a plan can hold: every track, every strip, the master.
pub const MAX_NODES: usize = MAX_TRACKS + MAX_STRIPS + 1;
/// Audio inputs a plan can hold: every track into one place, every strip's output and
/// sends.
const MAX_INPUTS: usize = MAX_TRACKS + MAX_STRIPS * (1 + MAX_SENDS_PER_STRIP);
/// Dependencies: the audio inputs, plus a key and a note source per insert.
const MAX_EDGES: usize = MAX_INPUTS + (MAX_STRIPS + 1) * MAX_EFFECTS_PER_TRACK * 2;
/// Not in the order.
const NONE: u16 = u16::MAX;

/// A node, by snapshot index.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum NodeRef {
    /// `Snapshot::tracks[i]`: its instrument, clips and input, its level and balance.
    Track(u16),
    /// `Snapshot::strips[i]`: its gather, inserts, EQ, width, fader, outputs and sends.
    Strip(u16),
    /// The master: its gather, its chain, its strip, the click.
    Master,
}

/// One contribution a node gathers, by snapshot index of its producer.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Input {
    /// A track's output (after its level, balance and delay line).
    Track(u16),
    /// A strip's output edge.
    Out(u16),
    /// A strip's `k`-th send — `k` counts the sends the engine accepted, which is the
    /// index its delay line and its level are kept under.
    Send(u16, u8),
}

/// The compiled graph. See the module's docs.
#[derive(Debug, Clone)]
pub struct GraphPlan {
    key: u64,
    tracks: usize,
    strips: usize,
    refused_tracks: usize,
    refused_strips: usize,
    /// Every node in the serial engine's order: the tracks in snapshot order, the strips in
    /// processing order, the master.
    nodes: Vec<NodeRef>,
    /// The strips in processing order (snapshot indices), each once.
    order: Vec<u16>,
    /// Each node's canonical inputs (ADR 0031 §1.3): `inputs[input_start[n]..input_start[n + 1]]`.
    input_start: Vec<u32>,
    inputs: Vec<Input>,
    /// Each node's successors — audio, key and note edges, each once.
    succ_start: Vec<u32>,
    succs: Vec<u32>,
    /// Each node's predecessor count.
    preds: Vec<u32>,
    scratch: Scratch,
}

/// Working memory for [`GraphPlan::compile_into`], kept with the plan so compiling into it
/// never allocates.
#[derive(Debug, Clone)]
struct Scratch {
    /// Each strip's place in the order, or [`NONE`].
    pos: Vec<u16>,
    /// Each audio source's destination node (or [`NONE`]), in canonical source order.
    dest: Vec<u16>,
    src: Vec<Input>,
    /// Per destination node, where its next input goes.
    fill: Vec<u32>,
    /// Dependency edges `(from, to)`.
    edges: Vec<(u16, u16)>,
    /// Per node, the last destination that counted it as a predecessor (dedup).
    stamp: Vec<u16>,
    strip_ids: IdMap,
    track_ids: IdMap,
}

/// A fixed open-addressing map from an id to an index (first one wins), cleared by
/// generation so reusing it touches nothing.
#[derive(Debug, Clone)]
struct IdMap {
    keys: Vec<u64>,
    vals: Vec<u16>,
    gens: Vec<u32>,
    gen: u32,
}

const ID_SLOTS: usize = 1024;

impl IdMap {
    fn new() -> Self {
        IdMap {
            keys: vec![0; ID_SLOTS],
            vals: vec![0; ID_SLOTS],
            gens: vec![0; ID_SLOTS],
            gen: 0,
        }
    }

    fn clear(&mut self) {
        self.gen = self.gen.wrapping_add(1);
        if self.gen == 0 {
            self.gens.iter_mut().for_each(|g| *g = 0);
            self.gen = 1;
        }
    }

    #[inline]
    fn home(id: u64) -> usize {
        (id.wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 54) as usize
    }

    fn insert(&mut self, id: u64, val: u16) {
        let mut i = Self::home(id);
        loop {
            if self.gens[i] != self.gen {
                self.gens[i] = self.gen;
                self.keys[i] = id;
                self.vals[i] = val;
                return;
            }
            if self.keys[i] == id {
                return; // The first one wins, as the engine's slot search does.
            }
            i = (i + 1) & (ID_SLOTS - 1);
        }
    }

    fn get(&self, id: u64) -> Option<u16> {
        let mut i = Self::home(id);
        loop {
            if self.gens[i] != self.gen {
                return None;
            }
            if self.keys[i] == id {
                return Some(self.vals[i]);
            }
            i = (i + 1) & (ID_SLOTS - 1);
        }
    }
}

/// FNV-1a over 64-bit words: small, allocation-free and the same on every machine.
struct Fnv(u64);

impl Fnv {
    fn new() -> Self {
        Fnv(0xcbf2_9ce4_8422_2325)
    }

    #[inline]
    fn word(&mut self, x: u64) {
        for b in x.to_le_bytes() {
            self.0 ^= b as u64;
            self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }

    #[inline]
    fn opt(&mut self, x: Option<u64>) {
        match x {
            Some(v) => {
                self.word(1);
                self.word(v);
            }
            None => self.word(0),
        }
    }
}

/// The hash of everything a plan is computed from, and nothing else (see the module's
/// docs). Cheap and allocation-free: the UI thread calls it on every snapshot to decide
/// whether to compile, the audio thread to check that the plan it holds is this
/// snapshot's.
pub fn topology_key(snap: &Snapshot) -> u64 {
    let mut h = Fnv::new();
    h.word(snap.tracks.len() as u64);
    h.word(snap.strips.len() as u64);
    for t in &snap.tracks {
        h.word(t.id.0);
        h.opt(t.strip.map(|s| s as u64));
    }
    for st in &snap.strips {
        h.word(st.id.0);
        h.opt(st.output.map(|s| s as u64));
        h.word(st.sends.len() as u64);
        for (to, _) in &st.sends {
            h.word(*to as u64);
        }
        h.word(st.effects.len() as u64);
        for e in &st.effects {
            h.opt(e.sidechain.map(|s| s.0));
            h.opt(e.midi_from.map(|t| t.0));
        }
    }
    h.word(snap.master_effects.len() as u64);
    for e in &snap.master_effects {
        h.opt(e.sidechain.map(|s| s.0));
        h.opt(e.midi_from.map(|t| t.0));
    }
    h.word(snap.strip_order.len() as u64);
    for s in &snap.strip_order {
        h.word(*s as u64);
    }
    h.0
}

/// Compile a snapshot's plan. Allocates: for the UI thread (and the offline renderers).
pub fn compile(snap: &Snapshot) -> Box<GraphPlan> {
    let mut plan = Box::new(GraphPlan::new());
    plan.compile_into(snap);
    plan
}

/// The key of an empty snapshot's plan: what a new processor holds before any snapshot.
fn topology_key_of_nothing() -> u64 {
    let mut h = Fnv::new();
    for _ in 0..4 {
        h.word(0);
    }
    h.0
}

impl Default for GraphPlan {
    fn default() -> Self {
        Self::new()
    }
}

impl GraphPlan {
    /// An empty plan with room for the largest graph the engine holds. Allocates.
    pub fn new() -> Self {
        let with = |n: usize, first: &[u32]| {
            let mut v = Vec::with_capacity(n);
            v.extend_from_slice(first);
            v
        };
        GraphPlan {
            key: topology_key_of_nothing(),
            tracks: 0,
            strips: 0,
            refused_tracks: 0,
            refused_strips: 0,
            nodes: {
                let mut v = Vec::with_capacity(MAX_NODES);
                v.push(NodeRef::Master);
                v
            },
            order: Vec::with_capacity(MAX_STRIPS),
            input_start: with(MAX_NODES + 1, &[0, 0]),
            inputs: Vec::with_capacity(MAX_INPUTS),
            succ_start: with(MAX_NODES + 1, &[0, 0]),
            succs: Vec::with_capacity(MAX_EDGES),
            preds: with(MAX_NODES, &[0]),
            scratch: Scratch {
                pos: Vec::with_capacity(MAX_STRIPS),
                dest: Vec::with_capacity(MAX_INPUTS),
                src: Vec::with_capacity(MAX_INPUTS),
                fill: Vec::with_capacity(MAX_NODES + 1),
                edges: Vec::with_capacity(MAX_EDGES),
                stamp: Vec::with_capacity(MAX_NODES),
                strip_ids: IdMap::new(),
                track_ids: IdMap::new(),
            },
        }
    }

    /// [`topology_key`] of the snapshot this plan was compiled from.
    pub fn key(&self) -> u64 {
        self.key
    }

    /// Tracks the engine plays (the snapshot's, up to `MAX_TRACKS`).
    pub fn track_count(&self) -> usize {
        self.tracks
    }

    /// Strips the engine holds (the snapshot's, up to `MAX_STRIPS`).
    pub fn strip_count(&self) -> usize {
        self.strips
    }

    /// The snapshot's tracks past `MAX_TRACKS`: refused, and said so
    /// (`EngineStats::refused_tracks`), never dropped silently.
    pub fn refused_tracks(&self) -> usize {
        self.refused_tracks
    }

    /// The snapshot's strips past `MAX_STRIPS`.
    pub fn refused_strips(&self) -> usize {
        self.refused_strips
    }

    /// Every node, in the order the serial engine runs them.
    pub fn nodes(&self) -> &[NodeRef] {
        &self.nodes
    }

    /// The strips in processing order (snapshot indices).
    pub fn order(&self) -> &[u16] {
        &self.order
    }

    /// The node index of the master (always the last).
    pub fn master(&self) -> usize {
        self.nodes.len() - 1
    }

    /// The node index of the strip at place `place` of [`Self::order`].
    pub fn strip_node(&self, place: usize) -> usize {
        self.tracks + place
    }

    /// Node `n`'s inputs in canonical order (ADR 0031 §1.3): the tracks that feed it in
    /// snapshot order; then, for each strip before it in the order, that strip's output if
    /// it leads here and its sends `k = 0, 1, …` that lead here.
    pub fn inputs(&self, n: usize) -> &[Input] {
        &self.inputs[self.input_start[n] as usize..self.input_start[n + 1] as usize]
    }

    /// The nodes that wait for node `n` (audio, key and note edges).
    pub fn successors(&self, n: usize) -> &[u32] {
        &self.succs[self.succ_start[n] as usize..self.succ_start[n + 1] as usize]
    }

    /// How many nodes node `n` waits for.
    pub fn predecessors(&self, n: usize) -> u32 {
        self.preds[n]
    }
}

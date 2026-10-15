//! The audio-thread processor. Owns every voice, buffer and queue it needs; after
//! construction it never allocates, locks, logs or panics (ADR 0005).
//!
//! Signal flow: **track** (instrument → channel gain/balance) → its **strip** (polarity →
//! swap → inserts, each through its slot's dry/wet → the strip's EQ → width → gain/pan
//! through the pan law → mute) → the strip it feeds, or the master (width → gain/balance
//! → polarity) — **and**, post-fader, a copy at each send's level into the strip the
//! send names.
//!
//! **A graph, run by a pool** (ADR 0031 §1–§2, `crate::graph`, `crate::sched`). This file
//! and its children are the control phase and the glue: messages, MIDI, the input ring,
//! recording marks, the transport, snapshots and delay compensation; then each block is
//! cut into **passes** (at loop wraps, automation control points and every `PASS_FRAMES`),
//! and in each pass every node runs once — the tracks (`graph/track.rs`) and the strips
//! (`graph/strip.rs`) on whichever thread of the pool takes them, each gathering its inputs
//! in the canonical order from what its predecessors left (`graph/gather.rs`); then the
//! master (`graph/master.rs`) on the callback. Who runs which node, and when, cannot
//! change a sample: the pool's output is the serial engine's, to the bit.
//!
//! **State lives in cells** (`graph/exec.rs`): each node's state, outputs and each
//! worker's scratch behind try-only cells shared with the pool. The control phase reaches
//! them between passes; a borrow that fails there or in a node is a scheduler bug, counted
//! in `EngineStats::schedule_conflicts` and skipped, never waited on.
//!
//! Firmus renders mono and is copied into both channels; Pulsus renders **stereo**,
//! because a pad carries its own pan and a kit is a dozen sources on one channel. From
//! the track's own gain onwards the two are the same signal path.
//!
//! Polarity sits at the *input* of a channel, where a console's Ø switch is, so that what
//! comes out of the strip is the whole channel inverted — inserts and all. On the master
//! it sits at the very end, which is the same place for the same reason: it is the last
//! thing before the output.

use crate::effects::EffectUnit;
use crate::graph::notes::Listener;
use crate::graph::{GraphPlan, PASS_FRAMES};
use crate::inst::{Controller, NativeUnit, NoteOn, UnitKind};
use crate::messages::*;
use crate::transport::Transport;
use cantus_core::{
    apply_width, balance_gains, EffectId, InstrumentSpec, NoteEvent, PanLaw, SendTap, Snapshot,
    StripId, TrackId,
};
use cantus_dsp::{ChannelEq, Click, Firmus, FirmusSettings, Pulsus, Wave};
use rtrb::{Consumer, Producer, RingBuffer};
use std::sync::Arc;
use std::time::Instant;

/// Automation (ADR 0027): control points, ramps, configured copies.
#[path = "processor_auto.rs"]
mod auto;
/// The graph's tasks: clustering, ranks, the serial hint.
#[path = "graph/cluster.rs"]
pub(crate) mod cluster;
/// What a pass reads: its context and the task map.
#[path = "graph/ctx.rs"]
pub(crate) mod ctx;
/// Every node's cells, as the pool shares them; the engine's `sched::Graph`.
#[path = "graph/exec.rs"]
pub(crate) mod exec;
/// The master node: its chain's state and installation (ADR 0026).
#[path = "graph/master.rs"]
mod master;
/// The master node's pass: gather, chain, strip, take, click.
#[path = "graph/master_run.rs"]
mod master_run;
/// Messages, MIDI and live notes.
#[path = "processor_msg.rs"]
mod msg;
/// The block: passes, the pool, and what is folded back after each.
#[path = "processor_pass.rs"]
mod pass;
/// The pool: how many workers, their start, the schedule fuzz's hook.
#[path = "processor_pool.rs"]
mod pool;
/// Input, monitoring, recording and audio clips (ADR 0025).
#[path = "processor_rec.rs"]
pub(crate) mod rec;
/// Calls into `cantus-rt` (FTZ/DAZ, the callback's priority), behind one small seam.
#[path = "graph/rt_shim.rs"]
mod rt_shim;
/// Snapshots, delay compensation, chains, and handing things back.
#[path = "processor_snap.rs"]
mod snap;
/// The strip nodes: state, gather and outputs.
#[path = "graph/strip.rs"]
pub(crate) mod strip;
/// A strip node's chain and outputs.
#[path = "graph/strip_run.rs"]
mod strip_run;
/// The track nodes.
#[path = "graph/track.rs"]
pub(crate) mod track;

use exec::Nodes;
use master::MasterStrip;
pub use pool::{default_workers, on_worker_start, Workers, MAX_WORKERS};
pub use rt_shim::AudioHost;
use strip::{StripOut, StripSlot};
use track::{TrackOut, TrackSlot};

/// Tag used for live (keyboard/MIDI) notes so they never collide with sequenced note ids.
const LIVE_TAG: u64 = u64::MAX;
/// How many snapshots we can hold back when the garbage queue is full.
const PENDING_GARBAGE: usize = 8;
/// MIDI events drained per block at most.
const MAX_MIDI_PER_BLOCK: usize = 512;
/// Inserts that may hear a track's notes at once (ADR 0022): every insert of every strip
/// and of the master.
pub(crate) const MAX_LISTENERS: usize = (MAX_STRIPS + 1) * MAX_EFFECTS_PER_TRACK;

/// Which of the slot's instruments is the live one.
///
/// A slot holds **both** engines rather than a boxed trait object, because switching a
/// track's instrument must not allocate on the audio thread — and neither must the
/// snapshot that carries the switch. It costs a few kilobytes per slot and buys a rule
/// that cannot be broken by accident (ADR 0005).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum SlotInstrument {
    Firmus,
    Pulsus,
    /// A native instrument's unit (ADR 0023), in `TrackSlot::native`.
    Native,
}

/// The pool the processor drives, when it has workers.
type EnginePool = crate::sched::Pool<Nodes, crate::sched::ChaseLev, crate::sched::DynChaos>;

#[allow(clippy::vec_box)]
pub struct Processor {
    sample_rate: f64,
    rx: Consumer<EngineMessage>,
    tx: Producer<EngineEvent>,
    midi_rx: Consumer<MidiEvent>,
    snapshot: Option<Arc<Snapshot>>,
    pending_garbage: Vec<Arc<Snapshot>>,
    /// Effect units received but not yet claimed by a snapshot.
    incoming_effects: Vec<Box<EffectUnit>>,
    /// Scratch space while re-ordering a track's chain (never grows past one chain).
    scratch_effects: Vec<Box<EffectUnit>>,
    /// Native instrument units received but not yet claimed by a snapshot.
    incoming_instruments: Vec<Box<NativeUnit>>,
    /// The track whose instrument reports telemetry (its editor is open).
    probed: Option<TrackId>,
    transport: Transport,
    /// Every node's cells (`graph/exec.rs`), shared with the pool's workers.
    nodes: Arc<Nodes>,
    /// The pool (ADR 0031 §2), when this processor has workers; `None` is `W = 0`, the
    /// serial engine on the callback alone.
    pool: Option<EnginePool>,
    /// Each slot's track and strip id, mirrored here so the control phase finds a slot
    /// without borrowing every node.
    track_ids: [Option<TrackId>; MAX_TRACKS],
    strip_ids: [Option<StripId>; MAX_STRIPS],
    /// Snapshot track index → slot index.
    track_to_slot: Vec<usize>,
    /// Snapshot strip index → strip slot index.
    strip_to_slot: Vec<usize>,
    /// Strip slots in processing order, from the plan.
    strip_play_order: Vec<usize>,
    /// The graph (ADR 0031 §1.2): nodes, order, canonical inputs. Compiled on the UI
    /// thread and sent with its snapshot, or compiled here in place when one arrives
    /// without (`graph::plan`).
    plan: Box<GraphPlan>,
    /// The tasks the pool runs (clusters, ranks), and whether the task map must be
    /// resolved again before the next pass (a plan, a slot or a line changed).
    topo: cluster::TaskTopo,
    map_dirty: bool,
    /// When the tasks were last clustered (blocks), and each slot's measured cost (ns per
    /// block, smoothed): what clustering and ranks are made of.
    clustered_at: u64,
    clustered_costs: bool,
    track_cost: [f32; MAX_TRACKS],
    strip_cost: [f32; MAX_STRIPS],
    /// Scratch: each plan node's cost, for the clustering.
    node_cost: Vec<f64>,
    /// The work the pool reported over this block's passes (the adaptive rule's load),
    /// the pool's wakes already counted, and the fuzz's schedule seed.
    block_work: u64,
    pool_wakes: u64,
    shuffle: Option<u64>,
    /// Instruments may split their voices over the pool (ADR 0038); `CANTUS_VOICE_SPLIT=0`
    /// or [`Processor::set_voice_split`] turns it off (the same bits either way).
    split_voices: bool,
    click: Click,
    /// The browser's audition voice, mixed after the master strip like the click.
    preview: crate::preview::Preview,
    metronome: bool,
    next_beat_tick: f64,
    live_track: Option<TrackId>,
    /// Tracks that play the live notes too (`EngineMessage::SetLiveEcho`).
    live_echo: [Option<TrackId>; crate::messages::MAX_LIVE_ECHO],
    recording: bool,
    stats: EngineStats,
    meters: Meters,
    /// Whether `transport.playing` was true on the previous block (to emit note-offs).
    was_playing: bool,
    capture_target: CaptureTarget,
    /// The strip slot being captured, resolved when the target or the snapshot changes.
    capture_slot: Option<usize>,
    /// The effect the analyser looks at.
    analysed: Option<cantus_core::EffectId>,
    /// The Speculums whose signals go to the scope rings (ADR 0056).
    scoped: [Option<cantus_core::EffectId>; crate::fx::speculum::MAX_SCOPES],
    /// How late each watched Speculum's signal is against the transport (`scope_delays`).
    scoped_delay: [usize; crate::fx::speculum::MAX_SCOPES],
    /// Delay compensation (`crate::pdc`): the planner of the lines the nodes run.
    pdc: crate::pdc::Compensation,
    /// Scratch for the callback's own delayed edges (the click, a silent take).
    /// The master's loudness meter, fed after the master strip (ADR 0052).
    master_loud: master_run::MasterLoud,
    edge_l: Vec<f32>,
    edge_r: Vec<f32>,
    /// How late each strip slot's fader output is against the timeline, in samples: the
    /// compensation it waits for plus its own inserts' latency (ADR 0018's `P`).
    strip_post: [usize; MAX_STRIPS],
    /// The inserts' and instruments' latencies when compensation was last planned, as one
    /// number (`Self::latency_signature`): a pass that finds another re-plans (ADR 0056).
    latency_sig: u64,
    /// The click's line (the output's latency) and the captured strip's (its path to the
    /// master), so both come out with the music they belong to (ADR 0022).
    click_line: Option<usize>,
    capture_line: Option<usize>,
    /// Samples played since the last start or seek: until the output's latency has passed,
    /// what is heard is still the start point (ADR 0022).
    played: u64,
    /// MIDI into effects (ADR 0022, `graph::notes`): how many listeners the cells hold,
    /// the last plan's (so a re-plan keeps their cursors), and which track slots have one.
    /// The notes themselves wait in the tracks' outboxes.
    listener_count: usize,
    listeners_prev: Vec<Listener>,
    listened: [bool; MAX_TRACKS],
    /// Samples processed before the current block: the clock notes to effects are timed
    /// on. `note_at` is the time of notes sent outside the passes (live notes,
    /// all-notes-off).
    clock: u64,
    note_at: u64,
    /// Blocks and passes run (the stamps nodes compare against).
    block: u64,
    pass: u64,
    /// Whether the last block ran on the pool (for the blame's critical path).
    last_parallel: bool,
    /// The first block after a stream starts promotes the callback's thread
    /// (`cantus_rt::promote_callback`, ADR 0031 R1), on the driver API the stream set.
    promote: Option<AudioHost>,
    /// FTZ/DAZ inside `process` (on; off only for the test that compares the two).
    flush_denormals: bool,
    /// The input stream's end and the recording rings (ADR 0025, `crate::recorder`).
    pub(crate) input: crate::recorder::InputPort,
    pub(crate) record_rings: Option<Box<crate::recorder::RecordRings>>,
    /// Input and recording state.
    live: rec::Live,
    /// The master's insert chain (ADR 0026, `graph/master.rs`).
    master: master::MasterChain,
    /// Automation (ADR 0027, `processor_auto.rs`).
    auto: auto::AutoState,
    /// Where each block's time goes, and who overran it (`crate::profile`).
    prof: crate::profile::Profiler,
}

impl Processor {
    pub fn new(
        sample_rate: f64,
        rx: Consumer<EngineMessage>,
        tx: Producer<EngineEvent>,
        midi_rx: Consumer<MidiEvent>,
    ) -> Self {
        let sr = sample_rate as f32;
        let pdc = crate::pdc::Compensation::new();
        let workers = pool::Workers::from_env();
        let nodes = Arc::new(Nodes::new(sample_rate, pool::SLOTS, pdc.lines.clone()));
        let mut p = Processor {
            sample_rate,
            rx,
            tx,
            midi_rx,
            snapshot: None,
            pending_garbage: Vec::with_capacity(PENDING_GARBAGE),
            incoming_effects: Vec::with_capacity(MAX_PENDING_EFFECTS),
            scratch_effects: Vec::with_capacity(MAX_EFFECTS_PER_TRACK),
            incoming_instruments: Vec::with_capacity(MAX_TRACKS),
            probed: None,
            transport: Transport::new(sample_rate),
            nodes,
            pool: None,
            track_ids: [None; MAX_TRACKS],
            strip_ids: [None; MAX_STRIPS],
            track_to_slot: Vec::with_capacity(MAX_TRACKS),
            strip_to_slot: Vec::with_capacity(MAX_STRIPS),
            strip_play_order: Vec::with_capacity(MAX_STRIPS),
            plan: Box::new(GraphPlan::new()),
            topo: cluster::TaskTopo::new(),
            map_dirty: true,
            clustered_at: 0,
            clustered_costs: false,
            track_cost: [0.0; MAX_TRACKS],
            strip_cost: [0.0; MAX_STRIPS],
            node_cost: Vec::with_capacity(crate::graph::MAX_NODES),
            block_work: 0,
            pool_wakes: 0,
            shuffle: None,
            split_voices: std::env::var("CANTUS_VOICE_SPLIT").map_or(true, |v| v.trim() != "0"),
            click: Click::new(sr),
            preview: crate::preview::Preview::new(sample_rate),
            metronome: false,
            next_beat_tick: 0.0,
            live_track: None,
            live_echo: [None; crate::messages::MAX_LIVE_ECHO],
            recording: false,
            stats: EngineStats {
                sample_rate,
                ..Default::default()
            },
            meters: Meters::default(),
            was_playing: false,
            capture_target: CaptureTarget::Off,
            capture_slot: None,
            analysed: None,
            scoped: [None; crate::fx::speculum::MAX_SCOPES],
            scoped_delay: [0; crate::fx::speculum::MAX_SCOPES],
            pdc,
            master_loud: master_run::MasterLoud::new(sr),
            edge_l: vec![0.0; PASS_FRAMES],
            edge_r: vec![0.0; PASS_FRAMES],
            strip_post: [0; MAX_STRIPS],
            latency_sig: 0,
            click_line: None,
            capture_line: None,
            played: 0,
            listener_count: 0,
            listeners_prev: Vec::with_capacity(MAX_LISTENERS),
            listened: [false; MAX_TRACKS],
            clock: 0,
            note_at: 0,
            block: 0,
            pass: 0,
            last_parallel: false,
            promote: None,
            flush_denormals: true,
            input: Default::default(),
            record_rings: None,
            live: rec::Live::new(),
            master: master::MasterChain::new(),
            auto: auto::AutoState::new(),
            prof: crate::profile::Profiler::new(),
        };
        p.prefault();
        p.set_workers(workers);
        p
    }

    /// Touch the callback's own buffers once, here on the thread that builds the
    /// processor, so the audio thread never takes their first page faults (ADR 0031 R11);
    /// the nodes' were touched when they were built.
    fn prefault(&mut self) {
        for v in [
            &mut self.edge_l,
            &mut self.edge_r,
            &mut self.master.buf_l,
            &mut self.master.buf_r,
            &mut self.master.dry_l,
            &mut self.master.dry_r,
        ] {
            exec::touch(v);
        }
        // The master meter's histograms (review 1: ADR 0031 R11).
        self.master_loud.prefault();
    }

    /// Ask the first block to promote the calling (callback) thread for the driver API
    /// `host` (ADR 0031 R1). For the device code that builds the output stream; never the
    /// offline renderer, the stems, the bench or the tests, which must not touch thread
    /// priority.
    pub fn promote_on_first_block(&mut self, host: AudioHost) {
        self.promote = Some(host);
    }

    /// Flush denormals inside `process` (on, always, in the application). Off exists for
    /// `tests/ftz.rs`, which renders the null corpus both ways and compares the bits.
    #[doc(hidden)]
    pub fn set_flush_denormals(&mut self, on: bool) {
        self.flush_denormals = on;
    }

    /// Switch the per-node profiler on or off (on by default; the offline renderer turns
    /// it off). Off, no node is blamed; the block histogram is kept either way.
    pub fn set_profiling(&mut self, on: bool) {
        self.prof.set_on(on);
    }

    /// Let the heavy instruments split their voices over the pool's threads (ADR 0038; on
    /// by default). The output is the same bits either way — a switch for measuring. A
    /// change re-clusters at the next block, so the serial hint matches it at once.
    pub fn set_voice_split(&mut self, on: bool) {
        if self.split_voices != on {
            self.split_voices = on;
            self.map_dirty = true;
        }
    }

    pub fn voice_split(&self) -> bool {
        self.split_voices
    }

    /// Whether the current clustering holds the plan to one thread (one long chain — the
    /// critical path above 80 % of the cost — with no instrument splitting its voices, ADR
    /// 0038 §6); `Mode::Auto` honours it. For tests and the bench.
    pub fn plan_held_serial(&self) -> bool {
        crate::sched::Topology::serial_hint(&self.topo)
    }

    /// Where the blocks' time went, node by node, since the totals were last reset — what
    /// the benchmark reports as hot spots.
    pub fn profiler(&self) -> &crate::profile::Profiler {
        &self.prof
    }

    pub fn profiler_mut(&mut self) -> &mut crate::profile::Profiler {
        &mut self.prof
    }

    /// The project's name for a profiler node, as the snapshot now stands.
    pub fn culprit(&self, node: crate::profile::Node) -> crate::profile::Culprit {
        use crate::profile::{Culprit, Node};
        match node {
            Node::Engine => None,
            Node::Track(s) => self
                .track_ids
                .get(s)
                .copied()
                .flatten()
                .map(Culprit::Instrument),
            Node::Strip(s) => self.strip_ids.get(s).copied().flatten().map(Culprit::Strip),
            Node::Insert(s, i) => {
                let strip = self.strip_ids.get(s).copied().flatten();
                let effect = self
                    .nodes
                    .strip(s)
                    .and_then(|st| st.effects.get(i).map(|e| e.id));
                strip.zip(effect).map(|(strip, effect)| Culprit::Insert {
                    strip,
                    effect,
                    index: i as u8,
                })
            }
            Node::MasterInsert(i) => self.master_effect(i).map(|effect| Culprit::MasterInsert {
                effect,
                index: i as u8,
            }),
        }
        .unwrap_or(Culprit::Engine)
    }

    /// The song position being **heard** now, in ticks: the engine's position less the
    /// output's latency (ADR 0022). Exactly the position the engine was at `latency`
    /// samples ago; after a start or a seek, the start point until its first sample has
    /// come out; across a loop wrap, counted back into the loop's end. Stopped, the
    /// engine's position.
    pub fn heard_position_f64(&self) -> f64 {
        let t = &self.transport;
        let latency = self.pdc.total as u64;
        if latency == 0 || !t.is_playing() {
            return t.position_f64();
        }
        let back = latency.min(self.played);
        if let Some(x) = t.position_back(back) {
            return x;
        }
        // The playhead wrapped (or the tempo changed) within the last `back` samples.
        let mut x = t.position_f64() - back as f64 * t.ticks_per_sample();
        if let Some(l) = t.loop_region() {
            let (start, end) = (l.start.0 as f64, l.end.0 as f64);
            if x < start && end > start {
                x = end - (start - x).rem_euclid(end - start);
            }
        }
        x
    }

    /// [`Self::heard_position_f64`] as whole ticks, rounded down like
    /// `Transport::position`.
    pub fn heard_position(&self) -> cantus_core::Ticks {
        cantus_core::Ticks(self.heard_position_f64().floor() as i64)
    }

    /// Copy every track's and strip's output into `tap` as it is mixed (`crate::stems`).
    /// For an offline render: the tap is allocated by the caller, off the audio thread.
    /// While a tap is attached the graph runs on the callback alone: its direct sum is
    /// taken in track order.
    pub fn attach_stem_tap(&mut self, tap: Box<crate::stems::StemTap>) {
        match self.nodes.tap.try_borrow_mut() {
            Some(mut t) => *t = Some(tap),
            None => self.nodes.conflict(),
        }
    }

    /// The stem tap, between blocks.
    pub fn stem_tap(&self) -> Option<StemTapRef<'_>> {
        let t = self.nodes.tap.try_borrow_mut()?;
        t.is_some().then_some(StemTapRef(t))
    }

    /// The slot a snapshot track plays in.
    pub fn track_slot(&self, index: usize) -> Option<usize> {
        self.track_to_slot.get(index).copied()
    }

    /// The slot a snapshot strip plays in.
    pub fn strip_slot(&self, index: usize) -> Option<usize> {
        self.strip_to_slot.get(index).copied()
    }

    /// How many samples a snapshot strip's fader output lags the timeline.
    pub fn strip_latency(&self, index: usize) -> usize {
        self.strip_slot(index)
            .map(|s| self.strip_post[s])
            .unwrap_or(0)
    }

    /// Give the processor the producer end of the analyser's ring (see
    /// [`ANALYSER_SAMPLES`]).
    pub fn attach_analyser(&mut self, producer: Producer<f32>) {
        match self.nodes.analysis.try_borrow_mut() {
            Some(mut a) => a.ring = Some(producer),
            None => self.nodes.conflict(),
        }
    }

    /// Give the processor the producer ends of the scope rings, one per watchable Speculum
    /// (ADR 0056; `EngineHandle::watch_scope`). Extra producers are ignored.
    pub fn attach_scopes(&mut self, producers: Vec<Producer<crate::fx::speculum::ScopeChunk>>) {
        for (cell, p) in self.nodes.scopes.iter().zip(producers) {
            match cell.try_borrow_mut() {
                Some(mut t) => t.ring = Some(p),
                None => self.nodes.conflict(),
            }
        }
    }

    /// The input stream's ring (interleaved, `channels` per frame): what the processor
    /// monitors and records from (ADR 0025). Contract: stored; the engine builder reads it.
    pub fn attach_input(&mut self, consumer: rtrb::Consumer<f32>, channels: usize) {
        self.input = crate::recorder::InputPort {
            ring: Some(consumer),
            channels,
            clock: None,
        };
    }

    /// The live input stream's clock, so the processor can tell how long the frames it
    /// pops have waited (`EngineStats::input_queue`, ADR 0025 §7). Set after
    /// [`Self::attach_input`].
    pub fn attach_input_clock(&mut self, clock: Arc<crate::devices::InputClock>) {
        self.input.clock = Some(clock);
    }

    /// Give the processor the producer end of the mix recorder's ring (see
    /// [`CAPTURE_SAMPLES`]). Built on the UI side before the processor moves to the audio
    /// thread, so nothing is allocated there.
    pub fn attach_capture(&mut self, producer: Producer<f32>) {
        match self.nodes.capture.try_borrow_mut() {
            Some(mut c) => c.ring = Some(producer),
            None => self.nodes.conflict(),
        }
    }

    fn resolve_capture(&mut self) {
        self.capture_slot = match self.capture_target {
            CaptureTarget::Strip(id) => self.strip_slot_for(id),
            _ => None,
        };
    }

    /// A processor with no hardware MIDI queue (tests, offline render).
    pub fn without_midi(
        sample_rate: f64,
        rx: Consumer<EngineMessage>,
        tx: Producer<EngineEvent>,
    ) -> Self {
        let (_p, c) = RingBuffer::<MidiEvent>::new(1);
        // The producer is dropped: the consumer simply stays empty forever.
        Self::new(sample_rate, rx, tx, c)
    }

    pub fn transport(&self) -> &Transport {
        &self.transport
    }

    pub fn stats(&self) -> &EngineStats {
        &self.stats
    }

    fn slot_for(&self, track: TrackId) -> Option<usize> {
        self.track_ids.iter().position(|s| *s == Some(track))
    }

    fn strip_slot_for(&self, strip: StripId) -> Option<usize> {
        self.strip_ids.iter().position(|s| *s == Some(strip))
    }
}

/// The stem tap, borrowed between blocks (`Processor::stem_tap`).
pub struct StemTapRef<'a>(cantus_rt::cell::NodeMut<'a, Option<Box<crate::stems::StemTap>>>);

impl std::ops::Deref for StemTapRef<'_> {
    type Target = crate::stems::StemTap;
    fn deref(&self) -> &crate::stems::StemTap {
        match self.0.as_deref() {
            Some(t) => t,
            // `Processor::stem_tap` hands out a `StemTapRef` only over a tap.
            None => unreachable!("a StemTapRef holds a tap"),
        }
    }
}

impl Processor {
    /// Render one block of interleaved output with `channels` channels.
    /// Called from the audio callback (or the offline renderer).
    pub fn process(&mut self, output: &mut [f32], channels: usize) {
        // Denormals flushed to zero for the whole block, the caller's mode restored when it
        // ends (ADR 0031 §5.2): the callback, the offline render, the stems, the bench and
        // every test then run in the same floating-point mode. The pool's workers hold
        // their own guard for their whole life.
        let _ftz = self.flush_denormals.then(rt_shim::DenormalGuard::new);
        let started = Instant::now();
        self.prof.begin(started);
        let channels = channels.max(1);
        let frames = (output.len() / channels).min(MAX_BLOCK_FRAMES);
        // The first block after a stream start raises the callback's own thread (R1): one
        // system call per stream start, counted as the amended ADR 0005 rule 3 asks.
        if let Some(host) = self.promote.take() {
            let period = std::time::Duration::from_secs_f64(frames as f64 / self.sample_rate);
            if rt_shim::promote_callback(host, period) {
                self.stats.wakes += 1;
            }
        }
        // The pool learns the device period (its workers start on the first one).
        if frames > 0 {
            if let Some(pool) = self.pool.as_mut() {
                pool.set_period(std::time::Duration::from_secs_f64(
                    frames as f64 / self.sample_rate,
                ));
            }
        }
        output.iter_mut().for_each(|s| *s = 0.0);
        // Notes sent before the first segment (live notes, a stop, a seek) are this
        // block's first sample.
        self.note_at = self.clock;

        self.drain_messages();
        self.drain_midi();
        self.retry_garbage();
        self.read_input(frames);
        self.record_block_start();

        if self.was_playing && !self.transport.is_playing() {
            self.release_all();
        }
        self.was_playing = self.transport.is_playing();

        self.block += 1;
        self.render(output, channels, frames);
        if let Some(done) = self.preview.mix(output, channels, frames) {
            self.give_back_preview(done);
        }
        self.after_block(started, frames);

        // Report. If the UI is not draining, dropping these is harmless.
        let _ = self.tx.push(EngineEvent::Position {
            tick: self.transport.position(),
            heard: self.heard_position(),
            playing: self.transport.is_playing(),
        });
        let _ = self.tx.push(EngineEvent::Meters(self.meters.clone()));
        let _ = self.tx.push(EngineEvent::Stats(self.stats));
    }

    /// Handle every pending message now, outside a block — for an offline render, which
    /// must have its delay lines before the first sample it keeps.
    pub fn settle(&mut self) {
        while !self.rx.is_empty() {
            self.drain_messages();
        }
    }
}

/// Where the song is at `transport`'s sample, for the units that follow it.
pub(crate) fn song_position_of(
    transport: &Transport,
    snapshot: &Option<Arc<Snapshot>>,
    sample_rate: f64,
) -> crate::fx::SongPosition {
    let tempo = transport.bpm();
    let ppq = cantus_core::PPQ as f64;
    let beats = transport.position_f64() / ppq;
    let (num, den) = snapshot
        .as_ref()
        .map(|s| (s.time_signature.numerator, s.time_signature.denominator))
        .unwrap_or((4, 4));
    let bar = f64::from(num.max(1)) * 4.0 / f64::from(den.max(1));
    crate::fx::SongPosition {
        playing: transport.is_playing(),
        tempo_bpm: tempo,
        beats,
        samples: (beats * 60.0 / tempo.max(1e-9) * sample_rate).round() as i64,
        bar_start_beats: (beats / bar).floor() * bar,
        time_sig_num: u16::from(num),
        time_sig_den: u16::from(den),
        loop_beats: transport
            .loop_region()
            .map(|l| (l.start.0 as f64 / ppq, l.end.0 as f64 / ppq)),
    }
}

/// A sequenced note as an effect hears it.
#[inline]
pub(crate) fn fx_note(ev: &NoteEvent) -> crate::fx::FxNote {
    if ev.is_on() {
        crate::fx::FxNote::On {
            pitch: ev.pitch,
            velocity: ev.velocity,
        }
    } else {
        crate::fx::FxNote::Off { pitch: ev.pitch }
    }
}

/// The id a native instrument sees for a note. Live input carries one tag for every key,
/// so it is made unique per key here: a unit may rely on an id naming one sounding note.
pub(crate) fn native_id(pitch: u8, tag: u64) -> u64 {
    if tag == LIVE_TAG {
        LIVE_TAG - 1 - pitch as u64
    } else {
        tag
    }
}

/// A MIDI controller or pitch bend as the instrument contract names it, scaled.
fn controller_of(ev: &MidiEvent) -> Option<(Controller, f32)> {
    match ev.status & 0xF0 {
        0xE0 => {
            let raw = ((ev.data2 as i32) << 7 | ev.data1 as i32) - 8192;
            Some((Controller::PitchBend, raw as f32 / 8192.0 * 2.0))
        }
        0xB0 => {
            let v = ev.data2 as f32 / 127.0;
            Some(match ev.data1 {
                1 => (Controller::ModWheel, v),
                11 => (Controller::Expression, v),
                64 => (Controller::Sustain, v),
                n => (Controller::Cc(n), v),
            })
        }
        _ => None,
    }
}

#[inline]
pub(crate) fn fire(slot: &mut TrackSlot, ev: &NoteEvent) {
    if ev.is_on() && slot.instrument == SlotInstrument::Pulsus {
        // A kit seeds what a hit chooses from where the note is (Pulsus II).
        let hit = cantus_dsp::pulsus::Hit {
            id: ev.note.0,
            tick: ev.tick.0,
            ordinal: ev.ordinal,
            sequenced: true,
        };
        slot.drums.trigger(ev.pitch, ev.velocity, hit);
    } else if ev.is_on() {
        slot.note_on(ev.pitch, ev.velocity, ev.note.0);
    } else {
        slot.note_off(ev.pitch, ev.note.0);
    }
}

/// Point the slot at the instrument the snapshot says the track hosts, and hand it its
/// parameters.
///
/// Switching instruments resets **both** engines rather than only the one being left:
/// what is on the way out has to stop, and what is on the way in must not start from
/// whatever state it was in when the track last used it.
fn apply_instrument(
    slot: &mut TrackSlot,
    spec: &InstrumentSpec,
    samples: &[Option<Arc<cantus_core::SampleData>>],
    assets: &crate::inst::Assets,
) {
    let wanted = match spec {
        InstrumentSpec::Firmus(_) => SlotInstrument::Firmus,
        InstrumentSpec::Pulsus(_) => SlotInstrument::Pulsus,
        InstrumentSpec::Native(_) | InstrumentSpec::External(_) => SlotInstrument::Native,
    };
    if slot.instrument != wanted {
        slot.reset();
        slot.instrument = wanted;
    }
    match spec {
        InstrumentSpec::Native(n) => {
            if let Some(u) = slot.native.as_mut() {
                u.unit.configure(n, assets);
            }
        }
        // A hosted plug-in (ADR 0028): its values, diffed against what it holds.
        InstrumentSpec::External(p) => {
            if let Some(u) = slot.native.as_mut() {
                u.unit.configure_hosted(p);
            }
        }
        InstrumentSpec::Firmus(p) => slot.synth.set_settings(&to_settings(p)),
        // Written into the kit's fixed tables in place: this runs on the audio thread,
        // once per snapshot, and must not allocate (ADR 0005). The snapshot carries the
        // decoded audio (`SampleCache::attach`).
        InstrumentSpec::Pulsus(k) => crate::pulsus_kit::configure(&mut slot.drums, k, samples),
    }
}

fn to_settings(p: &cantus_core::FirmusParams) -> FirmusSettings {
    FirmusSettings {
        osc1_wave: wave(p.osc1_wave),
        osc2_wave: wave(p.osc2_wave),
        osc2_semitones: p.osc2_semitones,
        osc2_detune_cents: p.osc2_detune_cents,
        osc_mix: p.osc_mix,
        cutoff_hz: p.cutoff_hz,
        resonance: p.resonance,
        filter_env_octaves: p.filter_env_octaves,
        key_track: p.key_track,
        amp_attack_s: p.amp_attack_s,
        amp_decay_s: p.amp_decay_s,
        amp_sustain: p.amp_sustain,
        amp_release_s: p.amp_release_s,
        filter_attack_s: p.filter_attack_s,
        filter_decay_s: p.filter_decay_s,
        filter_sustain: p.filter_sustain,
        filter_release_s: p.filter_release_s,
        gain_db: p.gain_db,
    }
}

fn wave(w: cantus_core::Waveform) -> Wave {
    match w {
        cantus_core::Waveform::Sine => Wave::Sine,
        cantus_core::Waveform::Triangle => Wave::Triangle,
        cantus_core::Waveform::Saw => Wave::Saw,
        cantus_core::Waveform::Square => Wave::Square,
    }
}

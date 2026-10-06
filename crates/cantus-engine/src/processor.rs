//! The audio-thread processor. Owns every voice, buffer and queue it needs; after
//! construction it never allocates, locks, logs or panics (ADR 0005).
//!
//! Signal flow: **track** (instrument → channel gain/balance) → its **strip** (polarity →
//! swap → inserts, each through its slot's dry/wet → the strip's EQ → width → gain/pan
//! through the pan law → mute) → the strip it feeds, or the master (width → gain/balance
//! → polarity) — **and**, post-fader, a copy at each send's level into the strip the
//! send names.
//!
//! **A graph, run serially** (ADR 0031 §1, `crate::graph`). This file is the control phase
//! and the glue: messages, MIDI, the input ring, recording marks, the transport, snapshots
//! and delay compensation; then each block is cut into **passes** (at loop wraps,
//! automation control points and every `PASS_FRAMES`), and in each pass every node runs in
//! the plan's order — the tracks (`graph/track.rs`), the strips in `Snapshot::strip_order`
//! (`graph/strip.rs`), the master (`graph/master.rs`) — each gathering its inputs in the
//! canonical order from what its predecessors left (`graph/gather.rs`). The audio thread
//! sorts nothing; it reads the plan.
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
use crate::graph::notes::{Listener, Outbox};
use crate::graph::{GraphPlan, PASS_FRAMES};
use crate::inst::{Controller, NativeUnit, NoteOn, UnitKind};
use crate::messages::*;
use crate::transport::Transport;
use cantus_core::{
    apply_width, balance_gains, EffectId, InstrumentSpec, NoteEvent, PanLaw, Snapshot, StripId,
    TrackId,
};
use cantus_dsp::{ChannelEq, Click, Firmus, FirmusSettings, Pulsus, Wave};
use rtrb::{Consumer, Producer, RingBuffer};
use std::sync::Arc;
use std::time::Instant;

/// Automation (ADR 0027): control points, ramps, configured copies.
#[path = "processor_auto.rs"]
mod auto;
/// The master node: its chain's state and installation (ADR 0026).
#[path = "graph/master.rs"]
mod master;
/// The master node's pass: gather, chain, strip, take, click.
#[path = "graph/master_run.rs"]
mod master_run;
/// Input, monitoring, recording and audio clips (ADR 0025).
#[path = "processor_rec.rs"]
mod rec;
/// The strip nodes: gather and outputs.
#[path = "graph/strip.rs"]
pub(crate) mod strip;
/// A strip node's chain and outputs.
#[path = "graph/strip_run.rs"]
mod strip_run;
/// The track nodes.
#[path = "graph/track.rs"]
pub(crate) mod track;
/// Calls into `cantus-rt` (FTZ/DAZ, the callback's priority), behind one small seam.
#[path = "graph/rt_shim.rs"]
#[allow(unsafe_code)]
mod rt_shim;

use master::MasterStrip;
use strip::{produced, StripOut};
use track::TrackOut;
pub use rt_shim::AudioHost;

/// Tag used for live (keyboard/MIDI) notes so they never collide with sequenced note ids.
const LIVE_TAG: u64 = u64::MAX;
/// How many snapshots we can hold back when the garbage queue is full.
const PENDING_GARBAGE: usize = 8;
/// MIDI events drained per block at most.
const MAX_MIDI_PER_BLOCK: usize = 512;
/// Inserts that may hear a track's notes at once (ADR 0022): every insert of every strip
/// and of the master.
const MAX_LISTENERS: usize = (MAX_STRIPS + 1) * MAX_EFFECTS_PER_TRACK;

/// Which of the slot's instruments is the live one.
///
/// A slot holds **both** engines rather than a boxed trait object, because switching a
/// track's instrument must not allocate on the audio thread — and neither must the
/// snapshot that carries the switch. It costs a few kilobytes per slot and buys a rule
/// that cannot be broken by accident (ADR 0005).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum SlotInstrument {
    Firmus,
    Pulsus,
    /// A native instrument's unit (ADR 0023), in `TrackSlot::native`.
    Native,
}

struct TrackSlot {
    id: Option<TrackId>,
    instrument: SlotInstrument,
    synth: Firmus,
    drums: Pulsus,
    /// The native instrument's unit, delivered by `AddInstrument`; `None` while the track
    /// hosts Firmus or Pulsus. Unlike those two it is not preallocated per slot: a native
    /// instrument can be large (a sampler, a piano), so it is built off the audio thread
    /// and swapped in.
    native: Option<Box<NativeUnit>>,
    /// Index into the snapshot track's events of the next event to fire.
    next_event: usize,
    gain: f32,
    pan: f32,
    audible: bool,
    /// The strip slot this track feeds; `None` is the master.
    strip: Option<usize>,
    peak: f32,
    /// The notes it plays, for the inserts that hear it (`graph::notes`).
    outbox: Outbox,
}

/// One mixer strip on the audio thread: its inserts and its controls. What it leaves for
/// its consumers each pass is its [`StripOut`].
///
/// Everything is allocated once for every possible strip, so routing can change with a
/// snapshot and nothing is allocated for it (ADR 0005).
#[allow(clippy::vec_box)]
struct StripSlot {
    id: Option<StripId>,
    /// Insert chain in processing order; capacity fixed at construction.
    effects: Vec<Box<EffectUnit>>,
    bypass: [bool; MAX_EFFECTS_PER_TRACK],
    /// Each slot's dry/wet; 1.0 is skipped entirely.
    mix: [f32; MAX_EFFECTS_PER_TRACK],
    /// The whole chain in or out.
    fx_enabled: bool,
    /// The strip's own EQ, one per channel. `eq_flat` says whether it runs at all.
    eq_l: ChannelEq,
    eq_r: ChannelEq,
    eq_flat: bool,
    gain: f32,
    pan: f32,
    width: f32,
    phase_invert: bool,
    swap: bool,
    muted: bool,
    /// The strip slot this one feeds; `None` is the master.
    output: Option<usize>,
    /// Post-fader sends as `(strip slot, level)`; the first `send_count` are live.
    sends: [(usize, f32); MAX_SENDS_PER_STRIP],
    send_count: usize,
    /// Per insert (aligned with `effects`): the strip it is keyed from, by id as the
    /// snapshot says and by slot once resolved.
    key_id: [Option<StripId>; MAX_EFFECTS_PER_TRACK],
    key: [Option<usize>; MAX_EFFECTS_PER_TRACK],
    /// Per insert: the track whose notes it hears (ADR 0022), and its place among the
    /// processor's listeners once resolved.
    midi_id: [Option<TrackId>; MAX_EFFECTS_PER_TRACK],
    listener: [Option<u16>; MAX_EFFECTS_PER_TRACK],
    /// Something's detector listens to this strip, so its post-fader output is kept.
    is_key: bool,
    /// Something came in, the last pass.
    active: bool,
    peak: f32,
}

impl StripSlot {
    fn new() -> Self {
        StripSlot {
            id: None,
            effects: Vec::with_capacity(MAX_EFFECTS_PER_TRACK),
            bypass: [false; MAX_EFFECTS_PER_TRACK],
            mix: [1.0; MAX_EFFECTS_PER_TRACK],
            fx_enabled: true,
            eq_l: ChannelEq::default(),
            eq_r: ChannelEq::default(),
            eq_flat: true,
            gain: 1.0,
            pan: 0.0,
            width: 1.0,
            phase_invert: false,
            swap: false,
            muted: false,
            output: None,
            sends: [(0, 0.0); MAX_SENDS_PER_STRIP],
            send_count: 0,
            key_id: [None; MAX_EFFECTS_PER_TRACK],
            key: [None; MAX_EFFECTS_PER_TRACK],
            midi_id: [None; MAX_EFFECTS_PER_TRACK],
            listener: [None; MAX_EFFECTS_PER_TRACK],
            is_key: false,
            active: false,
            peak: 0.0,
        }
    }
}

impl TrackSlot {
    fn note_on(&mut self, pitch: u8, velocity: u8, tag: u64) {
        match self.instrument {
            SlotInstrument::Firmus => self.synth.note_on(pitch, velocity, tag),
            SlotInstrument::Pulsus => self.drums.note_on(pitch, velocity, tag),
            SlotInstrument::Native => {
                if let Some(n) = self.native.as_mut() {
                    n.unit.note_on(NoteOn {
                        key: pitch,
                        velocity: velocity as f32 / 127.0,
                        id: native_id(pitch, tag),
                        channel: 0,
                    });
                }
            }
        }
    }

    fn note_off(&mut self, pitch: u8, tag: u64) {
        match self.instrument {
            SlotInstrument::Firmus => self.synth.note_off(pitch, tag),
            SlotInstrument::Pulsus => self.drums.note_off(pitch, tag),
            SlotInstrument::Native => {
                if let Some(n) = self.native.as_mut() {
                    n.unit.note_off(pitch, native_id(pitch, tag), 0.0);
                }
            }
        }
    }

    /// A channel-wide controller from live MIDI. Firmus reads none; Pulsus reads its
    /// hi-hat pedal (a plain CC, CC4 by default).
    fn controller(&mut self, controller: Controller, value: f32) {
        if let (SlotInstrument::Pulsus, Controller::Cc(n)) = (self.instrument, controller) {
            self.drums.controller(n, value);
        }
        if let (SlotInstrument::Native, Some(n)) = (self.instrument, self.native.as_mut()) {
            n.unit.controller(controller, value);
        }
    }

    fn all_notes_off(&mut self) {
        match self.instrument {
            SlotInstrument::Firmus => self.synth.all_notes_off(),
            SlotInstrument::Pulsus => self.drums.all_notes_off(),
            SlotInstrument::Native => {
                if let Some(n) = self.native.as_mut() {
                    n.unit.all_notes_off();
                }
            }
        }
    }

    /// Hard reset. Both engines, not just the live one: a slot that has just changed
    /// instrument must not keep the other one's tail waiting to be heard again.
    fn reset(&mut self) {
        self.synth.reset();
        self.drums.reset();
        if let Some(n) = self.native.as_mut() {
            n.unit.reset();
        }
    }

    fn active_voices(&self) -> usize {
        match self.instrument {
            SlotInstrument::Firmus => self.synth.active_voices(),
            SlotInstrument::Pulsus => self.drums.active_voices(),
            SlotInstrument::Native => self.native.as_ref().map_or(0, |n| n.unit.active_voices()),
        }
    }

    fn new(sample_rate: f32) -> Self {
        TrackSlot {
            id: None,
            instrument: SlotInstrument::Firmus,
            synth: Firmus::new(sample_rate),
            drums: Pulsus::new(sample_rate),
            native: None,
            next_event: 0,
            gain: 1.0,
            pan: 0.0,
            audible: true,
            strip: None,
            peak: 0.0,
            outbox: Outbox::new(),
        }
    }
}

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
    slots: Vec<TrackSlot>,
    /// Snapshot track index → slot index.
    track_to_slot: Vec<usize>,
    strips: Vec<StripSlot>,
    /// Snapshot strip index → strip slot index.
    strip_to_slot: Vec<usize>,
    /// Strip slots in processing order, from the plan.
    strip_play_order: Vec<usize>,
    /// The graph (ADR 0031 §1.2): nodes, order, canonical inputs. Compiled on the UI
    /// thread and sent with its snapshot, or compiled here in place when one arrives
    /// without (`graph::plan`).
    plan: Box<GraphPlan>,
    /// What every node left for its consumers this pass, by slot (`graph/track.rs`,
    /// `graph/strip.rs`), and each delayed send's line output, by line.
    track_out: Vec<TrackOut>,
    strip_out: Vec<StripOut>,
    edge_out: Vec<[Vec<f32>; 2]>,
    /// The scratch every node works in (one set: Phase 0 runs one node at a time; the
    /// pool gives each worker its own). Every node buffer is [`PASS_FRAMES`] long.
    mono: Vec<f32>,
    buf_l: Vec<f32>,
    buf_r: Vec<f32>,
    /// The dry copy a slot's dry/wet needs.
    dry_l: Vec<f32>,
    dry_r: Vec<f32>,
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
    /// The mix recorder's ring, when this processor has one (`attach_capture`).
    capture: Option<Producer<f32>>,
    capture_target: CaptureTarget,
    /// The strip slot being captured, resolved when the target or the snapshot changes.
    capture_slot: Option<usize>,
    /// The analyser's ring and the effect it looks at.
    analyser: Option<Producer<f32>>,
    analysed: Option<cantus_core::EffectId>,
    /// The analysed effect's input, kept while it runs so input and output go out as pairs.
    ana_pre: Vec<f32>,
    /// Post-fader output of the strips something is keyed from, by strip slot, and
    /// whether it was written this pass. Outside `strips` so a strip's effects can read
    /// another strip's output while the strip itself is borrowed.
    key_bufs: Vec<(Vec<f32>, Vec<f32>)>,
    key_valid: [bool; MAX_STRIPS],
    /// A key from a strip that made no sound this pass: silence, not "no key".
    silence: Vec<f32>,
    /// Delay compensation (`crate::pdc`): the pool of lines, and which line serves each
    /// edge — resolved once per snapshot so the render loop never searches.
    pdc: crate::pdc::Compensation,
    track_line: [Option<usize>; MAX_TRACKS],
    out_line: [Option<usize>; MAX_STRIPS],
    send_line: Vec<[Option<usize>; MAX_SENDS_PER_STRIP]>,
    dry_line: Vec<[Option<usize>; MAX_EFFECTS_PER_TRACK]>,
    /// Scratch for a delayed edge.
    edge_l: Vec<f32>,
    edge_r: Vec<f32>,
    /// Stems (`crate::stems`): a copy of every track's and strip's output while a stems
    /// render runs. `None` otherwise, which is always, live.
    stem_tap: Option<Box<crate::stems::StemTap>>,
    /// How late each strip slot's fader output is against the timeline, in samples: the
    /// compensation it waits for plus its own inserts' latency (ADR 0018's `P`).
    strip_post: [usize; MAX_STRIPS],
    /// The click's line (the output's latency) and the captured strip's (its path to the
    /// master), so both come out with the music they belong to (ADR 0022).
    click_line: Option<usize>,
    capture_line: Option<usize>,
    /// Samples played since the last start or seek: until the output's latency has passed,
    /// what is heard is still the start point (ADR 0022).
    played: u64,
    /// MIDI into effects (ADR 0022, `graph::notes`): the inserts hearing a track, which
    /// track slots have one, and the last plan's listeners (so a re-plan keeps their
    /// cursors) — all preallocated. The notes themselves wait in the tracks' outboxes.
    listeners: Vec<Listener>,
    listeners_prev: Vec<Listener>,
    listened: [bool; MAX_TRACKS],
    /// Samples processed before the current block: the clock notes to effects are timed
    /// on. `note_at` is the time of notes sent outside the passes (live notes,
    /// all-notes-off).
    clock: u64,
    note_at: u64,
    /// The captured strip ran this pass (otherwise its take records silence).
    strip_captured: bool,
    /// The first block after a stream starts promotes the callback's thread
    /// (`cantus_rt::promote_callback`, ADR 0031 R1), on the driver API the stream set.
    promote: Option<AudioHost>,
    /// FTZ/DAZ inside `process` (on; off only for the test that compares the two).
    flush_denormals: bool,
    /// The input stream's end and the recording rings (ADR 0025, `crate::recorder`).
    pub(crate) input: crate::recorder::InputPort,
    pub(crate) record_rings: Option<Box<crate::recorder::RecordRings>>,
    /// The block's input, each track's input settings, and the pass's state.
    live: rec::Live,
    /// The master's insert chain (ADR 0026, `processor_master.rs`).
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
        Processor {
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
            slots: (0..MAX_TRACKS).map(|_| TrackSlot::new(sr)).collect(),
            track_to_slot: Vec::with_capacity(MAX_TRACKS),
            strips: (0..MAX_STRIPS).map(|_| StripSlot::new()).collect(),
            strip_to_slot: Vec::with_capacity(MAX_STRIPS),
            strip_play_order: Vec::with_capacity(MAX_STRIPS),
            plan: Box::new(GraphPlan::new()),
            track_out: (0..MAX_TRACKS).map(|_| TrackOut::new()).collect(),
            strip_out: (0..MAX_STRIPS).map(|_| StripOut::new()).collect(),
            edge_out: (0..crate::pdc::POOL)
                .map(|_| [vec![0.0; PASS_FRAMES], vec![0.0; PASS_FRAMES]])
                .collect(),
            mono: vec![0.0; PASS_FRAMES],
            buf_l: vec![0.0; PASS_FRAMES],
            buf_r: vec![0.0; PASS_FRAMES],
            dry_l: vec![0.0; PASS_FRAMES],
            dry_r: vec![0.0; PASS_FRAMES],
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
            capture: None,
            capture_target: CaptureTarget::Off,
            capture_slot: None,
            analyser: None,
            analysed: None,
            ana_pre: vec![0.0; PASS_FRAMES],
            key_bufs: (0..MAX_STRIPS)
                .map(|_| (vec![0.0; PASS_FRAMES], vec![0.0; PASS_FRAMES]))
                .collect(),
            key_valid: [false; MAX_STRIPS],
            silence: vec![0.0; PASS_FRAMES],
            pdc: crate::pdc::Compensation::new(),
            track_line: [None; MAX_TRACKS],
            out_line: [None; MAX_STRIPS],
            send_line: vec![[None; MAX_SENDS_PER_STRIP]; MAX_STRIPS],
            dry_line: vec![[None; MAX_EFFECTS_PER_TRACK]; MAX_STRIPS],
            edge_l: vec![0.0; PASS_FRAMES],
            edge_r: vec![0.0; PASS_FRAMES],
            stem_tap: None,
            strip_post: [0; MAX_STRIPS],
            click_line: None,
            capture_line: None,
            played: 0,
            listeners: Vec::with_capacity(MAX_LISTENERS),
            listeners_prev: Vec::with_capacity(MAX_LISTENERS),
            listened: [false; MAX_TRACKS],
            clock: 0,
            note_at: 0,
            strip_captured: false,
            promote: None,
            flush_denormals: true,
            input: Default::default(),
            record_rings: None,
            live: rec::Live::new(),
            master: master::MasterChain::new(),
            auto: auto::AutoState::new(),
            prof: crate::profile::Profiler::new(),
        }
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
        let insert = |strip: Option<StripId>, effects: &[Box<EffectUnit>], i: usize| {
            Some((strip?, effects.get(i)?.id))
        };
        match node {
            Node::Engine => None,
            Node::Track(s) => self
                .slots
                .get(s)
                .and_then(|t| t.id)
                .map(Culprit::Instrument),
            Node::Strip(s) => self.strips.get(s).and_then(|t| t.id).map(Culprit::Strip),
            Node::Insert(s, i) => self.strips.get(s).and_then(|st| {
                insert(st.id, &st.effects, i).map(|(strip, effect)| Culprit::Insert {
                    strip,
                    effect,
                    index: i as u8,
                })
            }),
            Node::MasterInsert(i) => self.master_effect(i).map(|effect| Culprit::MasterInsert {
                effect,
                index: i as u8,
            }),
        }
        .unwrap_or(Culprit::Engine)
    }

    /// Send a note a track played, at absolute sample `at`, to every effect listening to
    /// it: into the track's outbox, where each listener takes it when the audio reaching
    /// its slot gets there (ADR 0022, `graph::notes`). Allocation-free; a full outbox drops
    /// the note and counts it.
    fn notes_to_fx(&mut self, track: usize, at: u64, note: crate::fx::FxNote) {
        if !self.slots[track].outbox.push(at, note) {
            self.stats.dropped_events += 1;
        }
    }

    /// The end of a block: what is past due and was not delivered (its effect did not
    /// run) is dropped, and each outbox learns how far its slowest listener has read.
    fn prune_notes(&mut self) {
        let now = self.clock;
        let mut floor = [u64::MAX; MAX_TRACKS];
        for l in self.listeners.iter_mut() {
            l.prune(&self.slots[l.track].outbox, now);
            floor[l.track] = floor[l.track].min(l.cursor);
        }
        for (slot, f) in floor.iter().enumerate() {
            if self.listened[slot] {
                let s = &mut self.slots[slot].outbox;
                s.set_floor(if *f == u64::MAX { s.written() } else { *f });
            }
        }
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
    pub fn attach_stem_tap(&mut self, tap: Box<crate::stems::StemTap>) {
        self.stem_tap = Some(tap);
    }

    pub fn stem_tap(&self) -> Option<&crate::stems::StemTap> {
        self.stem_tap.as_deref()
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
        self.analyser = Some(producer);
    }

    /// Give the processor the producer end of the mix recorder's ring (see
    /// [`CAPTURE_SAMPLES`]). Built on the UI side before the processor moves to the audio
    /// thread, so nothing is allocated there.
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

    pub fn attach_capture(&mut self, producer: Producer<f32>) {
        self.capture = Some(producer);
    }

    fn resolve_capture(&mut self) {
        self.capture_slot = match self.capture_target {
            CaptureTarget::Strip(id) => self.strip_slot_for(id),
            _ => None,
        };
    }

    /// Input/output pairs into the analyser's ring. The analyser is a picture, so a full
    /// ring simply drops the rest of the block.
    #[inline]
    fn analyse(ring: &mut Option<Producer<f32>>, pre: &[f32], l: &[f32], r: &[f32]) {
        if let Some(a) = ring.as_mut() {
            for k in 0..pre.len() {
                if a.slots() < 2 {
                    break;
                }
                let _ = a.push(pre[k]);
                let _ = a.push((l[k] + r[k]) * 0.5);
            }
        }
    }

    /// One stereo frame into the recorder's ring; a full ring drops it and counts it.
    #[inline]
    fn capture_frame(capture: &mut Option<Producer<f32>>, stats: &mut EngineStats, l: f32, r: f32) {
        if let Some(c) = capture.as_mut() {
            if c.slots() >= 2 {
                let _ = c.push(l);
                let _ = c.push(r);
            } else {
                stats.capture_overruns += 1;
            }
        }
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

    /// Render one block of interleaved output with `channels` channels.
    /// Called from the audio callback (or the offline renderer).
    pub fn process(&mut self, output: &mut [f32], channels: usize) {
        // Denormals flushed to zero for the whole block, the caller's mode restored when it
        // ends (ADR 0031 §5.2): the callback, the offline render, the stems, the bench and
        // every test then run in the same floating-point mode.
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

        self.render(output, channels, frames);
        if let Some(done) = self.preview.mix(output, channels, frames) {
            self.give_back_preview(done);
        }

        // Instrumentation.
        let ended = Instant::now();
        let elapsed = ended.saturating_duration_since(started).as_secs_f64() * 1e6;
        let budget = frames as f64 / self.sample_rate * 1e6;
        let overran = elapsed > budget && budget > 0.0;
        if let Some((node, ns)) = self.prof.end(ended, overran) {
            self.stats.last_dropout = Some(crate::profile::Dropout {
                culprit: self.culprit(node),
                culprit_us: ns as f32 / 1e3,
                block_us: elapsed as f32,
                budget_us: budget as f32,
                block: self.stats.blocks_processed,
            });
        }
        self.stats.histogram.record(elapsed);
        self.stats.block_frames = frames;
        self.stats.last_block_us = elapsed;
        self.stats.max_block_us = self.stats.max_block_us.max(elapsed);
        self.stats.load = if budget > 0.0 {
            (elapsed / budget) as f32
        } else {
            0.0
        };
        if overran {
            self.stats.overruns += 1;
        }
        self.stats.blocks_processed += 1;
        // The slots a track holds: a freed slot is never rendered again.
        self.stats.active_voices = self
            .slots
            .iter()
            .filter(|s| s.id.is_some())
            .map(TrackSlot::active_voices)
            .sum();

        // Report. If the UI is not draining, dropping these is harmless.
        let _ = self.tx.push(EngineEvent::Position {
            tick: self.transport.position(),
            heard: self.heard_position(),
            playing: self.transport.is_playing(),
        });
        let _ = self.tx.push(EngineEvent::Meters(self.meters.clone()));
        let _ = self.tx.push(EngineEvent::Stats(self.stats));
    }

    fn drain_messages(&mut self) {
        for _ in 0..MAX_MESSAGES_PER_BLOCK {
            let Ok(msg) = self.rx.pop() else { break };
            match msg {
                EngineMessage::SetSnapshot(snapshot) => self.install_snapshot(snapshot, None),
                EngineMessage::SetSnapshotPlanned(snapshot, plan) => {
                    self.install_snapshot(snapshot, Some(plan))
                }
                EngineMessage::Automation(rt) => self.auto_arrived(rt),
                EngineMessage::AddEffect(unit) => {
                    if self.incoming_effects.len() < MAX_PENDING_EFFECTS {
                        self.incoming_effects.push(unit);
                    } else {
                        self.stats.dropped_events += 1;
                        let _ = self.tx.push(EngineEvent::GarbageEffect(unit));
                    }
                }
                EngineMessage::ReplaceEffect(unit) => self.replace_effect(unit),
                EngineMessage::AddInstrument(unit) => {
                    if self.incoming_instruments.len() < self.incoming_instruments.capacity() {
                        self.incoming_instruments.push(unit);
                    } else {
                        self.stats.dropped_events += 1;
                        self.dispose_instrument(unit);
                    }
                }
                EngineMessage::ReplaceInstrument(unit) => self.replace_instrument(unit),
                EngineMessage::SetInstrumentProbe(track) => {
                    self.probed = track;
                    self.meters.inst = [0.0; crate::inst::TELEMETRY_LEN];
                }
                EngineMessage::Play => {
                    if !self.transport.is_playing() {
                        self.reseek_events();
                        self.transport.play();
                        self.played = 0;
                    }
                }
                EngineMessage::Stop => self.transport.stop(),
                EngineMessage::Seek(tick) => {
                    self.release_all();
                    self.transport.seek(tick);
                    self.reseek_events();
                    self.played = 0;
                }
                EngineMessage::NoteOn {
                    track,
                    pitch,
                    velocity,
                } => self.live_note(track, pitch, velocity.max(1)),
                EngineMessage::NoteOff { track, pitch } => self.live_note(track, pitch, 0),
                EngineMessage::Midi(ev) => self.midi(ev),
                EngineMessage::SetLiveTrack(t) => self.live_track = t,
                EngineMessage::SetLiveEcho(tracks) => self.set_live_echo(tracks),
                EngineMessage::SetRecording(on) => self.recording = on,
                EngineMessage::SetAnalyser(id) => self.analysed = id,
                EngineMessage::SetCapture(target) => {
                    self.capture_target = target;
                    self.resolve_capture();
                    // A strip's take is delayed to the master's time: its line.
                    self.compensate();
                }
                EngineMessage::Panic => {
                    // Every note on its way to an effect is forgotten.
                    for l in self.listeners.iter_mut() {
                        l.cursor = self.slots[l.track].outbox.written();
                    }
                    for s in &mut self.slots {
                        s.reset();
                    }
                    for st in &mut self.strips {
                        for e in &mut st.effects {
                            e.reset();
                        }
                    }
                    for e in self.master_units() {
                        e.reset();
                    }
                }
                EngineMessage::PdcLines(mut lines) => {
                    self.pdc.supply(&mut lines);
                    if self.tx.push(EngineEvent::GarbageLines(lines)).is_err() {
                        // Queue full: freed here. Extremely unlikely (queue holds 4096).
                        self.stats.dropped_events += 1;
                    }
                    self.compensate();
                }
                EngineMessage::EffectMemory { effect, memory } => {
                    self.supply_effect(effect, memory);
                }
                EngineMessage::AttachRecorder(rings) => {
                    self.live.new_pass();
                    if let Some(old) = self.record_rings.replace(rings) {
                        let _ = self.tx.push(EngineEvent::GarbageRecorder(old));
                    }
                }
                EngineMessage::DetachRecorder => {
                    self.flush_record_gaps();
                    if let Some(old) = self.record_rings.take() {
                        let _ = self.tx.push(EngineEvent::GarbageRecorder(old));
                    }
                }
                EngineMessage::SetRecordStart(at) => {
                    self.live.record_start = at.map(|t| t.0 as f64);
                }
                EngineMessage::Preview(Some((sample, gain))) => {
                    if let Some(old) = self.preview.start(sample, gain) {
                        self.give_back_preview(old);
                    }
                }
                EngineMessage::Preview(None) => self.preview.stop(),
            }
        }
    }

    /// Hand memory built on the UI thread to the effect that asked for it. The effect may
    /// switch paths now (a linear-phase EQ), so its latency may change: compensation is
    /// planned again. Whatever it hands back goes back to be freed.
    fn supply_effect(
        &mut self,
        effect: cantus_core::EffectId,
        memory: crate::effects::EffectMemory,
    ) {
        let unit = self
            .strips
            .iter_mut()
            .flat_map(|s| s.effects.iter_mut())
            .chain(self.master.effects.iter_mut())
            .chain(self.incoming_effects.iter_mut())
            .find(|e| e.id == effect);
        let back = match unit {
            Some(u) => u.supply(memory),
            None => Some(memory),
        };
        if let Some(back) = back {
            if self.tx.push(EngineEvent::GarbageMemory(back)).is_err() {
                // Queue full: freed here. Extremely unlikely (queue holds 4096).
                self.stats.dropped_events += 1;
            }
        }
        self.compensate();
        self.request_memory();
    }

    /// Ask the UI thread for memory any effect's settings need and it has not got (ADR
    /// 0022). Each need is asked for once; the effect runs its previous path meanwhile.
    fn request_memory(&mut self) {
        let strips = self
            .strips
            .iter_mut()
            .filter(|st| st.id.is_some())
            .map(|st| st.effects.iter_mut());
        for chain in strips.chain(std::iter::once(self.master.effects.iter_mut())) {
            for e in chain {
                if let Some(need) = e.take_request() {
                    let ev = EngineEvent::EffectNeed { effect: e.id, need };
                    if self.tx.push(ev).is_err() {
                        e.forget_request();
                    }
                }
            }
        }
    }

    fn drain_midi(&mut self) {
        for _ in 0..MAX_MIDI_PER_BLOCK {
            let Ok(ev) = self.midi_rx.pop() else { break };
            self.midi(ev);
        }
    }

    fn midi(&mut self, ev: MidiEvent) {
        let Some(track) = self.live_track else { return };
        match ev.status & 0xF0 {
            0x90 if ev.data2 > 0 => self.live_note(track, ev.data1, ev.data2),
            0x80 | 0x90 => self.live_note(track, ev.data1, 0),
            0xB0 if ev.data1 == 123 || ev.data1 == 120 => {
                for t in self.live_targets(track) {
                    if let Some(slot) = t.and_then(|t| self.slot_for(t)) {
                        self.slots[slot].all_notes_off();
                    }
                }
            }
            0xB0 | 0xE0 => {
                if let Some((c, v)) = controller_of(&ev) {
                    for t in self.live_targets(track) {
                        if let Some(slot) = t.and_then(|t| self.slot_for(t)) {
                            self.slots[slot].controller(c, v);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    /// The live track and the tracks echoing it (each once), as a fixed array.
    fn live_targets(
        &self,
        track: TrackId,
    ) -> [Option<TrackId>; crate::messages::MAX_LIVE_ECHO + 1] {
        let mut out = [None; crate::messages::MAX_LIVE_ECHO + 1];
        out[0] = Some(track);
        for (o, e) in out[1..].iter_mut().zip(self.live_echo) {
            *o = e.filter(|e| *e != track);
        }
        out
    }

    /// New echo tracks: a track that stops echoing lets go of what it was playing.
    fn set_live_echo(&mut self, tracks: [Option<TrackId>; crate::messages::MAX_LIVE_ECHO]) {
        for old in self.live_echo {
            if let Some(t) = old.filter(|t| !tracks.contains(&Some(*t))) {
                if let Some(slot) = self.slot_for(t) {
                    self.slots[slot].all_notes_off();
                }
            }
        }
        self.live_echo = tracks;
    }

    /// Play (velocity > 0) or release a live note and, while recording, report it.
    fn live_note(&mut self, track: TrackId, pitch: u8, velocity: u8) {
        // The echo tracks play it too (heard, not reported: the UI records the live
        // track's notes into every armed track itself).
        for e in self.live_echo {
            if let Some(slot) = e.filter(|e| *e != track).and_then(|e| self.slot_for(e)) {
                self.sound_live(slot, pitch, velocity);
            }
        }
        let Some(slot) = self.slot_for(track) else {
            return;
        };
        self.sound_live(slot, pitch, velocity);
        if self.recording && self.transport.is_playing() {
            // Where the player heard it: the output lags the engine by its latency.
            let _ = self.tx.push(EngineEvent::NoteInput(NoteInput {
                track,
                tick: self.heard_position(),
                pitch,
                velocity,
            }));
        }
    }

    /// Play or release a live note on one slot, and tell the effects listening to it.
    fn sound_live(&mut self, slot: usize, pitch: u8, velocity: u8) {
        if velocity > 0 {
            self.slots[slot].note_on(pitch, velocity, LIVE_TAG);
        } else {
            self.slots[slot].note_off(pitch, LIVE_TAG);
        }
        if self.listened[slot] {
            let note = if velocity > 0 {
                crate::fx::FxNote::On { pitch, velocity }
            } else {
                crate::fx::FxNote::Off { pitch }
            };
            self.notes_to_fx(slot, self.note_at, note);
        }
    }

    fn slot_for(&self, track: TrackId) -> Option<usize> {
        self.slots.iter().position(|s| s.id == Some(track))
    }

    fn strip_slot_for(&self, strip: StripId) -> Option<usize> {
        self.strips.iter().position(|s| s.id == Some(strip))
    }

    fn install_snapshot(&mut self, snapshot: Arc<Snapshot>, plan: Option<Box<GraphPlan>>) {
        // The plan: the one compiled for it on the UI thread, or — sent without one (a
        // test, a queue that was full) or with a stale one — compiled here, in place,
        // allocation-free. An unchanged topology keeps the plan it has.
        let key = crate::graph::topology_key(&snapshot);
        match plan {
            Some(p) if p.key() == key => {
                let old = std::mem::replace(&mut self.plan, p);
                self.dispose_plan(old);
            }
            Some(p) => {
                self.dispose_plan(p);
                self.plan.compile_into(&snapshot);
                self.stats.plans_compiled += 1;
            }
            None if self.plan.key() != key => {
                self.plan.compile_into(&snapshot);
                self.stats.plans_compiled += 1;
            }
            None => {}
        }
        // Past the limits: refused, and said so.
        self.stats.refused_tracks = self.plan.refused_tracks() as u64;
        self.stats.refused_strips = self.plan.refused_strips() as u64;
        // Map tracks to slots, keeping slots whose track id survives so voices keep ringing.
        self.track_to_slot.clear();
        let mut used = [false; MAX_TRACKS];
        for t in snapshot.tracks.iter().take(MAX_TRACKS) {
            if let Some(i) = self.slot_for(t.id) {
                used[i] = true;
            }
        }
        for t in snapshot.tracks.iter().take(MAX_TRACKS) {
            let slot = match self.slot_for(t.id) {
                Some(i) => i,
                None => {
                    let free = (0..MAX_TRACKS).find(|i| !used[*i] && self.slots[*i].id.is_none());
                    let i =
                        free.unwrap_or_else(|| (0..MAX_TRACKS).find(|i| !used[*i]).unwrap_or(0));
                    used[i] = true;
                    self.slots[i].reset();
                    self.slots[i].id = Some(t.id);
                    i
                }
            };
            self.track_to_slot.push(slot);
            self.swap_native(slot, t.id, UnitKind::of(&t.instrument));
            let s = &mut self.slots[slot];
            s.gain = t.gain;
            s.pan = t.pan;
            s.audible = t.audible;
            apply_instrument(s, &t.instrument, &t.samples, &t.assets);
        }
        // Slots whose track vanished: silence them and free them.
        for (i, in_use) in used.iter().enumerate() {
            if !in_use && self.slots[i].id.is_some() {
                self.slots[i].all_notes_off();
                self.slots[i].id = None;
                if let Some(u) = self.slots[i].native.take() {
                    self.dispose_instrument(u);
                }
            }
        }
        // Instrument units nobody claimed are stale.
        while let Some(u) = self.incoming_instruments.pop() {
            self.dispose_instrument(u);
        }

        // Strips, the same way: a strip whose id survives keeps its slot, so a reverb's
        // tail keeps ringing across a snapshot.
        self.strip_to_slot.clear();
        let mut strip_used = [false; MAX_STRIPS];
        for st in snapshot.strips.iter().take(MAX_STRIPS) {
            if let Some(i) = self.strip_slot_for(st.id) {
                strip_used[i] = true;
            }
        }
        for st in snapshot.strips.iter().take(MAX_STRIPS) {
            let slot = match self.strip_slot_for(st.id) {
                Some(i) => i,
                None => {
                    let free =
                        (0..MAX_STRIPS).find(|i| !strip_used[*i] && self.strips[*i].id.is_none());
                    let i = free
                        .unwrap_or_else(|| (0..MAX_STRIPS).find(|i| !strip_used[*i]).unwrap_or(0));
                    strip_used[i] = true;
                    self.strips[i].id = Some(st.id);
                    // A slot taken over by another strip must not carry the last one's
                    // EQ state into it.
                    self.strips[i].eq_l.reset();
                    self.strips[i].eq_r.reset();
                    i
                }
            };
            self.strip_to_slot.push(slot);
            self.rebuild_chain(slot, &st.effects);
            let sr = self.sample_rate as f32;
            let ss = &mut self.strips[slot];
            ss.gain = st.gain;
            ss.pan = st.pan;
            ss.width = st.width;
            ss.phase_invert = st.phase_invert;
            ss.swap = st.swap_channels;
            ss.muted = st.muted;
            ss.fx_enabled = st.fx_enabled;
            ss.eq_flat = st.eq.is_flat();
            for (i, b) in st.eq.bands.iter().enumerate() {
                ss.eq_l.set_band(i, sr, b.freq_hz, b.gain_db, b.q);
                ss.eq_r.set_band(i, sr, b.freq_hz, b.gain_db, b.q);
            }
        }
        // Routing is resolved to slot indices once, here — outputs and sends alike.
        for (si, st) in snapshot.strips.iter().take(MAX_STRIPS).enumerate() {
            let slot = self.strip_to_slot[si];
            self.strips[slot].output = st.output.and_then(|o| self.strip_to_slot.get(o).copied());
            let mut count = 0;
            for (to, level) in &st.sends {
                if count >= MAX_SENDS_PER_STRIP {
                    break;
                }
                // A send into itself cannot come through the commands; a file could hold
                // one, and it is dropped here rather than summed into a buffer that has
                // already been read.
                if let Some(d) = self.strip_to_slot.get(*to).copied().filter(|d| *d != slot) {
                    self.strips[slot].sends[count] = (d, *level);
                    count += 1;
                }
            }
            self.strips[slot].send_count = count;
        }
        for (ti, t) in snapshot.tracks.iter().take(MAX_TRACKS).enumerate() {
            let slot = self.track_to_slot[ti];
            self.slots[slot].strip = t.strip.and_then(|s| self.strip_to_slot.get(s).copied());
        }
        for s in self.strips.iter_mut() {
            s.is_key = false;
        }
        // Before the strips' keys are resolved below; the master's own keys after them.
        for slot in 0..MAX_STRIPS {
            if self.strips[slot].id.is_none() {
                continue;
            }
            for i in 0..MAX_EFFECTS_PER_TRACK {
                let k = if i < self.strips[slot].effects.len() {
                    self.strips[slot].key_id[i].and_then(|id| self.strip_slot_for(id))
                } else {
                    None
                };
                self.strips[slot].key[i] = k.filter(|k| *k != slot);
                if let Some(k) = self.strips[slot].key[i] {
                    self.strips[k].is_key = true;
                }
            }
        }
        self.install_master(&snapshot);
        self.resolve_capture();
        // The plan's order, in slots.
        self.strip_play_order.clear();
        for si in self.plan.order() {
            self.strip_play_order.push(self.strip_to_slot[*si as usize]);
        }
        for (i, in_use) in strip_used.iter().enumerate() {
            if !in_use && self.strips[i].id.is_some() {
                self.strips[i].id = None;
                self.strips[i].output = None;
                self.strips[i].send_count = 0;
                while let Some(e) = self.strips[i].effects.pop() {
                    self.dispose_effect(e);
                }
            }
        }
        // Units nobody claimed are stale.
        while let Some(e) = self.incoming_effects.pop() {
            self.dispose_effect(e);
        }
        self.live_install(&snapshot);
        // Before compensating: the click has a line of its own when it is on.
        self.metronome = snapshot.metronome;
        self.compensate();
        self.request_memory();
        self.transport.set_bpm(snapshot.tempo_bpm);
        self.transport.set_loop(snapshot.loop_region.clone());
        self.auto_install(&snapshot);

        let old = self.snapshot.replace(snapshot);
        if let Some(old) = old {
            self.dispose(old);
        }
        self.reseek_events();
    }

    /// Work out every edge's delay (`crate::pdc`) and give each edge that needs one a line.
    /// Runs on the audio thread at each snapshot; loops over fixed arrays, allocates
    /// nothing.
    fn compensate(&mut self) {
        use crate::pdc::Edge;
        let mut arrive = [0usize; MAX_STRIPS];
        let mut post = [0usize; MAX_STRIPS];
        let mut master = 0usize;
        let mut own = [0usize; MAX_STRIPS];
        // Soft bypass (ADR 0022): a bypassed insert — or a chain switched off — keeps its
        // latency, delaying its dry signal by it, so bypassing re-plans nothing.
        for (st, own) in self.strips.iter().zip(own.iter_mut()) {
            if st.id.is_none() {
                continue;
            }
            *own = st.effects.iter().map(|e| e.latency()).sum();
        }
        // A track leaves at its instrument's latency (a hosted synth's lookahead, ADR 0028),
        // so what it feeds arrives no earlier and every other path waits for it.
        let mut leave = [0usize; MAX_TRACKS];
        for ti in 0..self.track_to_slot.len() {
            let slot = self.track_to_slot[ti];
            let s = &self.slots[slot];
            let latency = match s.instrument {
                SlotInstrument::Native => s.native.as_ref().map_or(0, |n| n.unit.latency()),
                SlotInstrument::Firmus | SlotInstrument::Pulsus => 0,
            };
            leave[slot] = latency;
            match s.strip {
                Some(d) => arrive[d] = arrive[d].max(latency),
                None => master = master.max(latency),
            }
        }
        for oi in 0..self.strip_play_order.len() {
            let slot = self.strip_play_order[oi];
            let p = arrive[slot] + own[slot];
            post[slot] = p;
            let st = &self.strips[slot];
            match st.output {
                Some(d) => arrive[d] = arrive[d].max(p),
                None => master = master.max(p),
            }
            for (d, _) in st.sends.iter().take(st.send_count) {
                arrive[*d] = arrive[*d].max(p);
            }
        }
        self.strip_post = post;
        // Effects hearing a track's notes (ADR 0022): each note waits as long as the audio
        // reaching its slot does — the strip's arrival plus the latency of the inserts
        // before it — so note and sound meet at the effect.
        // A listener that was already there keeps its cursor (`Self::listen`).
        std::mem::swap(&mut self.listeners, &mut self.listeners_prev);
        self.listeners.clear();
        self.listened = [false; MAX_TRACKS];
        for st in self.strips.iter_mut() {
            st.listener = [None; MAX_EFFECTS_PER_TRACK];
        }
        for oi in 0..self.strip_play_order.len() {
            let slot = self.strip_play_order[oi];
            let mut ahead = arrive[slot] as u64;
            for i in 0..self.strips[slot].effects.len() {
                let (effect, latency, midi) = {
                    let st = &self.strips[slot];
                    (st.effects[i].id, st.effects[i].latency(), st.midi_id[i])
                };
                let track = midi.and_then(|id| self.slots.iter().position(|s| s.id == Some(id)));
                if let Some(track) = track {
                    self.strips[slot].listener[i] = self.listen(track, effect, ahead);
                }
                ahead += latency as u64;
            }
        }
        // The master chain delays every path equally: it moves what is heard, not the mix.
        let heard = master + self.master.latency();
        let mut kept = self.pdc.begin();
        for ti in 0..self.track_to_slot.len() {
            let slot = self.track_to_slot[ti];
            let want = match self.slots[slot].strip {
                Some(d) => arrive[d],
                None => master,
            };
            self.pdc
                .want(&mut kept, Edge::Track(slot), want - leave[slot]);
        }
        for oi in 0..self.strip_play_order.len() {
            let slot = self.strip_play_order[oi];
            let st = &self.strips[slot];
            let target = match st.output {
                Some(d) => arrive[d],
                None => master,
            };
            let out_delay = target - post[slot];
            let sends = st.sends;
            let send_count = st.send_count;
            // Every latent insert has a dry line of its latency, always running, so its
            // slot mix, its bypass and the chain switch all meet a dry signal already in
            // step with the effect — switching them re-plans nothing and nothing jumps.
            let mut dry = [0usize; MAX_EFFECTS_PER_TRACK];
            for (i, e) in st.effects.iter().enumerate() {
                dry[i] = e.latency();
            }
            self.pdc.want(&mut kept, Edge::Out(slot), out_delay);
            for (k, (d, _)) in sends.iter().take(send_count).enumerate() {
                self.pdc
                    .want(&mut kept, Edge::Send(slot, k), arrive[*d] - post[slot]);
            }
            for (i, delay) in dry.iter().enumerate() {
                self.pdc.want(&mut kept, Edge::Dry(slot, i), *delay);
            }
            if self.capture_slot == Some(slot) {
                self.pdc
                    .want(&mut kept, Edge::Capture, heard.saturating_sub(post[slot]));
            }
        }
        self.plan_master(&mut kept, master);
        if self.metronome {
            self.pdc.want(&mut kept, Edge::Click, heard);
        }
        self.pdc.finish(&kept);
        // Resolve the assignment once, so the render loop reads an array.
        self.track_line = [None; MAX_TRACKS];
        self.out_line = [None; MAX_STRIPS];
        self.send_line
            .iter_mut()
            .for_each(|s| *s = [None; MAX_SENDS_PER_STRIP]);
        self.dry_line
            .iter_mut()
            .for_each(|s| *s = [None; MAX_EFFECTS_PER_TRACK]);
        self.click_line = None;
        self.capture_line = None;
        self.master.dry_line = [None; MAX_EFFECTS_PER_TRACK];
        for (i, line) in self.pdc.lines.iter().enumerate() {
            match line.edge {
                Some(Edge::Track(s)) => self.track_line[s] = Some(i),
                Some(Edge::Out(s)) => self.out_line[s] = Some(i),
                Some(Edge::Send(s, k)) => self.send_line[s][k] = Some(i),
                Some(Edge::Dry(s, k)) => self.dry_line[s][k] = Some(i),
                Some(Edge::Click) => self.click_line = Some(i),
                Some(Edge::Capture) => self.capture_line = Some(i),
                Some(Edge::MasterDry(k)) => self.master.dry_line[k] = Some(i),
                None => {}
            }
        }
        self.pdc.total = heard;
        self.stats.latency_samples = heard as u64;
        self.stats.pdc_unserved = self.pdc.unserved as u64;
        if let Some((count, delay)) = self.pdc.take_request() {
            if self.tx.push(EngineEvent::PdcNeed { count, delay }).is_err() {
                self.pdc.forget_request();
            }
        }
    }

    /// Handle every pending message now, outside a block — for an offline render, which
    /// must have its delay lines before the first sample it keeps.
    pub fn settle(&mut self) {
        while !self.rx.is_empty() {
            self.drain_messages();
        }
    }

    /// Whether a strip's outgoing delay lines still hold signal, so the strip must run
    /// this segment even though nothing new came in.
    fn strip_lines_ringing(&self, slot: usize) -> bool {
        let ringing = |l: Option<usize>| l.is_some_and(|i| self.pdc.lines[i].ringing());
        ringing(self.out_line[slot]) || self.send_line[slot].iter().any(|l| ringing(*l))
    }

    /// Re-order a strip slot's effect chain to match `wanted`, pulling new units from the
    /// incoming pool and returning dropped ones. No allocation: every Vec involved was
    /// sized at construction and `wanted` is capped at MAX_EFFECTS_PER_TRACK.
    fn rebuild_chain(&mut self, slot: usize, wanted: &[cantus_core::EffectSlot]) {
        self.scratch_effects.clear();
        while let Some(e) = self.strips[slot].effects.pop() {
            self.scratch_effects.push(e);
        }
        for (i, want) in wanted.iter().take(MAX_EFFECTS_PER_TRACK).enumerate() {
            let found = self
                .scratch_effects
                .iter()
                .position(|e| e.id == want.id)
                .map(|p| self.scratch_effects.swap_remove(p))
                .or_else(|| {
                    self.incoming_effects
                        .iter()
                        .position(|e| e.id == want.id)
                        .map(|p| self.incoming_effects.swap_remove(p))
                });
            match found {
                Some(mut unit) => {
                    unit.configure(&want.spec);
                    let s = &mut self.strips[slot];
                    let idx = s.effects.len();
                    s.bypass[idx] = want.bypass;
                    s.mix[idx] = want.mix.clamp(0.0, 1.0);
                    s.key_id[idx] = want.sidechain;
                    s.midi_id[idx] = want.midi_from;
                    s.effects.push(unit);
                }
                None => {
                    // Unit not delivered yet (or dropped): skip it this snapshot.
                    self.stats.dropped_events += 1;
                    let _ = i;
                }
            }
        }
        while let Some(e) = self.scratch_effects.pop() {
            self.dispose_effect(e);
        }
    }

    fn dispose(&mut self, old: Arc<Snapshot>) {
        if let Err(rtrb::PushError::Full(EngineEvent::Garbage(old))) =
            self.tx.push(EngineEvent::Garbage(old))
        {
            if self.pending_garbage.len() < PENDING_GARBAGE {
                self.pending_garbage.push(old);
            } else {
                // Last resort: dropping here may free memory on the audio thread.
                self.stats.dropped_events += 1;
                drop(old);
            }
        }
    }

    /// A plan the processor no longer uses goes back to be freed on the UI thread.
    fn dispose_plan(&mut self, old: Box<GraphPlan>) {
        if self.tx.push(EngineEvent::GarbagePlan(old)).is_err() {
            // Queue full: freed here. Extremely unlikely (the queue holds thousands).
            self.stats.dropped_events += 1;
        }
    }

    /// Swap `unit` in for the running unit with its id — a hosted plug-in restarted (ADR
    /// 0019) — keeping its place, bypass, mix and key; hand the old one back; plan delay
    /// compensation again, since the new one may report another latency.
    fn replace_effect(&mut self, mut unit: Box<EffectUnit>) {
        let snapshot = self.snapshot.clone();
        if let Some(spec) = snapshot
            .as_ref()
            .and_then(|s| s.effect_slots().find(|e| e.id == unit.id).map(|e| &e.spec))
        {
            unit.configure(spec);
        }
        for slot in 0..MAX_STRIPS {
            if let Some(i) = self.strips[slot]
                .effects
                .iter()
                .position(|e| e.id == unit.id)
            {
                let mut old = std::mem::replace(&mut self.strips[slot].effects[i], unit);
                old.replaced = true;
                self.dispose_effect(old);
                self.compensate();
                return;
            }
        }
        if let Some(i) = self.master.effects.iter().position(|e| e.id == unit.id) {
            let mut old = std::mem::replace(&mut self.master.effects[i], unit);
            old.replaced = true;
            self.dispose_effect(old);
            self.compensate();
            return;
        }
        // Not in a chain yet: it waits with the new ones, in place of any waiting there.
        if let Some(p) = self.incoming_effects.iter().position(|e| e.id == unit.id) {
            let mut old = std::mem::replace(&mut self.incoming_effects[p], unit);
            old.replaced = true;
            self.dispose_effect(old);
        } else if self.incoming_effects.len() < MAX_PENDING_EFFECTS {
            self.incoming_effects.push(unit);
        } else {
            self.stats.dropped_events += 1;
            self.dispose_effect(unit);
        }
    }

    /// Where the song is now, for the units that follow it.
    fn song_position(&self) -> crate::fx::SongPosition {
        song_position_of(&self.transport, &self.snapshot, self.sample_rate)
    }

    /// Give a slot the native unit its track now needs (`kind`), or none. A unit already of
    /// the right kind stays, so voices ring across snapshots; otherwise the one delivered
    /// for this track and kind is taken from the incoming list and the old one goes back.
    fn swap_native(&mut self, slot: usize, track: TrackId, kind: Option<UnitKind>) {
        let have = self.slots[slot].native.as_ref().map(|n| (n.track, n.kind));
        if kind.is_some() && have == kind.map(|k| (track, k)) {
            return;
        }
        let incoming = kind.and_then(|k| {
            self.incoming_instruments
                .iter()
                .position(|u| u.track == track && u.kind == k)
                .map(|p| self.incoming_instruments.swap_remove(p))
        });
        let old = match incoming {
            Some(u) => self.slots[slot].native.replace(u),
            None => self.slots[slot].native.take(),
        };
        if let Some(old) = old {
            self.dispose_instrument(old);
        }
    }

    /// A unit in place of the one running for its track and kind (ADR 0028): swapped in
    /// now, keeping the slot; or, when no slot runs that track and kind, waiting with the
    /// new ones in place of any waiting there.
    fn replace_instrument(&mut self, unit: Box<NativeUnit>) {
        let (track, kind) = (unit.track, unit.kind);
        let running = self
            .slots
            .iter()
            .position(|s| s.id == Some(track) && s.native.as_ref().is_some_and(|n| n.kind == kind));
        if let Some(i) = running {
            if let Some(old) = self.slots[i].native.replace(unit) {
                self.dispose_instrument(old);
            }
            self.compensate();
            return;
        }
        let waiting = self
            .incoming_instruments
            .iter()
            .position(|u| u.track == track && u.kind == kind);
        if let Some(p) = waiting {
            let old = std::mem::replace(&mut self.incoming_instruments[p], unit);
            self.dispose_instrument(old);
        } else if self.incoming_instruments.len() < self.incoming_instruments.capacity() {
            self.incoming_instruments.push(unit);
        } else {
            self.stats.dropped_events += 1;
            self.dispose_instrument(unit);
        }
    }

    fn dispose_instrument(&mut self, mut unit: Box<NativeUnit>) {
        // It will not be processed again: a hosted instrument stops processing here, on
        // the audio thread, before it leaves (ADR 0028).
        unit.unit.stop();
        if self.tx.push(EngineEvent::GarbageInstrument(unit)).is_err() {
            self.stats.dropped_events += 1;
        }
    }

    fn dispose_effect(&mut self, mut unit: Box<EffectUnit>) {
        // It will not be processed again: a hosted plug-in stops processing here, on the
        // audio thread, before it leaves.
        unit.stop();
        if self.tx.push(EngineEvent::GarbageEffect(unit)).is_err() {
            // Queue full: the box is freed here. Extremely unlikely (queue holds 4096).
            self.stats.dropped_events += 1;
        }
    }

    /// An audition's sample goes back to the UI to be freed there. The queue holds
    /// thousands of events; were it full, the UI's own copy in its preview cache still
    /// keeps this from being the last reference.
    fn give_back_preview(&mut self, s: Arc<cantus_core::SampleData>) {
        if self.tx.push(EngineEvent::GarbagePreview(s)).is_err() {
            self.stats.dropped_events += 1;
        }
    }

    fn retry_garbage(&mut self) {
        while let Some(g) = self.pending_garbage.pop() {
            if let Err(rtrb::PushError::Full(EngineEvent::Garbage(g))) =
                self.tx.push(EngineEvent::Garbage(g))
            {
                self.pending_garbage.push(g);
                break;
            }
        }
    }

    /// Position each track's event cursor at the current playhead and reset the metronome.
    fn reseek_events(&mut self) {
        let pos = self.transport.position_f64();
        let tick = cantus_core::Ticks(pos.ceil() as i64);
        if let Some(snap) = &self.snapshot {
            for (ti, t) in snap
                .tracks
                .iter()
                .enumerate()
                .take(self.track_to_slot.len())
            {
                let slot = self.track_to_slot[ti];
                self.slots[slot].next_event = t.first_event_at_or_after(tick);
            }
            let beat = snap.time_signature.ticks_per_beat().0 as f64;
            self.next_beat_tick = (pos / beat).ceil() * beat;
        }
        self.auto_reseek();
    }

    fn release_all(&mut self) {
        for s in &mut self.slots {
            s.all_notes_off();
        }
        // And every effect listening to a track hears its notes stop (ADR 0022).
        let listened = self.listened;
        for (track, on) in listened.iter().enumerate() {
            if *on {
                self.notes_to_fx(track, self.note_at, crate::fx::FxNote::AllOff);
            }
        }
    }

    /// The block, as passes of the graph (`crate::graph`): cut only where everything must
    /// stop at once — a loop wrap, an automation control point, every `PASS_FRAMES` —
    /// and in each pass every node runs in the plan's order at its own segments.
    fn render(&mut self, output: &mut [f32], channels: usize, frames: usize) {
        if let Some(tap) = self.stem_tap.as_mut() {
            tap.begin(frames);
        }
        for s in &mut self.slots {
            s.peak = 0.0;
        }
        for st in &mut self.strips {
            st.peak = 0.0;
        }
        let mut master_peak = [0f32; 2];
        let (master_gain, master_width, master_pan, master_invert, law) = self
            .snapshot
            .as_ref()
            .map(|s| {
                (
                    s.master_gain,
                    s.master_width,
                    s.master_pan,
                    s.master_phase_invert,
                    s.pan_law,
                )
            })
            .unwrap_or((1.0, 1.0, 0.0, false, PanLaw::default()));
        // The master's pan is a *balance*, not a pan pot, so it does not take the
        // project's pan law. A pan law compensates for placing a **mono source** in a
        // stereo field; the master carries a mix that is already stereo, and running it
        // through, say, the −6 dB law would take 6 dB off every centred mix for nothing.
        // `balance_gains` is the balance formulation — the `ZeroDb` law's taper on the far
        // channel — and returns *exactly* unity at centre. `ZeroDb.gains(0)` computes
        // `cos(π/4)·√2`, which in f32 is 0.99999994: a master at its default touched every
        // sample by one rounding step. It no longer does (see the capture null test).
        let (master_l, master_r) = balance_gains(master_pan);
        let master = MasterStrip {
            gain: master_gain,
            width: master_width,
            l: master_l,
            r: master_r,
            sign: if master_invert { -1.0f32 } else { 1.0 },
        };
        let snap = self.snapshot.clone();

        let mut done = 0usize;
        let mut guard = 0usize;
        while done < frames {
            self.prof.enter(crate::profile::Node::Engine);
            guard += 1;
            if guard > MAX_BLOCK_FRAMES * 2 {
                break; // Impossible unless the loop region is degenerate; never spin.
            }
            // A control point due now runs first; the pass ends at the next one.
            let mut end = self.auto_point(done, frames).min(done + PASS_FRAMES);
            let mut wrap = false;
            if self.transport.is_playing() && snap.is_some() {
                if let Some(off) = self.transport.samples_until_loop_end() {
                    let off = off as usize;
                    if done + off <= end {
                        end = done + off;
                        wrap = true;
                    }
                }
            }
            if end <= done {
                // Only possible for a loop boundary that is due right now: what is due at
                // this sample fires first, as it always did, and then the loop wraps.
                if wrap {
                    if let Some(s) = snap.as_deref() {
                        self.fire_due_now(s, done);
                    }
                    self.note_at = self.clock + done as u64;
                    self.wrap_loop();
                }
                continue;
            }
            let n = end - done;
            self.run_pass(
                snap.as_deref(),
                &mut output[done * channels..end * channels],
                channels,
                done,
                n,
                law,
                master,
                &mut master_peak,
            );
            done = end;
            if self.transport.is_playing() {
                self.transport.advance(n as u64);
                self.played = self.played.saturating_add(n as u64);
                if wrap {
                    self.note_at = self.clock + done as u64;
                    self.wrap_loop();
                }
            }
        }
        self.clock += done as u64;
        // Notes for effects that did not run, or are gone, never arrive: what is past is
        // dropped.
        self.prune_notes();

        self.meters.master = master_peak;
        self.meters.master_gr = self.master_gr();
        self.meters.track_count = self.track_to_slot.len();
        for (ti, slot) in self.track_to_slot.iter().enumerate() {
            self.meters.tracks[ti] = self.slots[*slot].peak;
        }
        self.meters.strip_count = self.strip_to_slot.len();
        for (si, slot) in self.strip_to_slot.iter().enumerate() {
            let st = &self.strips[*slot];
            self.meters.strips[si] = st.peak;
            // The desk's reduction lane: the deepest of the strip's working compressors.
            let mut gr = 0.0f32;
            if st.fx_enabled && st.active {
                for (i, e) in st.effects.iter().enumerate() {
                    if let (false, Some(m)) = (st.bypass[i], e.comp_meter()) {
                        gr = gr.max(m.gr_db);
                    }
                }
            }
            self.meters.strip_gr[si] = gr;
        }
    }

    /// One pass `[p0, p0 + n)` of the block: every node once, in the plan's order — the
    /// tracks, the strips in `strip_order`, the master. `out` is the pass's stretch of the
    /// device's buffer.
    #[allow(clippy::too_many_arguments)]
    fn run_pass(
        &mut self,
        snap: Option<&Snapshot>,
        out: &mut [f32],
        channels: usize,
        p0: usize,
        n: usize,
        law: PanLaw,
        master: MasterStrip,
        peaks: &mut [f32; 2],
    ) {
        // No key has been written yet.
        self.key_valid = [false; MAX_STRIPS];
        // Armed tracks' input into their recording rings (ADR 0025).
        self.record_segment(p0, n);
        // Where the song is at the pass's first sample, for the strips' and the master's
        // units that follow it (hosted plug-ins): a pass never crosses a loop wrap.
        let position = self.song_position();
        // ---- tracks: instrument → channel gain and balance → their output ---------------
        if let Some(s) = snap {
            for ti in 0..self.track_to_slot.len() {
                self.run_track(s, ti, p0, n);
            }
        }
        // ---- strips, in dependency order: gather → polarity → swap → inserts → EQ →
        // width → gain/pan → output and sends ---------------------------------------------
        self.strip_captured = false;
        for place in 0..self.strip_play_order.len() {
            self.run_strip(place, p0, n, &position, law);
        }
        // A captured strip that did not run this pass — nothing came in, nothing rings —
        // still records its silence: a take must keep time.
        if matches!(self.capture_target, CaptureTarget::Strip(_)) && !self.strip_captured {
            match self.capture_line {
                Some(li) => {
                    // Its line may still hold what the strip played before.
                    self.edge_l[..n].iter_mut().for_each(|x| *x = 0.0);
                    self.edge_r[..n].iter_mut().for_each(|x| *x = 0.0);
                    self.pdc.lines[li].run(&mut self.edge_l[..n], &mut self.edge_r[..n]);
                    for i in 0..n {
                        Self::capture_frame(
                            &mut self.capture,
                            &mut self.stats,
                            self.edge_l[i],
                            self.edge_r[i],
                        );
                    }
                }
                None => {
                    for _ in 0..n {
                        Self::capture_frame(&mut self.capture, &mut self.stats, 0.0, 0.0);
                    }
                }
            }
        }
        // ---- the master: gather → chain → strip → take → click ---------------------------
        self.run_master_node(snap, out, channels, p0, n, &position, master, peaks);
    }

    /// At a loop boundary due right now (the playhead sits on the loop's end): whatever is
    /// due at this sample fires before the wrap releases it, as the serial engine did at
    /// the top of every segment.
    fn fire_due_now(&mut self, snap: &Snapshot, done: usize) {
        for ti in 0..self.track_to_slot.len() {
            let slot = self.track_to_slot[ti];
            let t = &snap.tracks[ti];
            loop {
                let s = &mut self.slots[slot];
                let Some(ev) = t.events.get(s.next_event) else {
                    break;
                };
                if self.transport.samples_until(ev.tick.0 as f64) != 0 {
                    break;
                }
                fire(s, ev);
                s.next_event += 1;
                if self.listened[slot] && !s.outbox.push(self.clock + done as u64, fx_note(ev)) {
                    self.stats.dropped_events += 1;
                }
            }
        }
        if self.metronome && self.transport.samples_until(self.next_beat_tick) == 0 {
            let beat_len = snap.time_signature.ticks_per_beat().0 as f64;
            let bar_len = snap.time_signature.ticks_per_bar().0 as f64;
            let in_bar = self.next_beat_tick.rem_euclid(bar_len);
            if self.click_allowed() {
                self.click.trigger(in_bar.abs() < 0.5);
            }
            self.next_beat_tick += beat_len;
        }
    }

    fn wrap_loop(&mut self) {
        // `Transport::advance` already moved the playhead to loop start when it detected
        // the wrap; if it has not (boundary exactly at segment start), force it.
        if let Some(l) = self.transport.loop_region().cloned() {
            if self.transport.samples_until_loop_end() == Some(0) {
                self.transport.seek(l.start);
            }
        }
        self.release_all();
        self.reseek_events();
        self.record_wrapped();
    }
}

/// Where the song is at `transport`'s sample, for the units that follow it.
fn song_position_of(
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
fn fx_note(ev: &NoteEvent) -> crate::fx::FxNote {
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
fn native_id(pitch: u8, tag: u64) -> u64 {
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
fn fire(slot: &mut TrackSlot, ev: &NoteEvent) {
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
    {
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
}

fn wave(w: cantus_core::Waveform) -> Wave {
    match w {
        cantus_core::Waveform::Sine => Wave::Sine,
        cantus_core::Waveform::Triangle => Wave::Triangle,
        cantus_core::Waveform::Saw => Wave::Saw,
        cantus_core::Waveform::Square => Wave::Square,
    }
}

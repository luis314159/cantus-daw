//! The audio-thread processor. Owns every voice, buffer and queue it needs; after
//! construction it never allocates, locks, logs or panics (ADR 0005).
//!
//! Signal flow: **track** (instrument → channel gain/balance) → its **strip** (polarity →
//! swap → inserts, each through its slot's dry/wet → the strip's EQ → width → gain/pan
//! through the pan law → mute) → the strip it feeds, or the master (width → gain/balance
//! → polarity) — **and**, post-fader, a copy at each send's level into the strip the
//! send names.
//!
//! Strips are processed in the order the snapshot gives (`Snapshot::strip_order`): every
//! strip after all the strips feeding it, so a bus sees its inputs complete. The audio
//! thread sorts nothing; it reads the order.
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
use crate::inst::{Controller, NativeUnit, NoteOn};
use crate::messages::*;
use crate::transport::Transport;
use cantus_core::{
    apply_width, balance_gains, InstrumentSpec, NoteEvent, PanLaw, Snapshot, StripId, TrackId,
};
use cantus_dsp::{ChannelEq, Click, Firmus, FirmusSettings, Pulsus, Wave};
use rtrb::{Consumer, Producer, RingBuffer};
use std::sync::Arc;
use std::time::Instant;

#[path = "processor_master.rs"]
mod master;
/// Input, monitoring, recording and audio clips (ADR 0025).
#[path = "processor_rec.rs"]
mod rec;

/// Tag used for live (keyboard/MIDI) notes so they never collide with sequenced note ids.
const LIVE_TAG: u64 = u64::MAX;
/// How many snapshots we can hold back when the garbage queue is full.
const PENDING_GARBAGE: usize = 8;
/// MIDI events drained per block at most.
const MAX_MIDI_PER_BLOCK: usize = 512;
/// Notes on their way to effects (ADR 0022): each waits for the latency ahead of its slot.
const MAX_FX_NOTES: usize = 1024;

/// An effect listening to a track's notes: the track's slot, the effect, and how late the
/// audio reaching the effect is — the notes wait as long, to arrive with it.
#[derive(Copy, Clone, Debug)]
struct FxListener {
    track: usize,
    effect: cantus_core::EffectId,
    delay: u64,
}

/// A note on its way to an effect, due at a sample of the processor's clock.
#[derive(Copy, Clone, Debug)]
struct FxNoteDue {
    due: u64,
    effect: cantus_core::EffectId,
    note: crate::fx::FxNote,
}

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
}

/// One mixer strip on the audio thread: its own stereo buffer, its inserts, its controls.
///
/// Buffers are allocated once for every possible strip, so routing can change with a
/// snapshot and nothing is allocated for it (ADR 0005). Sixty-four strips at the largest
/// block is a few megabytes, which is a fair price for a rule that cannot be broken.
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
    /// Per insert: the track whose notes it hears (ADR 0022).
    midi_id: [Option<TrackId>; MAX_EFFECTS_PER_TRACK],
    /// Something's detector listens to this strip, so its post-fader output is kept.
    is_key: bool,
    /// Something wrote into the buffer this segment.
    active: bool,
    peak: f32,
    buf_l: Vec<f32>,
    buf_r: Vec<f32>,
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
            is_key: false,
            active: false,
            peak: 0.0,
            buf_l: vec![0.0; MAX_BLOCK_FRAMES],
            buf_r: vec![0.0; MAX_BLOCK_FRAMES],
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
    /// Strip slots in processing order, from the snapshot.
    strip_play_order: Vec<usize>,
    mono: Vec<f32>,
    buf_l: Vec<f32>,
    buf_r: Vec<f32>,
    /// The dry copy a slot's dry/wet needs; one pair for the whole processor.
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
    /// whether it was written this segment. Outside `strips` so a strip's effects can read
    /// another strip's output while the strip itself is borrowed.
    key_bufs: Vec<(Vec<f32>, Vec<f32>)>,
    key_valid: [bool; MAX_STRIPS],
    /// A key from a strip that made no sound this segment: silence, not "no key".
    silence: Vec<f32>,
    /// Delay compensation (`crate::pdc`): the pool of lines, and which line serves each
    /// edge — resolved once per snapshot so the render loop never searches.
    pdc: crate::pdc::Compensation,
    track_line: [Option<usize>; MAX_TRACKS],
    out_line: [Option<usize>; MAX_STRIPS],
    send_line: [[Option<usize>; MAX_SENDS_PER_STRIP]; MAX_STRIPS],
    dry_line: [[Option<usize>; MAX_EFFECTS_PER_TRACK]; MAX_STRIPS],
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
    /// MIDI into effects (ADR 0022): who listens to which track, which track slots have a
    /// listener, and the notes on their way — all preallocated.
    fx_listeners: Vec<FxListener>,
    listened: [bool; MAX_TRACKS],
    fx_notes: Vec<FxNoteDue>,
    /// Samples processed before the current block: the clock notes to effects are timed
    /// on. `note_at` is the time of notes sent outside the segment loop (live notes,
    /// all-notes-off).
    clock: u64,
    note_at: u64,
    /// The input stream's end and the recording rings (ADR 0025, `crate::recorder`).
    pub(crate) input: crate::recorder::InputPort,
    pub(crate) record_rings: Option<Box<crate::recorder::RecordRings>>,
    /// The block's input, each track's input settings, and the pass's state.
    live: rec::Live,
    /// The master's insert chain (ADR 0026, `processor_master.rs`).
    master: master::MasterChain,
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
            mono: vec![0.0; MAX_BLOCK_FRAMES],
            buf_l: vec![0.0; MAX_BLOCK_FRAMES],
            buf_r: vec![0.0; MAX_BLOCK_FRAMES],
            dry_l: vec![0.0; MAX_BLOCK_FRAMES],
            dry_r: vec![0.0; MAX_BLOCK_FRAMES],
            click: Click::new(sr),
            preview: crate::preview::Preview::new(sr),
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
            ana_pre: vec![0.0; MAX_BLOCK_FRAMES],
            key_bufs: (0..MAX_STRIPS)
                .map(|_| (vec![0.0; MAX_BLOCK_FRAMES], vec![0.0; MAX_BLOCK_FRAMES]))
                .collect(),
            key_valid: [false; MAX_STRIPS],
            silence: vec![0.0; MAX_BLOCK_FRAMES],
            pdc: crate::pdc::Compensation::new(),
            track_line: [None; MAX_TRACKS],
            out_line: [None; MAX_STRIPS],
            send_line: [[None; MAX_SENDS_PER_STRIP]; MAX_STRIPS],
            dry_line: [[None; MAX_EFFECTS_PER_TRACK]; MAX_STRIPS],
            edge_l: vec![0.0; MAX_BLOCK_FRAMES],
            edge_r: vec![0.0; MAX_BLOCK_FRAMES],
            stem_tap: None,
            strip_post: [0; MAX_STRIPS],
            click_line: None,
            capture_line: None,
            played: 0,
            fx_listeners: Vec::with_capacity(MAX_STRIPS * MAX_EFFECTS_PER_TRACK),
            listened: [false; MAX_TRACKS],
            fx_notes: Vec::with_capacity(MAX_FX_NOTES),
            clock: 0,
            note_at: 0,
            input: Default::default(),
            record_rings: None,
            live: rec::Live::new(),
            master: master::MasterChain::new(),
        }
    }

    /// Send a note a track played to every effect listening to it, due when the audio
    /// reaching each effect gets there (ADR 0022). Allocation-free; a full queue drops the
    /// note and counts it.
    fn notes_to_fx(&mut self, track: usize, at: u64, note: crate::fx::FxNote) {
        for l in self.fx_listeners.iter().filter(|l| l.track == track) {
            if self.fx_notes.len() < MAX_FX_NOTES {
                self.fx_notes.push(FxNoteDue {
                    due: at + l.delay,
                    effect: l.effect,
                    note,
                });
            } else {
                self.stats.dropped_events += 1;
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
        let started = Instant::now();
        let channels = channels.max(1);
        let frames = (output.len() / channels).min(MAX_BLOCK_FRAMES);
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
        let elapsed = started.elapsed().as_secs_f64() * 1e6;
        let budget = frames as f64 / self.sample_rate * 1e6;
        self.stats.block_frames = frames;
        self.stats.last_block_us = elapsed;
        self.stats.max_block_us = self.stats.max_block_us.max(elapsed);
        self.stats.load = if budget > 0.0 {
            (elapsed / budget) as f32
        } else {
            0.0
        };
        if elapsed > budget && budget > 0.0 {
            self.stats.overruns += 1;
        }
        self.stats.blocks_processed += 1;
        self.stats.active_voices = self.slots.iter().map(TrackSlot::active_voices).sum();

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
                EngineMessage::SetSnapshot(snapshot) => self.install_snapshot(snapshot),
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
                    self.fx_notes.clear();
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

    fn install_snapshot(&mut self, snapshot: Arc<Snapshot>) {
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
            self.swap_native(slot, t.id, t.instrument.native().map(|n| n.kind));
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
        self.strip_play_order.clear();
        for si in &snapshot.strip_order {
            if let Some(slot) = self.strip_to_slot.get(*si) {
                self.strip_play_order.push(*slot);
            }
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
        self.fx_listeners.clear();
        self.listened = [false; MAX_TRACKS];
        for oi in 0..self.strip_play_order.len() {
            let slot = self.strip_play_order[oi];
            let mut ahead = arrive[slot] as u64;
            let st = &self.strips[slot];
            for (e, midi) in st.effects.iter().zip(st.midi_id.iter()) {
                let track = midi.and_then(|id| self.slots.iter().position(|s| s.id == Some(id)));
                if let Some(track) = track {
                    if self.fx_listeners.len() < self.fx_listeners.capacity() {
                        self.fx_listeners.push(FxListener {
                            track,
                            effect: e.id,
                            delay: ahead,
                        });
                        self.listened[track] = true;
                    }
                }
                ahead += e.latency() as u64;
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
            self.pdc.want(&mut kept, Edge::Track(slot), want);
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
        self.send_line = [[None; MAX_SENDS_PER_STRIP]; MAX_STRIPS];
        self.dry_line = [[None; MAX_EFFECTS_PER_TRACK]; MAX_STRIPS];
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
        let tempo = self.transport.bpm();
        let ppq = cantus_core::PPQ as f64;
        let beats = self.transport.position_f64() / ppq;
        let (num, den) = self
            .snapshot
            .as_ref()
            .map(|s| (s.time_signature.numerator, s.time_signature.denominator))
            .unwrap_or((4, 4));
        let bar = f64::from(num.max(1)) * 4.0 / f64::from(den.max(1));
        crate::fx::SongPosition {
            playing: self.transport.is_playing(),
            tempo_bpm: tempo,
            beats,
            samples: (beats * 60.0 / tempo.max(1e-9) * self.sample_rate).round() as i64,
            bar_start_beats: (beats / bar).floor() * bar,
            time_sig_num: u16::from(num),
            time_sig_den: u16::from(den),
            loop_beats: self
                .transport
                .loop_region()
                .map(|l| (l.start.0 as f64 / ppq, l.end.0 as f64 / ppq)),
        }
    }

    /// Give a slot the native unit its track now needs (`kind`), or none. A unit already of
    /// the right kind stays, so voices ring across snapshots; otherwise the one delivered
    /// for this track and kind is taken from the incoming list and the old one goes back.
    fn swap_native(
        &mut self,
        slot: usize,
        track: TrackId,
        kind: Option<cantus_core::InstrumentKind>,
    ) {
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

    fn dispose_instrument(&mut self, unit: Box<NativeUnit>) {
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
        let master_sign = if master_invert { -1.0f32 } else { 1.0 };

        let mut done = 0usize;
        let mut guard = 0usize;
        while done < frames {
            guard += 1;
            if guard > MAX_BLOCK_FRAMES * 2 {
                break; // Impossible unless the loop region is degenerate; never spin.
            }
            let mut seg_end = frames;
            let mut wrap_at_seg_end = false;

            if self.transport.is_playing() {
                if let Some(snap) = self.snapshot.clone() {
                    // Fire everything due now; find the next boundary.
                    for (ti, t) in snap
                        .tracks
                        .iter()
                        .enumerate()
                        .take(self.track_to_slot.len())
                    {
                        let slot = self.track_to_slot[ti];
                        loop {
                            let s = &mut self.slots[slot];
                            let Some(ev) = t.events.get(s.next_event) else {
                                break;
                            };
                            let off = self.transport.samples_until(ev.tick.0 as f64) as usize;
                            if off == 0 {
                                fire(s, ev);
                                s.next_event += 1;
                                if self.listened[slot] {
                                    let at = self.clock + done as u64;
                                    self.notes_to_fx(slot, at, fx_note(ev));
                                }
                            } else {
                                seg_end = seg_end.min(done + off);
                                break;
                            }
                        }
                    }
                    // Metronome beat boundary.
                    if self.metronome {
                        let off = self.transport.samples_until(self.next_beat_tick) as usize;
                        if off == 0 {
                            let beat_len = snap.time_signature.ticks_per_beat().0 as f64;
                            let bar_len = snap.time_signature.ticks_per_bar().0 as f64;
                            let in_bar = self.next_beat_tick.rem_euclid(bar_len);
                            if self.click_allowed() {
                                self.click.trigger(in_bar.abs() < 0.5);
                            }
                            self.next_beat_tick += beat_len;
                        } else {
                            seg_end = seg_end.min(done + off);
                        }
                    }
                    // Loop end boundary.
                    if let Some(off) = self.transport.samples_until_loop_end() {
                        let off = off as usize;
                        if done + off <= seg_end {
                            seg_end = done + off;
                            wrap_at_seg_end = true;
                        }
                    }
                }
            }
            if seg_end <= done {
                // Only possible for a loop boundary that is due right now.
                if wrap_at_seg_end {
                    self.note_at = self.clock + done as u64;
                    self.wrap_loop();
                }
                continue;
            }

            // Render [done, seg_end).
            let n = seg_end - done;
            let out = &mut output[done * channels..seg_end * channels];
            // Strips start each segment empty, and no key has been written yet.
            self.key_valid = [false; MAX_STRIPS];
            for slot in &self.strip_play_order {
                let st = &mut self.strips[*slot];
                st.active = false;
                st.buf_l[..n].iter_mut().for_each(|x| *x = 0.0);
                st.buf_r[..n].iter_mut().for_each(|x| *x = 0.0);
            }

            // Armed tracks' input into their recording rings (ADR 0025).
            self.record_segment(done, n);

            // ---- tracks: instrument → channel gain and balance → their strip -----------
            for ti in 0..self.track_to_slot.len() {
                let slot = self.track_to_slot[ti];
                let s = &mut self.slots[slot];
                let mono = &mut self.mono[..n];
                let l = &mut self.buf_l[..n];
                let r = &mut self.buf_r[..n];
                // Firmus is mono and is copied into both channels; Pulsus writes stereo
                // directly, because its pads are panned inside the instrument.
                let has_signal = match s.instrument {
                    SlotInstrument::Firmus => {
                        mono.iter_mut().for_each(|x| *x = 0.0);
                        let sig = s.synth.process_add(mono);
                        if sig {
                            l.copy_from_slice(mono);
                            r.copy_from_slice(mono);
                        }
                        sig
                    }
                    SlotInstrument::Pulsus => {
                        l.iter_mut().for_each(|x| *x = 0.0);
                        r.iter_mut().for_each(|x| *x = 0.0);
                        s.drums.process_add(l, r)
                    }
                    SlotInstrument::Native => {
                        l.iter_mut().for_each(|x| *x = 0.0);
                        r.iter_mut().for_each(|x| *x = 0.0);
                        match s.native.as_mut() {
                            Some(u) => {
                                let sig = u.unit.process(l, r);
                                if self.probed == s.id {
                                    self.meters.inst = u.unit.telemetry();
                                }
                                sig
                            }
                            None => false,
                        }
                    }
                };
                // Audio clips and the monitored input join the instrument's output, before
                // the channel's level (ADR 0025 §3, §4).
                let has_signal = self.live.add_track_audio(
                    self.snapshot.as_deref().and_then(|x| x.tracks.get(ti)),
                    slot,
                    has_signal,
                    done,
                    l,
                    r,
                    &self.transport,
                    self.recording,
                );
                let line = self.track_line[slot];
                let line_ringing = line.is_some_and(|i| self.pdc.lines[i].ringing());
                if !has_signal && !line_ringing {
                    continue; // Idle: nothing to mix, nothing to meter.
                }
                if !has_signal {
                    // Idle, but its compensation line still holds what it played.
                    l.iter_mut().for_each(|x| *x = 0.0);
                    r.iter_mut().for_each(|x| *x = 0.0);
                }
                // The channel's own level and balance — not the pan law, which is
                // applied once, at the strip. At their defaults these are exactly unity.
                let (bl, br) = balance_gains(s.pan);
                let g = if s.audible { s.gain } else { 0.0 };
                let (mut gl, mut gr) = (g * bl, g * br);
                if let Some(tap) = self.stem_tap.as_mut() {
                    tap.track(slot, s.strip.is_none(), done, l, r, gl, gr);
                }
                // A delayed edge: the level and balance first, then the line; what comes
                // out is added at unity (a multiplication by 1.0 changes nothing).
                if let Some(li) = line {
                    for i in 0..n {
                        l[i] *= gl;
                        r[i] *= gr;
                    }
                    self.pdc.lines[li].run(l, r);
                    (gl, gr) = (1.0, 1.0);
                }
                let mut peak = s.peak;
                match s.strip {
                    Some(st) => {
                        let strip = &mut self.strips[st];
                        strip.active = true;
                        for i in 0..n {
                            let sl = l[i] * gl;
                            let sr = r[i] * gr;
                            peak = peak.max(sl.abs()).max(sr.abs());
                            strip.buf_l[i] += sl;
                            strip.buf_r[i] += sr;
                        }
                    }
                    None => {
                        for i in 0..n {
                            let sl = l[i] * gl;
                            let sr = r[i] * gr;
                            peak = peak.max(sl.abs()).max(sr.abs());
                            let frame = &mut out[i * channels..(i + 1) * channels];
                            frame[0] += sl;
                            if channels > 1 {
                                frame[1] += sr;
                            } else {
                                frame[0] += sr;
                            }
                        }
                    }
                }
                s.peak = peak;
            }

            // ---- strips, in dependency order: polarity → swap → inserts → EQ → width →
            // gain/pan → output and sends ---------------------------------------------
            let mut strip_captured = false;
            // Where the song is at this segment's first sample, for hosted units.
            let position = self.song_position();
            for oi in 0..self.strip_play_order.len() {
                let slot = self.strip_play_order[oi];
                let (active, ringing, output_slot) = {
                    let st = &self.strips[slot];
                    // An EQ with signal still in it has to keep running over silence,
                    // or its state would freeze at a different point for every block
                    // size and the next note would meet a different filter. Quiet
                    // sections are exactly zero (denormals are flushed), so skipping
                    // them changes nothing.
                    let eq_ringing = !st.eq_flat && !(st.eq_l.is_quiet() && st.eq_r.is_quiet());
                    (st.active, !st.effects.is_empty() || eq_ringing, st.output)
                };
                if !active && !ringing && !self.strip_lines_ringing(slot) {
                    continue; // Nothing came in and nothing is ringing.
                }
                // Work in the shared scratch so the strip's own buffer can be handed to
                // the strip it feeds without the borrow checker having to see two strips
                // at once.
                let l = &mut self.buf_l[..n];
                let r = &mut self.buf_r[..n];
                let dry_l = &mut self.dry_l[..n];
                let dry_r = &mut self.dry_r[..n];
                let (gl, gr, muted) = {
                    let st = &mut self.strips[slot];
                    l.copy_from_slice(&st.buf_l[..n]);
                    r.copy_from_slice(&st.buf_r[..n]);
                    // Polarity first: the console's Ø is an input-stage switch, so the
                    // whole channel — inserts included — comes out inverted.
                    if st.phase_invert {
                        for (a, b) in l.iter_mut().zip(r.iter_mut()) {
                            *a = -*a;
                            *b = -*b;
                        }
                    }
                    // Swap is the other input-stage switch: the channel arrives mirrored,
                    // and a ping-pong delay behind it starts on the other side.
                    if st.swap {
                        for (a, b) in l.iter_mut().zip(r.iter_mut()) {
                            std::mem::swap(a, b);
                        }
                    }
                    for (i, e) in st.effects.iter_mut().enumerate() {
                        let looked_at = self.analysed == Some(e.id);
                        if looked_at {
                            for k in 0..n {
                                self.ana_pre[k] = (l[k] + r[k]) * 0.5;
                            }
                        }
                        // Notes from the track it listens to, due in this segment,
                        // at their sample in it (ADR 0022) — delivered even to a
                        // bypassed effect, so it knows what is held when it is back.
                        if !self.fx_notes.is_empty() {
                            let seg0 = self.clock + done as u64;
                            let seg1 = self.clock + seg_end as u64;
                            let id = e.id;
                            self.fx_notes.retain(|d| {
                                if d.effect == id && d.due < seg1 {
                                    e.note(d.due.saturating_sub(seg0) as usize, d.note);
                                    false
                                } else {
                                    true
                                }
                            });
                        }
                        // Bypassed, or the whole chain switched off. An insert with
                        // no latency is then a wire and does not run; a latent one is
                        // a *soft* bypass (ADR 0022): it keeps running, so its state
                        // follows the signal, and what leaves the slot is its dry
                        // line — the input delayed by exactly its latency.
                        let off = st.bypass[i] || !st.fx_enabled;
                        let latent = e.latency() > 0;
                        if off && !latent {
                            if looked_at {
                                Self::analyse(&mut self.analyser, &self.ana_pre[..n], l, r);
                            }
                            continue;
                        }
                        let mix = st.mix[i];
                        // A keyed effect hears the key strip's post-fader output —
                        // or silence, if that strip made none this segment.
                        let key = st.key[i].map(|k| {
                            if self.key_valid[k] {
                                (&self.key_bufs[k].0[..n], &self.key_bufs[k].1[..n])
                            } else {
                                (&self.silence[..n], &self.silence[..n])
                            }
                        });
                        // A unit that follows the song (a hosted plug-in) hears where it
                        // is before each call.
                        e.set_position(&position);
                        if mix >= 1.0 && !latent {
                            e.process_keyed(l, r, key);
                        } else {
                            // The slot's dry/wet. The effect runs even at 0 %, so
                            // its delay line or tail is where it would be when the
                            // knob comes back up; a latent effect's dry line runs
                            // at every setting, so it is in step whenever it is
                            // needed.
                            dry_l.copy_from_slice(l);
                            dry_r.copy_from_slice(r);
                            if let Some(li) = self.dry_line[slot][i] {
                                self.pdc.lines[li].run(dry_l, dry_r);
                            }
                            e.process_keyed(l, r, key);
                            if off {
                                l.copy_from_slice(dry_l);
                                r.copy_from_slice(dry_r);
                            } else if mix < 1.0 {
                                let dry = 1.0 - mix;
                                for i in 0..n {
                                    l[i] = l[i] * mix + dry_l[i] * dry;
                                    r[i] = r[i] * mix + dry_r[i] * dry;
                                }
                            }
                        }
                        // A hosted plug-in's own window: its gestures go to the host.
                        let effect = e.id;
                        e.drain_param_events(|event| {
                            if self
                                .tx
                                .push(EngineEvent::FxParam { effect, event })
                                .is_err()
                            {
                                self.stats.dropped_events += 1;
                            }
                        });
                        if off {
                            if looked_at {
                                Self::analyse(&mut self.analyser, &self.ana_pre[..n], l, r);
                            }
                            continue;
                        }
                        if looked_at {
                            Self::analyse(&mut self.analyser, &self.ana_pre[..n], l, r);
                            if let Some(d) = e.eq_deltas() {
                                self.meters.eq_deltas = d;
                            }
                            if let Some(c) = e.comp_meter() {
                                self.meters.comp = [c.input, c.output, c.gr_db];
                            }
                            if let Some(t) = e.telemetry() {
                                self.meters.fx = t;
                            }
                        }
                    }
                    // The strip's own EQ, after the inserts and before the channel is
                    // placed — where a console's channel EQ sits.
                    if !st.eq_flat {
                        st.eq_l.process(l);
                        st.eq_r.process(r);
                    }
                    // Width, then pan: narrowing a stereo reverb has to happen before
                    // the channel is placed, or the placement is what gets narrowed.
                    apply_width(l, r, st.width);
                    let (pl, pr) = law.gains(st.pan);
                    (st.gain * pl, st.gain * pr, st.muted)
                };
                let mut peak = 0f32;
                let (gl, gr) = if muted { (0.0, 0.0) } else { (gl, gr) };
                if let Some(tap) = self.stem_tap.as_mut() {
                    tap.strip(slot, done, l, r, gl, gr);
                }
                // A strip something is keyed from keeps what leaves its fader.
                if self.strips[slot].is_key {
                    let (kl, kr) = &mut self.key_bufs[slot];
                    for i in 0..n {
                        kl[i] = l[i] * gl;
                        kr[i] = r[i] * gr;
                    }
                    self.key_valid[slot] = true;
                }
                // The recorder, on a strip: exactly what leaves the fader — delayed, in a
                // project with latency, by the strip's path to the master, so the take
                // is in time with what is heard (ADR 0022).
                if self.capture_slot == Some(slot) {
                    strip_captured = true;
                    if let Some(li) = self.capture_line {
                        for i in 0..n {
                            self.edge_l[i] = l[i] * gl;
                            self.edge_r[i] = r[i] * gr;
                        }
                        self.pdc.lines[li].run(&mut self.edge_l[..n], &mut self.edge_r[..n]);
                        for i in 0..n {
                            Self::capture_frame(
                                &mut self.capture,
                                &mut self.stats,
                                self.edge_l[i],
                                self.edge_r[i],
                            );
                        }
                    } else {
                        for i in 0..n {
                            Self::capture_frame(
                                &mut self.capture,
                                &mut self.stats,
                                l[i] * gl,
                                r[i] * gr,
                            );
                        }
                    }
                }
                // A delayed output edge goes through its line first, at unity after.
                let (src_l, src_r, ogl, ogr): (&[f32], &[f32], f32, f32) = match self.out_line[slot]
                {
                    Some(li) => {
                        for i in 0..n {
                            self.edge_l[i] = l[i] * gl;
                            self.edge_r[i] = r[i] * gr;
                        }
                        self.pdc.lines[li].run(&mut self.edge_l[..n], &mut self.edge_r[..n]);
                        (&self.edge_l[..n], &self.edge_r[..n], 1.0, 1.0)
                    }
                    None => (&*l, &*r, gl, gr),
                };
                match output_slot {
                    Some(dest) => {
                        let d = &mut self.strips[dest];
                        d.active = true;
                        for i in 0..n {
                            let sl = src_l[i] * ogl;
                            let sr = src_r[i] * ogr;
                            peak = peak.max(sl.abs()).max(sr.abs());
                            d.buf_l[i] += sl;
                            d.buf_r[i] += sr;
                        }
                    }
                    None => {
                        for i in 0..n {
                            let sl = src_l[i] * ogl;
                            let sr = src_r[i] * ogr;
                            peak = peak.max(sl.abs()).max(sr.abs());
                            let frame = &mut out[i * channels..(i + 1) * channels];
                            frame[0] += sl;
                            if channels > 1 {
                                frame[1] += sr;
                            } else {
                                frame[0] += sr;
                            }
                        }
                    }
                }
                // Post-fader sends: the same signal that just left the fader, at each
                // send's level, into the strip it names. A muted strip sends nothing.
                let (send_count, sends) = {
                    let st = &mut self.strips[slot];
                    st.peak = st.peak.max(peak);
                    (st.send_count, st.sends)
                };
                if !muted || self.send_line[slot].iter().any(|x| x.is_some()) {
                    for (k, (dest, level)) in sends.iter().take(send_count).enumerate() {
                        let (sl, sr) = (gl * level, gr * level);
                        if let Some(li) = self.send_line[slot][k] {
                            // A delayed send: through its line, then added at unity. A
                            // muted strip feeds its line silence, so the line drains.
                            for i in 0..n {
                                self.edge_l[i] = l[i] * sl;
                                self.edge_r[i] = r[i] * sr;
                            }
                            self.pdc.lines[li].run(&mut self.edge_l[..n], &mut self.edge_r[..n]);
                            let d = &mut self.strips[*dest];
                            d.active = true;
                            for i in 0..n {
                                d.buf_l[i] += self.edge_l[i];
                                d.buf_r[i] += self.edge_r[i];
                            }
                            continue;
                        }
                        if muted {
                            continue;
                        }
                        let d = &mut self.strips[*dest];
                        d.active = true;
                        for i in 0..n {
                            d.buf_l[i] += l[i] * sl;
                            d.buf_r[i] += r[i] * sr;
                        }
                    }
                }
            }
            // A captured strip that was skipped this segment — nothing came in, nothing
            // rings — still records its silence: a take must keep time.
            if matches!(self.capture_target, CaptureTarget::Strip(_)) && !strip_captured {
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
            // The master's insert chain, on the summed mix, before its fader (ADR 0026).
            self.run_master(out, channels, n, done, seg_end, &position);
            // Master strip: width, then pan and gain, then polarity last of all.
            if channels > 1 && master_width != 1.0 {
                for i in 0..n {
                    let frame = &mut out[i * channels..(i + 1) * channels];
                    let (mut l, mut r) = (frame[0], frame[1]);
                    let (mid, side) = ((l + r) * 0.5, (l - r) * 0.5 * master_width.clamp(0.0, 2.0));
                    l = mid + side;
                    r = mid - side;
                    frame[0] = l;
                    frame[1] = r;
                }
            }
            for i in 0..n {
                let frame = &mut out[i * channels..(i + 1) * channels];
                for (ch, s) in frame.iter_mut().enumerate() {
                    let pan = match ch {
                        0 => master_l,
                        1 => master_r,
                        _ => 1.0,
                    };
                    *s *= master_gain * pan * master_sign;
                    if ch < 2 {
                        master_peak[ch] = master_peak[ch].max(s.abs());
                    }
                }
            }
            // The recorder, on the master: the mix as it leaves the master strip.
            if self.capture_target == CaptureTarget::Master {
                for i in 0..n {
                    let frame = &out[i * channels..(i + 1) * channels];
                    let (l, r) = (frame[0], if channels > 1 { frame[1] } else { frame[0] });
                    Self::capture_frame(&mut self.capture, &mut self.stats, l, r);
                }
            }
            // The click goes in *after* the master strip: it is for the player, so the
            // master fader does not turn it down and the recorder never hears it. With
            // latency it goes through a line of the output's latency, so it comes out
            // with the beat it marks (ADR 0022).
            let click_line = self
                .click_line
                .filter(|li| self.click.is_active() || self.pdc.lines[*li].ringing());
            if let Some(li) = click_line {
                let active = self.click.is_active();
                for i in 0..n {
                    let c = if active { self.click.next() } else { 0.0 };
                    self.edge_l[i] = c;
                    self.edge_r[i] = c;
                }
                self.pdc.lines[li].run(&mut self.edge_l[..n], &mut self.edge_r[..n]);
                for i in 0..n {
                    let frame = &mut out[i * channels..(i + 1) * channels];
                    frame[0] += self.edge_l[i];
                    if channels > 1 {
                        frame[1] += self.edge_r[i];
                    }
                }
            } else if self.click.is_active() && self.click_line.is_none() {
                for i in 0..n {
                    let c = self.click.next();
                    let frame = &mut out[i * channels..(i + 1) * channels];
                    frame[0] += c;
                    if channels > 1 {
                        frame[1] += c;
                    }
                }
            }

            done = seg_end;
            if self.transport.is_playing() {
                self.transport.advance(n as u64);
                self.played = self.played.saturating_add(n as u64);
                if wrap_at_seg_end {
                    self.note_at = self.clock + done as u64;
                    self.wrap_loop();
                }
            }
        }
        self.clock += done as u64;
        // Notes for effects that are gone (removed while the note waited) never find
        // their effect; what is past is dropped.
        if !self.fx_notes.is_empty() {
            let now = self.clock;
            self.fx_notes.retain(|d| d.due >= now);
        }

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
        InstrumentSpec::Native(_) => SlotInstrument::Native,
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

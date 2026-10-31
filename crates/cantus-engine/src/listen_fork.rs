//! A processor's copy, state and all (round 34, ADR 0083): what a range render resumes from.
//!
//! A checkpoint is the whole engine paused between two blocks: every unit's state (filters,
//! delay lines, voices, envelopes), the transport, the automation's and the modulators'
//! state, delay compensation's lines, the notes on their way to the inserts. [`Processor::
//! fork`] copies all of it into a processor of its own — fresh rings, a pool of its own —
//! that plays on from that sample exactly as the original would, to the bit (the test in
//! `listen.rs` holds it against a whole render).
//!
//! What cannot be copied makes the fork fail (`None`), and the range render pre-rolls
//! instead, saying so: a hosted plug-in's state belongs to the plug-in, a frozen render's
//! reader and a voice streaming from disk share a board with the reader thread, a sync
//! port is a device.
//!
//! **Only the cells in use** (review 3: a fork of the whole engine was 75 MB whatever the
//! song — 256 track cells of 180 KB each, a drum machine's pads in every one, and 256
//! strips): [`Cells`] names the track and strip slots a processor's lineage has ever been
//! given; every other cell is as a fresh processor makes it, so a copy leaves it fresh. A
//! checkpoint keeps an *image* with only those cells (`Processor::with_cells`), never played,
//! and a render resumes from a full processor copied from it.
//!
//! What is *not* copied, on purpose: the pool, its clustering and the per-pass scratch
//! (scheduling only — parallel equals serial, ADR 0031), the rings to the UI (the analyser,
//! the scopes, the mix recorder: an offline render has none) and the profiler's stamps.

use super::*;

/// A track slot's copy; `None` when its instrument or a frozen render cannot be copied.
fn fork_track(s: &TrackSlot) -> Option<TrackSlot> {
    if s.frozen.is_some() || s.parked.is_some() {
        return None;
    }
    let native = match s.native.as_ref() {
        Some(n) => Some(Box::new(n.fork()?)),
        None => None,
    };
    Some(TrackSlot {
        id: s.id,
        instrument: s.instrument,
        synth: s.synth.clone(),
        drums: s.drums.clone(),
        native,
        next_event: s.next_event,
        next_control: s.next_control,
        next_expr: s.next_expr,
        gain: s.gain,
        pan: s.pan,
        audible: s.audible,
        strip: s.strip,
        peak: s.peak,
        peak_block: s.peak_block,
        listened: s.listened,
        line: s.line,
        input: s.input,
        gain_ramp: s.gain_ramp,
        pan_ramp: s.pan_ramp,
        frozen: None,
        parked: None,
        frozen_place: s.frozen_place,
        rest: s.rest,
        notes_only: s.notes_only,
        lateness: s.lateness,
        notefx: s.notefx.clone(),
        source_line: s.source_line,
    })
}

/// A strip slot's copy; `None` when an insert or a frozen render cannot be copied.
fn fork_strip(s: &StripSlot) -> Option<StripSlot> {
    if s.frozen.is_some() || s.parked.is_some() {
        return None;
    }
    let mut effects = Vec::with_capacity(s.effects.capacity());
    for e in &s.effects {
        effects.push(Box::new(e.fork()?));
    }
    Some(StripSlot {
        id: s.id,
        effects,
        bypass: s.bypass,
        mix: s.mix,
        fx_enabled: s.fx_enabled,
        eq_l: s.eq_l.clone(),
        eq_r: s.eq_r.clone(),
        eq_flat: s.eq_flat,
        gain: s.gain,
        pan: s.pan,
        width: s.width,
        phase_invert: s.phase_invert,
        swap: s.swap,
        muted: s.muted,
        output: s.output,
        sends: s.sends,
        send_tap: s.send_tap,
        send_count: s.send_count,
        has_pre: s.has_pre,
        key_id: s.key_id,
        key: s.key,
        key_before: s.key_before,
        midi_id: s.midi_id,
        listener: s.listener,
        is_key: s.is_key,
        active: s.active,
        peak: s.peak,
        peak_block: s.peak_block,
        out_line: s.out_line,
        send_line: s.send_line,
        dry_line: s.dry_line,
        gain_ramp: s.gain_ramp,
        pan_ramp: s.pan_ramp,
        follow_peak: s.follow_peak,
        frozen: None,
        parked: None,
        frozen_place: s.frozen_place,
        rest: s.rest,
        ll_off: s.ll_off,
    })
}

/// A vector's copy with its capacity kept: the audio thread pushes into these without
/// growing them (ADR 0005).
fn with_room<T: Clone>(v: &Vec<T>) -> Vec<T> {
    let mut out = Vec::with_capacity(v.capacity());
    out.extend_from_slice(v);
    out
}

/// Copy one cell into another of the same kind (both free: between blocks).
fn copy_cell<T, U>(
    from: Option<cantus_rt::cell::NodeMut<'_, T>>,
    to: Option<cantus_rt::cell::NodeMut<'_, T>>,
    f: impl FnOnce(&T) -> Option<U>,
    put: impl FnOnce(&mut T, U),
) -> Option<()> {
    let (from, mut to) = (from?, to?);
    let v = f(&from)?;
    put(&mut to, v);
    Some(())
}

/// The track and strip slots some snapshot was installed into, in a processor's whole
/// lineage (its forks carry it): the cells that may differ from a fresh processor's. A slot
/// once given and freed stays named — it keeps what its track left (review 3).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Cells {
    tracks: Box<[bool]>,
    strips: Box<[bool]>,
}

impl Default for Cells {
    fn default() -> Self {
        Cells {
            tracks: vec![false; MAX_TRACKS].into_boxed_slice(),
            strips: vec![false; MAX_STRIPS].into_boxed_slice(),
        }
    }
}

impl Cells {
    /// Every slot: what a copy of an unknown lineage must take.
    pub(crate) fn all() -> Self {
        Cells {
            tracks: vec![true; MAX_TRACKS].into_boxed_slice(),
            strips: vec![true; MAX_STRIPS].into_boxed_slice(),
        }
    }

    /// Add the slots `p` has given its tracks and strips now (after an install).
    pub(crate) fn note(&mut self, p: &Processor) {
        for &i in &p.track_to_slot {
            if let Some(t) = self.tracks.get_mut(i) {
                *t = true;
            }
        }
        for &i in &p.strip_to_slot {
            if let Some(t) = self.strips.get_mut(i) {
                *t = true;
            }
        }
    }

    /// How many track and strip cells an image holding these needs (one past the last).
    pub(crate) fn room(&self) -> (usize, usize) {
        let last = |v: &[bool]| v.iter().rposition(|x| *x).map_or(0, |i| i + 1);
        (last(&self.tracks), last(&self.strips))
    }
}

/// Why a processor could not be copied, and whether that lasts (a hosted plug-in, a frozen
/// render) or was a moment's (a snapshot waiting for the grid, a busy node).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Refusal {
    pub why: &'static str,
    pub lasting: bool,
}

impl Processor {
    /// A copy of this processor that plays on from this sample exactly as this one would
    /// (see the module's notes), with its own message and event rings: the producer to
    /// send it messages through and the consumer of what it sends back. `None` when some
    /// state cannot be copied — a hosted plug-in, a frozen render, a voice streaming from
    /// disk, a sync port — or a snapshot is waiting for the control grid. Allocates: never
    /// on the audio thread; the processor must not be inside `process`.
    pub fn fork(
        &self,
        message_room: usize,
        event_room: usize,
    ) -> Option<(Processor, Producer<EngineMessage>, Consumer<EngineEvent>)> {
        self.fork_cells(message_room, event_room, &Cells::all(), false)
    }

    /// [`Self::fork`] of only the slots `cells` names (every other cell of this processor
    /// must be as a fresh one made it — see [`Cells`]): into a full processor that plays, or
    /// — `image` — into one holding only those cells, kept to be copied from again.
    pub(crate) fn fork_cells(
        &self,
        message_room: usize,
        event_room: usize,
        cells: &Cells,
        image: bool,
    ) -> Option<(Processor, Producer<EngineMessage>, Consumer<EngineEvent>)> {
        if self.deferred.is_some() || self.sync.is_some() || self.record_rings.is_some() {
            return None;
        }
        every_field_is_accounted_for(self);
        // Everything that can fail first, so a failure leaves nothing half built.
        let mut tracks = Vec::new();
        for (i, _) in cells.tracks.iter().enumerate().filter(|c| *c.1) {
            let s = self.nodes.tracks.get(i)?.state.try_borrow_mut()?;
            tracks.push((i, fork_track(&s)?));
        }
        let mut strips = Vec::new();
        for (i, _) in cells.strips.iter().enumerate().filter(|c| *c.1) {
            let s = self.nodes.strips.get(i)?.state.try_borrow_mut()?;
            strips.push((i, fork_strip(&s)?));
        }
        let master = self.master.fork()?;
        let mut incoming_effects = Vec::with_capacity(self.incoming_effects.capacity());
        for e in &self.incoming_effects {
            incoming_effects.push(Box::new(e.fork()?));
        }
        let mut incoming_instruments = Vec::with_capacity(self.incoming_instruments.capacity());
        for u in &self.incoming_instruments {
            incoming_instruments.push(Box::new(u.fork()?));
        }
        let tap = self.nodes.tap.try_borrow_mut()?.as_ref().cloned();

        let (tx, rx_engine) = RingBuffer::<EngineMessage>::new(message_room.max(1));
        let (tx_engine, rx) = RingBuffer::<EngineEvent>::new(event_room.max(1));
        let (n_tracks, n_strips) = if image {
            cells.room()
        } else {
            (MAX_TRACKS, MAX_STRIPS)
        };
        let (_midi, midi_rx) = RingBuffer::<MidiEvent>::new(1);
        let mut p = Processor::with_cells(
            self.sample_rate,
            rx_engine,
            tx_engine,
            midi_rx,
            n_tracks,
            n_strips,
        );
        p.copy_state_from(self, master, incoming_effects, incoming_instruments)?;
        p.copy_nodes_from(self, tracks, strips, tap, image)?;
        Some((p, tx, rx))
    }

    /// About how many bytes this processor holds — a checkpoint's image, for the
    /// checkpoints' budget (review 3). Its cells and its delay lines as they are, and the
    /// fixed part and each unit's state as measured at 48 kHz, four of a kind imaged sixteen
    /// times (`listen::tests::what_each_unit_adds_to_an_image`, the lines it grows taken
    /// off), scaled to the rate.
    pub(crate) fn approx_bytes(&self) -> usize {
        use cantus_core::{EffectKind as E, InstrumentKind as I};
        const KB: f64 = 1024.0;
        const FIXED: f64 = 4_700.0 * KB;
        const TRACK_CELL: usize = 192 << 10;
        const STRIP_CELL: usize = 36 << 10;
        let effect = |k: E| match k {
            E::Space | E::Maximizer => 2_000.0,
            E::Multiband => 1_050.0,
            E::Delay => 900.0,
            E::Reverb | E::TransientSplit => 250.0,
            _ => 200.0,
        };
        let instrument = |k: I| match k {
            I::Chorda => 5_150.0,
            I::Sampler => 4_300.0,
            I::Wavetable => 1_650.0,
            I::ElectricPiano => 650.0,
            I::Analog => 600.0,
            I::Organ => 550.0,
            I::Fm => 500.0,
            I::Bass => 400.0,
        };
        let n = &self.nodes;
        let lines: usize = (0..n.lines.len())
            .filter_map(|i| n.lines.get(i))
            .map(|l| l.len() * 2 * std::mem::size_of::<f32>())
            .sum();
        let mut units: f64 = self.master.effects.iter().map(|e| effect(e.kind)).sum();
        for c in n.tracks.iter() {
            if let Some(s) = c.state.try_borrow_mut() {
                units += match s.native.as_ref().map(|u| u.kind) {
                    Some(UnitKind::Native(k)) => instrument(k),
                    Some(UnitKind::Hosted(_)) => 1_024.0,
                    None => 0.0,
                };
            }
        }
        for c in n.strips.iter() {
            if let Some(s) = c.state.try_borrow_mut() {
                units += s.effects.iter().map(|e| effect(e.kind)).sum::<f64>();
            }
        }
        let scale = (self.sample_rate / 48_000.0).max(1.0);
        (FIXED + units * KB * scale) as usize
            + n.tracks.len() * TRACK_CELL
            + n.strips.len() * STRIP_CELL
            + lines
    }

    /// Why [`Self::fork`] fails, in words (`None`: it does not) — said by a range render
    /// that pre-rolls because of it (review 1). Tries the units' copies: off the audio
    /// thread, and only once a fork has failed.
    pub fn fork_refusal(&self) -> Option<Refusal> {
        let lasting = |why| Some(Refusal { why, lasting: true });
        // A moment's (review 2): the next checkpoint may well copy.
        let passing = |why| {
            Some(Refusal {
                why,
                lasting: false,
            })
        };
        if self.deferred.is_some() {
            return passing("a snapshot was waiting for the control grid");
        }
        if self.sync.is_some() {
            return lasting("a sync port is a device");
        }
        if self.record_rings.is_some() {
            return lasting("the recorder's rings are a device's");
        }
        for c in self.nodes.tracks.iter() {
            let Some(s) = c.state.try_borrow_mut() else {
                return passing("a node was busy");
            };
            if s.frozen.is_some() || s.parked.is_some() {
                return lasting("a frozen render is read from disk");
            }
            if s.native.as_ref().is_some_and(|n| n.fork().is_none()) {
                return lasting("an instrument's state cannot be copied (a voice streaming from disk, or a hosted instrument)");
            }
        }
        for c in self.nodes.strips.iter() {
            let Some(s) = c.state.try_borrow_mut() else {
                return passing("a node was busy");
            };
            if s.frozen.is_some() || s.parked.is_some() {
                return lasting("a frozen render is read from disk");
            }
            if s.effects.iter().any(|e| e.fork().is_none()) {
                return lasting("an insert's state cannot be copied (a hosted plug-in)");
            }
        }
        if self.master.fork().is_none() {
            return lasting("an insert's state cannot be copied (a hosted plug-in)");
        }
        if self.fork(1, 1).is_none() {
            return passing("the engine could not be copied");
        }
        None
    }

    /// The processor's own fields, from `o` (see [`Self::fork`]).
    #[allow(clippy::vec_box)] // as the processor's own fields
    fn copy_state_from(
        &mut self,
        o: &Processor,
        master: master::MasterChain,
        incoming_effects: Vec<Box<EffectUnit>>,
        incoming_instruments: Vec<Box<NativeUnit>>,
    ) -> Option<()> {
        self.snapshot = o.snapshot.clone();
        self.incoming_effects = incoming_effects;
        self.incoming_instruments = incoming_instruments;
        self.probed = o.probed;
        self.transport = o.transport.clone();
        self.track_ids = o.track_ids;
        self.strip_ids = o.strip_ids;
        self.track_to_slot = with_room(&o.track_to_slot);
        self.strip_to_slot = with_room(&o.strip_to_slot);
        self.strip_play_order = with_room(&o.strip_play_order);
        self.plan = o.plan.clone();
        // The pool clusters its tasks again (scheduling only: the same bits).
        self.map_dirty = true;
        self.track_cost = o.track_cost;
        self.strip_cost = o.strip_cost;
        self.block_work = o.block_work;
        self.pool_wakes = o.pool_wakes;
        self.shuffle = o.shuffle;
        self.split_voices = o.split_voices;
        self.offline = o.offline;
        self.low_latency = o.low_latency;
        self.click = o.click.clone();
        self.preview = o.preview.clone();
        self.metronome = o.metronome;
        self.next_beat_tick = o.next_beat_tick;
        self.live_track = o.live_track;
        self.live_echo = o.live_echo;
        self.mpe = o.mpe.clone();
        self.recording = o.recording;
        self.stats = o.stats;
        self.meters = o.meters.clone();
        self.was_playing = o.was_playing;
        self.capture_target = o.capture_target;
        self.capture_slot = o.capture_slot;
        self.analysed = o.analysed;
        self.scoped = o.scoped;
        self.scoped_delay = o.scoped_delay;
        self.lates = o.lates.clone();
        self.late_n = o.late_n;
        self.follow_lates = o.follow_lates.clone();
        self.follow_n = o.follow_n;
        // The planner's state, on this processor's own lines (shared with its nodes).
        let lines = self.pdc.lines.clone();
        self.pdc = o.pdc.clone();
        self.pdc.lines = lines;
        self.master_loud = o.master_loud.clone();
        self.strip_post = o.strip_post;
        self.strip_arrive = o.strip_arrive;
        self.arrive_floor = o.arrive_floor;
        self.master_arrive = o.master_arrive;
        self.latency_sig = o.latency_sig;
        self.click_line = o.click_line;
        self.capture_line = o.capture_line;
        self.played = o.played;
        self.listener_count = o.listener_count;
        self.listeners_prev = with_room(&o.listeners_prev);
        self.listened = o.listened;
        self.clock = o.clock;
        self.note_at = o.note_at;
        self.block = o.block;
        self.pass = o.pass;
        self.last_parallel = o.last_parallel;
        self.flush_denormals = o.flush_denormals;
        self.live = o.live.clone();
        self.master = master;
        self.auto = o.auto.clone();
        self.prof.set_on(o.prof.is_on());
        Some(())
    }

    /// Every node's cells, from `o`'s (see [`Self::fork`]).
    fn copy_nodes_from(
        &mut self,
        o: &Processor,
        tracks: Vec<(usize, TrackSlot)>,
        strips: Vec<(usize, StripSlot)>,
        tap: Option<Box<crate::stems::StemTap>>,
        image: bool,
    ) -> Option<()> {
        use std::sync::atomic::Ordering::Relaxed;
        let (a, b) = (&o.nodes, &self.nodes);
        for (i, t) in tracks {
            let (x, y) = (a.tracks.get(i)?, b.tracks.get(i)?);
            *y.state.try_borrow_mut()? = t;
            *y.out.try_borrow_mut()? = (*x.out.try_borrow()?).clone();
            *y.outbox.try_borrow_mut()? = (*x.outbox.try_borrow()?).clone();
        }
        for (i, s) in strips {
            let (x, y) = (a.strips.get(i)?, b.strips.get(i)?);
            *y.state.try_borrow_mut()? = s;
            *y.out.try_borrow_mut()? = (*x.out.try_borrow()?).clone();
            *y.key.try_borrow_mut()? = (*x.key.try_borrow()?).clone();
        }
        // A fresh line is left out of an image (an empty one stands for it) and stays fresh
        // in a copy from one: 64 KB each, 192 of them (review 3).
        for i in 0..a.lines.len() {
            let (from, mut to) = (a.lines.get(i)?, b.lines.get(i)?);
            if image && from.is_fresh() {
                *to = crate::pdc::DelayLine::with_len(0);
            } else if !from.is_empty() {
                *to = from.clone();
            }
        }
        for (x, y) in a.edges.iter().zip(b.edges.iter()) {
            *y.try_borrow_mut()? = (*x.try_borrow()?).clone();
        }
        for (x, y) in a.listeners.iter().zip(b.listeners.iter()) {
            copy_cell(
                x.try_borrow_mut(),
                y.try_borrow_mut(),
                |l| Some(*l),
                |to, v| *to = v,
            )?;
        }
        *b.input.try_borrow_mut()? = (*a.input.try_borrow()?).clone();
        // The pass's context is written before every pass but for the snapshot the nodes
        // read, which an install sets.
        b.frame.try_borrow_mut()?.ctx.snapshot = a.frame.try_borrow()?.ctx.snapshot.clone();
        *b.probe.try_borrow_mut()? = *a.probe.try_borrow_mut()?;
        *b.tap.try_borrow_mut()? = tap;
        let atoms = |x: &[std::sync::atomic::AtomicU32], y: &[std::sync::atomic::AtomicU32]| {
            for (x, y) in x.iter().zip(y) {
                y.store(x.load(Relaxed), Relaxed);
            }
        };
        atoms(&a.track_peak, &b.track_peak);
        atoms(&a.track_voices, &b.track_voices);
        atoms(&a.strip_peak, &b.strip_peak);
        atoms(&a.strip_gr, &b.strip_gr);
        for (x, y) in a.track_splits.iter().zip(b.track_splits.iter()) {
            y.store(x.load(Relaxed), Relaxed);
        }
        for (x, y) in [
            (&a.conflicts, &b.conflicts),
            (&a.dropped, &b.dropped),
            (&a.stream_late, &b.stream_late),
            (&a.stream_cut, &b.stream_cut),
            (&a.stream_wakes, &b.stream_wakes),
        ] {
            y.store(x.load(Relaxed), Relaxed);
        }
        Some(())
    }
}

/// Every field of the processor, of its nodes and of a track's and a strip's cells, by
/// name, with no `..`: a field added to
/// either fails to compile here until the fork says what becomes of it (review 1 — the
/// fork copies field by field, and a forgotten one would have compiled). Each is copied
/// above, or left fresh on purpose (the rings, the pool and its clustering, the per-pass
/// scratch, the UI's rings, the profiler's stamps — see the module's notes).
fn every_field_is_accounted_for(p: &Processor) {
    let Processor {
        sample_rate: _,
        rx: _,
        tx: _,
        midi_rx: _,
        snapshot: _,
        deferred: _,
        pending_garbage: _,
        incoming_effects: _,
        scratch_effects: _,
        incoming_instruments: _,
        probed: _,
        transport: _,
        nodes: _,
        pool: _,
        track_ids: _,
        strip_ids: _,
        track_to_slot: _,
        strip_to_slot: _,
        strip_play_order: _,
        plan: _,
        topo: _,
        map_dirty: _,
        clustered_at: _,
        clustered_costs: _,
        track_cost: _,
        strip_cost: _,
        node_cost: _,
        block_work: _,
        pool_wakes: _,
        shuffle: _,
        split_voices: _,
        offline: _,
        low_latency: _,
        click: _,
        preview: _,
        metronome: _,
        next_beat_tick: _,
        live_track: _,
        live_echo: _,
        mpe: _,
        recording: _,
        stats: _,
        meters: _,
        was_playing: _,
        capture_target: _,
        capture_slot: _,
        analysed: _,
        scoped: _,
        scoped_delay: _,
        lates: _,
        late_n: _,
        follow_lates: _,
        follow_n: _,
        pdc: _,
        master_loud: _,
        edge_l: _,
        edge_r: _,
        strip_post: _,
        strip_arrive: _,
        arrive_floor: _,
        master_arrive: _,
        latency_sig: _,
        click_line: _,
        capture_line: _,
        played: _,
        listener_count: _,
        listeners_prev: _,
        listened: _,
        clock: _,
        note_at: _,
        block: _,
        pass: _,
        last_parallel: _,
        promote: _,
        flush_denormals: _,
        input: _,
        record_rings: _,
        live: _,
        master: _,
        auto: _,
        prof: _,
        sync: _,
    } = p;
    let exec::Nodes {
        tracks: _,
        strips: _,
        lines: _,
        edges: _,
        listeners: _,
        frame: _,
        input: _,
        scratch: _,
        analysis: _,
        scopes: _,
        probe: _,
        capture: _,
        tap: _,
        track_peak: _,
        track_voices: _,
        track_splits: _,
        forks: _,
        strip_peak: _,
        strip_gr: _,
        timing: _,
        conflicts: _,
        dropped: _,
        stream_late: _,
        stream_cut: _,
        stream_wakes: _,
        origin: _,
    } = &*p.nodes;
    let exec::TrackCells {
        state: _,
        out: _,
        outbox: _,
    } = &p.nodes.tracks[0];
    let exec::StripCells {
        state: _,
        out: _,
        key: _,
    } = &p.nodes.strips[0];
}

#[cfg(test)]
mod tests {
    use super::*;
    use cantus_core::{Project, Snapshot, Ticks};

    fn song() -> Project {
        let mut p = Project::new("fork");
        let mut t = p.make_track("Lead");
        let mut c = p.make_clip("c", Ticks::ZERO, Ticks::beats(16));
        for i in 0..32 {
            let n = p.make_note(Ticks::EIGHTH * i, Ticks::QUARTER, 60 + (i % 7) as u8, 100);
            c.notes.push(n);
        }
        t.clips.push(c);
        p.tracks.push(t);
        p
    }

    fn playing(p: &Project) -> (Processor, Producer<EngineMessage>, Consumer<EngineEvent>) {
        let snap = Arc::new(Snapshot::compile(p, 1));
        let (mut tx, rx_e) = RingBuffer::<EngineMessage>::new(64);
        let (tx_e, rx) = RingBuffer::<EngineEvent>::new(1024);
        let mut proc_ = Processor::without_midi(48_000.0, rx_e, tx_e);
        tx.push(EngineMessage::Automation(
            crate::automation::Runtime::build(&snap),
        ))
        .ok();
        let plan = crate::graph::compile(&snap);
        tx.push(EngineMessage::SetSnapshotPlanned(snap, plan)).ok();
        proc_.settle();
        tx.push(EngineMessage::Seek(Ticks::ZERO)).ok();
        tx.push(EngineMessage::Play).ok();
        (proc_, tx, rx)
    }

    fn block(p: &mut Processor, rx: &mut Consumer<EngineEvent>, n: usize) -> Vec<f32> {
        let mut b = vec![0f32; n * 2];
        p.process(&mut b, 2);
        while rx.pop().is_ok() {}
        b
    }

    /// A copy taken between two blocks plays on exactly as the original does.
    #[test]
    fn a_fork_plays_on_as_the_original_does() {
        let p = song();
        let (mut a, _tx, mut arx) = playing(&p);
        for _ in 0..500 {
            block(&mut a, &mut arx, 64);
        }
        let (mut b, _btx, mut brx) = a.fork(64, 1024).expect("copies");
        for i in 0..400 {
            let x = block(&mut a, &mut arx, 64);
            let y = block(&mut b, &mut brx, 64);
            assert_eq!(x, y, "block {i} after the fork");
        }
    }

    /// Review 3: a checkpoint's image holds only the cells the song was given — one track
    /// here, not 256, and none of the fresh delay lines — and a processor copied back from
    /// it plays on as the original does, to the bit.
    #[test]
    fn an_image_holds_the_cells_in_use_and_plays_on_from_them_to_the_bit() {
        let p = song();
        let (mut a, _tx, mut arx) = playing(&p);
        for _ in 0..500 {
            block(&mut a, &mut arx, 64);
        }
        let mut cells = Cells::default();
        cells.note(&a);
        let (image, _itx, _irx) = a.fork_cells(1, 1, &cells, true).expect("an image");
        assert_eq!(image.nodes.tracks.len(), cells.room().0);
        assert!(
            image.nodes.tracks.len() <= 1,
            "{}",
            image.nodes.tracks.len()
        );
        // The new project's sixteen mixer strips, of 256.
        assert!(
            image.nodes.strips.len() <= cantus_core::DEFAULT_STRIPS,
            "{}",
            image.nodes.strips.len()
        );
        let left_out = (0..image.nodes.lines.len())
            .filter(|&i| image.nodes.lines.get(i).is_some_and(|l| l.is_empty()))
            .count();
        assert!(
            left_out > crate::pdc::POOL - 8,
            "{left_out} fresh lines left out"
        );
        assert!(
            image.approx_bytes() < 8 << 20,
            "{} bytes",
            image.approx_bytes()
        );
        let whole = a.approx_bytes();
        assert!(whole > 50 << 20, "a whole engine: {whole} bytes");
        let (mut b, _btx, mut brx) = image.fork_cells(64, 1024, &cells, false).expect("resumes");
        for i in 0..400 {
            let x = block(&mut a, &mut arx, 64);
            let y = block(&mut b, &mut brx, 64);
            assert_eq!(x, y, "block {i} after the image");
        }
    }
}

#[cfg(test)]
mod refusal_tests {
    use super::*;
    use cantus_core::{Project, Snapshot, Ticks};

    /// Review 2: a snapshot waiting for the control grid is a moment's refusal — not one a
    /// range render should remember for the setup.
    #[test]
    fn a_snapshot_waiting_for_the_grid_is_a_passing_refusal() {
        let mut p = Project::new("refusal");
        let t = p.make_track("Lead");
        p.tracks.push(t);
        let snap = Arc::new(Snapshot::compile(&p, 1));
        let (mut tx, rx_e) = RingBuffer::<EngineMessage>::new(64);
        let (tx_e, mut rx) = RingBuffer::<EngineEvent>::new(1024);
        let mut proc_ = Processor::without_midi(48_000.0, rx_e, tx_e);
        tx.push(EngineMessage::Automation(
            crate::automation::Runtime::build(&snap),
        ))
        .ok();
        tx.push(EngineMessage::SetSnapshot(snap.clone())).ok();
        proc_.settle();
        tx.push(EngineMessage::Seek(Ticks::ZERO)).ok();
        tx.push(EngineMessage::Play).ok();
        let mut b = vec![0f32; 14];
        proc_.process(&mut b, 2); // 7 frames: off the 16-sample grid
        tx.push(EngineMessage::SetSnapshot(snap)).ok();
        let mut none: [f32; 0] = [];
        proc_.process(&mut none, 2);
        while rx.pop().is_ok() {}
        let r = proc_.fork_refusal().expect("waiting for the grid");
        assert!(!r.lasting, "{r:?}");
    }

    /// Where a processor's memory goes (review 3's 80 MB a checkpoint): 256 of each cell
    /// made, the working set before and after.
    #[test]
    #[ignore]
    fn where_a_processors_memory_goes() {
        let ws = || cantus_rt::usage::working_set_bytes().unwrap_or(0) as f64 / 1e6;
        let sr = 48_000.0;
        let mut keep: Vec<Box<dyn std::any::Any>> = Vec::new();
        let mut m = |name: &str, f: &mut dyn FnMut() -> Box<dyn std::any::Any>| {
            let a = ws();
            keep.push(f());
            println!("{name}: {:.1} MB", ws() - a);
        };
        m("256 TrackSlot", &mut || {
            Box::new((0..256).map(|_| TrackSlot::new(sr)).collect::<Vec<_>>())
        });
        m("256 Firmus", &mut || {
            Box::new((0..256).map(|_| Firmus::new(sr)).collect::<Vec<_>>())
        });
        m("256 Pulsus", &mut || {
            Box::new((0..256).map(|_| Pulsus::new(sr)).collect::<Vec<_>>())
        });
        m("256 TrackOut", &mut || {
            Box::new((0..256).map(|_| TrackOut::new()).collect::<Vec<_>>())
        });
        m("256 Outbox", &mut || {
            Box::new(
                (0..256)
                    .map(|_| crate::graph::notes::Outbox::new())
                    .collect::<Vec<_>>(),
            )
        });
        m("256 StripSlot", &mut || {
            Box::new((0..256).map(|_| StripSlot::new()).collect::<Vec<_>>())
        });
        m("256 StripOut", &mut || {
            Box::new((0..256).map(|_| StripOut::new()).collect::<Vec<_>>())
        });
        m("a processor", &mut || {
            let (_tx, rx_e) = RingBuffer::<EngineMessage>::new(64);
            let (tx_e, _rx) = RingBuffer::<EngineEvent>::new(1024);
            Box::new(Processor::without_midi(sr as f64, rx_e, tx_e))
        });
    }
}

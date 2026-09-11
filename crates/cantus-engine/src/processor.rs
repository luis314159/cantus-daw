//! The audio-thread processor. Owns every voice, buffer and queue it needs; after
//! construction it never allocates, locks, logs or panics (ADR 0005).
//!
//! Signal flow per track: instrument → **polarity** → insert effects (stereo) →
//! **width** → gain/pan → master (**width → gain/pan → polarity**).
//!
//! Firmus renders mono and is copied into both channels; Pulsus renders **stereo**,
//! because a pad carries its own pan and a kit is a dozen sources on one channel. From
//! the polarity switch onwards the two are the same signal path.
//!
//! Polarity sits at the *input* of a channel, where a console's Ø switch is, so that what
//! comes out of the strip is the whole channel inverted — inserts and all. On the master
//! it sits at the very end, which is the same place for the same reason: it is the last
//! thing before the output.

use crate::effects::EffectUnit;
use crate::messages::*;
use crate::transport::Transport;
use cantus_core::{
    apply_width, DrumPad, InstrumentSpec, NoteEvent, PadVoice, PanLaw, Snapshot, TrackId,
};
use cantus_dsp::{
    Click, DrumVoice, Firmus, FirmusSettings, PadSettings, Pulsus, SampleRef, Wave, MAX_PADS,
};
use rtrb::{Consumer, Producer, RingBuffer};
use std::sync::Arc;
use std::time::Instant;

/// Tag used for live (keyboard/MIDI) notes so they never collide with sequenced note ids.
const LIVE_TAG: u64 = u64::MAX;
/// How many snapshots we can hold back when the garbage queue is full.
const PENDING_GARBAGE: usize = 8;
/// MIDI events drained per block at most.
const MAX_MIDI_PER_BLOCK: usize = 512;

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
}

#[allow(clippy::vec_box)]
struct TrackSlot {
    id: Option<TrackId>,
    instrument: SlotInstrument,
    synth: Firmus,
    drums: Pulsus,
    /// Insert chain in processing order; capacity fixed at construction.
    effects: Vec<Box<EffectUnit>>,
    bypass: [bool; MAX_EFFECTS_PER_TRACK],
    /// Index into the snapshot track's events of the next event to fire.
    next_event: usize,
    gain: f32,
    pan: f32,
    width: f32,
    phase_invert: bool,
    audible: bool,
    peak: f32,
}

impl TrackSlot {
    fn note_on(&mut self, pitch: u8, velocity: u8, tag: u64) {
        match self.instrument {
            SlotInstrument::Firmus => self.synth.note_on(pitch, velocity, tag),
            SlotInstrument::Pulsus => self.drums.note_on(pitch, velocity, tag),
        }
    }

    fn note_off(&mut self, pitch: u8, tag: u64) {
        match self.instrument {
            SlotInstrument::Firmus => self.synth.note_off(pitch, tag),
            SlotInstrument::Pulsus => self.drums.note_off(pitch, tag),
        }
    }

    fn all_notes_off(&mut self) {
        match self.instrument {
            SlotInstrument::Firmus => self.synth.all_notes_off(),
            SlotInstrument::Pulsus => self.drums.all_notes_off(),
        }
    }

    /// Hard reset. Both engines, not just the live one: a slot that has just changed
    /// instrument must not keep the other one's tail waiting to be heard again.
    fn reset(&mut self) {
        self.synth.reset();
        self.drums.reset();
    }

    fn active_voices(&self) -> usize {
        match self.instrument {
            SlotInstrument::Firmus => self.synth.active_voices(),
            SlotInstrument::Pulsus => self.drums.active_voices(),
        }
    }

    fn new(sample_rate: f32) -> Self {
        TrackSlot {
            id: None,
            instrument: SlotInstrument::Firmus,
            synth: Firmus::new(sample_rate),
            drums: Pulsus::new(sample_rate),
            effects: Vec::with_capacity(MAX_EFFECTS_PER_TRACK),
            bypass: [false; MAX_EFFECTS_PER_TRACK],
            next_event: 0,
            gain: 1.0,
            pan: 0.0,
            width: 1.0,
            phase_invert: false,
            audible: true,
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
    transport: Transport,
    slots: Vec<TrackSlot>,
    /// Snapshot track index → slot index.
    track_to_slot: Vec<usize>,
    mono: Vec<f32>,
    buf_l: Vec<f32>,
    buf_r: Vec<f32>,
    click: Click,
    metronome: bool,
    next_beat_tick: f64,
    live_track: Option<TrackId>,
    recording: bool,
    stats: EngineStats,
    meters: Meters,
    /// Whether `transport.playing` was true on the previous block (to emit note-offs).
    was_playing: bool,
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
            transport: Transport::new(sample_rate),
            slots: (0..MAX_TRACKS).map(|_| TrackSlot::new(sr)).collect(),
            track_to_slot: Vec::with_capacity(MAX_TRACKS),
            mono: vec![0.0; MAX_BLOCK_FRAMES],
            buf_l: vec![0.0; MAX_BLOCK_FRAMES],
            buf_r: vec![0.0; MAX_BLOCK_FRAMES],
            click: Click::new(sr),
            metronome: false,
            next_beat_tick: 0.0,
            live_track: None,
            recording: false,
            stats: EngineStats {
                sample_rate,
                ..Default::default()
            },
            meters: Meters::default(),
            was_playing: false,
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

        self.drain_messages();
        self.drain_midi();
        self.retry_garbage();

        if self.was_playing && !self.transport.is_playing() {
            self.release_all();
        }
        self.was_playing = self.transport.is_playing();

        self.render(output, channels, frames);

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
                EngineMessage::Play => {
                    if !self.transport.is_playing() {
                        self.reseek_events();
                        self.transport.play();
                    }
                }
                EngineMessage::Stop => self.transport.stop(),
                EngineMessage::Seek(tick) => {
                    self.release_all();
                    self.transport.seek(tick);
                    self.reseek_events();
                }
                EngineMessage::NoteOn {
                    track,
                    pitch,
                    velocity,
                } => self.live_note(track, pitch, velocity.max(1)),
                EngineMessage::NoteOff { track, pitch } => self.live_note(track, pitch, 0),
                EngineMessage::Midi(ev) => self.midi(ev),
                EngineMessage::SetLiveTrack(t) => self.live_track = t,
                EngineMessage::SetRecording(on) => self.recording = on,
                EngineMessage::Panic => {
                    for s in &mut self.slots {
                        s.reset();
                        for e in &mut s.effects {
                            e.reset();
                        }
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
                if let Some(slot) = self.slot_for(track) {
                    self.slots[slot].all_notes_off();
                }
            }
            _ => {}
        }
    }

    /// Play (velocity > 0) or release a live note and, while recording, report it.
    fn live_note(&mut self, track: TrackId, pitch: u8, velocity: u8) {
        let Some(slot) = self.slot_for(track) else {
            return;
        };
        if velocity > 0 {
            self.slots[slot].note_on(pitch, velocity, LIVE_TAG);
        } else {
            self.slots[slot].note_off(pitch, LIVE_TAG);
        }
        if self.recording && self.transport.is_playing() {
            let _ = self.tx.push(EngineEvent::NoteInput(NoteInput {
                track,
                tick: self.transport.position(),
                pitch,
                velocity,
            }));
        }
    }

    fn slot_for(&self, track: TrackId) -> Option<usize> {
        self.slots.iter().position(|s| s.id == Some(track))
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
            self.rebuild_chain(slot, &t.effects);
            let s = &mut self.slots[slot];
            s.gain = t.gain;
            s.pan = t.pan;
            s.width = t.width;
            s.phase_invert = t.phase_invert;
            s.audible = t.audible;
            apply_instrument(s, &t.instrument, &t.samples);
        }
        // Slots whose track vanished: silence them, free them, return their effects.
        for (i, in_use) in used.iter().enumerate() {
            if !in_use && self.slots[i].id.is_some() {
                self.slots[i].all_notes_off();
                self.slots[i].id = None;
                while let Some(e) = self.slots[i].effects.pop() {
                    self.dispose_effect(e);
                }
            }
        }
        // Units nobody claimed are stale.
        while let Some(e) = self.incoming_effects.pop() {
            self.dispose_effect(e);
        }
        self.transport.set_bpm(snapshot.tempo_bpm);
        self.transport.set_loop(snapshot.loop_region.clone());
        self.metronome = snapshot.metronome;

        let old = self.snapshot.replace(snapshot);
        if let Some(old) = old {
            self.dispose(old);
        }
        self.reseek_events();
    }

    /// Re-order a slot's effect chain to match `wanted`, pulling new units from the
    /// incoming pool and returning dropped ones. No allocation: every Vec involved was
    /// sized at construction and `wanted` is capped at MAX_EFFECTS_PER_TRACK.
    fn rebuild_chain(&mut self, slot: usize, wanted: &[cantus_core::EffectSlot]) {
        self.scratch_effects.clear();
        while let Some(e) = self.slots[slot].effects.pop() {
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
                    let s = &mut self.slots[slot];
                    let idx = s.effects.len();
                    s.bypass[idx] = want.bypass;
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

    fn dispose_effect(&mut self, unit: Box<EffectUnit>) {
        if self.tx.push(EngineEvent::GarbageEffect(unit)).is_err() {
            // Queue full: the box is freed here. Extremely unlikely (queue holds 4096).
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
    }

    fn render(&mut self, output: &mut [f32], channels: usize, frames: usize) {
        for s in &mut self.slots {
            s.peak = 0.0;
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
        // `ZeroDb` is the balance formulation: unity at centre, the far channel taper.
        let (master_l, master_r) = PanLaw::ZeroDb.gains(master_pan);
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
                            self.click.trigger(in_bar.abs() < 0.5);
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
                    self.wrap_loop();
                }
                continue;
            }

            // Render [done, seg_end).
            let n = seg_end - done;
            let out = &mut output[done * channels..seg_end * channels];
            for ti in 0..self.track_to_slot.len() {
                let slot = self.track_to_slot[ti];
                let s = &mut self.slots[slot];
                let mono = &mut self.mono[..n];
                let l = &mut self.buf_l[..n];
                let r = &mut self.buf_r[..n];
                // Firmus is mono and is copied into both channels; Pulsus writes stereo
                // directly, because its pads are panned inside the instrument. Everything
                // downstream of here is the same signal path for both.
                let has_signal = match s.instrument {
                    SlotInstrument::Firmus => {
                        mono.iter_mut().for_each(|x| *x = 0.0);
                        s.synth.process_add(mono)
                    }
                    SlotInstrument::Pulsus => {
                        l.iter_mut().for_each(|x| *x = 0.0);
                        r.iter_mut().for_each(|x| *x = 0.0);
                        s.drums.process_add(l, r)
                    }
                };
                if !has_signal && s.effects.is_empty() {
                    continue; // Idle: nothing to mix, nothing to meter.
                }
                // Polarity first: the console's Ø is an input-stage switch, so the whole
                // channel — inserts included — comes out inverted.
                match s.instrument {
                    SlotInstrument::Firmus => {
                        if s.phase_invert {
                            for x in mono.iter_mut() {
                                *x = -*x;
                            }
                        }
                        l.copy_from_slice(mono);
                        r.copy_from_slice(mono);
                    }
                    SlotInstrument::Pulsus => {
                        if s.phase_invert {
                            for (a, b) in l.iter_mut().zip(r.iter_mut()) {
                                *a = -*a;
                                *b = -*b;
                            }
                        }
                    }
                }
                for (i, e) in s.effects.iter_mut().enumerate() {
                    if !s.bypass[i] {
                        e.process(l, r);
                    }
                }
                // Width, then pan: narrowing a stereo reverb has to happen before the
                // channel is placed, or the placement is what gets narrowed.
                apply_width(l, r, s.width);
                let (gl, gr) = law.gains(s.pan);
                let g = if s.audible { s.gain } else { 0.0 };
                let (gl, gr) = (g * gl, g * gr);
                let mut peak = s.peak;
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
                s.peak = peak;
            }
            if self.click.is_active() {
                for i in 0..n {
                    let c = self.click.next();
                    let frame = &mut out[i * channels..(i + 1) * channels];
                    frame[0] += c;
                    if channels > 1 {
                        frame[1] += c;
                    }
                }
            }
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

            done = seg_end;
            if self.transport.is_playing() {
                self.transport.advance(n as u64);
                if wrap_at_seg_end {
                    self.wrap_loop();
                }
            }
        }

        self.meters.master = master_peak;
        self.meters.track_count = self.track_to_slot.len();
        for (ti, slot) in self.track_to_slot.iter().enumerate() {
            self.meters.tracks[ti] = self.slots[*slot].peak;
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
    }
}

#[inline]
fn fire(slot: &mut TrackSlot, ev: &NoteEvent) {
    if ev.is_on() {
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
) {
    let wanted = match spec {
        InstrumentSpec::Firmus(_) => SlotInstrument::Firmus,
        InstrumentSpec::Pulsus(_) => SlotInstrument::Pulsus,
    };
    if slot.instrument != wanted {
        slot.reset();
        slot.instrument = wanted;
    }
    match spec {
        InstrumentSpec::Firmus(p) => slot.synth.set_settings(&to_settings(p)),
        InstrumentSpec::Pulsus(k) => {
            // Written into the kit's fixed table in place: this runs on the audio thread,
            // once per snapshot, and must not allocate (ADR 0005).
            slot.drums.set_pad_count(k.pads.len());
            for (i, pad) in k.pads.iter().take(MAX_PADS).enumerate() {
                slot.drums.set_pad(i, pad.pitch, &to_pad(pad));
                // The snapshot carries the decoded audio (`SampleCache::attach`); this is
                // an `Arc` clone per pad and nothing more.
                let sample = samples.get(i).and_then(|s| s.as_ref()).map(|d| SampleRef {
                    frames: d.frames.clone(),
                    rate: d.rate as f32,
                    channels: d.channels as usize,
                });
                slot.drums.set_sample(i, sample);
            }
        }
    }
}

fn to_pad(pad: &DrumPad) -> PadSettings {
    PadSettings {
        voice: match pad.voice {
            PadVoice::Kick => DrumVoice::Kick,
            PadVoice::Snare => DrumVoice::Snare,
            PadVoice::HiHat => DrumVoice::HiHat,
            PadVoice::Sample => DrumVoice::Sample,
        },
        tune: pad.tune,
        decay: pad.decay,
        pitch_drop: pad.pitch_drop,
        pitch_decay: pad.pitch_decay,
        click: pad.click,
        drive: pad.drive,
        noise_decay: pad.noise_decay,
        tone: pad.tone,
        mix: pad.mix,
        hp_cutoff: pad.hp_cutoff,
        gain: cantus_core::db_to_linear(pad.level_db),
        pan: pad.pan,
        // A muted pad is already gone from the snapshot's events (`Snapshot::compile`),
        // so nothing here has to know about it.
        choke: pad.choke,
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

//! The audio-thread processor. Owns every voice, buffer and queue it needs; after
//! construction it never allocates, locks, logs or panics (ADR 0005).

use crate::messages::*;
use crate::transport::Transport;
use cantus_core::{pan_gains, InstrumentSpec, NoteEvent, Snapshot, TrackId, TrackSnapshot};
use cantus_dsp::{Click, Firmus, FirmusSettings, Wave};
use rtrb::{Consumer, Producer};
use std::sync::Arc;
use std::time::Instant;

/// Tag used for live (keyboard/MIDI) notes so they never collide with sequenced note ids.
const LIVE_TAG: u64 = u64::MAX;
/// How many snapshots we can hold back when the garbage queue is full.
const PENDING_GARBAGE: usize = 8;

struct TrackSlot {
    id: Option<TrackId>,
    synth: Firmus,
    /// Index into the snapshot track's events of the next event to fire.
    next_event: usize,
    gain: f32,
    pan: f32,
    audible: bool,
    peak: f32,
}

impl TrackSlot {
    fn new(sample_rate: f32) -> Self {
        TrackSlot {
            id: None,
            synth: Firmus::new(sample_rate),
            next_event: 0,
            gain: 1.0,
            pan: 0.0,
            audible: true,
            peak: 0.0,
        }
    }
}

pub struct Processor {
    sample_rate: f64,
    rx: Consumer<EngineMessage>,
    tx: Producer<EngineEvent>,
    snapshot: Option<Arc<Snapshot>>,
    pending_garbage: Vec<Arc<Snapshot>>,
    transport: Transport,
    slots: Vec<TrackSlot>,
    /// Snapshot track index → slot index.
    track_to_slot: Vec<usize>,
    mono: Vec<f32>,
    click: Click,
    metronome: bool,
    next_beat_tick: f64,
    stats: EngineStats,
    meters: Meters,
    /// Whether `transport.playing` was true on the previous block (to emit note-offs).
    was_playing: bool,
}

impl Processor {
    pub fn new(sample_rate: f64, rx: Consumer<EngineMessage>, tx: Producer<EngineEvent>) -> Self {
        let sr = sample_rate as f32;
        Processor {
            sample_rate,
            rx,
            tx,
            snapshot: None,
            pending_garbage: Vec::with_capacity(PENDING_GARBAGE),
            transport: Transport::new(sample_rate),
            slots: (0..MAX_TRACKS).map(|_| TrackSlot::new(sr)).collect(),
            track_to_slot: Vec::with_capacity(MAX_TRACKS),
            mono: vec![0.0; MAX_BLOCK_FRAMES],
            click: Click::new(sr),
            metronome: false,
            next_beat_tick: 0.0,
            stats: EngineStats {
                sample_rate,
                ..Default::default()
            },
            meters: Meters::default(),
            was_playing: false,
        }
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
        self.stats.active_voices = self.slots.iter().map(|s| s.synth.active_voices()).sum();

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
                } => {
                    if let Some(slot) = self.slot_for(track) {
                        self.slots[slot].synth.note_on(pitch, velocity, LIVE_TAG);
                    }
                }
                EngineMessage::NoteOff { track, pitch } => {
                    if let Some(slot) = self.slot_for(track) {
                        self.slots[slot].synth.note_off(pitch, LIVE_TAG);
                    }
                }
                EngineMessage::Panic => {
                    for s in &mut self.slots {
                        s.synth.reset();
                    }
                }
            }
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
                    self.slots[i].synth.reset();
                    self.slots[i].id = Some(t.id);
                    i
                }
            };
            self.track_to_slot.push(slot);
            let s = &mut self.slots[slot];
            s.gain = t.gain;
            s.pan = t.pan;
            s.audible = t.audible;
            s.synth.set_settings(&to_settings(&t.instrument));
        }
        // Slots whose track vanished: silence them and free them.
        for (i, s) in self.slots.iter_mut().enumerate() {
            if !used[i] && s.id.is_some() {
                s.synth.all_notes_off();
                s.id = None;
            }
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
            s.synth.all_notes_off();
        }
    }

    fn render(&mut self, output: &mut [f32], channels: usize, frames: usize) {
        for s in &mut self.slots {
            s.peak = 0.0;
        }
        let mut master_peak = [0f32; 2];
        let master_gain = self.snapshot.as_ref().map(|s| s.master_gain).unwrap_or(1.0);

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
                mono.iter_mut().for_each(|x| *x = 0.0);
                if !s.synth.process_add(mono) {
                    continue; // Idle: nothing to mix, nothing to meter.
                }
                let (pl, pr) = pan_gains(s.pan);
                let g = if s.audible { s.gain } else { 0.0 };
                let (gl, gr) = (g * pl, g * pr);
                let mut peak = s.peak;
                for (i, m) in mono.iter().enumerate() {
                    peak = peak.max(m.abs() * g);
                    let frame = &mut out[i * channels..(i + 1) * channels];
                    frame[0] += m * gl;
                    if channels > 1 {
                        frame[1] += m * gr;
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
            for i in 0..n {
                let frame = &mut out[i * channels..(i + 1) * channels];
                for (ch, s) in frame.iter_mut().enumerate() {
                    *s *= master_gain;
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
        slot.synth.note_on(ev.pitch, ev.velocity, ev.note.0);
    } else {
        slot.synth.note_off(ev.pitch, ev.note.0);
    }
}

fn to_settings(spec: &InstrumentSpec) -> FirmusSettings {
    match spec {
        InstrumentSpec::Firmus(p) => FirmusSettings {
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
        },
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

#[allow(dead_code)]
fn _track_snapshot_is_send(_: &TrackSnapshot) {}

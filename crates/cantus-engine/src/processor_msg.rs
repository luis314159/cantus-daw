//! The control phase's messages: from the UI (`EngineMessage`), from MIDI, and the live
//! notes they play — and the notes on their way to the effects that hear a track
//! (ADR 0022, `graph::notes`). Everything here runs on the callback between passes, so
//! every node's cell is free to borrow.

use super::*;

impl Processor {
    pub(super) fn drain_messages(&mut self) {
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
                    if let Some(mut p) = self.nodes.probe.try_borrow_mut() {
                        *p = None;
                    }
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
                EngineMessage::WatchScopes(ids) => self.scoped = ids,
                EngineMessage::SetCapture(target) => {
                    self.capture_target = target;
                    self.resolve_capture();
                    // A strip's take is delayed to the master's time: its line.
                    self.compensate();
                }
                EngineMessage::Panic => self.panic(),
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
                    let was = self.click_on();
                    self.live.record_start = at.map(|t| t.0 as f64);
                    // A count-in clicks with the switch off: its click needs its line.
                    if self.click_on() != was {
                        self.compensate();
                    }
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

    /// Every note on its way to an effect is forgotten, and every unit is reset.
    fn panic(&mut self) {
        let nodes = self.nodes.clone();
        for i in 0..self.listener_count {
            if let Some(mut l) = nodes.listener(i) {
                if let Some(ob) = nodes.read(&nodes.tracks[l.track].outbox) {
                    l.cursor = ob.written();
                }
            }
        }
        for slot in 0..MAX_TRACKS {
            if let Some(mut s) = nodes.track(slot) {
                s.reset();
            }
        }
        for slot in 0..MAX_STRIPS {
            if let Some(mut st) = nodes.strip(slot) {
                for e in &mut st.effects {
                    e.reset();
                }
            }
        }
        for e in self.master_units() {
            e.reset();
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
        let nodes = self.nodes.clone();
        let mut memory = Some(memory);
        let mut back = None;
        for slot in 0..MAX_STRIPS {
            if self.strip_ids[slot].is_none() {
                continue;
            }
            if let Some(mut st) = nodes.strip(slot) {
                if let Some(u) = st.effects.iter_mut().find(|e| e.id == effect) {
                    back = memory.take().and_then(|m| u.supply(m));
                    break;
                }
            }
        }
        if let Some(m) = memory.take() {
            let unit = self
                .master
                .effects
                .iter_mut()
                .chain(self.incoming_effects.iter_mut())
                .find(|e| e.id == effect);
            back = match unit {
                Some(u) => u.supply(m),
                None => Some(m),
            };
        }
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
    pub(super) fn request_memory(&mut self) {
        let nodes = self.nodes.clone();
        let ask = |e: &mut Box<EffectUnit>, tx: &mut Producer<EngineEvent>| {
            if let Some(need) = e.take_request() {
                let ev = EngineEvent::EffectNeed { effect: e.id, need };
                if tx.push(ev).is_err() {
                    e.forget_request();
                }
            }
        };
        for slot in 0..MAX_STRIPS {
            if self.strip_ids[slot].is_none() {
                continue;
            }
            if let Some(mut st) = nodes.strip(slot) {
                for e in st.effects.iter_mut() {
                    ask(e, &mut self.tx);
                }
            }
        }
        for e in self.master.effects.iter_mut() {
            ask(e, &mut self.tx);
        }
    }

    pub(super) fn drain_midi(&mut self) {
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
                        if let Some(mut s) = self.nodes.track(slot) {
                            s.all_notes_off();
                        }
                    }
                }
            }
            0xB0 | 0xE0 => {
                if let Some((c, v)) = controller_of(&ev) {
                    for t in self.live_targets(track) {
                        if let Some(slot) = t.and_then(|t| self.slot_for(t)) {
                            if let Some(mut s) = self.nodes.track(slot) {
                                s.controller(c, v);
                            }
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
                    if let Some(mut s) = self.nodes.track(slot) {
                        s.all_notes_off();
                    }
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
        // Modulators hearing the track: envelopes open, LFOs retrigger (ADR 0057).
        self.auto_live_note(slot, velocity > 0);
        if let Some(mut s) = self.nodes.track(slot) {
            if velocity > 0 {
                s.note_on(pitch, velocity, LIVE_TAG);
            } else {
                s.note_off(pitch, LIVE_TAG);
            }
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

    /// Send a note a track played, at absolute sample `at`, to every effect listening to
    /// it: into the track's outbox, where each listener takes it when the audio reaching
    /// its slot gets there (ADR 0022, `graph::notes`). Allocation-free; a full outbox drops
    /// the note and counts it.
    pub(super) fn notes_to_fx(&mut self, track: usize, at: u64, note: crate::fx::FxNote) {
        let pushed = match self.nodes.tracks[track].outbox.try_borrow_mut() {
            Some(mut ob) => ob.push(at, note),
            None => {
                self.nodes.conflict();
                false
            }
        };
        if !pushed {
            self.stats.dropped_events += 1;
        }
    }

    /// The end of a block: what is past due and was not delivered (its effect did not
    /// run) is dropped, and each outbox learns how far its slowest listener has read.
    pub(super) fn prune_notes(&mut self) {
        let now = self.clock;
        let nodes = &*self.nodes;
        let mut floor = [u64::MAX; MAX_TRACKS];
        for i in 0..self.listener_count {
            if let Some(mut l) = nodes.listener(i) {
                if let Some(ob) = nodes.read(&nodes.tracks[l.track].outbox) {
                    l.prune(&ob, now);
                }
                floor[l.track] = floor[l.track].min(l.cursor);
            }
        }
        for (slot, f) in floor.iter().enumerate() {
            if self.listened[slot] {
                if let Some(mut s) = nodes.tracks[slot].outbox.try_borrow_mut() {
                    let w = s.written();
                    s.set_floor(if *f == u64::MAX { w } else { *f });
                }
            }
        }
    }
}

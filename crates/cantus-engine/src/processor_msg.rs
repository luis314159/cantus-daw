//! The control phase's messages: from the UI (`EngineMessage`), from MIDI, and the live
//! notes they play — and the notes on their way to the effects that hear a track
//! (ADR 0022, `graph::notes`). Everything here runs on the callback between passes, so
//! every node's cell is free to borrow.

use super::track_controls::lane_of;
use super::*;

// Live MPE (round 33, ADR 0080): a child of this module, so it plays live keys the way it does. // r33:mpe
#[path = "inst/mpe_live.rs"] // r33:mpe
pub(super) mod mpe_live; // r33:mpe

impl Processor {
    pub(super) fn drain_messages(&mut self) {
        self.drain_retired(); // fx3:tabula
                              // A snapshot waits for the control grid: what came after it waits with it.
        if self.deferred.is_some() {
            return;
        }
        for _ in 0..MAX_MESSAGES_PER_BLOCK {
            let Ok(msg) = self.rx.pop() else { break };
            match msg {
                // While playing, on the automation's grid only (ADR 0076 §7).
                EngineMessage::SetSnapshot(snapshot) if self.defer_install() => {
                    self.deferred = Some((snapshot, None));
                    break;
                }
                EngineMessage::SetSnapshotPlanned(snapshot, plan) if self.defer_install() => {
                    self.deferred = Some((snapshot, Some(plan)));
                    break;
                }
                EngineMessage::SetSnapshot(snapshot) => self.install_snapshot(snapshot, None),
                EngineMessage::SetSnapshotPlanned(snapshot, plan) => {
                    self.install_snapshot(snapshot, Some(plan))
                }
                EngineMessage::Automation(rt) => self.auto_arrived(rt),
                // The session view (ADR 0076 §7): its presses go to the conductor beside the
                // engine handle (`EngineHandle::send`), never here; one that does is nothing.
                EngineMessage::Session(_) => {}
                EngineMessage::AddEffect(unit) => {
                    if self.incoming_effects.len() < MAX_PENDING_EFFECTS {
                        self.incoming_effects.push(unit);
                    } else {
                        self.stats.dropped_events += 1;
                        let _ = self.tx.push(EngineEvent::GarbageEffect(unit));
                    }
                }
                EngineMessage::ReplaceEffect(unit) => self.replace_effect(unit),
                // fx3:contract — Tabula's pedals (ADR 0131).
                EngineMessage::AddPedal {
                    effect,
                    pedal,
                    unit,
                } => self.adopt_pedal(effect, pedal, unit),
                EngineMessage::SetAnalysedPedal(p) => self.analysed_pedal = p,
                EngineMessage::AddInstrument(unit) => {
                    if self.incoming_instruments.len() < self.incoming_instruments.capacity() {
                        self.incoming_instruments.push(unit);
                    } else {
                        self.stats.dropped_events += 1;
                        self.dispose_instrument(unit);
                    }
                }
                EngineMessage::ReplaceInstrument(unit) => self.replace_instrument(unit),
                EngineMessage::AddPart(part) => self.part_arrived(part),
                EngineMessage::Cells(cells) => self.cells_arrived(cells),
                // ADR 0107: a ring made the first time its scope is watched. The cell had
                // none (each index is sent once), so nothing is freed here.
                EngineMessage::ScopeRing { index, ring } => {
                    match self.nodes.scopes.get(index).map(|c| c.try_borrow_mut()) {
                        Some(Some(mut tap)) if tap.ring.is_none() => tap.ring = Some(ring),
                        _ => self.nodes.conflict(),
                    }
                }
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
                        // The lanes chased now it plays (ADR 0068).
                        self.chase_controls();
                        self.played = 0;
                    }
                }
                EngineMessage::Stop => {
                    self.transport.stop();
                    self.sync_stop_pressed(); // r33:sync
                }
                EngineMessage::Seek(tick) => {
                    self.release_all();
                    self.transport.seek(tick);
                    self.reseek_events();
                    self.played = 0;
                }
                // r34:listen — a pre-roll on a whole sample (ADR 0083).
                EngineMessage::SeekFrac(tick) => {
                    self.release_all();
                    self.transport.seek_f64(tick);
                    self.reseek_events();
                    self.played = 0;
                }
                EngineMessage::NoteOn {
                    track,
                    pitch,
                    velocity,
                } => self.live_note(track, pitch, velocity.max(1), 0),
                EngineMessage::NoteOff { track, pitch } => self.live_note(track, pitch, 0, 0),
                EngineMessage::Midi(ev) => self.midi(ev),
                EngineMessage::SetLiveTrack(t) => self.live_track = t,
                EngineMessage::SetLiveEcho(tracks) => self.set_live_echo(tracks),
                EngineMessage::SetMpe(zones) => self.set_mpe(zones), // r33:mpe
                EngineMessage::StartPing(job) => self.ping_start(job), // r36:external
                EngineMessage::SetRecording(on) => self.recording = on,
                EngineMessage::SetLowLatency(limit) => self.set_low_latency(limit),
                EngineMessage::SetAnalyser(id) => self.analysed = id,
                EngineMessage::SetHearKey(id) => self.hear_key = id, // r40:pads
                EngineMessage::WatchScopes(ids) => {
                    // A ring whose watch changed (released, or given to another — or the
                    // same Speculum watched again) starts afresh: nothing of an earlier
                    // watch's chunk or losses goes into the new one (review 2).
                    for (i, (old, new)) in self.scoped.iter().zip(ids.iter()).enumerate() {
                        if old == new {
                            continue;
                        }
                        match self.nodes.scopes[i].try_borrow_mut() {
                            Some(mut tap) => {
                                tap.owner = None;
                                tap.pending = crate::fx::speculum::ScopeChunk::EMPTY;
                                tap.lost = 0;
                            }
                            None => self.nodes.conflict(),
                        }
                    }
                    self.scoped = ids;
                    self.scope_delays();
                }
                EngineMessage::SetCapture(target) => {
                    self.capture_target = target;
                    self.resolve_capture();
                    // A strip's take is delayed to the master's time: its line.
                    self.compensate();
                }
                EngineMessage::Panic => self.panic(),
                EngineMessage::PdcLines(mut lines) => {
                    self.pdc.supply(&mut lines);
                    self.edges_for_lines();
                    if self.tx.push(EngineEvent::GarbageLines(lines)).is_err() {
                        // Queue full: freed here. Extremely unlikely (queue holds 4096).
                        self.stats.dropped_events += 1;
                    }
                    self.compensate();
                }
                EngineMessage::EffectMemory {
                    effect,
                    pedal,
                    memory,
                } => {
                    self.supply_effect(effect, pedal, memory);
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
                EngineMessage::SetDeviceRoundTrip(frames) => {
                    self.live.round_trip = frames;
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
                EngineMessage::PreviewGain(gain) => self.preview.set_gain(gain), // r39:fixes
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
        pedal: Option<cantus_core::fx::tabula::PedalId>,
        memory: crate::effects::EffectMemory,
    ) {
        let nodes = self.nodes.clone();
        let mut memory = Some(memory);
        let mut back = [None, None];
        let mut busy = false;
        // What the unit let go of first (fix-a, A-M3: a Delay takes a new line only with
        // its old one given back), then the memory.
        // fx3:contract — memory for one of a board's pedals goes to that pedal.
        let mut give = |u: &mut EffectUnit, m: crate::effects::EffectMemory| {
            let u = match pedal {
                None => u,
                Some(p) => match u.pedal_mut(p) {
                    Some(k) => k,
                    None => {
                        back[1] = Some(m);
                        return;
                    }
                },
            };
            back[0] = u.take_spent();
            back[1] = u.supply(m);
            busy = u.busy();
        };
        for slot in 0..MAX_STRIPS {
            if self.strip_ids[slot].is_none() {
                continue;
            }
            if let Some(mut st) = nodes.strip(slot) {
                if let Some(u) = st.effects.iter_mut().find(|e| e.id == effect) {
                    if let Some(m) = memory.take() {
                        give(u, m);
                    }
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
            match unit {
                Some(u) => give(u, m),
                None => back[1] = Some(m),
            }
        }
        self.fx_busy |= busy;
        for back in back.into_iter().flatten() {
            if self.tx.push(EngineEvent::GarbageMemory(back)).is_err() {
                // Queue full: freed here. Extremely unlikely (queue holds 4096).
                self.stats.dropped_events += 1;
            }
        }
        self.compensate();
        self.request_memory();
    }

    /// fix-a (A-M3): while an effect moves memory in (a Delay's history into its new line,
    /// a stretch per pass), what it lets go of is sent back to be freed after each block —
    /// only while one is busy (a loop over the strips then, nothing otherwise). What finds
    /// the queue full waits in the unit for the next block.
    pub(super) fn sweep_spent(&mut self) {
        if !self.fx_busy {
            return;
        }
        let nodes = self.nodes.clone();
        let mut busy = false;
        let mut sweep = |e: &mut EffectUnit, tx: &mut Producer<EngineEvent>| {
            if tx.slots() > 0 {
                if let Some(m) = e.take_spent() {
                    let _ = tx.push(EngineEvent::GarbageMemory(m));
                }
            }
            busy |= e.busy();
        };
        for slot in 0..MAX_STRIPS {
            if self.strip_ids[slot].is_none() {
                continue;
            }
            if let Some(mut st) = nodes.strip(slot) {
                for e in st.effects.iter_mut() {
                    sweep(e, &mut self.tx);
                }
            }
        }
        for e in self.master.effects.iter_mut() {
            sweep(e, &mut self.tx);
        }
        self.fx_busy = busy;
    }

    /// fx3:contract — a pedal for a board the engine runs (`EngineMessage::AddPedal`): the
    /// board adopts it, or it goes back to be dropped (no such board, no room).
    fn adopt_pedal(
        &mut self,
        effect: cantus_core::EffectId,
        pedal: cantus_core::fx::tabula::PedalId,
        unit: Box<EffectUnit>,
    ) {
        let nodes = self.nodes.clone();
        let mut unit = Some(unit);
        let mut back = None;
        for slot in 0..MAX_STRIPS {
            if self.strip_ids[slot].is_none() {
                continue;
            }
            if let Some(mut st) = nodes.strip(slot) {
                if let Some(u) = st.effects.iter_mut().find(|e| e.id == effect) {
                    back = unit.take().and_then(|k| u.adopt(pedal, k));
                    break;
                }
            }
        }
        if let Some(k) = unit.take() {
            let host = self
                .master
                .effects
                .iter_mut()
                .chain(self.incoming_effects.iter_mut())
                .find(|e| e.id == effect);
            back = match host {
                Some(u) => u.adopt(pedal, k),
                None => Some(k),
            };
        }
        if let Some(back) = back {
            if self.tx.push(EngineEvent::GarbageEffect(back)).is_err() {
                self.stats.dropped_events += 1;
            }
        }
    }

    /// Ask the UI thread for memory any effect's settings need and it has not got (ADR
    /// 0022). Each need is asked for once; the effect runs its previous path meanwhile.
    pub(super) fn request_memory(&mut self) {
        let nodes = self.nodes.clone();
        let ask = |e: &mut Box<EffectUnit>, tx: &mut Producer<EngineEvent>| ask_memory(e, tx);
        self.boards.clear(); // fx3:tabula — where the boards are, for `drain_retired`
        for slot in 0..MAX_STRIPS {
            if self.strip_ids[slot].is_none() {
                continue;
            }
            if let Some(mut st) = nodes.strip(slot) {
                for e in st.effects.iter_mut() {
                    ask(e, &mut self.tx);
                    if e.kind == cantus_core::EffectKind::Board {
                        self.boards.push(slot, e.id); // fx3:tabula
                    }
                }
            }
        }
        for e in self.master.effects.iter_mut() {
            ask(e, &mut self.tx);
            if e.kind == cantus_core::EffectKind::Board {
                self.boards.push(usize::MAX, e.id); // fx3:tabula
            }
        }
    }

    /// fx3:tabula — the pedals the boards have let go (faded out, `FxUnit::take_retired`),
    /// handed back to be dropped off the audio thread. Each block; nothing to do without a
    /// board.
    pub(super) fn drain_retired(&mut self) {
        let nodes = self.nodes.clone();
        for i in 0..self.boards.len() {
            let (slot, id) = self.boards.at(i);
            // Only as many as the queue has room for: the rest wait in the board.
            let give = |e: &mut EffectUnit, tx: &mut Producer<EngineEvent>, lost: &mut u64| {
                while tx.slots() > 0 {
                    let Some(u) = e.take_retired() else { break };
                    if tx.push(EngineEvent::GarbageEffect(u)).is_err() {
                        *lost += 1;
                    }
                }
            };
            let mut lost = 0;
            if slot == usize::MAX {
                if let Some(e) = self.master.effects.iter_mut().find(|e| e.id == id) {
                    give(e, &mut self.tx, &mut lost);
                }
            } else if let Some(mut st) = nodes.strip(slot) {
                if let Some(e) = st.effects.iter_mut().find(|e| e.id == id) {
                    give(e, &mut self.tx, &mut lost);
                }
            }
            self.stats.dropped_events += lost;
        }
    }

    pub(super) fn drain_midi(&mut self) {
        for _ in 0..MAX_MIDI_PER_BLOCK {
            let Ok(ev) = self.midi_rx.pop() else { break };
            self.midi(ev);
        }
    }

    fn midi(&mut self, ev: MidiEvent) {
        // r33:mpe — an MPE port's member channels (ADR 0080)
        if self.mpe_midi(ev) {
            return;
        }
        let Some(track) = self.live_track else { return };
        match ev.status & 0xF0 {
            0x90 if ev.data2 > 0 => self.live_note(track, ev.data1, ev.data2, 0),
            // A note-off's second byte is its release velocity (ADR 0068); a note-on at 0 has none.
            0x80 => self.live_note(track, ev.data1, 0, ev.data2 & 0x7F),
            0x90 => self.live_note(track, ev.data1, 0, 0),
            0xB0 if ev.data1 == 123 || ev.data1 == 120 => {
                for t in self.live_targets(track) {
                    if let Some(slot) = t.and_then(|t| self.slot_for(t)) {
                        if let Some(mut s) = self.nodes.track(slot) {
                            s.all_notes_off();
                        }
                    }
                }
            }
            // Round 29 (ADR 0062): what was parsed and dropped before — key pressure to the
            // key's live note, program changes to the instrument.
            0xA0 => {
                let v = f32::from(ev.data2) / 127.0;
                for t in self.live_targets(track) {
                    if let Some(slot) = t.and_then(|t| self.slot_for(t)) {
                        if let Some(mut s) = self.nodes.track(slot) {
                            s.expression_live(ev.data1, crate::inst::NoteExpression::Pressure, v);
                        }
                    }
                }
            }
            0xC0 => {
                for t in self.live_targets(track) {
                    if let Some(slot) = t.and_then(|t| self.slot_for(t)) {
                        if let Some(mut s) = self.nodes.track(slot) {
                            s.program(ev.data1 & 0x7F);
                        }
                    }
                }
            }
            0xB0 | 0xD0 | 0xE0 => {
                if let Some((c, v)) = controller_of(&ev) {
                    for t in self.live_targets(track) {
                        if let Some(slot) = t.and_then(|t| self.slot_for(t)) {
                            if let Some(mut s) = self.nodes.track(slot) {
                                s.controller(c, v);
                            }
                        }
                    }
                }
                // Recorded into the take's lanes where it was heard (ADR 0068).
                if let Some((kind, value)) = lane_of(&ev) {
                    if self.recording && self.transport.is_playing() {
                        let _ = self.tx.push(EngineEvent::ControlInput(ControlInput {
                            track,
                            tick: self.heard_position(),
                            kind,
                            value,
                        }));
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
    /// `release` is a note-off's release velocity (0 none given, ADR 0068).
    fn live_note(&mut self, track: TrackId, pitch: u8, velocity: u8, release: u8) {
        // The echo tracks play it too (heard, not reported: the UI records the live
        // track's notes into every armed track itself).
        for e in self.live_echo {
            if let Some(slot) = e.filter(|e| *e != track).and_then(|e| self.slot_for(e)) {
                self.sound_live(slot, pitch, velocity, release);
            }
        }
        let Some(slot) = self.slot_for(track) else {
            return;
        };
        self.sound_live(slot, pitch, velocity, release);
        if self.recording && self.transport.is_playing() {
            // Where the player heard it: the output lags the engine by its latency.
            let _ = self.tx.push(EngineEvent::NoteInput(NoteInput {
                track,
                tick: self.heard_position(),
                pitch,
                velocity,
                release,
            }));
        }
    }

    /// Play or release a live note on one slot, and tell the effects listening to it.
    fn sound_live(&mut self, slot: usize, pitch: u8, velocity: u8, release: u8) {
        // Modulators hearing the track: envelopes open, LFOs retrigger (ADR 0057).
        self.auto_live_note(slot, velocity > 0);
        // Through its note effects (ADR 0069): what they sound at once is played and heard
        // now; an arpeggio's first step plays at the block's first sample, in the track's
        // pass, once every key of this block is in.
        let mut heard = [crate::fx::FxNote::AllOff; 64];
        let mut n_heard = 0usize;
        let mut through = false;
        if let Some(mut s) = self.nodes.track(slot) {
            if s.fx_active() {
                through = true;
                let now = crate::notefx::play::now_live(&self.transport, self.clock);
                let m = crate::notefx::Msg {
                    on: velocity > 0,
                    pitch,
                    velocity,
                    release,
                    tag: LIVE_TAG,
                    ordinal: 0,
                    live: true,
                };
                let lost = s.fx_play(
                    now,
                    Some((now, m)),
                    |_| false,
                    0,
                    |m| {
                        if n_heard < heard.len() {
                            heard[n_heard] = crate::notefx::play::fx_note(m);
                            n_heard += 1;
                        }
                    },
                );
                self.stats.dropped_events += lost;
            } else if velocity > 0 {
                s.note_on(pitch, velocity, LIVE_TAG);
            } else {
                s.note_off(pitch, LIVE_TAG, release);
            }
        }
        if self.listened[slot] {
            if through {
                for note in heard.into_iter().take(n_heard) {
                    self.notes_to_fx(slot, self.note_at, note);
                }
                return;
            }
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

/// Ask for the memory `e`'s settings need, if it changed since it last asked (ADR 0022):
/// after a snapshot, and after automation configured it (fix-a, A-M2). Forgotten when the
/// queue is full, so it is asked again.
pub(super) fn ask_memory(e: &mut EffectUnit, tx: &mut Producer<EngineEvent>) {
    if let Some(need) = e.take_request() {
        let ev = EngineEvent::EffectNeed {
            effect: e.id,
            pedal: None,
            need,
        };
        if tx.push(ev).is_err() {
            e.forget_request();
        }
    }
    // fx3:contract — and each of a board's pedals, by its id.
    let effect = e.id;
    for k in e.pedals_mut() {
        if let Some(need) = k.unit.take_request() {
            let ev = EngineEvent::EffectNeed {
                effect,
                pedal: Some(k.pedal),
                need,
            };
            if tx.push(ev).is_err() {
                k.unit.forget_request();
            }
        }
    }
}

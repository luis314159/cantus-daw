//! Installing a snapshot: slots for its tracks and strips (kept across snapshots so voices
//! and tails ring on), chains, routing, keys, and handing back what is no longer used —
//! on the callback, between passes, through the nodes' cells.

use super::*;

/// Not in the order.
const NOWHERE: u16 = u16::MAX;

/// Whether two frozen handles are the same render.
fn same_handle(
    a: &Option<Arc<crate::freeze::FrozenHandle>>,
    b: &Option<Arc<crate::freeze::FrozenHandle>>,
) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => Arc::ptr_eq(a, b),
        (None, None) => true,
        _ => false,
    }
}

/// The render frame of the snapshot's loop start, when it loops: a frozen reader keeps it
/// ready (ADR 0064).
fn loop_frame(snapshot: &Snapshot, rate: f64) -> Option<i64> {
    snapshot
        .loop_region
        .as_ref()
        .map(|l| crate::freeze::frame_of_tick(&snapshot.tempo, l.start, rate))
}

impl Processor {
    pub(super) fn install_snapshot(
        &mut self,
        snapshot: Arc<Snapshot>,
        plan: Option<Box<GraphPlan>>,
    ) {
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
        let nodes = self.nodes.clone();
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
                    let free = (0..MAX_TRACKS).find(|i| !used[*i] && self.track_ids[*i].is_none());
                    let i =
                        free.unwrap_or_else(|| (0..MAX_TRACKS).find(|i| !used[*i]).unwrap_or(0));
                    used[i] = true;
                    if let Some(mut s) = nodes.track(i) {
                        s.reset();
                        s.id = Some(t.id);
                    }
                    self.track_ids[i] = Some(t.id);
                    i
                }
            };
            self.track_to_slot.push(slot);
            self.swap_native(slot, t.id, UnitKind::of(&t.instrument));
            if let Some(mut s) = nodes.track(slot) {
                s.gain = t.gain;
                s.pan = t.pan;
                s.audible = t.audible;
                apply_instrument(&mut s, &t.instrument, &t.samples, &t.assets);
                // Freezing (ADR 0064): the render it plays, if any (an `Arc` clone; the
                // one it replaces is still the old snapshot's), and whether it rests.
                let h = crate::freeze::handle_of(t.frozen.as_ref());
                if !same_handle(&s.frozen, &h) {
                    s.frozen_place.block = None;
                    if let Some(old) = s.frozen.as_ref() {
                        crate::freeze::release(old);
                    }
                }
                // Frozen or resting, the instrument is not played: on either change its
                // voices go (review 1 — a note held when it froze sounded again, its
                // note-off long passed, when it thawed during playback).
                let quiet = |frozen: bool, rest: bool| frozen || rest;
                let was = quiet(s.frozen.is_some(), s.rest);
                s.frozen = h;
                s.frozen_place.loop_frame = loop_frame(&snapshot, self.sample_rate);
                s.rest = t.rest;
                if was != quiet(s.frozen.is_some(), s.rest) {
                    s.reset();
                }
            }
            if t.rest {
                nodes.track_peak[slot].store(0, std::sync::atomic::Ordering::Relaxed);
            }
        }
        // Slots whose track vanished: silence them and free them.
        for (i, in_use) in used.iter().enumerate() {
            if !in_use && self.track_ids[i].is_some() {
                self.track_ids[i] = None;
                let native = nodes.track(i).and_then(|mut s| {
                    s.all_notes_off();
                    s.id = None;
                    // Its render let go while the old snapshot still holds it (never the
                    // last owner on this thread).
                    if let Some(h) = s.frozen.take() {
                        crate::freeze::release(&h);
                    }
                    s.native.take()
                });
                if let Some(u) = native {
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
                        (0..MAX_STRIPS).find(|i| !strip_used[*i] && self.strip_ids[*i].is_none());
                    let i = free
                        .unwrap_or_else(|| (0..MAX_STRIPS).find(|i| !strip_used[*i]).unwrap_or(0));
                    strip_used[i] = true;
                    self.strip_ids[i] = Some(st.id);
                    if let Some(mut s) = nodes.strip(i) {
                        s.id = Some(st.id);
                        // A slot taken over by another strip must not carry the last one's
                        // EQ state into it.
                        s.eq_l.reset();
                        s.eq_r.reset();
                    }
                    i
                }
            };
            self.strip_to_slot.push(slot);
            self.rebuild_chain(slot, &st.effects);
            let sr = self.sample_rate as f32;
            if let Some(mut ss) = nodes.strip(slot) {
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
                let h = crate::freeze::handle_of(st.frozen.as_ref());
                if !same_handle(&ss.frozen, &h) {
                    ss.frozen_place.block = None;
                    if let Some(old) = ss.frozen.as_ref() {
                        crate::freeze::release(old);
                    }
                }
                ss.frozen = h;
                ss.frozen_place.loop_frame = loop_frame(&snapshot, self.sample_rate);
                // A strip that starts or stops resting comes back empty: its inserts and
                // EQ hold nothing from before (review 1).
                if ss.rest != st.rest {
                    for e in &mut ss.effects {
                        e.reset();
                    }
                    ss.eq_l.reset();
                    ss.eq_r.reset();
                }
                ss.rest = st.rest;
            }
            if st.rest {
                nodes.strip_peak[slot].store(0, std::sync::atomic::Ordering::Relaxed);
                nodes.strip_gr[slot].store(0, std::sync::atomic::Ordering::Relaxed);
            }
        }
        // Routing is resolved to slot indices once, here — outputs and sends alike.
        for (si, st) in snapshot.strips.iter().take(MAX_STRIPS).enumerate() {
            let slot = self.strip_to_slot[si];
            let Some(mut s) = nodes.strip(slot) else {
                continue;
            };
            s.output = st.output.and_then(|o| self.strip_to_slot.get(o).copied());
            let mut count = 0;
            s.has_pre = false;
            for (to, level, tap) in &st.sends {
                if count >= MAX_SENDS_PER_STRIP {
                    break;
                }
                // A send into itself cannot come through the commands; a file could hold
                // one, and it is dropped here rather than summed into a buffer that has
                // already been read.
                if let Some(d) = self.strip_to_slot.get(*to).copied().filter(|d| *d != slot) {
                    s.sends[count] = (d, *level);
                    s.send_tap[count] = *tap;
                    s.has_pre |= !tap.is_post();
                    count += 1;
                }
            }
            s.send_count = count;
        }
        for (ti, t) in snapshot.tracks.iter().take(MAX_TRACKS).enumerate() {
            let slot = self.track_to_slot[ti];
            if let Some(mut s) = nodes.track(slot) {
                s.strip = t.strip.and_then(|s| self.strip_to_slot.get(s).copied());
            }
        }
        self.resolve_keys();
        self.install_master(&snapshot);
        self.resolve_capture();
        // The plan's order, in slots.
        self.strip_play_order.clear();
        for si in self.plan.order() {
            self.strip_play_order.push(self.strip_to_slot[*si as usize]);
        }
        self.keys_in_order();
        for (i, in_use) in strip_used.iter().enumerate() {
            if !in_use && self.strip_ids[i].is_some() {
                self.strip_ids[i] = None;
                if let Some(mut s) = nodes.strip(i) {
                    s.id = None;
                    s.output = None;
                    s.send_count = 0;
                    s.has_pre = false;
                    s.rest = false;
                    if let Some(h) = s.frozen.take() {
                        crate::freeze::release(&h);
                    }
                    while let Some(e) = s.effects.pop() {
                        // `dispose_effect` needs `&mut self`: the unit goes through the
                        // scratch list, which never holds more than one chain.
                        if self.scratch_effects.len() < self.scratch_effects.capacity() {
                            self.scratch_effects.push(e);
                        } else {
                            self.stats.dropped_events += 1;
                        }
                    }
                }
                while let Some(e) = self.scratch_effects.pop() {
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
        // The tempo map (ADR 0060): an edit of the tempo re-anchors at the playhead; the
        // old map is the old snapshot's, freed with it off this thread.
        self.transport.set_map(snapshot.tempo.clone());
        self.transport.set_loop(snapshot.loop_region.clone());
        // A piece whose data this snapshot swapped crossfades from now (warp's renders).
        self.transport.mark_swap();
        self.auto_install(&snapshot);

        // The nodes read the snapshot through the pass's frame; its old copy goes before
        // the processor's, which goes back to the UI thread to be freed.
        match nodes.frame.try_borrow_mut() {
            Some(mut f) => {
                f.ctx.snapshot = Some(snapshot.clone());
                // The pass's copy of the transport lets go of the old map here too, while
                // the old snapshot still holds it (never the last owner on this thread).
                f.ctx.transport.clone_from(&self.transport);
            }
            None => nodes.conflict(),
        }
        let old = self.snapshot.replace(snapshot);
        if let Some(old) = old {
            self.dispose(old);
        }
        self.reseek_events();
        self.map_dirty = true;
    }

    /// Each insert's key, by strip slot; the strips something is keyed from keep their
    /// post-fader output. Before the master's own keys are resolved.
    fn resolve_keys(&mut self) {
        let nodes = self.nodes.clone();
        for slot in 0..MAX_STRIPS {
            if self.strip_ids[slot].is_some() {
                if let Some(mut s) = nodes.strip(slot) {
                    s.is_key = false;
                }
            }
        }
        for slot in 0..MAX_STRIPS {
            if self.strip_ids[slot].is_none() {
                continue;
            }
            let mut keys = [None; MAX_EFFECTS_PER_TRACK];
            if let Some(s) = nodes.strip(slot) {
                for (i, k) in keys.iter_mut().enumerate().take(s.effects.len()) {
                    *k = s.key_id[i]
                        .and_then(|id| self.strip_slot_for(id))
                        .filter(|k| *k != slot);
                }
            }
            if let Some(mut s) = nodes.strip(slot) {
                s.key = keys;
            }
            for k in keys.iter().flatten() {
                if let Some(mut s) = nodes.strip(*k) {
                    s.is_key = true;
                }
            }
        }
    }

    /// Whether each insert's key strip runs before it (its key is then a dependency, and
    /// heard); a key from a strip later in the order is silence, as the serial engine
    /// heard it, and is not even looked at — that strip may be running at the same time.
    fn keys_in_order(&mut self) {
        let nodes = self.nodes.clone();
        let mut place = [NOWHERE; MAX_STRIPS];
        for (p, &slot) in self.strip_play_order.iter().enumerate() {
            place[slot] = p as u16;
        }
        for (p, &slot) in self.strip_play_order.iter().enumerate() {
            if let Some(mut s) = nodes.strip(slot) {
                for i in 0..MAX_EFFECTS_PER_TRACK {
                    s.key_before[i] =
                        s.key[i].is_some_and(|k| place[k] != NOWHERE && (place[k] as usize) < p);
                }
            }
        }
    }

    /// Re-order a strip slot's effect chain to match `wanted`, pulling new units from the
    /// incoming pool and returning dropped ones. No allocation: every Vec involved was
    /// sized at construction and `wanted` is capped at MAX_EFFECTS_PER_TRACK.
    fn rebuild_chain(&mut self, slot: usize, wanted: &[cantus_core::EffectSlot]) {
        let nodes = self.nodes.clone();
        let Some(mut s) = nodes.strip(slot) else {
            return;
        };
        self.scratch_effects.clear();
        while let Some(e) = s.effects.pop() {
            self.scratch_effects.push(e);
        }
        for want in wanted.iter().take(MAX_EFFECTS_PER_TRACK) {
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
                    let idx = s.effects.len();
                    s.bypass[idx] = want.bypass;
                    s.mix[idx] = want.mix.clamp(0.0, 1.0);
                    s.key_id[idx] = want.sidechain;
                    s.midi_id[idx] = want.midi_from;
                    s.effects.push(unit);
                }
                // Unit not delivered yet (or dropped): skip it this snapshot.
                None => self.stats.dropped_events += 1,
            }
        }
        drop(s);
        while let Some(e) = self.scratch_effects.pop() {
            self.dispose_effect(e);
        }
    }
}

impl Processor {
    /// Work out every edge's delay (`crate::pdc`) and give each edge that needs one a line.
    /// Runs on the audio thread at each snapshot; loops over fixed arrays, allocates
    /// nothing.
    pub(super) fn compensate(&mut self) {
        use crate::pdc::Edge;
        let nodes = self.nodes.clone();
        let mut arrive = [0usize; MAX_STRIPS];
        let mut post = [0usize; MAX_STRIPS];
        let mut master = 0usize;
        let mut own = [0usize; MAX_STRIPS];
        // Freezing (ADR 0064): a resting strip or track is not run and delays nothing; a
        // strip playing its render gathers nothing, so nothing arrives into it (its render
        // is on the timeline: its own lateness is none).
        let mut rest_s = [false; MAX_STRIPS];
        let mut deaf = [false; MAX_STRIPS];
        // Soft bypass (ADR 0022): a bypassed insert — or a chain switched off — keeps its
        // latency, delaying its dry signal by it, so bypassing re-plans nothing.
        for (slot, own) in own.iter_mut().enumerate() {
            if self.strip_ids[slot].is_none() {
                continue;
            }
            if let Some(st) = nodes.strip(slot) {
                rest_s[slot] = st.rest;
                deaf[slot] = st.rest || st.frozen.is_some();
                if let Some(h) = st.frozen.as_ref() {
                    // The latency the render was made with: everything it gathered and
                    // its chain, kept (the render's frames arrive after it).
                    *own = h.latency;
                } else if !st.rest {
                    *own = st.effects.iter().map(|e| e.latency()).sum();
                }
            }
        }
        // Low-latency monitoring (ADR 0064 §4): the monitored path, its hard bypasses.
        let mon = self.monitor_plan(&rest_s);
        if mon.on {
            for (slot, own) in own.iter_mut().enumerate() {
                if self.strip_ids[slot].is_none() || rest_s[slot] {
                    continue;
                }
                if let Some(st) = nodes.strip(slot) {
                    if st.frozen.is_none() {
                        *own = st
                            .effects
                            .iter()
                            .enumerate()
                            .filter(|(i, _)| !st.ll_off[*i])
                            .map(|(_, e)| e.latency())
                            .sum();
                    }
                }
            }
        }
        // A monitored track into a strip carrying monitored input only (or into the
        // master) is not held back.
        let quick = |slot: usize, feeds: Option<usize>| {
            mon.track[slot] && feeds.is_none_or(|d| mon.pure[d])
        };
        let mut rest_t = [false; MAX_TRACKS];
        // A track leaves at its instrument's latency (a hosted synth's lookahead, ADR 0028),
        // so what it feeds arrives no earlier and every other path waits for it.
        let mut leave = [0usize; MAX_TRACKS];
        let mut feeds = [None; MAX_TRACKS];
        for ti in 0..self.track_to_slot.len() {
            let slot = self.track_to_slot[ti];
            let Some(s) = nodes.track(slot) else {
                continue;
            };
            rest_t[slot] = s.rest;
            if s.rest {
                continue;
            }
            let latency = s.latency();
            leave[slot] = latency;
            feeds[slot] = s.strip;
            match s.strip {
                Some(d) if deaf[d] => {}
                _ if quick(slot, s.strip) => {}
                Some(d) => arrive[d] = arrive[d].max(latency),
                None => master = master.max(latency),
            }
        }
        for oi in 0..self.strip_play_order.len() {
            let slot = self.strip_play_order[oi];
            if rest_s[slot] {
                post[slot] = 0;
                continue;
            }
            let p = arrive[slot] + own[slot];
            post[slot] = p;
            let Some(st) = nodes.strip(slot) else {
                continue;
            };
            // A strip carrying monitored input only is not waited for (ADR 0064 §4) —
            // but where it is mixed with backing, it waits with the backing there (review
            // 1: it arrived early, against what the ADR and the chip said).
            if mon.pure[slot] {
                let dests = st
                    .output
                    .into_iter()
                    .chain(st.sends.iter().take(st.send_count).map(|(d, _)| *d));
                for d in dests {
                    if !deaf[d] && !mon.pure[d] {
                        arrive[d] = arrive[d].max(p);
                    }
                }
                continue;
            }
            match st.output {
                Some(d) if deaf[d] => {}
                Some(d) => arrive[d] = arrive[d].max(p),
                None => master = master.max(p),
            }
            for (d, _) in st.sends.iter().take(st.send_count) {
                if !deaf[*d] {
                    arrive[*d] = arrive[*d].max(p);
                }
            }
        }
        self.strip_post = post;
        // Effects hearing a track's notes (ADR 0022): each note waits as long as the audio
        // reaching its slot does — the strip's arrival plus the latency of the inserts
        // before it — so note and sound meet at the effect.
        // A listener that was already there keeps its cursor (`Self::listen`).
        self.listeners_prev.clear();
        for i in 0..self.listener_count {
            if let Some(l) = nodes.listener(i) {
                self.listeners_prev.push(*l);
            }
        }
        self.listener_count = 0;
        self.listened = [false; MAX_TRACKS];
        for slot in 0..MAX_STRIPS {
            if self.strip_ids[slot].is_some() {
                if let Some(mut st) = nodes.strip(slot) {
                    st.listener = [None; MAX_EFFECTS_PER_TRACK];
                }
            }
        }
        for oi in 0..self.strip_play_order.len() {
            let slot = self.strip_play_order[oi];
            let mut ahead = arrive[slot] as u64;
            let mut inserts = [(EffectId(0), 0usize, None); MAX_EFFECTS_PER_TRACK];
            let count = match nodes.strip(slot) {
                // A resting strip's inserts do not run: they hear no notes.
                Some(st) if st.rest => 0,
                Some(st) => {
                    for (i, e) in st.effects.iter().enumerate() {
                        let latency = if st.ll_off[i] { 0 } else { e.latency() };
                        inserts[i] = (e.id, latency, st.midi_id[i]);
                    }
                    st.effects.len()
                }
                None => 0,
            };
            for (i, &(effect, latency, midi)) in inserts.iter().enumerate().take(count) {
                if let Some(track) = midi.and_then(|id| self.slot_for(id)) {
                    let l = self.listen(track, effect, ahead);
                    if let Some(mut st) = nodes.strip(slot) {
                        st.listener[i] = l;
                    }
                }
                ahead += latency as u64;
            }
        }
        // The master chain delays every path equally: it moves what is heard, not the mix.
        let heard = master + self.master.latency();
        let mut kept = self.pdc.begin();
        for ti in 0..self.track_to_slot.len() {
            let slot = self.track_to_slot[ti];
            let want = match feeds[slot] {
                // Gathered by nobody: no line.
                _ if rest_t[slot] => continue,
                Some(d) if deaf[d] => continue,
                // Monitored, not held back (ADR 0064 §4).
                f if quick(slot, f) => leave[slot],
                Some(d) => arrive[d],
                None => master,
            };
            self.pdc
                .want(&mut kept, Edge::Track(slot), want - leave[slot]);
        }
        for oi in 0..self.strip_play_order.len() {
            let slot = self.strip_play_order[oi];
            if rest_s[slot] {
                continue;
            }
            let Some(st) = nodes.strip(slot) else {
                continue;
            };
            let pure = mon.pure[slot];
            let target = match st.output {
                Some(d) if deaf[d] => None,
                Some(d) if pure && !mon.pure[d] => Some(arrive[d]),
                _ if pure => Some(post[slot]),
                Some(d) => Some(arrive[d]),
                None => Some(master),
            };
            let sends = st.sends;
            let send_count = st.send_count;
            // Every latent insert has a dry line of its latency, always running, so its
            // slot mix, its bypass and the chain switch all meet a dry signal already in
            // step with the effect — switching them re-plans nothing and nothing jumps.
            let mut dry = [0usize; MAX_EFFECTS_PER_TRACK];
            for (i, e) in st.effects.iter().enumerate() {
                // Bypassed hard for monitoring: no dry line (ADR 0064 §4).
                dry[i] = if st.ll_off[i] { 0 } else { e.latency() };
            }
            drop(st);
            if let Some(target) = target {
                self.pdc
                    .want(&mut kept, Edge::Out(slot), target - post[slot]);
            }
            for (k, (d, _)) in sends.iter().take(send_count).enumerate() {
                if !deaf[*d] {
                    let want = if pure && mon.pure[*d] {
                        0
                    } else {
                        arrive[*d] - post[slot]
                    };
                    self.pdc.want(&mut kept, Edge::Send(slot, k), want);
                }
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
        if self.click_on() {
            self.pdc.want(&mut kept, Edge::Click, heard);
        }
        self.pdc.finish(&kept);
        // Resolve the assignment once, into each node's own state, so the render loop
        // reads its own fields.
        for slot in 0..MAX_TRACKS {
            if self.track_ids[slot].is_some() {
                if let Some(mut s) = nodes.track(slot) {
                    s.line = None;
                    s.listened = self.listened[slot];
                }
            }
        }
        for slot in 0..MAX_STRIPS {
            if self.strip_ids[slot].is_some() {
                if let Some(mut s) = nodes.strip(slot) {
                    s.out_line = None;
                    s.send_line = [None; MAX_SENDS_PER_STRIP];
                    s.dry_line = [None; MAX_EFFECTS_PER_TRACK];
                }
            }
        }
        self.click_line = None;
        self.capture_line = None;
        self.master.dry_line = [None; MAX_EFFECTS_PER_TRACK];
        for i in 0..crate::pdc::POOL {
            match self.pdc.edge_of(i) {
                Some(Edge::Track(s)) => {
                    if let Some(mut t) = nodes.track(s) {
                        t.line = Some(i);
                    }
                }
                Some(Edge::Out(s)) => {
                    if let Some(mut st) = nodes.strip(s) {
                        st.out_line = Some(i);
                    }
                }
                Some(Edge::Send(s, k)) => {
                    if let Some(mut st) = nodes.strip(s) {
                        st.send_line[k] = Some(i);
                    }
                }
                Some(Edge::Dry(s, k)) => {
                    if let Some(mut st) = nodes.strip(s) {
                        st.dry_line[k] = Some(i);
                    }
                }
                Some(Edge::Click) => self.click_line = Some(i),
                Some(Edge::Capture) => self.capture_line = Some(i),
                Some(Edge::MasterDry(k)) => self.master.dry_line[k] = Some(i),
                None => {}
            }
        }
        self.monitored_wait(
            &mon,
            &super::monitor::Arrivals {
                arrive: &arrive,
                post: &post,
                own: &own,
                master,
                leave: &leave,
                feeds: &feeds,
                deaf: &deaf,
                rest_s: &rest_s,
                rest_t: &rest_t,
            },
        );
        self.pdc.total = heard;
        self.stats.latency_samples = heard as u64;
        self.stats.pdc_unserved = self.pdc.unserved as u64;
        if let Some((count, delay)) = self.pdc.take_request() {
            if self.tx.push(EngineEvent::PdcNeed { count, delay }).is_err() {
                self.pdc.forget_request();
            }
        }
        // A send that gained or lost a line is gathered another way.
        self.map_dirty = true;
        self.latency_sig = self.latency_signature();
        self.scope_delays();
    }

    /// The latencies compensation is planned from — every live strip's inserts, every
    /// track's instrument, the master's chain — folded into one number. Cheap (a call per
    /// insert); allocates nothing.
    pub(super) fn latency_signature(&self) -> u64 {
        let nodes = &self.nodes;
        let mut h = 0xcbf2_9ce4_8422_2325u64;
        let mut mix = |v: usize| {
            h ^= (v as u64).wrapping_add(1);
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        };
        for slot in 0..MAX_STRIPS {
            if self.strip_ids[slot].is_none() {
                continue;
            }
            mix(slot);
            if let Some(st) = nodes.strip(slot) {
                for e in &st.effects {
                    mix(e.latency());
                }
            }
        }
        for &slot in &self.track_to_slot {
            if let Some(t) = nodes.track(slot) {
                mix(t.latency());
            }
        }
        // The master chain insert by insert, as the strips: each has its own dry line.
        mix(usize::MAX);
        for e in &self.master.effects {
            mix(e.latency());
        }
        h
    }

    /// How late each watched Speculum's signal is against the transport — the compensation
    /// its slot waits: a strip's arrival and the latency of the inserts before it, or on the
    /// master the mix's arrival and the master inserts before it (ADR 0056, review 1). Its
    /// beat stamps are moved back by as much, so the beat-synced scope is on the beat it
    /// hears. Worked out with each plan and each change of what is watched.
    pub(super) fn scope_delays(&mut self) {
        // How late each insert's audio is against the transport: its strip's arrival
        // (compensation) and the inserts before it. Every insert hears the song position
        // that much earlier (`EffectUnit::late`: a synced LFO stays on the beat it shapes,
        // integration review); a watched Speculum dates its frames by it.
        let mut out = [0usize; crate::fx::speculum::MAX_SCOPES];
        let mut lates = std::mem::take(&mut self.lates);
        let mut follow = std::mem::take(&mut self.follow_lates);
        let (mut n, mut f) = (0usize, 0usize);
        // Every insert's lateness (its own wrap after a loop's), and the ones that follow
        // the song apart (their tempo steps, review 2).
        let mut keep = |l: usize, follows: bool| {
            if l > 0 && n < lates.len() {
                lates[n] = l;
                n += 1;
            }
            if l > 0 && follows && f < follow.len() {
                follow[f] = l;
                f += 1;
            }
        };
        let scoped = self.scoped;
        let at = |id: EffectId| scoped.iter().position(|s| *s == Some(id));
        let nodes = self.nodes.clone();
        for slot in 0..MAX_STRIPS {
            if self.strip_ids[slot].is_none() {
                continue;
            }
            if let Some(mut st) = nodes.strip(slot) {
                // An insert bypassed hard for monitoring delays nothing (review 1).
                let StripSlot {
                    effects, ll_off, ..
                } = &mut *st;
                let latency = |i: usize, e: &crate::effects::EffectUnit| {
                    if ll_off[i] {
                        0
                    } else {
                        e.latency()
                    }
                };
                let own: usize = effects.iter().enumerate().map(|(i, e)| latency(i, e)).sum();
                let mut ahead = self.strip_post[slot].saturating_sub(own);
                for (i, e) in effects.iter_mut().enumerate() {
                    e.late = ahead;
                    keep(ahead, e.follows_song() || at(e.id).is_some());
                    if let Some(k) = at(e.id) {
                        out[k] = ahead;
                    }
                    ahead += latency(i, e);
                }
            }
        }
        let mut ahead = self.pdc.total.saturating_sub(self.master.latency());
        let master_off = self.master.ll_off;
        for (i, e) in self.master.effects.iter_mut().enumerate() {
            e.late = ahead;
            keep(ahead, e.follows_song() || at(e.id).is_some());
            if let Some(k) = at(e.id) {
                out[k] = ahead;
            }
            if !master_off[i] {
                ahead += e.latency();
            }
        }
        self.scoped_delay = out;
        // Distinct, ascending (in place: no allocation).
        self.late_n = distinct(&mut lates[..n]);
        self.follow_n = distinct(&mut follow[..f]);
        self.lates = lates;
        self.follow_lates = follow;
    }

    /// Plan compensation again when a latency moved without an edit — automation or a
    /// modulator switching an oversampling or a look-ahead (ADR 0056: a latency is a
    /// configuration's, and the configuration may move while the song plays). Called
    /// before each pass, after the pass's automation is applied.
    pub(super) fn replan_if_latency_moved(&mut self) {
        if self.latency_signature() != self.latency_sig {
            self.compensate();
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
    pub(super) fn replace_effect(&mut self, mut unit: Box<EffectUnit>) {
        let snapshot = self.snapshot.clone();
        if let Some(spec) = snapshot
            .as_ref()
            .and_then(|s| s.effect_slots().find(|e| e.id == unit.id).map(|e| &e.spec))
        {
            unit.configure(spec);
        }
        let nodes = self.nodes.clone();
        for slot in 0..MAX_STRIPS {
            if self.strip_ids[slot].is_none() {
                continue;
            }
            let Some(mut st) = nodes.strip(slot) else {
                continue;
            };
            if let Some(i) = st.effects.iter().position(|e| e.id == unit.id) {
                let mut old = std::mem::replace(&mut st.effects[i], unit);
                drop(st);
                old.replaced = true;
                self.dispose_effect(old);
                self.auto_unit_swapped();
                self.compensate();
                return;
            }
        }
        if let Some(i) = self.master.effects.iter().position(|e| e.id == unit.id) {
            let mut old = std::mem::replace(&mut self.master.effects[i], unit);
            old.replaced = true;
            self.dispose_effect(old);
            self.auto_unit_swapped();
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
    pub(super) fn song_position(&self) -> crate::fx::SongPosition {
        song_position_of(&self.transport, &self.snapshot, self.sample_rate)
    }

    /// Give a slot the native unit its track now needs (`kind`), or none. A unit already of
    /// the right kind stays, so voices ring across snapshots; otherwise the one delivered
    /// for this track and kind is taken from the incoming list and the old one goes back.
    fn swap_native(&mut self, slot: usize, track: TrackId, kind: Option<UnitKind>) {
        let nodes = self.nodes.clone();
        let Some(mut s) = nodes.track(slot) else {
            return;
        };
        let have = s.native.as_ref().map(|n| (n.track, n.kind));
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
            Some(u) => s.native.replace(u),
            None => s.native.take(),
        };
        drop(s);
        if let Some(old) = old {
            self.dispose_instrument(old);
        }
    }

    /// A unit in place of the one running for its track and kind (ADR 0028): swapped in
    /// now, keeping the slot; or, when no slot runs that track and kind, waiting with the
    /// new ones in place of any waiting there.
    pub(super) fn replace_instrument(&mut self, unit: Box<NativeUnit>) {
        let (track, kind) = (unit.track, unit.kind);
        let nodes = self.nodes.clone();
        if let Some(slot) = self.slot_for(track) {
            if let Some(mut s) = nodes.track(slot) {
                if s.native.as_ref().is_some_and(|n| n.kind == kind) {
                    let old = s.native.replace(unit);
                    drop(s);
                    if let Some(old) = old {
                        self.dispose_instrument(old);
                    }
                    self.auto_unit_swapped();
                    self.compensate();
                    return;
                }
            }
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

    pub(super) fn dispose_instrument(&mut self, mut unit: Box<NativeUnit>) {
        // It will not be processed again: a hosted instrument stops processing here, on
        // the audio thread, before it leaves (ADR 0028).
        unit.unit.stop();
        if self.tx.push(EngineEvent::GarbageInstrument(unit)).is_err() {
            self.stats.dropped_events += 1;
        }
    }

    pub(super) fn dispose_effect(&mut self, mut unit: Box<EffectUnit>) {
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
    pub(super) fn give_back_preview(&mut self, s: Arc<cantus_core::SampleData>) {
        if self.tx.push(EngineEvent::GarbagePreview(s)).is_err() {
            self.stats.dropped_events += 1;
        }
    }

    pub(super) fn retry_garbage(&mut self) {
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
    pub(super) fn reseek_events(&mut self) {
        let pos = self.transport.position_f64();
        self.reseek_events_from(pos);
    }

    /// [`Self::reseek_events`] from `pos` (ticks): after a loop's wrap, the loop's start —
    /// the playhead sits a fraction of a sample past it (the overshoot carried), and what
    /// lies exactly on the start is due now, on every pass (round 29: it was skipped).
    pub(super) fn reseek_events_from(&mut self, pos: f64) {
        let tick = cantus_core::Ticks(pos.ceil() as i64);
        if let Some(snap) = &self.snapshot {
            for (ti, t) in snap
                .tracks
                .iter()
                .enumerate()
                .take(self.track_to_slot.len())
            {
                let slot = self.track_to_slot[ti];
                if let Some(mut s) = self.nodes.track(slot) {
                    s.next_event = t.first_event_at_or_after(tick);
                }
            }
            self.next_beat_tick = if snap.tempo.is_one_metre() {
                let beat = snap.time_signature.ticks_per_beat().0 as f64;
                (pos / beat).ceil() * beat
            } else {
                // Through every metre change (ADR 0060): the first beat line from here.
                snap.tempo.next_beat(pos).0
            };
        }
        self.auto_reseek();
    }

    pub(super) fn release_all(&mut self) {
        self.auto_live_release();
        for slot in 0..MAX_TRACKS {
            if self.track_ids[slot].is_some() {
                if let Some(mut s) = self.nodes.track(slot) {
                    s.all_notes_off();
                }
            }
        }
        // And every effect listening to a track hears its notes stop (ADR 0022).
        let listened = self.listened;
        for (track, on) in listened.iter().enumerate() {
            if *on {
                self.notes_to_fx(track, self.note_at, crate::fx::FxNote::AllOff);
            }
        }
    }
}

/// Sort `v` and keep each value once at its front, in place (no allocation): how many.
fn distinct(v: &mut [usize]) -> usize {
    v.sort_unstable();
    let mut d = 0;
    for i in 0..v.len() {
        if d == 0 || v[i] != v[d - 1] {
            v[d] = v[i];
            d += 1;
        }
    }
    d
}

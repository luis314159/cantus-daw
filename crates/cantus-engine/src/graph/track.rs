//! A **track node** (ADR 0031 §1.1): the slot's instrument, its event cursor, its audio
//! clips and monitored input, its level and balance, its delay line; it writes its
//! **output** for the pass (what its strip, or the master, gathers), its peak, its stem
//! and the notes it plays into its outbox.
//!
//! A track splits its pass only at **its own** events: the instrument receives each note
//! between two `process` calls at exactly the sample the serial engine gave it, and its
//! calls are no longer also cut at every other track's events — a split every native unit
//! is required to be indifferent to (`inst_contract.rs`).
//!
//! It runs on whichever thread of the pool takes it (`graph/exec.rs`): its state, output
//! and outbox are its own cells, and everything else it reads is the pass's frame.

use super::ctx::PassCtx;
use super::exec::{Nodes, Outgoing, PoolPar, PushEvent, Scratch};
use super::*;
use crate::automation::Ramp;
use crate::profile::Node;
use std::sync::atomic::Ordering::Relaxed;

/// A track slot's state: its instruments, its place in its events, its channel, and what
/// the control phase resolved for it (its line, whether effects hear it, its input).
pub(crate) struct TrackSlot {
    pub id: Option<TrackId>,
    pub instrument: SlotInstrument,
    pub synth: Firmus,
    pub drums: Pulsus,
    /// The native instrument's unit, delivered by `AddInstrument`; `None` while the track
    /// hosts Firmus or Pulsus. Unlike those two it is not preallocated per slot: a native
    /// instrument can be large (a sampler, a piano), so it is built off the audio thread
    /// and swapped in.
    pub native: Option<Box<NativeUnit>>,
    /// Index into the snapshot track's events of the next event to fire.
    pub next_event: usize,
    /// Index into the snapshot track's controller events of the next to fire (ADR 0068).
    pub next_control: usize,
    pub next_expr: usize, // r33:mpe — the next per-note expression event (ADR 0080)
    pub gain: f32,
    pub pan: f32,
    pub audible: bool,
    /// The strip slot this track feeds; `None` is the master.
    pub strip: Option<usize>,
    /// The block's peak, and the block it belongs to.
    pub peak: f32,
    pub peak_block: u64,
    /// Some insert hears this track's notes (ADR 0022): they go into its outbox.
    pub listened: bool,
    /// Its compensation line (`Edge::Track`).
    pub line: Option<usize>,
    /// Its input (ADR 0025).
    pub input: rec::TrackInputSlot,
    /// Automated level and balance ramping (ADR 0027).
    pub gain_ramp: Option<Ramp>,
    pub pan_ramp: Option<Ramp>,
    /// Frozen on its own (ADR 0064): the render it plays in place of its instrument and
    /// clips, and where its reader is.
    pub frozen: Option<Arc<crate::freeze::FrozenHandle>>,
    /// Frozen but played live for the moment (a solo, review 3): its render, whose board it
    /// keeps at the playhead without reading it.
    pub parked: Option<Arc<crate::freeze::FrozenHandle>>,
    pub frozen_place: crate::freeze::ReadPlace,
    /// It rests (ADR 0064): not run.
    pub rest: bool,
    /// It rests but for its notes (ADR 0064, review 2): its events go on to what hears them;
    /// its instrument is not played, its output is silence.
    pub notes_only: bool,
    /// The delay it keeps before its level (`Track::lateness`, schema 38), at the engine's
    /// rate: its source goes through its line ([`Self::source_line`]) and it is reported as
    /// its own (review 3: the audio itself stays on the grid).
    pub lateness: usize,
    /// Its note effects (ADR 0069), built with the slot: consulted only while a stage is
    /// on. `None` only for the instant its outputs are being played.
    pub notefx: Option<Box<crate::notefx::Chain>>,
    /// The line that delays its source by `lateness` (`Edge::Source`).
    pub source_line: Option<usize>,
}

impl TrackSlot {
    pub fn new(sample_rate: f32) -> Self {
        TrackSlot {
            id: None,
            instrument: SlotInstrument::Firmus,
            synth: Firmus::new(sample_rate),
            drums: Pulsus::new(sample_rate),
            native: None,
            next_event: 0,
            next_control: 0,
            next_expr: 0, // r33:mpe
            gain: 1.0,
            pan: 0.0,
            audible: true,
            strip: None,
            peak: 0.0,
            peak_block: 0,
            listened: false,
            line: None,
            input: rec::TrackInputSlot::default(),
            gain_ramp: None,
            pan_ramp: None,
            frozen: None,
            parked: None,
            frozen_place: crate::freeze::ReadPlace::default(),
            rest: false,
            notes_only: false,
            lateness: 0,
            source_line: None,
            notefx: Some(Box::new(crate::notefx::Chain::new())),
        }
    }

    pub fn note_on(&mut self, pitch: u8, velocity: u8, tag: u64) {
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

    /// A clip's note-on with its ordinal on the track ([`crate::inst::InstrumentUnit::note_on_seq`]).
    pub fn note_on_seq(&mut self, pitch: u8, velocity: u8, tag: u64, ordinal: u32) {
        match self.instrument {
            SlotInstrument::Native => {
                if let Some(n) = self.native.as_mut() {
                    n.unit.note_on_seq(
                        NoteOn {
                            key: pitch,
                            velocity: velocity as f32 / 127.0,
                            id: native_id(pitch, tag),
                            channel: 0,
                        },
                        ordinal,
                    );
                }
            }
            _ => self.note_on(pitch, velocity, tag),
        }
    }

    /// Release a note; `release` is its release velocity (0 none given, ADR 0068).
    pub fn note_off(&mut self, pitch: u8, tag: u64, release: u8) {
        match self.instrument {
            SlotInstrument::Firmus => self.synth.note_off(pitch, tag),
            SlotInstrument::Pulsus => self.drums.note_off(pitch, tag),
            SlotInstrument::Native => {
                if let Some(n) = self.native.as_mut() {
                    n.unit
                        .note_off(pitch, native_id(pitch, tag), f32::from(release) / 127.0);
                }
            }
        }
    }

    /// A channel-wide controller from live MIDI. Firmus reads none; Pulsus reads its
    /// hi-hat pedal (a plain CC, CC4 by default).
    pub fn controller(&mut self, controller: Controller, value: f32) {
        if let (SlotInstrument::Pulsus, Controller::Cc(n)) = (self.instrument, controller) {
            self.drums.controller(n, value);
        }
        if let (SlotInstrument::Native, Some(n)) = (self.instrument, self.native.as_mut()) {
            n.unit.controller(controller, value);
        }
    }

    /// A live key's expression (polyphonic pressure, round 29): the note the live key
    /// sounds, by its id. Firmus and Pulsus read none.
    pub fn expression_live(
        &mut self,
        pitch: u8,
        expression: crate::inst::NoteExpression,
        value: f32,
    ) {
        if let (SlotInstrument::Native, Some(n)) = (self.instrument, self.native.as_mut()) {
            n.unit.expression(
                native_id(pitch, crate::processor::LIVE_TAG),
                expression,
                value,
            );
        }
    }

    /// A live program change (round 29): a native unit's to take or leave.
    pub fn program(&mut self, program: u8) {
        if let (SlotInstrument::Native, Some(n)) = (self.instrument, self.native.as_mut()) {
            n.unit.program(program);
        }
    }

    /// Every note stops: the instrument's, and what its note effects hold or schedule
    /// (ADR 0069).
    pub fn all_notes_off(&mut self) {
        self.instrument_notes_off();
        if let Some(c) = self.notefx.as_mut() {
            c.reset();
        }
    }

    /// The instrument's notes stop (its note effects keep what they hold).
    pub fn instrument_notes_off(&mut self) {
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
    pub fn reset(&mut self) {
        self.synth.reset();
        self.drums.reset();
        if let Some(n) = self.native.as_mut() {
            n.unit.reset();
        }
        if let Some(c) = self.notefx.as_mut() {
            c.reset();
        }
    }

    // --- note effects (round 31, ADR 0069) ------------------------------------------------

    /// Whether its notes go through its note effects (some stage is on).
    #[inline]
    pub fn fx_active(&self) -> bool {
        self.notefx.as_ref().is_some_and(|c| c.is_active())
    }

    /// The snapshot's chain and key. A chain that changed shape starts afresh, and what it
    /// was sounding is let go on the instrument.
    pub fn configure_notefx(
        &mut self,
        chain: &[cantus_core::notefx::NoteEffect],
        key: Option<cantus_core::Key>,
    ) {
        let flush = self
            .notefx
            .as_mut()
            .is_some_and(|c| c.configure(chain, key));
        if flush {
            self.instrument_notes_off();
        }
    }

    /// One parameter of a note effect (automation, a modulator).
    pub fn set_notefx_param(&mut self, stage: usize, param: usize, v: f32) {
        if let Some(c) = self.notefx.as_mut() {
            c.set_param(stage, param, v);
        }
    }

    /// Through its note effects: `input` (if any) at `now`, then whatever `is_due` says is
    /// due; each note out of them played on the instrument (unless frozen or resting but
    /// for its notes) and handed to `heard` (the inserts that listen). `tick` is where the
    /// song is while stopped (rolling, each note is at its own tick),
    /// for a kit's seeded hits. Returns the outputs that found no room.
    pub fn fx_play(
        &mut self,
        now: crate::notefx::Now,
        input: Option<(crate::notefx::Now, crate::notefx::Msg)>,
        is_due: impl Fn(crate::notefx::Due) -> bool,
        tick: i64,
        mut heard: impl FnMut(&crate::notefx::Msg),
    ) -> u64 {
        let Some(mut chain) = self.notefx.take() else {
            return 0;
        };
        if let Some((at, m)) = input {
            chain.input(at, m);
        }
        chain.run_due(now, is_due);
        let plays = self.frozen.is_none() && !self.notes_only && !self.rest;
        for (at, m) in chain.take() {
            let tick = if now.rolling { at.round() as i64 } else { tick };
            if plays {
                self.sound(m, tick);
            }
            heard(m);
        }
        let dropped = chain.take_dropped();
        self.notefx = Some(chain);
        dropped
    }

    /// The next time its note effects need the track to stop and play (an arpeggio's step
    /// or note-off).
    pub fn fx_next_due(&self) -> Option<crate::notefx::Due> {
        self.notefx.as_ref().and_then(|c| c.next_due())
    }

    /// A note out of the note effects, on the instrument: a clip's as a sequenced note (a
    /// kit's seeded hit), a live one as live.
    pub fn sound(&mut self, m: &crate::notefx::Msg, tick: i64) {
        if !m.on {
            // r31 automation: the release velocity rides with the note (`m.release`).
            self.note_off(m.pitch, m.tag, m.release);
        } else if m.live {
            self.note_on(m.pitch, m.velocity, m.tag);
        } else if self.instrument == SlotInstrument::Pulsus {
            let hit = cantus_dsp::pulsus::Hit {
                id: m.tag,
                tick,
                ordinal: m.ordinal,
                sequenced: true,
            };
            self.drums.trigger(m.pitch, m.velocity, hit);
        } else {
            self.note_on_seq(m.pitch, m.velocity, m.tag, m.ordinal);
        }
    }
    pub fn active_voices(&self) -> usize {
        match self.instrument {
            SlotInstrument::Firmus => self.synth.active_voices(),
            SlotInstrument::Pulsus => self.drums.active_voices(),
            SlotInstrument::Native => self.native.as_ref().map_or(0, |n| n.unit.active_voices()),
        }
    }

    /// The instrument's latency, which the track leaves at (ADR 0028) — while it plays a
    /// frozen render, the latency the render was made with (ADR 0064).
    pub fn latency(&self) -> usize {
        if let Some(h) = self.frozen.as_ref() {
            return h.latency;
        }
        // The delay it keeps (a bounce's, schema 38) and its instrument's.
        self.lateness
            + match self.instrument {
                SlotInstrument::Native => self.native.as_ref().map_or(0, |n| n.unit.latency()),
                SlotInstrument::Firmus | SlotInstrument::Pulsus => 0,
            }
    }
}

/// A track slot's output for the current pass: the leveled (and, with compensation,
/// delayed) signal its strip or the master adds as it is. `active` is false when the track
/// was idle the whole pass — nothing to add.
#[derive(Clone)]
pub(crate) struct TrackOut {
    pub active: bool,
    pub l: Vec<f32>,
    pub r: Vec<f32>,
}

impl TrackOut {
    pub fn new() -> Self {
        TrackOut {
            active: false,
            l: vec![0.0; PASS_FRAMES],
            r: vec![0.0; PASS_FRAMES],
        }
    }
}

/// Run snapshot track `ti` (in slot `slot`) over the pass: its events fire at their
/// samples, and the instrument renders between them.
pub(crate) fn run_track(nodes: &Nodes, ctx: &PassCtx, ti: usize, slot: usize, sc: &mut Scratch) {
    sc.prof.enter(Node::Track(slot));
    let cells = &nodes.tracks[slot];
    let Some(mut s) = nodes.track(slot) else {
        return;
    };
    let Some(mut out) = cells.out.try_borrow_mut() else {
        nodes.conflict();
        return;
    };
    out.active = false;
    if s.peak_block != ctx.block {
        s.peak = 0.0;
        s.peak_block = ctx.block;
    }
    let track = ctx.snapshot.as_deref().and_then(|snap| snap.tracks.get(ti));
    if let Some(track) = track {
        let mut outbox = if s.listened {
            let o = cells.outbox.try_borrow_mut();
            if o.is_none() {
                nodes.conflict();
            }
            o
        } else {
            None
        };
        let (n, p0) = (ctx.n, ctx.p0);
        let playing = ctx.transport.is_playing();
        // Its note effects are consulted only while one is on (ADR 0069).
        let fx = s.fx_active();
        let base = ctx.clock + p0 as u64;
        let mut sub = 0usize;
        while sub < n {
            let mut end = n;
            if playing {
                // The clips' controller lanes (ADR 0068), on the same clock as the notes.
                let ctl_off = |c: &cantus_core::CtlEvent| {
                    (ctx.transport.samples_until(c.tick.0 as f64) as usize).saturating_sub(sub)
                };
                while let Some(ev) = track.events.get(s.next_event) {
                    // Counted from the transport's integer sample, so the same at any split.
                    let off = (ctx.transport.samples_until(ev.tick.0 as f64) as usize)
                        .saturating_sub(sub);
                    if off == 0 {
                        // Controllers due now go before a note-on due now (ADR 0068).
                        if ev.is_on() && !track.controls.is_empty() {
                            super::track_controls::due(&mut s, track, &ctl_off);
                        }
                        // Into its note effects (ADR 0069), unless the clip is baked: what
                        // they sound is played after this sample's events, below.
                        if fx && !track.direct.get(s.next_event).copied().unwrap_or(false) {
                            let now = crate::notefx::play::now_at(&ctx.transport, base, sub);
                            let input = (
                                crate::notefx::play::at_event(now, ev),
                                crate::notefx::play::msg_of(ev),
                            );
                            fx_step(nodes, ctx, &mut s, &mut outbox, base, sub, Some(input));
                            s.next_event += 1;
                            continue;
                        }
                        // Frozen (ADR 0064), the instrument is not played: its render is.
                        // The notes still go to the inserts that hear them — and only
                        // they, for a track resting but for its notes (review 2).
                        if s.frozen.is_none() && !s.notes_only {
                            fire(&mut s, ev);
                        }
                        s.next_event += 1;
                        if let Some(ob) = outbox.as_mut() {
                            let at = ctx.clock + (p0 + sub) as u64;
                            if !ob.push(at, fx_note(ev)) {
                                nodes.drop_event();
                            }
                        }
                    } else {
                        end = end.min(sub + off);
                        break;
                    }
                }
                if !track.controls.is_empty() {
                    if let Some(d) = super::track_controls::due(&mut s, track, &ctl_off) {
                        end = end.min(sub + d);
                    }
                }
            }
            // Its note effects (ADR 0069): what they have due here, and the track stops
            // again at their next step or note-off.
            if fx {
                fx_step(nodes, ctx, &mut s, &mut outbox, base, sub, None);
                if let Some(o) = s
                    .fx_next_due()
                    .and_then(|d| crate::notefx::play::offset_of(&ctx.transport, base, d))
                {
                    if o > sub {
                        end = end.min(o);
                    }
                }
            }
            // r33:mpe — per-note expression, last before the segment (ADR 0080)
            if playing && !track.expr.is_empty() {
                end = crate::inst::expr_play::due(&mut s, track, &ctx.transport, sub, end);
            }
            if !s.notes_only {
                track_segment(nodes, ctx, track, slot, &mut s, &mut out, sc, sub, end);
            }
            sub = end;
        }
    }
    nodes.track_peak[slot].store(s.peak.to_bits(), Relaxed);
    let voices = s.active_voices();
    nodes.track_voices[slot].store(voices as u32, Relaxed);
    // Divisible only while there is something to split: two voices or more on a unit that
    // splits them. Sticky until the next clustering reads it (`rebuild_map` clears it), so
    // a chord between two clusterings still counts (ADR 0038 §6).
    if voices >= 2
        && matches!(s.instrument, SlotInstrument::Native)
        && s.native.as_ref().is_some_and(|n| n.unit.splits_voices())
    {
        nodes.track_splits[slot].store(true, Relaxed);
    }
}

/// One call into the track's note effects at offset `sub` of the pass (ADR 0069): an input
/// (a clip event; nothing is fired with it, so every event of a sample is in before a step
/// there plays), or what is due by `sub`. What comes out is played and goes to the inserts
/// that hear the track, at this sample.
#[allow(clippy::too_many_arguments)]
fn fx_step<O: std::ops::DerefMut<Target = super::super::graph::notes::Outbox>>(
    nodes: &Nodes,
    ctx: &PassCtx,
    s: &mut TrackSlot,
    outbox: &mut Option<O>,
    base: u64,
    sub: usize,
    input: Option<(crate::notefx::Now, crate::notefx::Msg)>,
) {
    use crate::notefx::play;
    let t = &ctx.transport;
    let now = play::now_at(t, base, sub);
    let firing = input.is_none();
    let at = base + sub as u64;
    let lost = s.fx_play(
        now,
        input,
        |d| firing && play::is_due(t, base, sub, d),
        t.position_at(t.samples() + sub as u64).round() as i64,
        |m| {
            if let Some(ob) = outbox.as_mut() {
                if !ob.push(at, play::fx_note(m)) {
                    nodes.drop_event();
                }
            }
        },
    );
    if lost > 0 {
        nodes.dropped.fetch_add(lost, Relaxed);
    }
}

/// The track's own segment `[sub, end)` of the pass: instrument, clips and input, level
/// and balance, line, output.
#[allow(clippy::too_many_arguments)]
fn track_segment(
    nodes: &Nodes,
    ctx: &PassCtx,
    track: &cantus_core::TrackSnapshot,
    slot: usize,
    s: &mut TrackSlot,
    out: &mut TrackOut,
    sc: &mut Scratch,
    sub: usize,
    end: usize,
) {
    let m = end - sub;
    let done = ctx.p0 + sub;
    // The transport at the segment's first sample. A pass never crosses a loop wrap, so
    // this is the pass's transport moved on, never wrapped.
    let mut transport = ctx.transport.clone();
    transport.advance(sub as u64);
    let Scratch {
        mono,
        buf_l,
        buf_r,
        events,
        inter,
        ..
    } = sc;
    let mono = &mut mono[..m];
    let l = &mut buf_l[..m];
    let r = &mut buf_r[..m];
    // Played live while frozen (review 3): its render's blocks kept at the playhead.
    if s.frozen.is_none() {
        if let Some(h) = s.parked.clone() {
            crate::freeze::point(&h, &mut s.frozen_place, crate::freeze::frame_at(&transport));
        }
    }
    // Frozen on its own (ADR 0064): the render, in place of the instrument, the clips and
    // the input, then the same level and balance as live.
    if let Some(h) = s.frozen.clone() {
        let from = crate::freeze::frame_at(&transport);
        let playing = transport.is_playing();
        crate::freeze::point(&h, &mut s.frozen_place, from);
        let sounding = playing && from + (m as i64) > 0 && from < h.frames as i64;
        if sounding {
            let late = crate::freeze::read(&h, from, l, r, inter, ctx.offline);
            if late > 0 {
                nodes.stream_late.fetch_add(late as u64, Relaxed);
            }
        }
        return track_level(
            nodes, ctx, slot, s, out, l, r, sub, end, &transport, sounding,
        );
    }
    // Firmus is mono and is copied into both channels; Pulsus writes stereo directly,
    // because its pads are panned inside the instrument.
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
            let id = s.id;
            match s.native.as_mut() {
                Some(u) => {
                    // Where the song is at this segment's first sample, for the units
                    // that follow it (a hosted synth's arpeggiator).
                    let position = song_position_of(&transport, &ctx.snapshot, ctx.sample_rate);
                    u.unit.set_position(&position);
                    // A hosted instrument's automation in this segment, at its offsets
                    // into it (ADR 0057).
                    {
                        let crate::inst::NativeUnit {
                            unit, automation, ..
                        } = &mut **u;
                        automation.deliver(sub, end, |o, id, v| unit.automate(o, id, v));
                    }
                    let par = PoolPar {
                        forks: &nodes.forks,
                        on: ctx.split_voices,
                    };
                    let sig = u.unit.process_par(l, r, &par);
                    let late = u.unit.take_late_reads();
                    if late > 0 {
                        nodes.stream_late.fetch_add(late as u64, Relaxed);
                    }
                    // What found no room on its way into it, automated or not (review 2).
                    let lost = u.automation.take_dropped() + u.unit.take_dropped();
                    if lost > 0 {
                        nodes.dropped.fetch_add(lost, Relaxed);
                    }
                    let cut = u.unit.take_cut_notes();
                    if cut > 0 {
                        nodes.stream_cut.fetch_add(cut as u64, Relaxed);
                    }
                    let wakes = u.unit.take_stream_wakes();
                    if wakes > 0 {
                        nodes.stream_wakes.fetch_add(wakes as u64, Relaxed);
                    }
                    if ctx.probed.is_some() && ctx.probed == id {
                        match nodes.probe.try_borrow_mut() {
                            Some(mut p) => *p = Some(u.unit.telemetry()),
                            None => nodes.conflict(),
                        }
                    }
                    // A hosted instrument's own window: its gestures go to the host
                    // (ADR 0028), after the pass.
                    if let Some(track) = id {
                        for event in u.unit.param_events() {
                            if !events.push_event(Outgoing::Inst {
                                track,
                                event: *event,
                            }) {
                                nodes.drop_event();
                            }
                        }
                        u.unit.clear_param_events();
                    }
                    sig
                }
                None => false,
            }
        }
    };
    // Audio clips join the instrument's output, before the channel's level (ADR 0025 §3).
    let mut has_signal = rec::add_track_audio(
        None,
        &s.input,
        Some(track),
        has_signal,
        done,
        l,
        r,
        &transport,
        ctx.recording,
    );
    // The delay the track keeps (a bounce's, review 3): everything its source plays goes
    // through its line; what it monitors does not (heard as soon as it can be).
    if let Some(li) = s.source_line {
        if has_signal || nodes.line_ringing(Some(li)) {
            if !has_signal {
                l.iter_mut().for_each(|x| *x = 0.0);
                r.iter_mut().for_each(|x| *x = 0.0);
            }
            if let Some(mut line) = nodes.line(li) {
                line.run(l, r);
            }
            has_signal = true;
        }
    }
    // The monitored input joins it, before the channel's level (ADR 0025 §4).
    let has_signal = {
        let input = if s.input.has {
            let b = nodes.input.try_borrow();
            if b.is_none() {
                nodes.conflict();
            }
            b
        } else {
            None
        };
        rec::add_track_audio(
            input.as_deref(),
            &s.input,
            None,
            has_signal,
            done,
            l,
            r,
            &transport,
            ctx.recording,
        )
    };
    track_level(
        nodes, ctx, slot, s, out, l, r, sub, end, &transport, has_signal,
    );
}

/// The track's segment after its source: its level and balance, its line, its output. A
/// freeze's or a bounce's render taps the source here (ADR 0064).
#[allow(clippy::too_many_arguments)]
fn track_level(
    nodes: &Nodes,
    ctx: &PassCtx,
    slot: usize,
    s: &mut TrackSlot,
    out: &mut TrackOut,
    l: &mut [f32],
    r: &mut [f32],
    sub: usize,
    end: usize,
    transport: &crate::transport::Transport,
    has_signal: bool,
) {
    let m = end - sub;
    let done = ctx.p0 + sub;
    if ctx.tap {
        match nodes.tap.try_borrow_mut() {
            Some(mut tap) => {
                if let Some(tap) = tap.as_mut() {
                    if has_signal {
                        tap.track_source(slot, done, l, r);
                    }
                }
            }
            None => nodes.conflict(),
        }
    }
    let line = s.line;
    let line_ringing = nodes.line_ringing(line);
    if !has_signal && !line_ringing {
        // Idle: nothing to mix, nothing to meter. What the pass already holds of this
        // track keeps its place; this stretch of it is silence.
        if out.active {
            out.l[sub..end].iter_mut().for_each(|x| *x = 0.0);
            out.r[sub..end].iter_mut().for_each(|x| *x = 0.0);
        }
        return;
    }
    if !out.active {
        out.l[..sub].iter_mut().for_each(|x| *x = 0.0);
        out.r[..sub].iter_mut().for_each(|x| *x = 0.0);
        out.active = true;
    }
    if !has_signal {
        // Idle, but its compensation line still holds what it played.
        l.iter_mut().for_each(|x| *x = 0.0);
        r.iter_mut().for_each(|x| *x = 0.0);
    }
    // The channel's own level and balance — not the pan law, which is applied once,
    // at the strip. At their defaults these are exactly unity.
    let (bl, br) = balance_gains(s.pan);
    let g = if s.audible { s.gain } else { 0.0 };
    let (mut gl, mut gr) = (g * bl, g * br);
    // Automated level or balance ramping (ADR 0027): applied here, sample by sample,
    // and what follows is at unity.
    if let Some(k0) = ctx.k0(transport.samples()) {
        if auto::ramp_track(s, k0, l, r) {
            (gl, gr) = (1.0, 1.0);
        }
    }
    if ctx.tap {
        match nodes.tap.try_borrow_mut() {
            Some(mut tap) => {
                if let Some(tap) = tap.as_mut() {
                    tap.track(slot, done, l, r, gl, gr);
                }
            }
            None => nodes.conflict(),
        }
    }
    // A delayed edge: the level and balance first, then the line; what comes out is
    // added at unity (a multiplication by 1.0 changes nothing).
    if let Some(li) = line {
        for i in 0..m {
            l[i] *= gl;
            r[i] *= gr;
        }
        if let Some(mut line) = nodes.line(li) {
            line.run(l, r);
        }
        (gl, gr) = (1.0, 1.0);
    }
    // The output: exactly the product the serial engine added into its strip.
    let mut peak = s.peak;
    let (ol, or) = (&mut out.l[sub..end], &mut out.r[sub..end]);
    for i in 0..m {
        let sl = l[i] * gl;
        let sr = r[i] * gr;
        peak = peak.max(sl.abs()).max(sr.abs());
        ol[i] = sl;
        or[i] = sr;
    }
    s.peak = peak;
    // Into the master: the stems' Direct sum, as the master takes it (review 3).
    if ctx.tap && s.strip.is_none() {
        match nodes.tap.try_borrow_mut() {
            Some(mut tap) => {
                if let Some(tap) = tap.as_mut() {
                    tap.direct(done, ol, or);
                }
            }
            None => nodes.conflict(),
        }
    }
}

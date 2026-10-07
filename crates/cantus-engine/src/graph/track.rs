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

use super::exec::{Nodes, Outgoing, PushEvent, Scratch};
use super::*;
use crate::automation::Ramp;
use super::ctx::PassCtx;
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

    pub fn note_off(&mut self, pitch: u8, tag: u64) {
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
    pub fn controller(&mut self, controller: Controller, value: f32) {
        if let (SlotInstrument::Pulsus, Controller::Cc(n)) = (self.instrument, controller) {
            self.drums.controller(n, value);
        }
        if let (SlotInstrument::Native, Some(n)) = (self.instrument, self.native.as_mut()) {
            n.unit.controller(controller, value);
        }
    }

    pub fn all_notes_off(&mut self) {
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
    }

    pub fn active_voices(&self) -> usize {
        match self.instrument {
            SlotInstrument::Firmus => self.synth.active_voices(),
            SlotInstrument::Pulsus => self.drums.active_voices(),
            SlotInstrument::Native => self.native.as_ref().map_or(0, |n| n.unit.active_voices()),
        }
    }

    /// The instrument's latency, which the track leaves at (ADR 0028).
    pub fn latency(&self) -> usize {
        match self.instrument {
            SlotInstrument::Native => self.native.as_ref().map_or(0, |n| n.unit.latency()),
            SlotInstrument::Firmus | SlotInstrument::Pulsus => 0,
        }
    }
}

/// A track slot's output for the current pass: the leveled (and, with compensation,
/// delayed) signal its strip or the master adds as it is. `active` is false when the track
/// was idle the whole pass — nothing to add.
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
        let mut sub = 0usize;
        while sub < n {
            let mut end = n;
            if playing {
                loop {
                    let Some(ev) = track.events.get(s.next_event) else {
                        break;
                    };
                    // Counted from the transport's integer sample, so the same at any split.
                    let off = (ctx.transport.samples_until(ev.tick.0 as f64) as usize)
                        .saturating_sub(sub);
                    if off == 0 {
                        fire(&mut s, ev);
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
            }
            track_segment(nodes, ctx, track, slot, &mut s, &mut out, sc, sub, end);
            sub = end;
        }
    }
    nodes.track_peak[slot].store(s.peak.to_bits(), Relaxed);
    nodes.track_voices[slot].store(s.active_voices() as u32, Relaxed);
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
        ..
    } = sc;
    let mono = &mut mono[..m];
    let l = &mut buf_l[..m];
    let r = &mut buf_r[..m];
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
                    let sig = u.unit.process(l, r);
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
    // Audio clips and the monitored input join the instrument's output, before the
    // channel's level (ADR 0025 §3, §4).
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
            Some(track),
            has_signal,
            done,
            l,
            r,
            &transport,
            ctx.recording,
        )
    };
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
                    tap.track(slot, s.strip.is_none(), done, l, r, gl, gr);
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
}

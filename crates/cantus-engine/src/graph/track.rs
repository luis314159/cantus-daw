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
//! A child of `processor` (it owns the slot's state through the processor today).

use super::*;
use crate::profile::Node;

/// A track slot's output for the current pass: the leveled (and, with compensation,
/// delayed) signal its strip or the master adds as it is. `active` is false when the track
/// was idle the whole pass — nothing to add.
pub(super) struct TrackOut {
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

impl Processor {
    /// Run snapshot track `ti` over the pass `[p0, p0 + n)` of the block: its events fire
    /// at their samples, and the instrument renders between them.
    pub(super) fn run_track(&mut self, snap: &Snapshot, ti: usize, p0: usize, n: usize) {
        let slot = self.track_to_slot[ti];
        self.prof.enter(Node::Track(slot));
        self.track_out[slot].active = false;
        let playing = self.transport.is_playing();
        let track = &snap.tracks[ti];
        let mut sub = 0usize;
        while sub < n {
            let mut end = n;
            if playing {
                loop {
                    let s = &mut self.slots[slot];
                    let Some(ev) = track.events.get(s.next_event) else {
                        break;
                    };
                    // Counted from the transport's integer sample, so the same at any split.
                    let off = (self.transport.samples_until(ev.tick.0 as f64) as usize)
                        .saturating_sub(sub);
                    if off == 0 {
                        fire(s, ev);
                        s.next_event += 1;
                        if self.listened[slot] {
                            let at = self.clock + (p0 + sub) as u64;
                            if !s.outbox.push(at, fx_note(ev)) {
                                self.stats.dropped_events += 1;
                            }
                        }
                    } else {
                        end = end.min(sub + off);
                        break;
                    }
                }
            }
            self.track_segment(track, slot, p0, sub, end);
            sub = end;
        }
    }

    /// The track's own segment `[sub, end)` of the pass starting `p0` frames into the
    /// block: instrument, clips and input, level and balance, line, output.
    fn track_segment(
        &mut self,
        track: &cantus_core::TrackSnapshot,
        slot: usize,
        p0: usize,
        sub: usize,
        end: usize,
    ) {
        let m = end - sub;
        let done = p0 + sub;
        // The transport at the segment's first sample. A pass never crosses a loop wrap, so
        // this is the pass's transport moved on, never wrapped.
        let mut transport = self.transport.clone();
        transport.advance(sub as u64);
        let s = &mut self.slots[slot];
        let mono = &mut self.mono[..m];
        let l = &mut self.buf_l[..m];
        let r = &mut self.buf_r[..m];
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
                match s.native.as_mut() {
                    Some(u) => {
                        // Where the song is at this segment's first sample, for the units
                        // that follow it (a hosted synth's arpeggiator).
                        let position =
                            song_position_of(&transport, &self.snapshot, self.sample_rate);
                        u.unit.set_position(&position);
                        let sig = u.unit.process(l, r);
                        if self.probed == s.id {
                            self.meters.inst = u.unit.telemetry();
                        }
                        // A hosted instrument's own window: its gestures go to the host
                        // (ADR 0028).
                        if let Some(track) = s.id {
                            for event in u.unit.param_events() {
                                let event = *event;
                                if self
                                    .tx
                                    .push(EngineEvent::InstParam { track, event })
                                    .is_err()
                                {
                                    self.stats.dropped_events += 1;
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
        let has_signal = self.live.add_track_audio(
            Some(track),
            slot,
            has_signal,
            done,
            l,
            r,
            &transport,
            self.recording,
        );
        let line = self.track_line[slot];
        let line_ringing = line.is_some_and(|i| self.pdc.lines[i].ringing());
        let out = &mut self.track_out[slot];
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
        if let Some(k0) = self.auto.k0(transport.samples()) {
            if auto::ramp_track(&self.auto, slot, s, k0, l, r) {
                (gl, gr) = (1.0, 1.0);
            }
        }
        if let Some(tap) = self.stem_tap.as_mut() {
            tap.track(slot, s.strip.is_none(), done, l, r, gl, gr);
        }
        // A delayed edge: the level and balance first, then the line; what comes out is
        // added at unity (a multiplication by 1.0 changes nothing).
        if let Some(li) = line {
            for i in 0..m {
                l[i] *= gl;
                r[i] *= gr;
            }
            self.pdc.lines[li].run(l, r);
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
}

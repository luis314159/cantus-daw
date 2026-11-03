//! An external instrument's notes out (ADR 0094 §3).
//!
//! A track whose unit sends MIDI is **listened to** like a track an effect hears (ADR 0022):
//! the notes it plays — a clip's, a live key's, an all-notes-off — already land in its
//! outbox with the absolute engine sample they were played at. At the end of each block this
//! drains that outbox into the MIDI-out thread's ring (`sync_out`), each note stamped with
//! its sample and the unit's route (its id); the thread turns the sample into a host time
//! (the audio thread's own line plus the device's output latency) and writes it to the port,
//! so the note leaves when its sample would be heard — by the same clock the MIDI clock out
//! uses. No allocation, no lock, no I/O here: a stamp is copied into a ring.
//!
//! **No stuck notes.** Each track keeps the keys it holds on its synth. When a unit goes (off,
//! removed, another port or channel), the keys held get their note-offs and an all-notes-off
//! on the route and channel they were played on, before the new routing plays anything. A
//! note-on is only sent while the ring keeps [`OFF_RESERVE`] places free, so a note-off
//! always has room; one that still finds the ring full waits in a queue of its own and goes
//! first in the next block.
//!
//! The track's own instrument is not played (the synth is outboard): the track plays its
//! notes into the outbox only, and the return comes back through the track's input.

// Slots are indices into fixed arrays, as everywhere in the processor.
#![allow(clippy::needless_range_loop)]

use super::*;
use crate::external_ping::{PingError, PingJob, PingKind};
use crate::fx::FxNote;
use crate::transport::sync::Stamped;

/// Places of the MIDI-out ring a note-on leaves free, for note-offs.
pub(crate) const OFF_RESERVE: usize = 256;
/// Note-offs (and all-notes-offs) waiting for room, at most.
const OFF_QUEUE: usize = 512;

/// What the processor keeps of the external instruments.
#[derive(Debug)]
pub(crate) struct ExtMidi {
    /// Per track slot: the unit's route (its id; 0: none) and MIDI channel.
    route: [u64; MAX_TRACKS],
    channel: [u8; MAX_TRACKS],
    /// The keys each slot holds on its synth (128 bits).
    held: [[u64; 2]; MAX_TRACKS],
    /// The next outbox note each track has not sent.
    cursor: [u64; MAX_TRACKS],
    any: bool,
    /// Note-offs that found the ring full: (route, bytes), oldest first.
    offs: Box<[(u64, [u8; 3])]>,
    offs_n: usize,
    /// Some track has an external effect's send.
    any_send: bool,
    /// Note-ons the ring had no room for (counted into the stats' dropped events).
    pub dropped: u64,
    /// The ping running (ADR 0094 §4), and the block's scratch: the return pair's
    /// interleaved frames and the mono signal for the send pair.
    ping: Option<Box<PingJob>>,
    scratch_in: Box<[f32]>,
    scratch_out: Box<[f32]>,
    /// `EngineStats::input_repeated` when the ping started: more means an underrun.
    repeated_at: u64,
}

impl Default for ExtMidi {
    fn default() -> Self {
        ExtMidi {
            route: [0; MAX_TRACKS],
            channel: [0; MAX_TRACKS],
            held: [[0; 2]; MAX_TRACKS],
            cursor: [0; MAX_TRACKS],
            any: false,
            offs: vec![(0, [0; 3]); OFF_QUEUE].into_boxed_slice(),
            offs_n: 0,
            any_send: false,
            dropped: 0,
            ping: None,
            scratch_in: vec![0.0; MAX_BLOCK_FRAMES * 2].into_boxed_slice(),
            scratch_out: vec![0.0; MAX_BLOCK_FRAMES].into_boxed_slice(),
            repeated_at: 0,
        }
    }
}

impl ExtMidi {
    /// Queue a note-off (or all-notes-off) for the next block. Allocation-free; a full queue
    /// keeps the newest all-notes-off at its end (it stops everything the lost ones would).
    fn queue_off(&mut self, route: u64, bytes: [u8; 3]) {
        if self.offs_n < self.offs.len() {
            self.offs[self.offs_n] = (route, bytes);
            self.offs_n += 1;
        } else if bytes[0] & 0xF0 == 0xB0 {
            self.offs[self.offs.len() - 1] = (route, bytes);
        }
    }

    /// Every key `slot` holds gets its note-off, and the channel an all-notes-off, on the
    /// route and channel they were played on.
    fn release_slot(&mut self, slot: usize) {
        let (route, c) = (self.route[slot], self.channel[slot] & 0x0F);
        if route == 0 {
            return;
        }
        for key in 0..128u8 {
            let (w, b) = (usize::from(key >> 6), key & 63);
            if self.held[slot][w] & (1 << b) != 0 {
                self.queue_off(route, [0x80 | c, key, 0]);
            }
        }
        self.queue_off(route, [0xB0 | c, 123, 0]);
        self.held[slot] = [0; 2];
    }
}

/// The bytes of a note for `channel`: on, off, or all-notes-off (controller 123).
pub(crate) fn bytes_of(note: FxNote, channel: u8) -> ([u8; 3], usize) {
    let c = channel & 0x0F;
    match note {
        FxNote::On { pitch, velocity } => ([0x90 | c, pitch & 0x7F, velocity.clamp(1, 127)], 3),
        FxNote::Off { pitch } => ([0x80 | c, pitch & 0x7F, 0], 3),
        FxNote::AllOff => ([0xB0 | c, 123, 0], 3),
    }
}

impl Processor {
    /// With a new snapshot: each track's route and channel. A slot whose route or channel
    /// changes (or goes) releases what it holds on the old ones first; a route that is new
    /// starts from the notes written now, so none from before it is replayed.
    /// Allocation-free.
    pub(super) fn ext_plan(&mut self, snapshot: &Snapshot) {
        let nodes = self.nodes.clone();
        // **The choke point of every offline render** (export, stems, freeze, bounce, video,
        // listening renders, bakes — all run a processor with `set_offline(true)`): it plays
        // no outboard gear. No note goes out, no send is made, the track's own source plays,
        // and no latency of the gear is reported (`processor_snap`).
        let offline = self.offline;
        let mut route = [0u64; MAX_TRACKS];
        let mut channel = [0u8; MAX_TRACKS];
        let mut send = [None; MAX_TRACKS];
        for t in snapshot.tracks.iter().take(MAX_TRACKS) {
            let Some(slot) = self.slot_for(t.id) else {
                continue;
            };
            if offline {
                continue;
            }
            let link = t.external.iter().find(|l| {
                l.kind == cantus_core::external_unit::UnitKind::Instrument && l.route != 0
            });
            // An external effect's send: out of the interface's outputs past the master pair
            // (a send on 1-2 would play on the master).
            send[slot] = t
                .external
                .iter()
                .find(|l| l.kind == cantus_core::external_unit::UnitKind::Effect)
                .and_then(|l| l.send)
                .filter(|p| p.first >= 2)
                .map(|p| (usize::from(p.first), p.stereo));
            if let Some(l) = link {
                route[slot] = l.route;
                channel[slot] = l.midi.as_ref().map_or(0, |m| m.channel);
            }
        }
        for slot in 0..MAX_TRACKS {
            let changed =
                route[slot] != self.ext.route[slot] || channel[slot] != self.ext.channel[slot];
            if changed {
                self.ext.release_slot(slot);
            }
            if route[slot] != 0 && changed {
                self.ext.cursor[slot] = nodes
                    .read(&nodes.tracks[slot].outbox)
                    .map_or(0, |o| o.written());
            }
        }
        for slot in 0..MAX_TRACKS {
            if self.track_ids[slot].is_some() {
                if let Some(mut s) = nodes.track(slot) {
                    s.outboard = route[slot] != 0;
                    s.send = send[slot];
                }
            }
        }
        self.ext.route = route;
        self.ext.channel = channel;
        self.ext.any = route.iter().any(|r| *r != 0) || self.ext.offs_n > 0;
        self.ext.any_send = send.iter().any(|s| s.is_some());
    }

    /// After a pass: each external effect's send, mixed into the device's output at its pair,
    /// and cleared. `output` is the pass's slice (`n` frames, `channels` interleaved). A pair
    /// past the device's channels (an offline render runs two) is not written: nothing of a
    /// send ever lands on another channel.
    pub(super) fn ext_send_mix(&mut self, output: &mut [f32], channels: usize, n: usize) {
        if !self.ext.any_send {
            return;
        }
        let nodes = self.nodes.clone();
        for slot in 0..MAX_TRACKS {
            let Some(mut s) = nodes.track(slot) else {
                continue;
            };
            let Some((first, stereo)) = s.send else {
                continue;
            };
            if !s.send_dirty {
                continue;
            }
            let width = if stereo { 2 } else { 1 };
            if first + width <= channels {
                for i in 0..n.min(output.len() / channels.max(1)) {
                    output[i * channels + first] += s.send_l[i];
                    if stereo {
                        output[i * channels + first + 1] += s.send_r[i];
                    }
                }
            }
            s.send_l[..n].iter_mut().for_each(|x| *x = 0.0);
            s.send_r[..n].iter_mut().for_each(|x| *x = 0.0);
            s.send_dirty = false;
        }
    }

    /// The processor went offline or back: the gear's routes, sends and latencies follow.
    pub(super) fn ext_offline_changed(&mut self) {
        let Some(snap) = self.snapshot.clone() else {
            return;
        };
        self.ext_plan(&snap);
        let nodes = self.nodes.clone();
        for t in snap.tracks.iter() {
            if let Some(slot) = self.slot_for(t.id) {
                if let Some(mut s) = nodes.track(slot) {
                    s.external = if self.offline {
                        0
                    } else {
                        t.external
                            .iter()
                            .map(|l| l.latency_at(self.sample_rate))
                            .sum::<usize>()
                            .min(crate::pdc::MAX_DELAY)
                    };
                }
            }
        }
        self.compensate();
    }

    /// While the listeners are planned: the tracks whose units send MIDI are listened to, so
    /// their notes go to the outbox.
    pub(super) fn ext_listen(&mut self) {
        for slot in 0..MAX_TRACKS {
            if self.ext.route[slot] != 0 {
                self.listened[slot] = true;
            }
        }
    }

    /// The end of a block (before the outboxes are pruned): the note-offs that waited, then
    /// every note an external instrument's track played, to the MIDI-out ring, stamped with
    /// its sample.
    pub(super) fn ext_drain(&mut self) {
        if !self.ext.any && self.ext.offs_n == 0 {
            return;
        }
        let nodes = self.nodes.clone();
        let Some(mut run) = self.sync.take() else {
            return;
        };
        // What waited goes first, at this block's start.
        let mut sent = 0;
        while sent < self.ext.offs_n {
            let (route, bytes) = self.ext.offs[sent];
            if !run.push_ext(Stamped::note(self.clock, route, &bytes)) {
                break;
            }
            sent += 1;
        }
        self.ext.offs.copy_within(sent..self.ext.offs_n, 0);
        self.ext.offs_n -= sent;
        for slot in 0..MAX_TRACKS {
            let route = self.ext.route[slot];
            if route == 0 {
                continue;
            }
            let channel = self.ext.channel[slot];
            let Some(ob) = nodes.read(&nodes.tracks[slot].outbox) else {
                nodes.conflict();
                continue;
            };
            let mut dropped = 0;
            let ext = &mut self.ext;
            let mut cursor = ext.cursor[slot];
            ob.drain_from(&mut cursor, |at, note| {
                let (bytes, n) = bytes_of(note, channel);
                match note {
                    FxNote::On { pitch, .. } => {
                        // A note-on only while the reserve for note-offs is free.
                        if run.out_free() > OFF_RESERVE
                            && run.push_ext(Stamped::note(at, route, &bytes[..n]))
                        {
                            let (w, b) = (usize::from(pitch >> 6 & 1), pitch & 63);
                            ext.held[slot][w] |= 1 << b;
                        } else {
                            dropped += 1;
                        }
                    }
                    FxNote::Off { pitch } => {
                        let (w, b) = (usize::from(pitch >> 6 & 1), pitch & 63);
                        if ext.held[slot][w] & (1 << b) != 0 {
                            ext.held[slot][w] &= !(1 << b);
                            if !run.push_ext(Stamped::note(at, route, &bytes[..n])) {
                                ext.queue_off(route, bytes);
                            }
                        }
                    }
                    FxNote::AllOff => {
                        ext.held[slot] = [0; 2];
                        if !run.push_ext(Stamped::note(at, route, &bytes[..n])) {
                            ext.queue_off(route, bytes);
                        }
                    }
                }
            });
            self.ext.cursor[slot] = cursor;
            self.ext.dropped += dropped;
            self.stats.dropped_events += dropped;
        }
        self.sync = Some(run);
    }

    /// The keys a slot holds on its synth (tests, the stats).
    #[allow(dead_code)]
    pub(crate) fn ext_held(&self, slot: usize) -> [u64; 2] {
        self.ext.held[slot]
    }
}

// ---------------------------------------------------------------------------------------
// The latency ping (ADR 0094 §3): the audio thread runs it block by block

impl Processor {
    /// A job arrives: refused (and sent straight back) while another runs or the transport
    /// plays; else it starts with this block.
    pub(super) fn ping_start(&mut self, mut job: Box<PingJob>) {
        if self.ext.ping.is_some() || self.transport.is_playing() {
            job.refused = Some(PingError::Busy);
            job.finished = true;
            self.ping_return(job);
            return;
        }
        self.ext.repeated_at = self.stats.input_repeated;
        self.ext.ping = Some(job);
    }

    fn ping_return(&mut self, job: Box<PingJob>) {
        // A full event queue keeps the job here until it has room (it is checked each block).
        if let Err(rtrb::PushError::Full(EngineEvent::PingDone(job))) =
            self.tx.push(EngineEvent::PingDone(job))
        {
            self.ext.ping = Some(job);
        }
    }

    /// After the input is read, before the block renders: what the return pair delivered goes
    /// to the ping, the signal for the send pair is made, a note is stamped.
    pub(super) fn ping_input(&mut self, frames: usize) {
        let Some(mut job) = self.ext.ping.take() else {
            return;
        };
        if job.finished {
            self.ping_return(job);
            return;
        }
        // The transport started under it: the ping stops (a note it may hold is released)
        // and comes back aborted, never measuring the song.
        if self.transport.is_playing() {
            if let PingKind::Onset {
                route,
                channel,
                key,
                ..
            } = &job.kind
            {
                let c = *channel & 0x0F;
                self.ext.queue_off(*route, [0x80 | c, *key, 0]);
                self.ext.any = true;
            }
            job.refused = Some(PingError::Aborted);
            job.finished = true;
            self.ping_return(job);
            return;
        }
        let nodes = self.nodes.clone();
        let ret = match &job.kind {
            PingKind::Audio { ret, .. } | PingKind::Onset { ret, .. } => *ret,
        };
        let width = usize::from(ret.width());
        // The return pair's frames, interleaved, or none when the input delivered nothing.
        let mut have = false;
        if let Some(block) = nodes.read(&nodes.input) {
            let first = usize::from(ret.first);
            {
                for c in 0..width {
                    match block.channel(first + c, 0, frames) {
                        Some(ch) => {
                            have = true;
                            for (i, x) in ch.iter().enumerate() {
                                self.ext.scratch_in[i * width + c] = *x;
                            }
                        }
                        None => have = false,
                    }
                }
            }
        }
        let input = have.then(|| &self.ext.scratch_in[..frames * width]);
        let done = match &mut job.kind {
            PingKind::Audio { ping, .. } => ping.block(input, &mut self.ext.scratch_out[..frames]),
            PingKind::Onset {
                ping,
                route,
                channel,
                key,
                ..
            } => {
                // The return as one channel: the pair's first (an instrument is read mono).
                let mono: Option<&[f32]> = if have {
                    for i in 0..frames {
                        let mut m = 0.0f32;
                        for c in 0..width {
                            m = m.max(self.ext.scratch_in[i * width + c].abs());
                        }
                        self.ext.scratch_out[i] = m;
                    }
                    Some(&self.ext.scratch_out[..frames])
                } else {
                    None
                };
                let step = ping.block(mono, frames);
                if let Some(mut run) = self.sync.take() {
                    let at = self.clock;
                    let c = *channel & 0x0F;
                    if let Some(o) = step.on {
                        run.push_ext(Stamped::note(
                            at + u64::from(o),
                            *route,
                            &[0x90 | c, *key, 100],
                        ));
                    }
                    if let Some(o) = step.off {
                        let off = [0x80 | c, *key, 0];
                        if !run.push_ext(Stamped::note(at + u64::from(o), *route, &off)) {
                            self.ext.queue_off(*route, off);
                            self.ext.any = true;
                        }
                    }
                    self.sync = Some(run);
                } else if step.on.is_some() {
                    job.refused = Some(PingError::NoNote);
                }
                step.done || job.refused.is_some()
            }
        };
        if self.stats.input_repeated > self.ext.repeated_at {
            job.underrun = true;
        }
        if done {
            job.finished = true;
            self.ping_return(job);
        } else {
            self.ext.ping = Some(job);
        }
    }

    /// After the block rendered: while an audio ping runs, the output is the ping's alone.
    pub(super) fn ping_output(&mut self, output: &mut [f32], channels: usize, frames: usize) {
        let Some(job) = self.ext.ping.as_ref() else {
            return;
        };
        if job.finished {
            return;
        }
        output.iter_mut().for_each(|s| *s = 0.0);
        if let PingKind::Audio { send, .. } = &job.kind {
            let (first, width) = (usize::from(send.first), usize::from(send.width()));
            // A pair past the device's channels is not written (it would land on others).
            if first + width <= channels {
                for i in 0..frames.min(output.len() / channels.max(1)) {
                    let v = self.ext.scratch_out[i];
                    for c in 0..width {
                        output[i * channels + first + c] = v;
                    }
                }
            }
        }
    }
}

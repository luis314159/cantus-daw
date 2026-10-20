//! An isolated plug-in's audio half, in Cantus (ADR 0063): the engine calls it exactly as
//! it calls an in-process plug-in's, and it hands each block to the helper in lock-step.
//!
//! Per block: the input, the sidechain, the parameter changes, the notes and the transport
//! are copied into the instance's shared slot; the job is published; the thread spins for a
//! bounded time ([`SPIN`]), then sleeps on its bell until a **deadline** ([`deadline`]); the
//! helper's output and its parameter events are copied back. Nothing allocates.
//!
//! When the deadline passes, the block goes on **without** the plug-in — the effect's input
//! passes through, delayed by the latency the plug-in reported, as a bypassed slot does (so
//! nothing that runs in parallel moves against it), and an instrument stays silent (the
//! caller zeroed its buffers) — and the late job is never waited for again: the next blocks
//! pass dry until it has finished. Only the job that missed its deadline counts in
//! [`Status::late`]. What those dry blocks carried is **not lost**: the last value of each
//! parameter, the note-offs, chokes, expressions and MIDI controllers, and a reset, are kept
//! (in buffers made here, never grown) and go to the plug-in at the start of its next job —
//! so no note is left ringing and no value out of step with the project (review #3, #10).
//! When the helper is gone, every block passes dry from then on.
//!
//! An **offline** instance (a render, an export) waits for every block as long as it takes
//! while its helper lives, so the render equals the in-process one to the bit — in slices,
//! so a render that is cancelled stops, and a job unfinished past the offline hang limit
//! ends its helper and the render goes on dry (review #1, #11, #39, #46).
//!
//! This amends ADR 0005 rule 3 for this one case (ADR 0063): the wait is the plug-in's own
//! processing time, moved to another process; it is bounded by the deadline, and the ring
//! that wakes a parked helper is a non-blocking system call.

use super::sandbox::Sandbox;
use super::slot::{op, status, Slot, MAX_NOTES, MAX_PARAMS};
use crate::backend::{Audio, AudioThread, NoteEvent, NoteKind, ParamChange, ParamOut};
use cantus_rt::ipc::{Caller, Exchange, Outcome};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// How long the audio thread spins for the helper's answer before it sleeps on its bell:
/// enough for a light plug-in's whole job, so most blocks make no system call to wait.
pub const SPIN: Duration = Duration::from_micros(200);

/// The shortest deadline (`Sandboxes::set_deadline_floor` changes it, for tests on a busy
/// machine that compare bits in real time).
pub const DEADLINE_FLOOR: Duration = Duration::from_millis(5);

/// How often an offline wait looks whether its render was cancelled or its job hung.
const OFFLINE_SLICE: Duration = Duration::from_millis(100);

/// The deadline of a block of `frames` at `sample_rate`: twice the block's own duration,
/// at least `floor` ([`DEADLINE_FLOOR`]). A plug-in that takes longer was going to make the
/// device drop out in process too; out of process, the block is cut there instead of
/// waiting on.
pub fn deadline(frames: usize, sample_rate: f64, floor: Duration) -> Duration {
    let block = Duration::from_secs_f64(frames as f64 / sample_rate.max(1.0));
    (block * 2).max(floor)
}

/// What Cantus knows of one isolated instance across its halves — a restarted audio half
/// shares it, so it never hands the slot a job while the old half's late one still runs.
#[derive(Debug)]
pub struct Status {
    dead: AtomicBool,
    /// Jobs that missed their deadline (one per late job, not one per block it kept dry).
    pub late: AtomicU64,
    /// Milliseconds since `epoch` when the unfinished job was published; 0 for none.
    since_ms: AtomicU64,
    /// The unfinished job's number (valid while `since_ms` is not 0).
    job: AtomicU32,
    epoch: Instant,
}

impl Default for Status {
    fn default() -> Self {
        Status {
            dead: AtomicBool::new(false),
            late: AtomicU64::new(0),
            since_ms: AtomicU64::new(0),
            job: AtomicU32::new(0),
            epoch: Instant::now(),
        }
    }
}

impl Status {
    pub fn is_dead(&self) -> bool {
        self.dead.load(Ordering::Acquire)
    }

    pub fn mark_dead(&self) {
        self.dead.store(true, Ordering::Release);
    }

    fn now_ms(&self) -> u64 {
        (self.epoch.elapsed().as_millis() as u64).max(1)
    }

    /// Job `n` missed its deadline and runs on.
    fn set_outstanding(&self, n: u32) {
        self.job.store(n, Ordering::Release);
        self.since_ms.store(self.now_ms(), Ordering::Release);
    }

    /// The job that missed its deadline and has not been seen finished, if any.
    fn outstanding(&self) -> Option<u32> {
        (self.since_ms.load(Ordering::Acquire) != 0).then(|| self.job.load(Ordering::Acquire))
    }

    fn clear(&self) {
        self.since_ms.store(0, Ordering::Release);
    }

    /// How long the late job has been out — judged by the exchange, not by whether an
    /// audio half calls again: a job that has finished is cleared here, so a slot bypassed
    /// or restarted after one late block is never taken for hung (review #2, #38).
    pub fn unfinished_for(&self, x: &Exchange) -> Option<Duration> {
        let since = self.since_ms.load(Ordering::Acquire);
        if since == 0 {
            return None;
        }
        let n = self.job.load(Ordering::Acquire);
        if x.numbers().1 == n {
            let _ = self
                .since_ms
                .compare_exchange(since, 0, Ordering::AcqRel, Ordering::Relaxed);
            return None;
        }
        let now = self.epoch.elapsed().as_millis() as u64;
        Some(Duration::from_millis(now.saturating_sub(since)))
    }
}

/// What an offline half needs to end a hung render, or a cancelled one.
pub struct Offline {
    pub sandbox: Arc<Sandbox>,
    pub instance: u32,
    pub name: String,
    /// Set when the render is cancelled.
    pub cancel: Arc<AtomicBool>,
}

/// What blocks that passed without the plug-in carried, for its next job.
struct Carry {
    /// The last value of each parameter changed meanwhile, at offset 0.
    params: Vec<ParamChange>,
    /// Note-offs and chokes, and the last value of each controller and expression, at
    /// offset 0 (note-ons are dropped: a note late by a block or more is not played).
    notes: Vec<NoteEvent>,
    /// A reset asked for meanwhile (or forced, when the carry overflowed).
    reset: bool,
    /// Changes that found no room (more distinct parameters than the slot carries).
    lost: u64,
    /// This block's own changes and notes after the carried ones (made once).
    merged_params: Vec<ParamChange>,
    merged_notes: Vec<NoteEvent>,
}

/// What a carried event stands for: a controller or an expression is replaced by a later
/// one of the same kind, never piled up (review #9, #16).
fn same_kind(a: &NoteEvent, b: &NoteEvent) -> bool {
    match (a.kind, b.kind) {
        (
            NoteKind::Midi {
                status: sa,
                data1: da,
                ..
            },
            NoteKind::Midi {
                status: sb,
                data1: db,
                ..
            },
        ) => {
            a.channel == b.channel
                && sa & 0xF0 == sb & 0xF0
                // A controller is one per number; pitch bend and pressure one per channel.
                && (sa & 0xF0 != 0xB0 || da == db)
        }
        (
            NoteKind::Expression { expression: ea, .. },
            NoteKind::Expression { expression: eb, .. },
        ) => ea == eb && a.note_id == b.note_id && a.key == b.key && a.channel == b.channel,
        _ => false,
    }
}

/// A note-off or a choke: what must never be lost (a voice would ring on).
fn ends_a_note(e: &NoteEvent) -> bool {
    matches!(e.kind, NoteKind::Off { .. } | NoteKind::Choke)
}

impl Carry {
    fn new() -> Carry {
        Carry {
            params: Vec::with_capacity(MAX_PARAMS),
            notes: Vec::with_capacity(MAX_NOTES),
            reset: false,
            lost: 0,
            merged_params: Vec::with_capacity(MAX_PARAMS),
            merged_notes: Vec::with_capacity(MAX_NOTES),
        }
    }

    fn is_empty(&self) -> bool {
        self.params.is_empty() && self.notes.is_empty() && self.lost == 0
    }

    /// Keeps a dry block's changes and notes. Never grows a buffer: when the notes do not
    /// fit, the plug-in is reset instead (no voice can stay stuck) and the carried notes,
    /// which a reset supersedes, go.
    fn keep(&mut self, params: &[ParamChange], notes: &[NoteEvent]) {
        for p in params {
            if let Some(k) = self.params.iter_mut().find(|k| k.id == p.id) {
                k.value = p.value;
            } else if self.params.len() < self.params.capacity() {
                self.params.push(ParamChange {
                    offset: 0,
                    id: p.id,
                    value: p.value,
                });
            } else {
                self.lost += 1;
            }
        }
        for n in notes {
            if matches!(n.kind, NoteKind::On { .. }) || self.reset {
                continue;
            }
            let e = NoteEvent { offset: 0, ..*n };
            if let Some(k) = self.notes.iter_mut().find(|k| same_kind(k, &e)) {
                *k = e;
            } else if self.notes.len() < self.notes.capacity() {
                self.notes.push(e);
            } else {
                self.reset = true;
                self.notes.clear();
            }
        }
    }

    /// The carried changes and notes, then this block's: what the next job takes. This
    /// block's own events come first in priority — the carried note-offs and chokes next,
    /// the carried controllers and expressions in what room is left (review #8, #9, #23,
    /// #41). Returns what found no room (counted as lost), and whether a carried note-off
    /// had none (the plug-in is then reset first).
    fn merge(&mut self, params: &[ParamChange], notes: &[NoteEvent]) -> (u64, bool) {
        self.merged_params.clear();
        self.merged_notes.clear();
        let mut lost = std::mem::take(&mut self.lost);
        for p in self.params.iter().chain(params) {
            if self.merged_params.len() < self.merged_params.capacity() {
                self.merged_params.push(*p);
            } else {
                lost += 1;
            }
        }
        let cap = self.merged_notes.capacity();
        let own = notes.len().min(cap);
        lost += (notes.len() - own) as u64;
        let mut room = cap - own;
        let mut reset = false;
        // Carried events first in time (offset 0): the note-offs, then the rest.
        for pass in [true, false] {
            for n in self.notes.iter().filter(|n| ends_a_note(n) == pass) {
                if room > 0 {
                    self.merged_notes.push(*n);
                    room -= 1;
                } else if pass {
                    reset = true;
                }
            }
        }
        self.merged_notes.extend_from_slice(&notes[..own]);
        self.params.clear();
        self.notes.clear();
        (lost, reset)
    }
}

/// The audio half.
pub struct BridgeAudio {
    slot: Arc<Slot>,
    /// Keeps the helper's lobby mapped while the caller points into it.
    _lobby: Arc<super::sandbox::LobbyMem>,
    caller: Caller,
    status: Arc<Status>,
    /// `Some` for an offline instance (a render): no deadline.
    offline: Option<Offline>,
    sample_rate: f64,
    floor: Duration,
    /// The input of the last `latency` frames, per channel: what a block that passes
    /// without the plug-in plays, so it stays where the plug-in's output would have been.
    delay: [Box<[f32]>; 2],
    at: usize,
    /// This block's delayed input, per channel (the largest block's length).
    scratch: [Box<[f32]>; 2],
    carry: Carry,
    /// What a late job reported (its gestures and values), once it finished: handed to the
    /// next block's caller (made once; review #12, #42, #46).
    late_out: Box<ParamOut>,
    /// Told when this half is dropped (on the main thread), so the helper drops its own.
    on_drop: Option<Box<dyn FnOnce() + Send>>,
}

impl std::fmt::Debug for BridgeAudio {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BridgeAudio")
            .field("slot", &self.slot.name())
            .field("offline", &self.offline.is_some())
            .finish()
    }
}

impl BridgeAudio {
    pub(crate) fn new(
        (slot, lobby): (Arc<Slot>, Arc<super::sandbox::LobbyMem>),
        caller: Caller,
        status: Arc<Status>,
        (offline, sample_rate, floor, latency): (Option<Offline>, f64, Duration, usize),
        on_drop: Box<dyn FnOnce() + Send>,
    ) -> BridgeAudio {
        let longest = if latency > 0 { slot.max_block() } else { 0 };
        BridgeAudio {
            slot,
            _lobby: lobby,
            caller,
            status,
            offline,
            sample_rate,
            floor,
            delay: [
                vec![0.0; latency].into_boxed_slice(),
                vec![0.0; latency].into_boxed_slice(),
            ],
            at: 0,
            scratch: [
                vec![0.0; longest].into_boxed_slice(),
                vec![0.0; longest].into_boxed_slice(),
            ],
            carry: Carry::new(),
            late_out: Box::new(ParamOut::new()),
            on_drop: Some(on_drop),
        }
    }

    /// Runs the block's input through the latency line: the line takes it, and the input
    /// of `latency` frames ago goes into `scratch` — what the block plays if it passes
    /// without the plug-in. Nothing when the plug-in reports no latency.
    fn advance(&mut self, left: &[f32], right: &[f32]) {
        let n = self.delay[0].len();
        if n == 0 {
            return;
        }
        let start = self.at;
        let mut at = start;
        for c in 0..2 {
            at = start;
            let input = if c == 0 { left } else { right };
            let (line, scratch) = (&mut self.delay[c], &mut self.scratch[c]);
            for (i, x) in input.iter().enumerate() {
                scratch[i] = line[at];
                line[at] = *x;
                at += 1;
                if at == n {
                    at = 0;
                }
            }
        }
        self.at = at;
    }

    /// The block passes without the plug-in: the delayed input `advance` kept (or, with no
    /// latency, the input as it is).
    fn play_dry(&self, left: &mut [f32], right: &mut [f32]) {
        if self.delay[0].is_empty() {
            return;
        }
        let n = left.len();
        left.copy_from_slice(&self.scratch[0][..n]);
        right.copy_from_slice(&self.scratch[1][..n]);
    }

    /// Whether the plug-in may be given a job now: alive, no late job unfinished (this
    /// half's or a previous half's of the same instance), and a reset carried over from
    /// dry blocks delivered.
    fn ready(&mut self) -> bool {
        if self.status.is_dead() {
            return false;
        }
        if let Some(n) = self.status.outstanding() {
            if !self.caller.finished(n) {
                return false;
            }
            // Its audio is too late to play; what the plug-in reported in it is not.
            // SAFETY: the late job finished and no other is in flight: the area is ours.
            unsafe { self.slot.take_events(&mut self.late_out) };
            self.status.clear();
        }
        if self.carry.reset {
            self.carry.reset = false;
            // SAFETY: no job is in flight (just checked), so the slot's job area is ours.
            unsafe {
                self.slot
                    .put_job(op::RESET, &[], &[], None, None, &[], &[], false);
            }
            if !self.run(0) {
                return false;
            }
        }
        true
    }

    /// Publishes the job written in the slot and waits for it. `true` when it finished.
    fn run(&mut self, frames: usize) -> bool {
        let n = self.caller.submit();
        if self.offline.is_some() {
            return self.run_offline(n);
        }
        let deadline = Instant::now() + deadline(frames, self.sample_rate, self.floor);
        match self.caller.wait(n, SPIN, Some(deadline)) {
            Outcome::Done => true,
            Outcome::Late => {
                self.status.set_outstanding(n);
                self.status.late.fetch_add(1, Ordering::Relaxed);
                false
            }
            Outcome::Closed => {
                self.status.mark_dead();
                false
            }
        }
    }

    /// An offline job: waited for while it runs, in slices — until the render is
    /// cancelled, or the job has run past the offline hang limit, which ends its helper.
    fn run_offline(&mut self, n: u32) -> bool {
        let started = Instant::now();
        loop {
            match self
                .caller
                .wait(n, SPIN, Some(Instant::now() + OFFLINE_SLICE))
            {
                Outcome::Done => return true,
                Outcome::Closed => {
                    self.status.mark_dead();
                    return false;
                }
                Outcome::Late => {
                    let Some(o) = &self.offline else { return false };
                    if o.cancel.load(Ordering::Acquire) {
                        // The render stops; its own helper with it.
                        o.sandbox
                            .kill_blaming("the render was stopped", Some(o.instance));
                        self.status.mark_dead();
                        return false;
                    }
                    if started.elapsed() > o.sandbox.limits().offline {
                        o.sandbox.kill_blaming(
                            &format!("{} stopped responding while it rendered", o.name),
                            Some(o.instance),
                        );
                        self.status.mark_dead();
                        return false;
                    }
                }
            }
        }
    }
}

impl AudioThread for BridgeAudio {
    fn process(
        &mut self,
        audio: Audio<'_>,
        params: &[ParamChange],
        notes: &[NoteEvent],
        out: &mut ParamOut,
    ) {
        let frames = audio
            .left
            .len()
            .min(audio.right.len())
            .min(self.slot.max_block());
        let left = &mut audio.left[..frames];
        let right = &mut audio.right[..frames];
        self.advance(left, right);
        if !self.ready() {
            // Dry, where the plug-in's output would be, for an effect; silent for an
            // instrument (its buffers came zeroed). What the block carried is kept.
            if !self.status.is_dead() {
                self.carry.keep(params, notes);
            }
            self.play_dry(left, right);
            return;
        }
        // A late job's own report first, in its order.
        for e in self.late_out.events() {
            out.push(*e);
        }
        out.dropped += std::mem::take(&mut self.late_out.dropped);
        out.lost_in += std::mem::take(&mut self.late_out.lost_in);
        self.late_out.clear();
        let flush = cantus_rt::denormal::flushing();
        let carried = !self.carry.is_empty();
        if carried {
            let (lost, reset_first) = self.carry.merge(params, notes);
            out.lost_in += lost;
            if reset_first {
                // A note-off the buffer had no room for: the plug-in is reset first, so no
                // voice rings on.
                // SAFETY: no job is in flight (`ready`): the slot's job area is ours.
                unsafe {
                    self.slot
                        .put_job(op::RESET, &[], &[], None, None, &[], &[], false);
                }
                if !self.run(0) {
                    self.play_dry(left, right);
                    return;
                }
            }
        }
        let (params, notes): (&[ParamChange], &[NoteEvent]) = if carried {
            (&self.carry.merged_params, &self.carry.merged_notes)
        } else {
            (params, notes)
        };
        // SAFETY: no job is in flight (`ready`): the slot's job area is ours; the lengths
        // are within the slot's largest block.
        let lost = unsafe {
            self.slot.put_job(
                op::PROCESS,
                left,
                right,
                audio.key,
                audio.transport,
                params,
                notes,
                flush,
            )
        };
        out.lost_in += lost;
        if self.run(frames) {
            // SAFETY: the job finished: the area is ours again.
            let st = unsafe { self.slot.take_result(left, right, out) };
            debug_assert!(st == status::OK || st == status::NO_AUDIO);
        } else {
            // Late or gone: the delayed input this block, as a bypassed slot plays.
            self.play_dry(left, right);
        }
    }

    fn reset(&mut self) {
        if !self.ready() {
            // Delivered before the next job (review #3).
            if !self.status.is_dead() {
                self.carry.reset = true;
            }
            return;
        }
        // SAFETY: no job is in flight (`ready`), so the slot's job area is ours.
        unsafe {
            self.slot
                .put_job(op::RESET, &[], &[], None, None, &[], &[], false);
        }
        self.run(0);
    }

    fn stop(&mut self) {
        // The half's last call: a late job's half is dropped by the helper anyway.
        if !self.ready() {
            return;
        }
        // SAFETY: as in `reset`.
        unsafe {
            self.slot
                .put_job(op::STOP, &[], &[], None, None, &[], &[], false);
        }
        self.run(0);
    }
}

impl Drop for BridgeAudio {
    fn drop(&mut self) {
        if let Some(f) = self.on_drop.take() {
            f();
        }
    }
}

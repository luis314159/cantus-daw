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
    /// audio half calls again: a job that has finished is not out, so a slot bypassed or
    /// restarted after one late block is never taken for hung (review #2, #38). Only
    /// reads: the audio half clears it once it has taken the job's events, so the
    /// interface seeing the job finish first loses none of them (review isolation-4 #0,
    /// #11).
    pub fn unfinished_for(&self, x: &Exchange) -> Option<Duration> {
        let since = self.since_ms.load(Ordering::Acquire);
        if since == 0 {
            return None;
        }
        let n = self.job.load(Ordering::Acquire);
        if x.numbers().1 == n {
            return None;
        }
        let now = self.epoch.elapsed().as_millis() as u64;
        Some(Duration::from_millis(now.saturating_sub(since)))
    }

    /// Cantus (or the machine) was paused: the late job's clock starts again from now, so
    /// the pause is not taken for the plug-in's hang (review isolation-4 #12, #22).
    pub fn rebase(&self) {
        let since = self.since_ms.load(Ordering::Acquire);
        if since != 0 {
            let _ = self.since_ms.compare_exchange(
                since,
                self.now_ms(),
                Ordering::AcqRel,
                Ordering::Relaxed,
            );
        }
    }
}

/// Whether one slice of an offline wait took far longer than asked — more than the
/// silence limit past it: Cantus (or the machine) was paused, a laptop asleep, and the
/// job's clock starts again instead of the plug-in being taken for hung (review
/// isolation-4 #12, #22, #36).
fn slice_paused(took: Duration, silence: Duration) -> bool {
    took > OFFLINE_SLICE + silence
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
    /// What the dry blocks carried, at offset 0, **in the order it came**: note-offs and
    /// chokes; MIDI controllers, pitch bend, pressure, program changes and expressions,
    /// each folded to its last value (the older one removed, the newer appended, so the
    /// order of last values stands); switches (sustain and the other pedals) keep their
    /// last lift and press; (N)RPN, data entry and channel-mode messages are never folded
    /// (review isolation-4 #14, #65). Note-ons are dropped: a note late by a block or more
    /// is not played.
    notes: Vec<NoteEvent>,
    /// A reset asked for meanwhile (or forced, when the carry overflowed): it ends every
    /// voice, so the note-offs, chokes and per-note expressions before it are gone and
    /// those after it are not kept — the controllers are (review isolation-4 #8).
    reset: bool,
    /// Changes that found no room (more distinct parameters than the slot carries, or
    /// controllers past the carry's room).
    lost: u64,
    /// This block's own changes and notes after the carried ones (made once).
    merged_params: Vec<ParamChange>,
    merged_notes: Vec<NoteEvent>,
}

/// How a carried event stands for later ones of its kind.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Fold {
    /// Replaced by a later one of the same kind (a value).
    Last,
    /// A switch (CC 64–69): an off and an on are both kept, so a lift between two presses
    /// still reaches the plug-in.
    Switch,
    /// Kept as it is, in order (note-offs, chokes, (N)RPN and data entry, channel mode).
    Keep,
}

fn fold_of(e: &NoteEvent) -> Fold {
    match e.kind {
        NoteKind::Midi { status, data1, .. } => match status & 0xF0 {
            0xB0 => match data1 {
                64..=69 => Fold::Switch,
                6 | 38 | 96..=101 | 120..=127 => Fold::Keep,
                _ => Fold::Last,
            },
            0xA0 | 0xC0 | 0xD0 | 0xE0 => Fold::Last,
            _ => Fold::Keep,
        },
        NoteKind::Expression { .. } => Fold::Last,
        _ => Fold::Keep,
    }
}

/// Whether two carried events are the same controller or expression (review #9, #16):
/// a controller and poly pressure are one per number (key); pitch bend, channel pressure
/// and the program one per channel; an expression one per note.
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
                && (!matches!(sa & 0xF0, 0xA0 | 0xB0) || da == db)
        }
        (
            NoteKind::Expression { expression: ea, .. },
            NoteKind::Expression { expression: eb, .. },
        ) => ea == eb && a.note_id == b.note_id && a.key == b.key && a.channel == b.channel,
        _ => false,
    }
}

/// A switch controller's position: pressed (64 and up) or lifted.
fn pressed(e: &NoteEvent) -> bool {
    matches!(e.kind, NoteKind::Midi { data2, .. } if data2 >= 64)
}

/// A note-off or a choke: what must never be lost (a voice would ring on).
fn ends_a_note(e: &NoteEvent) -> bool {
    matches!(e.kind, NoteKind::Off { .. } | NoteKind::Choke)
}

/// What a reset supersedes: whatever belongs to a voice (it ends them all).
fn of_a_voice(e: &NoteEvent) -> bool {
    matches!(
        e.kind,
        NoteKind::On { .. } | NoteKind::Off { .. } | NoteKind::Choke | NoteKind::Expression { .. }
    )
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

    /// A carry with no room, swapped in while the real one is in use (allocates nothing).
    fn empty() -> Carry {
        Carry {
            params: Vec::new(),
            notes: Vec::new(),
            reset: false,
            lost: 0,
            merged_params: Vec::new(),
            merged_notes: Vec::new(),
        }
    }

    fn is_empty(&self) -> bool {
        self.params.is_empty() && self.notes.is_empty() && self.lost == 0
    }

    /// A reset asked for while the plug-in is late: delivered before its next job; the
    /// voices' events carried so far are superseded by it.
    fn reset_now(&mut self) {
        self.reset = true;
        self.notes.retain(|k| !of_a_voice(k));
    }

    /// Keeps a dry block's changes and notes. Never grows a buffer: when the notes do not
    /// fit, the plug-in is reset instead (no voice can stay stuck) and the voices' events,
    /// which the reset supersedes, go; the controllers stay (review isolation-4 #14, #65).
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
            if matches!(n.kind, NoteKind::On { .. }) || (self.reset && of_a_voice(n)) {
                continue;
            }
            let e = NoteEvent { offset: 0, ..*n };
            match fold_of(&e) {
                Fold::Last => {
                    if let Some(i) = self.notes.iter().position(|k| same_kind(k, &e)) {
                        self.notes.remove(i);
                    }
                }
                Fold::Switch => {
                    let first = self.notes.iter().position(|k| same_kind(k, &e));
                    let last = self.notes.iter().rposition(|k| same_kind(k, &e));
                    if let (Some(f), Some(l)) = (first, last) {
                        if pressed(&self.notes[l]) == pressed(&e) {
                            // The same position again: the newer stands for it.
                            self.notes.remove(l);
                        } else if f != l {
                            // A lift and a press kept already: the oldest goes.
                            self.notes.remove(f);
                        }
                    }
                }
                Fold::Keep => {}
            }
            if self.notes.len() < self.notes.capacity() {
                self.notes.push(e);
            } else {
                self.reset_now();
                if of_a_voice(&e) {
                    continue;
                }
                if self.notes.len() < self.notes.capacity() {
                    self.notes.push(e);
                } else {
                    self.lost += 1;
                }
            }
        }
    }

    /// Whether the carried note-offs and chokes would find no room beside `own` events of
    /// the block: the plug-in is then reset before the block, so no voice rings on — and
    /// that is decided before anything is merged (review isolation-4 #9, #15).
    fn needs_reset_first(&self, own: usize) -> bool {
        let room = self.merged_notes.capacity().saturating_sub(own);
        self.notes.iter().filter(|n| ends_a_note(n)).count() > room
    }

    /// The reset asked for by [`Self::needs_reset_first`] was delivered: the voices'
    /// events it supersedes go.
    fn reset_delivered(&mut self) {
        self.notes.retain(|k| !of_a_voice(k));
    }

    /// The carried changes and notes, then this block's: what the next job takes. The
    /// carried ones come first in time, in the order they came; when room is short the
    /// block's own events come first in priority, the carried note-offs and chokes next,
    /// and the oldest carried controllers go first — every one that finds no room is
    /// counted as lost (review #8, #9, #23, #41; isolation-4 #14, #15, #65). Returns what
    /// was lost, and whether a carried note-off found no room (never, once
    /// [`Self::needs_reset_first`] was heeded).
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
        let room = cap - own;
        let ends = self.notes.iter().filter(|n| ends_a_note(n)).count();
        let others = self.notes.len() - ends;
        let mut ends_room = room.min(ends);
        let mut skip = others.saturating_sub(room - ends_room);
        let reset = ends > room;
        for n in &self.notes {
            if ends_a_note(n) {
                if ends_room > 0 {
                    self.merged_notes.push(*n);
                    ends_room -= 1;
                } else {
                    lost += 1;
                }
            } else if skip > 0 {
                skip -= 1;
                lost += 1;
            } else {
                self.merged_notes.push(*n);
            }
        }
        self.merged_notes.extend_from_slice(&notes[..own]);
        self.params.clear();
        self.notes.clear();
        (lost, reset)
    }

    /// What the next job takes after dry blocks: the carried events merged with the
    /// block's own (`Some(lost)`), after a reset when the carried note-offs would find no
    /// room (`run_reset`, which says whether it finished in time). A reset that ran late
    /// leaves the carry whole and keeps this block's own events in it (`None`: the block
    /// passes dry; review isolation-4 #9, #15).
    fn deliver(
        &mut self,
        params: &[ParamChange],
        notes: &[NoteEvent],
        run_reset: impl FnOnce() -> bool,
    ) -> Option<u64> {
        if self.needs_reset_first(notes.len()) {
            if !run_reset() {
                self.keep(params, notes);
                return None;
            }
            self.reset_delivered();
        }
        Some(self.merge(params, notes).0)
    }
}

/// The audio half.
pub struct BridgeAudio {
    slot: Arc<Slot>,
    /// The helper's lobby: kept mapped while the caller points into it, and read for
    /// whether the helper is stalled (review isolation-4 #62).
    lobby: Arc<super::sandbox::LobbyMem>,
    caller: Caller,
    status: Arc<Status>,
    /// This half's job that missed its deadline and runs on, kept here — not only in
    /// [`Status`], which the interface reads too — so its events are taken however the
    /// two threads interleave (review isolation-4 #0, #11).
    late_job: Option<u32>,
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
            lobby,
            caller,
            status,
            late_job: None,
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
        // This half's late job, or a previous half's of the same instance.
        if let Some(n) = self.late_job.or_else(|| self.status.outstanding()) {
            if !self.caller.finished(n) {
                return false;
            }
            // Its audio is too late to play; what the plug-in reported in it is not.
            // SAFETY: the late job finished and no other is in flight: the area is ours.
            unsafe { self.slot.take_events(&mut self.late_out) };
            self.late_job = None;
            self.status.clear();
        }
        if self.offline.is_none() && self.lobby.stalled() {
            // No processing thread of the helper took a job of it in time and none has
            // finished one since: this instance would wait its whole deadline too, in the
            // same callback — it passes dry at once (review isolation-4 #62).
            return false;
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
                self.late_job = Some(n);
                self.status.set_outstanding(n);
                self.status.late.fetch_add(1, Ordering::Relaxed);
                // SAFETY: the exchange is in the slot's mapping, which `self.slot` keeps.
                let x = unsafe { &*self.slot.exchange() };
                if x.pending() {
                    // Not even taken: no processing thread was free for it.
                    self.lobby.mark_stalled();
                }
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
        let mut started = Instant::now();
        let mut last = started;
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
                    let now = Instant::now();
                    if slice_paused(now - last, o.sandbox.limits().silence) {
                        started = now;
                    }
                    last = now;
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
            let mut carry = std::mem::replace(&mut self.carry, Carry::empty());
            // A carried note-off with no room: the plug-in is reset first, so no voice
            // rings on.
            let delivered = carry.deliver(params, notes, || {
                // SAFETY: no job is in flight (`ready`): the slot's job area is ours.
                unsafe {
                    self.slot
                        .put_job(op::RESET, &[], &[], None, None, &[], &[], false)
                };
                self.run(0)
            });
            self.carry = carry;
            match delivered {
                Some(lost) => out.lost_in += lost,
                None => {
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
                self.carry.reset_now();
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::NoteExpression;

    fn midi(status: u8, data1: u8, data2: u8) -> NoteEvent {
        NoteEvent {
            offset: 3,
            note_id: -1,
            channel: 0,
            key: 0,
            kind: NoteKind::Midi {
                status,
                data1,
                data2,
            },
        }
    }

    fn off(key: u8) -> NoteEvent {
        NoteEvent {
            offset: 1,
            note_id: -1,
            channel: 0,
            key,
            kind: NoteKind::Off { velocity: 0.0 },
        }
    }

    fn pressure(key: u8, value: f64) -> NoteEvent {
        NoteEvent {
            offset: 2,
            note_id: -1,
            channel: 0,
            key,
            kind: NoteKind::Expression {
                expression: NoteExpression::Pressure,
                value,
            },
        }
    }

    /// What the next job takes, as (status or kind, data1, data2).
    fn merged(c: &mut Carry, notes: &[NoteEvent]) -> Vec<NoteEvent> {
        let _ = c.merge(&[], notes);
        c.merged_notes.clone()
    }

    /// A wheel swept through a long dry stretch reaches the plug-in as its last value
    /// (review isolation-4 #33).
    #[test]
    fn a_wheel_sweep_folds_to_its_last_value() {
        let mut c = Carry::new();
        for i in 0..2_000u32 {
            c.keep(&[], &[midi(0xE0, (i & 0x7F) as u8, (i >> 7) as u8 & 0x7F)]);
        }
        assert_eq!(c.notes.len(), 1);
        assert_eq!(c.notes[0].kind, midi(0xE0, (1999 & 0x7F) as u8, 15).kind);
        assert_eq!(c.notes[0].offset, 0, "carried events go at the block's start");
    }

    /// A sustain pedal lifted and pressed again while the plug-in was late: both reach it,
    /// so the old chord stops ringing (review isolation-4 #14).
    #[test]
    fn a_pedal_lift_and_press_both_reach_the_plugin() {
        let mut c = Carry::new();
        c.keep(&[], &[midi(0xB0, 64, 0)]);
        c.keep(&[], &[midi(0xB0, 64, 127)]);
        let m = merged(&mut c, &[]);
        let kinds: Vec<NoteKind> = m.iter().map(|e| e.kind).collect();
        assert_eq!(kinds, vec![midi(0xB0, 64, 0).kind, midi(0xB0, 64, 127).kind]);
    }

    /// The carried events keep the order they came in: a pedal pressed before a note-off
    /// still holds that note (review isolation-4 #14).
    #[test]
    fn carried_events_keep_their_order() {
        let mut c = Carry::new();
        c.keep(&[], &[midi(0xB0, 64, 127), off(60)]);
        let m = merged(&mut c, &[]);
        assert_eq!(m.len(), 2);
        assert_eq!(m[0].kind, midi(0xB0, 64, 127).kind);
        assert!(matches!(m[1].kind, NoteKind::Off { .. }));
    }

    /// A bank select stays before the program change that follows it, whatever was folded
    /// (review isolation-4 #14).
    #[test]
    fn a_bank_select_stays_before_its_program_change() {
        let mut c = Carry::new();
        c.keep(
            &[],
            &[
                midi(0xC0, 1, 0),
                midi(0xB0, 0, 2),
                midi(0xB0, 32, 0),
                midi(0xC0, 5, 0),
            ],
        );
        let m = merged(&mut c, &[]);
        let kinds: Vec<NoteKind> = m.iter().map(|e| e.kind).collect();
        assert_eq!(
            kinds,
            vec![
                midi(0xB0, 0, 2).kind,
                midi(0xB0, 32, 0).kind,
                midi(0xC0, 5, 0).kind
            ]
        );
    }

    /// Polyphonic pressure is one value per key, not per channel (review isolation-4 #65).
    #[test]
    fn poly_pressure_is_kept_per_key() {
        let mut c = Carry::new();
        c.keep(&[], &[midi(0xA0, 60, 10), midi(0xA0, 64, 20), midi(0xA0, 60, 30)]);
        let m = merged(&mut c, &[]);
        let kinds: Vec<NoteKind> = m.iter().map(|e| e.kind).collect();
        assert_eq!(kinds, vec![midi(0xA0, 64, 20).kind, midi(0xA0, 60, 30).kind]);
    }

    /// A reset asked for while the plug-in was late ends its voices, not its controllers:
    /// a pitch bend back to centre or a pedal lift after it still reaches the plug-in; the
    /// note-offs and per-note expressions it supersedes do not (review isolation-4 #8, #33).
    #[test]
    fn controllers_after_a_reset_are_kept() {
        let mut c = Carry::new();
        c.keep(&[], &[off(60), pressure(61, 0.5), midi(0xB0, 1, 9)]);
        c.reset_now();
        c.keep(&[], &[midi(0xE0, 0, 64), midi(0xB0, 64, 0), off(62), pressure(62, 1.0)]);
        assert!(c.reset);
        let kinds: Vec<NoteKind> = c.notes.iter().map(|e| e.kind).collect();
        assert_eq!(
            kinds,
            vec![
                midi(0xB0, 1, 9).kind,
                midi(0xE0, 0, 64).kind,
                midi(0xB0, 64, 0).kind
            ]
        );
    }

    /// Notes that overflow the carry reset the plug-in: the note-offs go (the reset ends
    /// every voice), the controllers stay (review isolation-4 #14, #65).
    #[test]
    fn an_overflow_resets_and_keeps_the_controllers() {
        let mut c = Carry::new();
        c.keep(&[], &[midi(0xB0, 7, 100)]);
        let offs: Vec<NoteEvent> = (0..MAX_NOTES).map(|k| off((k % 128) as u8)).collect();
        c.keep(&[], &offs);
        assert!(c.reset, "no room: a reset instead");
        c.keep(&[], &[midi(0xB0, 64, 0)]);
        let kinds: Vec<NoteKind> = c.notes.iter().map(|e| e.kind).collect();
        assert_eq!(kinds, vec![midi(0xB0, 7, 100).kind, midi(0xB0, 64, 0).kind]);
    }

    /// Carried controllers that find no room beside the block's own events are counted as
    /// lost, the oldest first (review isolation-4 #15, #65).
    #[test]
    fn carried_controllers_without_room_are_counted() {
        let mut c = Carry::new();
        let ccs: Vec<NoteEvent> = (0..10u8).map(|n| midi(0xB0, 10 + n, 1)).collect();
        c.keep(&[], &ccs);
        let own: Vec<NoteEvent> = (0..MAX_NOTES - 4).map(|k| off((k % 128) as u8)).collect();
        let (lost, _) = c.merge(&[], &own);
        assert_eq!(lost, 6);
        assert_eq!(c.merged_notes.len(), MAX_NOTES);
        // The newest four carried ones, then the block's own.
        assert_eq!(c.merged_notes[0].kind, midi(0xB0, 16, 1).kind);
        assert!(matches!(c.merged_notes[4].kind, NoteKind::Off { .. }));
    }

    /// The block's own note-off is never pushed out by carried events (review isolation-4
    /// #33).
    #[test]
    fn the_blocks_own_note_off_is_kept() {
        let mut c = Carry::new();
        let ccs: Vec<NoteEvent> = (0..MAX_NOTES)
            .map(|n| midi(0xB0, 6, (n % 128) as u8))
            .collect();
        c.keep(&[], &ccs);
        let (_, _) = c.merge(&[], &[off(60)]);
        assert!(c
            .merged_notes
            .iter()
            .any(|e| matches!(e.kind, NoteKind::Off { .. }) && e.key == 60));
    }

    /// Parameters past what the slot carries are counted, not dropped unseen.
    #[test]
    fn parameters_past_the_slots_room_are_counted() {
        let mut c = Carry::new();
        let ps: Vec<ParamChange> = (0..MAX_PARAMS as u32 + 3)
            .map(|id| ParamChange {
                offset: 0,
                id,
                value: 0.5,
            })
            .collect();
        c.keep(&ps, &[]);
        let (lost, _) = c.merge(&[], &[]);
        assert_eq!(lost, 3);
    }
    /// Carried note-offs with no room beside the block's own events: the plug-in is reset
    /// first, decided before anything is merged (review isolation-4 #15, #33).
    #[test]
    fn a_carried_note_off_without_room_asks_for_a_reset_first() {
        let mut c = Carry::new();
        let offs: Vec<NoteEvent> = (0..10).map(|k| off(k as u8)).collect();
        c.keep(&[], &offs);
        assert!(!c.needs_reset_first(MAX_NOTES - 10));
        assert!(c.needs_reset_first(MAX_NOTES - 9));
        let mut resets = 0;
        let own: Vec<NoteEvent> = (0..MAX_NOTES - 9).map(|_| midi(0xB0, 6, 1)).collect();
        let lost = c.deliver(&[], &own, || {
            resets += 1;
            true
        });
        assert_eq!(resets, 1);
        assert_eq!(lost, Some(0), "the reset superseded the note-offs: nothing lost");
        assert_eq!(c.merged_notes.len(), MAX_NOTES - 9);
    }

    /// When that reset runs late, the carry stays whole and keeps the block's own events
    /// too: nothing merged is dropped unseen (review isolation-4 #9, #15).
    #[test]
    fn a_late_reset_first_keeps_the_carry_and_the_blocks_events() {
        let mut c = Carry::new();
        let offs: Vec<NoteEvent> = (0..300).map(|k| off((k % 128) as u8)).collect();
        c.keep(
            &[ParamChange {
                offset: 0,
                id: 4,
                value: 0.25,
            }],
            &offs,
        );
        let own_params = [ParamChange {
            offset: 7,
            id: 5,
            value: 0.75,
        }];
        // The block's own events: a wheel's 300 steps (one value carried).
        let own: Vec<NoteEvent> = (0..300).map(|v| midi(0xB0, 1, (v % 128) as u8)).collect();
        let lost = c.deliver(&own_params, &own, || false);
        assert_eq!(lost, None, "the block passes dry");
        let ids: Vec<u32> = c.params.iter().map(|p| p.id).collect();
        assert_eq!(ids, vec![4, 5], "both values carried on");
        assert_eq!(c.notes.iter().filter(|n| ends_a_note(n)).count(), 300);
        assert_eq!(c.notes.last().map(|n| n.kind), Some(midi(0xB0, 1, 43).kind));
        assert_eq!(c.lost, 0);
    }

    /// A slice of an offline wait that took far longer than asked is a pause of Cantus, not
    /// the plug-in's hang (review isolation-4 #22, #36).
    #[test]
    fn a_long_slice_is_a_pause() {
        let s = Duration::from_secs(5);
        assert!(!slice_paused(OFFLINE_SLICE, s));
        assert!(!slice_paused(Duration::from_secs(4), s));
        assert!(slice_paused(Duration::from_secs(60), s));
    }
}

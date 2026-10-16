//! An isolated plug-in's audio half, in Cantus (ADR 0063): the engine calls it exactly as
//! it calls an in-process plug-in's, and it hands each block to the helper in lock-step.
//!
//! Per block: the input, the sidechain, the parameter changes, the notes and the transport
//! are copied into the instance's shared slot; the job is published; the thread spins for a
//! bounded time ([`SPIN`]), then sleeps on its bell until a **deadline** ([`deadline`]); the
//! helper's output and its parameter events are copied back. Nothing allocates.
//!
//! When the deadline passes, the block goes on **without** the plug-in — the effect's input
//! passes through (the buffers were never touched), an instrument stays silent (the caller
//! zeroed them) — and the late job is never waited for again: the next blocks pass dry
//! until it has finished, counted in [`Status::late`]. When the helper is gone, every block
//! passes dry from then on. An **offline** instance (a render, an export) has no deadline:
//! it waits for every block as long as it takes, while its helper lives, so the render
//! equals the in-process one to the bit.
//!
//! This amends ADR 0005 rule 3 for this one case (ADR 0063): the wait is the plug-in's own
//! processing time, moved to another process; it is bounded by the deadline, and the ring
//! that wakes a parked helper is a non-blocking system call.

use super::slot::{op, status, Slot};
use crate::backend::{Audio, AudioThread, NoteEvent, ParamChange, ParamOut};
use cantus_rt::ipc::{Caller, Outcome};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// How long the audio thread spins for the helper's answer before it sleeps on its bell:
/// enough for a light plug-in's whole job, so most blocks make no system call to wait.
pub const SPIN: Duration = Duration::from_micros(200);

/// The deadline of a block of `frames` at `sample_rate`: twice the block's own duration,
/// at least 5 ms. A plug-in that takes longer was going to make the device drop out in
/// process too; out of process, the block is cut there instead of waiting on.
pub fn deadline(frames: usize, sample_rate: f64) -> Duration {
    let block = Duration::from_secs_f64(frames as f64 / sample_rate.max(1.0));
    (block * 2).max(Duration::from_millis(5))
}

/// What Cantus knows of one isolated instance across its halves.
#[derive(Debug)]
pub struct Status {
    dead: AtomicBool,
    /// Blocks that passed without the plug-in (late, or waiting for a late one).
    pub late: AtomicU64,
    /// Milliseconds since `epoch` when the unfinished job was published; 0 for none.
    since_ms: AtomicU64,
    epoch: Instant,
}

impl Default for Status {
    fn default() -> Self {
        Status {
            dead: AtomicBool::new(false),
            late: AtomicU64::new(0),
            since_ms: AtomicU64::new(0),
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

    /// How long the unfinished job has been out, if one is.
    pub fn unfinished_for(&self) -> Option<Duration> {
        let since = self.since_ms.load(Ordering::Acquire);
        (since != 0).then(|| {
            let now = self.epoch.elapsed().as_millis() as u64;
            Duration::from_millis(now.saturating_sub(since))
        })
    }

    fn now_ms(&self) -> u64 {
        (self.epoch.elapsed().as_millis() as u64).max(1)
    }
}

/// The audio half.
pub struct BridgeAudio {
    slot: Arc<Slot>,
    caller: Caller,
    status: Arc<Status>,
    /// A job that went past its deadline and has not finished.
    outstanding: Option<u32>,
    offline: bool,
    sample_rate: f64,
    /// Told when this half is dropped (on the main thread), so the helper drops its own.
    on_drop: Option<Box<dyn FnOnce() + Send>>,
}

impl std::fmt::Debug for BridgeAudio {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BridgeAudio")
            .field("slot", &self.slot.name())
            .field("offline", &self.offline)
            .finish()
    }
}

impl BridgeAudio {
    pub(crate) fn new(
        slot: Arc<Slot>,
        caller: Caller,
        status: Arc<Status>,
        offline: bool,
        sample_rate: f64,
        on_drop: Box<dyn FnOnce() + Send>,
    ) -> BridgeAudio {
        BridgeAudio {
            slot,
            caller,
            status,
            outstanding: None,
            offline,
            sample_rate,
            on_drop: Some(on_drop),
        }
    }

    /// Whether the plug-in may be given a job now: alive, and no late job unfinished.
    fn ready(&mut self) -> bool {
        if self.status.is_dead() {
            return false;
        }
        if let Some(n) = self.outstanding {
            if !self.caller.finished(n) {
                self.status.late.fetch_add(1, Ordering::Relaxed);
                return false;
            }
            self.outstanding = None;
            self.status.since_ms.store(0, Ordering::Release);
        }
        true
    }

    /// Publishes the job written in the slot and waits for it. `true` when it finished.
    fn run(&mut self, frames: usize) -> bool {
        let n = self.caller.submit();
        let deadline = (!self.offline)
            .then(|| Instant::now() + deadline(frames, self.sample_rate));
        match self.caller.wait(n, SPIN, deadline) {
            Outcome::Done => true,
            Outcome::Late => {
                self.outstanding = Some(n);
                self.status
                    .since_ms
                    .store(self.status.now_ms(), Ordering::Release);
                self.status.late.fetch_add(1, Ordering::Relaxed);
                false
            }
            Outcome::Closed => {
                self.status.mark_dead();
                false
            }
        }
    }

    /// A job with no audio (a reset, a stop).
    fn control(&mut self, op: u32) {
        if !self.ready() {
            return;
        }
        // SAFETY: no job is in flight (`ready`), so the slot's job area is ours.
        unsafe {
            self.slot
                .put_job(op, &[], &[], None, None, &[], &[], false);
        }
        self.run(0);
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
        if !self.ready() {
            return; // dry for an effect, silent for an instrument
        }
        let frames = audio
            .left
            .len()
            .min(audio.right.len())
            .min(self.slot.max_block());
        let left = &mut audio.left[..frames];
        let right = &mut audio.right[..frames];
        let flush = cantus_rt::denormal::flushing();
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
        }
    }

    fn reset(&mut self) {
        self.control(op::RESET);
    }

    fn stop(&mut self) {
        self.control(op::STOP);
    }
}

impl Drop for BridgeAudio {
    fn drop(&mut self) {
        if let Some(f) = self.on_drop.take() {
            f();
        }
    }
}

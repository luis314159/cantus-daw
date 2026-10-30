//! The audio thread's side of sync (round 33, ADR 0078 §4, ADR 0079): once per block it
//! fits its clock to host time, follows the source's timeline if one is chosen, and makes
//! MIDI clock and time code; once per pass it stamps the clocks and quarter frames due in
//! the pass. A processor never given a seam ([`Processor::attach_sync`]) — every offline
//! render, every test before round 33 — returns at the first line: nothing here touches a
//! sample.
//!
//! **Following** is sample-accurate where the transport is: a start lands on the sample the
//! source asks for (a fractional seek, `Transport::seek_f64`); then the tempo is the
//! source's, corrected by the device's measured rate against the host clock and by a
//! proportional pull on the phase error (`PULL_S`), added as a new stretch of the followed
//! position (`transport_follow.rs`) so nothing already played moves. An error past
//! [`RELOCK_US`] — a locate at the master, a session joined — jumps instead.
//! **Latency.** The block's first engine sample is heard at its host time plus the
//! device's output latency plus the delay compensation the engine renders ahead by; that
//! moment is what is matched with the source's beat, and what clock out is stamped with.

use super::*;
use crate::transport::sync::{
    clock, mtc, HostTimeFilter, Lock, OutKind, Proposal, SourceKind, Stamped, SyncPort, Timeline,
};
use cantus_core::PPQ;
use std::sync::atomic::Ordering;

// Following a source, and making clock and time code.
#[path = "processor_sync_follow.rs"]
mod follow;
#[path = "processor_sync_out.rs"]
mod out;

/// A phase error is pulled in over this long (a proportional loop): 1 ms of error makes
/// the tempo 0.07 % faster or slower, well below hearing.
const PULL_S: f64 = 0.3;
/// The most the pull changes the tempo.
const PULL_MAX: f64 = 0.01;
/// An error past this is a jump of the source, followed by a jump (µs).
const RELOCK_US: f64 = 30_000.0;
/// Within this the follower says *locked* (µs).
const LOCKED_US: f64 = 1_000.0;
/// The tempo is retuned at most this often (µs), unless the source's own tempo moved, and
/// only for a change past [`RETUNE_PPM`]: each retune is a stretch of the followed
/// position, and few stretches keep a long audio clip on the one it started in.
const RETUNE_US: i64 = 500_000;
const RETUNE_PPM: f64 = 10e-6;
/// An error within this is no reason to pull (µs): the source's own jitter.
const DEADBAND_US: f64 = 150.0;
/// Ticks per MIDI clock.
const CLOCK_TICKS: f64 = PPQ as f64 / clock::PER_BEAT;

/// The sync state the processor keeps (boxed: the filter is a few kilobytes).
#[derive(Debug)]
pub(crate) struct SyncRun {
    port: SyncPort,
    /// The host clock (`sync::host_micros`; a test's own).
    now: fn() -> i64,
    telemetry: Option<Arc<crate::devices::DeviceTelemetry>>,
    filter: HostTimeFilter,
    /// The source being followed now, and Link's beat offset (song beats − session beats).
    locked: Option<SourceKind>,
    offset: f64,
    epoch: u32,
    /// A start waits for its moment; a stop of ours waits for the source to stop too.
    queued: bool,
    hold: bool,
    /// The device's rate against the host clock (µs per sample over the nominal),
    /// smoothed over ten seconds: the filter's slope moves by ppm with every late callback.
    drift: f64,
    /// The last timeline read whole: a read that keeps meeting a write keeps it (review
    /// r33 #2: it once read as "no source" and unfollowed).
    last_tl: Timeline,
    /// Whether the last block did sync's work: on the edge from idle the clock is fitted
    /// afresh (round 2, D4: an hours-old fit was used for up to 31 blocks).
    was_active: bool,
    /// The base tempo and the last retune.
    base_bpm: f64,
    retuned: i64,
    /// What the transport did at the end of the last block (the user's own play and stop
    /// are told from ours by it).
    was_playing: bool,
    /// This block's start: when it is heard (host µs), its tick and ticks per sample — what
    /// a tempo proposed at a tick is timed by.
    block_heard: i64,
    block_tick: f64,
    block_tps: f64,
    // ---- clock and time code out ----
    out_was_playing: bool,
    out_pos: f64,
    next_clock: f64,
    qf: Option<mtc::QfWriter>,
    expect: f64,
}

impl SyncRun {
    fn push(&mut self, m: Stamped) {
        if self.port.out.push(m).is_err() {
            self.port.shared.dropped.fetch_add(1, Ordering::Relaxed);
        }
    }
}

impl Processor {
    /// Give the processor its end of the sync seam and the device's telemetry (its output
    /// latency). Built off the audio thread, before the processor moves there.
    pub fn attach_sync(
        &mut self,
        port: SyncPort,
        telemetry: Option<Arc<crate::devices::DeviceTelemetry>>,
    ) {
        self.attach_sync_clocked(port, telemetry, crate::transport::sync::host_micros);
    }

    /// [`Self::attach_sync`] reading host time from `now` (a test's simulated clock).
    pub fn attach_sync_clocked(
        &mut self,
        port: SyncPort,
        telemetry: Option<Arc<crate::devices::DeviceTelemetry>>,
        now: fn() -> i64,
    ) {
        self.sync = Some(Box::new(SyncRun {
            port,
            now,
            telemetry,
            filter: HostTimeFilter::new(self.sample_rate),
            locked: None,
            offset: 0.0,
            epoch: 0,
            queued: false,
            hold: false,
            drift: 1.0,
            last_tl: Timeline::default(),
            was_active: false,
            base_bpm: 0.0,
            retuned: i64::MIN / 2,
            was_playing: false,
            block_heard: 0,
            block_tick: 0.0,
            block_tps: 0.0,
            out_was_playing: false,
            out_pos: f64::NAN,
            next_clock: f64::NAN,
            qf: None,
            expect: f64::NAN,
        }));
    }

    /// The block's sync, after the messages and before anything renders.
    pub(in crate::processor) fn sync_block(&mut self, frames: usize) {
        let Some(mut run) = self.sync.take() else {
            return;
        };
        let sr = self.sample_rate;
        if let Some(t) = run.port.shared.timeline.read() {
            run.last_tl = t;
        }
        let tl = run.last_tl;
        let out = &run.port.shared.out;
        let active = tl.kind != SourceKind::None
            || self.transport.following()
            || out.clock.load(Ordering::Relaxed)
            || out.mtc.load(Ordering::Relaxed);
        if active && !run.was_active {
            run.filter.reset();
        }
        run.was_active = active;
        if !active {
            // Nothing followed, nothing sent: no clock read, no fit (review r33 #7) — only
            // what an idle seam keeps up to date.
            run.locked = None;
            run.queued = false;
            run.port.shared.set_lock(Lock::Idle);
            self.out_block(&mut run);
            run.was_playing = self.transport.is_playing();
            self.sync = Some(run);
            return;
        }
        let now = (run.now)();
        run.filter.add(self.clock, now);
        let mut line = run.filter.line();
        line.latency_us = run
            .telemetry
            .as_ref()
            .map_or(0.0, |t| f64::from(t.output_latency()) / sr * 1e6);
        run.port.shared.host_line.publish(&line);
        let ahead = self.pdc.total as f64;
        // When this block's first engine sample reaches the speakers.
        let heard = line.host_of(self.clock) + line.latency_us + ahead / sr * 1e6;
        let heard_us = heard.round() as i64;
        run.block_heard = heard_us;
        run.block_tick = self.transport.position_f64();
        run.block_tps = self.transport.ticks_per_sample();
        // The device's real µs per sample against the nominal: the follower's tempo is
        // corrected by it, so a device a few ppm fast does not drift from the source.
        let raw = line.b * sr / 1e6;
        let k = (frames as f64 / (10.0 * sr)).min(1.0);
        run.drift += (raw - run.drift) * k;
        let drift = run.drift;
        match tl.kind {
            SourceKind::None => {
                if self.transport.following() {
                    self.transport.unfollow();
                }
                run.locked = None;
                run.queued = false;
                run.port.shared.set_lock(Lock::Idle);
            }
            SourceKind::Link => self.follow_link(&mut run, &tl, heard_us, frames, drift),
            SourceKind::Clock | SourceKind::Mtc => {
                self.follow_absolute(&mut run, &tl, heard_us, drift)
            }
        }
        run.port.shared.tempo_bits.store(
            if self.transport.following() {
                self.transport.bpm().to_bits()
            } else {
                0f64.to_bits()
            },
            Ordering::Relaxed,
        );
        self.out_block(&mut run);
        run.was_playing = self.transport.is_playing();
        self.sync = Some(run);
    }

    /// Propose `bpm` to the followed session (Link) from the moment `tick` is heard — the
    /// session conductor's scene tempo at its launch, a tempo set on purpose. Nothing else
    /// proposes: a new map is no tempo change of the user's (integrated review #7).
    pub fn propose_tempo(&mut self, bpm: f64, tick: f64) {
        let Some(run) = self.sync.as_mut() else {
            return;
        };
        if run.last_tl.kind != SourceKind::Link || !(bpm.is_finite() && bpm > 0.0) {
            return;
        }
        let ahead = if run.block_tps > 0.0 {
            (tick - run.block_tick) / run.block_tps / self.sample_rate * 1e6
        } else {
            0.0
        };
        let at = run.block_heard.saturating_add(ahead.round() as i64);
        if run
            .port
            .proposals
            .push(Proposal::Tempo { bpm, at })
            .is_err()
        {
            run.port.shared.dropped.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// The stop key (the message, whatever the transport did): a start waiting for its
    /// moment is called off, and a source that plays on is not followed until it stops
    /// (review r33 #8: a queued start began at the quantum anyway).
    pub(in crate::processor) fn sync_stop_pressed(&mut self) {
        if let Some(run) = self.sync.as_mut() {
            if run.queued {
                run.queued = false;
                run.hold = run.last_tl.playing;
                if run.last_tl.kind == SourceKind::Link && run.last_tl.start_stop {
                    let at = (run.now)();
                    let _ = run.port.proposals.push(Proposal::Stop { at });
                }
            }
        }
    }

    /// Start the transport so that it reaches `tick` `k` samples into this block, at
    /// `bpm` (the play message's own steps, at a fractional place).
    fn sync_start(&mut self, tick: f64, k: f64, bpm: f64) {
        let tps = bpm * PPQ as f64 / (60.0 * self.sample_rate);
        self.release_all();
        self.transport.follow(bpm);
        self.transport.seek_f64(tick - k * tps);
        self.reseek_events();
        self.transport.play();
        self.chase_controls();
        self.played = 0;
    }

    /// Jump while playing (a locate at the source).
    fn sync_jump(&mut self, tick: f64) {
        self.release_all();
        self.transport.seek_f64(tick);
        self.reseek_events();
        self.played = 0;
    }

    /// Retune to `bpm`, now if the base tempo moved, else at most every `RETUNE_US`.
    fn sync_retune(run: &mut SyncRun, t: &mut Transport, base: f64, bpm: f64, now: i64) {
        let moved = (base - run.base_bpm).abs() > 1e-9;
        if moved || now - run.retuned >= RETUNE_US {
            if moved || (bpm / t.bpm() - 1.0).abs() > RETUNE_PPM {
                t.follow(bpm);
                run.retuned = now;
            }
            run.base_bpm = base;
        }
    }

    fn report(run: &SyncRun, err_us: f64) {
        let s = &run.port.shared;
        s.error_us.store(err_us.round() as i64, Ordering::Relaxed);
        s.set_lock(if err_us.abs() <= LOCKED_US {
            Lock::Locked
        } else {
            Lock::Seeking
        });
    }
}

//! Following an outside clock (round 33, ADR 0078 §4, ADR 0079 §2–§3), measured: a
//! processor driven block by block on a simulated host clock, its device 50 ppm fast and
//! its callback's wake-up jittering ±1 ms around its mean, following a Link session, a jittery MIDI clock
//! (±1 ms, ±3 ms, a tempo ramp) and time code. The error is taken against the **true**
//! source — what it played at the moment our block is heard — never against the
//! follower's own view of it.

use cantus_core::*;
use cantus_engine::transport::sync::{self, clock::ClockFollower, mtc, Lock, SourceKind, Timeline};
use cantus_engine::*;
use std::cell::Cell;
use std::sync::Arc;

const SR: f64 = 48_000.0;
const BLOCK: usize = 256;
/// The device's clock against the host's (fast) and the callback's lateness.
const DRIFT: f64 = 50e-6;
/// Each callback wakes this much either side of its mean (the reported output latency is
/// from the mean: the driver's playback time minus the callback's, smoothed).
const JITTER_US: f64 = 1_000.0;
const T0: f64 = 5_000_000.0;

thread_local! {
    static NOW: Cell<i64> = const { Cell::new(0) };
}
fn now() -> i64 {
    NOW.with(|c| c.get())
}

struct Rig {
    tx: rtrb::Producer<EngineMessage>,
    rx: rtrb::Consumer<EngineEvent>,
    p: Processor,
    h: sync::SyncHandle,
    /// Blocks run, and a small random source for the lateness.
    blocks: u64,
    rng: u64,
}

fn rig(bpm: f64) -> Rig {
    let mut project = Project::new("follow");
    project.tempo_bpm = bpm;
    let (mut tx, rxe) = rtrb::RingBuffer::new(64);
    let (txe, rx) = rtrb::RingBuffer::new(8192);
    let mut p = Processor::without_midi(SR, rxe, txe);
    let (port, h) = sync::pair();
    p.attach_sync_clocked(port, None, now);
    let _ = tx.push(EngineMessage::SetSnapshot(Arc::new(Snapshot::compile(
        &project, 1,
    ))));
    p.settle();
    Rig {
        tx,
        rx,
        p,
        h,
        blocks: 0,
        rng: 0x9E37_79B9,
    }
}

impl Rig {
    /// The true host time of the processor's sample `s` (the device's real rate).
    fn true_time(&self, s: f64) -> f64 {
        T0 + s * 1e6 / (SR * (1.0 + DRIFT))
    }
    /// The engine sample the next block starts at.
    fn sample(&self) -> f64 {
        (self.blocks * BLOCK as u64) as f64
    }
    /// Run one block: the callback wakes late, the processor reads the clock then.
    fn block(&mut self) {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        let late = ((self.rng % 1000) as f64 / 500.0 - 1.0) * JITTER_US;
        NOW.with(|c| c.set((self.true_time(self.sample()) + late) as i64));
        let mut buf = vec![0f32; BLOCK * 2];
        self.p.process(&mut buf, 2);
        while self.rx.pop().is_ok() {}
        self.blocks += 1;
    }
    fn beats(&self) -> f64 {
        self.p.transport().position_f64() / PPQ as f64
    }
}

/// `e` folded into a quantum's half either side.
fn wrap(e: f64, q: f64) -> f64 {
    (e + q / 2.0).rem_euclid(q) - q / 2.0
}

/// Link at 120 BPM in 4, start/stop on: Cantus starts on its own in phase, locks within a
/// millisecond and stays there; a tempo change to 132 is followed and locked again; the
/// position never jumps once locked (the stretches only add).
#[test]
fn following_link_locks_within_a_millisecond() {
    let mut r = rig(100.0);
    let mut tl = Timeline {
        kind: SourceKind::Link,
        tempo: 120.0,
        host: 0,
        position: 0.0,
        rate: 1.0,
        playing: true,
        quantum: 4.0,
        start_stop: true,
        epoch: 1,
    };
    r.h.shared.timeline.publish(&tl);
    let blocks = (30.0 * SR / BLOCK as f64) as u64;
    let change = blocks / 2;
    let (mut worst1, mut worst2) = (0.0f64, 0.0f64);
    let mut prev: Option<f64> = None;
    for b in 0..blocks {
        if b == change {
            let at = r.true_time(r.sample()) as i64;
            tl = Timeline {
                tempo: 132.0,
                host: at,
                position: tl.beat_at(at),
                ..tl
            };
            r.h.shared.timeline.publish(&tl);
        }
        // Where we are as this block starts, against the session at the moment it is heard.
        let ours = r.beats();
        let truth = tl.beat_at(r.true_time(r.sample()) as i64);
        if r.p.transport().is_playing() {
            let err_ms = wrap(ours - truth, 4.0) * 60_000.0 / tl.tempo;
            let since = if b >= change { b - change } else { b };
            if since as f64 * BLOCK as f64 / SR > 6.0 {
                if b >= change {
                    worst2 = worst2.max(err_ms.abs());
                } else {
                    worst1 = worst1.max(err_ms.abs());
                }
            }
        }
        r.block();
        // The block's own tempo (a retune is at its start): it ended one step on — no
        // jump once locked.
        if r.p.transport().is_playing() {
            let step = BLOCK as f64 * r.p.transport().bpm() / 60.0 / SR;
            if prev.is_some() {
                assert!(
                    (r.beats() - ours - step).abs() < 1e-9,
                    "a jump at block {b}"
                );
            }
            prev = Some(ours);
        }
    }
    println!(
        "Link 120 BPM: worst phase error {worst1:.4} ms; after the change to 132: {worst2:.4} ms; \
         stretches {}",
        r.p.transport().follow_stretches()
    );
    assert!(r.p.transport().following());
    assert_eq!(r.h.shared.lock_state(), Lock::Locked);
    assert!(worst1 <= 1.0 && worst2 <= 1.0, "{worst1} {worst2}");
}

/// Review r33 #2: a read of the timeline that keeps meeting a write (the source's thread
/// pre-empted between its stores) keeps the last good timeline — following goes on, locked;
/// it once read as "no source", unfollowed and cut the notes.
#[test]
fn a_failed_read_keeps_the_last_timeline() {
    let mut r = rig(120.0);
    let tl = Timeline {
        kind: SourceKind::Link,
        tempo: 120.0,
        host: 0,
        position: 0.0,
        rate: 1.0,
        playing: true,
        quantum: 4.0,
        start_stop: true,
        epoch: 1,
    };
    r.h.shared.timeline.publish(&tl);
    for _ in 0..(8.0 * SR / BLOCK as f64) as usize {
        r.block();
    }
    assert!(r.p.transport().following() && r.p.transport().is_playing());
    assert_eq!(r.h.shared.lock_state(), Lock::Locked);
    r.h.shared.timeline.hold_for_tests(true);
    for _ in 0..4 {
        r.block();
        assert!(r.p.transport().following(), "still following");
        assert!(r.p.transport().is_playing(), "still playing");
        assert_eq!(r.h.shared.lock_state(), Lock::Locked);
    }
    r.h.shared.timeline.hold_for_tests(false);
    r.block();
    assert_eq!(r.h.shared.lock_state(), Lock::Locked);
}

/// Review r33 #4: a Link start lands inside a block — the transport starts before tick 0
/// by the samples before it — and clock out sends Start, its first clock on the sample the
/// transport crosses tick 0. It sent the pointer to 0 and Continue.
#[test]
fn a_link_start_inside_a_block_sends_start() {
    let mut r = rig(120.0);
    r.h.shared
        .out
        .clock
        .store(true, std::sync::atomic::Ordering::Relaxed);
    let tl = Timeline {
        kind: SourceKind::Link,
        tempo: 120.0,
        host: 0,
        position: 0.3,
        rate: 1.0,
        playing: true,
        quantum: 4.0,
        start_stop: true,
        epoch: 1,
    };
    r.h.shared.timeline.publish(&tl);
    let mut started_at = None;
    for _ in 0..1_000 {
        let before = r.p.transport().is_playing();
        r.block();
        if !before && r.p.transport().is_playing() {
            started_at = Some(r.blocks - 1);
            break;
        }
    }
    let b = started_at.expect("started");
    let ring = r.h.out.as_mut().unwrap();
    let mut m = Vec::new();
    while let Ok(x) = ring.pop() {
        m.push(x);
    }
    assert_eq!(
        m[0].msg(),
        &[0xFA],
        "Start, not the pointer and Continue: {:?}",
        m[0].msg()
    );
    let first = m.iter().find(|x| x.msg() == [0xF8]).expect("a clock");
    // Tick 0 is crossed inside the start's block, not on its first sample.
    assert!(
        first.at > b * BLOCK as u64 && first.at < (b + 1) * BLOCK as u64,
        "{}",
        first.at
    );
}

/// Review r33 #8: a play waiting for the quantum is called off by a stop — the transport,
/// stopped all along, does not start at the quantum anyway.
#[test]
fn a_stop_calls_off_a_queued_start() {
    let mut r = rig(120.0);
    for start_stop in [false, true] {
        let tl = Timeline {
            kind: SourceKind::Link,
            tempo: 120.0,
            host: 0,
            position: 0.0,
            rate: 1.0,
            playing: start_stop,
            quantum: 4.0,
            start_stop,
            epoch: 1,
        };
        r.h.shared.timeline.publish(&tl);
        for _ in 0..20 {
            r.block();
        }
        let _ = r.tx.push(EngineMessage::Stop);
        r.block();
        let _ = r.tx.push(EngineMessage::Play);
        r.block();
        assert!(!r.p.transport().is_playing(), "queued");
        let _ = r.tx.push(EngineMessage::Stop);
        // Past a whole quantum (2 s at 120 BPM in 4).
        for _ in 0..(3.0 * SR / BLOCK as f64) as usize {
            r.block();
            assert!(
                !r.p.transport().is_playing(),
                "start/stop {start_stop}: started anyway"
            );
        }
        assert_ne!(r.h.shared.lock_state(), Lock::Queued);
    }
}

/// Link with start/stop sync off: our own play waits for the quantum and lands in phase.
#[test]
fn our_own_play_waits_for_the_quantum() {
    let mut r = rig(120.0);
    let tl = Timeline {
        kind: SourceKind::Link,
        tempo: 120.0,
        host: 0,
        position: 0.0,
        rate: 1.0,
        playing: false,
        quantum: 4.0,
        start_stop: false,
        epoch: 1,
    };
    r.h.shared.timeline.publish(&tl);
    for _ in 0..50 {
        r.block();
    }
    assert!(!r.p.transport().is_playing());
    // The user's play, as the play button sends it.
    let _ = r.tx.push(EngineMessage::Play);
    r.block();
    assert_eq!(r.h.shared.lock_state(), Lock::Queued, "waits for the bar");
    let mut started = None;
    for b in 0..400 {
        if r.p.transport().is_playing() && started.is_none() {
            started = Some(b);
            let truth = tl.beat_at(r.true_time(r.sample()) as i64);
            let err_ms = wrap(r.beats() - truth, 4.0) * 500.0;
            println!("own play: started in phase within {err_ms:.4} ms");
            assert!(err_ms.abs() <= 1.0);
        }
        r.block();
    }
    assert!(started.is_some());
}

/// The clock times a master sends: `tempo(t)` BPM (µs → BPM), from `t0` on.
fn master_clocks(tempo: impl Fn(f64) -> f64, t0: f64, secs: f64) -> Vec<f64> {
    let mut v = Vec::new();
    let (mut t, mut beats, mut next) = (t0, 0.0f64, 0.0f64);
    let dt = 5.0;
    while t < t0 + secs * 1e6 {
        if beats >= next {
            v.push(t);
            next += 1.0 / 24.0;
        }
        beats += tempo(t - t0) / 60e6 * dt;
        t += dt;
    }
    v
}

/// The master's true beat at `t`, by its clock list (between clocks, linear).
fn beat_of(clocks: &[f64], t: f64) -> f64 {
    let k = clocks.partition_point(|c| *c <= t);
    if k == 0 {
        return 0.0;
    }
    let k = k.min(clocks.len() - 1);
    let (a, b) = (clocks[k - 1], clocks[k]);
    (k - 1) as f64 / 24.0 + (t - a) / (b - a) / 24.0
}

/// Follow `clocks` (sent with up to `jitter` µs either way) and measure the worst error
/// after `settle` seconds, ms.
fn follow_clock(clocks: &[f64], jitter: f64, settle: f64, secs: f64) -> f64 {
    let mut r = rig(100.0);
    let mut f = ClockFollower::new();
    f.feed((clocks[0] - 10_000.0) as i64, &[0xFA]);
    let mut next = 0;
    let mut rng = 12345u64;
    let mut worst = 0.0f64;
    let mut fworst = 0.0f64;
    let blocks = (secs * SR / BLOCK as f64) as u64;
    for b in 0..blocks {
        let host = r.true_time(r.sample());
        if b as f64 * BLOCK as f64 / SR > settle {
            let ft = f.timeline(host as i64);
            if ft.playing {
                let e = (ft.beat_at(host as i64) - beat_of(clocks, host)) * 60_000.0 / f.tempo();
                fworst = fworst.max(e.abs());
            }
        }
        while next < clocks.len() && clocks[next] <= host {
            rng = rng
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let j = ((rng >> 33) as f64 / (1u64 << 31) as f64 * 2.0 - 1.0) * jitter;
            f.feed((clocks[next] + j) as i64, &[0xF8]);
            next += 1;
        }
        r.h.shared.timeline.publish(&f.timeline(host as i64));
        if r.p.transport().is_playing() && b as f64 * BLOCK as f64 / SR > settle {
            let err = (r.beats() - beat_of(clocks, host)) * 60_000.0 / f.tempo().max(1.0);
            worst = worst.max(err.abs());
        }
        r.block();
    }
    println!("  (the follower's own view: worst {fworst:.3} ms)");
    worst
}

#[test]
fn following_a_jittery_midi_clock() {
    let t0 = T0 + 500_000.0;
    let steady = master_clocks(|_| 120.0, t0, 30.0);
    let e1 = follow_clock(&steady, 1_000.0, 8.0, 30.0);
    let e3 = follow_clock(&steady, 3_000.0, 8.0, 30.0);
    let ramp = master_clocks(|t| 100.0 + 40.0 * (t / 10e6).min(1.0), t0, 30.0);
    let er = follow_clock(&ramp, 1_000.0, 2.0, 30.0);
    println!(
        "MIDI clock in: ±1 ms jitter → worst {e1:.3} ms; ±3 ms → {e3:.3} ms; \
         ramp 100→140 BPM over 10 s (±1 ms) → {er:.3} ms"
    );
    assert!(e1 <= 1.0, "{e1}");
    assert!(e3 <= 2.0, "{e3}");
    assert!(er <= 2.0, "{er}");
}

/// Time code at 25 fps with ±1 ms jitter, an hour's offset: chased within a millisecond
/// or so, through the project's map.
#[test]
fn chasing_time_code() {
    let mut r = rig(120.0);
    r.h.shared.out.set_offset_s(3600.0);
    let rate = mtc::Rate::Fps25;
    let mut chaser = mtc::MtcChaser::new();
    let start = T0 + 300_000.0;
    // The master's code: song second 0 (code 01:00:00:00) at `start`.
    let quarter = 1e6 / (4.0 * rate.fps());
    let mut w = mtc::QfWriter::at(3600.0, rate);
    let mut j = 0u64;
    let mut worst = 0.0f64;
    let mut rng = 99u64;
    let blocks = (20.0 * SR / BLOCK as f64) as u64;
    for b in 0..blocks {
        let host = r.true_time(r.sample());
        loop {
            let at = start + j as f64 * quarter;
            if at > host {
                break;
            }
            rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
            let jit = ((rng >> 33) as f64 / (1u64 << 31) as f64 * 2.0 - 1.0) * 1_000.0;
            let msg = w.take(rate);
            chaser.feed((at + jit) as i64, &msg);
            j += 1;
        }
        r.h.shared.timeline.publish(&chaser.timeline(host as i64));
        if r.p.transport().is_playing() && b as f64 * BLOCK as f64 / SR > 6.0 {
            let truth = (host - start) / 1e6; // song seconds
            let ours =
                r.p.transport()
                    .map()
                    .seconds_at(r.p.transport().position_f64());
            worst = worst.max(((ours - truth) * 1e3).abs());
        }
        r.block();
    }
    println!("time code 25 fps, ±1 ms: worst {worst:.3} ms");
    assert!(r.p.transport().is_playing());
    assert!(worst <= 1.5, "{worst}");
}

/// Integrated review #7: following Link, new snapshots — a projection with a tempo point a
/// bar ahead, installed again once the playhead is past it, then the song's map back —
/// propose no tempo to the session (each was a new map, and the tempo under the playhead
/// was proposed at whatever install came next). A tempo proposed on purpose at a tick (a
/// scene's, at its launch) arrives, at the host time that tick is heard.
#[test]
fn only_an_explicit_tempo_is_proposed() {
    use cantus_core::tempo::TempoPoint;
    use cantus_engine::transport::sync::Proposal;
    let mut r = rig(120.0);
    let tl = Timeline {
        kind: SourceKind::Link,
        tempo: 120.0,
        host: 0,
        position: 0.0,
        rate: 1.0,
        playing: true,
        quantum: 4.0,
        start_stop: true,
        epoch: 1,
    };
    r.h.shared.timeline.publish(&tl);
    for _ in 0..(4.0 * SR / BLOCK as f64) as usize {
        r.block();
    }
    assert!(r.p.transport().following());
    let ahead = (r.p.transport().position_f64() / PPQ as f64).ceil() as i64 + 4;
    let projection = |rev: u64| {
        let mut p = Project::new("scene");
        p.tempo_bpm = 120.0;
        p.tempo_points = vec![
            TempoPoint {
                tick: Ticks::ZERO,
                bpm: 120.0,
                ramp: false,
            },
            TempoPoint {
                tick: Ticks::beats(ahead),
                bpm: 140.0,
                ramp: false,
            },
        ];
        Arc::new(Snapshot::compile(&p, rev))
    };
    let _ = r.tx.push(EngineMessage::SetSnapshot(projection(2)));
    for _ in 0..(3.0 * SR / BLOCK as f64) as usize {
        r.block();
    }
    // Past the point: the same projection again, then the song's map back.
    let _ = r.tx.push(EngineMessage::SetSnapshot(projection(3)));
    for _ in 0..20 {
        r.block();
    }
    let mut song = Project::new("follow");
    song.tempo_bpm = 120.0;
    let _ =
        r.tx.push(EngineMessage::SetSnapshot(Arc::new(Snapshot::compile(
            &song, 4,
        ))));
    for _ in 0..20 {
        r.block();
    }
    let mut proposed = Vec::new();
    while let Ok(p) = r.h.proposals.pop() {
        proposed.push(p);
    }
    assert!(
        !proposed.iter().any(|p| matches!(p, Proposal::Tempo { .. })),
        "a map's install proposed a tempo: {proposed:?}"
    );
    // On purpose, at a tick four beats on.
    let tick = r.p.transport().position_f64() + 4.0 * PPQ as f64;
    let heard_now = r.true_time(r.sample());
    r.p.propose_tempo(133.0, tick);
    match r.h.proposals.pop() {
        Ok(Proposal::Tempo { bpm, at }) => {
            assert_eq!(bpm, 133.0);
            // Four beats at the followed 120 BPM: two seconds after this block is heard.
            let want = heard_now + 2e6;
            assert!((at as f64 - want).abs() < 5_000.0, "at {at}, want {want}");
        }
        other => panic!("no tempo proposed: {other:?}"),
    }
}

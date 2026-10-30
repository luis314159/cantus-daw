//! The sync seam (round 33, ADR 0078–0079), through a processor driven block by block as
//! the audio callback drives it: the null tests first (a seam with nothing followed and
//! nothing sent touches no sample), then MIDI clock and time code out — every message on
//! its sample, at every block size — then following Link, MIDI clock and time code on a
//! simulated host clock that drifts and a callback that wakes late.

mod support;
use cantus_core::*;
use cantus_engine::transport::sync::{self, clock, mtc, OutKind, Stamped, SyncHandle};
use cantus_engine::*;
use std::cell::Cell;
use std::sync::atomic::Ordering;
use std::sync::Arc;

const SR: f64 = 48_000.0;

thread_local! {
    /// The simulated host clock (µs) the processor under test reads.
    static NOW: Cell<i64> = const { Cell::new(0) };
}

fn now() -> i64 {
    NOW.with(|c| c.get())
}

fn set_now(t: i64) {
    NOW.with(|c| c.set(t));
}

/// A processor of its own, outside any handle, with or without a seam.
struct Rig {
    tx: rtrb::Producer<EngineMessage>,
    rx: rtrb::Consumer<EngineEvent>,
    p: Processor,
    sync: Option<SyncHandle>,
}

fn rig(project: &Project, seam: bool) -> Rig {
    let (mut tx, rxe) = rtrb::RingBuffer::new(1024);
    let (txe, rx) = rtrb::RingBuffer::new(8192);
    let mut p = Processor::without_midi(SR, rxe, txe);
    let sync = seam.then(|| {
        let (port, handle) = sync::pair();
        p.attach_sync_clocked(port, None, now);
        handle
    });
    let _ = tx.push(EngineMessage::SetSnapshot(Arc::new(Snapshot::compile(
        project, 1,
    ))));
    p.settle();
    Rig { tx, rx, p, sync }
}

impl Rig {
    fn send(&mut self, m: EngineMessage) {
        let _ = self.tx.push(m);
    }
    /// One block of `n` frames; the events drained.
    fn block(&mut self, n: usize, out: &mut Vec<f32>) {
        let mut buf = vec![0f32; n * 2];
        self.p.process(&mut buf, 2);
        out.extend_from_slice(&buf);
        while self.rx.pop().is_ok() {}
    }
    fn shared(&self) -> &sync::SyncShared {
        &self.sync.as_ref().unwrap().shared
    }
    /// Everything stamped so far.
    fn stamped(&mut self) -> Vec<Stamped> {
        let ring = self.sync.as_mut().unwrap().out.as_mut().unwrap();
        let mut v = Vec::new();
        while let Ok(m) = ring.pop() {
            v.push(m);
        }
        v
    }
}

/// Play `project` from the start for `frames` at `block`, the clock advancing in step.
fn play(r: &mut Rig, frames: usize, block: usize) -> Vec<f32> {
    let mut out = Vec::with_capacity(frames * 2);
    r.send(EngineMessage::Play);
    let mut done = 0;
    while done < frames {
        let n = block.min(frames - done);
        set_now(1_000_000 + (done as f64 / SR * 1e6) as i64);
        r.block(n, &mut out);
        done += n;
    }
    out
}

/// Null: a seam attached, nothing followed and nothing sent, renders every sample as a
/// processor without one, to the bit, at several block sizes.
#[test]
fn a_seam_with_nothing_on_touches_no_sample() {
    let p = support::demo_project();
    for block in [64, 137, 512] {
        let mut plain = rig(&p, false);
        let mut seamed = rig(&p, true);
        let a = play(&mut plain, 200_000, block);
        let b = play(&mut seamed, 200_000, block);
        assert!(a.iter().any(|x| *x != 0.0), "the project sounds");
        assert!(
            a.iter().zip(&b).all(|(x, y)| x.to_bits() == y.to_bits()),
            "block {block}: the seam moved a sample"
        );
        assert!(seamed.stamped().is_empty());
    }
}

/// Review r33 #7: a seam with nothing followed and nothing sent reads no clock and fits no
/// line — the host line is never published (it was, every block, on every live engine).
#[test]
fn an_idle_seam_does_no_work() {
    let p = support::demo_project();
    let mut r = rig(&p, true);
    play(&mut r, 48_000, 256);
    assert_eq!(r.shared().host_line.read(), Some(sync::HostLine::default()));
    // Turned on, it works.
    r.shared().out.clock.store(true, Ordering::Relaxed);
    play(&mut r, 4_800, 256);
    assert_ne!(r.shared().host_line.read(), Some(sync::HostLine::default()));
}

/// Null: clock and time code out on change no sample either (they are messages only).
#[test]
fn clock_out_touches_no_sample() {
    let p = support::demo_project();
    let mut plain = rig(&p, false);
    let mut seamed = rig(&p, true);
    seamed.shared().out.clock.store(true, Ordering::Relaxed);
    seamed.shared().out.mtc.store(true, Ordering::Relaxed);
    let a = play(&mut plain, 150_000, 256);
    let b = play(&mut seamed, 150_000, 256);
    assert!(a.iter().zip(&b).all(|(x, y)| x.to_bits() == y.to_bits()));
    assert!(!seamed.stamped().is_empty());
}

fn clock_project(bpm: f64) -> Project {
    let mut p = Project::new("clock");
    p.tempo_bpm = bpm;
    p
}

/// The clocks a run stamps, at `block`.
fn clocks_at(p: &Project, block: usize, frames: usize) -> Vec<Stamped> {
    let mut r = rig(p, true);
    r.shared().out.clock.store(true, Ordering::Relaxed);
    play(&mut r, frames, block);
    r.stamped()
}

/// MIDI clock out at 120 BPM: Start on the first sample, then a clock every 1 000 samples
/// exactly (a 24th of a 24 000-sample beat) — the same stamps at every block size, so the
/// schedule's jitter is zero samples.
#[test]
fn clock_out_is_on_the_sample_at_every_block_size() {
    let p = clock_project(120.0);
    let want = clocks_at(&p, 64, 96_000);
    assert_eq!(want[0].msg(), &[clock::START]);
    assert_eq!(want[0].at, 0);
    let ticks: Vec<u64> = want
        .iter()
        .filter(|m| m.msg() == [clock::CLOCK])
        .map(|m| m.at)
        .collect();
    assert_eq!(ticks.len(), 96);
    let mut worst = 0.0f64;
    for (k, at) in ticks.iter().enumerate() {
        worst = worst.max((*at as f64 - k as f64 * 1_000.0).abs());
    }
    println!(
        "clock out at 120 BPM: worst deviation {worst} samples ({:.4} ms)",
        worst / SR * 1e3
    );
    assert!(worst / SR * 1e3 <= 0.1);
    for block in [1, 137, 512, 1024] {
        assert_eq!(clocks_at(&p, block, 96_000), want, "block {block}");
    }
}

/// Through a tempo ramp each clock is the sample the transport crosses its 24th of a beat
/// on — the arithmetic notes are due by — within a sample of the map's own seconds.
#[test]
fn clock_out_follows_the_tempo_map() {
    let mut p = clock_project(90.0);
    p.tempo_points = vec![
        TempoPoint {
            tick: Ticks::ZERO,
            bpm: 90.0,
            ramp: true,
        },
        TempoPoint {
            tick: Ticks::beats(8),
            bpm: 150.0,
            ramp: false,
        },
    ];
    let map = p.tempo_map();
    let got: Vec<u64> = clocks_at(&p, 333, 300_000)
        .iter()
        .filter(|m| m.msg() == [clock::CLOCK])
        .map(|m| m.at)
        .collect();
    let mut worst = 0.0f64;
    for (k, at) in got.iter().enumerate() {
        let tick = k as f64 * PPQ as f64 / 24.0;
        let want = map.sample_at(tick, SR);
        worst = worst.max((*at as f64 - want).abs());
    }
    println!("clock out through a ramp: worst {worst:.3} samples");
    assert!(worst <= 1.0, "{worst}");
}

/// A loop's wrap while running: Stop, the pointer to the loop's start, Continue, then the
/// clocks from there — on the wrap's sample.
#[test]
fn a_loop_wrap_sends_stop_pointer_continue() {
    let mut p = clock_project(120.0);
    p.loop_region = Some(LoopRegion {
        start: Ticks::beats(1),
        end: Ticks::beats(3),
    });
    p.loop_enabled = true;
    let mut r = rig(&p, true);
    r.shared().out.clock.store(true, Ordering::Relaxed);
    r.send(EngineMessage::Seek(Ticks::beats(1)));
    play(&mut r, 24_000 * 3, 256);
    let m = r.stamped();
    // Started at beat 1: the pointer (4 sixteenths) and Continue, not Start.
    assert_eq!(m[0].msg(), &clock::spp(1.0));
    assert_eq!(m[1].msg(), &[clock::CONTINUE]);
    let stop = m
        .iter()
        .position(|x| x.msg() == [clock::STOP])
        .expect("a stop");
    assert_eq!(m[stop].at, 48_000, "the wrap two beats in");
    assert_eq!(m[stop + 1].msg(), &clock::spp(1.0));
    assert_eq!(m[stop + 2].msg(), &[clock::CONTINUE]);
    assert_eq!(m[stop + 3].msg(), &[clock::CLOCK]);
    assert_eq!(m[stop + 3].at, 48_000);
}

/// Review r33 #4: started off the grid (tick 1 000, 1/24 beat past beat 1), Cantus sends the
/// pointer of the next sixteenth and Continue, and its first clock is that sixteenth — as
/// receivers read it (our own follower too): a slave's beat is the transport's at every
/// clock. It sent the sixteenth before and clocked from the next 24th, 1/12 beat out.
#[test]
fn a_continue_off_the_grid_puts_the_slave_on_the_transports_beat() {
    let p = clock_project(120.0);
    for tick in [1_000i64, 3_000, 239, 241] {
        let mut r = rig(&p, true);
        r.shared().out.clock.store(true, Ordering::Relaxed);
        r.send(EngineMessage::Seek(Ticks(tick)));
        play(&mut r, 48_000, 128);
        let m = r.stamped();
        assert_eq!(m[1].msg(), &[clock::CONTINUE], "{tick}");
        let mut f = clock::ClockFollower::new();
        let mut last = 0;
        for s in &m {
            let host = (s.at as f64 * 1e6 / SR) as i64;
            f.feed(host, s.msg());
            if s.msg() == [clock::CLOCK] {
                last = s.at;
            }
        }
        // The transport's beat at the last clock's sample: 120 BPM from `tick`.
        let ours = tick as f64 / PPQ as f64 + last as f64 / 24_000.0;
        let theirs = f.timeline((last as f64 * 1e6 / SR) as i64).position;
        assert!(
            (ours - theirs).abs() < 1e-6,
            "from tick {tick}: transport {ours}, slave {theirs}"
        );
    }
}

/// Time code out at each rate: a full frame at the start, then quarter frames that read
/// back as the song's time plus the offset, each on its quarter of a frame.
#[test]
fn time_code_out_reads_back_at_every_rate() {
    for rate in mtc::Rate::ALL {
        let p = clock_project(120.0);
        let mut r = rig(&p, true);
        let out = &r.shared().out;
        out.mtc.store(true, Ordering::Relaxed);
        out.mtc_rate
            .store(u32::from(rate.code()), Ordering::Relaxed);
        // An hour's offset, as the user types it: 01:00:00:00 at the rate (in drop-frame a
        // hair under 3 600 s of real time).
        let hour = mtc::Timecode {
            h: 1,
            m: 0,
            s: 0,
            f: 0,
            rate,
        };
        out.set_offset_s(hour.seconds());
        play(&mut r, 96_000, 256);
        let m = r.stamped();
        let first = mtc::Timecode::from_full_frame(m[0].msg()).expect("a full frame");
        assert_eq!(
            (first.h, first.m, first.s, first.f),
            (1, 0, 0, 0),
            "{rate:?}"
        );
        let mut reader = mtc::QfReader::default();
        let mut codes = 0;
        for (j, q) in m.iter().skip(1).enumerate() {
            assert_eq!(q.kind, OutKind::Mtc);
            let due = j as f64 / (4.0 * rate.fps()) * SR;
            assert!(
                (q.at as f64 - due).abs() <= 1.0,
                "{rate:?} piece {j}: {} vs {due}",
                q.at
            );
            if let Some(tc) = reader.feed(q.msg()[1]) {
                // The run's frame: the frame its piece 0 was sent at.
                let run = (j / 8) as i64 * 2;
                let want = mtc::Timecode::from_frames(hour.frames() + run, rate);
                assert_eq!(tc, want, "{rate:?}");
                codes += 1;
            }
        }
        assert!(codes >= 10, "{rate:?}: {codes} whole codes");
    }
}

/// Round 2, D4: sync turned on again after an hour off fits its clock afresh — the first
/// block's line is within 100 µs of the host clock (the hours-old fit, extrapolated across
/// a 20 ppm skew, was 72 ms off for up to 31 blocks).
#[test]
fn turned_on_again_the_clock_is_fitted_afresh() {
    let p = clock_project(120.0);
    let mut r = rig(&p, true);
    let mut out = Vec::new();
    let block = 4096usize;
    let mut s = 0u64;
    // On for 300 blocks of 512 (a full enough window that the line is refitted only every
    // 32 points), the host clock on the nominal rate.
    r.shared().out.clock.store(true, Ordering::Relaxed);
    for _ in 0..300 {
        set_now(1_000_000 + (s as f64 / SR * 1e6) as i64);
        r.block(512, &mut out);
        out.clear();
        s += 512;
    }
    // Off for an hour, the host clock now 20 ppm off the samples.
    r.shared().out.clock.store(false, Ordering::Relaxed);
    let skewed = |s: u64| 1_000_000 + (s as f64 / SR * 1e6 * (1.0 + 20e-6)) as i64;
    let end = s + 3600 * 48_000;
    while s < end {
        set_now(skewed(s));
        r.block(block, &mut out);
        out.clear();
        s += block as u64;
    }
    // On again: the first block's line.
    r.shared().out.clock.store(true, Ordering::Relaxed);
    set_now(skewed(s));
    r.block(block, &mut out);
    let line = r.shared().host_line.read().unwrap();
    let err = line.host_of(s) - skewed(s) as f64;
    println!("first block after an hour off: the line {err:.1} µs off");
    assert!(err.abs() < 100.0, "{err} µs");
}

//! MIDI clock and time code as data (ADR 0079): the time code's four rates and drop-frame
//! rules, the quarter frames read back, a chaser freewheeling through a dropout and
//! stopping when the code stops, and the MIDI-out thread's timing measured at a port.

use cantus_engine::midi::OutPort;
use cantus_engine::transport::sync::{self, mtc, out, HostLine, OutKind, Stamped};
use mtc::{Rate, Timecode};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::Duration;

fn tc(h: u8, m: u8, s: u8, f: u8, rate: Rate) -> Timecode {
    Timecode { h, m, s, f, rate }
}

/// Every frame of two hours comes back as itself, at every rate; drop-frame's labels never
/// show frames 0 and 1 at a minute's start except each tenth minute.
#[test]
fn every_frame_round_trips_and_drop_frame_drops_the_right_labels() {
    for rate in Rate::ALL {
        let frames = (2.0 * 3600.0 * rate.fps()) as i64;
        for n in (0..frames).step_by(7).chain(frames - 100..frames) {
            let t = Timecode::from_frames(n, rate);
            assert_eq!(t.frames(), n, "{rate:?} {}", t.label());
            if rate == Rate::Df2997 && t.s == 0 && !t.m.is_multiple_of(10) {
                assert!(t.f >= 2, "{} is a dropped label", t.label());
            }
        }
    }
    let df = Rate::Df2997;
    // 00:00:59;29 → 00:01:00;02; 00:09:59;29 → 00:10:00;00.
    let n = tc(0, 0, 59, 29, df).frames();
    assert_eq!(Timecode::from_frames(n + 1, df), tc(0, 1, 0, 2, df));
    let n = tc(0, 9, 59, 29, df).frames();
    assert_eq!(Timecode::from_frames(n + 1, df), tc(0, 10, 0, 0, df));
    // Ten minutes are 17 982 frames, an hour 107 892: drop-frame keeps to the clock.
    assert_eq!(tc(0, 10, 0, 0, df).frames(), 17_982);
    assert_eq!(tc(1, 0, 0, 0, df).frames(), 107_892);
    assert!((tc(1, 0, 0, 0, df).seconds() - 3599.9964).abs() < 1e-3);
    assert_eq!(tc(1, 0, 0, 0, df).label(), "01:00:00;00");
}

#[test]
fn the_full_frame_and_quarter_frames_carry_the_rate() {
    for rate in Rate::ALL {
        let t = tc(13, 37, 42, 21, rate);
        let ff = t.full_frame();
        assert_eq!(ff[5] >> 5, rate.code());
        assert_eq!(Timecode::from_full_frame(&ff), Some(t));
        let mut r = mtc::QfReader::default();
        let mut got = None;
        for p in 0..8 {
            let [st, d] = t.quarter_frame(p);
            assert_eq!(st, 0xF1);
            got = r.feed(d);
        }
        assert_eq!(got, Some(t));
    }
    // A run broken in the middle reads nothing until the next piece 0.
    let t = tc(1, 2, 3, 4, Rate::Fps25);
    let mut r = mtc::QfReader::default();
    for p in [0, 1, 2, 4, 5, 6, 7] {
        assert_eq!(r.feed(t.quarter_frame(p)[1]), None);
    }
}

/// The chaser: a full frame locates; the quarter frames run; a 150 ms dropout freewheels
/// through, the place kept; a silence past a quarter second stops it.
#[test]
fn the_chaser_freewheels_and_stops() {
    let rate = Rate::Fps30;
    let q = 1e6 / (4.0 * rate.fps());
    let mut c = mtc::MtcChaser::new();
    c.feed(0, &tc(1, 0, 0, 0, rate).full_frame());
    assert!(!c.running(0));
    assert!((c.timeline(0).position - 3600.0).abs() < 1e-9);
    let mut w = mtc::QfWriter::at(3600.0, rate);
    let mut t = 10_000.0;
    for _ in 0..240 {
        c.feed(t as i64, &w.take(rate));
        t += q;
    }
    let at = t as i64;
    let tl = c.timeline(at);
    assert!(tl.playing);
    let want = 3600.0 + 240.0 / (4.0 * rate.fps());
    assert!(
        (tl.seconds_at(at) - want).abs() < 2e-3,
        "{} vs {want}",
        tl.seconds_at(at)
    );
    // A dropout of 150 ms: still running, extrapolated.
    let gap = at + 150_000;
    assert!(c.running(gap));
    let ahead = c.timeline(gap).seconds_at(gap);
    assert!((ahead - (want + 0.15)).abs() < 3e-3, "{ahead}");
    // A silence of 300 ms: stopped.
    assert!(!c.running(at + 300_000));
}

/// Review r33 #3: the chaser counts quarters elapsed, not messages heard. One quarter
/// frame lost: two seconds on, within a millisecond (it stayed a quarter, 10 ms at 25 fps,
/// late for good). A 150 ms dropout freewheeled: on resuming, the same epoch (no jump, no
/// notes cut) and within 2 ms (it ended in a relocate).
#[test]
fn the_chaser_counts_quarters_not_messages() {
    let rate = Rate::Fps25;
    let q = 1e6 / (4.0 * rate.fps());
    let mut c = mtc::MtcChaser::new();
    let start = 50_000.0;
    let truth = |t: f64| 3600.0 + (t - start) / 1e6;
    let mut w = mtc::QfWriter::at(3600.0, rate);
    let mut j = 0u64;
    let mut send = |c: &mut mtc::MtcChaser, until: f64, skip: &dyn Fn(u64, f64) -> bool| loop {
        let at = start + j as f64 * q;
        if at > until {
            break;
        }
        let msg = w.take(rate);
        if !skip(j, at) {
            c.feed(at as i64, &msg);
        }
        j += 1;
    };
    // A second to lock, then one quarter frame lost, then two seconds.
    send(&mut c, start + 1e6, &|_, _| false);
    let lost = (1e6 / q) as u64 + 4;
    send(&mut c, start + 3e6, &|k, _| k == lost);
    let t = start + 3e6 + 1_000.0;
    let err = (c.timeline(t as i64).seconds_at(t as i64) - truth(t)) * 1e3;
    println!("time code chase, one quarter frame lost: {err:.3} ms two seconds on");
    assert!(err.abs() < 1.0, "{err} ms");
    let epoch = c.timeline(t as i64).epoch;
    // A 150 ms dropout, then the code again.
    let (from, to) = (start + 3.2e6, start + 3.35e6);
    send(&mut c, start + 4e6, &|_, at| at >= from && at < to);
    let t = start + 4e6 + 1_000.0;
    let tl = c.timeline(t as i64);
    let err = (tl.seconds_at(t as i64) - truth(t)) * 1e3;
    println!("time code chase, after a 150 ms dropout: {err:.3} ms");
    assert_eq!(tl.epoch, epoch, "a dropout freewheeled is no relocate");
    assert!(err.abs() < 2.0, "{err} ms");
}

/// A port that keeps when each message reached it.
#[derive(Clone, Default)]
struct Probe(Arc<Mutex<Arrivals>>);

/// What reached the port: when (host µs) and the bytes.
type Arrivals = Vec<(i64, Vec<u8>)>;

static PROBE: Mutex<Option<Probe>> = Mutex::new(None);

impl OutPort for Probe {
    fn write(&mut self, bytes: &[u8]) -> bool {
        self.0
            .lock()
            .unwrap()
            .push((sync::host_micros(), bytes.to_vec()));
        true
    }
}

fn probe_connect(name: &str) -> Option<Box<dyn OutPort>> {
    (name == "probe").then(|| Box::new(PROBE.lock().unwrap().clone().unwrap()) as Box<dyn OutPort>)
}

/// The MIDI-out thread sends each message when its sample is heard — the host line, the
/// device's latency and the port's offset — measured at the port: the worst lateness on
/// this machine is printed (the out thread sleeps on the high-resolution timer, then
/// yields to the moment).
#[test]
fn the_out_thread_sends_on_time() {
    let probe = Probe::default();
    *PROBE.lock().unwrap() = Some(probe.clone());
    let (mut port, mut handle) = sync::pair();
    let t0 = sync::host_micros() + 50_000;
    // 48 kHz exactly, sample 0 at t0, 5 ms of device latency.
    handle.shared.host_line.publish(&HostLine {
        s0: 0,
        a: t0 as f64,
        b: 1e6 / 48_000.0,
        latency_us: 5_000.0,
    });
    let thread = out::MidiOut::start(
        handle.out.take().unwrap(),
        handle.shared.clone(),
        cantus_engine::midi::pool::OutPool::new(probe_connect),
    );
    thread.set_ports(vec![out::PortChoice {
        name: "probe".into(),
        clock: true,
        mtc: false,
        offset_ms: 2.0,
    }]);
    std::thread::sleep(Duration::from_millis(20));
    // 200 clocks at 120 BPM (one per 1 000 samples), and a time code nobody takes.
    for k in 0..200u64 {
        port.out
            .push(Stamped::new(k * 1_000, OutKind::Clock, &[0xF8]))
            .unwrap();
        port.out
            .push(Stamped::new(k * 1_000, OutKind::Mtc, &[0xF1, 0]))
            .unwrap();
        if k % 50 == 49 {
            std::thread::sleep(Duration::from_millis(50));
        }
    }
    std::thread::sleep(Duration::from_millis(4_400));
    let got = probe.0.lock().unwrap().clone();
    drop(thread);
    assert_eq!(got.len(), 200, "the clocks only");
    let mut lates = Vec::new();
    for (k, (at, msg)) in got.iter().enumerate() {
        assert_eq!(msg, &vec![0xF8]);
        let due = t0 + (k as f64 * 1_000.0 * 1e6 / 48_000.0) as i64 + 5_000 + 2_000;
        let late = at - due;
        assert!(late >= -1, "clock {k} early by {} µs", -late);
        lates.push(late);
    }
    lates.sort_unstable();
    let (median, p90, worst) = (lates[100], lates[180], lates[199]);
    println!("MIDI out at the port: median lateness {median} µs, 90th {p90} µs, worst {worst} µs");
    // A busy machine (three builds at once) holds the thread off now and then: 3 ms at the
    // 90th percentile was seen while other builders compiled; idle it is ~20 µs. The
    // median is the schedule's own, held tight.
    assert!(
        median < 500 && p90 < 5_000,
        "median {median} µs, 90th {p90} µs"
    );
    assert_eq!(handle.shared.dropped.load(Ordering::Relaxed), 0);
}

/// Two ports' probes, by name.
static PORTS: Mutex<Vec<(String, Probe)>> = Mutex::new(Vec::new());

fn named_connect(name: &str) -> Option<Box<dyn OutPort>> {
    let ports = PORTS.lock().unwrap();
    let p = ports.iter().find(|(n, _)| n == name)?.1.clone();
    Some(Box::new(p))
}

/// Review r33 #9: messages waiting for their moment keep their port when the ports are
/// chosen again in another order (they were sent to whichever port took the old index).
#[test]
fn a_waiting_message_keeps_its_port_when_the_ports_change() {
    let (a, b) = (Probe::default(), Probe::default());
    *PORTS.lock().unwrap() = vec![("A".into(), a.clone()), ("B".into(), b.clone())];
    let (mut port, mut handle) = sync::pair();
    let t0 = sync::host_micros() + 300_000;
    handle.shared.host_line.publish(&HostLine {
        s0: 0,
        a: t0 as f64,
        b: 1e6 / 48_000.0,
        latency_us: 0.0,
    });
    let thread = out::MidiOut::start(
        handle.out.take().unwrap(),
        handle.shared.clone(),
        cantus_engine::midi::pool::OutPool::new(named_connect),
    );
    let choice = |name: &str, clock: bool, mtc: bool| out::PortChoice {
        name: name.into(),
        clock,
        mtc,
        offset_ms: 0.0,
    };
    thread.set_ports(vec![choice("A", true, false), choice("B", false, true)]);
    std::thread::sleep(Duration::from_millis(30));
    // A clock for A only, due in about 300 ms.
    port.out
        .push(Stamped::new(0, OutKind::Clock, &[0xF8]))
        .unwrap();
    std::thread::sleep(Duration::from_millis(60));
    // The same ports chosen in the other order before it is due.
    thread.set_ports(vec![choice("B", false, true), choice("A", true, false)]);
    std::thread::sleep(Duration::from_millis(500));
    drop(thread);
    assert_eq!(a.0.lock().unwrap().len(), 1, "A's clock reached A");
    assert!(b.0.lock().unwrap().is_empty(), "and not B");
}

/// Review r33 round 2, D1: the chaser counts by piece number, so a quarter frame delayed by
/// 0.6 of a quarter, or two delivered at the same moment, move the count by one each —
/// within a millisecond of the code at every quarter frame (counting by elapsed time put the
/// late one a quarter ahead, 10 ms at 25 fps, then stepped back).
#[test]
fn late_or_bunched_quarter_frames_keep_the_count() {
    let rate = Rate::Fps25;
    let q = 1e6 / (4.0 * rate.fps());
    let mut c = mtc::MtcChaser::new();
    let start = 50_000.0;
    let truth = |t: f64| 3600.0 + (t - start) / 1e6;
    let mut w = mtc::QfWriter::at(3600.0, rate);
    let mut worst = 0.0f64;
    for j in 0..400u64 {
        let due = start + j as f64 * q;
        let at = match j {
            200 => due + 0.6 * q, // late
            300 => due + q,       // held back: arrives with the next
            _ => due,
        };
        let msg = w.take(rate);
        c.feed(at as i64, &msg);
        if j > 100 {
            let t = c.timeline(at as i64);
            let err = (t.seconds_at(at as i64) - truth(at)) * 1e3;
            worst = worst.max(err.abs());
        }
    }
    println!("time code chase, one late and two bunched quarter frames: worst {worst:.3} ms");
    assert!(worst < 1.0, "{worst} ms");
}

/// Opens counted: one connection per port is what the class driver allows.
static OPENS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// This test's own port (the other tests' statics are theirs: they run at once).
static PADS: Mutex<Option<Probe>> = Mutex::new(None);

fn counted_connect(name: &str) -> Option<Box<dyn OutPort>> {
    OPENS.fetch_add(1, Ordering::Relaxed);
    let p = PADS.lock().unwrap().clone()?;
    (name == "Pads").then(|| Box::new(p) as Box<dyn OutPort>)
}

/// Integrated review #9: an output with Clock and Feedback on is opened once — clock out
/// and controller feedback write through the one connection — and both reach it. (Before,
/// each opened its own: on Windows' class driver the second open fails and one stream is
/// lost. The shared pool is new, so this cannot be run against the old code; it opened
/// twice by construction.)
#[test]
fn clock_and_feedback_share_one_connection() {
    let probe = Probe::default();
    *PADS.lock().unwrap() = Some(probe.clone());
    OPENS.store(0, Ordering::Relaxed);
    let pool = cantus_engine::midi::pool::OutPool::new(counted_connect);
    let (mut port, mut handle) = sync::pair();
    let t0 = sync::host_micros() + 50_000;
    handle.shared.host_line.publish(&HostLine {
        s0: 0,
        a: t0 as f64,
        b: 1e6 / 48_000.0,
        latency_us: 0.0,
    });
    let thread = out::MidiOut::start(
        handle.out.take().unwrap(),
        handle.shared.clone(),
        pool.clone(),
    );
    thread.set_ports(vec![out::PortChoice {
        name: "Pads".into(),
        clock: true,
        mtc: false,
        offset_ms: 0.0,
    }]);
    std::thread::sleep(Duration::from_millis(20));
    let mut feedback = cantus_engine::MidiOutputs::default();
    feedback.set_pool(pool.clone(), |input| Some(input.to_string()));
    port.out
        .push(Stamped::new(0, OutKind::Clock, &[0xF8]))
        .unwrap();
    assert!(feedback.send("Pads", &[0xB0, 7, 100]), "feedback went out");
    std::thread::sleep(Duration::from_millis(200));
    drop(thread);
    let got: Vec<Vec<u8>> = probe
        .0
        .lock()
        .unwrap()
        .iter()
        .map(|(_, b)| b.clone())
        .collect();
    assert!(got.contains(&vec![0xF8]), "the clock arrived: {got:?}");
    assert!(
        got.contains(&vec![0xB0, 7, 100]),
        "the feedback arrived: {got:?}"
    );
    assert_eq!(OPENS.load(Ordering::Relaxed), 1, "one connection");
}

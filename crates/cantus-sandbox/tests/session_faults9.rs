//! Round 39 (fixes; BACKLOG §8, "a shared helper's crash blamed on no one"): two plug-ins
//! in one helper, the gain aborts in its `process` a hundred times — a Windows fail-fast,
//! which runs no crash handler. Each time the crash is the gain's alone: counted once, for
//! it, and its neighbour never blamed.
//!
//! The race (beta.12's release run): on a loaded machine the aborted helper takes seconds
//! to end (the system's error reporting runs first), and meanwhile the neighbour's next job
//! was sent and never taken; Cantus's hang judgement laid the end on whichever slot it
//! looked at first — the neighbour, for a job no worker ever took. With
//! `CANTUS_ABORT_LOAD=24` (threads spinning beside the test) it showed within a few rounds;
//! a job is now laid on its plug-in only if a worker took it.

mod support;

use cantus_core::*;
use cantus_host::{Format, HostNotice};
use std::time::{Duration, Instant};
use support::*;

fn patient_host() -> cantus_host::PluginHost {
    let host = host();
    if let Some(p) = host.sandboxes() {
        p.set_deadline_floor(Duration::from_secs(5));
    }
    host
}

fn crash_notices(rig: &Rig) -> usize {
    rig.notices
        .iter()
        .filter(|n| matches!(n, HostNotice::PluginCrashed { .. }))
        .count()
}

fn frames_until(rig: &mut Rig, limit: Duration, done: impl Fn(&Rig) -> bool) {
    let start = Instant::now();
    while !done(rig) && start.elapsed() < limit {
        rig.frame();
        std::thread::sleep(Duration::from_millis(1));
    }
}

/// How many times (`CANTUS_ABORTS` for another count).
fn times() -> usize {
    std::env::var("CANTUS_ABORTS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(100)
}

/// `CANTUS_ABORT_LOAD` threads spinning beside the test, as a loaded runner's are (none
/// by default): what makes the race show.
fn load() -> (
    std::sync::Arc<std::sync::atomic::AtomicBool>,
    Vec<std::thread::JoinHandle<()>>,
) {
    let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let n: usize = std::env::var("CANTUS_ABORT_LOAD")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let spinners = (0..n)
        .map(|_| {
            let stop = stop.clone();
            std::thread::spawn(move || {
                let mut x = 0u64;
                while !stop.load(std::sync::atomic::Ordering::Relaxed) {
                    x = std::hint::black_box(x.wrapping_mul(31).wrapping_add(7));
                }
            })
        })
        .collect();
    (stop, spinners)
}

#[test]
fn a_hundred_aborts_in_a_shared_helper_are_each_the_aborting_plugins_alone() {
    let _g = serial();
    let (stop, spinners) = load();
    let mut nobody = 0;
    let mut neighbour = 0;
    let n = times();
    for round in 0..n {
        let project = demo_project();
        let strip = strip_of(&project, 0);
        let mut rig = Rig::new(project, patient_host());
        let a = rig.insert(strip, GAIN_ID, PluginIsolation::Together);
        let b = rig.insert(strip, VST3_GAIN_ID, PluginIsolation::Together);
        rig.set_param(strip, a, P_GAIN, -6.0);
        rig.set_param(strip, b, V_GAIN, -3.0);
        rig.settle();
        rig.engine.seek(Ticks::ZERO);
        rig.engine.play();
        // A few blocks and frames first, at a different point of the helper's tick each
        // round.
        for _ in 0..(round % 7) {
            rig.block(256);
            std::thread::sleep(Duration::from_millis(2));
        }
        rig.set_param(strip, a, P_FAULT, ABORT_PROCESS);
        for _ in 0..20 {
            rig.block(256);
        }
        frames_until(&mut rig, Duration::from_secs(10), |r| crash_notices(r) >= 2);
        rig.engine.stop();
        assert_eq!(crash_notices(&rig), 2, "round {round}: {:?}", rig.notices);
        if !rig.host.crash_culprit(a) {
            nobody += 1;
        }
        if rig.host.crash_culprit(b) {
            neighbour += 1;
        }
        assert_eq!(
            rig.host.crashes_this_session(Format::Vst3, VST3_GAIN_ID),
            0,
            "round {round}: the neighbour never counted"
        );
        assert!(rig
            .host
            .db
            .crash_record(Format::Vst3, VST3_GAIN_ID)
            .is_none());
    }
    stop.store(true, std::sync::atomic::Ordering::Relaxed);
    for s in spinners {
        let _ = s.join();
    }
    assert_eq!(neighbour, 0, "the neighbour blamed in {neighbour} of {n}");
    assert_eq!(nobody, 0, "the gain not blamed in {nobody} of {n}");
}

/// The race held without a loaded machine: a helper with one processing thread, the gain's
/// job taken and hanging on it, the neighbour's next job sent and never taken — both past
/// the hang limit at the same frame. The end is the gain's, whichever slot is judged first
/// (each order of the two, several times).
#[test]
fn a_hang_is_laid_on_the_job_that_runs_never_on_one_waiting_behind_it() {
    let _g = serial();
    for round in 0..6 {
        let project = demo_project();
        let strip = strip_of(&project, 0);
        let host = host();
        if let Some(p) = host.sandboxes() {
            p.set_threads(Some(1));
            p.set_hang_limit(Duration::from_millis(400));
        }
        let mut rig = Rig::new(project, host);
        let (a, b) = if round % 2 == 0 {
            let a = rig.insert(strip, GAIN_ID, PluginIsolation::Together);
            let b = rig.insert(strip, VST3_GAIN_ID, PluginIsolation::Together);
            (a, b)
        } else {
            let b = rig.insert(strip, VST3_GAIN_ID, PluginIsolation::Together);
            let a = rig.insert(strip, GAIN_ID, PluginIsolation::Together);
            (a, b)
        };
        rig.settle();
        rig.engine.seek(Ticks::ZERO);
        rig.engine.play();
        rig.set_param(strip, a, P_FAULT, HANG_PROCESS);
        // Blocks without frames until both jobs are past the limit: the next frame judges
        // the two at once.
        let start = Instant::now();
        while start.elapsed() < Duration::from_millis(900) {
            rig.processor.process(&mut vec![0f32; 512], 2);
        }
        frames_until(&mut rig, Duration::from_secs(10), |r| crash_notices(r) >= 2);
        rig.engine.stop();
        assert_eq!(crash_notices(&rig), 2, "round {round}: {:?}", rig.notices);
        assert!(rig.host.crash_culprit(a), "round {round}: the hanging gain");
        assert!(
            !rig.host.crash_culprit(b),
            "round {round}: not the neighbour"
        );
        assert_eq!(rig.host.crashes_this_session(Format::Clap, GAIN_ID), 1);
        assert_eq!(rig.host.crashes_this_session(Format::Vst3, VST3_GAIN_ID), 0);
    }
}

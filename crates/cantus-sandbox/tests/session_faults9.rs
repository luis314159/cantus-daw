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
//!
//! The race that stayed (beta.14's CI and release runs, "the gain not blamed in 2 of 100",
//! "6 of 100"; round 39, abort-blame): the neighbour's value, set just before the settle,
//! is a request its helper's main thread serves while the gain's job runs, and every tick
//! mirrors the neighbour; a loaded runner's main thread lags there — still answering the
//! neighbour, or put off the processor in its tick, when the gain aborts. The main thread's
//! word named the neighbour for the whole request and the whole tick, its own bookkeeping
//! included, so the end had two candidates — the gain's job and the neighbour — and was
//! laid on nobody. The word now names a plug-in only while the main thread runs its code.
//! `CANTUS_TEST_SANDBOX_LAG` holds that lag on any machine.
//!
//! What stays by design (the next CI run, 1 of 100; ADR 0063 §10): a main thread put off the
//! processor *inside* the neighbour's code when the gain aborts — nothing in the process says
//! which thread called the fail-fast. Both are told they are suspects, laid on neither, and
//! reloaded each runs alone for the session, so the gain's next abort is laid on it
//! (`CANTUS_TEST_SANDBOX_LAG_IN` holds that on any machine, the last test).

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
    let mut suspects = 0;
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
            // The main thread inside the neighbour's code at the abort (ADR 0063 §10):
            // both told they are suspects — the reload settles it (the last test) — never
            // an end laid on none with nobody pointed at.
            if rig.host.crash_suspect(a) && rig.host.crash_suspect(b) {
                suspects += 1;
            } else {
                nobody += 1;
            }
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
    eprintln!("{suspects} of {n} ends had the gain and its neighbour as suspects");
    assert_eq!(neighbour, 0, "the neighbour blamed in {neighbour} of {n}");
    assert_eq!(
        nobody, 0,
        "the gain neither blamed nor a suspect in {nobody} of {n}"
    );
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

/// The race that stayed, held on any machine (round 39, abort-blame): the helper's main
/// thread lags 60 ms over its own work about the neighbour (instance 2) — each answer, each
/// tick's mirroring, after the plug-in's calls, as a loaded runner's main thread is put off
/// the processor there — so the gain aborts while the neighbour's value, set just before the
/// settle, is still being answered. The end is the gain's alone, every round: the helper's
/// own work names no plug-in. (Before the fix, 7 of 7 laid on nobody.)
#[test]
fn an_abort_while_the_neighbours_answer_is_written_is_still_the_gains() {
    let _g = serial();
    // Set for the helpers this test starts, and gone after it whatever happens.
    struct Unset;
    impl Drop for Unset {
        fn drop(&mut self) {
            std::env::remove_var("CANTUS_TEST_SANDBOX_LAG");
        }
    }
    let _unset = Unset;
    std::env::set_var("CANTUS_TEST_SANDBOX_LAG", "2:60");
    let mut nobody = Vec::new();
    for round in 0..7 {
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
        for _ in 0..(round % 7) {
            rig.block(256);
        }
        rig.set_param(strip, a, P_FAULT, ABORT_PROCESS);
        for _ in 0..20 {
            rig.block(256);
        }
        frames_until(&mut rig, Duration::from_secs(10), |r| crash_notices(r) >= 2);
        rig.engine.stop();
        assert_eq!(crash_notices(&rig), 2, "round {round}: {:?}", rig.notices);
        if !rig.host.crash_culprit(a) {
            nobody.push(round);
        }
        assert!(
            !rig.host.crash_culprit(b),
            "round {round}: not the neighbour"
        );
        assert_eq!(rig.host.crashes_this_session(Format::Vst3, VST3_GAIN_ID), 0);
    }
    assert!(
        nobody.is_empty(),
        "the gain not blamed in rounds {nobody:?}"
    );
}

/// The end nothing chooses between, and how it is settled (ADR 0063 §10): the helper's main
/// thread held inside the neighbour's code (`CANTUS_TEST_SANDBOX_LAG_IN`, 30 ms a call — a
/// loaded runner's main thread put off the processor there, the 1 of 100 GitHub's Windows
/// runner still saw after abort-blame), so the gain's fail-fast abort has two candidates.
/// The end is laid on neither and both are told they are suspects; reloaded from the card,
/// each runs alone for the session (still *Together* in the project), so the gain's next
/// abort is laid on it alone — counted once, for it — and the neighbour keeps playing.
///
/// Windows only: there an abort is a fail-fast and no handler runs. On Linux and macOS the
/// helper's signal handler names the aborting thread's plug-in, so no end has two candidates.
#[cfg(windows)]
#[test]
fn an_end_nothing_chooses_between_is_settled_by_the_next() {
    let _g = serial();
    struct Unset;
    impl Drop for Unset {
        fn drop(&mut self) {
            std::env::remove_var("CANTUS_TEST_SANDBOX_LAG_IN");
        }
    }
    let _unset = Unset;
    std::env::set_var("CANTUS_TEST_SANDBOX_LAG_IN", "2:30");
    let mut unsettled = 0;
    let rounds = 5;
    for round in 0..rounds {
        let project = demo_project();
        let strip = strip_of(&project, 0);
        let mut rig = Rig::new(project, patient_host());
        let a = rig.insert(strip, GAIN_ID, PluginIsolation::Together);
        let b = rig.insert(strip, VST3_GAIN_ID, PluginIsolation::Together);
        rig.settle();
        rig.engine.seek(Ticks::ZERO);
        rig.engine.play();
        for _ in 0..round {
            rig.block(256);
        }
        rig.set_param(strip, a, P_FAULT, ABORT_PROCESS);
        for _ in 0..20 {
            rig.block(256);
        }
        frames_until(&mut rig, Duration::from_secs(10), |r| crash_notices(r) >= 2);
        assert_eq!(crash_notices(&rig), 2, "round {round}: {:?}", rig.notices);
        assert!(
            !rig.host.crash_culprit(b),
            "round {round}: not the neighbour"
        );
        assert_eq!(rig.host.crashes_this_session(Format::Vst3, VST3_GAIN_ID), 0);
        if rig.host.crash_culprit(a) {
            // The main thread was between calls at the abort: laid on the gain at once.
            assert!(!rig.host.crash_suspect(a) && !rig.host.crash_suspect(b));
            continue;
        }
        unsettled += 1;
        assert!(
            rig.host.crash_suspect(a) && rig.host.crash_suspect(b),
            "round {round}: both told they are suspects"
        );
        assert_eq!(rig.host.crashes_this_session(Format::Clap, GAIN_ID), 0);
        // Reloaded, as the card's *Reload all* does: each alone now.
        let failed = rig
            .host
            .reload_all_crashed(rig.bus.project(), &mut rig.engine);
        assert!(failed.is_empty(), "round {round}: {failed:?}");
        frames_until(&mut rig, Duration::from_secs(10), |r| {
            r.host
                .hosted(a)
                .is_some_and(|h| h.crashed().is_none() && h.apart())
                && r.host
                    .hosted(b)
                    .is_some_and(|h| h.crashed().is_none() && h.apart())
        });
        // Its load is answered later: settled first, as every insert here is.
        rig.settle();
        // The gain aborts again: alone, its end has one candidate.
        rig.set_param(strip, a, P_FAULT, 0.0);
        for _ in 0..4 {
            rig.block(256);
        }
        rig.set_param(strip, a, P_FAULT, ABORT_PROCESS);
        for _ in 0..20 {
            rig.block(256);
        }
        frames_until(&mut rig, Duration::from_secs(10), |r| {
            r.host.crashes_this_session(Format::Clap, GAIN_ID) == 1
        });
        for _ in 0..40 {
            rig.block(256);
        }
        rig.engine.stop();
        assert!(
            rig.host.crash_culprit(a),
            "round {round}: the gain, alone — crashes {} crashed {:?} suspect {} apart {:?} {:?}",
            rig.host.crashes_this_session(Format::Clap, GAIN_ID),
            rig.host.crashed(a),
            rig.host.crash_suspect(a),
            rig.host.hosted(a).map(|h| (h.apart(), h.isolation())),
            rig.notices
        );
        assert_eq!(rig.host.crashes_this_session(Format::Clap, GAIN_ID), 1);
        assert_eq!(rig.host.crashes_this_session(Format::Vst3, VST3_GAIN_ID), 0);
        assert!(
            rig.host.crashed(b).is_none(),
            "round {round}: the neighbour plays on"
        );
        let nb = rig.host.hosted(b).expect("the neighbour is loaded");
        assert!(nb.apart(), "round {round}: the neighbour runs alone");
        assert_eq!(
            nb.isolation(),
            PluginIsolation::Together,
            "as the project says"
        );
        assert_eq!(
            rig.plugin(b).isolation,
            Some(PluginIsolation::Together),
            "the project unchanged"
        );
    }
    // The knob holds the main thread in the neighbour's code nearly all the time: most
    // first ends are the ones nothing chooses between.
    eprintln!("{unsettled} of {rounds} first ends unsettled");
    assert!(unsettled > 0, "no end was unsettled: the knob did not hold");
}

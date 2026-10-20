//! The third review of the fix round (round 29, isolation-4), through real helpers: a late
//! job's events survive the interface seeing it finish first; a stalled helper costs one
//! deadline, not one per plug-in; calls queued behind a slow one wait from when they start;
//! a plug-in's dialog inside a call never freezes the others; a restart is polled, never
//! waited for again; a crash is laid on the plug-in that did it — in its destructor, in a
//! framework's timer, behind a crash reporter that hands on — and a hang never on a
//! bystander.

mod support;

use cantus_host::bridge::sandbox::Limits;
use cantus_host::bridge::Sandboxes;
use cantus_host::{
    AudioThread, Backend, Blame, Loaded, MainThread, NoteEvent, Notice, ParamChange, ParamEvent,
    ParamOut,
};
use std::sync::Arc;
use std::time::{Duration, Instant};
use support::*;

fn load(pool: &Arc<Sandboxes>, id: &str, group: &str) -> Loaded {
    let info = info(id);
    bridged(pool, info.format, group, false)
        .load(&info, setup())
        .expect("loads in the helper")
}

fn limits(silence: u64, call: u64, state: u64) -> Arc<Sandboxes> {
    let pool = pool();
    pool.set_limits(Limits {
        silence: Duration::from_millis(silence),
        load: Duration::from_secs(10),
        state: Duration::from_millis(state),
        call: Duration::from_millis(call),
        hang: Duration::from_millis(500),
        offline: Duration::from_secs(2),
    });
    pool
}

fn crashed(main: &mut dyn MainThread) -> Option<Blame> {
    main.idle().into_iter().find_map(|n| match n {
        Notice::Crashed { blame, reason } => {
            eprintln!("crashed: {reason} ({blame:?})");
            Some(blame)
        }
        _ => None,
    })
}

fn crashed_within(main: &mut dyn MainThread, limit: Duration) -> Option<Blame> {
    let start = Instant::now();
    while start.elapsed() < limit {
        if let Some(b) = crashed(main) {
            return Some(b);
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    None
}

fn block(audio: &mut dyn AudioThread, p: &[ParamChange], n: &[NoteEvent]) -> ParamOut {
    let (mut l, mut r) = (vec![0.0f32; 128], vec![0.0f32; 128]);
    call(audio, &mut l, &mut r, p, n)
}

/// A block of noise; its output.
fn noisy(audio: &mut dyn AudioThread, p: &[ParamChange]) -> Vec<f32> {
    let mut l = noise(128, 7);
    let mut r = l.clone();
    call(audio, &mut l, &mut r, p, &[]);
    l
}

/// A late job's events reach the host even when the interface's frame sees the job finish
/// before the audio thread's next block does (review isolation-4 #0, #11).
#[test]
fn a_late_jobs_events_survive_the_interface_seeing_it_finish_first() {
    let _g = serial();
    let pool = pool();
    let Loaded {
        main: mut m,
        audio: mut s,
    } = load(&pool, SYNTH_ID, "own-late-events-ui");
    let mut events = Vec::new();
    let mut take = |out: ParamOut| events.extend(out.events().iter().copied());
    take(block(
        s.as_mut(),
        &[ParamChange {
            offset: 0,
            id: 4,
            value: 2.0,
        }],
        &[],
    ));
    for _ in 0..3 {
        take(block(s.as_mut(), &[], &[]));
    }
    take(block(s.as_mut(), &fault(S_FAULT, SLOW_ONCE), &[]));
    std::thread::sleep(Duration::from_millis(100));
    // The interface's frame comes first: it sees the late job finished.
    m.idle();
    for _ in 0..3 {
        take(block(s.as_mut(), &[], &[]));
    }
    let count = |want: fn(&ParamEvent) -> bool| events.iter().filter(|e| want(e)).count();
    assert_eq!(
        (
            count(|e| matches!(e, ParamEvent::Begin(0))),
            count(|e| matches!(e, ParamEvent::End(0)))
        ),
        (1, 1),
        "{events:?}"
    );
}

/// A helper none of whose processing threads took a job by its deadline is stalled: its
/// other plug-ins pass dry at once instead of each waiting out a deadline in the same
/// callback, and play again once a job of it finishes (review isolation-4 #62).
#[test]
fn a_stalled_helper_costs_one_deadline_not_one_per_plugin() {
    let _g = serial();
    let pool = pool();
    pool.set_threads(Some(1));
    pool.set_deadline_floor(Duration::from_millis(100));
    let Loaded {
        main: _xm,
        audio: mut x,
    } = load(&pool, GAIN_ID, "stall");
    let Loaded {
        main: _am,
        audio: mut a,
    } = load(&pool, GAIN_ID, "stall");
    let Loaded {
        main: _bm,
        audio: mut b,
    } = load(&pool, GAIN_ID, "stall");
    let quiet = [ParamChange {
        offset: 0,
        id: P_GAIN,
        value: -60.0,
    }];
    noisy(b.as_mut(), &quiet);
    // X's job holds the helper's one processing thread for a second.
    noisy(x.as_mut(), &fault(P_FAULT, SLOW_SECOND));
    // A's job is not even taken by its deadline.
    noisy(a.as_mut(), &[]);
    let t = Instant::now();
    let dry = noisy(b.as_mut(), &[]);
    let took = t.elapsed();
    assert!(
        took < Duration::from_millis(50),
        "B waited {took:?} for a helper known to be stalled"
    );
    assert_eq!(dry, noise(128, 7), "B passed dry");
    std::thread::sleep(Duration::from_millis(1_200));
    // X's job finished: the helper plays again.
    noisy(a.as_mut(), &[]);
    let played = noisy(b.as_mut(), &[]);
    assert!(
        played.iter().all(|v| v.abs() < 0.01),
        "B processed again (its gain at -60 dB)"
    );
}

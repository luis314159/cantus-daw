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
        heavy: Duration::from_millis(state),
        modal: Duration::from_secs(10),
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
    // Only the slow block is late, however busy the machine.
    pool.set_deadline_floor(Duration::from_millis(30));
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

/// A pause of Cantus (a laptop asleep) while a job is late is not the plug-in's hang: its
/// job clock starts again with the helper's own (review isolation-4 #12, #22, #36).
#[test]
fn a_pause_while_a_job_is_late_ends_nothing() {
    let _g = serial();
    let pool = limits(300, 2_000, 2_000);
    let Loaded {
        main: mut m,
        audio: mut s,
    } = load(&pool, GAIN_ID, "own-pause");
    m.idle();
    noisy(s.as_mut(), &fault(P_FAULT, SLOW_SECOND));
    // Cantus does not run for longer than the silence and the hang limits.
    std::thread::sleep(Duration::from_millis(700));
    assert_eq!(crashed(m.as_mut()), None, "the pause was not taken for a hang");
    // The job finishes; nothing ended meanwhile.
    let start = Instant::now();
    while start.elapsed() < Duration::from_millis(600) {
        assert_eq!(crashed(m.as_mut()), None);
        std::thread::sleep(Duration::from_millis(20));
    }
    noisy(s.as_mut(), &[]);
    assert!(m.stopped().is_none());
}

#[cfg(windows)]
fn parent() -> raw_window_handle::RawWindowHandle {
    raw_window_handle::RawWindowHandle::Win32(raw_window_handle::Win32WindowHandle::new(
        std::num::NonZeroIsize::new(1).unwrap(),
    ))
}

fn arrived(main: &mut dyn MainThread, id: u32, value: f64) {
    let start = Instant::now();
    while main.param_value(id) != Some(value) {
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "{id} never arrived"
        );
        main.idle();
        std::thread::sleep(Duration::from_millis(2));
    }
}

/// Runs `f` while the main half idles, for `long`.
fn idle_for(main: &mut dyn MainThread, long: Duration) {
    let start = Instant::now();
    while start.elapsed() < long {
        main.idle();
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// A save queued behind the plug-in's own slow save (its progress window up, within its
/// limit) waits from when the helper starts it, not from when it was posted, and is the
/// state after the change, not the one before (review isolation-4 #1).
#[cfg(windows)]
#[test]
fn a_save_behind_a_slow_one_waits_from_its_own_start() {
    let _g = serial();
    let pool = limits(1_000, 1_000, 4_000);
    let Loaded {
        mut main,
        mut audio,
    } = load(&pool, GAIN_ID, "own-save-behind");
    let before = main.save_state().unwrap();
    noisy(
        audio.as_mut(),
        &[ParamChange {
            offset: 0,
            id: P_GAIN,
            value: -6.0,
        }],
    );
    arrived(main.as_mut(), P_GAIN, -6.0);
    // Its next two saves each run a 3 s loop of their own.
    noisy(audio.as_mut(), &fault(P_FAULT, PUMP_SAVE));
    main.checkpoint();
    main.idle();
    std::thread::sleep(Duration::from_millis(500));
    noisy(audio.as_mut(), &fault(P_FAULT, PUMP_SAVE));
    std::thread::sleep(Duration::from_millis(500));
    // A second on: saved, behind the background save (to ~3 s), then its own (to ~6 s).
    let t = Instant::now();
    let saved = main.save_state().unwrap();
    assert!(t.elapsed() < Duration::from_secs(8), "{:?}", t.elapsed());
    assert_ne!(saved, before, "the state after the change");
    idle_for(main.as_mut(), Duration::from_millis(200));
    assert_eq!(saved, main.save_state().unwrap());
}

/// A restart is never waited for past a moment: held behind the plug-in's own dialog, or
/// queued behind a neighbour's slow save, each look at it costs no wait, and it completes
/// once the dialog or the save ends (review isolation-4 #4, #23, #26, #51, #60).
#[cfg(windows)]
#[test]
fn a_restart_is_polled_never_waited_for() {
    let _g = serial();
    let pool = limits(5_000, 5_000, 5_000);
    let Loaded {
        main: mut a,
        audio: mut a_audio,
    } = load(&pool, GAIN_ID, "together-restart");
    // B's audio half goes at once, as the engine lets go of a restarting plug-in's.
    let Loaded { main: mut b, .. } = load(&pool, GAIN_ID, "together-restart");
    let Loaded {
        main: mut slow,
        audio: mut slow_audio,
    } = load(&pool, GAIN_ID, "together-restart");
    // A's own dialog, from its window: its restart is held for after it.
    a.open_editor(parent()).expect("the editor opens");
    noisy(a_audio.as_mut(), &fault(P_FAULT, PUMP_WINDOW));
    std::thread::sleep(Duration::from_millis(300));
    // The engine lets go of its audio half first (held for after the dialog too).
    drop(a_audio);
    let polls = |m: &mut Box<dyn MainThread>, what: &str| {
        let start = Instant::now();
        let mut first = true;
        loop {
            let t = Instant::now();
            let r = m.restart().expect("a restart");
            let took = t.elapsed();
            let bound = if first { 250 } else { 40 };
            assert!(
                took < Duration::from_millis(bound),
                "{what}: a look at the restart took {took:?}"
            );
            first = false;
            if r.is_some() {
                return start.elapsed();
            }
            assert!(start.elapsed() < Duration::from_secs(10), "{what} never restarted");
            m.idle();
            std::thread::sleep(Duration::from_millis(16));
        }
    };
    let took = polls(&mut a, "A behind its dialog");
    assert!(took > Duration::from_secs(1), "it waited for the dialog");
    // B's restart queued behind a neighbour's slow save (3 s, without pumping).
    noisy(slow_audio.as_mut(), &fault(P_FAULT, SLOW_SAVE));
    arrived(slow.as_mut(), P_FAULT, SLOW_SAVE);
    slow.checkpoint();
    slow.idle();
    std::thread::sleep(Duration::from_millis(200));
    let took = polls(&mut b, "B behind a slow save");
    assert!(took > Duration::from_millis(1_500), "it waited for the save");
    for m in [&mut a, &mut b, &mut slow] {
        assert!(crashed(m.as_mut()).is_none());
    }
}

/// A plug-in's dialog inside a call of its own (a progress window in its save) never makes
/// a whole project's save wait: its neighbours' states — as they are now — are taken from
/// inside the dialog, and its own is given up within one limit: the save as a whole
/// returns within one limit (review isolation-4 #13, #43, #54).
#[cfg(windows)]
#[test]
fn a_dialog_inside_a_call_never_holds_the_neighbours() {
    let _g = serial();
    let pool = limits(5_000, 1_000, 1_000);
    let Loaded {
        main: mut a,
        audio: mut a_audio,
    } = load(&pool, GAIN_ID, "together-dialog-call");
    let Loaded {
        main: mut b,
        audio: mut b_audio,
    } = load(&pool, GAIN_ID, "together-dialog-call");
    let Loaded {
        main: mut c,
        audio: mut c_audio,
    } = load(&pool, GAIN_ID, "together-dialog-call");
    let firsts: Vec<Vec<u8>> = [&mut b, &mut c]
        .into_iter()
        .map(|m| m.save_state().unwrap())
        .collect();
    // B and C moved since (their last-good states are the first ones).
    for (m, au) in [(&mut b, &mut b_audio), (&mut c, &mut c_audio)] {
        let gain = [ParamChange {
            offset: 0,
            id: P_GAIN,
            value: -6.0,
        }];
        noisy(au.as_mut(), &gain);
        arrived(m.as_mut(), P_GAIN, -6.0);
    }
    noisy(a_audio.as_mut(), &fault(P_FAULT, PUMP_SAVE));
    arrived(a.as_mut(), P_FAULT, PUMP_SAVE);
    a.checkpoint();
    a.idle();
    std::thread::sleep(Duration::from_millis(300));
    // The project saved: every plug-in's state, in turn.
    let t = Instant::now();
    assert!(a.save_state().is_ok());
    let now: Vec<Vec<u8>> = [&mut b, &mut c]
        .into_iter()
        .map(|m| m.save_state().unwrap())
        .collect();
    assert!(
        t.elapsed() < Duration::from_millis(1_500),
        "the save took {:?}",
        t.elapsed()
    );
    for (first, now) in firsts.iter().zip(&now) {
        assert_ne!(first, now, "a neighbour's state as it is now, taken in the dialog");
    }
    for m in [&mut a, &mut b, &mut c] {
        assert!(crashed(m.as_mut()).is_none());
    }
}

/// A state that takes longer than the state limit without pumping (a sampler reading its
/// samples) gives the caller up, not the helper: it is not taken for hung before the
/// heavy limit, and its answer still becomes the last-good state (review isolation-4 #66,
/// #1, #20).
#[test]
fn a_slow_state_past_its_limit_is_not_a_hang() {
    let _g = serial();
    let pool = pool();
    pool.set_limits(Limits {
        silence: Duration::from_millis(500),
        state: Duration::from_millis(1_000),
        heavy: Duration::from_secs(10),
        ..Limits::default()
    });
    let Loaded {
        mut main,
        mut audio,
    } = load(&pool, GAIN_ID, "own-heavy");
    noisy(
        audio.as_mut(),
        &[ParamChange {
            offset: 0,
            id: P_GAIN,
            value: -6.0,
        }],
    );
    arrived(main.as_mut(), P_GAIN, -6.0);
    noisy(audio.as_mut(), &fault(P_FAULT, SLOW_SAVE));
    arrived(main.as_mut(), P_FAULT, SLOW_SAVE);
    let before = main.save_state();
    // That save was the slow one (3 s, silent): given up, its answer kept for later.
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(4) {
        assert_eq!(crashed(main.as_mut()), None, "a slow state is not a hang");
        std::thread::sleep(Duration::from_millis(20));
    }
    let fresh = main.save_state().unwrap();
    // Gone now: what the slow save answered is what the project keeps.
    let mut l = vec![0.0; 64];
    let mut r = l.clone();
    call(audio.as_mut(), &mut l, &mut r, &fault(P_FAULT, ABORT_PROCESS), &[]);
    assert!(crashed_within(main.as_mut(), Duration::from_secs(5)).is_some());
    assert_eq!(main.save_state().unwrap(), fresh);
    drop(before);
}

/// A helper slow to start (an antivirus checking it on its first run) is judged by its
/// handshake's own limit, not by the silence limit (review isolation-4 #50).
#[test]
fn a_helper_slow_to_start_is_not_taken_for_hung() {
    let _g = serial();
    let pool = pool();
    pool.set_limits(Limits {
        silence: Duration::from_millis(300),
        load: Duration::from_secs(10),
        ..Limits::default()
    });
    std::env::set_var("CANTUS_SANDBOX_START_DELAY_MS", "1200");
    let info = info(GAIN_ID);
    let loaded = bridged(&pool, info.format, "own-slow-start", false).load(&info, setup());
    std::env::remove_var("CANTUS_SANDBOX_START_DELAY_MS");
    let Loaded { mut main, .. } = loaded.expect("it starts, slowly, and loads");
    assert!(crashed(main.as_mut()).is_none());
}

/// A parameter list the plug-in announced while its helper was too busy to answer in time
/// is taken when it comes, and the host told again (review isolation-4 #20).
#[test]
fn a_parameter_list_given_up_on_is_taken_when_it_comes() {
    let _g = serial();
    let pool = limits(5_000, 500, 5_000);
    let Loaded {
        mut main,
        mut audio,
    } = load(&pool, GAIN_ID, "own-params-late");
    noisy(audio.as_mut(), &fault(P_FAULT, RESCAN_SLOW));
    let start = Instant::now();
    let mut told = 0;
    while start.elapsed() < Duration::from_secs(4) {
        told += main
            .idle()
            .iter()
            .filter(|n| matches!(n, Notice::ParamsChanged))
            .count();
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(told, 2, "told when it changed, and again when its list came");
    assert!(!main.params().is_empty());
}

/// A window asked for while the plug-in's own dialog holds it: given up at once, saying
/// so, and opened — and the host told — once the dialog closes (review isolation-4 #39).
#[cfg(windows)]
#[test]
fn a_window_asked_for_during_a_dialog_opens_after_it() {
    let _g = serial();
    let pool = limits(5_000, 5_000, 5_000);
    let Loaded {
        mut main,
        mut audio,
    } = load(&pool, GAIN_ID, "own-window-late");
    main.open_editor(parent()).expect("the editor opens");
    noisy(audio.as_mut(), &fault(P_FAULT, PUMP_WINDOW));
    std::thread::sleep(Duration::from_millis(300));
    main.close_editor();
    let t = Instant::now();
    match main.open_editor(parent()) {
        Err(cantus_host::HostError::Busy(why)) => assert!(why.contains("holds"), "{why}"),
        other => panic!("{other:?}"),
    }
    assert!(t.elapsed() < Duration::from_millis(300), "{:?}", t.elapsed());
    let start = Instant::now();
    let mut opened = false;
    while start.elapsed() < Duration::from_secs(6) && !opened {
        opened = main.idle().contains(&Notice::EditorOpened);
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(opened, "the window opened after the dialog, and the host was told");
}

/// Saving while the plug-in's own save hangs answers at once with production limits, too
/// (review isolation-4 #27).
#[test]
fn saving_while_its_own_save_hangs_answers_at_once_with_real_limits() {
    let _g = serial();
    let pool = pool();
    let Loaded {
        mut main,
        mut audio,
    } = load(&pool, GAIN_ID, "own-hang-real");
    let good = main.save_state().unwrap();
    noisy(audio.as_mut(), &fault(P_FAULT, HANG_SAVE));
    arrived(main.as_mut(), P_FAULT, HANG_SAVE);
    main.checkpoint();
    idle_for(main.as_mut(), Duration::from_millis(1_600));
    let t = Instant::now();
    assert_eq!(main.save_state().unwrap(), good);
    assert!(
        t.elapsed() < Duration::from_millis(300),
        "{:?}",
        t.elapsed()
    );
}

/// The pause guard, through a helper: a helper silent since before a pause of Cantus is
/// not killed at the first look after it — and is, one silence later, when it really hangs
/// (review isolation-4 #36).
#[test]
fn a_pause_is_not_a_silence_but_a_real_silence_still_is() {
    let _g = serial();
    let pool = pool();
    pool.set_limits(Limits {
        silence: Duration::from_millis(300),
        state: Duration::from_millis(400),
        heavy: Duration::from_millis(400),
        ..Limits::default()
    });
    let Loaded {
        mut main,
        mut audio,
    } = load(&pool, GAIN_ID, "own-pause-silence");
    noisy(audio.as_mut(), &fault(P_FAULT, HANG_SAVE));
    arrived(main.as_mut(), P_FAULT, HANG_SAVE);
    main.checkpoint();
    main.idle();
    // Cantus pauses for a second: the helper (hung in its save) is silent meanwhile.
    std::thread::sleep(Duration::from_secs(1));
    assert_eq!(crashed(main.as_mut()), None, "not at the first look after a pause");
    assert!(
        crashed_within(main.as_mut(), Duration::from_millis(1_500)).is_some(),
        "a real hang is still ended"
    );
}

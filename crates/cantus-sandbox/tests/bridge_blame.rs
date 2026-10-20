//! Whose a helper's end is (round 29, review isolation-4), through real helpers: a crash in
//! a plug-in's destructor is its own, counted after it left; a crash or a hang in a
//! framework's timer (no window of the plug-in's) is the plug-in's whose library the timer
//! is in; a neighbour's window crashing inside another plug-in's dialog is the neighbour's;
//! a crash reporter that hands back to ours changes nothing; two jobs running at a fail-fast
//! abort is nobody's; the last plug-in of a helper is ended by its own destructor, not cut.

mod support;

use cantus_host::bridge::sandbox::Limits;
use cantus_host::bridge::Sandboxes;
use cantus_host::{AudioThread, Backend, Blame, Loaded, MainThread, NoteEvent, Notice, ParamChange};
use std::sync::Arc;
use std::time::{Duration, Instant};
use support::*;

fn load(pool: &Arc<Sandboxes>, id: &str, group: &str) -> Loaded {
    let info = info(id);
    bridged(pool, info.format, group, false)
        .load(&info, setup())
        .expect("loads in the helper")
}

fn crashed(main: &mut dyn MainThread) -> Option<(Blame, String)> {
    main.idle().into_iter().find_map(|n| match n {
        Notice::Crashed { blame, reason } => Some((blame, reason)),
        _ => None,
    })
}

fn crashed_within(main: &mut dyn MainThread, limit: Duration) -> Option<(Blame, String)> {
    let start = Instant::now();
    while start.elapsed() < limit {
        if let Some(b) = crashed(main) {
            eprintln!("crashed: {} ({:?})", b.1, b.0);
            return Some(b);
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    None
}

fn block(audio: &mut dyn AudioThread, p: &[ParamChange], n: &[NoteEvent]) {
    let (mut l, mut r) = (vec![0.0f32; 128], vec![0.0f32; 128]);
    call(audio, &mut l, &mut r, p, n);
}

#[cfg(windows)]
fn parent() -> raw_window_handle::RawWindowHandle {
    raw_window_handle::RawWindowHandle::Win32(raw_window_handle::Win32WindowHandle::new(
        std::num::NonZeroIsize::new(1).unwrap(),
    ))
}

/// A plug-in that crashes in its destructor as it is removed: the one that stays is a
/// neighbour of that end, never its culprit, and the one that left is counted for it
/// (review isolation-4 #2, #45, #56).
#[test]
fn a_crash_in_a_destructor_is_the_leaving_plugins() {
    let _g = serial();
    let pool = pool();
    let Loaded {
        main: mut a,
        audio: _a_audio,
    } = load(&pool, GAIN_ID, "together-destroy");
    let Loaded {
        main: x,
        audio: mut x_audio,
    } = load(&pool, GAIN_ID, "together-destroy");
    block(x_audio.as_mut(), &fault(P_FAULT, SEGV_DESTROY), &[]);
    std::thread::sleep(Duration::from_millis(100));
    // Removed, the transport stopped: its destructor crashes the helper.
    drop(x_audio);
    drop(x);
    let (blame, _) = crashed_within(a.as_mut(), Duration::from_secs(5)).expect("it ended");
    assert_eq!(blame, Blame::Neighbour, "the one that stays did nothing");
    let departed = pool.take_departed_crashes();
    assert_eq!(departed.len(), 1, "the one that left is told: {departed:?}");
    assert_eq!(departed[0].info.id, GAIN_ID);
}

/// A crash in a framework's timer — a thread timer, no window of the plug-in's, as
/// VSTGUI's and JUCE's are — is laid on the plug-in whose library it is in, and the other
/// plug-in of the helper is its neighbour (review isolation-4 #29, #55).
#[cfg(windows)]
#[test]
fn a_crash_in_a_framework_timer_is_its_plugins() {
    let _g = serial();
    let pool = pool();
    let Loaded {
        main: mut a,
        audio: mut a_audio,
    } = load(&pool, GAIN_ID, "together-timer-crash");
    let Loaded {
        main: mut b,
        audio: _b_audio,
    } = load(&pool, SYNTH_ID, "together-timer-crash");
    block(a_audio.as_mut(), &fault(P_FAULT, SEGV_THREAD_TIMER), &[]);
    let (blame, why) = crashed_within(a.as_mut(), Duration::from_secs(5)).expect("it ended");
    assert_eq!(blame, Blame::This, "{why}");
    assert_eq!(crashed_within(b.as_mut(), Duration::from_secs(2)).map(|c| c.0), Some(Blame::Neighbour));
}

/// A hang in a framework's timer is laid on the plug-in whose library the timer is in —
/// never on a bystander, never on nobody (review isolation-4 #3, #30, #44, #55).
#[cfg(windows)]
#[test]
fn a_hang_in_a_framework_timer_is_its_plugins() {
    let _g = serial();
    let pool = pool();
    pool.set_limits(Limits {
        silence: Duration::from_millis(600),
        ..Limits::default()
    });
    let Loaded {
        main: mut a,
        audio: mut a_audio,
    } = load(&pool, GAIN_ID, "together-timer-hang");
    let Loaded {
        main: mut b,
        audio: mut b_audio,
    } = load(&pool, SYNTH_ID, "together-timer-hang");
    block(a_audio.as_mut(), &fault(P_FAULT, HANG_THREAD_TIMER), &[]);
    // The song plays meanwhile: B's jobs run.
    let start = Instant::now();
    let mut ended = None;
    while start.elapsed() < Duration::from_secs(5) && ended.is_none() {
        block(b_audio.as_mut(), &[], &[]);
        ended = crashed(a.as_mut());
    }
    let (blame, why) = ended.expect("the hang was ended");
    assert_eq!(blame, Blame::This, "{why}");
    assert_eq!(crashed_within(b.as_mut(), Duration::from_secs(2)).map(|c| c.0), Some(Blame::Neighbour));
}

/// A neighbour's window crashing while another plug-in's dialog runs its loop: laid on the
/// neighbour whose window it was, not on the plug-in whose dialog dispatched it (review
/// isolation-4 #37, #48).
#[cfg(windows)]
#[test]
fn a_neighbours_window_crashing_inside_a_dialog_is_the_neighbours() {
    let _g = serial();
    let pool = pool();
    let Loaded {
        main: mut a,
        audio: mut a_audio,
    } = load(&pool, GAIN_ID, "together-dialog-crash");
    let Loaded {
        main: mut b,
        audio: mut b_audio,
    } = load(&pool, GAIN_ID, "together-dialog-crash");
    a.open_editor(parent()).expect("A's editor");
    b.open_editor(parent()).expect("B's editor");
    // A's window runs its dialog (3 s); inside it, B's window's timer crashes.
    block(a_audio.as_mut(), &fault(P_FAULT, PUMP_WINDOW), &[]);
    std::thread::sleep(Duration::from_millis(400));
    block(b_audio.as_mut(), &fault(P_FAULT, SEGV_EDITOR), &[]);
    let (blame, why) = crashed_within(b.as_mut(), Duration::from_secs(5)).expect("it ended");
    assert_eq!(blame, Blame::This, "{why}");
    assert_eq!(crashed_within(a.as_mut(), Duration::from_secs(2)).map(|c| c.0), Some(Blame::Neighbour));
}

/// A plug-in's crash reporter that hands every crash back to the filter before it (ours):
/// the crash still ends the helper as what it was — an access violation, laid on the
/// plug-in — not as a stack overflow of filters calling each other (review isolation-4 #7,
/// #16, #19, #28, #47, #61).
#[cfg(windows)]
#[test]
fn a_crash_reporter_that_hands_back_changes_nothing() {
    let _g = serial();
    let pool = pool();
    std::env::set_var("CANTUS_TEST_GAIN_CHAIN_FILTER", "1");
    let loaded = std::panic::catch_unwind(|| load(&pool, GAIN_ID, "own-chain-filter"));
    std::env::remove_var("CANTUS_TEST_GAIN_CHAIN_FILTER");
    let Loaded {
        mut main,
        mut audio,
    } = loaded.expect("loads");
    block(audio.as_mut(), &fault(P_FAULT, SEGV_PROCESS), &[]);
    let (blame, why) = crashed_within(main.as_mut(), Duration::from_secs(10)).expect("it ended");
    assert_eq!(blame, Blame::This, "{why}");
    assert!(why.contains("0xC0000005"), "the access violation it was: {why}");
}

/// Two jobs running when a plug-in aborts (a fail-fast: no handler runs): nothing says
/// whose — neither is laid on (review isolation-4 #32, review #21).
#[cfg(windows)]
#[test]
fn a_fail_fast_with_two_jobs_running_is_nobodys() {
    let _g = serial();
    let pool = pool();
    pool.set_threads(Some(2));
    let Loaded {
        main: mut a,
        audio: mut a_audio,
    } = load(&pool, GAIN_ID, "together-fail-fast");
    let Loaded {
        main: mut b,
        audio: mut b_audio,
    } = load(&pool, GAIN_ID, "together-fail-fast");
    // A's job runs on for a second (late, still running); B aborts on the other thread.
    block(a_audio.as_mut(), &fault(P_FAULT, SLOW_SECOND), &[]);
    block(b_audio.as_mut(), &fault(P_FAULT, ABORT_PROCESS), &[]);
    let a_blame = crashed_within(a.as_mut(), Duration::from_secs(5)).expect("ended").0;
    let b_blame = crashed_within(b.as_mut(), Duration::from_secs(2)).expect("ended").0;
    assert_eq!((a_blame, b_blame), (Blame::Unknown, Blame::Unknown));
}

/// The last plug-in of a helper, removed: its destructor runs to its end — the helper is
/// given the time to quit by itself, not cut down at once (review isolation-4 #46).
#[test]
fn the_last_plugins_destructor_runs_to_its_end() {
    let _g = serial();
    let pool = pool();
    let dir = tempfile::tempdir().expect("a folder");
    std::env::set_var("CANTUS_TEST_GAIN_DESTROY_MARKER", dir.path());
    let loaded = std::panic::catch_unwind(|| load(&pool, GAIN_ID, "own-destructor"));
    std::env::remove_var("CANTUS_TEST_GAIN_DESTROY_MARKER");
    let Loaded { main, audio } = loaded.expect("loads");
    let count = || std::fs::read_dir(dir.path()).map_or(0, |d| d.count());
    // (Loading may make and end a probe instance of its own.)
    std::thread::sleep(Duration::from_millis(500));
    let before = count();
    drop(audio);
    drop(main);
    let start = Instant::now();
    loop {
        if count() > before {
            break;
        }
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "its destructor never ran to its end"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// A plug-in's crash reporter that resumes an exception of its own (a licence check's
/// trick): no crash after all — so a later end of the helper is not laid on it (review
/// isolation-4 #47).
#[cfg(windows)]
#[test]
fn an_exception_a_plugin_handled_is_no_crash_of_its() {
    let _g = serial();
    let pool = pool();
    std::env::set_var("CANTUS_TEST_GAIN_CHAIN_FILTER", "1");
    let loaded = std::panic::catch_unwind(|| load(&pool, GAIN_ID, "together-handled"));
    std::env::remove_var("CANTUS_TEST_GAIN_CHAIN_FILTER");
    let Loaded {
        main: mut a,
        audio: mut a_audio,
    } = loaded.expect("loads");
    let Loaded {
        main: mut b,
        audio: mut b_audio,
    } = load(&pool, SYNTH_ID, "together-handled");
    // A raises its own exception, which its reporter resumes: it plays on.
    block(a_audio.as_mut(), &fault(P_FAULT, RAISE_HANDLED), &[]);
    std::thread::sleep(Duration::from_millis(200));
    assert!(crashed(a.as_mut()).is_none(), "it went on");
    // B then aborts (a fail-fast: no handler runs) with its job running alone.
    block(b_audio.as_mut(), &fault(S_FAULT, ABORT_PROCESS), &[]);
    let (blame, why) = crashed_within(b.as_mut(), Duration::from_secs(5)).expect("it ended");
    assert_eq!(blame, Blame::This, "{why}");
    assert_eq!(
        crashed_within(a.as_mut(), Duration::from_secs(2)).map(|c| c.0),
        Some(Blame::Neighbour)
    );
}

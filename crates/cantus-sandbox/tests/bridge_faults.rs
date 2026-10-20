//! The review's faults (round 29, isolation-2): a plug-in that hangs on its main thread
//! (saving, loading a state, opening its editor, a background save), one that runs a modal
//! loop (a save, its own window), one late block and the notes and values of the blocks
//! that pass without it, a late block that finished while nobody called again, an offline
//! render through a hung plug-in, a block sent after its instance left — each through a
//! real helper process.

mod support;

use cantus_host::bridge::sandbox::Limits;
use cantus_host::bridge::Sandboxes;
use cantus_host::{
    Backend, Blame, HostError, Loaded, MainThread, NoteEvent, NoteKind, Notice, ParamChange,
    ParamOut,
};
use std::sync::Arc;
use std::time::{Duration, Instant};
use support::*;

fn load(pool: &Arc<Sandboxes>, id: &str, group: &str, offline: bool) -> Loaded {
    let info = info(id);
    bridged(pool, info.format, group, offline)
        .load(&info, setup())
        .expect("loads in the helper")
}

/// Short limits, so a hang is ended within a test's patience.
fn quick_pool() -> Arc<Sandboxes> {
    let pool = pool();
    pool.set_limits(Limits {
        silence: Duration::from_secs(1),
        load: Duration::from_secs(5),
        state: Duration::from_secs(1),
        call: Duration::from_secs(1),
        hang: Duration::from_millis(500),
        offline: Duration::from_secs(1),
        heavy: Duration::from_secs(1),
        modal: Duration::from_secs(10),
    });
    pool
}

fn arrived(main: &mut dyn MainThread, id: u32, value: f64) {
    let start = Instant::now();
    while main.param_value(id) != Some(value) {
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "parameter {id} never arrived"
        );
        main.idle();
        std::thread::sleep(Duration::from_millis(2));
    }
}

/// Polls `idle` until it says the plug-in crashed (its reason and blame), or `limit`.
fn crashed_within(main: &mut dyn MainThread, limit: Duration) -> Option<(String, Blame)> {
    let start = Instant::now();
    while start.elapsed() < limit {
        for n in main.idle() {
            if let Notice::Crashed { reason, blame } = n {
                return Some((reason, blame));
            }
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    None
}

/// Arms `fault` in `audio` (a change inside `process`) and waits until it is in.
fn arm(main: &mut dyn MainThread, audio: &mut dyn cantus_host::AudioThread, id: u32, fault: f64) {
    let (mut l, mut r) = (vec![0.0; 64], vec![0.0; 64]);
    call(audio, &mut l, &mut r, &fault_change(id, fault), &[]);
    arrived(main, id, fault);
}

fn fault_change(id: u32, value: f64) -> [ParamChange; 1] {
    fault(id, value)
}

fn set(main: &mut dyn MainThread, id: u32, value: f64) {
    let mut out = ParamOut::new();
    main.flush(
        &[ParamChange {
            offset: 0,
            id,
            value,
        }],
        &mut out,
    );
}

#[test]
fn a_plugin_that_hangs_while_saving_answers_with_its_last_good_state_within_the_limit() {
    let _g = serial();
    let pool = quick_pool();
    for (id, gain, fault_id) in [(GAIN_ID, P_GAIN, P_FAULT), (VST3_GAIN_ID, V_GAIN, V_FAULT)] {
        let Loaded {
            mut main,
            mut audio,
        } = load(&pool, id, &format!("own-hang-save-{id}"), false);
        set(main.as_mut(), gain, -6.0);
        let good = main.save_state().expect("a state");
        arm(main.as_mut(), audio.as_mut(), fault_id, HANG_SAVE);
        let t = Instant::now();
        let saved = main
            .save_state()
            .expect("the last-good state, not an error");
        assert!(
            t.elapsed() < Duration::from_secs(4),
            "{id}: the save waited {:?}",
            t.elapsed()
        );
        assert_eq!(saved, good, "{id}");
        let (_, blame) = crashed_within(main.as_mut(), Duration::from_secs(5)).expect("told");
        assert_eq!(blame, Blame::This, "{id}: its own hang");
        assert!(crashed_within(main.as_mut(), Duration::from_millis(100)).is_none());
    }
}

#[test]
fn a_plugin_that_hangs_or_dies_loading_a_state_keeps_the_state_asked_for() {
    let _g = serial();
    let pool = quick_pool();
    for (fault_value, what) in [(HANG_LOAD, "hang"), (ABORT_LOAD, "abort")] {
        let Loaded {
            mut main,
            mut audio,
        } = load(&pool, GAIN_ID, &format!("own-load-{what}"), false);
        let default = main.save_state().unwrap();
        set(main.as_mut(), P_GAIN, -6.0);
        let changed = main.save_state().unwrap();
        assert_ne!(default, changed);
        arm(main.as_mut(), audio.as_mut(), P_FAULT, fault_value);
        // The project asks for the default state back (an undo, a preset)…
        let t = Instant::now();
        assert!(main.load_state(&default).is_err(), "{what}: an error");
        assert!(t.elapsed() < Duration::from_secs(4), "{:?}", t.elapsed());
        assert!(crashed_within(main.as_mut(), Duration::from_secs(5)).is_some());
        // …and that is what saving and reloading keep, not the state before it.
        assert_eq!(main.save_state().unwrap(), default, "{what}");
        // Gone, a state the project changes to (an undo) is still what it keeps.
        assert!(main.load_state(&changed).is_err());
        assert_eq!(
            main.save_state().unwrap(),
            changed,
            "{what}: after the crash"
        );
    }
}

/// The value a fresh instance shows for `id` once `state` is loaded into it.
fn value_in(pool: &Arc<Sandboxes>, state: &[u8], id: u32) -> Option<f64> {
    let Loaded { mut main, .. } = load(pool, GAIN_ID, "own-fresh", false);
    main.load_state(state).expect("loads");
    main.idle();
    main.param_value(id)
}

/// A background save the plug-in refuses once is asked again: the last-good state keeps
/// following what the user does (review #8, #50).
#[test]
fn a_refused_background_save_is_asked_again() {
    let _g = serial();
    let pool = pool();
    let Loaded {
        mut main,
        mut audio,
    } = load(&pool, GAIN_ID, "own-refused-save", false);
    set(main.as_mut(), P_GAIN, -6.0);
    arm(main.as_mut(), audio.as_mut(), P_FAULT, FAIL_SAVE_ONCE);
    main.checkpoint();
    let start = Instant::now();
    while start.elapsed() < Duration::from_millis(3500) {
        main.idle();
        std::thread::sleep(Duration::from_millis(10));
    }
    arm_crash(main.as_mut(), audio.as_mut());
    let saved = main.save_state().expect("the last-good state");
    assert_eq!(value_in(&pool, &saved, P_GAIN), Some(-6.0));
}

/// Crashes the helper from `process` and waits until it is told.
fn arm_crash(main: &mut dyn MainThread, audio: &mut dyn cantus_host::AudioThread) {
    let (mut l, mut r) = (vec![0.0; 64], vec![0.0; 64]);
    call(audio, &mut l, &mut r, &fault(P_FAULT, ABORT_PROCESS), &[]);
    assert!(crashed_within(main, Duration::from_secs(5)).is_some());
}

/// A background save answered after a state was loaded is older than it: dropped, so the
/// last-good state is the one loaded (review #7, #50).
#[cfg(windows)]
#[test]
fn a_background_state_older_than_a_loaded_one_is_dropped() {
    let _g = serial();
    let pool = pool();
    let Loaded {
        mut main,
        mut audio,
    } = load(&pool, GAIN_ID, "own-older-state", false);
    let first = main.save_state().unwrap();
    set(main.as_mut(), P_GAIN, -6.0);
    arm(main.as_mut(), audio.as_mut(), P_FAULT, PUMP_SAVE);
    main.checkpoint();
    // The background save is asked for (half a second on) and runs its 3 s loop.
    let start = Instant::now();
    while start.elapsed() < Duration::from_millis(800) {
        main.idle();
        std::thread::sleep(Duration::from_millis(10));
    }
    // Its own dialog runs the save: the load is held for after it — not waited for
    // (review isolation-4 #54) — and runs then.
    match main.load_state(&first) {
        Ok(()) | Err(HostError::Busy(_)) => {}
        Err(e) => panic!("{e}"),
    }
    let start = Instant::now();
    while start.elapsed() < Duration::from_millis(3_500) {
        main.idle();
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(
        main.save_state().unwrap(),
        first,
        "the held load ran after the loop"
    );
    arm_crash(main.as_mut(), audio.as_mut());
    assert_eq!(main.save_state().unwrap(), first);
}

#[cfg(windows)]
#[test]
fn a_plugin_that_hangs_opening_its_editor_is_an_error_within_the_limit() {
    let _g = serial();
    let pool = quick_pool();
    let Loaded {
        mut main,
        mut audio,
    } = load(&pool, GAIN_ID, "own-hang-editor", false);
    arm(main.as_mut(), audio.as_mut(), P_FAULT, HANG_EDITOR);
    let parent = raw_window_handle::RawWindowHandle::Win32(
        raw_window_handle::Win32WindowHandle::new(std::num::NonZeroIsize::new(1).unwrap()),
    );
    let t = Instant::now();
    assert!(main.open_editor(parent).is_err());
    assert!(t.elapsed() < Duration::from_secs(4), "{:?}", t.elapsed());
    assert!(crashed_within(main.as_mut(), Duration::from_secs(5)).is_some());
}

#[test]
fn a_background_save_that_hangs_is_ended_and_saving_meanwhile_does_not_wait() {
    let _g = serial();
    let pool = quick_pool();
    // A state may take 3 s: the hung background save is not overdue when Cantus saves.
    pool.set_limits(Limits {
        state: Duration::from_secs(3),
        heavy: Duration::from_secs(3),
        ..pool.limits()
    });
    let Loaded {
        mut main,
        mut audio,
    } = load(&pool, GAIN_ID, "own-hang-background", false);
    set(main.as_mut(), P_GAIN, -6.0);
    let good = main.save_state().unwrap();
    arm(main.as_mut(), audio.as_mut(), P_FAULT, HANG_SAVE);
    // A gesture ended: the last-good state is refreshed in the background — and hangs.
    main.checkpoint();
    let start = Instant::now();
    let mut saved_quickly = false;
    let mut told = None;
    while start.elapsed() < Duration::from_secs(8) && told.is_none() {
        for n in main.idle() {
            if let Notice::Crashed { reason, .. } = n {
                told = Some(reason);
            }
        }
        // Saving meanwhile answers at once with what it said last: its own request has
        // run a second with its helper silent — the helper's main thread is stuck in its
        // code (review #51; isolation-4 #27).
        if !saved_quickly && start.elapsed() > Duration::from_millis(1600) && told.is_none() {
            let t = Instant::now();
            assert_eq!(main.save_state().unwrap(), good);
            assert!(
                t.elapsed() < Duration::from_millis(300),
                "{:?}",
                t.elapsed()
            );
            saved_quickly = true;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(told.is_some(), "the hung helper was ended");
    assert!(
        start.elapsed() < Duration::from_secs(7),
        "{:?}",
        start.elapsed()
    );
}

/// A plug-in that runs a message loop of its own for 3 s (a modal dialog) is alive: with
/// a 1 s silence limit, neither its background save nor a save Cantus waits for kills it.
#[cfg(windows)]
#[test]
fn a_modal_loop_while_saving_is_not_taken_for_a_hang() {
    let _g = serial();
    let pool = quick_pool();
    let Loaded {
        mut main,
        mut audio,
    } = load(&pool, GAIN_ID, "own-pump-save", false);
    set(main.as_mut(), P_GAIN, -6.0);
    arm(main.as_mut(), audio.as_mut(), P_FAULT, PUMP_SAVE);
    main.checkpoint();
    let start = Instant::now();
    while start.elapsed() < Duration::from_millis(1200) {
        assert!(
            !main
                .idle()
                .iter()
                .any(|n| matches!(n, Notice::Crashed { .. })),
            "taken for hung while its modal loop ran"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    // A save while it pumps waits for it, and gets its state.
    assert!(main.save_state().is_ok());
    assert!(crashed_within(main.as_mut(), Duration::from_secs(4)).is_none());
}

/// While a plug-in's window runs a modal loop (a file dialog), Cantus's calls are served
/// from inside it: a save does not wait for the dialog to close.
#[cfg(windows)]
#[test]
fn calls_are_served_while_a_plugin_window_runs_a_modal_loop() {
    let _g = serial();
    let pool = quick_pool();
    let Loaded {
        mut main,
        mut audio,
    } = load(&pool, GAIN_ID, "own-pump-window", false);
    let parent = raw_window_handle::RawWindowHandle::Win32(
        raw_window_handle::Win32WindowHandle::new(std::num::NonZeroIsize::new(1).unwrap()),
    );
    main.open_editor(parent).expect("the editor opens");
    let (mut l, mut r) = (vec![0.0; 64], vec![0.0; 64]);
    call(
        audio.as_mut(),
        &mut l,
        &mut r,
        &fault(P_FAULT, PUMP_WINDOW),
        &[],
    );
    // Its window's timer starts the loop within a few tens of milliseconds.
    std::thread::sleep(Duration::from_millis(300));
    let t = Instant::now();
    assert!(main.save_state().is_ok());
    assert!(
        t.elapsed() < Duration::from_millis(1500),
        "the save waited for the dialog: {:?}",
        t.elapsed()
    );
    assert!(crashed_within(main.as_mut(), Duration::from_secs(3)).is_none());
    main.close_editor();
}

/// One block past its deadline (the plug-in slow once): the blocks after it pass dry at
/// once, only that job counts as late, and what they carried — a note-off, a value — reaches
/// the plug-in with its next job: no note left ringing, no value out of step (review #3,
/// #10, #20, #35).
#[test]
fn a_late_block_keeps_the_notes_and_values_of_the_blocks_that_pass_without_it() {
    let _g = serial();
    let pool = pool();
    let Loaded {
        mut main,
        mut audio,
    } = load(&pool, SYNTH_ID, "own-late-synth", false);
    let note = |kind| NoteEvent {
        offset: 0,
        note_id: 1,
        channel: 0,
        key: 60,
        kind,
    };
    let block = |audio: &mut dyn cantus_host::AudioThread, p: &[ParamChange], n: &[NoteEvent]| {
        let (mut l, mut r) = (vec![0.0f32; 128], vec![0.0f32; 128]);
        let t = Instant::now();
        call(audio, &mut l, &mut r, p, n);
        (t.elapsed(), l)
    };
    let mut before = block(audio.as_mut(), &[], &[note(NoteKind::On { velocity: 0.9 })]).1;
    for _ in 0..4 {
        before.extend(block(audio.as_mut(), &[], &[]).1);
    }
    assert!(
        peak(&before) > 0.0,
        "the note sounded before the late block"
    );
    // Slow once (50 ms): this block is late.
    block(audio.as_mut(), &fault(S_FAULT, SLOW_ONCE), &[]);
    // Dry blocks while it runs: at once, and what they carry kept.
    let (took, _) = block(
        audio.as_mut(),
        &[ParamChange {
            offset: 7,
            id: S_GAIN,
            value: -12.0,
        }],
        &[note(NoteKind::Off { velocity: 0.0 })],
    );
    assert!(
        took < Duration::from_millis(2),
        "a dry block waited {took:?}"
    );
    for _ in 0..3 {
        let (took, _) = block(audio.as_mut(), &[], &[]);
        assert!(
            took < Duration::from_millis(2),
            "a dry block waited {took:?}"
        );
    }
    std::thread::sleep(Duration::from_millis(150));
    // Two seconds on: the note was let go, and the value is the plug-in's.
    let mut last = Vec::new();
    for _ in 0..750 {
        last = block(audio.as_mut(), &[], &[]).1;
    }
    assert_eq!(peak(&last), 0.0, "the note ended");
    arrived(main.as_mut(), S_GAIN, -12.0);
    assert_eq!(main.late_blocks(), 1, "one job late, not one per dry block");
}

/// A block goes late, then nothing calls the plug-in again (its slot bypassed, the engine
/// restarting it): the late job finished, so the helper is not taken for hung (#2, #38).
#[test]
fn a_late_job_that_finished_is_not_a_hang_when_nothing_calls_again() {
    let _g = serial();
    let pool = quick_pool();
    let Loaded {
        mut main,
        mut audio,
    } = load(&pool, GAIN_ID, "own-late-then-quiet", false);
    let (mut l, mut r) = (vec![0.0; 128], vec![0.0; 128]);
    call(
        audio.as_mut(),
        &mut l,
        &mut r,
        &fault(P_FAULT, SLOW_ONCE),
        &[],
    );
    assert_eq!(main.late_blocks(), 1, "the block was late");
    let start = Instant::now();
    while start.elapsed() < Duration::from_millis(1500) {
        assert!(
            !main
                .idle()
                .iter()
                .any(|n| matches!(n, Notice::Crashed { .. })),
            "a finished job was taken for a hang"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    drop(audio);
}

/// An offline render through a plug-in that hangs in `process` ends within the offline
/// limit, the rest of it dry, and the plug-in says it stopped (review #1, #11, #46).
#[test]
fn an_offline_render_through_a_hung_plugin_ends_within_the_limit() {
    let _g = serial();
    let pool = quick_pool();
    let Loaded {
        mut main,
        mut audio,
    } = load(&pool, GAIN_ID, "own-offline-hang", true);
    let l0 = noise(256, 11);
    let (mut l, mut r) = (l0.clone(), l0.clone());
    let t = Instant::now();
    call(
        audio.as_mut(),
        &mut l,
        &mut r,
        &fault(P_FAULT, HANG_PROCESS),
        &[],
    );
    assert!(t.elapsed() < Duration::from_secs(3), "{:?}", t.elapsed());
    assert!(same_bits(&l, &l0), "dry");
    assert!(main.stopped().is_some(), "it says it stopped");
    let _ = crashed_within(main.as_mut(), Duration::from_secs(1));
}

/// A render's Stop ends the wait for a hung plug-in at once (review #1).
#[test]
fn a_cancelled_render_stops_waiting_for_a_hung_plugin() {
    let _g = serial();
    let pool = pool();
    let cancel = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let info = info(GAIN_ID);
    let mut backend = bridged(&pool, info.format, "own-offline-cancel", true);
    backend.cancel = Some(cancel.clone());
    let Loaded { main, mut audio } = backend.load(&info, setup()).expect("loads");
    let stop = {
        let cancel = cancel.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(400));
            cancel.store(true, std::sync::atomic::Ordering::Release);
        })
    };
    let (mut l, mut r) = (vec![0.0; 64], vec![0.0; 64]);
    let t = Instant::now();
    call(
        audio.as_mut(),
        &mut l,
        &mut r,
        &fault(P_FAULT, HANG_PROCESS),
        &[],
    );
    assert!(t.elapsed() < Duration::from_secs(2), "{:?}", t.elapsed());
    stop.join().unwrap();
    drop(main);
}

/// The engine may call an instance one block after its slot left (until it swaps
/// snapshots): that block passes at once, not after a whole deadline (review #5).
#[test]
fn a_block_after_its_instance_left_passes_at_once() {
    let _g = serial();
    let pool = pool();
    let Loaded { main, mut audio } = load(&pool, GAIN_ID, "own-unloaded", false);
    let (mut l, mut r) = (vec![0.0; 256], vec![0.0; 256]);
    call(audio.as_mut(), &mut l, &mut r, &[], &[]);
    drop(main);
    // The helper has taken the instance out (and ended: it was its last).
    std::thread::sleep(Duration::from_millis(300));
    let l0 = noise(256, 3);
    let (mut l, mut r) = (l0.clone(), l0.clone());
    let t = Instant::now();
    call(audio.as_mut(), &mut l, &mut r, &[], &[]);
    assert!(t.elapsed() < Duration::from_millis(2), "{:?}", t.elapsed());
    assert!(same_bits(&l, &l0));
}

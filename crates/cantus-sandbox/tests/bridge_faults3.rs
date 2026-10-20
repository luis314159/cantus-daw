//! The second review of the fix round (round 29, isolation-3), through real helpers: calls
//! held for a plug-in's modal loop never age into a kill nor freeze Cantus; a request is
//! timed from when its helper starts it; a crash is laid on the plug-in that did it — by
//! the crash handler, in an editor's window, with neighbours playing — and never on them; a
//! late job's own events reach the host; a flood of carried events never pushes out the
//! resuming block's note-off; a slow neighbour does not make a save stale; a helper that
//! cannot start is a load failure, not a crash.

mod support;

use cantus_host::bridge::sandbox::Limits;
use cantus_host::bridge::Sandboxes;
use cantus_host::{
    AudioThread, Backend, Blame, HostError, Loaded, MainThread, NoteEvent, NoteKind, Notice,
    ParamChange, ParamEvent, ParamOut,
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

fn arrived(main: &mut dyn MainThread, id: u32, value: f64) {
    let start = Instant::now();
    while main.param_value(id) != Some(value) {
        assert!(start.elapsed() < Duration::from_secs(5), "{id} never arrived");
        main.idle();
        std::thread::sleep(Duration::from_millis(2));
    }
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

#[cfg(windows)]
fn parent() -> raw_window_handle::RawWindowHandle {
    raw_window_handle::RawWindowHandle::Win32(raw_window_handle::Win32WindowHandle::new(
        std::num::NonZeroIsize::new(1).unwrap(),
    ))
}

/// A plug-in's own dialog open for 10 s (six times the call limit and more): its editor
/// closed from Cantus meanwhile (held for after the dialog, on purpose), a neighbour saved
/// and another moved. The helper lives, the neighbours are served at once, and a call on
/// the plug-in in its dialog comes back within its own limit (#4, #11, #14, #17, #36, #44).
#[cfg(windows)]
#[test]
fn a_long_modal_loop_kills_nothing_and_freezes_nothing() {
    let _g = serial();
    let pool = limits(1_000, 1_000, 1_000);
    let Loaded {
        main: mut a,
        audio: mut a_audio,
    } = load(&pool, GAIN_ID, "together-modal");
    let Loaded { main: mut b, .. } = load(&pool, GAIN_ID, "together-modal");
    let Loaded { main: mut c, .. } = load(&pool, GAIN_ID, "together-modal");
    a.open_editor(parent()).expect("the editor opens");
    block(a_audio.as_mut(), &fault(P_FAULT, PUMP_WINDOW_LONG), &[]);
    std::thread::sleep(Duration::from_millis(300));
    // Held for the main loop: A's window is the one in its dialog.
    a.close_editor();
    // The neighbours: served now.
    let t = Instant::now();
    let mut out = ParamOut::new();
    b.flush(
        &[ParamChange {
            offset: 0,
            id: P_GAIN,
            value: -6.0,
        }],
        &mut out,
    );
    assert!(c.save_state().is_ok());
    assert!(
        t.elapsed() < Duration::from_millis(800),
        "a neighbour waited {:?}",
        t.elapsed()
    );
    arrived(b.as_mut(), P_GAIN, -6.0);
    // The one in its dialog: answered within its own limit, never a frozen interface.
    let t = Instant::now();
    let _ = a.load_state(&[]);
    assert!(t.elapsed() < Duration::from_millis(1_800), "{:?}", t.elapsed());
    // Eight seconds more: nothing is killed.
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(8) {
        for m in [&mut a, &mut b, &mut c] {
            assert!(crashed(m.as_mut()).is_none(), "a live helper was killed");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// A request queued behind a slow one is not judged until the helper starts it: a 3 s
/// save (within its 4 s limit) with a neighbour's text asked behind it kills nothing
/// (#3, #37).
#[test]
fn a_request_queued_behind_a_slow_one_is_not_late() {
    let _g = serial();
    let pool = limits(1_000, 1_000, 4_000);
    let Loaded {
        main: mut slow,
        audio: mut slow_audio,
    } = load(&pool, VST3_GAIN_ID, "together-queue");
    let Loaded { main: mut b, .. } = load(&pool, GAIN_ID, "together-queue");
    // VST3's fault 10 blocks its thread for 3 s in the next save, without pumping.
    block(slow_audio.as_mut(), &fault(V_FAULT, PUMP_SAVE), &[]);
    arrived(slow.as_mut(), V_FAULT, PUMP_SAVE);
    slow.checkpoint();
    let start = Instant::now();
    let mut asked = false;
    while start.elapsed() < Duration::from_secs(6) {
        if !asked && start.elapsed() > Duration::from_millis(900) {
            // Behind the save: a text the editor wants.
            let _ = b.param_text(P_GAIN, -3.0);
            asked = true;
        }
        assert!(crashed(slow.as_mut()).is_none(), "killed while within its limit");
        assert!(crashed(b.as_mut()).is_none());
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Two plug-ins of one helper both processing when one crashes with an access violation:
/// the crash handler names it alone — its neighbour, whose job was running too, is not
/// blamed (#19, #20, #38, #45).
#[test]
fn the_crash_handler_names_the_plugin_whose_thread_crashed() {
    let _g = serial();
    let pool = pool();
    let Loaded {
        main: mut a,
        audio: mut a_audio,
    } = load(&pool, GAIN_ID, "together-handler");
    let Loaded {
        main: mut b,
        audio: mut b_audio,
    } = load(&pool, GAIN_ID, "together-handler");
    // B is slow for one block (50 ms) on one thread while A crashes on another.
    let slow = std::thread::spawn(move || {
        block(b_audio.as_mut(), &fault(P_FAULT, SLOW_ONCE), &[]);
        b_audio
    });
    std::thread::sleep(Duration::from_millis(15));
    block(a_audio.as_mut(), &fault(P_FAULT, SEGV_PROCESS), &[]);
    let _b_audio = slow.join().unwrap();
    assert_eq!(
        crashed_within(a.as_mut(), Duration::from_secs(5)),
        Some(Blame::This)
    );
    assert_eq!(
        crashed_within(b.as_mut(), Duration::from_secs(5)),
        Some(Blame::Neighbour)
    );
}

/// A crash in a plug-in's editor (its window's timer, on the helper's main thread outside
/// any call) is laid on that plug-in, not on a neighbour that was processing (#1, #18,
/// #39).
#[cfg(windows)]
#[test]
fn a_crash_in_an_editor_is_laid_on_its_plugin() {
    let _g = serial();
    let pool = pool();
    let Loaded {
        main: mut a,
        audio: mut a_audio,
    } = load(&pool, GAIN_ID, "together-editor-crash");
    let Loaded {
        main: mut b,
        audio: mut b_audio,
    } = load(&pool, GAIN_ID, "together-editor-crash");
    a.open_editor(parent()).expect("the editor opens");
    block(a_audio.as_mut(), &fault(P_FAULT, SEGV_EDITOR), &[]);
    // B keeps processing meanwhile.
    let start = Instant::now();
    let mut blame_a = None;
    while start.elapsed() < Duration::from_secs(5) && blame_a.is_none() {
        block(b_audio.as_mut(), &[], &[]);
        blame_a = crashed(a.as_mut());
    }
    assert_eq!(blame_a, Some(Blame::This));
    assert_eq!(
        crashed_within(b.as_mut(), Duration::from_secs(5)),
        Some(Blame::Neighbour)
    );
}

/// What a late job's plug-in reported — a gesture's values and its end — reaches the host
/// when the job finishes (#12, #42, #46).
#[test]
fn a_late_jobs_events_reach_the_host() {
    let _g = serial();
    let pool = pool();
    let Loaded {
        main: _main,
        audio: mut s,
    } = load(&pool, SYNTH_ID, "own-late-events");
    let mut events = Vec::new();
    let mut take = |out: ParamOut| events.extend(out.events().iter().copied());
    // A gesture of five blocks: Begin, three values, End; the End's block is slow.
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
    for _ in 0..3 {
        take(block(s.as_mut(), &[], &[]));
    }
    let ends = events
        .iter()
        .filter(|e| matches!(e, ParamEvent::End(0)))
        .count();
    let begins = events
        .iter()
        .filter(|e| matches!(e, ParamEvent::Begin(0)))
        .count();
    assert_eq!((begins, ends), (1, 1), "{events:?}");
}

/// A dense controller stream while a plug-in is late does not push the resuming block's
/// note-off out: the note ends (#8, #9, #16, #23, #41).
#[test]
fn a_controller_flood_while_late_never_drops_the_resuming_note_off() {
    let _g = serial();
    let pool = pool();
    let Loaded {
        main: _main,
        audio: mut s,
    } = load(&pool, SYNTH_ID, "own-flood");
    let note = |kind| NoteEvent {
        offset: 0,
        note_id: 1,
        channel: 0,
        key: 60,
        kind,
    };
    block(s.as_mut(), &[], &[note(NoteKind::On { velocity: 0.9 })]);
    block(s.as_mut(), &fault(S_FAULT, SLOW_ONCE), &[]);
    // Dry blocks: 496 different controllers move (16 channels × 31 numbers) — as many
    // kinds as the carry holds, short of overflowing it.
    let kinds: Vec<(u8, u8)> = (0..16u8)
        .flat_map(|c| (1..32u8).map(move |n| (c, n)))
        .collect();
    for chunk in kinds.chunks(31) {
        let cc: Vec<NoteEvent> = chunk
            .iter()
            .enumerate()
            .map(|(i, (c, n))| NoteEvent {
                offset: i as u32,
                note_id: -1,
                channel: *c,
                key: 0,
                kind: NoteKind::Midi {
                    status: 0xB0,
                    data1: *n,
                    data2: 64,
                },
            })
            .collect();
        block(s.as_mut(), &[], &cc);
    }
    std::thread::sleep(Duration::from_millis(100));
    // The resuming block: twenty controllers of its own, then it lets the note go.
    let mut own: Vec<NoteEvent> = (0..20)
        .map(|i| NoteEvent {
            offset: i,
            note_id: -1,
            channel: 0,
            key: 0,
            kind: NoteKind::Midi {
                status: 0xB0,
                data1: 7,
                data2: i as u8,
            },
        })
        .collect();
    own.push(NoteEvent {
        offset: 100,
        ..note(NoteKind::Off { velocity: 0.0 })
    });
    block(s.as_mut(), &[], &own);
    let mut last = Vec::new();
    for _ in 0..750 {
        let (mut l, mut r) = (vec![0.0f32; 128], vec![0.0f32; 128]);
        call(s.as_mut(), &mut l, &mut r, &[], &[]);
        last = l;
    }
    assert_eq!(peak(&last), 0.0, "the note ended");
}

/// A neighbour's slow but healthy save does not make another plug-in's save answer with an
/// old state (#2, #24).
#[test]
fn a_slow_neighbour_does_not_make_a_save_stale() {
    let _g = serial();
    let pool = limits(5_000, 5_000, 5_000);
    let Loaded {
        main: mut slow,
        audio: mut slow_audio,
    } = load(&pool, VST3_GAIN_ID, "together-stale");
    let Loaded { main: mut b, .. } = load(&pool, GAIN_ID, "together-stale");
    let before = b.save_state().unwrap();
    block(slow_audio.as_mut(), &fault(V_FAULT, PUMP_SAVE), &[]);
    arrived(slow.as_mut(), V_FAULT, PUMP_SAVE);
    // The neighbour's save keeps the helper busy for 3 s.
    assert!(slow.save_state().is_ok());
    let mut out = ParamOut::new();
    b.flush(
        &[ParamChange {
            offset: 0,
            id: P_GAIN,
            value: -9.0,
        }],
        &mut out,
    );
    let after = b.save_state().unwrap();
    assert_ne!(after, before, "a fresh state, not the old one");
}

/// A helper that ends before its handshake (another build's, a missing library) makes a
/// load fail; it is not this plug-in's crash (#40).
#[test]
fn a_helper_that_cannot_start_is_a_load_failure() {
    let _g = serial();
    let exe = if cfg!(windows) {
        std::path::PathBuf::from(r"C:\Windows\System32\whoami.exe")
    } else {
        std::path::PathBuf::from("/bin/false")
    };
    let pool = Sandboxes::new(exe);
    let info = info(GAIN_ID);
    let r = bridged(&pool, info.format, "together", false).load(&info, setup());
    assert!(matches!(r, Err(HostError::Load(_))), "{:?}", r.err());
}

//! Round 31 (item 0): a helper's life is read where its main thread counts it, through
//! real helpers (ADR 0063) — never inferred from a heartbeat whose timer a virtual machine,
//! or a throttled background process, fires late.

mod support;

use cantus_host::bridge::sandbox::Limits;
use cantus_host::bridge::Sandboxes;
use cantus_host::{AudioThread, Backend, Loaded, MainThread, Notice};
use std::sync::Arc;
use std::time::{Duration, Instant};
use support::*;

/// How late the helper's timers fire in these tests: its heartbeat (every 200 ms) then
/// comes every 350 ms — past the 300 ms silence limit below — while its main loop still
/// goes round every 165 ms.
const LATE_MS: &str = "150";

fn pool_with_short_silence() -> Arc<Sandboxes> {
    let pool = pool();
    pool.set_limits(Limits {
        silence: Duration::from_millis(300),
        load: Duration::from_secs(10),
        state: Duration::from_secs(2),
        call: Duration::from_secs(2),
        hang: Duration::from_millis(500),
        offline: Duration::from_secs(2),
        heavy: Duration::from_secs(2),
    });
    pool
}

/// A gain loaded in a helper of its own whose timers fire late.
fn load_late(pool: &Arc<Sandboxes>, group: &str) -> Loaded {
    std::env::set_var("CANTUS_TEST_SANDBOX_LATE_TIMERS_MS", LATE_MS);
    let info = info(GAIN_ID);
    let loaded = bridged(pool, info.format, group, false)
        .load(&info, setup())
        .map(loaded);
    std::env::remove_var("CANTUS_TEST_SANDBOX_LATE_TIMERS_MS");
    loaded.expect("loads in the helper")
}

fn crashed(main: &mut dyn MainThread) -> Option<String> {
    main.idle().into_iter().find_map(|n| match n {
        Notice::Crashed { reason, .. } => Some(reason),
        _ => None,
    })
}

fn noisy(audio: &mut dyn AudioThread) -> Vec<f32> {
    let mut l = noise(128, 7);
    let mut r = l.clone();
    call(audio, &mut l, &mut r, &[], &[]);
    l
}

/// Its heartbeat late past the silence limit, its main thread alive: never taken for hung
/// (before round 31 it was ended as "stopped answering" — the macOS runner's pause test).
#[test]
fn a_helper_whose_timers_fire_late_is_not_taken_for_hung() {
    let _g = serial();
    let pool = pool_with_short_silence();
    let Loaded {
        main: mut m,
        audio: mut a,
    } = load_late(&pool, "own-late-timers");
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(2) {
        assert_eq!(crashed(m.as_mut()), None, "alive, its timers late");
        std::thread::sleep(Duration::from_millis(20));
    }
    noisy(a.as_mut());
    assert!(m.stopped().is_none());
}

/// A main thread that truly stops going round (a plug-in's call that never returns, not
/// pumping) is still found hung, by the same reading — a late timer changes nothing there.
#[test]
fn a_helper_whose_main_thread_stops_is_still_found_hung() {
    let _g = serial();
    let pool = pool_with_short_silence();
    let Loaded {
        main: mut m,
        audio: mut a,
    } = load_late(&pool, "own-late-hang");
    // Its save hangs, past the state limit (2 s) and the heavy one.
    call(
        a.as_mut(),
        &mut [0.0; 64],
        &mut [0.0; 64],
        &fault(P_FAULT, HANG_SAVE),
        &[],
    );
    m.ask_state_now();
    let start = Instant::now();
    let why = loop {
        if let Some(why) = crashed(m.as_mut()) {
            break why;
        }
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "a hung helper is found"
        );
        std::thread::sleep(Duration::from_millis(20));
    };
    assert!(why.contains("did not answer"), "{why}");
}

//! Review isolation-6, through real helpers and the whole host (round 29, ADR 0063): a slow
//! but healthy load or state is no hang (the waiter gives up at its own limit, the helper
//! lives — judged at the shipped ratio of silence to limit); a render's preparation is its
//! owner's and its content's; a state given while a load is on its way stands; a state
//! answer is never lost; another build's helper is a failed load; a dialog nothing names
//! shows every card of its helper; a slow load shows none; a plug-in let go parks its
//! state as it is now; a fresh insert's values never make a stray step nor lose a redo.

mod support;

use cantus_core::*;
use cantus_host::bridge::sandbox::Limits;
use cantus_host::{Backend, HostNotice, Loaded, Notice, RenderFor, RenderLoss};
use std::time::{Duration, Instant};
use support::*;

fn patient_host() -> cantus_host::PluginHost {
    let host = host();
    if let Some(p) = host.sandboxes() {
        p.set_deadline_floor(Duration::from_secs(5));
    }
    host
}

/// The shipped limits' ratio, a tenth as long: silence 5 s, load and state 30 s, heavy 120 s
/// become 0.4 s (the heartbeat's 0.2 s fits twice), 2.4 s and 9.6 s.
fn shipped_ratio() -> Limits {
    Limits {
        silence: Duration::from_millis(400),
        load: Duration::from_millis(2_400),
        state: Duration::from_millis(2_400),
        heavy: Duration::from_millis(9_600),
        ..Limits::default()
    }
}

fn frames_until(rig: &mut Rig, limit: Duration, done: impl Fn(&Rig) -> bool) -> bool {
    let start = Instant::now();
    while !done(rig) {
        if start.elapsed() > limit {
            return false;
        }
        rig.frame();
        std::thread::sleep(Duration::from_millis(5));
    }
    true
}

/// Inserts `id` as the menu does, without waiting for its load.
fn insert_now(rig: &mut Rig, strip: StripId, id: &str, iso: PluginIsolation) -> EffectId {
    let slot = rig
        .host
        .make_slot_isolated(&mut rig.bus, strip, &info(id), Some(iso))
        .expect("asked");
    let effect = slot.id;
    let index = rig
        .bus
        .project()
        .strip(strip)
        .map_or(0, |s| s.effects.len());
    rig.bus
        .execute_merging(
            Command::InsertEffect {
                strip,
                index,
                slot: Box::new(slot),
            },
            Origin::User,
            cantus_host::PluginHost::load_key(effect),
        )
        .expect("inserted");
    rig.sync();
    effect
}

/// A load that reads for 4 s without pumping, at the shipped ratio of silence to limit: a
/// failed load at its limit (Reload), never a hang — its helper and the neighbour in it live,
/// nothing is counted (review isolation-6 #0, #12; isolation-4 #66).
#[test]
fn a_slow_silent_load_fails_at_its_limit_and_its_helper_lives() {
    let _g = serial();
    let dir = tempfile::tempdir().expect("a folder");
    std::env::set_var(
        "CANTUS_TEST_GAIN_SLOW_ACTIVATE_ONCE",
        dir.path().join("slow"),
    );
    std::env::set_var("CANTUS_TEST_GAIN_SLOW_MS", "4000");
    let project = demo_project();
    let strip = strip_of(&project, 0);
    let host = patient_host();
    if let Some(pool) = host.sandboxes() {
        pool.set_limits(shipped_ratio());
    }
    let mut rig = Rig::new(project, host);
    // The neighbour, another library in the same helper (started with the slow start set).
    let neighbour = rig.insert(strip, VST3_GAIN_ID, PluginIsolation::Together);
    let slow = insert_now(&mut rig, strip, GAIN_ID, PluginIsolation::Together);
    std::env::remove_var("CANTUS_TEST_GAIN_SLOW_ACTIVATE_ONCE");
    std::env::remove_var("CANTUS_TEST_GAIN_SLOW_MS");
    assert!(
        frames_until(&mut rig, Duration::from_secs(6), |r| r
            .host
            .load_error(slow)
            .is_some()),
        "a failed load at its limit"
    );
    // Past the slow start: nothing was ended.
    frames_until(&mut rig, Duration::from_secs(3), |_| false);
    assert!(rig.host.crashed(slow).is_none(), "not a crash");
    assert!(rig.host.crashed(neighbour).is_none(), "its neighbour lives");
    assert!(rig.host.hosted(neighbour).is_some());
    assert_eq!(
        rig.host
            .crashes_this_session(cantus_host::Format::Clap, GAIN_ID),
        0
    );
    assert!(rig
        .notices
        .iter()
        .all(|n| !matches!(n, HostNotice::PluginCrashed { .. })));
}

/// A slow state (3 s, not pumping), past its limit at the shipped ratio: given up at its
/// limit, said as what it said last; its helper lives, and its late answer is kept (review
/// isolation-6 #12).
#[test]
fn a_slow_silent_state_past_its_limit_is_not_a_hang() {
    let _g = serial();
    let pool = pool();
    pool.set_limits(shipped_ratio());
    let info = info(GAIN_ID);
    let Loaded {
        mut main,
        mut audio,
    } = bridged(&pool, info.format, "own-slow-state", false)
        .load(&info, setup())
        .map(loaded)
        .expect("loads");
    let mut out = cantus_host::ParamOut::new();
    let (mut l, mut r) = (vec![0.0f32; 64], vec![0.0f32; 64]);
    audio.process(
        cantus_host::Audio {
            left: &mut l,
            right: &mut r,
            key: None,
            transport: None,
        },
        &fault(P_FAULT, SLOW_SAVE),
        &[],
        &mut out,
    );
    let start = Instant::now();
    while main.param_value(P_FAULT) != Some(SLOW_SAVE) {
        assert!(start.elapsed() < Duration::from_secs(5));
        main.idle();
        std::thread::sleep(Duration::from_millis(5));
    }
    let t = Instant::now();
    match fresh_state(main.as_mut()) {
        cantus_host::StatePoll::Stale(_, why) => assert!(why.contains("in time"), "{why}"),
        other => panic!("given up at its limit: {other:?}"),
    }
    assert!(
        t.elapsed() < Duration::from_millis(2_900),
        "{:?}",
        t.elapsed()
    );
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(4) {
        assert!(
            !main
                .idle()
                .iter()
                .any(|n| matches!(n, Notice::Crashed { .. })),
            "a slow state is not a hang"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(matches!(
        fresh_state(main.as_mut()),
        cantus_host::StatePoll::Fresh(_)
    ));
}

/// A render's preparation is its owner's and its content's: asked for other values it is
/// made anew; another owner waits while it is in use and takes its turn once it is given up —
/// never taking it from each other frame after frame (review isolation-6 #1, #4, #13, #15,
/// #26).
#[test]
fn a_preparation_is_its_owners_and_its_contents() {
    let _g = serial();
    let project = demo_project();
    let strip = strip_of(&project, 0);
    let mut rig = Rig::new(project, patient_host());
    let id = rig.insert(strip, GAIN_ID, PluginIsolation::Together);
    rig.settle();
    let ready = |rig: &mut Rig, owner: RenderFor, stored: bool| {
        let p = rig.bus.project().clone();
        rig.host.render_ready(owner, &p, SR, stored)
    };
    let start = Instant::now();
    while !ready(&mut rig, RenderFor::Freeze, true) {
        assert!(start.elapsed() < Duration::from_secs(20));
        rig.frame();
        std::thread::sleep(Duration::from_millis(5));
    }
    // A value moved: the same owner's render of the new content is a new preparation.
    rig.set_param(strip, id, P_GAIN, -9.0);
    assert!(
        !ready(&mut rig, RenderFor::Freeze, true),
        "made anew for its new content"
    );
    // Another owner waits while the freeze's is in use — however often it asks.
    for _ in 0..20 {
        assert!(
            !ready(&mut rig, RenderFor::Bake, false),
            "it waits its turn"
        );
        ready(&mut rig, RenderFor::Freeze, true);
        rig.frame();
        std::thread::sleep(Duration::from_millis(5));
    }
    let start = Instant::now();
    while !ready(&mut rig, RenderFor::Freeze, true) {
        assert!(start.elapsed() < Duration::from_secs(20));
        assert!(!ready(&mut rig, RenderFor::Bake, false));
        rig.frame();
        std::thread::sleep(Duration::from_millis(5));
    }
    // Given up: the bake's turn.
    rig.host.drop_render(RenderFor::Freeze);
    let start = Instant::now();
    while !ready(&mut rig, RenderFor::Bake, false) {
        assert!(start.elapsed() < Duration::from_secs(20), "its turn came");
        rig.frame();
        std::thread::sleep(Duration::from_millis(5));
    }
    rig.host.finish_render();
}

/// A render built while a plug-in's state is still on its way lacks it — and says so,
/// never silently (review isolation-6 #2, #11, #22).
#[test]
fn a_render_without_its_instance_says_so() {
    let _g = serial();
    let project = demo_project();
    let strip = strip_of(&project, 0);
    let mut rig = Rig::new(project, patient_host());
    let id = rig.insert(strip, GAIN_ID, PluginIsolation::Together);
    rig.set_param(strip, id, P_FAULT, SLOW_SAVE);
    rig.settle();
    let p = rig.bus.project().clone();
    assert!(
        !rig.host.render_ready(RenderFor::Export, &p, SR, false),
        "its state is asked for (3 s)"
    );
    let units = rig.host.render_units(&p, SR);
    let failures = rig.host.render_failures();
    rig.host.finish_render();
    assert!(units.is_empty());
    assert_eq!(failures.len(), 1, "{failures:?}");
    assert_eq!(failures[0].kind, RenderLoss::NeverLoaded);
}

/// A state given while a load is on its way is newer than the one it loads with, and
/// stands — what is saved, parked and recorded (review isolation-6 #5, #19, #29).
#[test]
fn a_state_given_while_it_loads_stands() {
    let _g = serial();
    let pool = pool();
    let info = info(GAIN_ID);
    let load = |group: &str| {
        bridged(&pool, info.format, group, false)
            .load(&info, setup())
            .expect("asked")
    };
    // The state to give: a gain of -9 dB.
    let mut other = loaded(load("own-other-state"));
    let mut out = cantus_host::ParamOut::new();
    other.main.flush(
        &[cantus_host::ParamChange {
            offset: 0,
            id: P_GAIN,
            value: -9.0,
        }],
        &mut out,
    );
    let start = Instant::now();
    while other.main.param_value(P_GAIN) != Some(-9.0) {
        assert!(start.elapsed() < Duration::from_secs(5));
        other.main.idle();
        std::thread::sleep(Duration::from_millis(5));
    }
    let given = state_now(other.main.as_mut());
    let mut l = load("own-given-while-loading");
    l.main.load_state(&given).expect("sent");
    let mut l = loaded(l);
    assert_eq!(
        l.main.save_state().expect("a state"),
        given,
        "the state given, not the one it loaded with"
    );
}

/// A save's state answer that came in just before a state is given is still taken: the
/// save ends (review isolation-6 #18, #23).
#[test]
fn a_state_answer_is_never_lost() {
    let _g = serial();
    let pool = pool();
    let info = info(GAIN_ID);
    let Loaded { mut main, .. } = bridged(&pool, info.format, "own-answer-kept", false)
        .load(&info, setup())
        .map(loaded)
        .expect("loads");
    let given = state_now(main.as_mut());
    main.ask_state_now();
    // Its answer lands; nothing takes it before a state is given.
    std::thread::sleep(Duration::from_millis(500));
    main.load_state(&given).expect("sent");
    let start = Instant::now();
    while main.state_waiting() {
        assert!(
            start.elapsed() < Duration::from_secs(3),
            "the save waited for an answer already in"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// A helper of another build (it greets with another protocol) is a failed load, nobody's
/// crash (review isolation-6 #9).
#[test]
fn another_builds_helper_is_a_failed_load() {
    let _g = serial();
    std::env::set_var("CANTUS_SANDBOX_SPEAKS_PROTOCOL", "5");
    let pool = pool();
    let info = info(GAIN_ID);
    let r = bridged(&pool, info.format, "own-other-build", false).load(&info, setup());
    std::env::remove_var("CANTUS_SANDBOX_SPEAKS_PROTOCOL");
    let mut l = r.expect("asked");
    let start = Instant::now();
    let mut said = None;
    while said.is_none() && start.elapsed() < Duration::from_secs(10) {
        for n in l.main.idle() {
            match n {
                Notice::LoadFailed(why) => said = Some(Ok(why)),
                Notice::Crashed { reason, .. } => said = Some(Err(reason)),
                _ => {}
            }
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    match said {
        Some(Ok(why)) => assert!(why.contains("protocol"), "{why}"),
        other => panic!("a failed load, not a crash: {other:?}"),
    }
}

/// A dialog nothing names — a timer of a library two instances share opens it, outside any
/// call — holds its helper: every plug-in of the helper shows the card, never none (review
/// isolation-6 #14, #27).
#[cfg(windows)]
#[test]
fn a_dialog_nothing_names_shows_every_card_of_its_helper() {
    let _g = serial();
    let dir = tempfile::tempdir().expect("a folder");
    std::env::set_var(
        "CANTUS_TEST_GAIN_PUMP_ON_TIMER_ONCE",
        dir.path().join("timer"),
    );
    let pool = pool();
    let info = info(GAIN_ID);
    let r = bridged(&pool, info.format, "together-unnamed", false).load(&info, setup());
    std::env::remove_var("CANTUS_TEST_GAIN_PUMP_ON_TIMER_ONCE");
    let mut a = loaded(r.expect("asked"));
    let mut b = bridged(&pool, info.format, "together-unnamed", false)
        .load(&info, setup())
        .expect("asked");
    let start = Instant::now();
    let mut both = false;
    while !both && start.elapsed() < Duration::from_millis(3_400) {
        let _ = a.main.idle();
        let _ = b.main.idle();
        both = a.main.waiting() && b.main.waiting();
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(both, "each plug-in of the helper shows the card");
}

/// A healthy load that takes its time (3 s, not pumping) shows no waiting card — its slot
/// says it is loading — and loads (review isolation-6 #28).
#[test]
fn a_slow_load_shows_no_card_and_says_loading() {
    let _g = serial();
    let dir = tempfile::tempdir().expect("a folder");
    std::env::set_var(
        "CANTUS_TEST_GAIN_SLOW_ACTIVATE_ONCE",
        dir.path().join("slow"),
    );
    std::env::set_var("CANTUS_TEST_GAIN_SLOW_MS", "3000");
    let project = demo_project();
    let strip = strip_of(&project, 0);
    let mut rig = Rig::new(project, patient_host());
    let id = insert_now(&mut rig, strip, GAIN_ID, PluginIsolation::Individually);
    std::env::remove_var("CANTUS_TEST_GAIN_SLOW_ACTIVATE_ONCE");
    std::env::remove_var("CANTUS_TEST_GAIN_SLOW_MS");
    let start = Instant::now();
    while start.elapsed() < Duration::from_millis(2_700) {
        assert!(rig.host.waiting().0.is_empty(), "no card for a slow load");
        assert!(rig.host.effect_loading(id), "its slot says loading");
        rig.frame();
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(frames_until(&mut rig, Duration::from_secs(10), |r| !r
        .host
        .effect_loading(id)));
    assert!(rig.host.hosted(id).is_some_and(|h| !h.params.is_empty()));
}

/// A plug-in let go — its slot removed — parks its state as it is now; one whose state
/// cannot be had in time parks what it said last and says so; a device change loads every
/// plug-in again once its state is in (review isolation-6 #24).
#[test]
fn a_plugin_let_go_parks_its_state_as_it_is_now_or_says_so() {
    let _g = serial();
    let project = demo_project();
    let strip = strip_of(&project, 0);
    let host = patient_host();
    if let Some(pool) = host.sandboxes() {
        pool.set_limits(Limits {
            state: Duration::from_millis(500),
            heavy: Duration::from_millis(500),
            silence: Duration::from_millis(400),
            ..pool.limits()
        });
    }
    let mut rig = Rig::new(project, host);
    let id = rig.insert(strip, GAIN_ID, PluginIsolation::Individually);
    rig.settle();
    // A device change: let go, its state asked for, loaded again once it is in.
    rig.host.reload_all(SR);
    assert!(rig.host.hosted(id).is_none());
    let start = Instant::now();
    while rig.host.hosted(id).is_none() {
        assert!(start.elapsed() < Duration::from_secs(10), "loaded again");
        rig.frame();
        if rig.host.take_resync() {
            rig.resync();
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    rig.until_loaded();
    // Removed while its save hangs: what it said last is parked, and said.
    rig.set_param(strip, id, P_FAULT, HANG_SAVE);
    rig.settle();
    rig.exec(Command::RemoveEffect { strip, effect: id });
    let said = frames_until(&mut rig, Duration::from_secs(5), |r| {
        r.notices
            .iter()
            .any(|n| matches!(n, HostNotice::StateKept { .. }))
    });
    assert!(said, "{:?}", rig.notices);
}

/// A fresh insert's values join its step only while it is on top and nothing was undone
/// since: an edit undone meanwhile keeps its redo, and no stray step is made (review
/// isolation-6 #7, #17, #30).
#[test]
fn a_fresh_inserts_values_never_make_a_stray_step_nor_lose_a_redo() {
    let _g = serial();
    let dir = tempfile::tempdir().expect("a folder");
    std::env::set_var(
        "CANTUS_TEST_GAIN_SLOW_ACTIVATE_ONCE",
        dir.path().join("slow"),
    );
    let project = demo_project();
    let strip = strip_of(&project, 0);
    let mut rig = Rig::new(project, patient_host());
    let id = insert_now(&mut rig, strip, GAIN_ID, PluginIsolation::Individually);
    std::env::remove_var("CANTUS_TEST_GAIN_SLOW_ACTIVATE_ONCE");
    rig.exec(Command::SetEffectBypass {
        strip,
        effect: id,
        bypass: true,
    });
    rig.bus.undo().expect("undone");
    rig.resync();
    rig.until_loaded();
    assert!(rig.bus.can_redo(), "the bypass's redo survived the load");
    assert_eq!(rig.bus.history().len(), 1, "the insert alone");
    // On top, nothing undone: its values join the insert's step.
    let other = rig.insert(strip, GAIN_ID, PluginIsolation::Individually);
    assert_eq!(rig.bus.history().len(), 2, "one step for the second insert");
    assert!(!rig.plugin(other).params.is_empty(), "its values recorded");
}

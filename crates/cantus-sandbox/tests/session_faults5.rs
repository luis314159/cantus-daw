//! Review isolation-5's four rules, through the whole host (round 29, ADR 0063): the
//! interface never waits on a helper — a load, a save's states, a reload, a move, a render's
//! plug-ins and a window are asked for and looked at frame by frame; a save names a plug-in
//! it kept as it was last said; nothing is ended under a plug-in's own code; a load past
//! its limit is a failed load, never retried.

mod support;

use cantus_core::*;
use cantus_engine::{render_with_instruments, RenderOptions};
use cantus_host::bridge::sandbox::Limits;
use cantus_host::{Backend, HostNotice, Loaded, RenderLoss};
use std::time::{Duration, Instant};
use support::*;

/// A host whose real-time blocks wait as long as it takes.
fn patient_host() -> cantus_host::PluginHost {
    let host = host();
    if let Some(p) = host.sandboxes() {
        p.set_deadline_floor(Duration::from_secs(5));
    }
    host
}

/// The longest `f` took, over its calls.
struct Worst(Duration, &'static str);

impl Worst {
    fn time<R>(&mut self, what: &'static str, f: impl FnOnce() -> R) -> R {
        let t = Instant::now();
        let r = f();
        if t.elapsed() > self.0 {
            self.0 = t.elapsed();
            self.1 = what;
        }
        r
    }
}

/// Rule 1: a plug-in slow to start (1.5 s, not pumping) is inserted, loaded, saved, moved,
/// reloaded and rendered, and nothing the interface does waits on its helper: every call a
/// frame makes returns within a frame's share (50 ms), while the work goes on in the helper.
#[test]
fn nothing_the_interface_does_waits_on_a_helper() {
    let _g = serial();
    let project = demo_project();
    let strip = strip_of(&project, 0);
    let mut rig = Rig::new(project, patient_host());
    let mut w = Worst(Duration::ZERO, "");
    let dir = tempfile::tempdir().expect("a folder");
    let slow =
        |n: &str| std::env::set_var("CANTUS_TEST_GAIN_SLOW_ACTIVATE_ONCE", dir.path().join(n));
    // Inserted: the slot at once, its plug-in loading in its helper.
    slow("insert");
    let info = info(GAIN_ID);
    let slot = w.time("insert", || {
        rig.host
            .make_slot_isolated(&mut rig.bus, strip, &info, Some(PluginIsolation::Together))
            .expect("asked")
    });
    let id = slot.id;
    rig.bus
        .execute_merging(
            Command::InsertEffect {
                strip,
                index: 0,
                slot: Box::new(slot),
            },
            Origin::User,
            cantus_host::PluginHost::load_key(id),
        )
        .unwrap();
    w.time("sync", || rig.sync());
    // A value set before it has loaded: sent after its load, and it stands.
    w.time("set", || rig.set_param(strip, id, P_GAIN, -9.0));
    let frames = |rig: &mut Rig, w: &mut Worst, until: &dyn Fn(&mut Rig) -> bool| {
        let start = Instant::now();
        while !until(rig) {
            assert!(start.elapsed() < Duration::from_secs(20), "in time");
            w.time("frame", || rig.frame());
            std::thread::sleep(Duration::from_millis(5));
        }
    };
    assert!(rig.host.loading(), "loading in its helper");
    frames(&mut rig, &mut w, &|r| !r.host.loading());
    std::env::remove_var("CANTUS_TEST_GAIN_SLOW_ACTIVATE_ONCE");
    assert!(rig.host.hosted(id).is_some_and(|h| !h.params.is_empty()));
    // The value set meanwhile stands, in its own step; the insert is no longer on top, so
    // its values are not recorded after it — no stray step (review isolation-6 #17).
    assert_eq!(rig.bus.history().len(), 2, "the insert and the value set");
    assert_eq!(
        rig.plugin(id).params.get(&P_GAIN),
        Some(&-9.0),
        "the value set stands"
    );
    frames(&mut rig, &mut w, &|r| {
        r.host.hosted(id).and_then(|h| h.param_value(P_GAIN)) == Some(-9.0)
    });
    // Saved: asked, looked at, taken.
    w.time("ask states", || rig.host.ask_states());
    // `states_in` is what a save looks at each frame.
    frames(&mut rig, &mut w, &|r| r.host.states_in());
    let saved = w.time("save", || rig.host.project_for_save(rig.bus.project()));
    assert!(rig.host.saved_stale().is_empty());
    assert!(saved.all_effects().any(|(_, e)| e.id == id
        && e.spec
            .external
            .as_ref()
            .is_some_and(|p| !p.state.is_empty())));
    // Moved to a helper of its own, slow to start again.
    slow("move");
    rig.bus
        .execute(
            Command::SetPluginIsolation {
                strip,
                effect: id,
                isolation: Some(PluginIsolation::Individually),
            },
            Origin::User,
        )
        .unwrap();
    w.time("sync", || rig.sync());
    frames(&mut rig, &mut w, &|r| {
        !r.host.loading() && r.host.isolation(id) == Some(PluginIsolation::Individually)
    });
    // Reloaded, slow to start again.
    slow("reload");
    w.time("reload", || {
        rig.host
            .reload(rig.bus.project(), &mut rig.engine, id)
            .expect("asked")
    });
    frames(&mut rig, &mut w, &|r| !r.host.loading());
    // Rendered: its render instance loads in a helper of its own, slow to start too.
    slow("render");
    let start = Instant::now();
    while !w.time("render ready", || {
        rig.host
            .render_ready(cantus_host::RenderFor::Export, rig.bus.project(), SR, false)
    }) {
        assert!(start.elapsed() < Duration::from_secs(20), "in time");
        w.time("frame", || rig.frame());
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(
        start.elapsed() > Duration::from_millis(1_400),
        "it waited for the slow start, frame by frame: {:?}",
        start.elapsed()
    );
    std::env::remove_var("CANTUS_TEST_GAIN_SLOW_ACTIVATE_ONCE");
    let units = w.time("units", || rig.host.render_units(rig.bus.project(), SR));
    assert_eq!(units.len(), 1);
    assert!(rig.host.render_failures().is_empty());
    rig.host.finish_render();
    assert!(w.0 < Duration::from_millis(50), "{} took {:?}", w.1, w.0);
}

/// Rule 2: a save whose plug-in cannot give its state in time (its save hangs, past the
/// state limit from its start) is written with what it said last — and says which plug-in,
/// and why; a neighbour in another helper is saved as it is now and not named.
#[test]
fn a_save_names_a_plugin_kept_as_it_was_last_said() {
    let _g = serial();
    let project = demo_project();
    let strip = strip_of(&project, 0);
    let mut rig = Rig::new(project, patient_host());
    if let Some(pool) = rig.host.sandboxes() {
        pool.set_limits(Limits {
            silence: Duration::from_secs(30),
            state: Duration::from_millis(500),
            ..pool.limits()
        });
    }
    let stuck = rig.insert(strip, GAIN_ID, PluginIsolation::Individually);
    let well = rig.insert(strip, VST3_GAIN_ID, PluginIsolation::Individually);
    rig.set_param(strip, stuck, P_GAIN, -6.0);
    rig.settle();
    rig.host.ask_states();
    let start = Instant::now();
    while !rig.host.states_in() {
        assert!(start.elapsed() < Duration::from_secs(10));
        rig.frame();
        std::thread::sleep(Duration::from_millis(5));
    }
    let good = rig.host.project_for_save(rig.bus.project());
    assert!(rig.host.saved_stale().is_empty(), "both answered");
    rig.set_param(strip, stuck, P_FAULT, HANG_SAVE);
    rig.settle();
    rig.host.ask_states();
    let start = Instant::now();
    while !rig.host.states_in() {
        assert!(start.elapsed() < Duration::from_secs(10));
        rig.frame();
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(
        start.elapsed() < Duration::from_secs(3),
        "past its limit from its start: {:?}",
        start.elapsed()
    );
    let saved = rig.host.project_for_save(rig.bus.project());
    let stale = rig.host.saved_stale().to_vec();
    assert_eq!(stale.len(), 1, "{stale:?}");
    assert_eq!(stale[0].0, info(GAIN_ID).name, "the stuck one named");
    assert!(stale[0].1.contains("did not answer"), "{stale:?}");
    let state_of = |p: &Project, e: EffectId| {
        p.all_effects()
            .find(|(_, s)| s.id == e)
            .and_then(|(_, s)| s.spec.external.as_ref().map(|x| x.state.clone()))
    };
    assert_eq!(
        state_of(&saved, stuck),
        state_of(&good, stuck),
        "what it said last"
    );
    assert!(state_of(&saved, well).is_some());
}

/// Rule 3: an instance removed while a neighbour's dialog runs its loop is not destroyed
/// under that loop — its unload waits for the helper's main loop — and is destroyed after
/// it (review isolation-5 #1, #40, #50, #69).
#[cfg(windows)]
#[test]
fn an_instance_removed_during_a_dialog_is_ended_after_it() {
    let _g = serial();
    let dir = tempfile::tempdir().expect("a folder");
    std::env::set_var("CANTUS_TEST_GAIN_DESTROY_MARKER", dir.path());
    let pool = pool();
    let load = |id: &str| {
        let info = info(id);
        bridged(&pool, info.format, "together-destroy-later", false)
            .load(&info, setup())
            .map(loaded)
            .expect("loads")
    };
    let Loaded {
        main: mut a,
        audio: mut a_audio,
    } = load(SYNTH_ID);
    let Loaded {
        main: b,
        audio: b_audio,
    } = load(GAIN_ID);
    std::env::remove_var("CANTUS_TEST_GAIN_DESTROY_MARKER");
    let parent = raw_window_handle::RawWindowHandle::Win32(
        raw_window_handle::Win32WindowHandle::new(std::num::NonZeroIsize::new(1).unwrap()),
    );
    open_window(a.as_mut(), parent).expect("A's editor");
    let (mut l, mut r) = (vec![0.0f32; 64], vec![0.0f32; 64]);
    call(
        a_audio.as_mut(),
        &mut l,
        &mut r,
        &fault(S_FAULT, PUMP_WINDOW),
        &[],
    );
    std::thread::sleep(Duration::from_millis(300));
    // B leaves while A's dialog runs (3 s).
    drop(b_audio);
    drop(b);
    let destroyed = || {
        std::fs::read_dir(dir.path())
            .map(|d| d.count())
            .unwrap_or(0)
    };
    std::thread::sleep(Duration::from_millis(1_500));
    assert_eq!(destroyed(), 0, "not destroyed under A's dialog");
    let start = Instant::now();
    while destroyed() == 0 {
        assert!(
            start.elapsed() < Duration::from_secs(6),
            "destroyed after the dialog"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(a.stopped().is_none(), "nothing ended");
}

/// A render waits for its plug-ins frame by frame; a heard plug-in that is not loaded live
/// (it could not be loaded) is a loss of the render, said — never dropped silently (review
/// isolation-5 #27).
#[test]
fn a_render_says_a_plugin_that_was_not_loaded() {
    let _g = serial();
    let project = demo_project();
    let strip = strip_of(&project, 0);
    let mut rig = Rig::new(project, patient_host());
    let a = rig.insert(strip, GAIN_ID, PluginIsolation::Together);
    let b = rig.insert(strip, GAIN_ID, PluginIsolation::Together);
    rig.settle();
    assert_eq!(
        rig.host.neighbours(Some(a), None),
        1,
        "one shares its process"
    );
    rig.host
        .mark_missing_for_tests(b, "it did not load within 30 s");
    rig.prepare_render(false);
    let units = rig.host.render_units(rig.bus.project(), SR);
    let insts = rig.host.render_instruments(rig.bus.project(), SR);
    assert_eq!(units.len(), 1, "the loaded one");
    let snapshot = std::sync::Arc::new(Snapshot::compile(rig.bus.project(), 1));
    let opts = RenderOptions {
        block_frames: 512,
        tail_seconds: 0.0,
        ..Default::default()
    };
    let _ = render_with_instruments(snapshot, &opts, units, insts);
    let failures = rig.host.render_failures();
    rig.host.finish_render();
    assert_eq!(failures.len(), 1, "{failures:?}");
    assert_eq!(failures[0].kind, RenderLoss::NeverLoaded);
    assert!(rig
        .notices
        .iter()
        .all(|n| !matches!(n, HostNotice::PluginCrashed { .. })));
}

/// *Stop it* ends a plug-in's helper at the user's word: never counted as its crash, nor
/// remembered; the plug-in stopped is told so, the ones that shared its process that they
/// went with it (review isolation-5 #28, #44, #67).
#[test]
fn stopping_a_plugin_is_never_its_crash() {
    let _g = serial();
    let project = demo_project();
    let strip = strip_of(&project, 0);
    let mut rig = Rig::new(project, patient_host());
    let a = rig.insert(strip, GAIN_ID, PluginIsolation::Together);
    let b = rig.insert(strip, GAIN_ID, PluginIsolation::Together);
    rig.settle();
    assert_eq!(
        rig.host.neighbours(Some(a), None),
        1,
        "the card says one goes with it"
    );
    rig.host.stop(a);
    let start = Instant::now();
    while (rig.host.crashed(a).is_none() || rig.host.crashed(b).is_none())
        && start.elapsed() < Duration::from_secs(5)
    {
        rig.frame();
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(rig.host.crashed(a), Some("it was stopped by you"));
    let why_b = rig.host.crashed(b).unwrap_or_default().to_string();
    assert!(why_b.contains("was ended to stop"), "{why_b}");
    assert!(!rig.host.crash_culprit(a) && !rig.host.crash_culprit(b));
    assert_eq!(
        rig.host
            .crashes_this_session(cantus_host::Format::Clap, GAIN_ID),
        0
    );
    assert!(rig
        .host
        .db
        .crash_record(cantus_host::Format::Clap, GAIN_ID)
        .is_none());
}

/// A state not had within its limit (a slow save, not pumping) is saved as it was last
/// said — and its late answer, when it comes, is the last-good state, current: the next
/// save takes it without asking anew (review isolation-5 #38).
#[test]
fn a_state_answered_late_becomes_the_last_good_one() {
    let _g = serial();
    let project = demo_project();
    let strip = strip_of(&project, 0);
    let mut rig = Rig::new(project, patient_host());
    if let Some(pool) = rig.host.sandboxes() {
        pool.set_limits(Limits {
            silence: Duration::from_secs(30),
            state: Duration::from_millis(500),
            ..pool.limits()
        });
    }
    let id = rig.insert(strip, GAIN_ID, PluginIsolation::Individually);
    rig.set_param(strip, id, P_GAIN, -6.0);
    rig.set_param(strip, id, P_FAULT, SLOW_SAVE);
    rig.settle();
    let gain_in = |p: &Project| {
        p.all_effects()
            .find(|(_, s)| s.id == id)
            .and_then(|(_, s)| s.spec.external.as_ref())
            .map(|x| {
                let b = x.state.bytes();
                f64::from_le_bytes(b[4..12].try_into().unwrap())
            })
    };
    rig.host.ask_states();
    let start = Instant::now();
    while !rig.host.states_in() {
        assert!(start.elapsed() < Duration::from_secs(10));
        rig.frame();
        std::thread::sleep(Duration::from_millis(5));
    }
    rig.host.project_for_save(rig.bus.project());
    assert_eq!(rig.host.saved_stale().len(), 1, "kept as it was last said");
    // Its slow answer comes (3 s): the last-good state, as it is now.
    let start = Instant::now();
    while start.elapsed() < Duration::from_millis(3_500) {
        rig.frame();
        std::thread::sleep(Duration::from_millis(10));
    }
    let saved = rig.host.project_for_save(rig.bus.project());
    assert!(
        rig.host.saved_stale().is_empty(),
        "{:?}",
        rig.host.saved_stale()
    );
    assert_eq!(gain_in(&saved), Some(-6.0));
}

/// Cantus quitting gives its helpers a moment to end their plug-ins by themselves — their
/// destructors run (they write preferences, release licences) — before they are ended
/// (review isolation-5 #24, #54).
#[test]
fn quitting_lets_the_helpers_end_their_plugins() {
    let _g = serial();
    let dir = tempfile::tempdir().expect("a folder");
    std::env::set_var("CANTUS_TEST_GAIN_DESTROY_MARKER", dir.path());
    let pool = pool();
    let info = info(GAIN_ID);
    let Loaded { main, audio } = bridged(&pool, info.format, "own-quit", false)
        .load(&info, setup())
        .map(loaded)
        .expect("loads");
    std::env::remove_var("CANTUS_TEST_GAIN_DESTROY_MARKER");
    // The session ends: its plug-in let go, then the helpers' pool (Cantus quitting).
    drop(audio);
    drop(main);
    drop(pool);
    let destroyed = std::fs::read_dir(dir.path())
        .map(|d| d.count())
        .unwrap_or(0);
    assert_eq!(destroyed, 1, "its destructor ran before Cantus was gone");
}

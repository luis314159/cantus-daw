//! The third review of the fix round through the host and engine (round 29,
//! isolation-4): a load given up on while its helper was busy is tried again; an end
//! nothing lays on one plug-in is counted for none and offers none a process of its own; a
//! reload that fails says why now and whose; a plug-in that crashed as it left is counted;
//! a render waits for a state that takes its time, and one that cannot load a plug-in —
//! its state refused, or its process crashing — says so, and how.

mod support;

use cantus_core::*;
use cantus_engine::{render_with_instruments, RenderOptions};
use cantus_host::bridge::sandbox::Limits;
use cantus_host::{Format, HostNotice, RenderLoss};
use std::time::{Duration, Instant};
use support::*;

fn patient_host() -> cantus_host::PluginHost {
    let host = host();
    if let Some(p) = host.sandboxes() {
        p.set_deadline_floor(Duration::from_secs(5));
    }
    host
}

fn frames_until(rig: &mut Rig, limit: Duration, done: impl Fn(&Rig) -> bool) {
    let start = Instant::now();
    while !done(rig) && start.elapsed() < limit {
        rig.frame();
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn render(rig: &mut Rig, stored: bool) -> Vec<f32> {
    rig.prepare_render(stored);
    let (units, insts) = if stored {
        (
            rig.host.render_units_stored(rig.bus.project(), SR),
            rig.host.render_instruments_stored(rig.bus.project(), SR),
        )
    } else {
        (
            rig.host.render_units(rig.bus.project(), SR),
            rig.host.render_instruments(rig.bus.project(), SR),
        )
    };
    let snapshot = std::sync::Arc::new(Snapshot::compile(rig.bus.project(), 1));
    let opts = RenderOptions {
        block_frames: 512,
        tail_seconds: 0.0,
        ..Default::default()
    };
    render_with_instruments(snapshot, &opts, units, insts)
        .unwrap()
        .samples
}

/// A plug-in whose load shows its own licence dialog past the load limit is not given up
/// on, nor thrown away and loaded again: it waits for the dialog (its card says so) and
/// loads once it closes (review isolation-4 #57; isolation-5 rule 4).
#[test]
fn a_load_held_by_its_own_dialog_waits_for_it_and_loads() {
    let _g = serial();
    let project = demo_project();
    let strip = strip_of(&project, 0);
    let mut rig = Rig::new(project, patient_host());
    let id = rig.insert(strip, GAIN_ID, PluginIsolation::Together);
    rig.settle();
    // Saved and opened again in a fresh session whose helper shows the dialog once.
    let saved = rig.host.project_for_save(rig.bus.project());
    drop(rig);
    let dir = tempfile::tempdir().expect("a folder");
    std::env::set_var(
        "CANTUS_TEST_GAIN_PUMP_ACTIVATE_ONCE",
        dir.path().join("pumped"),
    );
    let mut host = patient_host();
    if let Some(pool) = host.sandboxes() {
        pool.set_limits(Limits {
            load: Duration::from_millis(500),
            ..pool.limits()
        });
    }
    host.default_isolation = PluginIsolation::Together;
    let t = Instant::now();
    let rig2 = Rig::new(saved, host);
    std::env::remove_var("CANTUS_TEST_GAIN_PUMP_ACTIVATE_ONCE");
    assert!(dir.path().join("pumped").exists(), "its dialog ran");
    assert!(t.elapsed() > Duration::from_millis(1_400), "{:?}", t.elapsed());
    assert!(rig2.host.isolation(id).is_some(), "loaded after its dialog");
    assert!(rig2.host.crashed(id).is_none() && rig2.host.load_error(id).is_none());
    assert!(
        rig2.notices.iter().all(|n| !matches!(
            n,
            HostNotice::LoadFailed { .. } | HostNotice::PluginCrashed { .. }
        )),
        "{:?}",
        rig2.notices
    );
}

/// A load past its limit without a dialog (a plug-in slow to start, not pumping): a failed
/// load — not a crash, its helper and neighbours left alone — with Reload, never tried
/// again by itself; its late answer is not taken (isolation-5 rule 4, #3, #11, #20, #31,
/// #51, #62).
#[test]
fn a_load_past_its_limit_is_a_failed_load_never_retried() {
    let _g = serial();
    let project = demo_project();
    let strip = strip_of(&project, 0);
    let mut rig = Rig::new(project, patient_host());
    let id = rig.insert(strip, GAIN_ID, PluginIsolation::Together);
    rig.settle();
    let saved = rig.host.project_for_save(rig.bus.project());
    drop(rig);
    let dir = tempfile::tempdir().expect("a folder");
    std::env::set_var(
        "CANTUS_TEST_GAIN_SLOW_ACTIVATE_ONCE",
        dir.path().join("slow"),
    );
    let mut host = patient_host();
    if let Some(pool) = host.sandboxes() {
        pool.set_limits(Limits {
            silence: Duration::from_secs(10),
            load: Duration::from_millis(500),
            ..pool.limits()
        });
    }
    host.default_isolation = PluginIsolation::Together;
    let mut rig = Rig::new(saved, host);
    std::env::remove_var("CANTUS_TEST_GAIN_SLOW_ACTIVATE_ONCE");
    assert!(rig.host.load_error(id).is_some(), "a failed load");
    assert!(rig.host.crashed(id).is_none(), "not a crash");
    let failed = |r: &Rig| {
        r.notices
            .iter()
            .filter(|n| matches!(n, HostNotice::LoadFailed { .. }))
            .count()
    };
    assert_eq!(failed(&rig), 1);
    // Three seconds on (its late answer came meanwhile): still a failed load, never tried
    // again, said once.
    frames_until(&mut rig, Duration::from_secs(3), |_| false);
    assert!(rig.host.load_error(id).is_some() && rig.host.isolation(id).is_none());
    assert_eq!(failed(&rig), 1);
    assert!(rig.host.crashed_all().0.is_empty());
    // Reload: it loads.
    rig.host
        .reload(rig.bus.project(), &mut rig.engine, id)
        .expect("asked");
    rig.settle();
    assert!(rig.host.isolation(id).is_some(), "loaded");
    assert!(rig.host.load_error(id).is_none());
}

/// Two plug-ins' jobs running at a fail-fast abort: nothing says whose — neither is counted,
/// remembered, or offered a process of its own (review isolation-4 #58).
#[cfg(windows)]
#[test]
fn an_end_nothing_lays_on_one_plugin_is_counted_for_none() {
    let _g = serial();
    let mut p = Project::new("two");
    for k in 0..2 {
        let mut t = p.make_track(format!("T{k}"));
        let clip = p.make_clip("Riff", Ticks::ZERO, Ticks::beats(8));
        t.clips.push(clip);
        p.tracks.push(t);
    }
    let (s0, s1) = (strip_of(&p, 0), strip_of(&p, 1));
    let host = host();
    if let Some(pool) = host.sandboxes() {
        pool.set_threads(Some(2));
    }
    let mut rig = Rig::new(p, host);
    let a = rig.insert(s0, GAIN_ID, PluginIsolation::Together);
    let b = rig.insert(s1, GAIN_ID, PluginIsolation::Together);
    rig.settle();
    rig.engine.seek(Ticks::ZERO);
    rig.engine.play();
    for _ in 0..10 {
        rig.block(256);
    }
    // A's job runs on for a second; B aborts on the other thread meanwhile.
    rig.set_param(s0, a, P_FAULT, SLOW_SECOND);
    rig.set_param(s1, b, P_FAULT, ABORT_PROCESS);
    for _ in 0..4 {
        rig.block(256);
    }
    frames_until(&mut rig, Duration::from_secs(5), |r| {
        r.host.crashed(a).is_some() && r.host.crashed(b).is_some()
    });
    rig.engine.stop();
    assert!(rig.host.crashed(a).is_some() && rig.host.crashed(b).is_some());
    assert!(!rig.host.crash_culprit(a) && !rig.host.crash_culprit(b));
    assert_eq!(rig.host.crashes_this_session(Format::Clap, GAIN_ID), 0);
    assert!(rig.host.db.crash_record(Format::Clap, GAIN_ID).is_none());
}

/// A reload that crashes again says why it stopped now, and whose doing that was: a
/// plug-in that stopped as a neighbour and now crashes by itself is offered a process of
/// its own (review isolation-4 #64; isolation-5 #36: the neighbour turned culprit).
#[test]
fn a_reload_that_fails_says_why_now_and_whose() {
    let _g = serial();
    let mut p = demo_project();
    let mut t = p.make_track("Two");
    let clip = p.make_clip("Riff", Ticks::ZERO, Ticks::beats(8));
    t.clips.push(clip);
    p.tracks.push(t);
    let (s0, s1) = (strip_of(&p, 0), strip_of(&p, 1));
    let mut rig = Rig::new(p, patient_host());
    let a = rig.insert(s0, GAIN_ID, PluginIsolation::Together);
    let b = rig.insert(s1, GAIN_ID, PluginIsolation::Together);
    rig.settle();
    rig.engine.seek(Ticks::ZERO);
    rig.engine.play();
    rig.set_param(s0, a, P_FAULT, SEGV_PROCESS);
    for _ in 0..10 {
        rig.block(256);
    }
    frames_until(&mut rig, Duration::from_secs(5), |r| {
        r.host.crashed(a).is_some() && r.host.crashed(b).is_some()
    });
    rig.engine.stop();
    assert!(rig.host.crash_culprit(a), "A did it");
    assert!(!rig.host.crash_culprit(b), "B was a neighbour");
    let before = rig.host.crashed(b).map(str::to_string);
    // B, reloaded, now crashes by itself as it starts.
    std::env::set_var("CANTUS_TEST_GAIN_ABORT_ON_ACTIVATE", "1");
    rig.host
        .reload(rig.bus.project(), &mut rig.engine, b)
        .expect("asked: its load is answered later");
    std::env::remove_var("CANTUS_TEST_GAIN_ABORT_ON_ACTIVATE");
    frames_until(&mut rig, Duration::from_secs(5), |r| {
        r.host.crash_culprit(b)
    });
    assert!(rig.host.crashed(b).is_some(), "it crashed again");
    assert!(rig.host.crash_culprit(b), "its own doing this time");
    let now = rig.host.crashed(b).map(str::to_string);
    assert!(
        now.is_some() && now != before,
        "why it stopped now: {now:?} (before: {before:?})"
    );
    assert_eq!(
        rig.host.crashes_this_session(Format::Clap, GAIN_ID),
        2,
        "A's crash and B's own, each counted once"
    );
}

/// A plug-in that crashes in its destructor as it is removed is counted as its own crash,
/// though it left (review isolation-4 #45).
#[test]
fn a_plugin_that_crashes_as_it_leaves_is_counted() {
    let _g = serial();
    let mut p = demo_project();
    let mut t = p.make_track("Two");
    let clip = p.make_clip("Riff", Ticks::ZERO, Ticks::beats(8));
    t.clips.push(clip);
    p.tracks.push(t);
    let (s0, s1) = (strip_of(&p, 0), strip_of(&p, 1));
    let mut rig = Rig::new(p, patient_host());
    let a = rig.insert(s0, VST3_GAIN_ID, PluginIsolation::Together);
    let x = rig.insert(s1, GAIN_ID, PluginIsolation::Together);
    rig.settle();
    rig.engine.seek(Ticks::ZERO);
    rig.engine.play();
    rig.set_param(s1, x, P_FAULT, SEGV_DESTROY);
    for _ in 0..10 {
        rig.block(256);
    }
    rig.engine.stop();
    rig.exec(Command::RemoveEffect {
        strip: s1,
        effect: x,
    });
    frames_until(&mut rig, Duration::from_secs(5), |r| {
        r.host.crashed(a).is_some()
    });
    frames_until(&mut rig, Duration::from_millis(300), |_| false);
    assert!(
        !rig.host.crash_culprit(a),
        "the one that stayed did nothing"
    );
    assert_eq!(rig.host.crashes_this_session(Format::Clap, GAIN_ID), 1);
    assert_eq!(rig.host.crashes_this_session(Format::Vst3, VST3_GAIN_ID), 0);
    assert!(rig.host.db.crash_record(Format::Clap, GAIN_ID).is_some());
}

/// A render whose plug-in takes its time to load its state (its own dialog in the render's
/// helper) waits for it, so the render equals one that did not wait — never a render that
/// starts with another state (review isolation-4 #10).
#[test]
fn a_render_waits_for_a_state_that_takes_its_time() {
    let _g = serial();
    let project = demo_project();
    let strip = strip_of(&project, 0);
    let mut rig = Rig::new(project, patient_host());
    let id = rig.insert(strip, GAIN_ID, PluginIsolation::Together);
    // A state that doubles its output: what only the state carries, never the values the
    // engine sends with the first block.
    let mut state = b"CTG1".to_vec();
    for v in [-12.0f64, 0.0, 0.0, 0.0] {
        state.extend_from_slice(&v.to_le_bytes());
    }
    state.extend_from_slice(&2u32.to_le_bytes());
    state.extend_from_slice(b"x2");
    rig.exec(Command::SetPluginState {
        strip,
        effect: id,
        state: PluginState::new(state),
        params: [(P_GAIN, -12.0)].into_iter().collect(),
    });
    rig.settle();
    let reference = render(&mut rig, false);
    assert!(rig.host.render_failures().is_empty());
    rig.host.finish_render();
    if let Some(pool) = rig.host.sandboxes() {
        pool.set_limits(Limits {
            state: Duration::from_millis(500),
            call: Duration::from_millis(500),
            ..pool.limits()
        });
    }
    std::env::set_var("CANTUS_TEST_GAIN_PUMP_LOAD_MS", "2000");
    let waited = render(&mut rig, false);
    std::env::remove_var("CANTUS_TEST_GAIN_PUMP_LOAD_MS");
    let failures = rig.host.render_failures();
    rig.host.finish_render();
    assert!(failures.is_empty(), "{failures:?}");
    assert_eq!(differing(&reference, &waited), 0, "the same render");
}

/// The same render, its instance's load (its state in a dialog, a render's instance has no
/// card) past the load limit — the same live and offline: the render says it lacks the
/// plug-in, from the start (review isolation-4 #10, #41; isolation-5 #6, #14, #22).
#[test]
fn a_render_whose_state_never_comes_says_so() {
    let _g = serial();
    let project = demo_project();
    let strip = strip_of(&project, 0);
    let mut rig = Rig::new(project, patient_host());
    let _id = rig.insert(strip, GAIN_ID, PluginIsolation::Together);
    rig.settle();
    if let Some(pool) = rig.host.sandboxes() {
        pool.set_limits(Limits {
            load: Duration::from_millis(800),
            ..pool.limits()
        });
    }
    std::env::set_var("CANTUS_TEST_GAIN_PUMP_LOAD_MS", "3000");
    let _ = render(&mut rig, false);
    std::env::remove_var("CANTUS_TEST_GAIN_PUMP_LOAD_MS");
    let failures = rig.host.render_failures();
    rig.host.finish_render();
    assert_eq!(failures.len(), 1, "{failures:?}");
    assert_eq!(failures[0].kind, RenderLoss::NeverLoaded);
}

/// A render instance that refuses its state is not run with another one: the render says
/// it lacks it (review isolation-4 #24, #34, #63).
#[test]
fn a_render_whose_state_is_refused_says_so() {
    let _g = serial();
    let project = demo_project();
    let strip = strip_of(&project, 0);
    let mut rig = Rig::new(project, patient_host());
    let id = rig.insert(strip, GAIN_ID, PluginIsolation::Together);
    rig.settle();
    rig.exec(Command::SetPluginState {
        strip,
        effect: id,
        state: PluginState::new(b"not a state of it".to_vec()),
        params: Default::default(),
    });
    rig.settle();
    let _ = render(&mut rig, true);
    let failures = rig.host.render_failures();
    rig.host.finish_render();
    assert_eq!(failures.len(), 1, "{failures:?}");
    assert_eq!(failures[0].kind, RenderLoss::NeverLoaded);
}

/// A render whose plug-in crashes as its render helper loads it says so — as one it never
/// had, not one that stopped part way (review isolation-4 #34, #41).
#[test]
fn a_render_whose_plugin_crashes_loading_says_it_never_had_it() {
    let _g = serial();
    let project = demo_project();
    let strip = strip_of(&project, 0);
    let mut rig = Rig::new(project, patient_host());
    let id = rig.insert(strip, GAIN_ID, PluginIsolation::Together);
    rig.settle();
    std::env::set_var("CANTUS_TEST_GAIN_ABORT_ON_ACTIVATE", "1");
    let _ = render(&mut rig, false);
    std::env::remove_var("CANTUS_TEST_GAIN_ABORT_ON_ACTIVATE");
    let failures = rig.host.render_failures();
    rig.host.finish_render();
    assert_eq!(failures.len(), 1, "{failures:?}");
    assert_eq!(failures[0].kind, RenderLoss::NeverLoaded);
    // The render instance's crash is the render's loss, never the live plug-in's: not
    // told, not counted, the live one untouched (isolation-5 #41: frames ran meanwhile).
    frames_until(&mut rig, Duration::from_millis(300), |_| false);
    assert!(rig
        .notices
        .iter()
        .all(|n| !matches!(n, HostNotice::PluginCrashed { .. })));
    assert_eq!(rig.host.crashes_this_session(Format::Clap, GAIN_ID), 0);
    assert!(rig.host.crashed(id).is_none());
}

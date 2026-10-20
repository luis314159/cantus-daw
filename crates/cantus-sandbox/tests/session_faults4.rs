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

/// A plug-in whose load was given up on while its helper was busy (its own licence
/// dialog) is not left missing for the session: it is tried again, and loads (review
/// isolation-4 #57).
#[test]
fn a_load_given_up_on_while_busy_is_tried_again() {
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
            modal: Duration::from_millis(500),
            ..pool.limits()
        });
    }
    host.default_isolation = PluginIsolation::Together;
    let mut rig2 = Rig::new(saved, host);
    std::env::remove_var("CANTUS_TEST_GAIN_PUMP_ACTIVATE_ONCE");
    assert!(
        rig2.host.isolation(id).is_none(),
        "given up on: its helper was busy"
    );
    frames_until(&mut rig2, Duration::from_secs(8), |r| {
        r.host.isolation(id).is_some()
    });
    assert!(rig2.host.isolation(id).is_some(), "tried again, and loaded");
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
/// its own (review isolation-4 #64).
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
    let b = rig.insert(s1, VST3_GAIN_ID, PluginIsolation::Together);
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
    assert!(!rig.host.crash_culprit(b), "B was a neighbour");
    let before = rig.host.crashed(b).map(str::to_string);
    // B now crashes by itself whenever it starts (a VST3 that aborts on activation is not
    // to hand: its helper is made to fail with A's, which aborts).
    std::env::set_var("CANTUS_TEST_GAIN_ABORT_ON_ACTIVATE", "1");
    let r = rig.host.reload(rig.bus.project(), &mut rig.engine, a);
    std::env::remove_var("CANTUS_TEST_GAIN_ABORT_ON_ACTIVATE");
    assert!(r.is_err(), "it crashed again");
    assert!(rig.host.crash_culprit(a), "its own doing this time");
    let now = rig.host.crashed(a).map(str::to_string);
    assert!(now.is_some() && now != before, "why it stopped now: {now:?}");
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
    assert!(!rig.host.crash_culprit(a), "the one that stayed did nothing");
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
    rig.set_param(strip, id, P_GAIN, -12.0);
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

/// The same render, its helper's state load past the offline limit: the render says it
/// lacks the plug-in, from the start (review isolation-4 #10, #41).
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
            state: Duration::from_millis(300),
            offline: Duration::from_millis(800),
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
    let _id = rig.insert(strip, GAIN_ID, PluginIsolation::Together);
    rig.settle();
    std::env::set_var("CANTUS_TEST_GAIN_ABORT_ON_ACTIVATE", "1");
    let _ = render(&mut rig, false);
    std::env::remove_var("CANTUS_TEST_GAIN_ABORT_ON_ACTIVATE");
    let failures = rig.host.render_failures();
    rig.host.finish_render();
    assert_eq!(failures.len(), 1, "{failures:?}");
    assert_eq!(failures[0].kind, RenderLoss::NeverLoaded);
    assert!(rig.notices.iter().all(|n| !matches!(n, HostNotice::PluginCrashed { .. })));
}

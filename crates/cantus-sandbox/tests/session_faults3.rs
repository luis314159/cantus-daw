//! The second review of the fix round through the host and engine (round 29,
//! isolation-3): a reload that crashes again is counted; a render that cannot load a
//! plug-in says so; under the worker pool a crash is laid on the plug-in that did it and
//! counted once.

mod support;

use cantus_core::*;
use cantus_engine::processor::Workers;
use cantus_engine::{render_with_instruments, RenderOptions};
use cantus_host::bridge::sandbox::Limits;
use cantus_host::{Format, HostNotice};
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

/// A plug-in that crashes every time it is reloaded is counted each time, so it is
/// offered a process of its own (review #15).
#[test]
fn a_reload_that_crashes_again_is_counted() {
    let _g = serial();
    let project = demo_project();
    let strip = strip_of(&project, 0);
    let mut rig = Rig::new(project, patient_host());
    let id = rig.insert(strip, GAIN_ID, PluginIsolation::Individually);
    rig.settle();
    rig.engine.seek(Ticks::ZERO);
    rig.engine.play();
    rig.set_param(strip, id, P_FAULT, ABORT_PROCESS);
    for _ in 0..10 {
        rig.block(256);
    }
    frames_until(&mut rig, Duration::from_secs(5), |r| {
        r.host.crashed(id).is_some()
    });
    rig.engine.stop();
    assert!(rig.host.crashed(id).is_some());
    assert_eq!(rig.host.crashes_this_session(Format::Clap, GAIN_ID), 1);
    std::env::set_var("CANTUS_TEST_GAIN_ABORT_ON_ACTIVATE", "1");
    for k in 0..2u32 {
        // Asked; its load is answered later, and it crashes again.
        rig.host
            .reload(rig.bus.project(), &mut rig.engine, id)
            .expect("asked");
        frames_until(&mut rig, Duration::from_secs(5), |r| {
            r.host.crashes_this_session(Format::Clap, GAIN_ID) == 2 + k
        });
    }
    std::env::remove_var("CANTUS_TEST_GAIN_ABORT_ON_ACTIVATE");
    assert_eq!(rig.host.crashes_this_session(Format::Clap, GAIN_ID), 3);
    assert!(rig.host.crashed(id).is_some(), "still stopped");
}

/// A render whose isolated plug-in cannot even be loaded (its helper never answers in
/// time) is not reported done (review #6, #47).
#[test]
fn a_render_that_cannot_load_a_plugin_says_so() {
    let _g = serial();
    let project = demo_project();
    let strip = strip_of(&project, 0);
    let mut rig = Rig::new(project, patient_host());
    let _id = rig.insert(strip, GAIN_ID, PluginIsolation::Together);
    rig.settle();
    if let Some(pool) = rig.host.sandboxes() {
        pool.set_limits(Limits {
            load: Duration::from_millis(1),
            ..pool.limits()
        });
    }
    rig.prepare_render(false);
    let units = rig.host.render_units(rig.bus.project(), SR);
    let insts = rig.host.render_instruments(rig.bus.project(), SR);
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
    assert_eq!(failures[0].name, info(GAIN_ID).name);
    assert!(rig.host.render_failures().is_empty(), "finished: forgotten");
}

/// Two strips of one helper played by the worker pool: one plug-in crashes (an access
/// violation, which the crash handler names) — only it is counted and remembered, its
/// neighbour is not (review #19, #20, #45).
#[test]
fn under_the_pool_a_crash_is_laid_on_the_plugin_that_did_it() {
    let _g = serial();
    let mut p = Project::new("two");
    for k in 0..2 {
        let mut t = p.make_track(format!("T{k}"));
        let mut clip = p.make_clip("Riff", Ticks::ZERO, Ticks::beats(8));
        for i in 0..8 {
            let n = p.make_note(Ticks::QUARTER * i, Ticks::QUARTER, 60 + i as u8, 90);
            clip.notes.push(n);
        }
        t.clips.push(clip);
        p.tracks.push(t);
    }
    let (s0, s1) = (strip_of(&p, 0), strip_of(&p, 1));
    let mut rig = Rig::new(p, patient_host());
    rig.processor.set_workers(Workers::parallel(3));
    let a = rig.insert(s0, GAIN_ID, PluginIsolation::Together);
    let b = rig.insert(s1, VST3_GAIN_ID, PluginIsolation::Together);
    rig.settle();
    rig.engine.seek(Ticks::ZERO);
    rig.engine.play();
    for _ in 0..10 {
        rig.block(256);
    }
    rig.set_param(s0, a, P_FAULT, SEGV_PROCESS);
    for _ in 0..20 {
        rig.block(256);
    }
    frames_until(&mut rig, Duration::from_secs(5), |r| {
        r.notices
            .iter()
            .filter(|n| matches!(n, HostNotice::PluginCrashed { .. }))
            .count()
            >= 2
    });
    rig.engine.stop();
    assert!(rig.host.crash_culprit(a));
    assert!(!rig.host.crash_culprit(b), "its neighbour did not do it");
    assert_eq!(rig.host.crashes_this_session(Format::Clap, GAIN_ID), 1);
    assert_eq!(rig.host.crashes_this_session(Format::Vst3, VST3_GAIN_ID), 0);
    assert!(rig
        .host
        .db
        .crash_record(Format::Vst3, VST3_GAIN_ID)
        .is_none());
}

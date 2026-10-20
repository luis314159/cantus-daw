//! The review's faults through the whole host and engine (round 29, isolation-2): the
//! worker pool against the serial engine, instruments to the bit, a crash in a shared
//! helper counted once and only for the plug-in that did it, a helper that is not
//! installed, plug-ins that crash while they load, renders that lose a plug-in, the
//! processes each choice makes.

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

/// Four tracks, each with the riff, each into its own strip.
fn four_tracks() -> Project {
    let mut p = Project::new("four");
    for (k, base) in [48u8, 55, 60, 67].iter().enumerate() {
        let mut t = p.make_track(format!("T{k}"));
        let mut clip = p.make_clip("Riff", Ticks::ZERO, Ticks::beats(8));
        for i in 0..8 {
            let n = p.make_note(
                Ticks::QUARTER * i,
                Ticks::QUARTER - Ticks(40),
                base + i as u8,
                90,
            );
            clip.notes.push(n);
        }
        t.clips.push(clip);
        p.tracks.push(t);
    }
    p
}

/// The four tracks with the test gain on each strip, at its own gain, run `iso`.
fn four_rig(iso: PluginIsolation) -> (Rig, Vec<(StripId, EffectId)>) {
    let project = four_tracks();
    let strips: Vec<StripId> = (0..4).map(|k| strip_of(&project, k)).collect();
    let mut rig = Rig::new(project, patient_host());
    let mut ids = Vec::new();
    for (k, strip) in strips.into_iter().enumerate() {
        let id = rig.insert(strip, GAIN_ID, iso);
        rig.set_param(strip, id, P_GAIN, -1.0 - 3.0 * k as f64);
        ids.push((strip, id));
    }
    rig.settle();
    (rig, ids)
}

fn crash_notices(rig: &Rig) -> usize {
    rig.notices
        .iter()
        .filter(|n| {
            matches!(
                n,
                HostNotice::PluginCrashed { .. } | HostNotice::InstrumentCrashed { .. }
            )
        })
        .count()
}

fn pid_of(rig: &Rig, effect: EffectId) -> u32 {
    rig.host
        .hosted(effect)
        .and_then(|h| h.param_value(P_PROCESS))
        .expect("the Process parameter")
        .round() as u32
}

fn frames_until(rig: &mut Rig, limit: Duration, done: impl Fn(&Rig) -> bool) {
    let start = Instant::now();
    while !done(rig) && start.elapsed() < limit {
        rig.frame();
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// Isolated plug-ins called by the engine's worker pool — several of them in one helper,
/// at once — make the bits the serial engine makes, and the in-process plug-ins (#23).
#[test]
fn the_worker_pool_plays_isolated_plugins_to_the_bit_of_the_serial_engine() {
    let _g = serial();
    let (mut within, _) = four_rig(PluginIsolation::Within);
    let reference = within.play(24_000, 256);
    assert!(peak(&reference) > 0.01);
    for iso in [PluginIsolation::Together, PluginIsolation::Individually] {
        let (mut serial_rig, _) = four_rig(iso);
        let serial_out = serial_rig.play(24_000, 256);
        let (mut pooled, ids) = four_rig(iso);
        pooled.processor.set_workers(Workers::parallel(3));
        let pooled_out = pooled.play(24_000, 256);
        assert_eq!(differing(&serial_out, &reference), 0, "{iso:?}: serial");
        assert_eq!(differing(&pooled_out, &reference), 0, "{iso:?}: pooled");
        assert_eq!(pooled.host.late_blocks(), 0);
        // A crash mid-song under the pool: the song plays on, told once per plug-in.
        let (strip, id) = ids[1];
        pooled.engine.seek(Ticks::ZERO);
        pooled.engine.play();
        for _ in 0..20 {
            pooled.block(256);
        }
        pooled.set_param(strip, id, P_FAULT, ABORT_PROCESS);
        let mut after = Vec::new();
        for _ in 0..40 {
            after.extend(pooled.block(256));
        }
        frames_until(&mut pooled, Duration::from_secs(5), |r| {
            r.host.crashed(id).is_some()
        });
        pooled.engine.stop();
        assert!(pooled.host.crashed(id).is_some(), "{iso:?}: told");
        assert!(peak(&after) > 0.01, "{iso:?}: the song played on");
    }
}

/// A hosted instrument plays the same bits isolated as within Cantus, through the
/// engine's per-segment calls, at three block sizes (#24).
#[test]
fn an_isolated_instrument_plays_the_project_to_the_bit_of_in_process() {
    let _g = serial();
    let make = |iso: PluginIsolation| {
        let mut p = Project::new("synth");
        let mut keys = p.make_track("Keys");
        let mut clip = p.make_clip("Run", Ticks::ZERO, Ticks::beats(4));
        for i in 0..16 {
            // Sixteenths, so notes land inside blocks of every size.
            let n = p.make_note(
                Ticks::QUARTER / 4 * i,
                Ticks::QUARTER / 4,
                60 + i as u8,
                100,
            );
            clip.notes.push(n);
        }
        keys.clips.push(clip);
        let track = keys.id;
        p.tracks.push(keys);
        let mut rig = Rig::new(p, patient_host());
        let spec = rig
            .host
            .make_instrument_isolated(track, &info(SYNTH_ID), Some(iso))
            .expect("the synth loads");
        rig.exec(Command::SetInstrument {
            track,
            spec: Box::new(spec),
        });
        rig.settle();
        rig
    };
    let mut within = make(PluginIsolation::Within);
    let mut together = make(PluginIsolation::Together);
    for block in [64usize, 256, 1000] {
        let a = within.play(48_000, block);
        let b = together.play(48_000, block);
        assert!(peak(&a) > 0.01);
        assert_eq!(differing(&a, &b), 0, "at {block} frames per block");
    }
}

/// Two plug-ins in one helper, one crashes: both stop and are told, but the crash is
/// counted once, for the one that did it, and only it is remembered; reloading brings both
/// back with their values (#25, #49).
#[test]
fn a_crash_in_a_shared_helper_is_counted_once_and_only_for_the_plugin_that_did_it() {
    let _g = serial();
    for fault_value in [ABORT_PROCESS, SEGV_PROCESS] {
        let project = demo_project();
        let strip = strip_of(&project, 0);
        let mut rig = Rig::new(project, patient_host());
        let a = rig.insert(strip, GAIN_ID, PluginIsolation::Together);
        let b = rig.insert(strip, VST3_GAIN_ID, PluginIsolation::Together);
        rig.set_param(strip, a, P_GAIN, -6.0);
        rig.set_param(strip, b, V_GAIN, -3.0);
        rig.settle();
        let reference = rig.play(24_000, 256);
        rig.engine.seek(Ticks::ZERO);
        rig.engine.play();
        rig.set_param(strip, a, P_FAULT, fault_value);
        for _ in 0..20 {
            rig.block(256);
        }
        frames_until(&mut rig, Duration::from_secs(5), |r| crash_notices(r) >= 2);
        rig.engine.stop();
        assert_eq!(crash_notices(&rig), 2, "both told: {:?}", rig.notices);
        assert!(rig.host.crash_culprit(a), "{fault_value}: the gain did it");
        assert!(
            !rig.host.crash_culprit(b),
            "{fault_value}: its neighbour did not"
        );
        assert_eq!(rig.host.crashes_this_session(Format::Clap, GAIN_ID), 1);
        assert_eq!(rig.host.crashes_this_session(Format::Vst3, VST3_GAIN_ID), 0);
        assert!(rig.host.db.crash_record(Format::Clap, GAIN_ID).is_some());
        assert!(rig
            .host
            .db
            .crash_record(Format::Vst3, VST3_GAIN_ID)
            .is_none());
        // Reload all: both back, their values with them.
        rig.exec(Command::SetPluginParam {
            strip,
            effect: a,
            id: P_FAULT,
            value: Some(0.0),
        });
        let failed = rig
            .host
            .reload_all_crashed(rig.bus.project(), &mut rig.engine);
        assert!(failed.is_empty(), "{failed:?}");
        rig.settle();
        let back = rig.play(24_000, 256);
        assert_eq!(differing(&back, &reference), 0, "{fault_value}: as before");
    }
}

/// The same plug-in twice in one helper counts one crash per helper death (#25).
#[test]
fn two_instances_of_one_plugin_count_one_crash() {
    let _g = serial();
    let project = demo_project();
    let strip = strip_of(&project, 0);
    let mut rig = Rig::new(project, patient_host());
    let a = rig.insert(strip, GAIN_ID, PluginIsolation::Together);
    let _b = rig.insert(strip, GAIN_ID, PluginIsolation::Together);
    rig.settle();
    rig.engine.seek(Ticks::ZERO);
    rig.engine.play();
    rig.set_param(strip, a, P_FAULT, ABORT_PROCESS);
    for _ in 0..20 {
        rig.block(256);
    }
    frames_until(&mut rig, Duration::from_secs(5), |r| crash_notices(r) >= 2);
    rig.engine.stop();
    assert_eq!(crash_notices(&rig), 2);
    assert_eq!(rig.host.crashes_this_session(Format::Clap, GAIN_ID), 1);
}

/// The helper is not installed: plug-ins set to run apart run within Cantus, and the
/// application is told once (#26).
#[test]
fn without_the_helper_plugins_run_within_cantus_and_it_is_said_once() {
    let _g = serial();
    let mut host = host();
    host.set_sandboxes(cantus_host::bridge::Sandboxes::new(
        "no-such-cantus-sandbox.exe".into(),
    ));
    host.default_isolation = PluginIsolation::Together;
    let project = demo_project();
    let strip = strip_of(&project, 0);
    let mut rig = Rig::new(project, host);
    let id = rig.insert(strip, GAIN_ID, PluginIsolation::Together);
    let _other = rig.insert(strip, GAIN_ID, PluginIsolation::Individually);
    for _ in 0..10 {
        rig.block(256);
    }
    rig.resync();
    rig.settle();
    assert_eq!(pid_of(&rig, id), std::process::id(), "within Cantus");
    assert_eq!(rig.host.isolation(id), Some(PluginIsolation::Within));
    let told = rig
        .notices
        .iter()
        .filter(|n| matches!(n, HostNotice::IsolationUnavailable))
        .count();
    assert_eq!(told, 1, "{:?}", rig.notices);
    assert!(!rig
        .notices
        .iter()
        .any(|n| matches!(n, HostNotice::LoadFailed { .. })));
}

/// *Individually* gives each plug-in a process; *By maker* one per maker (#29).
#[test]
fn each_choice_makes_the_processes_it_says() {
    let _g = serial();
    let project = demo_project();
    let strip = strip_of(&project, 0);
    let mut rig = Rig::new(project, patient_host());
    let a = rig.insert(strip, GAIN_ID, PluginIsolation::Individually);
    let b = rig.insert(strip, GAIN_ID, PluginIsolation::Individually);
    let c = rig.insert(strip, GAIN_ID, PluginIsolation::ByVendor);
    let d = rig.insert(strip, GAIN_ID, PluginIsolation::ByVendor);
    rig.settle();
    assert_ne!(pid_of(&rig, a), pid_of(&rig, b));
    assert_eq!(pid_of(&rig, c), pid_of(&rig, d));
    assert!(![pid_of(&rig, a), pid_of(&rig, b)].contains(&pid_of(&rig, c)));
    assert!(![a, b, c, d]
        .iter()
        .any(|e| pid_of(&rig, *e) == std::process::id()));
}

/// A plug-in that crashes its helper while it loads is a crash, not a missing plug-in:
/// told and counted as one, reloadable, and tried again when where it runs changes (#15).
#[test]
fn a_plugin_that_crashes_while_loading_can_be_reloaded_or_moved() {
    let _g = serial();
    let project = demo_project();
    let strip = strip_of(&project, 0);
    let mut rig = Rig::new(project, patient_host());
    // A VST3 gain in the shared helper, its fault (one per process) armed to abort the
    // next activation there.
    let armed = rig.insert(strip, VST3_GAIN_ID, PluginIsolation::Together);
    let moved = rig.insert(strip, VST3_GAIN_ID, PluginIsolation::Within);
    rig.settle();
    rig.set_param(strip, armed, V_FAULT, ABORT_ACTIVATE);
    for _ in 0..8 {
        rig.block(256);
    }
    // The second moves into that helper: its activation there aborts it.
    rig.exec(Command::SetPluginIsolation {
        strip,
        effect: moved,
        isolation: Some(PluginIsolation::Together),
    });
    frames_until(&mut rig, Duration::from_secs(5), |r| {
        r.host.crashed(moved).is_some() && r.host.crashed(armed).is_some()
    });
    assert!(rig.host.crashed(moved).is_some(), "{:?}", rig.notices);
    assert!(rig
        .notices
        .iter()
        .any(|n| matches!(n, HostNotice::PluginCrashed { effect, .. } if *effect == moved)));
    assert!(!rig
        .notices
        .iter()
        .any(|n| matches!(n, HostNotice::LoadFailed { effect, .. } if *effect == moved)));
    assert_eq!(rig.host.crashes_this_session(Format::Vst3, VST3_GAIN_ID), 1);
    // Run on its own: tried again there, and it loads.
    rig.exec(Command::SetPluginIsolation {
        strip,
        effect: moved,
        isolation: Some(PluginIsolation::Individually),
    });
    rig.settle();
    assert!(rig.host.crashed(moved).is_none());
    assert_eq!(
        rig.host.isolation(moved),
        Some(PluginIsolation::Individually)
    );
    // The armed one, reloaded (in a fresh helper), runs again too.
    rig.exec(Command::SetPluginParam {
        strip,
        effect: armed,
        id: V_FAULT,
        value: Some(0.0),
    });
    rig.host
        .reload(rig.bus.project(), &mut rig.engine, armed)
        .expect("reloads");
    rig.settle();
    assert!(rig.host.crashed(armed).is_none());
}

/// An automation clip that sets the gain's Fault parameter at beat 2: what crashes (or
/// hangs) an offline render's instance in its `process`.
fn fault_at_beat_two(rig: &mut Rig, effect: EffectId, value: f64) {
    let track = rig.bus.project().tracks[0].id;
    let pos = (value / 14.0) as f32;
    let mut pts = vec![AutoPoint::new(Ticks::ZERO, pos)];
    pts[0].shape = Shape::Hold;
    let mut clip = rig.bus.new_clip("fault", Ticks::beats(2), Ticks::beats(2));
    clip.automation = Some(Automation::new(
        AutoTarget::Plugin {
            effect,
            id: P_FAULT,
        },
        pts,
    ));
    rig.exec(Command::InsertClip {
        track,
        clip: Box::new(clip),
    });
}

fn render(rig: &mut Rig) -> Vec<f32> {
    let units = rig.host.render_units(rig.bus.project(), SR);
    let insts = rig.host.render_instruments(rig.bus.project(), SR);
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

/// A render whose isolated plug-in crashes, or hangs, on the way says so — it is not a
/// success with the plug-in silently missing — and a hang ends within the offline limit;
/// the live plug-in, in its own helper, plays on (#6, #39, #46, #47).
#[test]
fn a_render_that_loses_a_plugin_says_so() {
    let _g = serial();
    for (value, what) in [(ABORT_PROCESS, "crash"), (HANG_PROCESS, "hang")] {
        let project = demo_project();
        let strip = strip_of(&project, 0);
        let mut rig = Rig::new(project, patient_host());
        if let Some(pool) = rig.host.sandboxes() {
            pool.set_limits(Limits {
                offline: Duration::from_secs(1),
                ..pool.limits()
            });
        }
        let id = rig.insert(strip, GAIN_ID, PluginIsolation::Together);
        rig.settle();
        fault_at_beat_two(&mut rig, id, value);
        rig.settle();
        let t = Instant::now();
        let _ = render(&mut rig);
        assert!(
            t.elapsed() < Duration::from_secs(20),
            "{what}: {:?}",
            t.elapsed()
        );
        let failures = rig.host.render_failures();
        rig.host.finish_render();
        assert_eq!(failures.len(), 1, "{what}: {failures:?}");
        assert_eq!(failures[0].name, info(GAIN_ID).name);
        // The live instance ran in another helper: untouched.
        for _ in 0..10 {
            rig.frame();
        }
        assert!(
            rig.host.crashed(id).is_none(),
            "{what}: the live one plays on"
        );
    }
}

//! Isolated plug-ins through the whole host and engine (ADR 0063): a project plays the same
//! bits isolated as in process; a plug-in that crashes mid-song leaves Cantus playing, is
//! told once, and comes back with what the user had; the project loses nothing; a change
//! of where it runs moves it; an offline render through isolated plug-ins equals the
//! in-process one; a hang never blocks the interface's frames.

mod support;

use cantus_core::*;
use cantus_engine::{render_with_instruments, RenderOptions};
use cantus_host::HostNotice;
use std::time::{Duration, Instant};
use support::*;

/// A host whose real-time blocks wait as long as it takes (a busy test machine must not
/// make a block late where bits are compared).
fn patient_host() -> cantus_host::PluginHost {
    let host = host();
    if let Some(p) = host.sandboxes() {
        p.set_deadline_floor(Duration::from_secs(5));
    }
    host
}

/// The project with the test gain on its first strip at −6 dB, latency 37, run `iso`.
fn gain_rig(iso: PluginIsolation) -> (Rig, StripId, EffectId) {
    let project = demo_project();
    let strip = strip_of(&project, 0);
    let mut rig = Rig::new(project, patient_host());
    let id = rig.insert(strip, GAIN_ID, iso);
    rig.set_param(strip, id, P_GAIN, -6.0);
    rig.set_param(strip, id, P_LATENCY, 37.0);
    rig.settle();
    (rig, strip, id)
}

fn crashes(rig: &Rig) -> usize {
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

/// The pid the plug-in reports (its Process parameter).
fn pid_of(rig: &Rig, effect: EffectId) -> u32 {
    rig.host
        .hosted(effect)
        .and_then(|h| h.param_value(P_PROCESS))
        .expect("the Process parameter")
        .round() as u32
}

#[test]
fn an_isolated_insert_plays_the_project_to_the_bit_of_in_process() {
    let _g = serial();
    let (mut within, _, w_id) = gain_rig(PluginIsolation::Within);
    let (mut together, _, t_id) = gain_rig(PluginIsolation::Together);
    assert_eq!(pid_of(&within, w_id), std::process::id());
    assert_ne!(pid_of(&together, t_id), std::process::id());
    assert_eq!(together.host.hosted(t_id).unwrap().latency(), 37);
    for block in [64usize, 256, 1000] {
        let a = within.play(24_000, block);
        let b = together.play(24_000, block);
        assert!(peak(&a) > 0.01);
        assert_eq!(differing(&a, &b), 0, "at {block} frames per block");
    }
    assert_eq!(together.host.late_blocks(), 0);
}

#[test]
fn a_crash_mid_song_keeps_cantus_playing_says_so_once_and_reload_brings_back_what_was_set() {
    let _g = serial();
    // What the song sounds like with the slot as a wire, and with the gain at −6 dB.
    let (mut reference, r_strip, r_id) = gain_rig(PluginIsolation::Within);
    let with_gain = reference.play(188 * 256, 256);
    reference.exec(Command::SetEffectBypass {
        strip: r_strip,
        effect: r_id,
        bypass: true,
    });
    reference.settle();
    let dry = reference.play(188 * 256, 256);
    assert_ne!(differing(&with_gain, &dry), 0);

    let (mut rig, strip, id) = gain_rig(PluginIsolation::Individually);
    let first_pid = pid_of(&rig, id);
    rig.engine.seek(Ticks::ZERO);
    rig.engine.play();
    let mut out = Vec::new();
    for _ in 0..40 {
        out.extend(rig.block(256));
    }
    // The fault, as any parameter change: at the next block, inside `process`.
    rig.set_param(strip, id, P_FAULT, ABORT_PROCESS);
    for _ in 0..148 {
        out.extend(rig.block(256));
    }
    let deadline = Instant::now() + Duration::from_secs(5);
    while crashes(&rig) == 0 && Instant::now() < deadline {
        rig.frame();
        std::thread::sleep(Duration::from_millis(5));
    }
    rig.engine.stop();
    assert_eq!(crashes(&rig), 1, "told once: {:?}", rig.notices);
    assert!(rig.host.crashed(id).is_some());
    // Before the crash: the gain; from the crashing block on: the dry song, still playing.
    let n = 40 * 256 * 2;
    assert_eq!(differing(&out[..n], &with_gain[..n]), 0, "before the crash");
    assert_eq!(
        differing(&out[n + 512..], &dry[n + 512..out.len()]),
        0,
        "after the crash the song plays on without it"
    );
    assert!(peak(&out[n + 512..]) > 0.01, "audio kept flowing");
    // Reload: a new process, the values the user set, the song as before the crash.
    rig.host
        .reload(rig.bus.project(), &mut rig.engine, id)
        .expect("reloads");
    rig.settle();
    assert!(rig.host.crashed(id).is_none());
    assert_ne!(pid_of(&rig, id), first_pid, "a new helper");
    assert_eq!(rig.host.hosted(id).unwrap().param_value(P_GAIN), Some(-6.0));
    assert_eq!(rig.host.hosted(id).unwrap().latency(), 37);
    let again = rig.play(188 * 256, 256);
    assert_eq!(
        differing(&again, &with_gain),
        0,
        "what the user had, to the bit"
    );
    assert_eq!(
        rig.host
            .crashes_this_session(cantus_host::Format::Clap, GAIN_ID),
        1
    );
}

#[test]
fn the_project_saved_after_a_crash_keeps_the_plugins_last_good_state() {
    let _g = serial();
    let (mut rig, strip, id) = gain_rig(PluginIsolation::Together);
    // The plug-in's own editor moves Gain in a gesture (−3, −4, −5 dB): recorded in the
    // project, and in the plug-in's state.
    rig.set_param(strip, id, 3, 3.0);
    rig.engine.play();
    for _ in 0..40 {
        rig.block(256);
    }
    assert_eq!(rig.plugin(id).params.get(&P_GAIN), Some(&-5.0));
    // Give the background refresh of the last-good state its moment.
    for _ in 0..60 {
        rig.frame();
        std::thread::sleep(Duration::from_millis(10));
    }
    rig.set_param(strip, id, P_FAULT, ABORT_PROCESS);
    for _ in 0..20 {
        rig.block(256);
    }
    let deadline = Instant::now() + Duration::from_secs(5);
    while crashes(&rig) == 0 && Instant::now() < deadline {
        rig.frame();
    }
    assert_eq!(crashes(&rig), 1);
    let saved = rig.host.project_for_save(rig.bus.project());
    let plugin = saved
        .all_effects()
        .find(|(_, e)| e.id == id)
        .and_then(|(_, e)| e.spec.external.as_deref())
        .unwrap();
    let bytes = plugin.state.bytes();
    assert_eq!(&bytes[..4], b"CTG1");
    let gain = f64::from_le_bytes(bytes[4..12].try_into().unwrap());
    assert_eq!(gain, -5.0, "the state the plug-in had when it crashed");
    assert_eq!(plugin.params.get(&P_GAIN), Some(&-5.0));
}

#[test]
fn an_instrument_that_crashes_goes_silent_and_reload_brings_it_back() {
    let _g = serial();
    let mut p = Project::new("synth");
    let mut keys = p.make_track("Keys");
    let mut clip = p.make_clip("Chord", Ticks::ZERO, Ticks::beats(16));
    for (i, pitch) in [60u8, 64, 67, 72].iter().cycle().take(16).enumerate() {
        let n = p.make_note(Ticks::QUARTER * i as i64, Ticks::QUARTER, *pitch, 100);
        clip.notes.push(n);
    }
    keys.clips.push(clip);
    let track = keys.id;
    p.tracks.push(keys);
    let mut rig = Rig::new(p, patient_host());
    let info = info(SYNTH_ID);
    let spec = rig
        .host
        .make_instrument_isolated(track, &info, Some(PluginIsolation::Individually))
        .expect("the synth loads");
    rig.exec(Command::SetInstrument {
        track,
        spec: Box::new(spec),
    });
    rig.settle();
    rig.engine.seek(Ticks::ZERO);
    rig.engine.play();
    let mut before = Vec::new();
    for _ in 0..40 {
        before.extend(rig.block(256));
    }
    assert!(peak(&before) > 0.01, "it plays");
    rig.exec(Command::SetInstrumentPluginParam {
        track,
        id: S_FAULT,
        value: Some(ABORT_PROCESS),
    });
    let mut after = Vec::new();
    for _ in 0..60 {
        after.extend(rig.block(256));
    }
    let deadline = Instant::now() + Duration::from_secs(5);
    while crashes(&rig) == 0 && Instant::now() < deadline {
        rig.frame();
    }
    assert_eq!(crashes(&rig), 1, "{:?}", rig.notices);
    assert!(rig.host.instrument_crashed(track).is_some());
    assert_eq!(peak(&after[2048..]), 0.0, "silent, and the engine went on");
    rig.host
        .reload_instrument(rig.bus.project(), &mut rig.engine, track)
        .expect("reloads");
    // The next note (a beat is 24 000 frames) finds it back.
    let mut back = Vec::new();
    for _ in 0..120 {
        back.extend(rig.block(256));
    }
    assert!(peak(&back) > 0.01, "it plays again");
    rig.engine.stop();
}

#[test]
fn changing_where_a_plugin_runs_moves_it_and_keeps_its_sound() {
    let _g = serial();
    let (mut rig, strip, id) = gain_rig(PluginIsolation::Within);
    let inside = rig.play(24_000, 256);
    assert_eq!(pid_of(&rig, id), std::process::id());
    rig.exec(Command::SetPluginIsolation {
        strip,
        effect: id,
        isolation: Some(PluginIsolation::Individually),
    });
    rig.settle();
    assert_ne!(pid_of(&rig, id), std::process::id(), "moved out");
    assert_eq!(rig.host.isolation(id), Some(PluginIsolation::Individually));
    let outside = rig.play(24_000, 256);
    assert_eq!(differing(&inside, &outside), 0);
    // Undo: back within, the same sound.
    rig.bus.undo().unwrap();
    rig.resync();
    rig.settle();
    assert_eq!(pid_of(&rig, id), std::process::id(), "moved back");
    let back = rig.play(24_000, 256);
    assert_eq!(differing(&inside, &back), 0);
    // The machine's default moves every plug-in that follows it.
    rig.exec(Command::SetPluginIsolation {
        strip,
        effect: id,
        isolation: None,
    });
    rig.host.default_isolation = PluginIsolation::Together;
    rig.resync();
    rig.settle();
    assert_eq!(rig.host.isolation(id), Some(PluginIsolation::Together));
}

#[test]
fn an_offline_render_through_isolated_plugins_equals_the_in_process_one() {
    let _g = serial();
    let render = |iso: PluginIsolation| {
        let (mut rig, _, _) = gain_rig(iso);
        let units = rig.host.render_units(rig.bus.project(), SR);
        let insts = rig.host.render_instruments(rig.bus.project(), SR);
        assert_eq!(units.len(), 1);
        let snapshot = std::sync::Arc::new(Snapshot::compile(rig.bus.project(), 1));
        let opts = RenderOptions {
            block_frames: 333,
            tail_seconds: 0.25,
            ..Default::default()
        };
        let out = render_with_instruments(snapshot, &opts, units, insts)
            .unwrap()
            .samples;
        rig.host.finish_render();
        out
    };
    let within = render(PluginIsolation::Within);
    let together = render(PluginIsolation::Together);
    assert!(peak(&within) > 0.01);
    assert_eq!(differing(&within, &together), 0);
}

#[test]
fn a_hang_never_blocks_the_interfaces_frames_and_is_ended() {
    let _g = serial();
    let project = demo_project();
    let strip = strip_of(&project, 0);
    let mut rig = Rig::new(project, host());
    if let Some(p) = rig.host.sandboxes() {
        p.set_hang_limit(Duration::from_millis(500));
    }
    let id = rig.insert(strip, GAIN_ID, PluginIsolation::Individually);
    rig.settle();
    rig.engine.play();
    rig.set_param(strip, id, P_FAULT, HANG_PROCESS);
    let start = Instant::now();
    let mut worst = Duration::ZERO;
    while crashes(&rig) == 0 && start.elapsed() < Duration::from_secs(10) {
        rig.processor.process(&mut vec![0f32; 512], 2);
        let t = Instant::now();
        rig.frame();
        worst = worst.max(t.elapsed());
    }
    rig.engine.stop();
    assert_eq!(crashes(&rig), 1, "the hang was ended and told");
    assert!(worst < Duration::from_millis(100), "a frame took {worst:?}");
}

#[test]
fn every_crash_is_counted_and_the_database_remembers_it() {
    let _g = serial();
    let (mut rig, strip, id) = gain_rig(PluginIsolation::Individually);
    for k in 1..=3u32 {
        rig.engine.play();
        rig.set_param(strip, id, P_FAULT, ABORT_PROCESS);
        let deadline = Instant::now() + Duration::from_secs(5);
        while (crashes(&rig) as u32) < k && Instant::now() < deadline {
            rig.block(256);
        }
        rig.engine.stop();
        assert_eq!(crashes(&rig) as u32, k);
        rig.set_param(strip, id, P_FAULT, 0.0);
        rig.host
            .reload(rig.bus.project(), &mut rig.engine, id)
            .expect("reloads");
        rig.settle();
    }
    assert_eq!(
        rig.host
            .crashes_this_session(cantus_host::Format::Clap, GAIN_ID),
        3
    );
    let record = rig
        .host
        .db
        .crash_record(cantus_host::Format::Clap, GAIN_ID)
        .expect("remembered");
    assert_eq!(record.count, 3);
}

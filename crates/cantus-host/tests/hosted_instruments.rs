//! Hosted CLAP instruments on tracks (ADR 0028), through the whole host — `PluginHost`,
//! the engine handle, a processor driven in place of the audio callback, the command bus —
//! against the repository's own test instrument (`cantus-test-clap-synth`).

mod support;
use cantus_core::*;
use cantus_engine::{render_with_instruments, RenderOptions};
use cantus_host::{HostNotice, PluginKind};
use support::*;

/// One second at 48 kHz: the chord on beat 2 and E4 on beat 4 both sound in it… almost:
/// beat 4 is at 1.5 s, so plays run to 2 s.
const FRAMES: usize = 96_000;

fn rig_with_synth() -> (Rig, TrackId) {
    let project = chord_project();
    let track = project.tracks[0].id;
    let mut rig = Rig::new(project, host_with_synth());
    rig.set_synth(track);
    rig.settle();
    (rig, track)
}

#[test]
fn the_scan_knows_it_is_an_instrument() {
    let host = host_with_synth();
    let info = synth_info(&host);
    assert_eq!(info.kind, PluginKind::Instrument);
    assert_eq!((info.main_inputs, info.main_outputs), (0, 2));
    assert_eq!(info.name, "Cantus Test Synth");
}

#[test]
fn notes_in_audio_out_sample_accurate() {
    let (mut rig, track) = rig_with_synth();
    let spec = rig.instrument(track).clone();
    assert_eq!(spec.id, SYNTH_ID);
    assert_eq!(spec.name, "Cantus Test Synth");
    assert_eq!(spec.params[&S_GAIN], 0.0, "the values it starts with are the project's");
    assert!(!spec.state.is_empty(), "and so is its state");
    let out = rig.play(FRAMES, 256);
    assert!(out.iter().all(|x| x.is_finite()));
    // Beat 2 at 120 BPM is sample 24 000; each sine starts at phase 0 there, so the first
    // sample that is not zero is the one after it.
    let first = out.chunks(2).position(|f| f[0] != 0.0).expect("it sounds");
    assert_eq!(first, 24_001, "the chord starts on its sample");
    assert!(out[..24_000 * 2].iter().all(|x| *x == 0.0));
    assert!(peak(&out) > 0.05, "a chord at velocity 100: {}", peak(&out));
    assert_eq!(rig.engine.stats().latency_samples, 0);
}

#[test]
fn every_block_size_gives_the_same_samples() {
    let (mut rig, _) = rig_with_synth();
    let reference = rig.play(FRAMES, 256);
    for block in [7, 64, 1000, 4096] {
        let out = rig.play(FRAMES, block);
        assert_eq!(differing(&out, &reference), 0, "block {block}");
    }
}

#[test]
fn an_offline_render_equals_playing_it() {
    let (mut rig, track) = rig_with_synth();
    rig.set_inst_param(track, S_GAIN, -6.0);
    rig.settle();
    let played = rig.play(FRAMES, 256);
    let units = rig.host.render_instruments(rig.bus.project(), SR);
    assert_eq!(units.len(), 1, "one fresh instance for the render");
    let snapshot = std::sync::Arc::new(Snapshot::compile(rig.bus.project(), 1));
    let opts = RenderOptions {
        block_frames: 333,
        tail_seconds: 1.0,
        ..Default::default()
    };
    let rendered = render_with_instruments(snapshot, &opts, Vec::new(), units)
        .unwrap()
        .samples;
    rig.host.finish_render();
    assert!(rendered.len() >= played.len());
    assert_eq!(differing(&played, &rendered[..played.len()]), 0, "the bounce is what was heard");
    assert!(peak(&played) > 0.02);
    assert_eq!(rig.host.hosted_instrument(track).unwrap().param_value(S_GAIN), Some(-6.0));
}

#[test]
fn a_project_value_reaches_it_and_undo_takes_it_back() {
    let (mut rig, track) = rig_with_synth();
    let loud = rig.play(FRAMES, 256);
    rig.set_inst_param(track, S_GAIN, -20.0);
    rig.settle();
    assert_eq!(rig.host.hosted_instrument(track).unwrap().param_value(S_GAIN), Some(-20.0));
    let quiet = rig.play(FRAMES, 256);
    let ratio = peak(&quiet) / peak(&loud);
    assert!((ratio - 0.1).abs() < 1e-3, "−20 dB is a tenth: {ratio}");
    rig.bus.undo();
    rig.sync();
    rig.settle();
    assert_eq!(rig.host.hosted_instrument(track).unwrap().param_value(S_GAIN), Some(0.0));
    assert_eq!(differing(&rig.play(FRAMES, 256), &loud), 0);
}

#[test]
fn its_state_round_trips_through_save_and_open() {
    let (mut rig, track) = rig_with_synth();
    rig.set_inst_param(track, S_GAIN, -4.5);
    rig.settle();
    let before = rig.play(FRAMES, 256);
    let saved = rig.host.project_for_save(rig.bus.project());
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("synth.cantus");
    cantus_project::save(&saved, &path, "test").unwrap();
    let opened = cantus_project::load(&path, true).unwrap().project;
    let plugin = opened.tracks[0].instrument.external().unwrap();
    assert_eq!(plugin.params[&S_GAIN], -4.5);
    assert!(plugin.state.bytes().starts_with(b"CTS1"));
    // Another host, another engine: the same sound.
    let mut again = Rig::new(opened, host_with_synth());
    again.settle();
    assert_eq!(again.host.hosted_instrument(track).unwrap().param_value(S_GAIN), Some(-4.5));
    assert_eq!(differing(&again.play(FRAMES, 256), &before), 0);
}

#[test]
fn a_missing_instrument_opens_plays_silence_says_so_and_keeps_its_data() {
    let mut project = chord_project();
    let mut plugin = ExternalPlugin::new(PluginFormat::Clap, "org.nowhere.synth", "Nowhere");
    plugin.vendor = "Absent".into();
    plugin.state = PluginState::new(vec![9, 8, 7, 0, 255]);
    plugin.params.insert(3, 0.1 + 0.2);
    project.tracks[0].instrument = InstrumentSpec::External(plugin.clone());
    let track = project.tracks[0].id;
    let mut rig = Rig::new(project, host_with_synth());
    rig.settle();
    assert!(rig.notices.iter().any(|n| matches!(
        n,
        HostNotice::InstrumentLoadFailed { track: t, name, .. } if *t == track && name == "Nowhere"
    )));
    assert_eq!(
        rig.host.instrument_missing(track),
        Some("not installed on this computer")
    );
    assert!(rig.host.hosted_instrument(track).is_none());
    assert_eq!(peak(&rig.play(FRAMES, 256)), 0.0, "silence, not an error");
    let saved = rig.host.project_for_save(rig.bus.project());
    assert_eq!(saved.tracks[0].instrument, InstrumentSpec::External(plugin));
}

#[test]
fn its_latency_is_compensated_against_an_inverted_twin() {
    // The chord twice: on the first track the synth adds 100 samples of latency; the twin
    // (no latency, polarity inverted on its own strip) must wait for it, and they cancel.
    let mut project = chord_project();
    let mut twin = project.tracks[0].clone();
    twin.id = TrackId(project.alloc_id());
    for c in &mut twin.clips {
        c.id = ClipId(project.alloc_id());
    }
    let twin_strip = project.strips[1].id;
    twin.strip = Some(twin_strip);
    let twin_id = twin.id;
    project.tracks.push(twin);
    project.strip_mut(twin_strip).unwrap().phase_invert = true;
    let track = project.tracks[0].id;
    let mut rig = Rig::new(project, host_with_synth());
    rig.set_synth(track);
    rig.set_synth(twin_id);
    rig.set_inst_param(track, S_LATENCY, 100.0);
    rig.settle();
    assert!(!rig.host.instrument_restarting(track));
    assert_eq!(rig.host.hosted_instrument(track).unwrap().latency(), 100);
    assert_eq!(rig.engine.stats().latency_samples, 100);
    let out = rig.play(FRAMES, 256);
    assert_eq!(peak(&out), 0.0, "the twins cancel exactly");
    rig.exec(Command::SetStripMute {
        id: twin_strip,
        muted: true,
    });
    rig.settle();
    let alone = rig.play(FRAMES, 256);
    let first = alone.chunks(2).position(|f| f[0] != 0.0).expect("it sounds");
    assert_eq!(first, 24_101, "the latent path is really 100 samples late");
}

#[test]
fn a_gesture_in_its_window_is_one_undo_step() {
    let (mut rig, track) = rig_with_synth();
    // The test synth performs a gesture on Gain when Gesture is set, as its editor would.
    rig.set_inst_param(track, S_GESTURE, 3.0);
    let before = rig.bus.history().len();
    for _ in 0..20 {
        rig.block(64);
    }
    assert_eq!(rig.bus.history().len(), before + 1, "one gesture, one step");
    assert_eq!(rig.instrument(track).params[&S_GAIN], -5.0, "the gesture's last value");
    assert_eq!(rig.bus.undo_label_key(), Some("cmd.set_plugin_param"));
    for _ in 0..40 {
        rig.block(256);
    }
    assert_eq!(rig.bus.history().len(), before + 1, "nothing echoes");
    assert_eq!(rig.host.hosted_instrument(track).unwrap().param_value(S_GAIN), Some(-5.0));
    rig.bus.undo();
    rig.sync();
    rig.settle();
    assert_eq!(rig.instrument(track).params[&S_GAIN], 0.0);
    assert_eq!(rig.host.hosted_instrument(track).unwrap().param_value(S_GAIN), Some(0.0));
}

#[test]
fn the_song_position_reaches_it() {
    let (mut rig, track) = rig_with_synth();
    rig.exec(Command::SetTempo { bpm: 133.0 });
    rig.engine.play();
    for _ in 0..8 {
        rig.block(256);
    }
    let h = rig.host.hosted_instrument(track).unwrap();
    assert_eq!(h.param_value(S_TEMPO), Some(133.0));
    assert_eq!(h.param_value(S_PLAYING), Some(1.0));
    rig.engine.stop();
}

#[test]
fn stopping_releases_every_note() {
    let (mut rig, _) = rig_with_synth();
    rig.engine.seek(Ticks::ZERO);
    rig.engine.play();
    // Into the chord (beat 2 is sample 24 000), then stop while it holds.
    let mut done = 0;
    while done < 30_000 {
        rig.block(500);
        done += 500;
    }
    rig.engine.stop();
    rig.block(256);
    // The release is 480 samples: after it, exact silence.
    for _ in 0..4 {
        rig.block(256);
    }
    let after = rig.block(256);
    assert!(after.iter().all(|x| *x == 0.0), "no note hangs after a stop");
}

#[test]
fn switching_back_to_firmus_and_undoing_brings_its_sound_back() {
    let (mut rig, track) = rig_with_synth();
    rig.set_inst_param(track, S_GAIN, -3.0);
    rig.settle();
    let hosted = rig.play(FRAMES, 256);
    rig.exec(Command::SetInstrument {
        track,
        spec: Box::new(InstrumentSpec::default()),
    });
    rig.settle();
    assert!(rig.host.hosted_instrument(track).is_none(), "unloaded with its track");
    assert_ne!(differing(&rig.play(FRAMES, 256), &hosted), 0, "Firmus plays now");
    rig.bus.undo();
    rig.sync();
    rig.settle();
    assert_eq!(rig.host.hosted_instrument(track).unwrap().param_value(S_GAIN), Some(-3.0));
    assert_eq!(differing(&rig.play(FRAMES, 256), &hosted), 0);
}

#[cfg(windows)]
#[test]
fn its_window_opens_from_the_track_and_closes() {
    let (mut rig, track) = rig_with_synth();
    assert!(rig.host.hosted_instrument(track).unwrap().has_editor());
    rig.host
        .open_instrument_window(track, "Keys — Cantus Test Synth", None)
        .expect("the editor opens");
    assert!(rig.host.instrument_window_open(track));
    // A second open only brings it forward.
    rig.host
        .open_instrument_window(track, "Keys — Cantus Test Synth", None)
        .unwrap();
    assert!(rig.host.close_instrument_window(track));
    assert!(!rig.host.instrument_window_open(track));
    // And it keeps playing.
    assert!(peak(&rig.play(FRAMES, 256)) > 0.05);
}

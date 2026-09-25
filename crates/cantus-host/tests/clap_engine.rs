//! A hosted CLAP plug-in through the whole host: `PluginHost` loads it and hands it to the
//! engine, the processor runs it on its "audio thread" (this test), and the command bus
//! records what it does. The plug-in is `cantus-test-clap`, built from source.

mod support;
use cantus_core::*;

use support::*;

const FRAMES: usize = 48_000;

fn with_plugin() -> (Rig, StripId, EffectId) {
    let project = demo_project();
    let strip = strip_of(&project, 0);
    let mut rig = Rig::new(project, host_with_plugin(shared_folder()));
    let id = rig.insert_plugin(strip);
    (rig, strip, id)
}

#[test]
fn at_0_db_the_plugin_is_bit_exact_through_the_whole_engine() {
    let plain = render_plain(&demo_project(), FRAMES);
    assert!(peak(&plain) > 0.05, "the riff plays");
    let (mut rig, _, id) = with_plugin();
    assert!(rig.host.hosted(id).is_some(), "loaded");
    assert!(rig.engine.knows_effect(id), "delivered");
    rig.settle();
    let hosted = rig.play(FRAMES, 256);
    assert_eq!(hosted.len(), plain.len());
    let differing = hosted
        .iter()
        .zip(&plain)
        .filter(|(a, b)| a.to_bits() != b.to_bits())
        .count();
    assert_eq!(differing, 0, "a gain of exactly 1.0 must touch no sample");
}

#[test]
fn a_project_parameter_change_reaches_the_plugin() {
    let plain = render_plain(&demo_project(), FRAMES);
    let (mut rig, strip, id) = with_plugin();
    rig.set_param(strip, id, P_GAIN, -6.0);
    rig.settle();
    assert_eq!(rig.host.hosted(id).unwrap().param_value(P_GAIN), Some(-6.0));
    let hosted = rig.play(FRAMES, 256);
    // The plug-in scales before the strip's fader and pan law, the reference after: the
    // same product, rounded in another order, so within a rounding step or two.
    let g = 10f64.powf(-6.0 / 20.0) as f32;
    for (h, p) in hosted.iter().zip(&plain) {
        assert!(
            (h - p * g).abs() <= 4.0 * f32::EPSILON * p.abs(),
            "{h} vs {}",
            p * g
        );
    }
    assert!(peak(&hosted) > 0.02);
    // Undo is a parameter change too.
    rig.bus.undo();
    rig.sync();
    rig.settle();
    assert_eq!(rig.host.hosted(id).unwrap().param_value(P_GAIN), Some(0.0));
}

#[test]
fn a_plugin_gesture_is_one_undo_step_and_does_not_echo() {
    let (mut rig, strip, id) = with_plugin();
    rig.settle();
    // The test plug-in performs a gesture on Gain when its hidden Gesture parameter is
    // set: begin, −3, −4, −5 dB, end, one event per block — as a knob in its window would.
    rig.set_param(strip, id, P_GESTURE, 3.0);
    let before = rig.bus.history().len();
    for _ in 0..20 {
        rig.block(64);
    }
    assert_eq!(rig.bus.history().len(), before + 1, "one gesture, one step");
    assert_eq!(rig.plugin(id).params[&P_GAIN], -5.0);
    assert_eq!(rig.bus.undo_label_key(), Some("cmd.set_plugin_param"));
    // Nothing echoes back: the plug-in keeps its value, and no further step appears.
    for _ in 0..40 {
        rig.block(256);
    }
    assert_eq!(rig.bus.history().len(), before + 1);
    assert_eq!(rig.host.hosted(id).unwrap().param_value(P_GAIN), Some(-5.0));
    // Undo takes the whole gesture back, in the project and in the plug-in.
    rig.bus.undo();
    rig.sync();
    rig.settle();
    assert_eq!(rig.plugin(id).params[&P_GAIN], 0.0);
    assert_eq!(rig.host.hosted(id).unwrap().param_value(P_GAIN), Some(0.0));
}

#[test]
fn delay_compensation_lines_up_a_latent_plugin_with_an_inverted_twin() {
    // Two copies of the riff, the second on its own strip with its polarity inverted: with
    // the plug-in's latency compensated on the other path, they cancel to exact silence.
    let mut project = demo_project();
    let mut twin = project.tracks[0].clone();
    twin.id = TrackId(project.alloc_id());
    for c in &mut twin.clips {
        c.id = ClipId(project.alloc_id());
    }
    let twin_strip = project.strips[1].id;
    twin.strip = Some(twin_strip);
    project.tracks.push(twin);
    project.strip_mut(twin_strip).unwrap().phase_invert = true;
    let strip = strip_of(&project, 0);
    let mut rig = Rig::new(project, host_with_plugin(shared_folder()));
    let id = rig.insert_plugin(strip);
    // A latency change takes a restart: the host takes the plug-in out, reactivates it,
    // and the engine plans compensation again with what it now reports.
    rig.set_param(strip, id, P_LATENCY, 100.0);
    rig.settle();
    assert!(!rig.host.restarting(id));
    assert_eq!(rig.host.hosted(id).unwrap().latency(), 100);
    assert_eq!(rig.engine.stats().latency_samples, 100);
    let out = rig.play(FRAMES, 256);
    assert_eq!(peak(&out), 0.0, "the twins cancel exactly");
    // And the latent path alone is really delayed: not silence.
    rig.exec(Command::SetStripMute {
        id: twin_strip,
        muted: true,
    });
    rig.settle();
    assert!(peak(&rig.play(FRAMES, 256)) > 0.05);
}

#[test]
fn block_sizes_from_1_to_4096_give_identical_output() {
    let mut outputs = Vec::new();
    for block in [1usize, 7, 64, 1000, 4096] {
        let (mut rig, strip, id) = with_plugin();
        rig.set_param(strip, id, P_GAIN, -3.0);
        rig.set_param(strip, id, P_LATENCY, 37.0);
        rig.settle();
        assert_eq!(rig.host.hosted(id).unwrap().latency(), 37);
        outputs.push((block, rig.play(12_000, block)));
    }
    assert!(peak(&outputs[0].1) > 0.05);
    for (block, out) in &outputs[1..] {
        assert_eq!(out.len(), outputs[0].1.len());
        let differing = out
            .iter()
            .zip(&outputs[0].1)
            .filter(|(a, b)| a.to_bits() != b.to_bits())
            .count();
        assert_eq!(differing, 0, "block {block} differs from block 1");
    }
}

#[test]
fn the_sidechain_reaches_the_plugins_second_input() {
    // The plug-in's strip has no signal of its own; keyed from the riff's strip with Key
    // on, it outputs the key — the riff's strip after its fader — which then goes through
    // its own strip's pan law. So the master carries the riff, plus the riff at that gain.
    let project = demo_project();
    let riff_strip = strip_of(&project, 0);
    let keyed_strip = project.strips[1].id;
    let (gl, gr) = project.pan_law.gains(0.0);
    let plain = render_plain(&project, FRAMES);
    let mut rig = Rig::new(project.clone(), host_with_plugin(shared_folder()));
    let id = rig.insert_plugin(keyed_strip);
    rig.set_param(keyed_strip, id, P_KEY, 1.0);
    rig.exec(Command::SetEffectSidechain {
        strip: keyed_strip,
        effect: id,
        source: Some(riff_strip),
    });
    rig.settle();
    let out = rig.play(FRAMES, 256);
    assert!(peak(&plain) > 0.05);
    for (i, (o, p)) in out.iter().zip(&plain).enumerate() {
        let g = if i % 2 == 0 { gl } else { gr };
        assert_eq!(o.to_bits(), (p + p * g).to_bits(), "sample {i}");
    }
}

#[test]
fn a_missing_plugin_is_a_wire_and_keeps_its_data() {
    let mut project = demo_project();
    let strip = strip_of(&project, 0);
    let plain = render_plain(&project, FRAMES);
    let mut plugin = ExternalPlugin::new(PluginFormat::Clap, "org.example.absent", "Absent");
    plugin.vendor = "Nobody".into();
    plugin.state = PluginState::new(vec![1, 2, 3, 4, 5]);
    plugin.params.insert(7, 0.25);
    let mut slot = project.make_effect(EffectKind::External);
    slot.spec = EffectSpec::external(plugin.clone());
    let id = slot.id;
    project.strip_mut(strip).unwrap().effects.push(slot);

    let mut rig = Rig::new(project, host_with_plugin(shared_folder()));
    assert!(rig.host.missing(id).is_some(), "not installed");
    assert!(rig.host.hosted(id).is_none());
    rig.settle();
    let out = rig.play(FRAMES, 256);
    assert!(out
        .iter()
        .zip(&plain)
        .all(|(a, b)| a.to_bits() == b.to_bits()));
    // Saving writes exactly what was there.
    let saved = rig.host.project_for_save(rig.bus.project());
    let kept = saved
        .strips
        .iter()
        .find_map(|s| s.effect(id))
        .and_then(|e| e.spec.external.as_deref())
        .unwrap();
    assert_eq!(*kept, plugin);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("missing.cantus");
    cantus_project::save(&saved, &path, "test").unwrap();
    let back = cantus_project::load(&path, true).unwrap().project;
    let again = back
        .strips
        .iter()
        .find_map(|s| s.effect(id))
        .and_then(|e| e.spec.external.as_deref())
        .unwrap();
    assert_eq!(*again, plugin);
}

#[test]
fn state_round_trips_through_save_and_open() {
    let (mut rig, strip, id) = with_plugin();
    rig.set_param(strip, id, P_GAIN, -7.5);
    // A tag that is not a parameter, set through a state only the plug-in understands.
    let (state, mut params) = rig.host.capture(id).unwrap();
    let mut bytes = state.bytes().to_vec();
    let tag = b"kept across save";
    let len = bytes.len();
    bytes.truncate(len - 4);
    bytes.extend_from_slice(&(tag.len() as u32).to_le_bytes());
    bytes.extend_from_slice(tag);
    params.insert(P_GAIN, -7.5);
    rig.exec(Command::SetPluginState {
        strip,
        effect: id,
        state: PluginState::new(bytes.clone()),
        params,
    });
    rig.settle();
    let live = rig.host.capture(id).unwrap().0;
    assert_eq!(live.bytes(), &bytes[..], "the plug-in took the state");

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state.cantus");
    let saved = rig.host.project_for_save(rig.bus.project());
    cantus_project::save(&saved, &path, "test").unwrap();
    let opened = cantus_project::load(&path, true).unwrap().project;
    assert_eq!(opened, saved);

    let mut again = Rig::new(opened, host_with_plugin(shared_folder()));
    again.settle();
    let (state2, params2) = again.host.capture(id).unwrap();
    assert_eq!(
        state2.bytes(),
        &bytes[..],
        "state, tag included, round-trips"
    );
    assert_eq!(params2[&P_GAIN], -7.5);
    assert_eq!(again.plugin(id).params[&P_GAIN], -7.5);
}

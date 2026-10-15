//! A hosted CLAP plug-in's parameter driven by automation and by modulators (ADR 0057),
//! through the whole host: the values reach the plug-in **at their sample offsets inside
//! the block** (never at the block's start), the same at every block size, and a parameter
//! no longer driven goes back to the project's value. The plug-in is `cantus-test-clap`,
//! whose gain applies its events sample-accurately.

mod support;
use cantus_core::modulator::*;
use cantus_core::*;

use support::*;

/// The test plug-in's gain parameter, −60…12 dB.
const P_GAIN: u32 = 0;
/// A sixteenth at 120 BPM.
const STEP: usize = 6_000;

fn with_plugin() -> (Rig, StripId, EffectId) {
    let project = demo_project();
    let strip = strip_of(&project, 0);
    let mut rig = Rig::new(project, host_with_plugin(shared_folder()));
    let id = rig.insert_plugin(strip);
    (rig, strip, id)
}

/// The gain the test plug-in applies at a normalised position.
fn gain_at(norm: f64) -> f64 {
    10f64.powf((-60.0 + 72.0 * norm.clamp(0.0, 1.0)) / 20.0)
}

/// A step sequence on the plug-in's gain, from the lead track's Modulators panel.
fn steps_on(rig: &mut Rig, effect: EffectId, steps: &[f32], depth: f32) -> ModulatorId {
    let track = rig.bus.project().tracks[0].id;
    let id = ModulatorId(rig.bus.project().next_id + 1000);
    let mut m = Modulator::new(id, ModKind::Step);
    if let ModSource::Step(s) = &mut m.source {
        s.steps = steps.to_vec();
        s.division = DIV_SIXTEENTH;
    }
    m.routes.push(ModRoute {
        target: AutoTarget::Plugin { effect, id: P_GAIN },
        depth,
    });
    rig.exec(Command::InsertModulator {
        host: ModHost::Track(track),
        index: 0,
        modulator: Box::new(m),
    });
    id
}

/// The gain each sample was given, as the hosted output over the plain one.
fn ratios(hosted: &[f32], plain: &[f32]) -> Vec<Option<f64>> {
    hosted
        .iter()
        .zip(plain)
        .step_by(2)
        .map(|(h, p)| (p.abs() > 1e-3).then(|| f64::from(*h) / f64::from(*p)))
        .collect()
}

#[test]
fn a_step_sequence_on_a_hosted_parameter_lands_on_its_samples_at_every_block_size() {
    let frames = 8 * STEP;
    let plain = render_plain(&demo_project(), frames);
    let steps = [1.0f32, -0.5, 0.25, -1.0];
    let depth = 0.3f32;
    let mut outputs = Vec::new();
    for block in [1usize, 7, 64, 1000, 4096] {
        let (mut rig, _, id) = with_plugin();
        steps_on(&mut rig, id, &steps, depth);
        rig.settle();
        outputs.push((block, rig.play(frames, block)));
    }
    let reference = &outputs[0].1;
    for (block, out) in &outputs[1..] {
        assert_eq!(
            differing(out, reference),
            0,
            "block {block} differs from block 1"
        );
    }
    // Every step's gain, from its first sample to its last: the change lands on the step's
    // own sample, not at the start of the block it falls in.
    let r = ratios(&outputs[3].1, &plain);
    let base = 60.0 / 72.0;
    let mut checked = 0;
    for k in 0..8 {
        let want = gain_at(base + f64::from(depth * steps[k % steps.len()]));
        for f in [k * STEP, k * STEP + 1, (k + 1) * STEP - 1] {
            if let Some(g) = r[f] {
                assert!(
                    (g / want - 1.0).abs() < 1e-4,
                    "step {k}, frame {f}: gain {g} wants {want}"
                );
                checked += 1;
            }
        }
    }
    assert!(checked >= 12, "only {checked} frames had signal");
}

#[test]
fn an_automation_clip_drives_a_hosted_parameter_and_lets_go_of_it() {
    let frames = 4 * STEP * 4;
    let plain = render_plain(&demo_project(), frames);
    let (mut rig, _, id) = with_plugin();
    // −12 dB from beat 1 (a held step), the project's value before it.
    let pos: f32 = 48.0 / 72.0;
    let track = rig.bus.project().tracks[0].id;
    let mut pts = vec![AutoPoint::new(Ticks::ZERO, pos)];
    pts[0].shape = Shape::Hold;
    let mut clip = rig.bus.new_clip("gain", Ticks::beats(1), Ticks::beats(2));
    clip.automation = Some(Automation::new(
        AutoTarget::Plugin {
            effect: id,
            id: P_GAIN,
        },
        pts,
    ));
    let clip_id = clip.id;
    rig.exec(Command::InsertClip {
        track,
        clip: Box::new(clip),
    });
    rig.settle();
    let out = rig.play(frames, 1000);
    let r = ratios(&out, &plain);
    let beat = 4 * STEP;
    let want = gain_at(f64::from(pos));
    for f in [beat, beat + 1, beat + 777, 3 * beat - 1] {
        if let Some(g) = r[f] {
            assert!((g / want - 1.0).abs() < 1e-4, "frame {f}: {g}");
        }
    }
    // Before the clip the project's own value (0 dB): exactly the plain output.
    assert_eq!(
        differing(&out[..2 * (beat - 1)], &plain[..2 * (beat - 1)]),
        0
    );
    // The clip removed: the parameter is the project's again everywhere.
    rig.exec(Command::RemoveClip {
        track,
        clip: clip_id,
    });
    rig.settle();
    let after = rig.play(frames, 1000);
    assert_eq!(differing(&after, &plain), 0, "the plug-in let go of it");
}

#[test]
fn modulation_offsets_the_projects_value_and_stops_with_its_modulator() {
    let frames = 8 * STEP;
    let plain = render_plain(&demo_project(), frames);
    let (mut rig, strip, id) = with_plugin();
    rig.set_param(strip, id, P_GAIN, -12.0);
    let m = steps_on(&mut rig, id, &[0.5, -0.5], 0.2);
    rig.settle();
    let r = ratios(&rig.play(frames, 256), &plain);
    let base = 48.0 / 72.0;
    for (k, s) in [(0usize, 0.5f64), (1, -0.5), (2, 0.5)] {
        if let Some(g) = r[k * STEP + 100] {
            let want = gain_at(base + 0.2 * s);
            assert!((g / want - 1.0).abs() < 1e-4, "step {k}: {g} wants {want}");
        }
    }
    rig.exec(Command::RemoveModulator { id: m });
    rig.settle();
    let r = ratios(&rig.play(frames, 256), &plain);
    let want = gain_at(base);
    assert!(r.iter().flatten().all(|g| (g / want - 1.0).abs() < 1e-4));
}

#[test]
fn a_live_note_opens_an_envelope_on_a_hosted_parameter_while_playing() {
    // Review 1: a modulator driving only hosted parameters got no control point while
    // playing, so an envelope opened by a key played now never moved.
    let mut project = demo_project();
    let mut keys = project.make_track("Keys");
    keys.muted = true;
    let keys_id = keys.id;
    project.tracks.push(keys);
    let frames = 4 * STEP;
    let plain = render_plain(&project, frames);
    let strip = strip_of(&project, 0);
    let lead = project.tracks[0].id;
    let mut rig = Rig::new(project, host_with_plugin(shared_folder()));
    let effect = rig.insert_plugin(strip);
    let mut m = Modulator::new(
        ModulatorId(rig.bus.project().next_id + 500),
        ModKind::Envelope,
    );
    if let ModSource::Envelope(e) = &mut m.source {
        e.attack_ms = 200.0;
        e.decay_ms = 0.0;
        e.sustain = 1.0;
        e.release_ms = 200.0;
        e.trigger = EnvTrigger::Notes {
            track: Some(keys_id),
        };
    }
    let id = m.id;
    m.routes.push(ModRoute {
        target: AutoTarget::Plugin { effect, id: P_GAIN },
        depth: 0.2,
    });
    rig.exec(Command::InsertModulator {
        host: ModHost::Track(lead),
        index: 0,
        modulator: Box::new(m),
    });
    rig.settle();
    rig.engine.seek(Ticks::ZERO);
    rig.engine.play();
    let on = 4096;
    let mut out = Vec::with_capacity(frames * 2);
    let mut told = Vec::new();
    while out.len() / 2 < frames {
        if out.len() / 2 == on {
            rig.engine.note_on(keys_id, 60, 100);
        }
        out.extend(rig.block(256));
        told.push(rig.engine.modulation().value(id).unwrap_or(0.0));
    }
    rig.engine.stop();
    let r = ratios(&out, &plain);
    let base = 60.0 / 72.0;
    // Before the key: the project's value.
    for f in [1000, on - 1] {
        if let Some(g) = r[f] {
            assert!((g - 1.0).abs() < 1e-4, "frame {f}: {g}");
        }
    }
    // A quarter of the attack in, then held at the top.
    let early = r[on + 2400].expect("signal");
    assert!(
        early > 1.2 && early < gain_at(base + 0.2),
        "50 ms in: {early}"
    );
    for f in [on + 12_000, on + 15_000] {
        let g = r[f].expect("signal");
        let want = gain_at(base + 0.2);
        assert!((g / want - 1.0).abs() < 1e-3, "frame {f}: {g} wants {want}");
    }
    // The interface hears it rise too.
    let late = told[(on + 12_000) / 256];
    assert!(late > 0.99, "the panel's live value: {late}");
}

//! Automation on the audio thread (ADR 0027): absent is a null, present is block-size
//! independent and bit-identical offline, and a fader ride lands the gains it draws.

use cantus_core::automation::range::GAIN;
use cantus_core::*;
use cantus_dsp::nulltest::{assert_null, compare, peak};
use cantus_engine::*;
use std::sync::Arc;

const SR: f64 = 48_000.0;

fn pt(beats: f64, value: f32) -> AutoPoint {
    AutoPoint::new(Ticks((beats * PPQ as f64) as i64), value)
}

/// One Firmus track holding a saw for eight beats, on a strip with a saturator, and a
/// native organ on a second track playing a chord.
fn base_project() -> Project {
    let mut p = Project::new("auto");
    let mut lead = p.make_track("Lead");
    let f = lead.instrument.firmus_mut().unwrap();
    f.osc1_wave = Waveform::Saw;
    f.amp_release_s = 0.05;
    let mut clip = p.make_clip("held", Ticks::ZERO, Ticks::beats(8));
    clip.notes
        .push(p.make_note(Ticks::ZERO, Ticks::beats(8), 57, 100));
    lead.clips.push(clip);
    let strip = lead.strip.unwrap();
    p.tracks.push(lead);
    let sat = p.make_effect(EffectKind::Saturator);
    p.strip_mut(strip).unwrap().effects.push(sat);

    let mut organ = p.make_track("Organ");
    organ.instrument = InstrumentSpec::Native(NativeSpec::new(InstrumentKind::Organ));
    let mut chord = p.make_clip("chord", Ticks::beats(1), Ticks::beats(6));
    for k in [60u8, 64, 67] {
        chord
            .notes
            .push(p.make_note(Ticks::ZERO, Ticks::beats(6), k, 90));
    }
    organ.clips.push(chord);
    p.tracks.push(organ);
    p
}

fn add_auto(
    p: &mut Project,
    track: usize,
    target: AutoTarget,
    start: f64,
    len: f64,
    pts: Vec<AutoPoint>,
) {
    let c = p.make_automation_clip(
        "a",
        target,
        Ticks((start * PPQ as f64) as i64),
        Ticks((len * PPQ as f64) as i64),
        pts,
    );
    p.tracks[track].clips.push(c);
}

/// Every kind of destination at once: a fader ride, a pan sweep, an effect's drive, a
/// Firmus cutoff on a bent curve, an organ's level in steps, the master's width and level.
fn automated_project() -> Project {
    let mut p = base_project();
    let strip = p.tracks[0].strip.unwrap();
    let sat = p.strip(strip).unwrap().effects[0].id;
    let (lead, organ) = (p.tracks[0].id, p.tracks[1].id);
    add_auto(
        &mut p,
        0,
        AutoTarget::StripGain(strip),
        0.5,
        4.0,
        vec![pt(0.0, 0.3), pt(4.0, 0.8)],
    );
    add_auto(
        &mut p,
        0,
        AutoTarget::StripPan(strip),
        1.0,
        6.0,
        edit_lfo(6.0),
    );
    add_auto(
        &mut p,
        0,
        AutoTarget::Effect {
            effect: sat,
            key: "drive".into(),
        },
        2.0,
        4.0,
        vec![pt(0.0, 0.0), pt(2.0, 0.7)],
    );
    let mut bent = vec![pt(0.0, 0.2), pt(3.0, 0.9)];
    bent[0].tension = 0.6;
    add_auto(
        &mut p,
        0,
        AutoTarget::Firmus {
            track: lead,
            param: FirmusParam::Cutoff,
        },
        0.0,
        8.0,
        bent,
    );
    let mut steps = vec![pt(0.0, 0.8), pt(1.0, 0.4), pt(2.0, 0.9)];
    steps.iter_mut().for_each(|x| x.shape = Shape::Hold);
    add_auto(
        &mut p,
        1,
        AutoTarget::Instrument {
            track: organ,
            key: "gain".into(),
        },
        1.0,
        5.0,
        steps,
    );
    add_auto(
        &mut p,
        1,
        AutoTarget::MasterWidth,
        3.0,
        2.0,
        vec![pt(0.0, 0.2), pt(2.0, 0.6)],
    );
    add_auto(
        &mut p,
        1,
        AutoTarget::MasterGain,
        0.0,
        8.0,
        vec![pt(0.0, 0.8), pt(8.0, 0.7)],
    );
    add_auto(
        &mut p,
        1,
        AutoTarget::TrackPan(organ),
        2.0,
        2.0,
        vec![pt(0.0, 0.5), pt(2.0, 0.0)],
    );
    p
}

fn edit_lfo(beats: f64) -> Vec<AutoPoint> {
    cantus_core::automation::edit::lfo(
        cantus_core::automation::edit::Lfo::Sine,
        Ticks::ZERO,
        Ticks((beats * PPQ as f64) as i64),
        3,
        0.1,
        0.9,
        0,
    )
}

fn snap(p: &Project) -> Arc<Snapshot> {
    Arc::new(Snapshot::compile(p, 1))
}

fn opts(block: usize) -> RenderOptions {
    RenderOptions {
        tail_seconds: 0.25,
        block_frames: block,
        ..Default::default()
    }
}

#[test]
fn no_automation_muted_or_orphaned_renders_exactly_as_none() {
    let plain = render(snap(&base_project()), &opts(512)).unwrap();
    assert!(plain.peak > 0.05);
    // A muted clip and a clip whose effect is gone: the lanes are not there at all.
    let mut p = base_project();
    let strip = p.tracks[0].strip.unwrap();
    add_auto(
        &mut p,
        0,
        AutoTarget::StripGain(strip),
        0.0,
        4.0,
        vec![pt(0.0, 0.0)],
    );
    p.tracks[0].clips.last_mut().unwrap().muted = true;
    add_auto(
        &mut p,
        0,
        AutoTarget::Effect {
            effect: EffectId(9_999_999),
            key: "drive".into(),
        },
        0.0,
        4.0,
        vec![pt(0.0, 1.0)],
    );
    assert!(Snapshot::compile(&p, 1).automation.is_empty());
    let r = render(snap(&p), &opts(512)).unwrap();
    assert_null(&plain.samples, &r.samples, 0.0, "absent automation");
}

#[test]
fn automation_is_block_size_independent() {
    let s = snap(&automated_project());
    let reference = render(s.clone(), &opts(512)).unwrap();
    assert!(reference.peak > 0.05 && reference.peak.is_finite());
    let plain = render(snap(&base_project()), &opts(512)).unwrap();
    assert!(
        !compare(&plain.samples, &reference.samples).passes(1e-3),
        "automation must be heard"
    );
    for block in [1usize, 7, 16, 64, 333, 1024, 8192] {
        let r = render(s.clone(), &opts(block)).unwrap();
        assert_null(
            &reference.samples,
            &r.samples,
            0.0,
            &format!("block {block}"),
        );
    }
}

#[test]
fn realtime_processing_matches_the_offline_render() {
    let s = snap(&automated_project());
    let offline = render(s.clone(), &opts(256)).unwrap();
    let (mut tx, rx_engine) = rtrb::RingBuffer::<EngineMessage>::new(64);
    let (tx_engine, mut rx) = rtrb::RingBuffer::<EngineEvent>::new(4096);
    let mut p = Processor::without_midi(SR, rx_engine, tx_engine);
    let mut live = (*s).clone();
    live.loop_region = None;
    live.metronome = false;
    let live = Arc::new(live);
    for slot in live.effect_slots() {
        let unit = EffectUnit::new(slot, SR as f32);
        tx.push(EngineMessage::AddEffect(Box::new(unit))).unwrap();
    }
    for unit in cantus_engine::inst::units_for(&live, SR as f32, |_, _| false) {
        tx.push(EngineMessage::AddInstrument(unit)).unwrap();
    }
    tx.push(EngineMessage::Automation(AutomationRuntime::build(&live)))
        .unwrap();
    tx.push(EngineMessage::SetSnapshot(live)).unwrap();
    tx.push(EngineMessage::Seek(Ticks::ZERO)).unwrap();
    tx.push(EngineMessage::Play).unwrap();
    let total = offline.frames();
    let mut out = Vec::with_capacity(total * 2);
    let sizes = [128usize, 333, 512, 96, 1024, 17, 5];
    let mut i = 0;
    while out.len() / 2 < total {
        let n = sizes[i % sizes.len()].min(total - out.len() / 2);
        i += 1;
        let mut buf = vec![0f32; n * 2];
        p.process(&mut buf, 2);
        out.extend_from_slice(&buf);
        while rx.pop().is_ok() {}
    }
    assert_null(&offline.samples, &out, 0.0, "realtime vs offline");
}

/// A fader ride: a step to −6 dB, then a ramp. At every control point the output is the
/// unautomated output times the gain the curve draws there (the pan law is the same on
/// both), and between two control points it moves in a straight line.
#[test]
fn a_ramp_on_a_fader_lands_the_gains_it_draws() {
    let mut p = base_project();
    p.tracks.truncate(1);
    p.strips.iter_mut().for_each(|s| s.effects.clear());
    let strip = p.tracks[0].strip.unwrap();
    let reference = render(snap(&p), &opts(512)).unwrap();
    let minus6 = GAIN.pos(-6.0);
    let mut pts = vec![
        pt(0.0, GAIN.pos(0.0)),
        pt(1.0, minus6),
        pt(2.0, minus6),
        pt(4.0, GAIN.pos(-20.0)),
    ];
    pts[0].shape = Shape::Hold;
    add_auto(&mut p, 0, AutoTarget::StripGain(strip), 0.0, 4.0, pts);
    let lane =
        cantus_core::automation::compile::lane_for(&p, &AutoTarget::StripGain(strip)).unwrap();
    let r = render(snap(&p), &opts(512)).unwrap();
    let tps = p.tempo_bpm * PPQ as f64 / (60.0 * SR);
    let mut checked = 0;
    for frame in (0..(4.0 * PPQ as f64 / tps) as usize).step_by(16 * 37) {
        let (a, b) = (reference.samples[frame * 2], r.samples[frame * 2]);
        if a.abs() < 1e-2 {
            continue;
        }
        let want = db_to_linear(lane.value_at(frame as f64 * tps));
        assert!(
            (b / a - want).abs() < 2e-4,
            "frame {frame}: gain {} want {want}",
            b / a
        );
        checked += 1;
    }
    assert!(checked > 20, "only {checked} frames checked");
    // The held step: exactly −6 dB, the whole beat.
    let beat = (PPQ as f64 / tps) as usize;
    for frame in (beat + 32..2 * beat).step_by(101) {
        let (a, b) = (reference.samples[frame * 2], r.samples[frame * 2]);
        if a.abs() > 1e-2 {
            assert!((b / a - db_to_linear(-6.0)).abs() < 1e-4, "frame {frame}");
        }
    }
}

/// An effect's parameter before its clip is the knob's: the render is identical up to the
/// first control point inside the clip, and different after.
#[test]
fn an_effect_parameter_follows_its_clip_and_only_from_its_start() {
    let mut p = base_project();
    p.tracks.truncate(1);
    let strip = p.tracks[0].strip.unwrap();
    let sat = p.strip(strip).unwrap().effects[0].id;
    let plain = render(snap(&p), &opts(512)).unwrap();
    add_auto(
        &mut p,
        0,
        AutoTarget::Effect {
            effect: sat,
            key: "drive".into(),
        },
        2.0,
        2.0,
        vec![pt(0.0, 0.0), pt(2.0, 1.0)],
    );
    let r = render(snap(&p), &opts(512)).unwrap();
    let start = (2.0 * 60.0 / p.tempo_bpm * SR) as usize;
    let c = compare(&plain.samples, &r.samples);
    let first = c.first_diff_index.expect("the drive is heard");
    assert!(
        first / 2 >= start,
        "differs at frame {} before the clip at {start}",
        first / 2
    );
    assert!(peak(&r.samples) > 0.05);
}

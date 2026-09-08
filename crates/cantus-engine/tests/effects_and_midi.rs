//! Insert effects, hardware-MIDI routing and recording feedback.

use cantus_core::*;
use cantus_dsp::nulltest::{assert_null, peak};
use cantus_engine::*;
use std::sync::Arc;

fn project_with_effects() -> Project {
    let mut p = Project::new("fx");
    p.tempo_bpm = 120.0;
    let mut t = p.make_track("Lead");
    let mut clip = p.make_clip("c", Ticks::ZERO, Ticks::beats(4));
    for i in 0..4 {
        clip.notes
            .push(p.make_note(Ticks::beats(i), Ticks::EIGHTH, 60 + i as u8 * 3, 100));
    }
    t.clips.push(clip);
    let mut delay = p.make_effect(EffectKind::Delay);
    delay.spec.set("time_ms", 250.0).unwrap();
    delay.spec.set("mix", 0.5).unwrap();
    let reverb = p.make_effect(EffectKind::Reverb);
    t.effects.push(delay);
    t.effects.push(reverb);
    p.tracks.push(t);
    p
}

fn processor() -> (
    rtrb::Producer<EngineMessage>,
    rtrb::Consumer<EngineEvent>,
    Processor,
) {
    let (tx, rx_engine) = rtrb::RingBuffer::<EngineMessage>::new(256);
    let (tx_engine, rx) = rtrb::RingBuffer::<EngineEvent>::new(4096);
    let p = Processor::without_midi(48_000.0, rx_engine, tx_engine);
    (tx, rx, p)
}

#[test]
fn effect_chain_renders_block_size_independent_and_audibly_different() {
    let project = project_with_effects();
    let with_fx = Arc::new(Snapshot::compile(&project, 1));
    let mut dry_project = project.clone();
    dry_project.tracks[0].effects.clear();
    let dry = Arc::new(Snapshot::compile(&dry_project, 1));

    let mut opts = RenderOptions {
        tail_seconds: 1.0,
        ..Default::default()
    };
    let reference = render(with_fx.clone(), &opts).unwrap();
    let dry_render = render(dry, &opts).unwrap();
    assert!(
        reference.peak > 0.05 && reference.peak < 1.0,
        "peak {}",
        reference.peak
    );
    // The tail after the last note must be louder with delay+reverb than without.
    let tail_from = reference.frames() - 48_000 / 2;
    let tail = |r: &Rendered| peak(&r.samples[tail_from * 2..]);
    assert!(
        tail(&reference) > tail(&dry_render) * 4.0,
        "{} vs {}",
        tail(&reference),
        tail(&dry_render)
    );

    for block in [1usize, 37, 256, 4096] {
        opts.block_frames = block;
        let r = render(with_fx.clone(), &opts).unwrap();
        assert_null(
            &reference.samples,
            &r.samples,
            0.0,
            &format!("fx block {block}"),
        );
    }
}

#[test]
fn bypass_removes_the_effect_from_the_signal() {
    let project = project_with_effects();
    let mut bypassed = project.clone();
    for e in &mut bypassed.tracks[0].effects {
        e.bypass = true;
    }
    let mut dry = project.clone();
    dry.tracks[0].effects.clear();
    let opts = RenderOptions::default();
    let a = render(Arc::new(Snapshot::compile(&bypassed, 1)), &opts).unwrap();
    let b = render(Arc::new(Snapshot::compile(&dry, 1)), &opts).unwrap();
    assert_null(&a.samples, &b.samples, 0.0, "bypassed == no effects");
}

#[test]
fn removed_effects_come_back_as_garbage_and_new_ones_are_claimed() {
    let project = project_with_effects();
    let (mut tx, mut rx, mut p) = processor();
    let snap = Arc::new(Snapshot::compile(&project, 1));
    for slot in &project.tracks[0].effects {
        tx.push(EngineMessage::AddEffect(Box::new(EffectUnit::new(
            slot, 48_000.0,
        ))))
        .unwrap();
    }
    tx.push(EngineMessage::SetSnapshot(snap)).unwrap();
    let mut buf = vec![0f32; 256 * 2];
    p.process(&mut buf, 2);
    let garbage = |rx: &mut rtrb::Consumer<EngineEvent>| {
        let mut n = 0;
        while let Ok(ev) = rx.pop() {
            if matches!(ev, EngineEvent::GarbageEffect(_)) {
                n += 1;
            }
        }
        n
    };
    assert_eq!(garbage(&mut rx), 0, "claimed units must not be returned");

    // Remove one effect: exactly one unit comes back.
    let mut fewer = project.clone();
    let removed = fewer.tracks[0].effects.remove(0);
    tx.push(EngineMessage::SetSnapshot(Arc::new(Snapshot::compile(
        &fewer, 2,
    ))))
    .unwrap();
    p.process(&mut buf, 2);
    assert_eq!(garbage(&mut rx), 1);

    // A unit that no snapshot references is returned too.
    tx.push(EngineMessage::AddEffect(Box::new(EffectUnit::new(
        &removed, 48_000.0,
    ))))
    .unwrap();
    tx.push(EngineMessage::SetSnapshot(Arc::new(Snapshot::compile(
        &fewer, 3,
    ))))
    .unwrap();
    p.process(&mut buf, 2);
    assert_eq!(garbage(&mut rx), 1);
    assert_eq!(p.stats().dropped_events, 0);
}

#[test]
fn hardware_midi_plays_the_live_track_and_recording_reports_notes() {
    let mut project = Project::new("midi");
    let t = project.make_track("Live");
    let tid = t.id;
    project.tracks.push(t);
    let (mut tx, mut rx, mut p) = processor();
    tx.push(EngineMessage::SetSnapshot(Arc::new(Snapshot::compile(
        &project, 1,
    ))))
    .unwrap();
    let mut buf = vec![0f32; 480 * 2];

    // Without a live track, MIDI is ignored.
    tx.push(EngineMessage::Midi(MidiEvent::note_on(60, 100)))
        .unwrap();
    p.process(&mut buf, 2);
    assert_eq!(peak(&buf), 0.0);
    assert_eq!(p.stats().active_voices, 0);

    tx.push(EngineMessage::SetLiveTrack(Some(tid))).unwrap();
    tx.push(EngineMessage::SetRecording(true)).unwrap();
    tx.push(EngineMessage::Play).unwrap();
    tx.push(EngineMessage::Midi(MidiEvent::note_on(60, 100)))
        .unwrap();
    for _ in 0..10 {
        p.process(&mut buf, 2);
    }
    assert!(peak(&buf) > 0.01, "live note must sound");
    tx.push(EngineMessage::Midi(MidiEvent::note_off(60)))
        .unwrap();
    p.process(&mut buf, 2);

    let mut inputs = Vec::new();
    while let Ok(ev) = rx.pop() {
        if let EngineEvent::NoteInput(n) = ev {
            inputs.push(n);
        }
    }
    assert_eq!(inputs.len(), 2);
    assert_eq!(
        (inputs[0].pitch, inputs[0].velocity, inputs[0].track),
        (60, 100, tid)
    );
    assert_eq!(inputs[1].velocity, 0);
    assert!(inputs[1].tick > inputs[0].tick, "{inputs:?}");
    // 10 blocks of 480 frames at 120 bpm = 100 ms = 192 ticks.
    assert!(
        (inputs[1].tick.0 - inputs[0].tick.0 - 192).abs() <= 1,
        "{inputs:?}"
    );
}

#[test]
fn recording_off_reports_nothing() {
    let mut project = Project::new("midi");
    let t = project.make_track("Live");
    let tid = t.id;
    project.tracks.push(t);
    let (mut tx, mut rx, mut p) = processor();
    tx.push(EngineMessage::SetSnapshot(Arc::new(Snapshot::compile(
        &project, 1,
    ))))
    .unwrap();
    tx.push(EngineMessage::Play).unwrap();
    tx.push(EngineMessage::NoteOn {
        track: tid,
        pitch: 64,
        velocity: 90,
    })
    .unwrap();
    let mut buf = vec![0f32; 256 * 2];
    p.process(&mut buf, 2);
    assert!(peak(&buf) > 0.0);
    while let Ok(ev) = rx.pop() {
        assert!(!matches!(ev, EngineEvent::NoteInput(_)));
    }
}

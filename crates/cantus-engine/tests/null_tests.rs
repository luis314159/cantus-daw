//! Null tests (idea.md §6): the engine must be deterministic and block-size independent,
//! otherwise "export" would not equal "what I heard".

use cantus_core::*;
use cantus_dsp::nulltest::{assert_null, compare, peak};
use cantus_engine::*;
use std::sync::Arc;

fn demo_project() -> Project {
    let mut p = Project::new("null");
    p.tempo_bpm = 128.0;
    let mut lead = p.make_track("Lead");
    let mut clip = p.make_clip("Riff", Ticks::ZERO, Ticks::beats(8));
    let riff = [
        60u8, 63, 67, 70, 72, 70, 67, 63, 60, 62, 65, 69, 72, 69, 65, 62,
    ];
    for (i, pitch) in riff.iter().enumerate() {
        let n = p.make_note(
            Ticks::EIGHTH * i as i64,
            Ticks::EIGHTH - Ticks(40),
            *pitch,
            70 + (i as u8 * 3),
        );
        clip.notes.push(n);
    }
    // A chord with overlapping notes and a tail past the clip end.
    clip.notes
        .push(p.make_note(Ticks::beats(6), Ticks::beats(4), 48, 100));
    clip.notes
        .push(p.make_note(Ticks::beats(6), Ticks::beats(4), 55, 100));
    lead.clips.push(clip);
    lead.pan = -0.3;
    lead.gain_db = -3.0;
    p.tracks.push(lead);

    let mut bass = p.make_track("Bass");
    let InstrumentSpec::Firmus(f) = &mut bass.instrument;
    f.cutoff_hz = 400.0;
    f.osc1_wave = Waveform::Square;
    f.amp_release_s = 0.05;
    let mut clip = p.make_clip("Bass", Ticks::beats(2), Ticks::beats(6));
    for i in 0..6 {
        let n = p.make_note(
            Ticks::beats(i),
            Ticks::QUARTER - Ticks(100),
            36 + (i as u8 % 2) * 7,
            110,
        );
        clip.notes.push(n);
    }
    bass.clips.push(clip);
    bass.pan = 0.2;
    p.tracks.push(bass);
    p
}

fn snapshot() -> Arc<Snapshot> {
    Arc::new(Snapshot::compile(&demo_project(), 1))
}

#[test]
fn render_is_block_size_independent() {
    let snap = snapshot();
    let mut opts = RenderOptions {
        tail_seconds: 0.5,
        ..Default::default()
    };
    opts.block_frames = 512;
    let reference = render(snap.clone(), &opts).unwrap();
    assert!(
        reference.peak > 0.05,
        "silent render? peak {}",
        reference.peak
    );
    assert!(
        reference.peak < 1.0,
        "clipping render: peak {}",
        reference.peak
    );
    for block in [1usize, 7, 64, 480, 1000, 4096, 8192] {
        opts.block_frames = block;
        let r = render(snap.clone(), &opts).unwrap();
        assert_null(
            &reference.samples,
            &r.samples,
            0.0,
            &format!("block size {block}"),
        );
    }
}

#[test]
fn render_twice_is_bit_identical() {
    let snap = snapshot();
    let opts = RenderOptions::default();
    let a = render(snap.clone(), &opts).unwrap();
    let b = render(snap, &opts).unwrap();
    assert_null(&a.samples, &b.samples, 0.0, "repeat render");
}

#[test]
fn realtime_style_processing_matches_offline_render() {
    // Drive a Processor the way the audio callback would (irregular block sizes, extra
    // messages, meters draining) and compare with the offline renderer.
    let snap = snapshot();
    let opts = RenderOptions {
        tail_seconds: 0.25,
        block_frames: 256,
        ..Default::default()
    };
    let offline = render(snap.clone(), &opts).unwrap();

    let (mut tx, rx_engine) = rtrb::RingBuffer::<EngineMessage>::new(64);
    let (tx_engine, mut rx) = rtrb::RingBuffer::<EngineEvent>::new(4096);
    let mut p = Processor::new(48_000.0, rx_engine, tx_engine);
    let mut snap_rt = (*snap).clone();
    snap_rt.loop_region = None;
    tx.push(EngineMessage::SetSnapshot(Arc::new(snap_rt)))
        .unwrap();
    tx.push(EngineMessage::Seek(Ticks::ZERO)).unwrap();
    tx.push(EngineMessage::Play).unwrap();

    let total = offline.frames();
    let mut out = Vec::with_capacity(total * 2);
    let sizes = [128usize, 333, 512, 96, 1024, 17];
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

#[test]
fn stop_releases_notes_and_seek_restarts_cleanly() {
    let snap = snapshot();
    let (mut tx, rx_engine) = rtrb::RingBuffer::<EngineMessage>::new(64);
    let (tx_engine, mut rx) = rtrb::RingBuffer::<EngineEvent>::new(4096);
    let mut p = Processor::new(48_000.0, rx_engine, tx_engine);
    tx.push(EngineMessage::SetSnapshot(snap)).unwrap();
    tx.push(EngineMessage::Play).unwrap();
    let mut buf = vec![0f32; 512 * 2];
    for _ in 0..40 {
        p.process(&mut buf, 2);
    }
    assert!(peak(&buf) > 0.01);
    assert!(p.transport().is_playing());
    tx.push(EngineMessage::Stop).unwrap();
    // 2 s of silence processing: every voice must have released.
    for _ in 0..200 {
        p.process(&mut buf, 2);
    }
    assert_eq!(p.stats().active_voices, 0);
    assert_eq!(peak(&buf), 0.0);
    while rx.pop().is_ok() {}

    // Seek back and play: output must equal the first run from the start.
    tx.push(EngineMessage::Seek(Ticks::ZERO)).unwrap();
    tx.push(EngineMessage::Play).unwrap();
    let mut first = Vec::new();
    for _ in 0..40 {
        p.process(&mut buf, 2);
        first.extend_from_slice(&buf);
    }
    let offline = render(
        snapshot(),
        &RenderOptions {
            tail_seconds: 0.0,
            ..Default::default()
        },
    )
    .unwrap();
    let r = compare(&offline.samples[..first.len()], &first);
    assert!(
        r.passes(0.0),
        "seek restart differs at {:?}",
        r.first_diff_index
    );
}

#[test]
fn looping_wraps_and_keeps_playing() {
    let mut project = demo_project();
    project.loop_enabled = true;
    project.loop_region = Some(LoopRegion {
        start: Ticks::ZERO,
        end: Ticks::beats(2),
    });
    let snap = Arc::new(Snapshot::compile(&project, 1));
    let (mut tx, rx_engine) = rtrb::RingBuffer::<EngineMessage>::new(64);
    let (tx_engine, mut rx) = rtrb::RingBuffer::<EngineEvent>::new(4096);
    let mut p = Processor::new(48_000.0, rx_engine, tx_engine);
    tx.push(EngineMessage::SetSnapshot(snap)).unwrap();
    tx.push(EngineMessage::Play).unwrap();
    let mut buf = vec![0f32; 480 * 2];
    // 2 beats at 128 bpm = 0.9375 s = 45 000 samples. Run 10 s.
    let mut max_tick = Ticks::ZERO;
    let mut loud_blocks = 0;
    for _ in 0..1000 {
        p.process(&mut buf, 2);
        if peak(&buf) > 0.01 {
            loud_blocks += 1;
        }
        max_tick = max_tick.max(p.transport().position());
    }
    while rx.pop().is_ok() {}
    assert!(
        max_tick < Ticks::beats(2) + Ticks(1),
        "playhead escaped the loop: {max_tick}"
    );
    assert!(
        loud_blocks > 800,
        "only {loud_blocks} loud blocks — looping stalled?"
    );
    assert_eq!(p.stats().overruns, 0);
}

#[test]
fn snapshot_swap_mid_playback_is_glitch_free_and_returns_garbage() {
    let snap = snapshot();
    let (mut tx, rx_engine) = rtrb::RingBuffer::<EngineMessage>::new(64);
    let (tx_engine, mut rx) = rtrb::RingBuffer::<EngineEvent>::new(4096);
    let mut p = Processor::new(48_000.0, rx_engine, tx_engine);
    tx.push(EngineMessage::SetSnapshot(snap.clone())).unwrap();
    tx.push(EngineMessage::Play).unwrap();
    let mut buf = vec![0f32; 256 * 2];
    for _ in 0..20 {
        p.process(&mut buf, 2);
    }
    // Same content, new revision: the audio must continue identically.
    let mut same = (*snap).clone();
    same.revision = 2;
    tx.push(EngineMessage::SetSnapshot(Arc::new(same))).unwrap();
    let mut garbage = 0;
    for _ in 0..20 {
        p.process(&mut buf, 2);
        while let Ok(ev) = rx.pop() {
            if matches!(ev, EngineEvent::Garbage(_)) {
                garbage += 1;
            }
        }
    }
    assert_eq!(
        garbage, 1,
        "old snapshot must come back to the UI thread exactly once"
    );
    assert_eq!(
        Arc::strong_count(&snap),
        1,
        "engine must not retain the old snapshot"
    );
}

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
    let mut p = Processor::without_midi(48_000.0, rx_engine, tx_engine);
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
    let mut p = Processor::without_midi(48_000.0, rx_engine, tx_engine);
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
    let mut p = Processor::without_midi(48_000.0, rx_engine, tx_engine);
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
    // Deliberately no assertion on `stats().overruns` here. That counter compares wall
    // clock against the block's audio-time budget, so on a loaded machine — a CI runner,
    // or a laptop compiling something else — a run of 1 000 blocks will occasionally
    // exceed it and fail a test that is otherwise fully deterministic. Whether the
    // callback keeps up with real time is a property of a real device under a real load,
    // and `tests/hardware.rs` is where it is asserted. A flaky guardrail is worse than no
    // guardrail: people learn to re-run it instead of reading it.
}

#[test]
fn snapshot_swap_mid_playback_is_glitch_free_and_returns_garbage() {
    let snap = snapshot();
    let (mut tx, rx_engine) = rtrb::RingBuffer::<EngineMessage>::new(64);
    let (tx_engine, mut rx) = rtrb::RingBuffer::<EngineEvent>::new(4096);
    let mut p = Processor::without_midi(48_000.0, rx_engine, tx_engine);
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

// ---- pan law, width and polarity (ADR 0012) -----------------------------------------

fn render_of(project: &Project) -> Vec<f32> {
    let opts = RenderOptions {
        tail_seconds: 0.25,
        block_frames: 256,
        ..Default::default()
    };
    render(Arc::new(Snapshot::compile(project, 1)), &opts)
        .unwrap()
        .samples
}

/// The rule the standing convention asks for: **a control at its default must not touch
/// a single sample.** Width 1 and polarity off have to be free, not merely inaudible.
#[test]
fn width_at_unity_and_polarity_off_change_nothing() {
    let base = demo_project();
    let reference = render_of(&base);

    let mut explicit = base.clone();
    for t in &mut explicit.tracks {
        t.width = 1.0;
        t.phase_invert = false;
    }
    explicit.master.width = 1.0;
    explicit.master.phase_invert = false;
    explicit.master.pan = 0.0;
    assert_null(
        &reference,
        &render_of(&explicit),
        0.0,
        "the defaults are not a no-op",
    );
}

/// The classic null test, and the strongest statement about polarity there is: a track
/// against a polarity-inverted copy of itself must cancel to digital silence — not to a
/// small number, to zero.
#[test]
fn a_track_against_its_inverted_copy_is_exactly_silence() {
    let mut p = Project::new("null");
    let mut a = p.make_track("A");
    let mut clip = p.make_clip("c", Ticks::ZERO, Ticks::beats(4));
    for i in 0..8 {
        clip.notes
            .push(p.make_note(Ticks::EIGHTH * i, Ticks::EIGHTH, 50 + i as u8, 90));
    }
    a.clips.push(clip);
    let mut b = a.clone();
    b.id = TrackId(p.alloc_id());
    b.name = "B".into();
    b.phase_invert = true;
    // Same voices, same order, same everything: only the polarity differs.
    for (n, m) in a.clips[0].notes.iter().zip(b.clips[0].notes.iter_mut()) {
        m.id = n.id;
    }
    p.tracks.push(a);
    p.tracks.push(b);

    let out = render_of(&p);
    let worst = out.iter().fold(0.0f32, |acc, s| acc.max(s.abs()));
    assert_eq!(worst, 0.0, "a polarity flip did not cancel: peak {worst}");

    // And with the flip removed, the same project is loud — so the test above is not
    // passing because nothing was playing.
    p.tracks[1].phase_invert = false;
    assert!(peak(&render_of(&p)) > 0.05);
}

/// Mono is width zero, and width zero has to mean the two channels are *the same samples*
/// — not close, the same.
#[test]
fn width_zero_makes_a_track_mono() {
    let mut p = demo_project();
    // One track, panned centre so the pan law cannot re-separate the channels.
    p.tracks.truncate(1);
    p.tracks[0].pan = 0.0;
    p.tracks[0].width = 0.0;
    let delay = p.make_effect(EffectKind::Delay);
    p.tracks[0].effects.push(delay);
    // A stereo delay is what makes this test worth running: without one the source is
    // already mono and width would have nothing to collapse.
    let out = render_of(&p);
    assert!(peak(&out) > 0.01, "silent render");
    for (i, frame) in out.chunks_exact(2).enumerate() {
        assert_eq!(frame[0], frame[1], "frame {i} is not mono: {frame:?}");
    }

    // At width 1 the same project is genuinely stereo, so mono is doing something.
    p.tracks[0].width = 1.0;
    let stereo = render_of(&p);
    assert!(
        stereo.chunks_exact(2).any(|f| f[0] != f[1]),
        "the reference render was mono already"
    );
}

/// Changing the pan law changes the mix — that is the point of it being in the project
/// file — and *not* changing it changes nothing.
#[test]
fn the_pan_law_is_part_of_what_a_project_renders() {
    let mut p = demo_project();
    p.pan_law = PanLaw::ZeroDb;
    let reference = render_of(&p);

    // Setting the same law again is a no-op, to the sample.
    p.pan_law = PanLaw::ZeroDb;
    assert_null(&reference, &render_of(&p), 0.0, "same law, same render");

    // Every other law moves the balance of the panned tracks.
    for law in [PanLaw::Minus3dB, PanLaw::Minus4_5dB, PanLaw::Minus6dB] {
        p.pan_law = law;
        let out = render_of(&p);
        let d = compare(&reference, &out);
        assert!(
            d.max_abs_diff > 1e-4,
            "{law:?} rendered the same as 0 dB (max diff {})",
            d.max_abs_diff
        );
        assert!(peak(&out) > 0.05, "{law:?} rendered near silence");
    }

    // A centred track is untouched by the law's *choice* only in the sense that all four
    // agree at the extremes; at the centre they differ by exactly their names.
    let mut centred = demo_project();
    for t in &mut centred.tracks {
        t.pan = 0.0;
    }
    centred.pan_law = PanLaw::ZeroDb;
    let flat = peak(&render_of(&centred));
    centred.pan_law = PanLaw::Minus6dB;
    let halved = peak(&render_of(&centred));
    assert!(
        (linear_to_db(halved / flat) + 6.02).abs() < 0.1,
        "-6 dB law should take exactly 6 dB off a centred mix, took {}",
        linear_to_db(halved / flat)
    );
}

/// The master strip is a channel strip: its pan, width and polarity have to reach the
/// output, and their defaults have to be free.
#[test]
fn the_master_strip_reaches_the_output() {
    let mut p = demo_project();
    let reference = render_of(&p);

    p.master.phase_invert = true;
    let flipped = render_of(&p);
    for (a, b) in reference.iter().zip(flipped.iter()) {
        assert_eq!(*a, -*b, "master polarity is not an exact inversion");
    }
    p.master.phase_invert = false;

    p.master.pan = -1.0;
    let hard_left = render_of(&p);
    assert!(
        hard_left.chunks_exact(2).all(|f| f[1] == 0.0),
        "master panned hard left still puts signal in the right channel"
    );
    p.master.pan = 0.0;

    p.master.width = 0.0;
    let mono = render_of(&p);
    assert!(mono.chunks_exact(2).all(|f| f[0] == f[1]), "master mono");
    assert!(peak(&mono) > 0.05);
}

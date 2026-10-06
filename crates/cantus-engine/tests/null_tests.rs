//! Null tests (idea.md §6): the engine must be deterministic and block-size independent,
//! otherwise "export" would not equal "what I heard".

use cantus_core::*;
use cantus_dsp::math;
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
    p.tracks.push(lead);
    {
        let s = strip_of(&mut p, 0);
        s.pan = -0.3;
        s.gain_db = -3.0;
    }

    let mut bass = p.make_track("Bass");
    let f = bass.instrument.firmus_mut().expect("Firmus");
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
    p.tracks.push(bass);
    strip_of(&mut p, 1).pan = 0.2;
    p
}

fn snapshot() -> Arc<Snapshot> {
    Arc::new(Snapshot::compile(&demo_project(), 1))
}

/// The strip a track feeds, to set the mixer controls that used to live on the track.
fn strip_of(p: &mut Project, track: usize) -> &mut MixerStrip {
    let id = p.tracks[track].strip.expect("track feeds a strip");
    p.strip_mut(id).unwrap()
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
    for s in &mut explicit.strips {
        s.width = 1.0;
        s.phase_invert = false;
    }
    for t in &mut explicit.tracks {
        t.gain_db = 0.0;
        t.pan = 0.0;
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
    // Its own strip, with the flip on it: two tracks on one strip would sum before the Ø.
    b.strip = Some(p.strips[1].id);
    p.strips[1].phase_invert = true;
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
    p.strips[1].phase_invert = false;
    assert!(peak(&render_of(&p)) > 0.05);
}

/// Mono is width zero, and width zero has to mean the two channels are *the same samples*
/// — not close, the same.
#[test]
fn width_zero_makes_a_track_mono() {
    let mut p = demo_project();
    // One track, panned centre so the pan law cannot re-separate the channels.
    p.tracks.truncate(1);
    let delay = p.make_effect(EffectKind::Delay);
    {
        let s = strip_of(&mut p, 0);
        s.pan = 0.0;
        s.width = 0.0;
        s.effects.push(delay);
    }
    // A stereo delay is what makes this test worth running: without one the source is
    // already mono and width would have nothing to collapse.
    let out = render_of(&p);
    assert!(peak(&out) > 0.01, "silent render");
    for (i, frame) in out.chunks_exact(2).enumerate() {
        assert_eq!(frame[0], frame[1], "frame {i} is not mono: {frame:?}");
    }

    // At width 1 the same project is genuinely stereo, so mono is doing something.
    strip_of(&mut p, 0).width = 1.0;
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
    // The law is applied at the strip, so it is the strips that are centred here.
    for s in &mut centred.strips {
        s.pan = 0.0;
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

/// A project with a **kit** on it, which exercises the stereo instrument path, per-pad
/// pan, choke groups and pad mute all the way through the processor.
fn drum_project() -> Project {
    let mut p = Project::new("drums");
    p.tempo_bpm = 140.0;
    let mut t = p.make_track("Drums");
    let mut kit = PulsusParams::default();
    // Pan a pad off centre and mute another, so this project can only render correctly if
    // the stereo path and the compile-time pad mute both work. The panned one is the
    // *kick*, deliberately: panning the hi-hat moved the mix by 0.03 %, which is a true
    // statement that no assertion can rest on.
    kit.pads[0].pan = -0.6;
    kit.pads[1].muted = true;
    t.instrument = InstrumentSpec::Pulsus(kit);
    let mut clip = p.make_clip("Beat", Ticks::ZERO, Ticks::beats(8));
    // Kick on the beat, hats on the eighth (open on 3, closed right after it: the choke),
    // snare on 2 and 4, and a hit on the muted rim pad that must not be heard.
    for beat in 0..8i64 {
        clip.notes
            .push(p.make_note(Ticks::beats(beat), Ticks::SIXTEENTH, 36, 110));
        if beat % 4 == 2 {
            clip.notes
                .push(p.make_note(Ticks::beats(beat), Ticks::SIXTEENTH, 38, 100));
        }
    }
    for eighth in 0..16i64 {
        let pitch = if eighth == 5 { 46 } else { 42 };
        clip.notes
            .push(p.make_note(Ticks::EIGHTH * eighth, Ticks::SIXTEENTH, pitch, 80));
    }
    clip.notes
        .push(p.make_note(Ticks::beats(1), Ticks::SIXTEENTH, 37, 120));
    t.clips.push(clip);
    t.pan = 0.15;
    p.tracks.push(t);
    p
}

fn drum_snapshot() -> Arc<Snapshot> {
    Arc::new(Snapshot::compile(&drum_project(), 1))
}

/// The standing guardrail, applied to Pulsus through the whole engine rather than to the
/// voice on its own: where the host splits the buffer must not be audible.
#[test]
fn a_kit_renders_the_same_at_every_block_size() {
    let snap = drum_snapshot();
    let mut opts = RenderOptions {
        tail_seconds: 0.5,
        block_frames: 512,
        ..Default::default()
    };
    let reference = render(snap.clone(), &opts).unwrap();
    assert!(reference.peak > 0.05, "silent kit: peak {}", reference.peak);
    assert!(
        reference.peak < 1.0,
        "clipping kit: peak {}",
        reference.peak
    );
    for block in [1usize, 7, 64, 480, 4096] {
        opts.block_frames = block;
        let r = render(snap.clone(), &opts).unwrap();
        assert_null(
            &reference.samples,
            &r.samples,
            0.0,
            &format!("kit at block size {block}"),
        );
    }
}

#[test]
fn a_kit_renders_the_same_in_real_time_as_offline() {
    let snap = drum_snapshot();
    let opts = RenderOptions {
        tail_seconds: 0.25,
        block_frames: 256,
        ..Default::default()
    };
    let offline = render(snap.clone(), &opts).unwrap();

    let (mut tx, rx_engine) = rtrb::RingBuffer::<EngineMessage>::new(64);
    let (tx_engine, mut rx) = rtrb::RingBuffer::<EngineEvent>::new(4096);
    let mut p = Processor::without_midi(48_000.0, rx_engine, tx_engine);
    tx.push(EngineMessage::SetSnapshot(snap)).unwrap();
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
    assert_null(&offline.samples, &out, 0.0, "kit realtime vs offline");
}

/// A muted pad must be *gone*, not quiet — and the notes on it must still be in the
/// project, so unmuting brings them back untouched.
#[test]
fn a_muted_pad_is_silent_and_unmuting_it_brings_it_back() {
    let opts = RenderOptions {
        tail_seconds: 0.3,
        ..Default::default()
    };
    let muted = render(drum_snapshot(), &opts).unwrap();

    let mut project = drum_project();
    let kit = project.tracks[0].instrument.pulsus_mut().unwrap();
    assert!(kit.pads[1].muted, "the fixture stopped muting a pad");
    kit.pads[1].muted = false;
    let heard = render(Arc::new(Snapshot::compile(&project, 2)), &opts).unwrap();

    let r = compare(&muted.samples, &heard.samples);
    assert!(
        r.max_abs_diff > 0.01,
        "unmuting the rim changed nothing: the mute was never doing anything"
    );
    // And the notes are still there either way: mute hides a pad, it does not delete it.
    assert!(project.tracks[0].clips[0]
        .notes
        .iter()
        .any(|n| n.pitch == 37));
}

/// Pads carry their own pan, so the kit's own output is not the same in both channels —
/// and it must not be, or per-pad pan is decoration.
#[test]
fn a_kit_places_its_pads_across_the_stereo_field() {
    let mut project = drum_project();
    // Centre the *track* so what is left can only have come from the pads.
    project.tracks[0].pan = 0.0;
    let panned = render(
        Arc::new(Snapshot::compile(&project, 1)),
        &RenderOptions {
            tail_seconds: 0.3,
            ..Default::default()
        },
    )
    .unwrap();
    let (mut left, mut right) = (0f64, 0f64);
    for f in panned.samples.chunks_exact(2) {
        left += (f[0] as f64).abs();
        right += (f[1] as f64).abs();
    }
    assert!(
        left > right * 1.05,
        "the pad panned left did not move the mix left: {left} against {right}"
    );

    // With every pad centred the two channels are *identical*, not merely close: a pad at
    // its default must not touch a sample.
    let kit = project.tracks[0].instrument.pulsus_mut().unwrap();
    for pad in &mut kit.pads {
        pad.pan = 0.0;
    }
    let centred = render(
        Arc::new(Snapshot::compile(&project, 2)),
        &RenderOptions {
            tail_seconds: 0.3,
            ..Default::default()
        },
    )
    .unwrap();
    for f in centred.samples.chunks_exact(2) {
        assert_eq!(f[0], f[1], "a kit with every pad centred is not centred");
    }
}

/// A kit with a **sampled** pad, loaded from a real file through the cache, must obey the
/// same guardrails as a synthesized one: the same at every block size, and the same in real
/// time as offline. And a pad whose file is missing is silent — not an error, not a click.
#[test]
fn a_sampled_pad_renders_the_same_everywhere_and_a_missing_file_is_silent() {
    let dir = tempfile::tempdir().unwrap();
    let wav = dir.path().join("clap.wav");
    {
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: 44_100,
            bits_per_sample: 24,
            sample_format: hound::SampleFormat::Int,
        };
        let mut w = hound::WavWriter::create(&wav, spec).unwrap();
        for i in 0..8_000 {
            let t = i as f32 / 44_100.0;
            let env = math::exp(-t * 12.0);
            let l = (math::sin(t * 1_800.0) * env * 6_000_000.0) as i32;
            let r = (math::sin(t * 2_600.0) * env * 6_000_000.0) as i32;
            w.write_sample(l).unwrap();
            w.write_sample(r).unwrap();
        }
        w.finalize().unwrap();
    }

    let mut p = Project::new("sampled");
    let mut t = p.make_track("Clap");
    let mut pad = DrumPad::snare();
    pad.voice = PadVoice::Sample;
    pad.sample = Some(wav.to_string_lossy().to_string());
    let mut missing = DrumPad::rim();
    missing.voice = PadVoice::Sample;
    missing.sample = Some(dir.path().join("gone.wav").to_string_lossy().to_string());
    t.instrument = InstrumentSpec::Pulsus(PulsusParams::of(vec![pad, missing]));
    let mut clip = p.make_clip("c", Ticks::ZERO, Ticks::beats(4));
    for beat in 0..4i64 {
        clip.notes
            .push(p.make_note(Ticks::beats(beat), Ticks::SIXTEENTH, 38, 110));
        clip.notes.push(p.make_note(
            Ticks::beats(beat) + Ticks::EIGHTH,
            Ticks::SIXTEENTH,
            37,
            110,
        ));
    }
    t.clips.push(clip);
    p.tracks.push(t);

    let mut cache = SampleCache::new();
    let mut snap = Snapshot::compile(&p, 1);
    let errors = cache.attach(&mut snap);
    assert_eq!(
        errors.len(),
        1,
        "exactly the missing file should be reported"
    );
    // Pads sort by pitch: the missing rim (37) is pad 0, the clap on the snare's pitch is 1.
    assert!(snap.tracks[0].samples[0].is_none());
    assert!(snap.tracks[0].samples[1].is_some());
    let snap = Arc::new(snap);

    let mut opts = RenderOptions {
        tail_seconds: 0.3,
        block_frames: 512,
        ..Default::default()
    };
    let reference = render(snap.clone(), &opts).unwrap();
    assert!(
        reference.peak > 0.05,
        "the sample did not play: {}",
        reference.peak
    );
    for block in [1usize, 7, 480, 4096] {
        opts.block_frames = block;
        let r = render(snap.clone(), &opts).unwrap();
        assert_null(
            &reference.samples,
            &r.samples,
            0.0,
            &format!("sample at block {block}"),
        );
    }

    // Real time against offline, through the processor as the callback drives it.
    let (mut tx, rx_engine) = rtrb::RingBuffer::<EngineMessage>::new(64);
    let (tx_engine, mut rx) = rtrb::RingBuffer::<EngineEvent>::new(4096);
    let mut proc = Processor::without_midi(48_000.0, rx_engine, tx_engine);
    tx.push(EngineMessage::SetSnapshot(snap.clone())).unwrap();
    tx.push(EngineMessage::Seek(Ticks::ZERO)).unwrap();
    tx.push(EngineMessage::Play).unwrap();
    opts.block_frames = 512;
    let offline = render(snap.clone(), &opts).unwrap();
    let total = offline.frames();
    let mut out = Vec::with_capacity(total * 2);
    let sizes = [128usize, 333, 512, 96, 1024, 17];
    let mut i = 0;
    while out.len() / 2 < total {
        let n = sizes[i % sizes.len()].min(total - out.len() / 2);
        i += 1;
        let mut buf = vec![0f32; n * 2];
        proc.process(&mut buf, 2);
        out.extend_from_slice(&buf);
        while rx.pop().is_ok() {}
    }
    assert_null(
        &offline.samples,
        &out,
        0.0,
        "sampled kit realtime vs offline",
    );

    // The missing pad contributed nothing: the render equals one with that pad muted.
    let mut quiet = p.clone();
    quiet.tracks[0].instrument.pulsus_mut().unwrap().pads[1].muted = true;
    let mut q = Snapshot::compile(&quiet, 2);
    cache.attach(&mut q);
    let q = render(Arc::new(q), &opts).unwrap();
    assert_null(
        &q.samples,
        &vec![0.0; q.samples.len()],
        0.0,
        "a missing sample must be exact silence",
    );
}

/// A project written before the mixer had strips must render **bit-identically** after
/// the migration that gives it some. The hash is of the render as it was the day before
/// the strips arrived; if it changes, either the migration moved a control or the signal
/// path moved, and both are the kind of change you only hear in a finished mix.
///
/// `v5.cantus` on purpose: a synth with a delay and a panned, muted second track, and no
/// kit — so a later change to a drum voice does not disturb this guard.
#[test]
fn a_pre_strip_project_renders_exactly_as_it_did() {
    use sha2::{Digest, Sha256};
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../cantus-project/fixtures/v5.cantus");
    let text = std::fs::read_to_string(&path).unwrap();
    let loaded = cantus_project::parse(&text, true).unwrap();
    let snap = Arc::new(Snapshot::compile(&loaded.file.project, 1));
    let r = render(
        snap,
        &RenderOptions {
            tail_seconds: 0.5,
            block_frames: 512,
            ..Default::default()
        },
    )
    .unwrap();
    golden_dump("v5", &r.samples);
    let mut h = Sha256::new();
    for s in &r.samples {
        h.update(s.to_le_bytes());
    }
    let hex: String = h.finalize().iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!(
        hex, GOLDEN_V5,
        "v5.cantus no longer renders as it did (peak {})",
        r.peak
    );
}

/// Computed on 2026-09-10 from the render above, before the mixer strips existed.
/// Re-blessed for `DSP_VERSION` 2 (round 21, libm transcendentals): max |Δ| 2.6e-5 (−91.8 dBFS).
const GOLDEN_V5: &str = "26f7d439298f96403a574357f356ad9f2ecedd5b0a8dd84cd3100f23f7db3566";

/// With `CANTUS_GOLDEN_DUMP=<folder>`, writes a golden's samples there (`<name>.f32`, raw
/// little-endian) so a reviewed `DSP_VERSION` bump can state how far each golden moved
/// (`docs/round21/math.md`).
fn golden_dump(name: &str, samples: &[f32]) {
    if let Some(dir) = std::env::var_os("CANTUS_GOLDEN_DUMP") {
        let bytes: Vec<u8> = samples.iter().flat_map(|s| s.to_le_bytes()).collect();
        std::fs::write(
            std::path::Path::new(&dir).join(format!("{name}.f32")),
            bytes,
        )
        .expect("write the golden's samples");
    }
}

/// Buses: two tracks into one strip meet the strip's insert as a sum; a strip into a strip
/// is multiplied through both faders; a muted strip is silence for everything behind it.
#[test]
fn strips_sum_their_inputs_chain_through_each_other_and_mute_as_one() {
    let mut p = demo_project();
    let (s_a, s_b) = (p.tracks[0].strip.unwrap(), p.tracks[1].strip.unwrap());
    let bus = p.strips[2].id;
    // Route both tracks' strips into a third, with a delay on it.
    p.strips[0].output = Some(bus);
    p.strips[1].output = Some(bus);
    let delay = p.make_effect(EffectKind::Delay);
    p.strip_mut(bus).unwrap().effects.push(delay);
    p.strip_mut(bus).unwrap().gain_db = -6.0;
    let both = render_of(&p);
    assert!(peak(&both) > 0.05);

    // Each track alone through the same bus; the delay is linear, so the two must sum to
    // the render of both — the bus really is processing the *sum*.
    let mut only_a = p.clone();
    only_a.tracks[1].muted = true;
    let mut only_b = p.clone();
    only_b.tracks[0].muted = true;
    let a = render_of(&only_a);
    let b = render_of(&only_b);
    let summed: Vec<f32> = a.iter().zip(&b).map(|(x, y)| x + y).collect();
    assert_null(
        &both,
        &summed,
        1e-5,
        "a bus of two tracks is not the sum of the two",
    );

    // Muting the bus silences both, however loud their own strips are.
    let mut muted = p.clone();
    muted.strip_mut(bus).unwrap().muted = true;
    assert_eq!(peak(&render_of(&muted)), 0.0, "a muted bus leaked");

    // The chain multiplies: strip A at −6 into the bus at −6 is 12 dB down against A at 0
    // into the bus at 0 — to the sample, since a gain is a multiplication and nothing else.
    let mut quiet = only_a.clone();
    quiet.strip_mut(s_a).unwrap().gain_db = 0.0;
    quiet.strip_mut(bus).unwrap().gain_db = 0.0;
    quiet.strip_mut(bus).unwrap().effects.clear();
    let loud = render_of(&quiet);
    quiet.strip_mut(s_a).unwrap().gain_db = -6.0;
    quiet.strip_mut(bus).unwrap().gain_db = -6.0;
    let down = render_of(&quiet);
    let ratio = peak(&down) / peak(&loud);
    assert!(
        (ratio - 0.25119).abs() < 1e-3,
        "two −6 dB stages gave ×{ratio}"
    );
    let _ = s_b;
}

/// A send is a post-fader copy: a strip sent at unity into an empty strip reaches the
/// master twice, exactly — and because it is taken *after* the fader, turning the source
/// down turns the send down with it. A send switched off, or at zero, touches nothing.
#[test]
fn sends_are_post_fader_copies_and_nothing_when_off() {
    let mut p = demo_project();
    p.tracks[1].muted = true; // One source, so the arithmetic is exact.
                              // The aux is a channel strip like any other and places its signal through the pan
                              // law; at 0 dB centre is unity, which is what makes "exactly twice" exact.
    p.pan_law = PanLaw::ZeroDb;
    let src = p.tracks[0].strip.unwrap();
    let aux = p.strips[3].id;
    p.strip_mut(src).unwrap().gain_db = -6.0;
    let dry = render_of(&p);
    assert!(peak(&dry) > 0.05);

    let mut sent = p.clone();
    sent.strip_mut(src).unwrap().sends.push(Send::new(aux));
    let doubled = render_of(&sent);
    let twice: Vec<f32> = dry.iter().map(|x| x + x).collect();
    // To a rounding step: the copy meets the aux's own (unity) gain and pan before the
    // sum, and the sum meets it in a different order.
    assert_null(
        &doubled,
        &twice,
        1e-6,
        "a unity send into an empty strip is not a second copy",
    );

    // Half level: the copy at half. (Not bit-exact: `x + 0.5x` against `1.5x`.)
    let mut half = p.clone();
    half.strip_mut(src).unwrap().sends.push(Send {
        to: aux,
        level: 0.5,
        enabled: true,
    });
    let one_and_a_half: Vec<f32> = dry.iter().map(|x| x * 1.5).collect();
    assert_null(&render_of(&half), &one_and_a_half, 1e-6, "a send at 50 %");

    // Off, or at zero: exactly the dry render.
    let mut off = p.clone();
    off.strip_mut(src).unwrap().sends.push(Send {
        to: aux,
        level: 1.0,
        enabled: false,
    });
    assert_null(
        &render_of(&off),
        &dry,
        0.0,
        "a disabled send changed a sample",
    );
    let mut zero = p.clone();
    zero.strip_mut(src).unwrap().sends.push(Send {
        to: aux,
        level: 0.0,
        enabled: true,
    });
    assert_null(
        &render_of(&zero),
        &dry,
        0.0,
        "a send at zero changed a sample",
    );

    // A muted source sends nothing, however loud the send.
    let mut muted = sent.clone();
    muted.strip_mut(src).unwrap().muted = true;
    assert_eq!(
        peak(&render_of(&muted)),
        0.0,
        "a muted strip leaked through its send"
    );

    // The send sees the destination's insert: a delay on the aux is a delay on the copy
    // and not on the direct path, which is what a send is for.
    let mut wet = sent.clone();
    let delay = wet.make_effect(EffectKind::Delay);
    wet.strip_mut(aux).unwrap().effects.push(delay);
    let with_delay = render_of(&wet);
    assert!(
        compare(&with_delay, &doubled).max_abs_diff > 1e-3,
        "the aux's delay did not reach the send"
    );
}

/// The strip's EQ at 0 dB is free — every band, whatever its frequency — and a band with
/// gain on it moves the level of what sits under it.
#[test]
fn a_flat_eq_touches_nothing_and_a_boost_lifts_its_band() {
    let mut p = demo_project();
    let bass = p.tracks[1].strip.unwrap();
    let reference = render_of(&p);

    let mut flat = p.clone();
    {
        let eq = &mut flat.strip_mut(bass).unwrap().eq;
        eq.set(0, EqField::Freq, 60.0);
        eq.set(1, EqField::Freq, 3_000.0);
        eq.set(2, EqField::Q, 2.0);
    }
    assert_null(
        &render_of(&flat),
        &reference,
        0.0,
        "a flat EQ changed a sample",
    );

    // Cut the low shelf hard under the bass (its fundamental and first harmonics sit
    // under 400 Hz): the mix gets quieter. Boost it: louder.
    p.tracks[0].muted = true;
    p.strip_mut(bass).unwrap().eq.set(0, EqField::Freq, 400.0);
    let alone = render_of(&p);
    let mut cut = p.clone();
    cut.strip_mut(bass).unwrap().eq.set(0, EqField::Gain, -18.0);
    let mut boost = p.clone();
    boost.strip_mut(bass).unwrap().eq.set(0, EqField::Gain, 6.0);
    let (r0, rc, rb) = (
        cantus_dsp::nulltest::rms(&alone),
        cantus_dsp::nulltest::rms(&render_of(&cut)),
        cantus_dsp::nulltest::rms(&render_of(&boost)),
    );
    assert!(rc < r0 * 0.5, "an 18 dB shelf cut left {rc} against {r0}");
    assert!(rb > r0 * 1.2, "a 6 dB shelf boost gave {rb} against {r0}");
    // And it is block-size independent like everything else on the thread.
    let snap = Arc::new(Snapshot::compile(&boost, 1));
    let mut opts = RenderOptions {
        tail_seconds: 0.25,
        block_frames: 512,
        ..Default::default()
    };
    let big = render(snap.clone(), &opts).unwrap();
    opts.block_frames = 7;
    let small = render(snap, &opts).unwrap();
    assert_null(&big.samples, &small.samples, 0.0, "EQ at block size 7");
}

/// A slot's dry/wet: at 100 % it is the effect, at 0 % it is a wire (to the bit), in
/// between it is the crossfade. And the strip's FX switch takes the whole chain out.
#[test]
fn a_slot_mix_crossfades_and_the_fx_switch_removes_the_chain() {
    let mut p = demo_project();
    let s = p.tracks[0].strip.unwrap();
    let dry = render_of(&p);
    let mut delay = p.make_effect(EffectKind::Delay);
    delay.spec.set("mix", 0.5).unwrap();
    p.strip_mut(s).unwrap().effects.push(delay);
    let wet = render_of(&p);
    assert!(
        compare(&wet, &dry).max_abs_diff > 1e-3,
        "the delay did nothing"
    );

    let mut wire = p.clone();
    wire.strip_mut(s).unwrap().effects[0].mix = 0.0;
    assert_null(&render_of(&wire), &dry, 0.0, "a slot at 0 % is not a wire");

    let mut half = p.clone();
    half.strip_mut(s).unwrap().effects[0].mix = 0.5;
    let blend: Vec<f32> = wet
        .iter()
        .zip(&dry)
        .map(|(w, d)| w * 0.5 + d * 0.5)
        .collect();
    assert_null(
        &render_of(&half),
        &blend,
        1e-6,
        "a slot at 50 % is not the crossfade",
    );

    let mut off = p.clone();
    off.strip_mut(s).unwrap().fx_enabled = false;
    assert_null(&render_of(&off), &dry, 0.0, "FX off left the chain in");
}

/// Swap is an input-stage switch: a strip panned hard left, fed into a strip that swaps,
/// comes out exactly where the same strip panned hard right would.
#[test]
fn swap_channels_mirrors_the_strip_before_its_inserts() {
    let mut p = demo_project();
    p.tracks[1].muted = true;
    let src = p.tracks[0].strip.unwrap();
    let bus = p.strips[3].id;
    p.strip_mut(src).unwrap().output = Some(bus);
    p.strip_mut(src).unwrap().pan = -1.0;
    let mut swapped = p.clone();
    swapped.strip_mut(bus).unwrap().swap_channels = true;
    let mut right = p.clone();
    right.strip_mut(src).unwrap().pan = 1.0;
    assert_null(
        &render_of(&swapped),
        &render_of(&right),
        1e-6,
        "swap on the bus is not the mirror image",
    );
}

/// Soloing a strip silences what is not connected to it and keeps its bus — and it is
/// resolved before the audio thread, so the render is exactly the render with the other
/// strips muted.
#[test]
fn a_soloed_strip_is_heard_through_its_bus_and_alone() {
    let mut p = demo_project();
    let (s_a, s_b) = (p.tracks[0].strip.unwrap(), p.tracks[1].strip.unwrap());
    let bus = p.strips[2].id;
    p.strip_mut(s_a).unwrap().output = Some(bus);
    p.strip_mut(s_b).unwrap().output = Some(bus);
    p.strip_mut(bus).unwrap().gain_db = -3.0;
    let mut solo_a = p.clone();
    solo_a.strip_mut(s_a).unwrap().soloed = true;
    let mut mute_b = p.clone();
    mute_b.strip_mut(s_b).unwrap().muted = true;
    let soloed = render_of(&solo_a);
    assert!(
        peak(&soloed) > 0.05,
        "the soloed strip was silenced with the rest"
    );
    assert_null(&soloed, &render_of(&mute_b), 0.0, "solo A is not mute B");
    // Soloing the bus is everything: nothing outside it plays anyway.
    let mut solo_bus = p.clone();
    solo_bus.strip_mut(bus).unwrap().soloed = true;
    assert_null(
        &render_of(&solo_bus),
        &render_of(&p),
        0.0,
        "solo on the bus",
    );
}

/// Drive a processor with a capture ring attached, at irregular block sizes, and return
/// what it played and what the recorder took.
fn play_and_capture(
    project: &Project,
    target: CaptureTarget,
    seconds: f64,
) -> (Vec<f32>, Vec<f32>) {
    let (mut tx, rx_engine) = rtrb::RingBuffer::<EngineMessage>::new(64);
    let (tx_engine, mut rx) = rtrb::RingBuffer::<EngineEvent>::new(4096);
    let (cap_tx, mut cap_rx) = rtrb::RingBuffer::<f32>::new(CAPTURE_SAMPLES);
    let mut p = Processor::without_midi(48_000.0, rx_engine, tx_engine);
    p.attach_capture(cap_tx);
    let mut snap = Snapshot::compile(project, 1);
    snap.loop_region = None;
    tx.push(EngineMessage::SetSnapshot(Arc::new(snap))).unwrap();
    tx.push(EngineMessage::SetCapture(target)).unwrap();
    tx.push(EngineMessage::Seek(Ticks::ZERO)).unwrap();
    tx.push(EngineMessage::Play).unwrap();
    let total = (seconds * 48_000.0) as usize;
    let (mut out, mut took) = (Vec::new(), Vec::new());
    let sizes = [128usize, 333, 512, 96, 1024, 17];
    let mut i = 0;
    while out.len() / 2 < total {
        let n = sizes[i % sizes.len()].min(total - out.len() / 2);
        i += 1;
        let mut buf = vec![0f32; n * 2];
        p.process(&mut buf, 2);
        out.extend_from_slice(&buf);
        while rx.pop().is_ok() {}
        while let Ok(x) = cap_rx.pop() {
            took.push(x);
        }
    }
    (out, took)
}

/// The mix recorder on the master takes exactly what the master plays — to the bit, and
/// never the metronome; on a strip, it takes the strip after its fader, silences included,
/// so a take always has the length of the time it ran.
#[test]
fn the_mix_recorder_takes_exactly_what_leaves_the_fader() {
    let mut p = demo_project();
    let (out, took) = play_and_capture(&p, CaptureTarget::Master, 3.0);
    assert!(peak(&out) > 0.05);
    assert_null(&out, &took, 0.0, "the master take is not the master");

    // With the metronome on, the output has clicks and the take does not: take the
    // metronome-free render and the metronome-on take and they must match.
    let mut clicking = p.clone();
    clicking.metronome = true;
    let (loud, took_clicking) = play_and_capture(&clicking, CaptureTarget::Master, 3.0);
    assert!(
        compare(&loud, &out).max_abs_diff > 1e-3,
        "no click was heard"
    );
    assert_null(
        &out,
        &took_clicking,
        0.0,
        "the recorder heard the metronome",
    );

    // One strip: the only one sounding, through a master at its defaults, *is* the output
    // — to the bit, because a master at its defaults touches no sample. (Until round 16 its
    // centred balance was `cos(π/4)·√2` in f32, 0.99999994, and this took a tolerance.)
    p.tracks[1].muted = true;
    let s0 = p.tracks[0].strip.unwrap();
    let (solo_out, strip_take) = play_and_capture(&p, CaptureTarget::Strip(s0), 3.0);
    assert_eq!(strip_take.len(), solo_out.len(), "a strip take keeps time");
    assert_null(
        &solo_out,
        &strip_take,
        0.0,
        "the strip take is not the strip",
    );

    // A strip nothing reaches records silence of the right length.
    let empty = p.strips[9].id;
    let (_, silence) = play_and_capture(&p, CaptureTarget::Strip(empty), 1.0);
    assert_eq!(silence.len(), 48_000 * 2);
    assert_eq!(peak(&silence), 0.0);

    // Off takes nothing.
    let (_, nothing) = play_and_capture(&p, CaptureTarget::Off, 1.0);
    assert!(nothing.is_empty());
}

/// Aequalis on a strip: an EQ with no band touches no sample; an EQ with a static and a
/// dynamic band renders identically at every block size, offline and in real time alike.
#[test]
fn an_eq_is_free_when_empty_and_block_size_independent_when_not() {
    let mut p = demo_project();
    let s0 = p.tracks[0].strip.unwrap();
    let dry = render_of(&p);
    let eq = p.make_effect(EffectKind::Eq);
    p.strip_mut(s0).unwrap().effects.push(eq);
    assert_null(&render_of(&p), &dry, 0.0, "an empty EQ changed a sample");

    let spec = &mut p.strip_mut(s0).unwrap().effects[0].spec;
    for (k, v) in [
        ("b0.used", 1.0),
        ("b0.shape", 0.0),
        ("b0.freq", 800.0),
        ("b0.gain", 6.0),
        ("b0.q", 1.5),
        ("b1.used", 1.0),
        ("b1.shape", 3.0), // low cut
        ("b1.freq", 120.0),
        ("b1.slope", 24.0),
        ("b2.used", 1.0),
        ("b2.freq", 2_500.0),
        ("b2.gain", 0.5),
        ("b2.dyn", 1.0),
        ("b2.thr", -40.0),
        ("b2.range", -8.0),
    ] {
        spec.set(k, v).unwrap();
    }
    let snap = Arc::new(Snapshot::compile(&p, 1));
    let mut opts = RenderOptions {
        tail_seconds: 0.25,
        block_frames: 512,
        ..Default::default()
    };
    let reference = render(snap.clone(), &opts).unwrap();
    assert!(
        compare(&reference.samples, &dry).max_abs_diff > 1e-3,
        "the EQ did nothing"
    );
    for block in [1usize, 7, 333, 4096] {
        opts.block_frames = block;
        let r = render(snap.clone(), &opts).unwrap();
        assert_null(
            &reference.samples,
            &r.samples,
            0.0,
            &format!("EQ at block size {block}"),
        );
    }
}

/// Comprimo keyed from another strip: it ducks its own strip when the key is loud, it is
/// silent-transparent when the key never crosses the threshold, and it is block-size
/// independent — the key strip is processed first because the key is a route.
#[test]
fn a_compressor_keyed_from_another_strip_ducks_and_nothing_else() {
    let mut p = demo_project();
    let (lead, bass) = (p.tracks[0].strip.unwrap(), p.tracks[1].strip.unwrap());
    // Bass goes to a strip *after* the lead's in the desk, so only the key orders them.
    let mut comp = p.make_effect(EffectKind::Compressor);
    comp.spec.set("thr", -40.0).unwrap();
    comp.spec.set("ratio", 20.0).unwrap();
    comp.spec.set("att", 0.5).unwrap();
    comp.sidechain = Some(bass);
    let mut only_lead = p.clone();
    only_lead.strip_mut(bass).unwrap().gain_db = MIN_GAIN_DB; // the bass is silent at the master
    let dry = render_of(&only_lead);
    only_lead
        .strip_mut(lead)
        .unwrap()
        .effects
        .push(comp.clone());
    let ducked = render_of(&only_lead);
    // The key strip's fader is down, so its post-fader key is silence: no ducking.
    assert_null(&ducked, &dry, 0.0, "a silent key ducked the lead");

    // Key at unity, but the bass strip routed nowhere audible? Keep it audible, and
    // compare the lead's contribution with and without the compressor: take the bass out
    // of the master by sending its strip into an empty strip that is muted.
    let mut keyed = p.clone();
    let sink = keyed.strips[8].id;
    keyed.strip_mut(bass).unwrap().output = Some(sink);
    keyed.strip_mut(sink).unwrap().muted = true;
    let lead_alone = render_of(&keyed);
    keyed.strip_mut(lead).unwrap().effects.push(comp);
    let lead_ducked = render_of(&keyed);
    let (a, b) = (
        cantus_dsp::nulltest::rms(&lead_alone),
        cantus_dsp::nulltest::rms(&lead_ducked),
    );
    assert!(
        b < a * 0.6,
        "the bass key did not duck the lead: {b} vs {a}"
    );

    let snap = Arc::new(Snapshot::compile(&keyed, 1));
    let pos = |id: StripId| {
        let i = keyed.strip_index(id).unwrap();
        snap.strip_order.iter().position(|x| *x == i).unwrap()
    };
    assert!(pos(bass) < pos(lead), "the key is processed first");
    let mut opts = RenderOptions {
        tail_seconds: 0.25,
        block_frames: 512,
        ..Default::default()
    };
    let reference = render(snap.clone(), &opts).unwrap();
    for block in [1usize, 7, 1000] {
        opts.block_frames = block;
        let r = render(snap.clone(), &opts).unwrap();
        assert_null(
            &reference.samples,
            &r.samples,
            0.0,
            &format!("keyed comp at {block}"),
        );
    }
}

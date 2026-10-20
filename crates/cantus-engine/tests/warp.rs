//! The warp engine (round 29, ADR 0061): the analysis at import (tempo, beats, transients,
//! the WAV `acid` chunk) cached on disk, the render of a warped clip played in its source's
//! place — under the tempo map, a marker on its tick to the sample — the identity to the
//! bit, offline = real time, and what plays while a render is on its way.

use cantus_core::tempo::TempoPoint;
use cantus_core::warp::{Detected, Warp, WarpMode};
use cantus_core::*;
use cantus_dsp::beat::testkit::{click_track, drum_loop};
use cantus_dsp::nulltest::assert_null;
use cantus_engine::warp::WarpService;
use cantus_engine::*;
use std::path::Path;
use std::sync::Arc;

fn write_wav(path: &Path, x: &[f32], rate: u32) {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut w = hound::WavWriter::create(path, spec).unwrap();
    for s in x {
        w.write_sample(*s).unwrap();
    }
    w.finalize().unwrap();
}

/// Append an `acid` chunk saying `bpm` (and fix the RIFF size).
fn add_acid(path: &Path, bpm: f32) {
    let mut bytes = std::fs::read(path).unwrap();
    let mut chunk = Vec::new();
    chunk.extend_from_slice(b"acid");
    chunk.extend_from_slice(&24u32.to_le_bytes());
    chunk.extend_from_slice(&0u32.to_le_bytes()); // flags
    chunk.extend_from_slice(&60u16.to_le_bytes()); // root note
    chunk.extend_from_slice(&0x8000u16.to_le_bytes());
    chunk.extend_from_slice(&0f32.to_le_bytes());
    chunk.extend_from_slice(&16u32.to_le_bytes()); // beats
    chunk.extend_from_slice(&4u16.to_le_bytes());
    chunk.extend_from_slice(&4u16.to_le_bytes());
    chunk.extend_from_slice(&bpm.to_le_bytes());
    bytes.extend_from_slice(&chunk);
    let riff = (bytes.len() - 8) as u32;
    bytes[4..8].copy_from_slice(&riff.to_le_bytes());
    std::fs::write(path, bytes).unwrap();
}

/// A project at `bpm` playing `path` (mono, `frames` long at `rate`) from beat 0 for
/// `beats`, its clip warped by `warp`.
fn project(
    path: &Path,
    rate: u32,
    frames: usize,
    bpm: f64,
    beats: i64,
    warp: Option<Warp>,
) -> Project {
    let mut p = Project::new("warp");
    p.tempo_bpm = bpm;
    let mut t = p.make_track("Loop");
    t.strip = None;
    let id = AudioSourceId(p.alloc_id());
    p.audio_pool.push(AudioSource {
        id,
        name: "loop".into(),
        path: path.to_string_lossy().into_owned(),
        sample_rate: rate,
        channels: 1,
        frames: frames as u64,
        sha256: None,
        recorded: false,
    });
    let mut c = p.make_audio_clip("loop", id, Ticks::ZERO, Ticks::beats(beats));
    c.audio.as_mut().unwrap().warp = warp.map(Box::new);
    t.clips.push(c);
    p.tracks.push(t);
    p
}

/// Offline, at `rate`, a manual cache rendering at once: the left channel.
fn bounce(p: &Project, rate: u32) -> Vec<f32> {
    let mut s = Snapshot::compile(p, 1);
    let mut cache = SampleCache::new();
    assert!(cache.attach_audio(&mut s, rate).is_empty());
    let opts = RenderOptions {
        sample_rate: rate,
        tail_seconds: 0.0,
        block_frames: 256,
        ..Default::default()
    };
    let r = render(Arc::new(s), &opts).unwrap();
    r.samples.chunks_exact(2).map(|f| f[0]).collect()
}

fn rise(x: &[f32], from: usize, to: usize) -> Option<usize> {
    let peak = x[from..to.min(x.len())]
        .iter()
        .fold(0.0f32, |m, v| m.max(v.abs()));
    (from..to.min(x.len())).find(|&i| x[i].abs() >= 0.25 * peak)
}

#[test]
fn the_analysis_finds_a_loops_tempo_and_hits_and_keeps_them_on_disk() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("drums 92.wav");
    let mut m = drum_loop(44_100.0, 92.0, 4, 0.0, 0.0, 7);
    let exact = (60.0 / 92.0 * 16.0 * 44_100.0_f64).round() as usize;
    m.audio.truncate(exact);
    write_wav(&path, &m.audio, 44_100);
    let cache = dir.path().join("cache");
    let svc = WarpService::new();
    svc.set_disk_cache(Some(cache.clone()));
    let a = svc.analyse_now(&path, 44_100).unwrap();
    assert_eq!(a.grid.bars, Some(4));
    assert!((a.grid.bpm - 92.0).abs() < 0.01, "{}", a.grid.bpm);
    assert!(
        a.onsets.len() >= m.onsets.len() - 1,
        "{} hits",
        a.onsets.len()
    );
    assert_eq!(svc.computed(), 1);
    // A second service finds it on disk.
    let again = WarpService::new();
    again.set_disk_cache(Some(cache));
    let b = again.analyse_now(&path, 44_100).unwrap();
    assert_eq!(again.computed(), 0, "read, not computed");
    assert_eq!(*a, *b);
}

#[test]
fn the_acid_chunk_says_the_tempo() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("loop.wav");
    let m = click_track(48_000.0, 97.5, 2, 0.0);
    write_wav(&path, &m.audio, 48_000);
    add_acid(&path, 97.5);
    let a = WarpService::new().analyse_now(&path, 48_000).unwrap();
    assert_eq!(a.acid_bpm, Some(97.5));
}

#[test]
fn an_identity_warp_plays_the_source_to_the_bit() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("loop.wav");
    let m = drum_loop(48_000.0, 120.0, 2, 0.0, 0.0, 3);
    write_wav(&path, &m.audio, 48_000);
    let plain = bounce(
        &project(&path, 48_000, m.audio.len(), 120.0, 8, None),
        48_000,
    );
    for mode in WarpMode::ALL {
        let mut w = Warp::identity(48_000, 120.0, 0);
        w.mode = mode;
        let w = w.add_marker(Ticks::beats(4)).unwrap();
        let warped = bounce(
            &project(&path, 48_000, m.audio.len(), 120.0, 8, Some(w)),
            48_000,
        );
        assert_null(&plain, &warped, 0.0, "identity warp");
    }
}

/// The brief's first "done when": a drum loop at 92 BPM in a 128 BPM project plays in
/// time — every bar's downbeat on its bar line to the sample, every hit on its beat.
#[test]
fn a_92_bpm_loop_plays_in_time_in_a_128_bpm_project() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("drums 92.wav");
    let mut m = drum_loop(44_100.0, 92.0, 4, 0.0, 0.0, 9);
    let exact = (60.0 / 92.0 * 16.0 * 44_100.0_f64).round() as usize;
    m.audio.truncate(exact);
    write_wav(&path, &m.audio, 44_100);
    let a = WarpService::new().analyse_now(&path, 44_100).unwrap();
    for mode in [WarpMode::Rhythm, WarpMode::Complex] {
        let mut w = Warp::from_grid(
            44_100,
            a.grid.bpm,
            &a.grid.beats,
            a.grid.downbeat,
            4,
            Detected {
                bpm: a.grid.bpm,
                confidence: a.grid.confidence,
                downbeat: a.grid.first_downbeat().unwrap(),
                bars: a.grid.bars,
            },
        );
        w.mode = mode;
        let out = bounce(&project(&path, 44_100, exact, 128.0, 16, Some(w)), 48_000);
        let per = 60.0 / 128.0 * 48_000.0;
        let heard = cantus_dsp::onset::detect(&out, 48_000.0).frames;
        for beat in 1..15 {
            let at = (beat as f64 * per).round() as usize;
            // The hits heard, found the way the import finds them.
            let r = heard
                .iter()
                .copied()
                .min_by(|a, b| (a - at as f64).abs().total_cmp(&(b - at as f64).abs()))
                .unwrap();
            let err = r - at as f64;
            assert!(
                err.abs() <= 48.0,
                "{mode:?}: beat {beat} heard at {r} ({err:+})"
            );
        }
    }
}

/// A marker lands on its tick to the sample, at 44.1, 48 and 96 kHz and under a ramp.
#[test]
fn a_marker_lands_on_its_tick_to_the_sample() {
    let dir = tempfile::tempdir().unwrap();
    for rate in [44_100u32, 48_000, 96_000] {
        let path = dir.path().join(format!("hit {rate}.wav"));
        // One click a second; the second one (frame `rate`) marked onto beat 3.
        let mut x = vec![0.0f32; rate as usize * 4];
        for k in 0..4 {
            x[k * rate as usize] = 0.9;
        }
        write_wav(&path, &x, rate);
        let mut w = Warp::identity(rate, 120.0, 0);
        w = w.add_marker(Ticks::beats(2)).unwrap();
        w = w.move_marker(1, Ticks::beats(3));
        w.mode = WarpMode::Rhythm;
        let mut p = project(&path, rate, x.len(), 120.0, 8, Some(w));
        for ramp in [false, true] {
            if ramp {
                p.tempo_points = vec![
                    TempoPoint {
                        tick: Ticks::ZERO,
                        bpm: 120.0,
                        ramp: true,
                    },
                    TempoPoint {
                        tick: Ticks::beats(8),
                        bpm: 150.0,
                        ramp: false,
                    },
                ];
            }
            let out = bounce(&p, rate);
            let want = p
                .tempo_map()
                .sample_at(Ticks::beats(3).0 as f64, rate as f64)
                .round() as usize;
            let got = rise(&out, want - 200, want + 200).unwrap();
            assert!(
                (got as i64 - want as i64).abs() <= 1,
                "{rate} Hz, ramp {ramp}: the marked click at {got}, its tick at {want}"
            );
        }
    }
}

#[test]
fn offline_equals_real_time_at_any_block_size() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("loop.wav");
    let m = drum_loop(48_000.0, 100.0, 2, 0.3, 0.0, 5);
    write_wav(&path, &m.audio, 48_000);
    let mut w = Warp::identity(48_000, 100.0, 0)
        .add_marker(Ticks::beats(4))
        .unwrap();
    w = w.move_marker(1, Ticks::beats(3));
    w.cents = -300;
    let p = project(&path, 48_000, m.audio.len(), 120.0, 8, Some(w));
    let mut s = Snapshot::compile(&p, 1);
    let mut cache = SampleCache::new();
    assert!(cache.attach_audio(&mut s, 48_000).is_empty());
    // A render, attached (not nothing, not the source).
    let data = s.tracks[0].audio[0].data.clone().expect("the render");
    assert_ne!(data.len_frames(), m.audio.len());
    let snap = Arc::new(s);
    let opts = RenderOptions {
        sample_rate: 48_000,
        tail_seconds: 0.0,
        block_frames: 256,
        ..Default::default()
    };
    let offline = render(snap.clone(), &opts).unwrap();
    let peak = offline.samples.iter().fold(0.0f32, |a, v| a.max(v.abs()));
    assert!(peak > 0.01, "it sounds ({peak})");
    let plain = bounce(
        &project(&path, 48_000, m.audio.len(), 120.0, 8, None),
        48_000,
    );
    let left: Vec<f32> = offline.samples.chunks_exact(2).map(|f| f[0]).collect();
    assert!(
        left.iter().zip(&plain).any(|(a, b)| (a - b).abs() > 1e-3),
        "the warp is heard, not the source"
    );
    for sizes in [
        vec![1usize],
        vec![37],
        vec![64],
        vec![512],
        vec![128, 333, 17, 1024],
    ] {
        let (mut tx, rx_engine) = rtrb::RingBuffer::<EngineMessage>::new(64);
        let (tx_engine, mut rx) = rtrb::RingBuffer::<EngineEvent>::new(4096);
        let mut proc = Processor::without_midi(48_000.0, rx_engine, tx_engine);
        tx.push(EngineMessage::SetSnapshot(snap.clone())).unwrap();
        tx.push(EngineMessage::Seek(Ticks::ZERO)).unwrap();
        tx.push(EngineMessage::Play).unwrap();
        let total = offline.frames();
        let mut out = Vec::with_capacity(total * 2);
        let mut i = 0;
        while out.len() / 2 < total {
            let n = sizes[i % sizes.len()].min(total - out.len() / 2);
            i += 1;
            let mut buf = vec![0f32; n * 2];
            proc.process(&mut buf, 2);
            out.extend_from_slice(&buf);
            while rx.pop().is_ok() {}
        }
        assert_null(&offline.samples, &out, 0.0, "real time against offline");
    }
}

fn settle(cache: &mut SampleCache) {
    let t = std::time::Instant::now();
    while cache.loading() && t.elapsed() < std::time::Duration::from_secs(120) {
        cache.poll_loaded();
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(!cache.loading());
}

/// While a new render is on its way the clip plays the newest one it had (or the bridge),
/// never its source at the wrong speed; then the new one.
#[test]
fn while_a_render_is_on_its_way_the_last_render_plays() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("loop.wav");
    let m = drum_loop(48_000.0, 100.0, 2, 0.0, 0.0, 6);
    write_wav(&path, &m.audio, 48_000);
    let base = Warp::identity(48_000, 100.0, 0)
        .add_marker(Ticks::beats(4))
        .unwrap();
    let one = base.move_marker(1, Ticks::beats(3));
    let two = base.move_marker(1, Ticks::beats(5));
    let mut cache = SampleCache::new();
    cache.set_background(true);
    cache.set_pitch_background(true);
    let attach = |cache: &mut SampleCache, w: &Warp, rev: u64| {
        let mut s = Snapshot::compile(
            &project(&path, 48_000, m.audio.len(), 120.0, 8, Some(w.clone())),
            rev,
        );
        cache.attach_audio(&mut s, 48_000);
        s.tracks[0].audio[0].data.clone()
    };
    attach(&mut cache, &one, 1);
    settle(&mut cache);
    attach(&mut cache, &one, 2);
    settle(&mut cache);
    let first = attach(&mut cache, &one, 3).expect("the first render");
    let source_len = m.audio.len();
    assert_ne!(first.len_frames(), source_len, "a render, not the source");
    // Moved: at once, the render it had — never the source, never nothing (the bridge
    // itself is held to its place, length and pitch in `warp_review.rs`).
    let meanwhile = attach(&mut cache, &two, 4).expect("something plays");
    assert!(Arc::ptr_eq(&meanwhile, &first), "the render it had");
    assert!(cache.loading(), "the new one on its way");
    settle(&mut cache);
    let second = attach(&mut cache, &two, 5).unwrap();
    assert!(!Arc::ptr_eq(&second, &first));
}

#[test]
fn an_export_waits_for_the_warp_renders() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("loop.wav");
    let m = drum_loop(48_000.0, 100.0, 2, 0.0, 0.0, 8);
    write_wav(&path, &m.audio, 48_000);
    let w = Warp::identity(48_000, 100.0, 0)
        .add_marker(Ticks::beats(4))
        .unwrap();
    let w = w.move_marker(1, Ticks::beats(3));
    let p = project(&path, 48_000, m.audio.len(), 120.0, 8, Some(w));
    let mut engine = EngineHandle::dummy();
    let snap = engine.compile_for_render(&p, 1);
    let mut manual = Snapshot::compile(&p, 1);
    SampleCache::new().attach_audio(&mut manual, engine.sample_rate.round() as u32);
    let want = manual.tracks[0].audio[0].data.clone().unwrap();
    let got = snap.tracks[0].audio[0].data.clone().unwrap();
    assert_eq!(
        got.frames[..],
        want.frames[..],
        "the final render, not a bridge"
    );
}

/// The engine's render and the interface read a warped clip through one mapping
/// (`audio_time::clip_frame_at`): sample for sample, under a ramp.
#[test]
fn the_render_and_the_interface_read_one_mapping() {
    use cantus_core::tempo::TempoMap;
    use cantus_core::time::TimeSignature;
    let map = Arc::new(TempoMap::new(
        100.0,
        TimeSignature::default(),
        &[
            TempoPoint {
                tick: Ticks::ZERO,
                bpm: 100.0,
                ramp: true,
            },
            TempoPoint {
                tick: Ticks::beats(12),
                bpm: 150.0,
                ramp: false,
            },
        ],
        &[],
    ));
    let w = Warp::identity(44_100, 92.0, 0)
        .add_marker(Ticks::beats(4))
        .unwrap();
    let w = w.move_marker(1, Ticks::beats(5));
    let mut region = AudioRegion::of(AudioSourceId(1));
    region.warp = Some(Box::new(w.clone()));
    let plan = cantus_engine::warp::Plan {
        warp: Arc::new(w),
        map: map.clone(),
        start: Ticks::beats(2),
        end: Ticks::beats(14),
        offset: Ticks::beats(1),
        slip_frames: 0.0,
        source_rate: 44_100,
        rate: 48_000,
        path: Default::default(),
        input_key: 1,
    };
    let scale = 48_000.0 / 44_100.0;
    for k in [0.0, 1.0, 12_345.0, 100_000.0, 250_000.5] {
        let t = plan.tick_at(k);
        let ui = audio_time::clip_frame_at(&map, plan.start, plan.offset, &region, 44_100.0, t);
        assert_eq!((ui * scale).to_bits(), plan.source_at(k).to_bits(), "{k}");
    }
}

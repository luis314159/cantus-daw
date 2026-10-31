//! The `separate` job around a network, in this process: segments, their padding, the
//! cross-fade, the normalisation and the files, with the fixture network whose right
//! answer is known (`spec::Family::Fixture`). The helper process around it is
//! `helper_process.rs`'s.

mod common;

use cantus_ml::jobs::{Ctx, Report, Stop};
use cantus_ml::proto::JobError;
use cantus_ml::runtime::Runtime;
use cantus_ml::spec::SeparateResult;
use common::*;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

fn run(args: cantus_ml::spec::SeparateArgs) -> (Result<SeparateResult, Stop>, Vec<(f32, String)>) {
    let mut rt = Runtime::new();
    let mut seen = Vec::new();
    let mut progress = |d: f32, s: &str| seen.push((d, s.to_string()));
    let mut log = |_: &str, _: &str| {};
    let mut ctx = Ctx {
        id: 1,
        runtime: &mut rt,
        report: Report::new(Arc::new(AtomicBool::new(false)), &mut progress, &mut log),
        test_hooks: true,
    };
    let r = cantus_ml::separate::job(serde_json::to_value(&args).unwrap(), &mut ctx)
        .map(|v| serde_json::from_value(v).unwrap());
    (r, seen)
}

/// A network that passes only its time branch gives back exactly `wave[s]` times the
/// track, every sample of it: the segments, their centring, the triangular cross-fade
/// and the normalisation are demucs's, or this fails at the segments' joins.
#[test]
fn the_time_branch_comes_back_exact_across_every_join() {
    let dir = tempfile::tempdir().unwrap();
    // 20 s: three segments, two joins, a short last one padded at both sides.
    let frames = 20 * RATE as usize + 123;
    let x = music(frames);
    let input = dir.path().join("in.wav");
    write_wav(&input, 2, &x.0, &x.1);
    let gains = [0.1, 0.2, 0.3, 0.4];
    let args = fixture_args(input, dir.path().join("out"), [0.0; 4], gains);
    let (r, seen) = run(args);
    let r = r.unwrap();
    assert_eq!(r.frames, frames as u64);
    assert_eq!(r.inferences, 4, "offsets 0, 257 985, 515 970, 773 955");
    assert_eq!(r.device, "fixture");
    let mean = mono_mean(&x.0, &x.1);
    for (s, f) in r.stems.iter().enumerate() {
        assert_eq!(f.name, SOURCES[s]);
        let stem = read_wav(&f.path);
        assert_eq!(stem.0.len(), frames);
        let w = worst(&stem, &x, gains[s], mean, 0..frames);
        assert!(w < 2e-6, "{}: {w}", f.name);
    }
    // Progress rises to 1, through the stages.
    assert!(seen.windows(2).all(|p| p[0].0 <= p[1].0));
    assert_eq!(seen.last().unwrap().0, 1.0);
    assert!(seen.iter().any(|(_, s)| s == "separate"));
}

/// The spectral branch: the inverse of the spectrogram, cross-faded the same way. It is
/// exact except within 2 048 samples of a segment's window's ends, where `_ispec` lacks
/// the two frames `_spec` dropped — as in PyTorch (`stft_reference.rs` holds those samples
/// to PyTorch's). There the cross-fade's small weight keeps what leaks small — but at the
/// track's own two ends, where no other segment overlaps.
#[test]
fn the_spectral_branch_comes_back_but_at_the_windows_ends() {
    let dir = tempfile::tempdir().unwrap();
    let frames = 12 * RATE as usize;
    let x = music(frames);
    let input = dir.path().join("in.wav");
    write_wav(&input, 2, &x.0, &x.1);
    let gains = [0.4, 0.3, 0.2, 0.1];
    let (r, _) = run(fixture_args(input, dir.path().join("out"), gains, [0.0; 4]));
    let r = r.unwrap();
    let mean = mono_mean(&x.0, &x.1);
    // Each segment's window, as `TensorChunk.padded` centres it.
    let plan = cantus_ml::demucs::Plan::new(frames as u64, SEGMENT, 0.25);
    let near_an_end = |i: usize| {
        plan.offsets.iter().any(|&o| {
            let len = (SEGMENT as u64).min(frames as u64 - o) as i64;
            let start = o as i64 - (SEGMENT as i64 - len) / 2;
            let i = i as i64;
            (i - start).abs() < 2048 || (i - (start + SEGMENT as i64)).abs() < 2048
        })
    };
    for (s, f) in r.stems.iter().enumerate() {
        let stem = read_wav(&f.path);
        let (mut inside, mut zone) = (0f32, 0f32);
        // The track's own two ends are a window's end with nothing to cross-fade with.
        for i in 2048..frames - 2048 {
            let w = worst(&stem, &x, gains[s], mean, i..i + 1);
            if near_an_end(i) {
                zone = zone.max(w);
            } else {
                inside = inside.max(w);
            }
        }
        assert!(inside < 2e-5, "{}: {inside}", f.name);
        assert!(zone < 1e-3, "{}: {zone}", f.name);
    }
}

/// A track shorter than one segment (padded at both ends) and a mono one (both channels
/// the same, as demucs doubles it).
#[test]
fn a_short_mono_track_separates() {
    let dir = tempfile::tempdir().unwrap();
    let frames = 3 * RATE as usize;
    let x = music(frames);
    let input = dir.path().join("mono.wav");
    write_wav(&input, 1, &x.0, &x.0);
    let gains = [0.25; 4];
    let (r, _) = run(fixture_args(input, dir.path().join("out"), [0.0; 4], gains));
    let r = r.unwrap();
    assert_eq!(r.inferences, 1);
    let mono = (x.0.clone(), x.0.clone());
    let mean = mono_mean(&mono.0, &mono.1);
    for f in &r.stems {
        let stem = read_wav(&f.path);
        assert_eq!(stem.0, stem.1);
        assert!(worst(&stem, &mono, 0.25, mean, 0..frames) < 2e-6);
    }
}

#[test]
fn a_track_at_another_rate_is_refused_and_nothing_is_written() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("48k.wav");
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: 48_000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(&input, spec).unwrap();
    for _ in 0..4800 {
        w.write_sample(0i16).unwrap();
    }
    w.finalize().unwrap();
    let out = dir.path().join("out");
    let (r, _) = run(fixture_args(input, out.clone(), [0.0; 4], [0.25; 4]));
    match r {
        Err(Stop::Failed(JobError::Input(m))) => assert!(m.contains("48000 Hz"), "{m}"),
        other => panic!("{other:?}"),
    }
    assert!(!out.exists());
}

/// 16-bit integer input reads at its scale.
#[test]
fn integer_wav_input_is_scaled() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("pcm16.wav");
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: RATE,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let frames = 2 * RATE as usize;
    let x = music(frames);
    let mut w = hound::WavWriter::create(&input, spec).unwrap();
    let q = |v: f32| (v * 32768.0).round() as i16;
    for i in 0..frames {
        w.write_sample(q(x.0[i])).unwrap();
        w.write_sample(q(x.1[i])).unwrap();
    }
    w.finalize().unwrap();
    let (r, _) = run(fixture_args(
        input,
        dir.path().join("out"),
        [0.0; 4],
        [1.0, 0.0, 0.0, 0.0],
    ));
    let stem = read_wav(&r.unwrap().stems[0].path);
    let err = (0..frames)
        .map(|i| (stem.0[i] - x.0[i]).abs().max((stem.1[i] - x.1[i]).abs()))
        .fold(0f32, f32::max);
    assert!(err < 2.0 / 32768.0, "{err}");
}

#[test]
fn fixtures_are_refused_outside_tests() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("in.wav");
    let x = music(RATE as usize);
    write_wav(&input, 2, &x.0, &x.1);
    let args = fixture_args(input, dir.path().join("out"), [0.0; 4], [0.25; 4]);
    let mut rt = Runtime::new();
    let mut progress = |_: f32, _: &str| {};
    let mut log = |_: &str, _: &str| {};
    let mut ctx = Ctx {
        id: 1,
        runtime: &mut rt,
        report: Report::new(Arc::new(AtomicBool::new(false)), &mut progress, &mut log),
        test_hooks: false,
    };
    let r = cantus_ml::separate::job(serde_json::to_value(&args).unwrap(), &mut ctx);
    assert!(
        matches!(r, Err(Stop::Failed(JobError::BadArgs(_)))),
        "{r:?}"
    );
}

/// A runtime library or a model file that is not the one installed is never loaded
/// (review 1 #5): its hash is checked first.
#[test]
fn a_library_or_model_not_as_installed_is_refused_before_it_loads() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("in.wav");
    let x = music(RATE as usize);
    write_wav(&input, 2, &x.0, &x.1);
    let lib = dir.path().join("onnxruntime.dll");
    std::fs::write(&lib, b"planted").unwrap();
    let graph = dir.path().join("htdemucs.onnx");
    std::fs::write(&graph, b"not a graph").unwrap();
    let mut args = fixture_args(input, dir.path().join("out"), [0.0; 4], [0.25; 4]);
    args.model.family = cantus_ml::spec::Family::HtDemucs;
    args.model.graph = graph.clone();
    args.runtime.library = lib;
    args.runtime.sha256 = Some("0".repeat(64));
    match run(args.clone()).0 {
        Err(Stop::Failed(JobError::Runtime(m))) => assert!(m.contains("not the file"), "{m}"),
        other => panic!("{other:?}"),
    }
    // The graph not as installed: refused for the graph, before the library is even
    // looked at.
    args.runtime.sha256 = None;
    args.model.check = vec![cantus_ml::spec::FileCheck {
        path: graph,
        sha256: "1".repeat(64),
    }];
    match run(args).0 {
        Err(Stop::Failed(JobError::Model(m))) => assert!(m.contains("not the file"), "{m}"),
        other => panic!("{other:?}"),
    }
}

/// The shift trick: each pass moved later and back, the passes averaged — the time branch
/// still comes back exact, every sample, so every pass was put back where it belongs.
#[test]
fn shifted_passes_line_up_to_the_sample() {
    let dir = tempfile::tempdir().unwrap();
    let frames = 9 * RATE as usize + 77;
    let x = music(frames);
    let input = dir.path().join("in.wav");
    write_wav(&input, 2, &x.0, &x.1);
    let gains = [0.1, 0.2, 0.3, 0.4];
    let mut args = fixture_args(input, dir.path().join("out"), [0.0; 4], gains);
    args.shifts = 3;
    let (r, seen) = run(args);
    let r = r.unwrap();
    assert_eq!(r.inferences, 3 * 2, "three passes of two segments");
    let mean = mono_mean(&x.0, &x.1);
    for (s, f) in r.stems.iter().enumerate() {
        let stem = read_wav(&f.path);
        assert_eq!(stem.0.len(), frames);
        let w = worst(&stem, &x, gains[s], mean, 0..frames);
        assert!(w < 2e-6, "{}: {w}", f.name);
    }
    // No pass's files left behind; progress rose through every pass.
    let left: Vec<_> = std::fs::read_dir(dir.path().join("out"))
        .unwrap()
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().starts_with(".pass"))
        .collect();
    assert!(left.is_empty());
    assert!(seen.windows(2).all(|p| p[0].0 <= p[1].0), "{seen:?}");
}

/// *Best*'s average is said a block at a time and stops on a cancel: a long song's last
/// step is never silent long enough to look hung, nor deaf to *Cancel* (review 2 #7).
#[test]
fn the_average_says_its_progress_and_stops_on_a_cancel() {
    let dir = tempfile::tempdir().unwrap();
    let frames = 9 * RATE as usize;
    let x = music(frames);
    let input = dir.path().join("in.wav");
    write_wav(&input, 2, &x.0, &x.1);
    let out = dir.path().join("out");
    let mut args = fixture_args(input, out.clone(), [0.0; 4], [0.25; 4]);
    args.shifts = 3;
    let (r, seen) = run(args.clone());
    assert!(r.is_ok());
    let inside = seen
        .iter()
        .filter(|(d, s)| s == "write" && *d > 0.95 && *d < 1.0)
        .count();
    assert!(inside >= 4, "the average said {inside} steps: {seen:?}");
    // A cancel once the average has begun: cancelled, nothing left.
    std::fs::remove_dir_all(&out).unwrap();
    let cancel = Arc::new(AtomicBool::new(false));
    let flag = cancel.clone();
    let mut rt = Runtime::new();
    let mut progress = move |d: f32, s: &str| {
        if s == "write" && d > 0.95 {
            flag.store(true, std::sync::atomic::Ordering::Relaxed);
        }
    };
    let mut log = |_: &str, _: &str| {};
    let mut ctx = Ctx {
        id: 2,
        runtime: &mut rt,
        report: Report::new(cancel, &mut progress, &mut log),
        test_hooks: true,
    };
    let r = cantus_ml::separate::job(serde_json::to_value(&args).unwrap(), &mut ctx);
    assert!(matches!(r, Err(Stop::Cancelled)), "{r:?}");
    let left: Vec<_> = std::fs::read_dir(&out)
        .map(|d| d.flatten().map(|e| e.file_name()).collect())
        .unwrap_or_default();
    assert!(left.is_empty(), "{left:?}");
}

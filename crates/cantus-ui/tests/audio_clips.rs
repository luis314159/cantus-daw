//! Audio clips you can see and shape (ADR 0025): peaks, import, fades and gain by hand,
//! normalise, split and glue, saving with audio, and a missing file found again.

mod support;

use cantus_core::{AudioRegion, Command, Project, Ticks};
use cantus_ui::waveform::{Lookup, PeakStore, SourcePeaks, Which, LODS};
use cantus_ui::CantusApp;
use std::path::{Path, PathBuf};
use std::time::Duration;
use support::app;

/// A 16-bit WAV of `frames` frames: `f(frame, channel)`.
fn wav(path: &Path, rate: u32, channels: u16, frames: usize, f: impl Fn(usize, usize) -> f32) {
    let spec = hound::WavSpec {
        channels,
        sample_rate: rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(path, spec).unwrap();
    for i in 0..frames {
        for c in 0..channels as usize {
            w.write_sample((f(i, c) * 32767.0).round() as i16).unwrap();
        }
    }
    w.finalize().unwrap();
}

fn ready(store: &PeakStore, path: &Path) -> std::sync::Arc<SourcePeaks> {
    let _ = store.lookup_path(path, None);
    assert!(
        store.wait_idle(Duration::from_secs(20)),
        "the peaks never came"
    );
    match store.lookup_path(path, None) {
        Lookup::Ready(p) => p,
        other => panic!("not ready: {other:?}"),
    }
}

fn empty_app() -> CantusApp {
    let mut p = Project::empty("Audio");
    let t = p.make_track("Keys");
    p.tracks.push(t);
    app(p)
}

#[test]
fn peaks_of_a_known_file_at_each_level_of_detail() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("steps.wav");
    // Left: +0.5 for the first 4096 frames, then -0.25; right: always -0.5.
    let frames = 10_000;
    wav(&file, 48_000, 2, frames, |i, c| match c {
        0 if i < 4096 => 0.5,
        0 => -0.25,
        _ => -0.5,
    });
    let store = PeakStore::default();
    let p = ready(&store, &file);
    assert_eq!((p.rate, p.channels, p.frames), (48_000, 2, frames as u64));
    let fpp: Vec<usize> = p.levels.iter().map(|l| l.frames_per_peak).collect();
    assert_eq!(fpp, LODS);
    for l in &p.levels {
        assert_eq!(l.mono.len(), frames.div_ceil(l.frames_per_peak));
        assert_eq!(l.channels.len(), 2);
    }
    let q = |v: f32| (v * 32767.0).round() / 32768.0;
    let l0 = &p.levels[0];
    assert_eq!(l0.channels[0][0], (q(0.5), q(0.5)));
    assert_eq!(l0.channels[1][0], (q(-0.5), q(-0.5)));
    // Mono is the sum over two: (0.5 - 0.5) / 2 = 0 before the step.
    assert!(l0.mono[0].0.abs() < 1e-6 && l0.mono[0].1.abs() < 1e-6);
    // The coarsest level's first pair spans the step on the left.
    let l2 = &p.levels[2];
    assert_eq!(l2.channels[0][0], (q(0.5), q(0.5)));
    assert_eq!(l2.channels[0][1], (q(-0.25), q(-0.25)));
    // Columns read the coarsest level finer than a column, and agree with level 0.
    let mut cols = Vec::new();
    p.columns(Which::Channel(0), 0.0, 5000.0, 2, &mut cols);
    assert_eq!(cols[0], (q(-0.25), q(0.5)));
    assert_eq!(cols[1], (q(-0.25), q(-0.25)));
    assert!((p.peak() - q(0.5)).abs() < 1e-6);
    assert!(
        (p.peak_in(5000.0, 9000.0) - q(0.5)).abs() < 1e-6,
        "right is -0.5"
    );
}

#[test]
fn the_disk_cache_gives_back_what_was_computed() {
    let dir = tempfile::tempdir().unwrap();
    let cache = tempfile::tempdir().unwrap();
    let file = dir.path().join("sine.wav");
    wav(&file, 44_100, 1, 30_000, |i, _| {
        (i as f32 * 0.01).sin() * 0.8
    });
    let first = PeakStore::default();
    first.set_disk_cache(Some(cache.path().to_path_buf()));
    let a = ready(&first, &file);
    let written: Vec<_> = std::fs::read_dir(cache.path()).unwrap().flatten().collect();
    assert_eq!(written.len(), 1, "one peak file");
    let second = PeakStore::default();
    second.set_disk_cache(Some(cache.path().to_path_buf()));
    let b = ready(&second, &file);
    assert_eq!(a.levels.len(), b.levels.len());
    for (x, y) in a.levels.iter().zip(&b.levels) {
        for (p, q) in x.mono.iter().zip(&y.mono) {
            assert!((p.0 - q.0).abs() < 1e-4 && (p.1 - q.1).abs() < 1e-4);
        }
    }
}

#[test]
fn a_missing_file_is_known_at_once() {
    let store = PeakStore::default();
    let gone = Path::new("Z:/nowhere/gone.wav");
    assert!(matches!(store.lookup_path(gone, None), Lookup::Missing));
}

fn import(a: &mut CantusApp, file: &Path) -> (cantus_core::TrackId, cantus_core::ClipId) {
    assert_eq!(a.import_audio(&[file.to_path_buf()], Ticks::ZERO, None), 1);
    let (t, c) = a.clip_selection()[0];
    (t, c)
}

#[test]
fn import_makes_a_source_and_a_clip_as_long_as_the_file() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("Two seconds.wav");
    wav(&file, 48_000, 1, 96_000, |_, _| 0.1);
    for (bpm, ticks) in [(120.0, Ticks::beats(4)), (90.0, Ticks::beats(3))] {
        let mut a = empty_app();
        a.exec(Command::SetTempo { bpm });
        let tracks = a.project().tracks.len();
        let (t, c) = import(&mut a, &file);
        let p = a.project();
        assert_eq!(p.tracks.len(), tracks + 1, "a new track for the file");
        let track = p.track(t).unwrap();
        assert_eq!(track.name, "Two seconds");
        let clip = track.clip(c).unwrap();
        assert_eq!(clip.length, ticks, "at {bpm} BPM");
        let src = p.audio_source(clip.audio.as_ref().unwrap().source).unwrap();
        assert!(!src.recorded);
        assert_eq!(
            (src.sample_rate, src.channels, src.frames),
            (48_000, 1, 96_000)
        );
        assert_eq!(Path::new(&src.path), file, "absolute before the first save");
        // One undo step takes the whole import away.
        a.undo();
        assert_eq!(a.project().tracks.len(), tracks);
        assert!(a.project().audio_pool.is_empty());
    }
}

#[test]
fn files_dropped_on_a_track_land_on_it_and_the_ones_below() {
    let dir = tempfile::tempdir().unwrap();
    let files: Vec<PathBuf> = (0..3)
        .map(|i| {
            let f = dir.path().join(format!("f{i}.wav"));
            wav(&f, 48_000, 1, 4800, |_, _| 0.2);
            f
        })
        .collect();
    let mut a = empty_app();
    let first = a.project().tracks[0].id;
    let n = a.import_audio(&files, Ticks::beats(8), Some(0));
    assert_eq!(n, 3);
    let p = a.project();
    assert_eq!(p.tracks.len(), 3, "one existing track, two made");
    assert_eq!(p.track(first).unwrap().clips.len(), 1);
    assert!(p.tracks.iter().all(|t| t.clips.len() == 1));
    assert!(p.tracks.iter().all(|t| t.clips[0].start == Ticks::beats(8)));
    let strips: Vec<_> = p.tracks.iter().map(|t| t.strip).collect();
    assert!(
        strips[1] != strips[2] || strips[1].is_none(),
        "new tracks share no strip"
    );
}

#[path = "audio_clips/files.rs"]
mod files;
#[path = "audio_clips/gestures.rs"]
mod gestures;

/// Shared by the other parts of this test.
pub(crate) fn region(a: &CantusApp, r: (cantus_core::TrackId, cantus_core::ClipId)) -> AudioRegion {
    a.project()
        .track(r.0)
        .unwrap()
        .clip(r.1)
        .unwrap()
        .audio
        .clone()
        .unwrap()
}

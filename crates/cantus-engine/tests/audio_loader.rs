//! Audio sources load in the background for a live engine (ADR 0025 §9): the snapshot
//! is made at once with the clip silent, the file is decoded and converted on the loader
//! thread, and the next snapshot has exactly what a load on the spot would have given.

use cantus_core::*;
use cantus_engine::*;
use std::time::{Duration, Instant};

fn write_wav(path: &std::path::Path, rate: u32, frames: usize) {
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(path, spec).unwrap();
    for i in 0..frames * 2 {
        w.write_sample(((i * 37) % 2000) as i16 - 1000).unwrap();
    }
    w.finalize().unwrap();
}

fn project(path: &std::path::Path) -> Project {
    let mut p = Project::new("loader");
    let mut t = p.make_track("A");
    let id = AudioSourceId(p.alloc_id());
    p.audio_pool.push(AudioSource {
        id,
        name: "a".into(),
        path: path.to_string_lossy().into_owned(),
        sample_rate: 44_100,
        channels: 2,
        frames: 44_100,
        sha256: None,
        recorded: false,
    });
    let c = p.make_audio_clip("c", id, Ticks::ZERO, Ticks::beats(2));
    t.clips.push(c);
    p.tracks.push(t);
    p
}

#[test]
fn a_source_loads_in_the_background_and_arrives_whole() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.wav");
    write_wav(&path, 44_100, 44_100);
    let p = project(&path);
    let stored = p.audio_pool[0].path.clone();

    let mut cache = SampleCache::new();
    cache.set_background(true);
    let mut s = Snapshot::compile(&p, 1);
    let started = Instant::now();
    assert!(
        cache.attach_audio(&mut s, 48_000).is_empty(),
        "loading is no error"
    );
    assert!(
        s.tracks[0].audio[0].data.is_none(),
        "silent until it arrives"
    );
    assert_eq!(cache.audio_status(&stored), SampleStatus::Loading);
    assert!(cache.loading());
    // Asking again does not queue it twice.
    let mut again = Snapshot::compile(&p, 2);
    cache.attach_audio(&mut again, 48_000);
    while !cache.poll_loaded() {
        assert!(
            started.elapsed() < Duration::from_secs(20),
            "it never arrived"
        );
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(!cache.loading());
    let mut s = Snapshot::compile(&p, 3);
    assert!(cache.attach_audio(&mut s, 48_000).is_empty());
    let got = s.tracks[0].audio[0].data.clone().expect("attached");

    // The same as a load on the spot.
    let mut sync = SampleCache::new();
    let mut s2 = Snapshot::compile(&p, 1);
    sync.attach_audio(&mut s2, 48_000);
    let want = s2.tracks[0].audio[0].data.clone().unwrap();
    assert_eq!(got.rate, 48_000);
    assert_eq!(got.frames[..], want.frames[..]);
}

#[test]
fn a_missing_source_fails_in_the_background_too() {
    let dir = tempfile::tempdir().unwrap();
    let p = project(&dir.path().join("gone.wav"));
    let mut cache = SampleCache::new();
    cache.set_background(true);
    let mut s = Snapshot::compile(&p, 1);
    cache.attach_audio(&mut s, 48_000);
    let started = Instant::now();
    while !cache.poll_loaded() {
        assert!(started.elapsed() < Duration::from_secs(10));
        std::thread::sleep(Duration::from_millis(2));
    }
    let mut s = Snapshot::compile(&p, 2);
    let errors = cache.attach_audio(&mut s, 48_000);
    assert_eq!(errors.len(), 1, "now it is reported");
    assert!(matches!(
        cache.audio_status(&p.audio_pool[0].path),
        SampleStatus::Failed(_)
    ));
}

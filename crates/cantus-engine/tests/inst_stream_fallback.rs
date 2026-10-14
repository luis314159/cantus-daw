//! No test writes the shared fallback cache folder (`%TEMP%/cantus-stream-cache`): a debug
//! build — every test run — refuses to stream without a folder of its own, so a test that
//! forgets to set one fails instead of leaving files behind for every later run. Its own
//! test binary: it needs the hub with no folder set.

use cantus_engine::inst as einst;
use std::path::Path;

fn wav(path: &Path, frames: usize) {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 48_000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(path, spec).unwrap();
    for i in 0..frames {
        w.write_sample(((i % 89) as i16 - 44) * 200).unwrap();
    }
    w.finalize().unwrap();
}

#[test]
fn a_test_without_a_cache_folder_of_its_own_fails_and_touches_no_fallback() {
    // The refusal is a debug build's (a release build keeps the fallback for tools).
    if !cfg!(debug_assertions) {
        return;
    }
    let hub = einst::stream::hub();
    assert!(!hub.cache_dir_set());
    let fallback = std::env::temp_dir().join("cantus-stream-cache");
    let before = std::fs::metadata(&fallback).and_then(|m| m.modified()).ok();
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("long.wav");
    wav(&src, 100_000);
    let decode = |p: &Path| {
        cantus_engine::SampleCache::new()
            .load(p)
            .map(|a| (*a).clone())
    };
    let r = einst::stream::load(&src, &decode);
    assert!(
        matches!(&r, Err(e) if e.contains("no sample cache folder set")),
        "refused: {r:?}"
    );
    // Nothing read or swept there either.
    assert_eq!(hub.cache_size(), 0);
    let _ = hub.sweep();
    let after = std::fs::metadata(&fallback).and_then(|m| m.modified()).ok();
    assert_eq!(before, after, "the fallback folder untouched");
}

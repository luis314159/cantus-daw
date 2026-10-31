//! What the machine-learning tests share (ADR 0085): this build's `cantus-ml` helper,
//! built from source into `target/test-plugins` (the test's cargo holds the workspace's
//! own target folder), once per process.
#![allow(dead_code)]

use std::path::{Path, PathBuf};

/// The helper, built.
pub fn helper_exe() -> PathBuf {
    static ONE: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _one = ONE.lock().unwrap_or_else(|e| e.into_inner());
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
    let target = root.join("target").join("test-plugins");
    let status =
        std::process::Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
            .args([
                "build",
                "-p",
                "cantus-ml",
                "--bin",
                "cantus-ml",
                "--target-dir",
            ])
            .arg(&target)
            .current_dir(&root)
            .status()
            .expect("cargo");
    assert!(status.success(), "cantus-ml builds");
    target.join("debug").join(if cfg!(windows) {
        "cantus-ml.exe"
    } else {
        "cantus-ml"
    })
}

/// A stereo WAV of a few seconds at 44.1 kHz.
pub fn write_track(path: &Path, seconds: f32) -> (Vec<f32>, Vec<f32>) {
    let n = (seconds * 44_100.0) as usize;
    let (l, r): (Vec<f32>, Vec<f32>) = (0..n)
        .map(|i| {
            let t = i as f32;
            (0.4 * (t * 0.013).sin(), 0.3 * (t * 0.009 + 1.0).sin())
        })
        .unzip();
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: 44_100,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut w = hound::WavWriter::create(path, spec).unwrap();
    for i in 0..n {
        w.write_sample(l[i]).unwrap();
        w.write_sample(r[i]).unwrap();
    }
    w.finalize().unwrap();
    (l, r)
}

//! The real model, through the real helper and ONNX Runtime, against Meta's own code on
//! the same file. Needs files no test may download, so it is ignored by default:
//!
//! ```text
//! CANTUS_ORT_LIBRARY=…/onnxruntime.dll        the runtime's library
//! CANTUS_HTDEMUCS=…/htdemucs.onnx             the graph, its .weights beside it
//! CANTUS_DEMUCS_REFERENCE=…/ref/funk          input.wav and PyTorch's <source>.wav
//!                                             (tools/reference_htdemucs.py wrote them)
//! cargo test -p cantus-ml --release --test htdemucs_real -- --ignored --nocapture
//! ```
//!
//! It prints, per source, the largest difference and the signal-to-difference ratio, the
//! time taken and the real-time factor, and fails under 60 dB (a faithful port is far
//! above: the network alone matches PyTorch at 100 dB).

mod common;

use cantus_ml::client::{ClientConfig, MlClient, Outcome};
use cantus_ml::spec::{Device, Family, ModelSpec, RuntimeSpec, SeparateArgs, SeparateResult};
use common::*;
use std::path::PathBuf;
use std::time::Duration;

fn var(name: &str) -> PathBuf {
    PathBuf::from(std::env::var_os(name).unwrap_or_else(|| panic!("set {name}")))
}

#[test]
#[ignore = "needs the runtime, the model and PyTorch's reference output (see the file's header)"]
fn htdemucs_matches_pytorch() {
    let reference = var("CANTUS_DEMUCS_REFERENCE");
    let out = tempfile::tempdir().unwrap();
    let args = SeparateArgs {
        runtime: RuntimeSpec {
            library: var("CANTUS_ORT_LIBRARY"),
            threads: 0,
            // CANTUS_ML_DEVICE=auto for the graphics runtime (DirectML, CoreML).
            device: match std::env::var("CANTUS_ML_DEVICE").as_deref() {
                Ok("auto") => Device::Auto,
                _ => Device::Cpu,
            },
            sha256: None,
        },
        model: ModelSpec {
            graph: var("CANTUS_HTDEMUCS"),
            family: Family::HtDemucs,
            // CANTUS_DEMUCS_SOURCES=drums,bass,other,vocals,guitar,piano for the 6-stem one.
            sources: std::env::var("CANTUS_DEMUCS_SOURCES")
                .map(|v| v.split(',').map(str::to_string).collect())
                .unwrap_or_else(|_| SOURCES.iter().map(|s| s.to_string()).collect()),
            sample_rate: RATE,
            segment: SEGMENT,
            check: Vec::new(),
        },
        input: reference.join("input.wav"),
        out_dir: out.path().to_path_buf(),
        overlap: 0.25,
        // CANTUS_DEMUCS_SHIFTS=n against `reference_htdemucs.py --shifts n`.
        shifts: std::env::var("CANTUS_DEMUCS_SHIFTS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0),
    };
    let mut c = MlClient::new(ClientConfig::new(env!("CARGO_BIN_EXE_cantus-ml")));
    let id = c.start("separate", serde_json::to_value(&args).unwrap());
    let r: SeparateResult = match c.wait(id, Duration::from_secs(3600)) {
        Some(Outcome::Done(v)) => serde_json::from_value(v).unwrap(),
        other => panic!("{other:?}"),
    };
    for u in c.poll() {
        if let cantus_ml::Update::Log { level, message } = u {
            println!("[{level}] {message}");
        }
    }
    let secs = r.frames as f64 / r.sample_rate as f64;
    println!(
        "{secs:.1} s of audio in {:.1} s ({:.2}x real time), {} inferences on {}, {:.1} s in the network",
        r.seconds,
        secs / r.seconds,
        r.inferences,
        r.device,
        r.network_seconds
    );
    let mut worst_snr = f64::MAX;
    for f in &r.stems {
        let got = read_wav(&f.path);
        let want = read_wav(&reference.join(format!("{}.wav", f.name)));
        assert_eq!(got.0.len(), want.0.len());
        let (mut sig, mut diff, mut max) = (0f64, 0f64, 0f32);
        for (g, w) in got.0.iter().chain(&got.1).zip(want.0.iter().chain(&want.1)) {
            sig += (*w as f64).powi(2);
            diff += ((g - w) as f64).powi(2);
            max = max.max((g - w).abs());
        }
        let snr = 10.0 * (sig / diff.max(1e-30)).log10();
        println!("{:>7}: largest difference {max:.2e}, {snr:.1} dB", f.name);
        worst_snr = worst_snr.min(snr);
    }
    assert!(worst_snr > 60.0, "{worst_snr} dB");
}

/// A cancel inside a real run (`RunOptions::terminate`): ended within one segment's time,
/// not after the song (review 1 #9). Same files as above.
#[test]
#[ignore = "needs the runtime and the model (see the file's header)"]
fn a_cancel_stops_a_real_run_inside_its_segment() {
    let reference = var("CANTUS_DEMUCS_REFERENCE");
    let out = tempfile::tempdir().unwrap();
    let args = SeparateArgs {
        runtime: RuntimeSpec {
            library: var("CANTUS_ORT_LIBRARY"),
            threads: 0,
            device: Device::Cpu,
            sha256: None,
        },
        model: ModelSpec {
            graph: var("CANTUS_HTDEMUCS"),
            family: Family::HtDemucs,
            sources: SOURCES.iter().map(|s| s.to_string()).collect(),
            sample_rate: RATE,
            segment: SEGMENT,
            check: Vec::new(),
        },
        input: reference.join("input.wav"),
        out_dir: out.path().to_path_buf(),
        overlap: 0.25,
        shifts: 0,
    };
    let mut c = MlClient::new(ClientConfig::new(env!("CARGO_BIN_EXE_cantus-ml")));
    let id = c.start("separate", serde_json::to_value(&args).unwrap());
    // Into the separation proper: past the load.
    let t = std::time::Instant::now();
    loop {
        let u = c.poll();
        if u.iter()
            .any(|u| matches!(u, cantus_ml::Update::Progress { done, .. } if *done > 0.06))
        {
            break;
        }
        assert!(t.elapsed() < Duration::from_secs(600));
        std::thread::sleep(Duration::from_millis(20));
    }
    // Half a segment in, then cancel.
    std::thread::sleep(Duration::from_millis(800));
    let asked = std::time::Instant::now();
    c.cancel(id);
    assert_eq!(
        c.wait(id, Duration::from_secs(60)),
        Some(Outcome::Cancelled)
    );
    let took = asked.elapsed();
    println!("cancelled in {:.0} ms", took.as_secs_f64() * 1000.0);
    assert!(took < Duration::from_millis(1500), "{took:?}");
    // The helper is released; the runtime's library and the model can then be removed.
    let pid = c.helper_pid();
    c.release();
    let t = std::time::Instant::now();
    while c.helper_pid() != 0 && t.elapsed() < Duration::from_secs(10) {
        std::thread::sleep(Duration::from_millis(50));
    }
    let _ = pid;
    assert_eq!(c.helper_pid(), 0);
}

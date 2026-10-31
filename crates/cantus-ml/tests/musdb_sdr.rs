//! Separate every track of MUSDB18-HQ's test set with the real model, for the SDR the
//! paper reports (ADR 0085). MUSDB18-HQ is for non-commercial research: downloaded locally
//! for measuring only, never committed. Ignored by default:
//!
//! ```text
//! CANTUS_ORT_LIBRARY=…/onnxruntime.dll CANTUS_HTDEMUCS=…/htdemucs.onnx
//! CANTUS_MUSDB=…/musdb18hq/test CANTUS_MUSDB_OUT=…/musdb-out [CANTUS_DEMUCS_SHIFTS=4]
//! cargo test -p cantus-ml --release --test musdb_sdr -- --ignored --nocapture
//! ```
//!
//! then `crates/cantus-ml/tools/musdb_sdr.py <musdb test> <out>` computes BSS Eval v4's SDR
//! (museval, 1-second windows; the median over each track's windows, then over the tracks)
//! and prints it beside the published numbers. A track already separated is skipped.

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
#[ignore = "needs MUSDB18-HQ, the runtime and the model (see the file's header)"]
fn separate_musdb18hq_test_set() {
    let musdb = var("CANTUS_MUSDB");
    let out = var("CANTUS_MUSDB_OUT");
    let shifts: u32 = std::env::var("CANTUS_DEMUCS_SHIFTS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let mut tracks: Vec<PathBuf> = std::fs::read_dir(&musdb)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.join("mixture.wav").is_file())
        .collect();
    tracks.sort();
    let mut c = MlClient::new(ClientConfig::new(env!("CARGO_BIN_EXE_cantus-ml")));
    let (mut audio, mut took) = (0f64, 0f64);
    for t in &tracks {
        let dest = out.join(t.file_name().unwrap());
        if SOURCES
            .iter()
            .all(|s| dest.join(format!("{s}.wav")).is_file())
        {
            continue;
        }
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
            input: t.join("mixture.wav"),
            out_dir: dest,
            overlap: 0.25,
            shifts,
        };
        let id = c.start("separate", serde_json::to_value(&args).unwrap());
        let r: SeparateResult = match c.wait(id, Duration::from_secs(7200)) {
            Some(Outcome::Done(v)) => serde_json::from_value(v).unwrap(),
            other => panic!("{}: {other:?}", t.display()),
        };
        let secs = r.frames as f64 / r.sample_rate as f64;
        audio += secs;
        took += r.seconds;
        println!(
            "{:>60}: {secs:.0} s in {:.0} s",
            t.file_name().unwrap().to_string_lossy(),
            r.seconds
        );
    }
    if took > 0.0 {
        println!(
            "{audio:.0} s of audio in {took:.0} s ({:.2}x real time)",
            audio / took
        );
    }
}

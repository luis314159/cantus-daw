//! The whole path a user takes, from nothing: the model's licence accepted, ONNX Runtime
//! and HT-Demucs downloaded from their makers through the content manager (hashes
//! checked), the weights rebuilt from Meta's file, and a song separated by the helper.
//! It downloads about 100 MB, so it is ignored by default:
//!
//! ```text
//! CANTUS_ML_NET=1 cargo test -p cantus-ml --release --test install_and_separate -- --ignored --nocapture
//! ```
//!
//! `CANTUS_ML_MODELS=<dir>` keeps the download between runs (default: a fresh folder).

mod common;

use cantus_content::models::{ModelEvent, ModelManifest, ModelStatus, Models};
use cantus_ml::client::{ClientConfig, MlClient, Outcome};
use cantus_ml::spec::{Device, Family, ModelSpec, RuntimeSpec, SeparateArgs, SeparateResult};
use common::*;
use std::time::{Duration, Instant};

#[test]
#[ignore = "downloads the runtime and the model (about 100 MB); CANTUS_ML_NET=1"]
fn install_htdemucs_from_its_makers_and_separate_a_song() {
    assert_eq!(
        std::env::var("CANTUS_ML_NET").as_deref(),
        Ok("1"),
        "set CANTUS_ML_NET=1"
    );
    let fresh = tempfile::tempdir().unwrap();
    let root = std::env::var_os("CANTUS_ML_MODELS")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| fresh.path().to_path_buf());
    let mut models = Models::new(&root, ModelManifest::bundled());
    models.accept("htdemucs").unwrap();
    let t = Instant::now();
    models.install("htdemucs").unwrap();
    let events = models.wait_idle(Duration::from_secs(30 * 60));
    println!(
        "installed in {:.1} s: {events:?}",
        t.elapsed().as_secs_f64()
    );
    assert_eq!(models.status("htdemucs"), ModelStatus::Ready, "{events:?}");
    assert!(!events.iter().any(|e| matches!(e, ModelEvent::Failed(..))));
    let ready = models.ready("htdemucs").unwrap();

    let dir = tempfile::tempdir().unwrap();
    let x = music(15 * RATE as usize);
    let input = dir.path().join("in.wav");
    write_wav(&input, 2, &x.0, &x.1);
    let args = SeparateArgs {
        runtime: RuntimeSpec {
            library: ready.library.clone(),
            threads: 0,
            device: Device::Cpu,
            sha256: Some(ready.library_sha256.clone()),
        },
        model: ModelSpec {
            graph: ready.graph.clone(),
            family: Family::HtDemucs,
            sources: ready.sources.clone(),
            sample_rate: ready.sample_rate,
            segment: ready.segment,
            check: ready
                .files
                .iter()
                .map(|(path, sha256)| cantus_ml::spec::FileCheck {
                    path: path.clone(),
                    sha256: sha256.clone(),
                })
                .collect(),
        },
        input,
        out_dir: dir.path().join("stems"),
        overlap: 0.25,
        shifts: 0,
    };
    let mut c = MlClient::new(ClientConfig::new(env!("CARGO_BIN_EXE_cantus-ml")));
    let id = c.start("separate", serde_json::to_value(&args).unwrap());
    let r: SeparateResult = match c.wait(id, Duration::from_secs(1800)) {
        Some(Outcome::Done(v)) => serde_json::from_value(v).unwrap(),
        other => panic!("{other:?}"),
    };
    println!(
        "separated 15 s in {:.1} s ({:.1} s in the network)",
        r.seconds, r.network_seconds
    );
    // The stems add up to the song, less what the model leaves out. On this signal Meta's
    // own code leaves 19.0 dB (`tools/reference_htdemucs.py`; 28.9 dB on the funk demo,
    // 32.4 dB on three minutes of the demos); a broken graph or wrong weights would not.
    let stems: Vec<_> = r.stems.iter().map(|s| read_wav(&s.path)).collect();
    let (mut sig, mut res) = (0f64, 0f64);
    for i in 0..x.0.len() {
        let l: f32 = stems.iter().map(|s| s.0[i]).sum();
        let rr: f32 = stems.iter().map(|s| s.1[i]).sum();
        sig += (x.0[i] as f64).powi(2) + (x.1[i] as f64).powi(2);
        res += ((l - x.0[i]) as f64).powi(2) + ((rr - x.1[i]) as f64).powi(2);
    }
    let snr = 10.0 * (sig / res.max(1e-30)).log10();
    println!("the stems' sum against the song: {snr:.1} dB");
    assert!(snr > 18.5, "{snr} dB");
}

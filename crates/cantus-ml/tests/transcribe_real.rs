//! Basic Pitch itself, through ONNX Runtime and the real helper, against Spotify's own
//! `basic_pitch.inference.predict` on the same files (ADR 0086). Needs files no test may
//! download, so it is ignored by default:
//!
//! ```text
//! CANTUS_ORT_LIBRARY=…/onnxruntime.dll     the runtime's library
//! CANTUS_BASIC_PITCH=…/nmp.onnx            Spotify's graph
//! CANTUS_BP_REFERENCE=…/dir                <name>.wav at 22 050 Hz and, beside each,
//!                                          <name>.json and <name>.json.{note,onset,contour}.f32
//!                                          (tools/reference_basic_pitch.py predict)
//! cargo test -p cantus-ml --release --test transcribe_real -- --ignored --nocapture
//! ```
//!
//! For each file it prints the posteriorgrams' largest difference from Basic Pitch's (the
//! same network on two runtimes' arithmetic) and how many notes differ, and fails if the
//! posteriorgrams differ by more than 10⁻⁴ or more than 1 % of the notes do.

use cantus_ml::client::{ClientConfig, MlClient, Outcome};
use cantus_ml::spec::{Device, RuntimeSpec};
use cantus_ml::transcribe::notes::{self, Posteriors, CONTOURS, NOTES, WINDOW};
use cantus_ml::transcribe::spec::*;
use std::path::{Path, PathBuf};
use std::time::Duration;

fn var(name: &str) -> PathBuf {
    PathBuf::from(std::env::var_os(name).unwrap_or_else(|| panic!("set {name}")))
}

fn f32s(path: &Path) -> Vec<f32> {
    std::fs::read(path)
        .unwrap()
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes(c.try_into().unwrap()))
        .collect()
}

/// The posteriorgrams in this process (the runtime directly, as the job runs it).
fn posteriors(
    runtime: &mut cantus_ml::runtime::Runtime,
    lib: &Path,
    graph: &Path,
    wav: &Path,
) -> Posteriors {
    let rt = RuntimeSpec {
        library: lib.to_path_buf(),
        threads: 0,
        device: Device::Cpu,
        sha256: None,
    };
    let (x, rate) = cantus_ml::transcribe::read_mono(wav).unwrap();
    assert_eq!(rate, 22_050);
    let (mut m, _) = runtime.model(&rt, graph, &[], &mut |_, _| {}).unwrap();
    let cancel = std::sync::atomic::AtomicBool::new(false);
    let mut wins: [Vec<Vec<f32>>; 3] = Default::default();
    let mut buf = vec![0.0f32; WINDOW];
    for k in 0..notes::window_count(x.len()) {
        notes::window(&x, k, &mut buf);
        let outs = m
            .run(
                &[cantus_ml::runtime::Input {
                    name: "serving_default_input_2:0",
                    shape: &[1, WINDOW, 1],
                    data: &buf,
                }],
                &cancel,
            )
            .unwrap();
        for (i, name) in [
            "StatefulPartitionedCall:1",
            "StatefulPartitionedCall:2",
            "StatefulPartitionedCall:0",
        ]
        .iter()
        .enumerate()
        {
            wins[i].push(outs.iter().find(|o| o.name == *name).unwrap().data.clone());
        }
    }
    let note = notes::unwrap(&wins[0], NOTES, x.len());
    Posteriors {
        frames: note.len() / NOTES,
        note,
        onset: notes::unwrap(&wins[1], NOTES, x.len()),
        contour: notes::unwrap(&wins[2], CONTOURS, x.len()),
    }
}

#[test]
#[ignore = "needs the runtime, Basic Pitch's graph and its reference output (see the file's header)"]
fn basic_pitch_in_cantus_matches_basic_pitch() {
    let (lib, graph, dir) = (
        var("CANTUS_ORT_LIBRARY"),
        var("CANTUS_BASIC_PITCH"),
        var("CANTUS_BP_REFERENCE"),
    );
    let mut wavs: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "wav") && p.with_extension("json").is_file())
        .collect();
    wavs.sort();
    assert!(!wavs.is_empty(), "no references in {}", dir.display());
    let mut c = MlClient::new(ClientConfig::new(env!("CARGO_BIN_EXE_cantus-ml")));
    let out = tempfile::tempdir().unwrap();
    let (mut all, mut differ) = (0usize, 0usize);
    // One runtime for the process: ONNX Runtime's environment is made once.
    let mut runtime = cantus_ml::runtime::Runtime::new();
    for wav in &wavs {
        let json = wav.with_extension("json");
        let want: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&json).unwrap()).unwrap();
        // The network: our runtime's posteriorgrams against Basic Pitch's.
        let p = posteriors(&mut runtime, &lib, &graph, wav);
        let mut worst = 0.0f32;
        for (k, mine) in [
            ("note", &p.note),
            ("onset", &p.onset),
            ("contour", &p.contour),
        ] {
            let theirs = f32s(&PathBuf::from(format!("{}.{k}.f32", json.display())));
            assert_eq!(mine.len(), theirs.len(), "{k}");
            for (a, b) in mine.iter().zip(&theirs) {
                worst = worst.max((a - b).abs());
            }
        }
        // The notes, through the helper.
        let args = TranscribeArgs {
            mode: Mode::Polyphonic,
            input: wav.clone(),
            out: out.path().join("notes.json"),
            runtime: Some(RuntimeSpec {
                library: lib.clone(),
                threads: 0,
                device: Device::Cpu,
                sha256: None,
            }),
            model: Some(NoteModel {
                graph: graph.clone(),
                family: NoteFamily::BasicPitch,
                check: Vec::new(),
            }),
            notes: NoteSettings::default(),
            drums: DrumSettings::default(),
        };
        let id = c.start("transcribe", serde_json::to_value(&args).unwrap());
        let r: TranscribeResult = match c.wait(id, Duration::from_secs(600)) {
            Some(Outcome::Done(v)) => serde_json::from_value(v).unwrap(),
            other => panic!("{other:?}"),
        };
        let got: Transcription = serde_json::from_slice(&std::fs::read(&r.file).unwrap()).unwrap();
        let theirs = want["notes"].as_array().unwrap();
        let key = |s: f64, e: f64, p: u64| ((s * 1e4).round() as i64, (e * 1e4).round() as i64, p);
        let mut a: Vec<_> = got
            .notes
            .iter()
            .map(|n| key(n.start, n.end, n.pitch as u64))
            .collect();
        let mut b: Vec<_> = theirs
            .iter()
            .map(|n| {
                key(
                    n["start"].as_f64().unwrap(),
                    n["end"].as_f64().unwrap(),
                    n["pitch"].as_u64().unwrap(),
                )
            })
            .collect();
        a.sort();
        b.sort();
        let common = a.iter().filter(|x| b.binary_search(x).is_ok()).count();
        let d = a.len().max(b.len()) - common;
        all += b.len();
        differ += d;
        println!(
            "{}: {:.1} s in {:.2} s ({:.0}x real time, {:.2} s in the network); posteriorgrams within {worst:.1e}; {} notes, {} of Basic Pitch's {} differ",
            wav.file_name().unwrap().to_string_lossy(),
            r.duration,
            r.seconds,
            r.duration / r.seconds,
            r.network_seconds,
            a.len(),
            d,
            b.len()
        );
        assert!(worst <= 1e-4, "posteriorgrams differ by {worst}");
    }
    println!("in all: {differ} of {all} notes differ");
    assert!(differ * 100 <= all, "{differ} of {all} notes differ");
}

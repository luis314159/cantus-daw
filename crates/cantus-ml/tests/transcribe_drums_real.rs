//! Drums with Onsets and Frames Drums and Cantus's hi-hats, measured (ADR 0086 §3): MDB
//! Drums (the drum stems, or `CANTUS_MDB_MIX=1` the mixes; the first eight tracks tuned
//! the thresholds, the other fifteen are held out) and, when given, Pulsus's kits playing
//! a known pattern (`cantus-engine/tests/transcribe_drums.rs` writes them with
//! `CANTUS_DRUMS_WRITE`). Ignored: it needs the runtime, the model and the datasets.
//!
//! ```text
//! CANTUS_ORT_LIBRARY=…/onnxruntime.dll CANTUS_OAF_DRUMS=…/oaf_drums.onnx \
//! CANTUS_MDB_DRUMS=".../MDB Drums" CANTUS_DRUMS_EXTRA=…/pulsus-renders \
//! cargo test -p cantus-ml --release --test transcribe_drums_real -- --ignored --nocapture
//! ```

use cantus_dsp::onset::eval::{onsets, Score};
use cantus_ml::spec::{Device, RuntimeSpec};
use cantus_ml::transcribe::drums_model;
use cantus_ml::transcribe::spec::DrumClass;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

const CLASSES: [DrumClass; 5] = [
    DrumClass::Kick,
    DrumClass::Snare,
    DrumClass::Toms,
    DrumClass::HiHat,
    DrumClass::Cymbals,
];

/// A track: its name, its audio, its annotations.
type Track = (String, PathBuf, PathBuf);

fn class(code: &str) -> Option<DrumClass> {
    match code {
        "KD" => Some(DrumClass::Kick),
        "SD" => Some(DrumClass::Snare),
        "TT" => Some(DrumClass::Toms),
        "HH" => Some(DrumClass::HiHat),
        "CY" => Some(DrumClass::Cymbals),
        _ => None,
    }
}

fn truth(path: &Path) -> Vec<(f64, DrumClass)> {
    std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .filter_map(|l| {
            let mut it = l.split_whitespace();
            let t: f64 = it.next()?.parse().ok()?;
            Some((t, class(it.next()?)?))
        })
        .collect()
}

/// A WAV as 44.1 kHz mono (MDB and the renders are at 44.1 kHz).
fn read(path: &Path) -> Vec<f32> {
    let (x, rate) = cantus_ml::transcribe::read_mono(path).unwrap();
    assert_eq!(rate, 44_100, "{}", path.display());
    x
}

#[test]
#[ignore = "needs the runtime, the drum model and MDB Drums (see the file's header)"]
fn drums_with_the_model_per_class() {
    let lib = PathBuf::from(std::env::var_os("CANTUS_ORT_LIBRARY").expect("CANTUS_ORT_LIBRARY"));
    let graph = PathBuf::from(std::env::var_os("CANTUS_OAF_DRUMS").expect("CANTUS_OAF_DRUMS"));
    let mdb = PathBuf::from(std::env::var_os("CANTUS_MDB_DRUMS").expect("CANTUS_MDB_DRUMS"));
    let mix = std::env::var_os("CANTUS_MDB_MIX").is_some();
    let mut runtime = cantus_ml::runtime::Runtime::new();
    let rt = RuntimeSpec {
        library: lib,
        threads: 0,
        device: Device::Cpu,
        sha256: None,
    };
    let mut names: Vec<String> = std::fs::read_dir(mdb.join("annotations").join("class"))
        .unwrap()
        .map(|e| {
            e.unwrap()
                .file_name()
                .to_string_lossy()
                .replace("_class.txt", "")
        })
        .collect();
    names.sort();
    let mut sets: Vec<(&str, Vec<Track>)> = vec![
        ("MDB tuning (first 8)", Vec::new()),
        ("MDB held out (15)", Vec::new()),
    ];
    for (i, n) in names.iter().enumerate() {
        let wav = if mix {
            mdb.join("audio")
                .join("full_mix")
                .join(format!("{n}_MIX.wav"))
        } else {
            mdb.join("audio")
                .join("drum_only")
                .join(format!("{n}_Drum.wav"))
        };
        let ann = mdb
            .join("annotations")
            .join("class")
            .join(format!("{n}_class.txt"));
        sets[usize::from(i >= 8)].1.push((n.clone(), wav, ann));
    }
    if let Some(dir) = std::env::var_os("CANTUS_DRUMS_EXTRA").map(PathBuf::from) {
        let mut v: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.to_string_lossy().ends_with("_Drum.wav"))
            .map(|w| {
                let ann = PathBuf::from(w.to_string_lossy().replace("_Drum.wav", "_class.txt"));
                (w.file_name().unwrap().to_string_lossy().to_string(), w, ann)
            })
            .collect();
        v.sort();
        sets.push(("Pulsus's kits", v));
    }
    let cancel = AtomicBool::new(false);
    for (label, set) in &sets {
        let mut per: Vec<Vec<Score>> = vec![Vec::new(); 5];
        let mut alone: Vec<Vec<Score>> = vec![Vec::new(); 5];
        let mut seconds = (0.0, 0.0);
        for (_, wav, ann) in set {
            let x = read(wav);
            let t = truth(ann);
            let t0 = std::time::Instant::now();
            let (mut m, _) = runtime.model(&rt, &graph, &[], &mut |_, _| {}).unwrap();
            let p = drums_model::probs(&mut m, &x, &cancel, &mut |_| {}).unwrap();
            let found = drums_model::hybrid(&x, 44_100, 0.5, &p, &|| false).unwrap();
            seconds.0 += t0.elapsed().as_secs_f64();
            seconds.1 += x.len() as f64 / 44_100.0;
            let dsp = cantus_ml::transcribe::drums(&x, 44_100, 0.5);
            for (i, c) in CLASSES.iter().enumerate() {
                let tr: Vec<f64> = t.iter().filter(|h| h.1 == *c).map(|h| h.0).collect();
                let f: Vec<f64> = found
                    .iter()
                    .filter(|h| h.class == *c)
                    .map(|h| h.time)
                    .collect();
                let g: Vec<f64> = dsp
                    .iter()
                    .filter(|h| h.class == *c)
                    .map(|h| h.time)
                    .collect();
                per[i].push(onsets(&tr, &f, 0.05));
                alone[i].push(onsets(&tr, &g, 0.05));
            }
        }
        let line = |v: &Vec<Vec<Score>>| {
            let scores: Vec<Score> = v.iter().map(|s| Score::sum(s)).collect();
            let pooled = Score::sum(&scores);
            format!(
                "{} · all classes {:.3}",
                CLASSES
                    .iter()
                    .zip(&scores)
                    .map(|(c, s)| format!(
                        "{c:?} {:.3} (P {:.2} R {:.2})",
                        s.f, s.precision, s.recall
                    ))
                    .collect::<Vec<_>>()
                    .join(" · "),
                pooled.f
            )
        };
        println!("{label}{}:", if mix { ", mixes" } else { "" });
        println!(
            "  with the model: {}  ({:.0}x real time)",
            line(&per),
            seconds.1 / seconds.0
        );
        println!("  detector alone: {}", line(&alone));
    }
}

/// Drums on MDB's full mixes after HT-Demucs separates the drums (the helper's own
/// `separate` job, then the drum stem transcribed): what *Separate the drums first* buys.
/// Needs `CANTUS_HTDEMUCS` (the graph, its weights beside it) besides the above.
#[test]
#[ignore = "needs the runtime, both models and MDB Drums (see the file's header)"]
fn drums_on_mixes_after_separation() {
    use cantus_ml::client::{ClientConfig, MlClient, Outcome};
    use cantus_ml::spec::{Family, ModelSpec, SeparateArgs, SeparateResult};
    let lib = PathBuf::from(std::env::var_os("CANTUS_ORT_LIBRARY").expect("CANTUS_ORT_LIBRARY"));
    let graph = PathBuf::from(std::env::var_os("CANTUS_OAF_DRUMS").expect("CANTUS_OAF_DRUMS"));
    let demucs = PathBuf::from(std::env::var_os("CANTUS_HTDEMUCS").expect("CANTUS_HTDEMUCS"));
    let mdb = PathBuf::from(std::env::var_os("CANTUS_MDB_DRUMS").expect("CANTUS_MDB_DRUMS"));
    let rt = RuntimeSpec {
        library: lib,
        threads: 0,
        device: Device::Cpu,
        sha256: None,
    };
    let mut names: Vec<String> = std::fs::read_dir(mdb.join("annotations").join("class"))
        .unwrap()
        .map(|e| {
            e.unwrap()
                .file_name()
                .to_string_lossy()
                .replace("_class.txt", "")
        })
        .collect();
    names.sort();
    let mut client = MlClient::new(ClientConfig::new(env!("CARGO_BIN_EXE_cantus-ml")));
    let mut runtime = cantus_ml::runtime::Runtime::new();
    let cancel = AtomicBool::new(false);
    let out = tempfile::tempdir().unwrap();
    let mut with: Vec<Vec<Score>> = vec![Vec::new(); 5];
    let mut alone: Vec<Vec<Score>> = vec![Vec::new(); 5];
    for (i, n) in names.iter().enumerate() {
        if i < 8 {
            continue;
        }
        let mix = mdb
            .join("audio")
            .join("full_mix")
            .join(format!("{n}_MIX.wav"));
        let args = SeparateArgs {
            runtime: rt.clone(),
            model: ModelSpec {
                graph: demucs.clone(),
                family: Family::HtDemucs,
                sources: ["drums", "bass", "other", "vocals"]
                    .iter()
                    .map(|s| s.to_string())
                    .collect(),
                sample_rate: 44_100,
                segment: 343_980,
                check: Vec::new(),
            },
            input: mix,
            out_dir: out.path().join(n),
            overlap: 0.25,
            shifts: 0,
        };
        let id = client.start("separate", serde_json::to_value(&args).unwrap());
        let r: SeparateResult = match client.wait(id, std::time::Duration::from_secs(1800)) {
            Some(Outcome::Done(v)) => serde_json::from_value(v).unwrap(),
            other => panic!("{other:?}"),
        };
        let drums = r
            .stems
            .iter()
            .find(|s| s.name == "drums")
            .unwrap()
            .path
            .clone();
        let x = read(&drums);
        let t = truth(
            &mdb.join("annotations")
                .join("class")
                .join(format!("{n}_class.txt")),
        );
        let (mut m, _) = runtime.model(&rt, &graph, &[], &mut |_, _| {}).unwrap();
        let p = drums_model::probs(&mut m, &x, &cancel, &mut |_| {}).unwrap();
        let a = drums_model::hybrid(&x, 44_100, 0.5, &p, &|| false).unwrap();
        let b = cantus_ml::transcribe::drums(&x, 44_100, 0.5);
        for (k, c) in CLASSES.iter().enumerate() {
            let tr: Vec<f64> = t.iter().filter(|h| h.1 == *c).map(|h| h.0).collect();
            let f: Vec<f64> = a.iter().filter(|h| h.class == *c).map(|h| h.time).collect();
            let g: Vec<f64> = b.iter().filter(|h| h.class == *c).map(|h| h.time).collect();
            with[k].push(onsets(&tr, &f, 0.05));
            alone[k].push(onsets(&tr, &g, 0.05));
        }
        println!("{n} separated in {:.1} s", r.seconds);
    }
    for (label, v) in [("with the model", &with), ("detector alone", &alone)] {
        let scores: Vec<Score> = v.iter().map(|s| Score::sum(s)).collect();
        println!(
            "MDB held-out mixes, drums separated first, {label}: {} · all {:.3}",
            CLASSES
                .iter()
                .zip(&scores)
                .map(|(c, s)| format!("{c:?} {:.3}", s.f))
                .collect::<Vec<_>>()
                .join(" · "),
            Score::sum(&scores).f
        );
    }
}

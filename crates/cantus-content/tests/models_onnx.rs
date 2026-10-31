//! A model whose maker ships its ONNX graph (Basic Pitch, ADR 0086): the bundled entry
//! valid, and an install end to end from `file://` fixtures — the licence gate, the graph
//! used as it arrived (nothing rebuilt, nothing removed), ready across a restart, a graph
//! touched after it arrived no longer ready, and validation refusing a graph that is not
//! among the model's files.

use cantus_content::fetch::{file_url, Net};
use cantus_content::install::Patience;
use cantus_content::manifest::{Credit, FileEntry, Source, Text, ZipSource};
use cantus_content::models::*;
use sha2::{Digest, Sha256};
use std::io::Write;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

const GRAPH: &[u8] = b"\x08\x08an onnx graph, weights inside";

fn manifest(dir: &Path) -> ModelManifest {
    let sha_file = |p: &Path| hex(&Sha256::digest(std::fs::read(p).unwrap()));
    let graph = dir.join("nmp.onnx");
    std::fs::write(&graph, GRAPH).unwrap();
    let licence = dir.join("LICENSE");
    std::fs::write(&licence, "Apache License\n").unwrap();
    let rt = dir.join("rt.zip");
    {
        let mut z = zip::ZipWriter::new(std::fs::File::create(&rt).unwrap());
        let o = zip::write::SimpleFileOptions::default();
        z.start_file("pkg/lib/runtime.bin", o).unwrap();
        z.write_all(b"a library").unwrap();
        z.start_file("pkg/LICENSE", o).unwrap();
        z.write_all(b"MIT License\n").unwrap();
        z.finish().unwrap();
    }
    let mut m = ModelManifest::bundled();
    let credit = Credit {
        name: "Us".into(),
        url: "https://example.com".into(),
        licence_url: "https://example.com/l".into(),
    };
    for r in m.runtimes.iter_mut() {
        r.id = format!("rt-{}", r.platform.replace('_', "-"));
        r.library = "lib/runtime.bin".into();
        r.installed_size = 100;
        r.credits = vec![credit.clone()];
        r.source = Source::Zip(ZipSource {
            url: file_url(&rt),
            size: std::fs::metadata(&rt).unwrap().len(),
            sha256: sha_file(&rt),
            strip: "pkg/".into(),
            include: vec!["lib/runtime.bin".into(), "LICENSE".into()],
            exclude: vec![],
        });
    }
    let mut model = m.model("basic-pitch").expect("bundled").clone();
    model.id = "notes".into();
    model.name = Text {
        en: "Notes".into(),
        es: "Notas".into(),
    };
    model.files = vec![
        FileEntry {
            url: file_url(&graph),
            to: "nmp.onnx".into(),
            size: GRAPH.len() as u64,
            sha256: sha_file(&graph),
        },
        FileEntry {
            url: file_url(&licence),
            to: "LICENSE".into(),
            size: 15,
            sha256: sha_file(&licence),
        },
    ];
    m.models = vec![model];
    m
}

fn models(root: &Path, m: &ModelManifest) -> Models {
    Models::with_transport(root, m.clone(), Arc::new(Net::new()), Patience::none())
}

#[test]
fn the_bundled_basic_pitch_is_valid_permissive_and_onnx() {
    let m = ModelManifest::bundled();
    m.validate(true).unwrap();
    let bp = m.model("basic-pitch").unwrap();
    assert_eq!(bp.licence, "Apache-2.0");
    assert_eq!(bp.task, Task::Transcription);
    assert_eq!(
        bp.prepare,
        Prepare::Onnx {
            file: "nmp.onnx".into(),
            patch: None,
        }
    );
    assert_eq!(bp.sample_rate, 22_050);
    assert!(bp.files.iter().all(|f| f
        .url
        .contains("spotify/basic-pitch/fa5997af0a8210982619003269994a1be25eddf3/")));
    assert_eq!(bp.installed_size, bp.download_size());
    assert!(!bp.name.es.is_empty() && !bp.description.es.is_empty());
}

#[test]
fn an_onnx_model_installs_as_it_arrived_and_is_ready() {
    let src = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let man = manifest(src.path());
    man.validate(false).unwrap();
    let mut m = models(root.path(), &man);
    assert!(m.install("notes").unwrap_err().contains("licence"));
    m.accept("notes").unwrap();
    m.install("notes").unwrap();
    let events = m.wait_idle(Duration::from_secs(30));
    assert_eq!(
        events,
        vec![ModelEvent::Ready("notes".into())],
        "{:?}",
        m.status("notes")
    );
    let r = m.ready("notes").unwrap();
    assert_eq!(r.graph.file_name().unwrap(), "nmp.onnx");
    assert_eq!(std::fs::read(&r.graph).unwrap(), GRAPH);
    assert_eq!(r.family, "basic_pitch");
    assert_eq!(r.sample_rate, 22_050);
    assert!(!r.graph.with_extension("weights").exists());
    drop(m);
    let m = models(root.path(), &man);
    assert_eq!(
        m.status("notes"),
        ModelStatus::Ready,
        "remembered across a restart"
    );
    // A graph touched after it arrived is no longer whole: its size differs.
    std::fs::write(&r.graph, b"short").unwrap();
    let m2 = models(root.path(), &man);
    assert_ne!(m2.status("notes"), ModelStatus::Ready);
}

#[test]
fn validation_refuses_an_onnx_that_is_not_among_the_files() {
    let src = tempfile::tempdir().unwrap();
    let mut man = manifest(src.path());
    man.models[0].prepare = Prepare::Onnx {
        file: "other.onnx".into(),
        patch: None,
    };
    assert!(man.validate(false).unwrap_err().contains("other.onnx"));
    man.models[0].prepare = Prepare::Onnx {
        file: "LICENSE".into(),
        patch: None,
    };
    assert!(man.validate(false).is_err());
}

fn sha(b: &[u8]) -> String {
    hex(&Sha256::digest(b))
}

#[test]
fn a_patched_onnx_is_edited_in_a_copy_and_its_hash_held() {
    let src = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let mut man = manifest(src.path());
    // GRAPH holds "graph" once: it becomes "GRAPH" in the copy.
    let edited: Vec<u8> = String::from_utf8_lossy(GRAPH)
        .replace("graph", "GRAPH")
        .into_bytes();
    let patch = OnnxPatch {
        find: hex(b"graph"),
        replace: hex(b"GRAPH"),
        sha256: sha(&edited),
    };
    man.models[0].prepare = Prepare::Onnx {
        file: "nmp.onnx".into(),
        patch: Some(patch.clone()),
    };
    man.validate(false).unwrap();
    let mut m = models(root.path(), &man);
    m.accept("notes").unwrap();
    m.install("notes").unwrap();
    m.wait_idle(Duration::from_secs(30));
    let r = m.ready("notes").expect("ready");
    assert_eq!(r.graph.file_name().unwrap(), "nmp.cantus.onnx");
    assert_eq!(std::fs::read(&r.graph).unwrap(), edited);
    assert_eq!(
        std::fs::read(r.graph.with_file_name("nmp.onnx")).unwrap(),
        GRAPH,
        "the maker's file untouched"
    );
    // The edit is exact: bytes found twice or not at all, or another result, are refused.
    assert!(cantus_content::models::onnx_apply(
        b"ab ab",
        &OnnxPatch {
            find: hex(b"ab"),
            replace: hex(b"cd"),
            sha256: sha(b"x")
        }
    )
    .is_err());
    assert!(cantus_content::models::onnx_apply(
        b"xyz",
        &OnnxPatch {
            find: hex(b"ab"),
            replace: hex(b"cd"),
            sha256: sha(b"x")
        }
    )
    .is_err());
    let src2 = tempfile::tempdir().unwrap();
    let root2 = tempfile::tempdir().unwrap();
    let mut bad = manifest(src2.path());
    bad.models[0].prepare = Prepare::Onnx {
        file: "nmp.onnx".into(),
        patch: Some(OnnxPatch {
            sha256: sha(b"something else"),
            ..patch.clone()
        }),
    };
    let mut m = models(root2.path(), &bad);
    m.accept("notes").unwrap();
    m.install("notes").unwrap();
    m.wait_idle(Duration::from_secs(30));
    assert!(
        matches!(m.status("notes"), ModelStatus::Failed(_)),
        "{:?}",
        m.status("notes")
    );
    // Validation: lengths must match.
    bad.models[0].prepare = Prepare::Onnx {
        file: "nmp.onnx".into(),
        patch: Some(OnnxPatch {
            replace: hex(b"GRAPHS"),
            ..patch
        }),
    };
    assert!(bad.validate(false).is_err());
}

#[test]
fn the_drum_models_patch_points_the_hat_at_pitch_46() {
    // The conversion's output gather: eight MIDI pitches less 21 as little-endian int64.
    let m = ModelManifest::bundled();
    let Prepare::Onnx { patch: Some(p), .. } = &m.model("oaf-drums").unwrap().prepare else {
        panic!("the drum model names its patch");
    };
    let pitches = |h: &str| -> Vec<i64> {
        (0..8)
            .map(|i| i64::from_str_radix(&h[i * 16..i * 16 + 2], 16).unwrap() + 21)
            .collect()
    };
    assert_eq!(pitches(&p.find), [36, 38, 48, 42, 51, 53, 49, 75]);
    assert_eq!(pitches(&p.replace), [36, 38, 48, 46, 51, 53, 49, 75]);
}

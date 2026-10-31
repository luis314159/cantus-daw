//! Machine-learning models through the content manager (ADR 0085): the bundled list and
//! its licences, the graphs compiled in, and an install end to end from `file://`
//! fixtures — a small PyTorch file of our own making, its weights rebuilt and checked —
//! with the licence gate, removal, and a damaged download.

use cantus_content::fetch::{file_url, Net};
use cantus_content::install::Patience;
use cantus_content::manifest::{Credit, FileEntry, Source, Text, ZipSource};
use cantus_content::models::*;
use cantus_content::models_th::{MapEntry, WeightMap};
use sha2::{Digest, Sha256};
use std::io::Write;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

#[test]
fn the_bundled_list_is_valid_and_every_licence_permissive() {
    let m = ModelManifest::bundled();
    m.validate(true).unwrap();
    assert!(m.models.iter().any(|x| x.recommended));
    for x in &m.models {
        assert!(LICENCES.contains(&x.licence.as_str()));
        for p in PLATFORMS {
            assert!(m.runtime(&x.runtime, p).is_some(), "{} on {p}", x.id);
        }
        assert!(!x.credits.is_empty() && !x.provenance.training.en.is_empty());
        assert!(
            !x.name.es.is_empty() && !x.description.es.is_empty(),
            "{} in Spanish",
            x.id
        );
        let card = &x.provenance;
        assert!(
            [
                &card.weights,
                &card.training,
                &card.quality,
                &card.conversion
            ]
            .iter()
            .all(|t| !t.en.is_empty() && !t.es.is_empty()),
            "{}'s provenance card in Spanish",
            x.id
        );
    }
    for r in &m.runtimes {
        assert!(LICENCES.contains(&r.licence.as_str()));
    }
    // This build's own system has a runtime.
    assert!(PLATFORMS.contains(&platform().as_str()), "{}", platform());
}

#[test]
fn the_compiled_in_graphs_are_the_ones_their_maps_were_made_with() {
    for (name, gz, _) in GRAPHS {
        let (_, map) = graph(name).unwrap();
        let mut onnx = Vec::new();
        std::io::Read::read_to_end(&mut flate2::read::GzDecoder::new(gz), &mut onnx).unwrap();
        assert_eq!(hex(&Sha256::digest(&onnx)), map.graph_sha256, "{name}");
        // ONNX is protobuf: the first field is ir_version (tag 0x08).
        assert_eq!(onnx[0], 0x08, "{name}");
        // Offsets rise, aligned, inside the file.
        let mut end = 0;
        for t in &map.tensors {
            assert!(t.offset >= end && t.offset % 64 == 0, "{name}: {}", t.key);
            end = t.offset + t.shape.iter().product::<u64>() * 2;
        }
        assert_eq!(end, map.weights_size);
    }
}

#[test]
fn validation_refuses_what_adr_0042_refuses() {
    let base = ModelManifest::bundled();
    let mut m = base.clone();
    m.models[0].licence = "CC-BY-NC-4.0".into();
    assert!(m.validate(true).unwrap_err().contains("licence"));
    let mut m = base.clone();
    m.models[0].files[0].url = "http://example.com/x.th".into();
    assert!(m.validate(true).is_err());
    let mut m = base.clone();
    m.models[0].files[0].sha256 = "0".repeat(64);
    assert!(m.validate(true).unwrap_err().contains("another file"));
    let mut m = base.clone();
    m.runtimes.retain(|r| r.platform != "linux-x86_64");
    assert!(m.validate(true).unwrap_err().contains("every system"));
    let mut m = base;
    m.runtimes[0].licence = "GPL-3.0-only".into();
    assert!(m.validate(true).is_err());
}

// A small PyTorch file of our own: what `torch.save({"state": OrderedDict(...)})` writes,
// pickle and storages, for two half-precision tensors.

fn pickle_state(tensors: &[(&str, &str, Vec<u64>)]) -> Vec<u8> {
    let s = |out: &mut Vec<u8>, v: &str| {
        out.push(b'X');
        out.extend((v.len() as u32).to_le_bytes());
        out.extend(v.as_bytes());
    };
    let int = |out: &mut Vec<u8>, v: u64| {
        out.push(b'J');
        out.extend((v as u32).to_le_bytes());
    };
    let mut p = vec![0x80, 2, b'}', b'('];
    s(&mut p, "state");
    p.extend(b"ccollections\nOrderedDict\n)R(");
    for (key, storage, shape) in tensors {
        s(&mut p, key);
        p.extend(b"ctorch._utils\n_rebuild_tensor_v2\n(");
        // The storage's persistent id.
        p.push(b'(');
        s(&mut p, "storage");
        p.extend(b"ctorch\nHalfStorage\n");
        s(&mut p, storage);
        s(&mut p, "cpu");
        int(&mut p, shape.iter().product());
        p.extend(b"tQ");
        int(&mut p, 0);
        p.push(b'(');
        for d in shape {
            int(&mut p, *d);
        }
        p.push(b't');
        p.push(b'(');
        let mut stride: Vec<u64> = shape
            .iter()
            .rev()
            .scan(1, |acc, d| {
                let s = *acc;
                *acc *= d;
                Some(s)
            })
            .collect();
        stride.reverse();
        for d in &stride {
            int(&mut p, *d);
        }
        p.push(b't');
        p.push(0x89);
        p.extend(b"ccollections\nOrderedDict\n)R");
        p.extend(b"tR");
    }
    p.extend(b"uu.");
    p
}

struct Fixture {
    manifest: ModelManifest,
    gz: Vec<u8>,
    map: WeightMap,
}

fn fixture(dir: &Path) -> Fixture {
    // Two tensors: [3, 2] and [5]; f16 bit patterns 0..6 and 100..105.
    let a: Vec<u8> = (0u16..6).flat_map(|v| v.to_le_bytes()).collect();
    let b: Vec<u8> = (100u16..105).flat_map(|v| v.to_le_bytes()).collect();
    let th = dir.join("tiny.th");
    {
        let mut z = zip::ZipWriter::new(std::fs::File::create(&th).unwrap());
        let o = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        z.start_file("tiny/data.pkl", o).unwrap();
        z.write_all(&pickle_state(&[
            ("enc.weight", "0", vec![3, 2]),
            ("enc.bias", "1", vec![5]),
        ]))
        .unwrap();
        z.start_file("tiny/data/0", o).unwrap();
        z.write_all(&a).unwrap();
        z.start_file("tiny/data/1", o).unwrap();
        z.write_all(&b).unwrap();
        z.finish().unwrap();
    }
    let licence = dir.join("LICENSE");
    std::fs::write(&licence, "MIT License\n").unwrap();
    // The weights file the map describes: a at 0, b at 64.
    let mut weights = a.clone();
    weights.resize(64, 0);
    weights.extend(&b);
    let graph = b"\x08\x08not really a graph".to_vec();
    let mut gz = Vec::new();
    {
        let mut e = flate2::write::GzEncoder::new(&mut gz, flate2::Compression::best());
        e.write_all(&graph).unwrap();
        e.finish().unwrap();
    }
    let sha_file = |p: &Path| hex(&Sha256::digest(std::fs::read(p).unwrap()));
    let map = WeightMap {
        format: 1,
        source_sha256: sha_file(&th),
        graph_sha256: hex(&Sha256::digest(&graph)),
        weights_size: weights.len() as u64,
        weights_sha256: hex(&Sha256::digest(&weights)),
        tensors: vec![
            MapEntry {
                key: "enc.weight".into(),
                offset: 0,
                dtype: "f16".into(),
                shape: vec![3, 2],
            },
            MapEntry {
                key: "enc.bias".into(),
                offset: 64,
                dtype: "f16".into(),
                shape: vec![5],
            },
        ],
    };
    // A runtime archive: a "library" and its licence.
    let rt = dir.join("rt.zip");
    {
        let mut z = zip::ZipWriter::new(std::fs::File::create(&rt).unwrap());
        let o = zip::write::SimpleFileOptions::default();
        z.start_file("pkg/lib/runtime.bin", o).unwrap();
        z.write_all(b"a library").unwrap();
        z.start_file("pkg/LICENSE", o).unwrap();
        z.write_all(b"MIT License\n").unwrap();
        z.start_file("pkg/docs/huge.pdb", o).unwrap();
        z.write_all(b"left out").unwrap();
        z.finish().unwrap();
    }
    let mut manifest = ModelManifest::bundled();
    let credit = Credit {
        name: "Us".into(),
        url: "https://example.com".into(),
        licence_url: "https://example.com/l".into(),
    };
    for r in manifest.runtimes.iter_mut() {
        r.id = format!("rt-{}", r.platform.replace('_', "-"));
        r.library = "lib/runtime.bin".into();
        r.library_sha256 = hex(&Sha256::digest(b"a library"));
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
    let mut model = manifest.models[0].clone();
    model.id = "tiny".into();
    model.name = Text {
        en: "Tiny".into(),
        es: "Diminuto".into(),
    };
    model.files = vec![
        FileEntry {
            url: file_url(&th),
            to: "tiny.th".into(),
            size: std::fs::metadata(&th).unwrap().len(),
            sha256: sha_file(&th),
        },
        FileEntry {
            url: file_url(&licence),
            to: "LICENSE".into(),
            size: 12,
            sha256: sha_file(&licence),
        },
    ];
    model.prepare = Prepare::DemucsTh {
        th: "tiny.th".into(),
        graph: "tiny".into(),
    };
    model.installed_size = 1000;
    manifest.models = vec![model];
    Fixture { manifest, gz, map }
}

fn models(root: &Path, f: &Fixture) -> Models {
    Models::with_graphs(
        root,
        f.manifest.clone(),
        Arc::new(Net::new()),
        Patience::none(),
        vec![("tiny".into(), f.gz.clone(), f.map.clone())],
    )
}

const WAIT: Duration = Duration::from_secs(30);

#[test]
fn a_model_installs_only_once_its_licence_is_accepted_and_is_then_ready() {
    let src = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let f = fixture(src.path());
    let mut m = models(root.path(), &f);
    assert_eq!(m.status("tiny"), ModelStatus::Absent { accepted: false });
    assert!(m.install("tiny").unwrap_err().contains("licence"));
    m.accept("tiny").unwrap();
    assert_eq!(m.status("tiny"), ModelStatus::Absent { accepted: true });
    m.install("tiny").unwrap();
    let events = m.wait_idle(WAIT);
    assert_eq!(
        events,
        vec![ModelEvent::Ready("tiny".into())],
        "{:?}",
        m.status("tiny")
    );
    assert_eq!(m.status("tiny"), ModelStatus::Ready);
    let r = m.ready("tiny").unwrap();
    assert_eq!(
        std::fs::read(&r.graph).unwrap(),
        b"\x08\x08not really a graph"
    );
    assert_eq!(std::fs::read(&r.library).unwrap(), b"a library");
    let w = std::fs::read(r.graph.with_extension("weights")).unwrap();
    assert_eq!(
        &w[..12],
        &(0u16..6).flat_map(|v| v.to_le_bytes()).collect::<Vec<_>>()[..]
    );
    assert_eq!(
        &w[64..],
        &(100u16..105)
            .flat_map(|v| v.to_le_bytes())
            .collect::<Vec<_>>()[..]
    );
    // The maker's file is gone once its weights are in place; the licence stays.
    let dir = r.graph.parent().unwrap();
    assert!(!dir.join("tiny.th").exists());
    assert!(dir.join("LICENSE").is_file());
    assert!(!r
        .library
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("docs")
        .exists());
    // The acceptance is remembered by a new store over the same folder, and so is the
    // install.
    drop(m);
    let m = models(root.path(), &f);
    assert!(m.accepted("tiny"));
    assert_eq!(m.status("tiny"), ModelStatus::Ready);
}

#[test]
fn a_changed_licence_asks_again() {
    let src = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let mut f = fixture(src.path());
    let mut m = models(root.path(), &f);
    m.accept("tiny").unwrap();
    drop(m);
    f.manifest.models[0].licence = "Apache-2.0".into();
    let m = models(root.path(), &f);
    assert!(!m.accepted("tiny"));
}

#[test]
fn removing_the_last_model_removes_its_runtime() {
    let src = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let f = fixture(src.path());
    let mut m = models(root.path(), &f);
    m.accept("tiny").unwrap();
    m.install("tiny").unwrap();
    m.wait_idle(WAIT);
    let r = m.ready("tiny").unwrap();
    m.remove("tiny").unwrap();
    assert_eq!(m.status("tiny"), ModelStatus::Absent { accepted: true });
    assert!(!r.graph.exists() && !r.library.exists());
}

#[test]
fn weights_that_do_not_rebuild_to_their_hash_fail_the_install() {
    let src = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let mut f = fixture(src.path());
    f.map.weights_sha256 = "0".repeat(64);
    let mut m = models(root.path(), &f);
    m.accept("tiny").unwrap();
    m.install("tiny").unwrap();
    let events = m.wait_idle(WAIT);
    match &events[..] {
        [ModelEvent::Failed(id, e)] => {
            assert_eq!(id, "tiny");
            assert!(e.contains("hash"), "{e}");
        }
        other => panic!("{other:?}"),
    }
    assert!(matches!(m.status("tiny"), ModelStatus::Failed(_)));
    assert!(m.ready("tiny").is_none());
    let dir = root.path().join("tiny");
    assert!(!dir.join("tiny.weights").exists() && !dir.join(MARKER).exists());
}

#[test]
fn a_download_that_is_not_the_file_is_refused() {
    let src = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let f = fixture(src.path());
    std::fs::write(
        src.path().join("tiny.th"),
        b"<html>blocked by a proxy</html>",
    )
    .unwrap();
    let mut m = models(root.path(), &f);
    m.accept("tiny").unwrap();
    m.install("tiny").unwrap();
    let events = m.wait_idle(WAIT);
    assert!(
        matches!(&events[..], [ModelEvent::Failed(..)]),
        "{events:?}"
    );
    assert!(m.ready("tiny").is_none());
}

/// The job checks what `ready` names against these hashes (review 1 #5); a file changed
/// after its preparing is not ready.
#[test]
fn ready_names_every_hash_and_a_changed_file_is_not_ready() {
    let src = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let f = fixture(src.path());
    let mut m = models(root.path(), &f);
    m.accept("tiny").unwrap();
    m.install("tiny").unwrap();
    m.wait_idle(WAIT);
    let r = m.ready("tiny").unwrap();
    assert_eq!(r.library_sha256, hex(&Sha256::digest(b"a library")));
    for (p, sha) in &r.files {
        assert_eq!(
            &hex(&Sha256::digest(std::fs::read(p).unwrap())),
            sha,
            "{}",
            p.display()
        );
    }
    // The weights changed, the same size: still ready to the eye — a start never hashes
    // every model (review 2 #15) — and the job's check refuses them, since the hash it
    // gets is the one this build knows, not the file's.
    let w = r.graph.with_extension("weights");
    let mut bytes = std::fs::read(&w).unwrap();
    bytes[0] ^= 0xff;
    std::fs::write(&w, &bytes).unwrap();
    let m = models(root.path(), &f);
    let again = m.ready("tiny").expect("ready without hashing at start");
    let (_, want) = again.files.iter().find(|(p, _)| *p == w).unwrap();
    assert_ne!(&hex(&Sha256::digest(&bytes)), want);
    // Shorter weights are not ready: the size is checked at once.
    std::fs::write(&w, &bytes[1..]).unwrap();
    assert!(models(root.path(), &f).ready("tiny").is_none());
}

/// The hashes a ready model hands the job are this build's, from its compiled-in weight
/// map — not the marker's on disk, which whoever swaps the files can rewrite too
/// (review 2 #9).
#[test]
fn the_hashes_come_from_the_build_not_the_marker() {
    let src = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let f = fixture(src.path());
    let mut m = models(root.path(), &f);
    m.accept("tiny").unwrap();
    m.install("tiny").unwrap();
    m.wait_idle(WAIT);
    let want = m.ready("tiny").unwrap().files;
    let marker = want[0].0.parent().unwrap().join(MARKER);
    let text = std::fs::read_to_string(&marker).unwrap();
    let forged = text
        .replace(&want[0].1, &"0".repeat(64))
        .replace(&want[1].1, &"1".repeat(64));
    assert_ne!(forged, text, "the marker names the hashes");
    std::fs::write(&marker, forged).unwrap();
    let m = models(root.path(), &f);
    assert_eq!(m.ready("tiny").unwrap().files, want);
}

/// A download cut short goes on from where it stopped: the part already there is kept
/// (the file's first half is changed under it, so only a resumed download hashes right).
#[test]
fn a_cut_download_resumes() {
    let src = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let f = fixture(src.path());
    let th = src.path().join("tiny.th");
    let whole = std::fs::read(&th).unwrap();
    let half = whole.len() / 2;
    let sha = hex(&Sha256::digest(&whole));
    let downloads = root.path().join(".downloads");
    std::fs::create_dir_all(&downloads).unwrap();
    std::fs::write(downloads.join(format!("{sha}.part")), &whole[..half]).unwrap();
    let mut changed = whole.clone();
    changed[..half].iter_mut().for_each(|b| *b = 0);
    std::fs::write(&th, changed).unwrap();
    let mut m = models(root.path(), &f);
    m.accept("tiny").unwrap();
    m.install("tiny").unwrap();
    let events = m.wait_idle(WAIT);
    assert_eq!(
        events,
        vec![ModelEvent::Ready("tiny".into())],
        "{:?}",
        m.status("tiny")
    );
}

/// The runtime's licence changing asks again, as the model's does.
#[test]
fn a_changed_runtime_licence_asks_again() {
    let src = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let mut f = fixture(src.path());
    let mut m = models(root.path(), &f);
    m.accept("tiny").unwrap();
    drop(m);
    for r in f.manifest.runtimes.iter_mut() {
        r.licence = "Apache-2.0".into();
    }
    let m = models(root.path(), &f);
    assert!(!m.accepted("tiny"));
}

/// A preparing cut short (Cantus closed while the weights were rebuilt) is done again at
/// the next start, from the maker's file still there.
#[test]
fn an_interrupted_prepare_resumes_at_start() {
    let src = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let f = fixture(src.path());
    // The files arrive but nothing can prepare them (this store has no graph called
    // "tiny"): what a closed Cantus leaves.
    let mut m = Models::with_transport(
        root.path(),
        f.manifest.clone(),
        Arc::new(Net::new()),
        Patience::none(),
    );
    m.accept("tiny").unwrap();
    m.install("tiny").unwrap();
    m.wait_idle(WAIT);
    assert!(m.ready("tiny").is_none());
    assert!(root.path().join("tiny").join("tiny.th").is_file());
    drop(m);
    let mut m = models(root.path(), &f);
    assert_eq!(m.status("tiny"), ModelStatus::Preparing);
    m.wait_idle(WAIT);
    assert_eq!(m.status("tiny"), ModelStatus::Ready);
}

/// The weights gone after the maker's file was removed: an install downloads it again
/// instead of failing forever (review 1 #8).
#[test]
fn lost_weights_are_downloaded_again() {
    let src = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let f = fixture(src.path());
    let mut m = models(root.path(), &f);
    m.accept("tiny").unwrap();
    m.install("tiny").unwrap();
    m.wait_idle(WAIT);
    let r = m.ready("tiny").unwrap();
    std::fs::remove_file(r.graph.with_extension("weights")).unwrap();
    drop(m);
    let mut m = models(root.path(), &f);
    assert_eq!(m.status("tiny"), ModelStatus::Absent { accepted: true });
    m.install("tiny").unwrap();
    let events = m.wait_idle(WAIT);
    assert_eq!(
        events,
        vec![ModelEvent::Ready("tiny".into())],
        "{:?}",
        m.status("tiny")
    );
}

/// The graphs adapt Meta's MIT code: its licence beside them and in the notices
/// (review 1 #4).
#[test]
fn the_graphs_carry_demucss_licence() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let here = std::fs::read_to_string(root.join("models").join("LICENSE")).unwrap();
    assert!(here.contains("Copyright (c) Meta Platforms, Inc. and affiliates"));
    assert!(here.contains("Permission is hereby granted"));
    let notices = std::fs::read_to_string(root.join("../../THIRD-PARTY-NOTICES.md")).unwrap();
    assert!(notices.contains("Demucs (the HT-Demucs graphs)"));
    assert!(notices.contains("Copyright (c) Meta Platforms, Inc. and affiliates"));
}

#[test]
fn a_text_not_translated_yet_reads_as_english() {
    let t: cantus_content::manifest::Text = serde_json::from_str("\"Drums\"").unwrap();
    assert_eq!((t.en.as_str(), t.get("es")), ("Drums", "Drums"));
    let t: cantus_content::manifest::Text =
        serde_json::from_str(r#"{"en": "Drums", "es": "Batería"}"#).unwrap();
    assert_eq!(t.get("es"), "Batería");
}

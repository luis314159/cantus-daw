//! Schema 54 (round 37, ADR 0098): Chorda's second model. Every Chorda saved before plays
//! the first model, said explicitly once opened; a new one is model 2.

use cantus_core::{InstrumentKind, NativeSpec, Project};
use serde_json::Value;
use std::path::PathBuf;

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures")
}

fn chordas(p: &Project) -> Vec<&NativeSpec> {
    p.tracks
        .iter()
        .filter_map(|t| t.instrument.native())
        .filter(|n| n.kind == InstrumentKind::Chorda)
        .collect()
}

#[test]
fn every_older_fixtures_chorda_opens_as_model_1_said_explicitly() {
    let mut seen = 0;
    // Every file saved before schema 54 (the fixtures from v54 on hold a model-2 Chorda).
    for v in 28..54 {
        let path = dir().join(format!("v{v}.cantus"));
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let p = cantus_project::parse(&text, true).unwrap().file.project;
        for c in chordas(&p) {
            assert!(c.is_stored("model"), "v{v}: the model is written");
            assert_eq!(c.get("model"), 1.0, "v{v}");
            seen += 1;
        }
    }
    assert!(seen >= 20, "the fixtures since v28 hold a Chorda ({seen})");
}

/// A Chorda saved with no `model` key at all (a hand-edited file, a writer that left it
/// out) meant the first model: the migration writes it.
#[test]
fn a_chorda_with_no_model_key_opens_as_model_1() {
    let text = std::fs::read_to_string(dir().join("v53.cantus")).unwrap();
    let mut json: Value = serde_json::from_str(&text).unwrap();
    let mut stripped = 0;
    for t in json["project"]["tracks"].as_array_mut().unwrap() {
        if t["instrument"]["kind"] == "Chorda" {
            if let Some(p) = t["instrument"]["params"].as_object_mut() {
                p.remove("model");
            }
            stripped += 1;
        }
    }
    assert!(stripped > 0);
    let text = serde_json::to_string(&json).unwrap();
    let r = cantus_project::parse(&text, true).unwrap();
    assert_eq!(r.migrated_from, Some(53));
    for c in chordas(&r.file.project) {
        assert!(c.is_stored("model"));
        assert_eq!(c.get("model"), 1.0);
    }
}

#[test]
fn the_newest_fixtures_chorda_is_model_2_and_a_new_one_is_too() {
    let newest = format!("v{}.cantus", cantus_project::SCHEMA_VERSION);
    let text = std::fs::read_to_string(dir().join(newest)).unwrap();
    let p = cantus_project::parse(&text, true).unwrap().file.project;
    let c = chordas(&p);
    assert!(!c.is_empty());
    for c in c {
        assert_eq!(c.get("model"), 2.0);
    }
    let fresh = NativeSpec::new(InstrumentKind::Chorda);
    assert!(fresh.is_stored("model"));
    assert_eq!(fresh.get("model"), 2.0);
    assert_eq!(cantus_core::inst::chorda::MODEL, 2.0);
}

/// A model already written is never touched (the migration only fills what is absent).
#[test]
fn a_written_model_is_kept() {
    let text = std::fs::read_to_string(dir().join("v53.cantus")).unwrap();
    let mut json: Value = serde_json::from_str(&text).unwrap();
    for t in json["project"]["tracks"].as_array_mut().unwrap() {
        if t["instrument"]["kind"] == "Chorda" {
            t["instrument"]["params"]["model"] = Value::from(2.0);
        }
    }
    let p = cantus_project::parse(&serde_json::to_string(&json).unwrap(), true)
        .unwrap()
        .file
        .project;
    for c in chordas(&p) {
        assert_eq!(c.get("model"), 2.0);
    }
}

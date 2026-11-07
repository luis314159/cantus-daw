//! Schema 58 (round 37, ADR 0104): Chorda's look. Every Chorda saved before opens with the
//! look it was always shown with — a black concert grand — written into its text; the
//! newest fixture's walnut upright survives a save and a load; a look already there is
//! kept.

use cantus_core::inst::chorda_look::{self, Body, Finish, Look, BODY_KEY, FINISH_KEY};
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
fn every_older_fixtures_chorda_opens_as_a_black_concert_grand_said_explicitly() {
    let mut seen = 0;
    for v in 28..58 {
        let Ok(text) = std::fs::read_to_string(dir().join(format!("v{v}.cantus"))) else {
            continue;
        };
        let p = cantus_project::parse(&text, true).unwrap().file.project;
        for c in chordas(&p) {
            assert_eq!(c.text(BODY_KEY), Some("concert"), "v{v}");
            assert_eq!(c.text(FINISH_KEY), Some("black"), "v{v}");
            assert_eq!(chorda_look::of(c), Look::default(), "v{v}");
            seen += 1;
        }
    }
    assert!(seen >= 20, "the fixtures since v28 hold a Chorda ({seen})");
}

#[test]
fn the_newest_fixtures_look_survives_a_save_and_a_load() {
    let newest = format!("v{}.cantus", cantus_project::SCHEMA_VERSION);
    let text = std::fs::read_to_string(dir().join(newest)).unwrap();
    let p = cantus_project::parse(&text, true).unwrap().file.project;
    let want = Look::new(Body::Upright, Finish::Walnut);
    assert!(chordas(&p).iter().all(|c| chorda_look::of(c) == want));
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("again.cantus");
    cantus_project::save(&p, &path, "test").unwrap();
    let again = cantus_project::load(&path, true).unwrap().project;
    let c = chordas(&again);
    assert!(!c.is_empty());
    for c in c {
        assert_eq!(chorda_look::of(c), want);
    }
}

/// A look already written (a hand edit of a v57 file) is never touched by the migration,
/// and a Chorda's other text (its modulation routes) stays as it was.
#[test]
fn a_look_already_there_is_kept() {
    let text = std::fs::read_to_string(dir().join("v57.cantus")).unwrap();
    let mut json: Value = serde_json::from_str(&text).unwrap();
    let mut marked = 0;
    for t in json["project"]["tracks"].as_array_mut().unwrap() {
        if t["instrument"]["kind"] == "Chorda" {
            let inst = t["instrument"].as_object_mut().unwrap();
            let text = inst
                .entry("text")
                .or_insert_with(|| Value::Object(Default::default()));
            text["look.finish"] = Value::from("rosewood");
            text["mod1.src"] = Value::from("vel");
            marked += 1;
        }
    }
    assert!(marked > 0);
    let r = cantus_project::parse(&serde_json::to_string(&json).unwrap(), true).unwrap();
    assert_eq!(r.migrated_from, Some(57));
    for c in chordas(&r.file.project) {
        assert_eq!(c.text(FINISH_KEY), Some("rosewood"));
        assert_eq!(c.text(BODY_KEY), Some("concert"));
        assert_eq!(c.text("mod1.src"), Some("vel"));
    }
}

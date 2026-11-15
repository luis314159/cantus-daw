//! Schema 65 (round 40, ADR 0122): the cast's default character became Compás, the Slate
//! pictogram. A dancer saved from schema 59 to 64 that named no character showed Fuga the
//! ninja; loading it writes Fuga in, so an old song keeps its dancer. A character already
//! named is kept, a dancer saved before 59 without one keeps the songbird (schema 59), and a
//! dancer saved at 65 without one stays without (it shows the default of today).

use cantus_core::inst::dancer;
use cantus_core::InstrumentKind;
use serde_json::Value;
use std::path::{Path, PathBuf};

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures")
}

/// A fixture's JSON with its dancer's text changed by `edit`, saved as `version`.
fn with(
    fixture: &str,
    edit: impl Fn(&mut serde_json::Map<String, Value>),
    version: u32,
) -> PathBuf {
    let text = std::fs::read_to_string(fixtures().join(fixture)).unwrap();
    let mut v: Value = serde_json::from_str(&text).unwrap();
    v["schema_version"] = Value::from(version);
    let tracks = v["project"]["tracks"].as_array_mut().unwrap();
    let dancer = tracks
        .iter_mut()
        .find(|t| t["instrument"]["kind"] == "Dancer")
        .expect("the fixture's dancer");
    let inst = dancer["instrument"].as_object_mut().unwrap();
    edit(inst);
    let dir = std::env::temp_dir().join(format!(
        "cantus-schema65-{}-{fixture}-{version}-{}",
        std::process::id(),
        inst.get("text")
            .map_or("none".to_string(), |t| t.to_string().len().to_string())
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("song.cantus");
    std::fs::write(&path, serde_json::to_string(&v).unwrap()).unwrap();
    path
}

fn character(path: &Path) -> Option<String> {
    let p = cantus_project::load(path, false).unwrap().project;
    let t = p
        .tracks
        .iter()
        .find(|t| {
            t.instrument
                .native()
                .is_some_and(|n| n.kind == InstrumentKind::Dancer)
        })
        .expect("a dancer");
    t.instrument
        .native()
        .unwrap()
        .text(dancer::CHARACTER_KEY)
        .map(str::to_string)
}

fn no_text(inst: &mut serde_json::Map<String, Value>) {
    inst.remove("text");
}

#[test]
fn a_dancer_saved_from_59_to_64_that_named_none_keeps_fuga() {
    for version in [59, 61, 64] {
        let path = with("v61.cantus", no_text, version);
        assert_eq!(
            character(&path).as_deref(),
            Some("builtin:ninja"),
            "saved at {version}"
        );
    }
    assert_eq!(dancer::DEFAULT_CHARACTER, "builtin:compas");
}

#[test]
fn a_dancer_saved_before_59_that_named_none_keeps_the_songbird() {
    let path = with("v57.cantus", no_text, 57);
    assert_eq!(character(&path).as_deref(), Some("builtin:songbird"));
}

#[test]
fn a_named_character_is_kept() {
    let path = with(
        "v61.cantus",
        |inst| {
            inst.insert(
                "text".into(),
                serde_json::json!({ "character": "builtin:mecha" }),
            );
        },
        61,
    );
    assert_eq!(character(&path).as_deref(), Some("builtin:mecha"));
}

#[test]
fn a_new_dancer_without_a_character_shows_the_default_of_today() {
    let path = with("v61.cantus", no_text, 65);
    assert_eq!(character(&path), None);
}

#[test]
fn the_newest_fixture_keeps_its_dancer() {
    let newest = format!("v{}.cantus", cantus_project::SCHEMA_VERSION);
    const { assert!(cantus_project::SCHEMA_VERSION >= 65) }; // r40:p3 — 66–68 followed
    assert_eq!(
        character(&fixtures().join(newest)).as_deref(),
        Some("builtin:songbird")
    );
}

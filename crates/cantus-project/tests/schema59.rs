//! Schema 59 (round 37, ADR 0105): the cast's default character became Fuga the ninja. A
//! dancer saved before that named no character showed the songbird; loading it writes the
//! songbird in, so an old song keeps its dancer. A character already named is kept, and a
//! dancer saved at 59 without one stays without (it shows the default of today).

use cantus_core::inst::dancer;
use cantus_core::InstrumentKind;
use serde_json::Value;
use std::path::{Path, PathBuf};

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures")
}

/// The v57 fixture's JSON with its dancer's text changed by `edit`, saved as `version`.
fn v57_with(edit: impl Fn(&mut serde_json::Map<String, Value>), version: u32) -> PathBuf {
    let text = std::fs::read_to_string(fixtures().join("v57.cantus")).unwrap();
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
        "cantus-schema59-{}-{version}-{}",
        std::process::id(),
        inst.contains_key("text")
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

#[test]
fn an_old_dancer_that_named_no_character_keeps_the_songbird() {
    let path = v57_with(
        |inst| {
            inst.remove("text");
        },
        57,
    );
    assert_eq!(character(&path).as_deref(), Some("builtin:songbird"));
    assert_ne!(dancer::DEFAULT_CHARACTER, "builtin:songbird");
}

#[test]
fn an_old_dancer_keeps_the_character_it_named() {
    let path = v57_with(
        |inst| {
            inst.insert(
                "text".into(),
                serde_json::json!({ "character": "builtin:cat" }),
            );
        },
        57,
    );
    assert_eq!(character(&path).as_deref(), Some("builtin:cat"));
}

#[test]
fn a_new_dancer_without_a_character_shows_the_default_of_today() {
    let path = v57_with(
        |inst| {
            inst.remove("text");
        },
        59,
    );
    assert_eq!(character(&path), None);
}

#[test]
fn the_newest_fixture_keeps_its_dancer() {
    let newest = format!("v{}.cantus", cantus_project::SCHEMA_VERSION);
    assert_eq!(
        character(&fixtures().join(newest)).as_deref(),
        Some("builtin:songbird")
    );
}

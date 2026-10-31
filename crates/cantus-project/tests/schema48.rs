//! Schema 48 (round 34, ADR 0085): a file's provenance saved beside the pool, loaded back,
//! and every older file loading with none; a card for a file not in the pool (a hand edit)
//! dropped at load.

use cantus_core::Provenance;
use std::path::Path;

fn fixtures() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures")
}

#[test]
fn the_v48_fixture_carries_its_card_and_v47_has_none() {
    let p = cantus_project::load(&fixtures().join("v48.cantus"), false)
        .unwrap()
        .project;
    let (id, card) = p.provenance.iter().next().expect("a card");
    assert!(p.audio_source(*id).is_some());
    assert_eq!(card.model, "htdemucs");
    assert_eq!(card.part, "vocals");
    let old = cantus_project::load(&fixtures().join("v47.cantus"), false).unwrap();
    assert!(old.project.provenance.is_empty());
}

#[test]
fn a_card_for_no_file_is_dropped_and_a_saved_card_round_trips() {
    let dir = tempfile::tempdir().unwrap();
    let mut p = cantus_project::load(&fixtures().join("v48.cantus"), false)
        .unwrap()
        .project;
    let card = Provenance {
        kind: "separated".into(),
        model: "htdemucs".into(),
        model_name: "HT-Demucs".into(),
        model_version: 1,
        licence: "MIT".into(),
        date: "2026-10-30".into(),
        from: "Gone".into(),
        part: String::new(),
    };
    p.provenance
        .insert(cantus_core::AudioSourceId(987_654), card);
    let path = dir.path().join("p.cantus");
    cantus_project::save(&p, &path, "test").unwrap();
    let back = cantus_project::load(&path, true).unwrap().project;
    assert_eq!(back.provenance.len(), p.provenance.len() - 1);
    assert!(!back
        .provenance
        .contains_key(&cantus_core::AudioSourceId(987_654)));
}

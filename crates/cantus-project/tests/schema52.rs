//! Schema 52 (round 36, ADR 0096 §6): the audio
//! editor's markers, regions and loop points saved beside the pool, loaded back, every older
//! file loading with none, and marks of no file or out of range dropped at load.

use cantus_core::{SourceMarker, SourceMarks};
use std::path::Path;

fn fixtures() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures")
}

#[test]
fn the_v52_fixture_carries_marks_and_v51_has_none() {
    let p = cantus_project::load(&fixtures().join("v52.cantus"), false)
        .unwrap()
        .project;
    let (id, m) = p.source_marks.iter().next().expect("marks");
    let frames = p.audio_source(*id).expect("their file").frames;
    assert_eq!(m.markers.len(), 2);
    assert_eq!(m.markers[0].name, "Verse");
    assert_eq!(m.regions[0].name, "Take 2");
    assert!(m.loop_points.is_some());
    assert!(m.is_valid(frames));
    let old = cantus_project::load(&fixtures().join("v51.cantus"), false).unwrap();
    assert!(old.project.source_marks.is_empty());
}

#[test]
fn marks_round_trip_and_bad_ones_are_dropped_at_load() {
    let dir = tempfile::tempdir().unwrap();
    let mut p = cantus_project::load(&fixtures().join("v52.cantus"), false)
        .unwrap()
        .project;
    let id = p.audio_pool[0].id;
    let frames = p.audio_pool[0].frames;
    // For a file not in the pool, and out of a file's range: a hand edit, dropped.
    p.source_marks.insert(
        cantus_core::AudioSourceId(9_999_999),
        SourceMarks {
            loop_points: Some((0, 10)),
            ..SourceMarks::default()
        },
    );
    if let Some(other) = p.audio_pool.get(1).map(|s| s.id) {
        p.source_marks.insert(
            other,
            SourceMarks {
                markers: vec![SourceMarker {
                    frame: u64::MAX,
                    name: String::new(),
                }],
                ..SourceMarks::default()
            },
        );
    }
    let path = dir.path().join("marks.cantus");
    cantus_project::save(&p, &path, "test").unwrap();
    let back = cantus_project::load(&path, false).unwrap().project;
    assert_eq!(back.source_marks.len(), 1);
    assert_eq!(back.source_marks[&id], p.source_marks[&id]);
    assert!(back.source_marks[&id].is_valid(frames));
}

//! Schema 62 (round 40, ADR 0118): a track's record tap — *Before FX* (the default, every
//! older track) or *After FX*. Every older fixture reads with every track before its inserts;
//! the v62 fixture's second track records after them, and saving keeps it. A track before its
//! inserts writes nothing (the file of a song that never chose is the file it was).

use cantus_core::RecordTap;
use std::path::{Path, PathBuf};

fn fixture(v: u32) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join(format!("v{v}.cantus"))
}

#[test]
fn every_older_fixture_records_before_its_inserts() {
    for v in 1..62 {
        let path = fixture(v);
        if !path.exists() {
            continue;
        }
        let p = cantus_project::load(&path, false)
            .unwrap_or_else(|e| panic!("v{v}: {e:?}"))
            .project;
        assert!(
            p.tracks.iter().all(|t| t.record_tap == RecordTap::BeforeFx),
            "v{v}"
        );
    }
}

#[test]
fn the_v62_fixture_records_its_second_track_after_its_inserts_and_keeps_it() {
    let p = cantus_project::load(&fixture(62), false).unwrap().project;
    assert_eq!(p.tracks[1].record_tap, RecordTap::AfterFx);
    assert!(p
        .tracks
        .iter()
        .enumerate()
        .all(|(i, t)| i == 1 || t.record_tap.is_before()));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("again.cantus");
    cantus_project::save(&p, &path, "test").unwrap();
    let back = cantus_project::load(&path, false).unwrap().project;
    assert_eq!(back, p);
    let text = std::fs::read_to_string(&path).unwrap();
    assert_eq!(
        text.matches("record_tap").count(),
        1,
        "only the track that chose"
    );
}

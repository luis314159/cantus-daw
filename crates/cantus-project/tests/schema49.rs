//! Schema 49 (round 35, ADR 0087): the dancer — a native instrument that makes no sound,
//! its numbers, its character and where it shows — saved, loaded back, and automation of
//! its `move` kept; every older file loading with none.

use cantus_core::inst::dancer::{self, Move};
use cantus_core::{AutoTarget, InstrumentKind, TrackIcon};
use std::path::Path;

fn fixtures() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures")
}

#[test]
fn the_v49_fixture_carries_a_dancer_with_its_move_automated() {
    let p = cantus_project::load(&fixtures().join("v49.cantus"), false)
        .unwrap()
        .project;
    let t = p
        .tracks
        .iter()
        .find(|t| {
            t.instrument
                .native()
                .is_some_and(|n| n.kind == InstrumentKind::Dancer)
        })
        .expect("a dancer");
    let n = t.instrument.native().unwrap();
    assert_eq!(n.get("energy"), 0.9);
    assert_eq!(n.get("hue"), -40.0);
    assert_eq!(n.get("view.mode"), 1.0);
    assert_eq!(
        n.get("speed"),
        2.0,
        "a default is not stored and reads back"
    );
    assert_eq!(n.text(dancer::CHARACTER_KEY), Some("builtin:songbird"));
    assert_eq!(t.icon, Some(TrackIcon::Dance));
    let auto = t
        .clips
        .iter()
        .find_map(|c| c.automation.as_ref())
        .expect("automation");
    assert_eq!(
        auto.target,
        AutoTarget::Instrument {
            track: t.id,
            key: "move".into()
        }
    );
    let v = auto.points[0].value * 16.0;
    assert_eq!(dancer::move_of(v), Some(Move::Clap));
    let note = t
        .clips
        .iter()
        .flat_map(|c| &c.notes)
        .next()
        .expect("a note");
    assert_eq!(Move::from_key(note.pitch), Some(Move::Jump));
    // The last track is still the one the freeze block froze.
    assert!(p.tracks.last().unwrap().frozen.is_some());
}

#[test]
fn no_older_file_has_a_dancer() {
    for v in 35..49 {
        let path = fixtures().join(format!("v{v}.cantus"));
        if !path.exists() {
            continue;
        }
        let p = cantus_project::load(&path, false).unwrap().project;
        assert!(
            p.tracks.iter().all(|t| t
                .instrument
                .native()
                .is_none_or(|n| n.kind != InstrumentKind::Dancer)),
            "v{v}"
        );
    }
}

#[test]
fn a_dancer_round_trips() {
    let dir = tempfile::tempdir().unwrap();
    let p = cantus_project::load(&fixtures().join("v49.cantus"), false)
        .unwrap()
        .project;
    let path = dir.path().join("p.cantus");
    cantus_project::save(&p, &path, "test").unwrap();
    let back = cantus_project::load(&path, true).unwrap().project;
    assert_eq!(back.tracks, p.tracks);
}

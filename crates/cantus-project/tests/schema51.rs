//! Schema 51 (round 36, ADR 0094): external units — an instrument and an effect with their
//! routing, trim and measured latency — saved, loaded back and kept; every older file loads
//! with none.

use cantus_core::external_unit::{Pair, UnitKind};
use std::path::Path;

fn fixtures() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures")
}

#[test]
fn the_v51_fixture_carries_its_units() {
    let p = cantus_project::load(&fixtures().join("v51.cantus"), false)
        .unwrap()
        .project;
    assert_eq!(p.external_units.len(), 2);
    let inst = p
        .external_units
        .iter()
        .find(|u| u.kind == UnitKind::Instrument)
        .unwrap();
    assert_eq!(inst.name, "Bass synth");
    assert_eq!(inst.midi.as_ref().unwrap().port, "Hardware Synth");
    assert_eq!(inst.midi.as_ref().unwrap().channel, 2);
    assert_eq!(inst.ret, Pair::stereo(4));
    assert_eq!(inst.latency_frames(48_000.0), 612 + 72);
    assert_eq!(inst.device, "Fixture Interface");
    assert!(p.track(inst.track).is_some());
    let fx = p
        .external_units
        .iter()
        .find(|u| u.kind == UnitKind::Effect)
        .unwrap();
    assert_eq!(fx.send, Some(Pair::stereo(2)));
    assert_eq!(fx.return_db, -3.0);
    assert!(!fx.is_measured());
}

#[test]
fn units_round_trip_and_older_fixtures_have_none() {
    let dir = tempfile::tempdir().unwrap();
    let p = cantus_project::load(&fixtures().join("v51.cantus"), false)
        .unwrap()
        .project;
    let path = dir.path().join("p.cantus");
    cantus_project::save(&p, &path, "test").unwrap();
    let back = cantus_project::load(&path, true).unwrap().project;
    assert_eq!(back.external_units, p.external_units);
    for v in ["v49.cantus", "v48.cantus", "v36.cantus", "v20.cantus"] {
        let f = fixtures().join(v);
        if f.exists() {
            let old = cantus_project::load(&f, false).unwrap().project;
            assert!(old.external_units.is_empty(), "{v}");
        }
    }
}

/// Every fixture is a state the program can be in: its frozen keys are its content keys, and
/// no frozen region holds outboard gear (a freeze of one is refused, `CannotFreeze::External`).
#[test]
fn every_fixture_is_consistent_and_no_frozen_region_holds_outboard_gear() {
    use cantus_core::freeze;
    let mut seen = 0;
    for entry in std::fs::read_dir(fixtures()).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("cantus") {
            continue;
        }
        let p = cantus_project::load(&path, false).unwrap().project;
        seen += 1;
        assert!(freeze::consistent(&p).is_ok(), "{}", path.display());
        for target in p.frozen_targets() {
            let r = freeze::region(&p, target);
            assert!(
                !p.external_units
                    .iter()
                    .any(|u| u.enabled && r.tracks.contains(&u.track)),
                "{}: {target:?} holds outboard gear",
                path.display()
            );
        }
    }
    assert!(seen > 40, "the fixtures were read: {seen}");
}

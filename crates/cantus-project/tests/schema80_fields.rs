//! Schema 80's two switches outside the tuning itself (round 42, ADR 0155 and 0156; the
//! mid-round review's reach 11): an external instrument's *MTS* (its track's tuning sent as
//! MIDI Tuning Standard SysEx) and a hosted instrument's *Tuning by* (MTS-ESP or per-note
//! tuning). Each saved and loaded back as it was; at their defaults neither is written, so
//! the file of a song that never chose is the file it was. r42 fix-engine

use cantus_core::external::{ExternalPlugin, PluginFormat, TuningBy};
use cantus_core::external_unit::{ExternalUnit, ExternalUnitId, MidiRoute, Pair};
use cantus_core::*;
use std::path::Path;

fn song(mts: bool, by: TuningBy) -> Project {
    let mut p = cantus_project::load(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/v80.cantus"),
        false,
    )
    .unwrap()
    .project;
    let mut synth = p.make_track("Hosted");
    let mut plugin = ExternalPlugin::new(PluginFormat::Clap, "org.example.synth", "Synth");
    plugin.tuning_by = by;
    synth.instrument = InstrumentSpec::External(plugin);
    let gear = p.make_track("Gear");
    let mut unit = ExternalUnit::instrument(
        ExternalUnitId(7),
        gear.id,
        MidiRoute {
            port: "Synth".into(),
            ordinal: 0,
            channel: 0,
        },
        Pair::stereo(0),
    );
    unit.mts = mts;
    p.tracks.push(synth);
    p.tracks.push(gear);
    p.external_units.push(unit);
    p
}

fn round_trip(p: &Project) -> (Project, String) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("again.cantus");
    cantus_project::save(p, &path, "test").unwrap();
    let back = cantus_project::load(&path, false).unwrap().project;
    (back, std::fs::read_to_string(&path).unwrap())
}

#[test]
fn mts_and_tuning_by_are_saved_and_read_back() {
    let p = song(true, TuningBy::NoteExpression);
    let (back, text) = round_trip(&p);
    assert_eq!(back, p);
    assert!(back.external_units[0].mts);
    let by = back.tracks.iter().find_map(|t| t.instrument.external());
    assert_eq!(by.map(|x| x.tuning_by), Some(TuningBy::NoteExpression));
    assert!(text.contains("\"mts\""), "the switch written");
    assert!(text.contains("\"tuning_by\""));
}

#[test]
fn at_their_defaults_neither_is_written() {
    let p = song(false, TuningBy::MtsEsp);
    let (back, text) = round_trip(&p);
    assert_eq!(back, p);
    assert!(!text.contains("\"mts\""), "off: not written");
    assert!(!text.contains("\"tuning_by\""), "MTS-ESP: not written");
}

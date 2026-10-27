//! Schema 43 (round 31, ADR 0069): note effects, the song's key, the groove pool and a
//! clip's groove come back from the file whole; an older file has none of them and reads
//! as it always did.

use cantus_core::groove::{GrooveSource, SwingGrid};
use cantus_core::notefx::{ArpMode, ChordShape, NoteFxKind, ScaleMode};
use cantus_core::{Key, ScaleKind};

fn fixture(name: &str) -> cantus_core::Project {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures");
    let text = std::fs::read_to_string(dir.join(name)).unwrap();
    cantus_project::parse(&text, true).unwrap().file.project
}

#[test]
fn the_v43_fixture_keeps_note_effects_key_and_grooves() {
    let p = fixture("v43.cantus");
    assert_eq!(p.key, Some(Key::new(2, ScaleKind::Minor)));
    let chain = &p.tracks[0].note_fx;
    assert_eq!(chain.len(), 3);
    match &chain[0].kind {
        NoteFxKind::Chord(c) => {
            assert_eq!(c.shape, ChordShape::ScaleSeventh);
            assert_eq!(c.inversion, 1);
            assert!(!c.key.own);
        }
        k => panic!("{k:?}"),
    }
    match &chain[1].kind {
        NoteFxKind::Arp(a) => {
            assert_eq!(a.mode, ArpMode::UpDown);
            assert_eq!((a.octaves, a.rate, a.gate, a.swing), (2, 11, 0.6, 0.58));
        }
        k => panic!("{k:?}"),
    }
    match &chain[2].kind {
        NoteFxKind::Scale(s) => assert_eq!(s.mode, ScaleMode::Drop),
        k => panic!("{k:?}"),
    }
    assert!(chain[0].enabled && chain[1].enabled && !chain[2].enabled);
    assert_eq!(p.grooves.len(), 1);
    let g = &p.grooves[0];
    assert!(g.is_valid());
    assert_eq!(g.name, "Fixture feel");
    assert_eq!(g.points[1].shift, 48.0);
    let first = p.tracks[0].clips[0]
        .groove
        .expect("the first clip's groove");
    assert_eq!(first.source, GrooveSource::Pool { id: g.id });
    assert_eq!(
        (first.timing, first.velocity, first.length),
        (0.8, 0.5, 0.0)
    );
    let second = &p.tracks[0].clips[1];
    assert_eq!(
        second.groove.map(|g| g.source),
        Some(GrooveSource::Swing {
            grid: SwingGrid::Sixteenths,
            percent: 62
        })
    );
    assert!(second.skip_note_fx);
    // Saved again, it is the same project.
    let text = cantus_project::ProjectFile::wrap(p.clone(), "again")
        .unwrap()
        .to_json()
        .unwrap();
    let back = cantus_project::parse(&text, true).unwrap().file.project;
    assert_eq!(back, p);
}

#[test]
fn an_older_file_has_no_note_effects_key_or_groove() {
    let p = fixture("v41.cantus");
    assert!(p.key.is_none() && p.grooves.is_empty());
    for t in &p.tracks {
        assert!(t.note_fx.is_empty());
        assert!(t
            .clips
            .iter()
            .all(|c| c.groove.is_none() && !c.skip_note_fx));
    }
}

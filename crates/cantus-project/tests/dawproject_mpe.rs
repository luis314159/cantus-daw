//! DAWproject's per-note expressions (round 33, ADR 0080): a note's bend, pressure and
//! timbre go out as `transpose`, `pressure` and `timbre` timelines inside the note, valid
//! against the official schema, and come back as the same curves, nothing reported lost.

mod support;

use cantus_core::model::note_expr::{ExprKind, ExprPoint, NoteExpr};
use cantus_core::*;
use cantus_project::dawproject;
use support::xsd::Schema;

fn project() -> Project {
    let mut p = Project::new("Expressive");
    let mut t = p.make_track("Lead");
    let mut c = p.make_clip("c", Ticks::ZERO, Ticks::beats(4));
    let mut n = p.make_note(Ticks::beats(1), Ticks::beats(2), 62, 100);
    n.expr = NoteExpr {
        bend: vec![
            ExprPoint(0, ExprKind::Bend.store(-2.0)),
            ExprPoint(480, 0),
            ExprPoint(1440, ExprKind::Bend.store(0.5)),
        ],
        pressure: vec![
            ExprPoint(0, ExprKind::Pressure.store(0.25)),
            ExprPoint(960, ExprKind::Pressure.store(1.0)),
        ],
        timbre: vec![ExprPoint(0, ExprKind::Timbre.store(0.5))],
    }
    .boxed();
    c.notes.push(n);
    c.notes
        .push(p.make_note(Ticks::ZERO, Ticks::beats(1), 60, 90));
    t.clips.push(c);
    p.tracks.push(t);
    p
}

#[test]
fn note_expressions_travel_through_dawproject() {
    let p = project();
    let parts = dawproject::write::parts(&p, "0.1.0");
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/dawproject/");
    let xsd = Schema::parse(&std::fs::read_to_string(format!("{dir}Project.xsd")).unwrap());
    let errors = xsd.validate(&parts.project_xml);
    assert!(errors.is_empty(), "project.xml:\n{}", errors.join("\n"));
    assert!(parts.project_xml.contains("expression=\"transpose\""));
    let back = dawproject::import(&dawproject::export(&p, "0.1.0").bytes).unwrap();
    assert!(
        back.report.is_lossless(),
        "{:?}",
        back.report.dropped_items().collect::<Vec<_>>()
    );
    let notes: Vec<&Note> = back.project.tracks[0]
        .clips
        .iter()
        .flat_map(|c| c.notes.iter())
        .collect();
    let with: Vec<_> = notes.iter().filter(|n| n.expr.is_some()).collect();
    assert_eq!(with.len(), 1);
    let want = p.tracks[0].clips[0].notes[0].expr.as_deref().unwrap();
    let got = with[0].expr.as_deref().unwrap();
    for kind in ExprKind::ALL {
        for at in (0..1920).step_by(60) {
            let (a, b) = (
                want.value_at(kind, at).unwrap(),
                got.value_at(kind, at).unwrap(),
            );
            assert!((a - b).abs() < 1e-6, "{kind:?} at {at}: {a} vs {b}");
        }
    }
    assert!(notes.iter().any(|n| n.pitch == 60 && n.expr.is_none()));
}

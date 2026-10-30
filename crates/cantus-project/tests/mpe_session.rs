//! A v47 file whose session clip (ADR 0076) holds a note with a curve out of order loads
//! with the curve repaired (ADR 0080), as an arrangement clip's is.

use cantus_core::model::note_expr::{ExprPoint, NoteExpr};
use cantus_core::session::{Scene, SceneId, SessionClip, Slot};
use cantus_core::*;
use cantus_project::format::{parse, ProjectFile};

#[test]
fn a_session_clips_malformed_curve_loads_repaired() {
    let mut p = Project::new("slots");
    let mut tr = p.make_track("t");
    let scene = SceneId(p.alloc_id());
    p.session.scenes.push(Scene::new(scene, "S"));
    let mut c = p.make_clip("c", Ticks::ZERO, Ticks::beats(4));
    let mut n = p.make_note(Ticks::ZERO, Ticks(960), 60, 100);
    n.expr = Some(Box::new(NoteExpr {
        bend: vec![ExprPoint(9, 0), ExprPoint(1, 5)],
        ..NoteExpr::default()
    }));
    c.notes.push(n);
    let cid = c.id;
    tr.slots.push(Slot {
        scene,
        clip: Some(SessionClip::new(c)),
        stop: true,
    });
    let t = tr.id;
    p.tracks.push(tr);
    let text = ProjectFile::wrap(p, "test").unwrap().to_json().unwrap();
    let back = parse(&text, false).expect("opens").file.project;
    let e = back.track(t).unwrap().clip(cid).unwrap().notes[0]
        .expr
        .as_deref()
        .unwrap()
        .clone();
    assert!(e.is_valid(), "{e:?}");
    assert_eq!(e.bend, vec![ExprPoint(1, 5), ExprPoint(9, 0)]);
}

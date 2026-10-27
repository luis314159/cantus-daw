//! DAWproject export says what of round 31 it leaves out (ADR 0069, ADR 0021's rule that a
//! loss is reported): a track's note effects, clips' grooves, the song's key.

use cantus_core::groove::{ClipGroove, GrooveSource, SwingGrid};
use cantus_core::notefx::{NoteEffect, NoteFxId, NoteFxType};
use cantus_core::*;
use cantus_project::dawproject;

#[test]
fn note_effects_grooves_and_the_key_are_reported_left_out() {
    let mut p = Project::new("x");
    let mut t = p.make_track("Keys");
    let mut c = p.make_clip("A", Ticks::ZERO, Ticks::beats(4));
    c.notes
        .push(p.make_note(Ticks::ZERO, Ticks::QUARTER, 60, 90));
    c.groove = Some(ClipGroove::new(GrooveSource::Swing {
        grid: SwingGrid::Sixteenths,
        percent: 62,
    }));
    t.clips.push(c);
    t.note_fx = vec![NoteEffect::new(NoteFxId(p.alloc_id()), NoteFxType::Arp)];
    p.tracks.push(t);
    let plain = dawproject::export(&p.clone(), "0");
    assert!(plain.report.has("interop.report.x_note_fx"));
    assert_eq!(
        plain.report.get("interop.report.x_grooves").map(|_| ()),
        Some(())
    );
    assert!(!plain.report.has("interop.report.x_key"));
    p.key = Some(Key::new(2, ScaleKind::Minor));
    assert!(dawproject::export(&p, "0")
        .report
        .has("interop.report.x_key"));
    // Nothing of it: nothing said.
    let mut q = p.clone();
    q.key = None;
    q.tracks[0].note_fx.clear();
    q.tracks[0].clips[0].groove = None;
    let r = dawproject::export(&q, "0").report;
    assert!(!r.has("interop.report.x_note_fx") && !r.has("interop.report.x_grooves"));
}

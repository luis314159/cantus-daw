//! The curves' own rules (ADR 0080): values, splits, joins, stretches, the lane's edits,
//! the MIDI 1.0/2.0 translation, the compiled stream, the command's inverse.

use super::midi2::*;
use super::ops::*;
use super::*;
use crate::model::{Clip, Note, NoteId, Project};
use crate::time::Ticks;

fn st(x: f64) -> i64 {
    ExprKind::Bend.store(x)
}

fn bend(points: &[(i64, f64)]) -> NoteExpr {
    NoteExpr {
        bend: points.iter().map(|&(a, v)| ExprPoint(a, st(v))).collect(),
        ..NoteExpr::default()
    }
}

#[test]
fn values_are_linear_between_points_and_held_outside() {
    let e = bend(&[(100, 0.0), (200, 2.0)]);
    assert_eq!(e.value_at(ExprKind::Bend, 0), Some(0.0));
    assert_eq!(e.value_at(ExprKind::Bend, 150), Some(1.0));
    assert_eq!(e.value_at(ExprKind::Bend, 500), Some(2.0));
    assert_eq!(
        e.value_at(ExprKind::Pressure, 150),
        None,
        "no curve, no value"
    );
    assert_eq!(ExprKind::Pressure.store(1.0), UNIT_MAX);
    assert_eq!(ExprKind::Pressure.store(7.0), UNIT_MAX, "clamped");
}

#[test]
fn a_split_meets_without_a_jump_and_loses_nothing() {
    let e = bend(&[(0, 0.0), (100, 1.0), (300, -1.0)]);
    for cut in [0, 50, 100, 200, 299, 300, 400] {
        let (l, r) = split(Some(&e), cut);
        let (l, r) = (l.unwrap(), r.unwrap());
        for t in 0..cut {
            assert_eq!(
                l.raw_at(ExprKind::Bend, t),
                e.raw_at(ExprKind::Bend, t),
                "left {cut}@{t}"
            );
        }
        for t in cut..500 {
            assert_eq!(
                r.raw_at(ExprKind::Bend, t - cut),
                e.raw_at(ExprKind::Bend, t),
                "right {cut}@{t}"
            );
        }
        assert!(l.is_valid() && r.is_valid());
        // Joined back, the note sounds as it did.
        let j = join(Some(&l), Some(&r), cut).unwrap();
        for t in 0..500 {
            assert_eq!(
                j.raw_at(ExprKind::Bend, t),
                e.raw_at(ExprKind::Bend, t),
                "join {cut}@{t}"
            );
        }
    }
    assert_eq!(split(None, 10), (None, None));
}

#[test]
fn trims_stretches_and_reverses_keep_what_is_heard_where_it_was() {
    let e = bend(&[(0, 0.0), (100, 1.0), (200, 0.0)]);
    let t = trim_start(Some(&e), 50).unwrap();
    assert_eq!(t.raw_at(ExprKind::Bend, 0), e.raw_at(ExprKind::Bend, 50));
    assert_eq!(t.raw_at(ExprKind::Bend, 50), e.raw_at(ExprKind::Bend, 100));
    let out = trim_start(Some(&e), -40).unwrap();
    assert_eq!(
        out.raw_at(ExprKind::Bend, 140),
        e.raw_at(ExprKind::Bend, 100)
    );
    let s = stretch(Some(&e), 2, 1).unwrap();
    assert_eq!(s.raw_at(ExprKind::Bend, 200), e.raw_at(ExprKind::Bend, 100));
    let r = reverse(Some(&bend(&[(0, 0.0), (100, 2.0)])), 100).unwrap();
    assert_eq!(r.value_at(ExprKind::Bend, 0), Some(2.0));
    assert_eq!(r.value_at(ExprKind::Bend, 100), Some(0.0));
}

#[test]
fn the_lane_edits_do_what_they_say() {
    let c: Vec<ExprPoint> = (0..=100)
        .map(|i| ExprPoint(i * 10, st((i % 2) as f64)))
        .collect();
    // Smoothing takes the zig-zag down; the ends stay.
    let s = smooth(&c, ExprKind::Bend, 4);
    assert_eq!(s.first(), c.first());
    assert_eq!(s.last(), c.last());
    let swing = |v: &[ExprPoint]| {
        v[40..60].iter().map(|p| p.1).max().unwrap() - v[40..60].iter().map(|p| p.1).min().unwrap()
    };
    assert!(swing(&s) < swing(&c) / 4);
    // Thinning a straight line keeps its ends only; within tolerance everywhere.
    let line: Vec<ExprPoint> = (0..=50)
        .map(|i| ExprPoint(i * 10, st(i as f64 / 10.0)))
        .collect();
    let th = thin(&line, default_tolerance(ExprKind::Bend));
    assert_eq!(th.len(), 2);
    for p in &line {
        assert!((raw_at(&th, p.0).unwrap() - p.1).abs() <= default_tolerance(ExprKind::Bend));
    }
    // Scale about the rest: timbre about its middle.
    let tb = vec![ExprPoint(0, UNIT_MAX)];
    assert_eq!(
        scale(&tb, ExprKind::Timbre, 0.0)[0].1,
        ExprKind::Timbre.rest()
    );
    assert_eq!(
        scale(&[ExprPoint(0, st(1.0))], ExprKind::Bend, 2.0)[0].1,
        st(2.0)
    );
    // Draw replaces only its stretch.
    let d = draw(&line, &[ExprPoint(100, 0), ExprPoint(200, 0)]);
    assert_eq!(raw_at(&d, 150), Some(0));
    assert_eq!(raw_at(&d, 300), raw_at(&line, 300));
    assert_eq!(flatten(&line, ExprKind::Bend, 500), vec![ExprPoint(0, 0)]);
    assert_eq!(snap_semitone(st(1.4)), st(1.0));
    assert_eq!(snap_semitone(st(-1.6)), st(-2.0));
}

#[test]
fn normalising_sorts_dedups_clamps_and_starts_at_zero() {
    let e = NoteExpr {
        pressure: vec![
            ExprPoint(50, 3),
            ExprPoint(-10, 1),
            ExprPoint(-30, 1),
            ExprPoint(50, 9),
            ExprPoint(10, UNIT_MAX * 2),
        ],
        ..NoteExpr::default()
    }
    .normalised();
    assert!(e.is_valid());
    assert_eq!(
        e.pressure[0].0, -10,
        "of the points before the start, the last"
    );
    assert_eq!(e.pressure.last().unwrap().1, 9, "the last of one tick wins");
    assert_eq!(e.pressure[1].1, UNIT_MAX);
    assert_eq!(NoteExpr::default().boxed(), None);
}

#[test]
fn midi_1_and_2_translate_exactly() {
    // Min-Center-Max: 0, the centre and the top land on 0, the centre and the top.
    assert_eq!(scale_up(0, 7, 32), 0);
    assert_eq!(scale_up(64, 7, 32), 0x8000_0000);
    assert_eq!(scale_up(127, 7, 32), u32::MAX);
    assert_eq!(scale_up(8192, 14, 32), 0x8000_0000);
    assert_eq!(scale_up(16383, 14, 32), u32::MAX);
    for v in 0..128u8 {
        assert_eq!(unit_to_7(unit_of_7(v)), v);
    }
    // A member's 14-bit bend at 48 semitones round-trips to the bit.
    for raw in [0u16, 1, 4096, 8191, 8192, 8193, 12000, 16383] {
        assert_eq!(bend_to_14(bend_of_14(raw, 48), 48), raw);
    }
    assert_eq!(bend_of_14(16383, 48), st(48.0) - st(48.0) / 8192);
    assert_eq!(bend_of_14(0, 2), st(-2.0));
    // MIDI 2.0's per-note bend and Pitch 7.25.
    assert_eq!(bend_to_32(0, 48), 0x8000_0000);
    assert_eq!(bend_of_32(bend_to_32(st(12.5), 48), 48), st(12.5));
    assert_eq!(pitch_725(60, st(1.0)), 61 << 25);
    assert_eq!(bend_of_725(60, pitch_725(60, st(-0.25))), st(-0.25));
    // Packets encode and decode.
    let msgs = [
        Ump::NoteOn {
            group: 1,
            channel: 3,
            key: 60,
            velocity: 0xABCD,
            pitch: Some(60 << 9),
        },
        Ump::NoteOff {
            group: 0,
            channel: 3,
            key: 60,
            velocity: 7,
        },
        Ump::PolyPressure {
            group: 0,
            channel: 2,
            key: 61,
            value: 0x1234_5678,
        },
        Ump::of_expression(0, 1, 62, ExprKind::Bend, st(2.0)),
        Ump::of_expression(0, 1, 62, ExprKind::Timbre, UNIT_MAX / 3),
        Ump::PerNoteBend {
            group: 0,
            channel: 1,
            key: 62,
            value: 0x9000_0000,
        },
    ];
    for m in msgs {
        assert_eq!(Ump::decode(m.encode()), Some(m));
    }
    assert_eq!(
        Ump::of_expression(0, 1, 62, ExprKind::Bend, st(2.0)).expression(48),
        Some((ExprKind::Bend, st(2.0)))
    );
    assert_eq!(
        Ump::of_expression(0, 1, 62, ExprKind::Pressure, 77).expression(48),
        Some((ExprKind::Pressure, 77))
    );
    // The first word of a note-on, by hand (M2-104 §7.4.1): type 4, group, op 9, channel.
    assert_eq!(msgs[0].encode()[0], 0x4193_3C03);
}

fn project_with(notes: Vec<Note>) -> (Project, crate::model::TrackId, crate::model::ClipId) {
    let mut p = Project::empty("x");
    let mut tr = p.make_track("t");
    let mut c: Clip = p.make_clip("c", Ticks(0), Ticks::beats(8));
    c.notes = notes;
    let (t, cid) = (tr.id, c.id);
    tr.clips.push(c);
    p.tracks.push(tr);
    (p, t, cid)
}

#[test]
fn the_stream_starts_at_the_note_on_and_stops_before_the_note_off() {
    let mut n = Note {
        id: NoteId(9000),
        start: Ticks(960),
        length: Ticks(960),
        pitch: 60,
        velocity: 100,
        muted: false,
        release: 0,
        expr: None,
    };
    n.expr = bend(&[(0, 0.0), (480, 2.0), (2000, 5.0)]).boxed();
    let (p, t, _) = project_with(vec![n]);
    let ev = compile::compile_track(p.track(t).unwrap(), &p.grooves);
    assert_eq!(ev[0].tick, Ticks(960));
    assert_eq!(ev[0].value, 0.0);
    assert!(ev
        .iter()
        .all(|e| e.tick >= Ticks(960) && e.tick < Ticks(1920)));
    assert!(ev.windows(2).all(|w| w[0].tick < w[1].tick));
    // The point at 480 is sent at its own tick, exactly.
    let at = ev.iter().find(|e| e.tick == Ticks(1440)).unwrap();
    assert_eq!(at.value, 2.0);
    // Between the points, the grid's steps.
    assert!(ev.iter().any(|e| e.tick == Ticks(970)));
    // A project with no expression compiles to nothing.
    let (p, t, _) = project_with(vec![Note {
        expr: None,
        ..p.track(t).unwrap().clips[0].notes[0].clone()
    }]);
    assert!(compile::compile_track(p.track(t).unwrap(), &p.grooves).is_empty());
}

#[test]
fn the_command_sets_and_its_inverse_restores_exactly() {
    let n = Note {
        id: NoteId(9000),
        start: Ticks(0),
        length: Ticks(960),
        pitch: 60,
        velocity: 100,
        muted: false,
        release: 0,
        expr: None,
    };
    let (mut p, t, c) = project_with(vec![n]);
    let before = p.clone();
    let cmd = crate::Command::SetNoteExpr {
        track: t,
        clip: c,
        edit: ExprEdit::Draw,
        notes: vec![(NoteId(9000), bend(&[(0, 1.0)]).boxed())],
    };
    assert_eq!(cmd.label_key(), "cmd.mpe.draw");
    let inv = cmd.apply(&mut p).unwrap();
    assert!(p.track(t).unwrap().clips[0].notes[0].expr.is_some());
    inv.apply(&mut p).unwrap();
    assert_eq!(p, before);
    // A missing note or a malformed curve changes nothing.
    let bad = crate::Command::SetNoteExpr {
        track: t,
        clip: c,
        edit: ExprEdit::Set,
        notes: vec![(
            NoteId(9000),
            Some(Box::new(NoteExpr {
                bend: vec![ExprPoint(5, 0), ExprPoint(1, 0)],
                ..NoteExpr::default()
            })),
        )],
    };
    assert!(bad.apply(&mut p).is_err());
    let missing = crate::Command::SetNoteExpr {
        track: t,
        clip: c,
        edit: ExprEdit::Set,
        notes: vec![(NoteId(1), None)],
    };
    assert!(missing.apply(&mut p).is_err());
    assert_eq!(p, before);
}

#[test]
fn a_note_without_expression_serialises_as_before() {
    let n = Note {
        id: NoteId(3),
        start: Ticks(0),
        length: Ticks(10),
        pitch: 60,
        velocity: 100,
        muted: false,
        release: 0,
        expr: None,
    };
    let s = serde_json::to_string(&n).unwrap();
    assert!(!s.contains("expr"), "{s}");
    let with = Note {
        expr: bend(&[(0, 1.0), (5, -1.0)]).boxed(),
        ..n
    };
    let s = serde_json::to_string(&with).unwrap();
    assert!(
        s.contains("\"expr\":{\"bend\":[[0,16777216],[5,-16777216]]}"),
        "{s}"
    );
    let back: Note = serde_json::from_str(&s).unwrap();
    assert_eq!(back, with);
}

/// Review fix 10: a side without a curve is at rest only over its own part; the other's
/// value holds over its whole part (a constant +2 joined to a plain note does not ramp).
#[test]
fn a_join_holds_each_side_over_its_own_part() {
    let a = bend(&[(0, 2.0)]);
    let j = join(Some(&a), None, 480).unwrap();
    assert_eq!(j.raw_at(ExprKind::Bend, 240), Some(st(2.0)));
    assert_eq!(j.raw_at(ExprKind::Bend, 479), Some(st(2.0)));
    assert_eq!(j.raw_at(ExprKind::Bend, 480), Some(0));
    let k = join(None, Some(&a), 480).unwrap();
    assert_eq!(k.raw_at(ExprKind::Bend, 240), Some(0));
    assert_eq!(k.raw_at(ExprKind::Bend, 600), Some(st(2.0)));
}

/// A project whose track holds a session clip (ADR 0076) with one note.
pub(crate) fn slot_project(
    expr: Option<Box<NoteExpr>>,
) -> (Project, crate::model::TrackId, crate::model::ClipId, NoteId) {
    use crate::session::{Scene, SceneId, SessionClip, Slot};
    let mut p = Project::empty("slots");
    let mut tr = p.make_track("t");
    let scene = SceneId(p.alloc_id());
    p.session.scenes.push(Scene::new(scene, "S"));
    let mut c: Clip = p.make_clip("c", Ticks(0), Ticks::beats(4));
    let mut n = p.make_note(Ticks(0), Ticks(960), 60, 100);
    n.expr = expr;
    let id = n.id;
    c.notes.push(n);
    let (t, cid) = (tr.id, c.id);
    tr.slots.push(Slot {
        scene,
        clip: Some(SessionClip::new(c)),
        stop: true,
    });
    p.tracks.push(tr);
    (p, t, cid, id)
}

/// Review (integration): an expression edit on a session clip's note is made — and undone —
/// as on an arrangement clip's (the piano roll draws them through `Track::clip`).
#[test]
fn an_expression_edit_reaches_a_session_clip() {
    let (mut p, t, c, id) = slot_project(bend(&[(0, 1.0)]).boxed());
    let before = p.clone();
    let cmd = cmd::command_for(&p, t, c, &[id], ExprEdit::Clear, |_| None).expect("a change");
    let inv = cmd.apply(&mut p).unwrap();
    assert!(p.track(t).unwrap().clip(c).unwrap().notes[0].expr.is_none());
    inv.apply(&mut p).unwrap();
    assert_eq!(p, before);
}

/// A malformed curve in a session clip is repaired as one in the arrangement is.
#[test]
fn repair_reaches_session_clips() {
    let bad = NoteExpr {
        bend: vec![ExprPoint(9, 0), ExprPoint(1, 5)],
        ..NoteExpr::default()
    };
    let (mut p, t, c, _) = slot_project(Some(Box::new(bad)));
    assert_eq!(repair_project(&mut p), 1);
    assert!(p.track(t).unwrap().clip(c).unwrap().notes[0]
        .expr
        .as_deref()
        .unwrap()
        .is_valid());
}

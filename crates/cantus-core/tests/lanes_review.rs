//! Round 31's reviews (ADR 0068): a time-range cut or copy of a looping, trimmed note clip
//! carries its controller lanes as they were heard — in the piece and in what stays of the
//! clip — with every segment's shape: a held pedal stays held, a curve stays that curve.

use cantus_core::automation::{AutoPoint, Shape};
use cantus_core::clip_lanes::{compile_track, ClipLane, CtlEvent, LaneKind, STEP};
use cantus_core::clipboard;
use cantus_core::*;

/// A track whose one clip starts at bar 2, shows its content from beat 1, loops every four
/// beats and runs ten, with notes and `lanes`.
fn song(lanes: Vec<ClipLane>) -> (CommandBus, TrackId) {
    let mut p = Project::empty("lanes");
    let mut t = p.make_track("Keys");
    let id = t.id;
    let mut c = p.make_clip("loop", Ticks::beats(4), Ticks::beats(10));
    c.offset = Ticks::beats(1);
    c.content_length = Some(Ticks::beats(4));
    for k in 0..6 {
        c.notes
            .push(p.make_note(Ticks::beats(k), Ticks(240), 60 + k as u8, 100));
    }
    c.lanes = lanes;
    t.clips.push(c);
    p.tracks.push(t);
    (CommandBus::new(p), id)
}

fn pt(tick: Ticks, value: f32, shape: Shape, tension: f32) -> AutoPoint {
    AutoPoint {
        tick,
        value,
        tension,
        shape,
    }
}

/// A bend ramping up and down and the mod wheel opening: straight lines.
fn ramps() -> Vec<ClipLane> {
    vec![
        ClipLane::new(
            LaneKind::PitchBend,
            vec![
                AutoPoint::new(Ticks::ZERO, 0.1),
                AutoPoint::new(Ticks::beats(3), 0.9),
                AutoPoint::new(Ticks::beats(5), 0.3),
            ],
        ),
        ClipLane::new(
            LaneKind::Cc(1),
            vec![
                AutoPoint::new(Ticks::beats(1), 0.0),
                AutoPoint::new(Ticks::beats(5), 1.0),
            ],
        ),
    ]
}

/// A sustain pedal drawn as holds: down, up, down, up — the range begins inside an up.
fn pedal() -> Vec<ClipLane> {
    vec![ClipLane::new(
        LaneKind::Cc(64),
        vec![
            pt(Ticks::ZERO, 1.0, Shape::Hold, 0.0),
            pt(Ticks::beats(2), 0.0, Shape::Hold, 0.0),
            pt(Ticks::beats(3) + Ticks(480), 1.0, Shape::Hold, 0.0),
            pt(Ticks::beats(5), 0.0, Shape::Hold, 0.0),
        ],
    )]
}

/// Every other shape: a bent curve, an S, stairs into a pulse, a wave.
fn curves() -> Vec<ClipLane> {
    vec![
        ClipLane::new(
            LaneKind::PitchBend,
            vec![
                pt(Ticks::ZERO, 0.2, Shape::Curve, 0.7),
                pt(Ticks::beats(2) + Ticks(300), 0.9, Shape::SCurve, 0.3),
                pt(Ticks::beats(3) + Ticks(100), 0.1, Shape::Stairs, 0.5),
                pt(Ticks::beats(4) + Ticks(240), 0.8, Shape::Pulse, 0.6),
                pt(Ticks::beats(5), 0.6, Shape::Curve, 0.0),
            ],
        ),
        ClipLane::new(
            LaneKind::Cc(1),
            vec![
                pt(Ticks::beats(1), 0.0, Shape::Wave, 0.4),
                pt(Ticks::beats(5), 1.0, Shape::Curve, 0.0),
            ],
        ),
    ]
}

/// What a stream holds for `kind` at `t`: its last value at or before (`None` before any).
fn held(ev: &[CtlEvent], kind: LaneKind, t: Ticks) -> Option<f32> {
    ev.iter()
        .rfind(|e| e.kind == kind && e.tick <= t)
        .map(|e| e.value)
}

/// `a` and `b` are heard alike over `[from, to)` on the engine's grid — within `tol` (0:
/// to the bit) — and within a hair between its ticks.
fn heard_alike(a: &[CtlEvent], b: &[CtlEvent], (from, to): (Ticks, Ticks), tol: f32, what: &str) {
    let kinds = cantus_core::clip_lanes::kinds(a);
    let mut t = from;
    while t < to {
        let on_grid = t.0 % STEP == 0;
        for &kind in &kinds {
            let (x, y) = (held(a, kind, t), held(b, kind, t));
            let alike = match (x, y) {
                (Some(x), Some(y)) if on_grid && tol == 0.0 => x == y,
                (Some(x), Some(y)) if on_grid => (x - y).abs() <= tol,
                (Some(x), Some(y)) => (x - y).abs() < 0.05,
                (x, y) => x == y,
            };
            assert!(alike, "{what}: {kind:?} at {t:?}: {x:?} against {y:?}");
        }
        t += Ticks(STEP / 2 + 1);
    }
}

fn ids(next: &mut u64) -> impl FnMut() -> u64 + '_ {
    move || {
        *next += 1;
        *next - 1
    }
}

/// The range begins inside a repeat (no repeat begins there): the piece is written out
/// once, its lanes with it, and the cut leaves the rest playing what it played.
fn crop_plays_as_heard(lanes: Vec<ClipLane>, tol: f32, what: &str) {
    let (mut bus, t) = song(lanes);
    let before = compile_track(bus.project().track(t).unwrap());
    let (from, to) = (Ticks::beats(9) + Ticks(480), Ticks::beats(12) + Ticks(240));
    let mut next = bus.project().next_id;
    let crops = clipboard::time_crops(bus.project(), from, to, &mut ids(&mut next));
    assert_eq!(crops.len(), 1);
    // The piece, alone on a track where it was: what was heard over the range.
    let mut alone = bus.project().track(t).unwrap().clone();
    alone.clips = crops[0].pieces.clone();
    assert_eq!(alone.clips.len(), 1, "a note clip's piece is one clip");
    let piece = format!("{what}: the piece");
    heard_alike(&before, &compile_track(&alone), (from, to), tol, &piece);
    // The cut: what stays on either side plays what it played.
    let cut = crops.into_iter().flat_map(|c| c.cut).collect();
    bus.execute(
        Command::Batch {
            label_key: "cmd.cut".into(),
            commands: cut,
        },
        Origin::User,
    )
    .expect("the cut");
    let after = compile_track(bus.project().track(t).unwrap());
    let left = format!("{what}: before the range");
    heard_alike(&before, &after, (Ticks::ZERO, from), 0.0, &left);
    let right = format!("{what}: after the range");
    heard_alike(&before, &after, (to, Ticks::beats(14)), tol, &right);
}

/// Straight lines: within rounding (a line cut in two is drawn again from its value there).
#[test]
fn a_crop_of_a_looping_clip_carries_its_lanes_as_heard() {
    crop_plays_as_heard(ramps(), 1e-5, "ramps");
}

/// A held pedal stays held: no ramp where the stretch begins inside a hold.
#[test]
fn a_held_pedal_stays_held_through_a_crop() {
    crop_plays_as_heard(pedal(), 0.0, "pedal");
}

/// Curves, S-curves, stairs, pulses and waves cut mid-segment: the engine's samples, to
/// the bit.
#[test]
fn every_shape_survives_a_crop_to_the_bit() {
    crop_plays_as_heard(curves(), 0.0, "curves");
}

//! The controller lanes' own rules (ADR 0068): what a lane compiles to through a clip's
//! window and loop, the rests, the edits' guarantees.

use super::edit::{glue, lanes_from_messages, line, overwrite, paste, pencil, shift, span, steps};
use super::*;
use crate::automation::{AutoPoint, Shape};
use crate::model::{Clip, ClipId, Project, Track};
use crate::time::Ticks;

fn clip(start: i64, length: i64, lanes: Vec<ClipLane>) -> Clip {
    let mut p = Project::empty("t");
    let mut c = p.make_clip("c", Ticks(start), Ticks(length));
    c.id = ClipId(1);
    c.lanes = lanes;
    c
}

fn bend(points: &[(i64, f32)]) -> ClipLane {
    ClipLane::new(
        LaneKind::PitchBend,
        points
            .iter()
            .map(|(t, v)| AutoPoint::new(Ticks(*t), *v))
            .collect(),
    )
}

fn track_of(clips: Vec<Clip>) -> Track {
    let mut p = Project::empty("t");
    let mut t = p.make_track("t");
    t.clips = clips;
    t
}

/// No lanes, no events: nothing for the engine to split at or send.
#[test]
fn a_clip_without_lanes_compiles_to_nothing() {
    let t = track_of(vec![clip(0, 3840, vec![])]);
    assert!(compile_track(&t).is_empty());
    // A lane with no points is none either.
    let t = track_of(vec![clip(
        0,
        3840,
        vec![ClipLane::new(LaneKind::Cc(1), vec![])],
    )]);
    assert!(compile_track(&t).is_empty());
}

/// A flat lane sends its value where the clip begins and its rest where it ends; a ramp is
/// sampled on the timeline's grid; every point is sent at its own tick.
#[test]
fn a_lane_plays_its_curve_between_its_value_and_its_rest() {
    let c = clip(960, 1920, vec![bend(&[(0, 0.5), (480, 1.0)])]);
    let ev = compile_track(&track_of(vec![c]));
    assert_eq!(ev[0].tick, Ticks(960));
    assert_eq!(ev[0].value, 0.5);
    let last = ev.last().unwrap();
    assert_eq!(last.tick, Ticks(2880));
    assert_eq!(last.value, LaneKind::PitchBend.rest());
    // The ramp: every STEP ticks, rising, ending at the point.
    let ramp: Vec<&CtlEvent> = ev
        .iter()
        .filter(|e| e.tick > Ticks(960) && e.tick <= Ticks(1440))
        .collect();
    assert_eq!(ramp.len() as i64, 480 / STEP);
    assert!(ramp.windows(2).all(|w| w[1].value > w[0].value));
    assert_eq!(ramp.last().unwrap().value, 1.0);
    assert!(ramp.iter().all(|e| e.tick.0 % STEP == 0));
}

/// A window that starts inside a bend starts bent; a slipped window plays the curve moved
/// with the notes; a looping clip plays its content again each period.
#[test]
fn a_lane_follows_its_clips_window_and_loop() {
    let lane = bend(&[(0, 0.0), (960, 1.0)]);
    // Trimmed from the left by half a beat: content 480 at the start.
    let mut c = clip(1000, 960, vec![lane.clone()]);
    c.offset = Ticks(480);
    let ev = compile_track(&track_of(vec![c]));
    assert_eq!(ev[0].tick, Ticks(1000));
    assert!((ev[0].value - 0.5).abs() < 1e-6);
    // Looping every beat over three beats: the ramp three times, each from the bottom.
    let mut c = clip(0, 2880, vec![lane]);
    c.content_length = Some(Ticks(960));
    let ev = compile_track(&track_of(vec![c]));
    let starts: Vec<&CtlEvent> = ev.iter().filter(|e| e.value == 0.0).collect();
    assert_eq!(
        starts.iter().map(|e| e.tick.0).collect::<Vec<_>>(),
        vec![0, 960, 1920]
    );
}

/// The halves of a split meet without a rest between them; a muted clip, a take lane and
/// an audio clip send nothing.
#[test]
fn split_halves_meet_and_silent_clips_send_nothing() {
    let lane = bend(&[(0, 0.75)]);
    let a = clip(0, 960, vec![lane.clone()]);
    let mut b = clip(960, 960, vec![lane.clone()]);
    b.offset = Ticks(960);
    b.id = ClipId(2);
    let ev = compile_track(&track_of(vec![a, b]));
    assert!(
        !ev.iter()
            .any(|e| e.tick == Ticks(960) && e.value == LaneKind::PitchBend.rest()),
        "{ev:?}"
    );
    let mut m = clip(0, 960, vec![lane.clone()]);
    m.muted = true;
    let mut take = clip(0, 960, vec![lane]);
    take.lane = 1;
    assert!(compile_track(&track_of(vec![m, take])).is_empty());
}

/// What a command accepts: kinds a lane may drive, points in range and order, one lane per
/// kind.
#[test]
fn lanes_are_validated() {
    assert!(lanes_valid(&[bend(&[(0, 0.5)])]));
    assert!(!lanes_valid(&[ClipLane::new(LaneKind::Cc(120), vec![])]));
    assert!(!lanes_valid(&[bend(&[(0, 1.5)])]));
    assert!(!lanes_valid(&[bend(&[(-1, 0.5)])]));
    assert!(!lanes_valid(&[bend(&[(0, 0.5)]), bend(&[(0, 0.2)])]));
    let mut unsorted = bend(&[(0, 0.5)]);
    unsorted.points.insert(0, AutoPoint::new(Ticks(10), 0.1));
    assert!(!lanes_valid(&[unsorted]));
}

/// MIDI's values both ways: the bend's centre is 8192 exactly, its ends exact; a CC's
/// 0…127 round trips.
#[test]
fn midi_values_round_trip() {
    let b = LaneKind::PitchBend;
    assert_eq!(b.midi(0.5), 8192);
    assert_eq!(b.midi(0.0), 0);
    assert_eq!(b.midi(1.0), 16383);
    for v in [0u16, 1, 8191, 8192, 8193, 16382, 16383] {
        assert_eq!(b.midi(b.pos_of_midi(v)), v);
    }
    let c = LaneKind::Cc(74);
    for v in 0..=127u16 {
        assert_eq!(c.midi(c.pos_of_midi(v)), v);
    }
}

/// Drawing over a stretch changes that stretch only: the old value is heard up to it and
/// comes back after it.
#[test]
fn an_overwrite_keeps_what_lies_outside_it() {
    let lane = bend(&[(0, 0.2), (3840, 0.8)]);
    let before = lane.value_at(959.0).unwrap();
    let after = lane.value_at(1921.0).unwrap();
    let new = [
        AutoPoint::new(Ticks(960), 1.0),
        AutoPoint::new(Ticks(1920), 1.0),
    ];
    let pts = overwrite(&lane.points, Ticks(960), Ticks(1920), &new);
    let out = ClipLane::new(LaneKind::PitchBend, pts);
    assert!((out.value_at(959.0).unwrap() - before).abs() < 1e-3);
    assert_eq!(out.value_at(1500.0).unwrap(), 1.0);
    assert!((out.value_at(1921.0).unwrap() - after).abs() < 1e-3);
    assert_eq!(out.value_at(3840.0).unwrap(), 0.8);
}

/// The line tool: a straight line between its ends; with tension, a bent one that still
/// meets them. The pencil thins a stroke to the bake's tolerance.
#[test]
fn line_and_pencil_write_what_they_draw() {
    let pts = line(&[], Ticks(0), 0.0, Ticks(960), 1.0, 0.0);
    let l = ClipLane::new(LaneKind::Cc(1), pts);
    assert!((l.value_at(480.0).unwrap() - 0.5).abs() < 1e-6);
    let bent = ClipLane::new(
        LaneKind::Cc(1),
        line(&[], Ticks(0), 0.0, Ticks(960), 1.0, 0.5),
    );
    assert!(bent.value_at(480.0).unwrap() < 0.4);
    assert_eq!(bent.value_at(960.0).unwrap(), 1.0);
    // A stroke drawn right to left is the same line.
    let back = line(&[], Ticks(960), 1.0, Ticks(0), 0.0, 0.0);
    assert_eq!(
        ClipLane::new(LaneKind::Cc(1), back).value_at(480.0),
        Some(0.5)
    );
    // A straight stroke of 97 samples thins to its two ends.
    let stroke: Vec<(Ticks, f32)> = (0..=96).map(|i| (Ticks(i * 10), i as f32 / 96.0)).collect();
    let p = pencil(&[], LaneKind::Cc(1), &stroke);
    assert_eq!(p.len(), 2, "{p:?}");
    // A pedal's lane steps.
    let pedal = pencil(&[], LaneKind::Cc(64), &[(Ticks(0), 0.9), (Ticks(100), 0.2)]);
    assert!(pedal.iter().all(|p| p.value == 0.0 || p.value == 1.0));
}

/// Copy and paste carry what was heard; a paste where the clip has no lane makes one that
/// rests outside the paste.
#[test]
fn spans_paste_what_was_heard() {
    let lane = bend(&[(0, 0.0), (960, 1.0)]);
    let s = span(&lane.points, Ticks(480), Ticks(960)).unwrap();
    assert_eq!(s[0].tick, Ticks::ZERO);
    assert!((s[0].value - 0.5).abs() < 1e-6);
    let pasted = paste(&[], &[ClipLane::new(LaneKind::PitchBend, s)], Ticks(1920));
    let l = &pasted[0];
    assert_eq!(l.value_at(0.0), Some(LaneKind::PitchBend.rest()));
    assert!((l.value_at(1920.0).unwrap() - 0.5).abs() < 1e-6);
    assert_eq!(l.value_at(5000.0), Some(LaneKind::PitchBend.rest()));
}

/// Notes moved carry their controllers: the stretch lands at its new place.
#[test]
fn a_shift_moves_the_stretch() {
    let lanes = vec![bend(&[
        (0, 0.5),
        (960, 0.5),
        (1000, 1.0),
        (1900, 1.0),
        (1920, 0.5),
    ])];
    let moved = shift(&lanes, Ticks(960), Ticks(1920), Ticks(1920));
    let l = &moved[0];
    assert!((l.value_at(3400.0).unwrap() - 1.0).abs() < 1e-6, "{l:?}");
}

/// Glue: the halves of a split give back the lanes they were cut from; a clip glued after
/// another brings its curve into the first's frame.
#[test]
fn glue_gives_back_a_split_and_joins_curves() {
    let lane = bend(&[(0, 0.0), (1920, 1.0)]);
    let a = clip(0, 960, vec![lane.clone()]);
    let mut b = clip(960, 960, vec![lane]);
    b.offset = Ticks(960);
    b.id = ClipId(2);
    assert_eq!(glue(&[&a, &b]), None);
    let c = clip(0, 960, vec![]);
    let mut d = clip(960, 960, vec![bend(&[(0, 1.0)])]);
    d.id = ClipId(3);
    let lanes = glue(&[&c, &d]).unwrap();
    assert_eq!(lanes[0].value_at(1200.0), Some(1.0));
    assert_eq!(lanes[0].value_at(100.0), Some(LaneKind::PitchBend.rest()));
}

/// MIDI messages become steps; a value repeated is dropped; the kind rests before its
/// first message.
#[test]
fn messages_become_steps() {
    let s = steps(&[
        (Ticks(0), 0.5),
        (Ticks(10), 0.5),
        (Ticks(20), 0.7),
        (Ticks(20), 0.9),
    ]);
    assert_eq!(s.len(), 2);
    assert!(s.iter().all(|p| p.shape == Shape::Hold));
    assert_eq!(s[1].value, 0.9);
    let lanes = lanes_from_messages(&[
        (Ticks(480), LaneKind::Cc(64), 1.0),
        (Ticks(960), LaneKind::Cc(64), 0.0),
    ]);
    assert_eq!(lanes[0].value_at(0.0), Some(0.0));
    assert_eq!(lanes[0].value_at(600.0), Some(1.0));
    assert_eq!(lanes[0].value_at(1000.0), Some(0.0));
}

/// Review #8: a clip glued after one with a lane, itself with none, rests that controller
/// over its window, as it did when it played.
#[test]
fn glue_rests_a_lane_over_a_clip_without_it() {
    let a = clip(0, 960, vec![bend(&[(0, 0.9)])]);
    let mut b = clip(960, 960, vec![]);
    b.id = ClipId(2);
    b.notes.clear();
    let lanes = glue(&[&a, &b]).expect("changed");
    assert_eq!(lanes[0].value_at(500.0), Some(0.9));
    assert_eq!(lanes[0].value_at(1500.0), Some(LaneKind::PitchBend.rest()));
}

/// Review #9: overlapping clips on one track: where the later one ends inside the earlier,
/// the controller follows the earlier one, not its rest.
#[test]
fn overlapping_clips_do_not_rest_each_other() {
    let long = clip(0, 3840, vec![bend(&[(0, 0.8)])]);
    let mut short = clip(960, 960, vec![bend(&[(0, 0.2)])]);
    short.id = ClipId(2);
    let ev = compile_track(&track_of(vec![long, short]));
    let at_end: Vec<f32> = ev
        .iter()
        .filter(|e| e.tick == Ticks(1920))
        .map(|e| e.value)
        .collect();
    assert_eq!(at_end, vec![0.8], "{ev:?}");
    assert_eq!(ev.last().unwrap().value, LaneKind::PitchBend.rest());
}

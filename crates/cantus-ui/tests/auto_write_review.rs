//! The review of round 31's automation (ADR 0068): each confirmed finding about writing
//! automation in the application, held by a test.

mod support;
use cantus_core::controller::*;
use cantus_core::*;
use cantus_engine::midi::Heard;
use cantus_engine::{MidiEvent, NoteInput};
use cantus_ui::controllers::Scope;
use cantus_ui::CantusApp;
use std::time::{Duration, Instant};
use support::*;

fn bar(n: i64) -> Ticks {
    Ticks::beats(4 * n)
}

fn msg_at(v: u8, at: Instant) -> Heard {
    Heard {
        port: 0,
        event: MidiEvent {
            status: 0xB0,
            data1: 10,
            data2: v,
        },
        at,
    }
}

/// The starter song with CC 10 on the first strip's pan, its track in `mode`, armed.
fn pan_knob(mode: AutoMode) -> (CantusApp, StripId, TrackId) {
    let mut a = app(cantus_ui::starter_project());
    a.midi_port_names = vec!["Keys".into()];
    let s = a.project().tracks[0].strip.expect("a strip");
    let t = a.project().tracks[0].id;
    let mut m = Mapping::new(
        MappingId(1),
        MidiSource {
            port: PortRef {
                name: "Keys".into(),
                ordinal: 0,
            },
            channel: Some(0),
            control: Control::Cc(10),
        },
        MappingTarget::Param(AutoTarget::StripPan(s)),
        MapMode::Absolute,
    );
    m.takeover = Takeover::Jump;
    assert!(a.add_mapping(Scope::Project, m));
    assert!(a.exec(Command::SetTrackAutoMode { track: t, mode }));
    a.set_auto_armed(true);
    (a, s, t)
}

fn pan_clips(a: &CantusApp, s: StripId) -> usize {
    a.project()
        .tracks
        .iter()
        .flat_map(|t| t.clips.iter())
        .filter(|c| {
            c.automation
                .as_ref()
                .is_some_and(|x| x.target == AutoTarget::StripPan(s))
        })
        .count()
}

/// One frame at `pos`, playing: the pass's jump test, the controller's messages, the holds.
fn frame(a: &mut CantusApp, pos: Ticks, msgs: &[Heard]) {
    a.engine.set_heard_for_tests(pos, true);
    a.auto_write_frame();
    a.controller_heard(msgs);
    a.auto_write_settle();
}

/// #1: a hardware fader ridden in an armed Latch pass writes, with no recording running.
#[test]
fn a_mapped_controller_writes_in_an_armed_pass() {
    let (mut a, s, _) = pan_knob(AutoMode::Latch);
    frame(&mut a, bar(1), &[]);
    frame(&mut a, bar(1) + Ticks(200), &[msg_at(100, Instant::now())]);
    frame(&mut a, bar(2), &[msg_at(20, Instant::now())]);
    a.engine.set_heard_for_tests(bar(3), false);
    a.auto_write_frame();
    assert_eq!(pan_clips(&a, s), 1, "the ride is written");
}

/// #2: a controller's messages, told after the frame's own sample at their earlier times,
/// are all kept, in order.
#[test]
fn every_controller_message_lands_where_it_was_heard() {
    let (mut a, s, _) = pan_knob(AutoMode::Latch);
    frame(&mut a, bar(1), &[]);
    for (k, v) in [10u8, 40, 80, 120].iter().enumerate() {
        let pos = bar(1) + Ticks(480 * (k as i64 + 1));
        let old = Instant::now() - Duration::from_millis(60);
        frame(&mut a, pos, &[msg_at(*v, old)]);
    }
    let points = a
        .written_points()
        .get(&AutoTarget::StripPan(s))
        .cloned()
        .unwrap();
    let values: Vec<f32> = points.iter().map(|p| p.1).collect();
    for v in [10u8, 40, 80, 120] {
        let pos = f32::from(v) / 127.0;
        assert!(
            values.iter().any(|x| (x - pos).abs() < 0.02),
            "{v} kept: {points:?}"
        );
    }
    assert!(
        points.windows(2).all(|w| w[0].0 <= w[1].0),
        "in order: {points:?}"
    );
}

/// #7: a slow frame (three beats in one) is no seek: a latched run goes on unbroken, one
/// clip; a real seek still ends it.
#[test]
fn a_slow_frame_is_no_seek() {
    let (mut a, s, _) = pan_knob(AutoMode::Latch);
    frame(&mut a, bar(1), &[]);
    let key = MergeKey::of(&"pan-drag");
    a.exec_merging(Command::SetStripPan { id: s, pan: 0.5 }, key);
    a.end_gesture();
    frame(&mut a, bar(1) + Ticks(40), &[]);
    frame(&mut a, bar(2), &[]);
    a.engine.set_heard_for_tests(bar(2) + Ticks(40), false);
    a.auto_write_frame();
    assert_eq!(pan_clips(&a, s), 1, "one stretch, unbroken");
}

/// #6: a recording pass that records a take and automation is one undo step: undo takes
/// the take and the automation back together.
#[test]
fn a_recording_pass_with_automation_is_one_step() {
    let mut a = app(cantus_ui::starter_project());
    let keys = a.project().tracks[0].id;
    let s = a.project().tracks[0].strip.expect("a strip");
    a.toggle_arm(keys, false);
    let before = a.project().clone();
    let steps = a.bus.history().len();
    a.engine.seek(bar(1));
    a.start_recording();
    assert!(a.is_recording());
    a.engine.set_heard_for_tests(bar(1) + Ticks(100), true);
    a.auto_write_frame();
    a.exec(Command::SetStripPan { id: s, pan: -0.4 });
    a.auto_write_settle();
    let ev = |tick: i64, vel: u8| NoteInput {
        track: keys,
        tick: Ticks(tick),
        pitch: 62,
        velocity: vel,
        release: 0,
    };
    a.ingest_note_inputs(vec![ev(bar(1).0 + 200, 100), ev(bar(1).0 + 700, 0)]);
    a.engine.set_heard_for_tests(bar(2), true);
    a.auto_write_frame();
    a.auto_write_settle();
    a.stop_recording();
    assert_eq!(pan_clips(&a, s), 1);
    // The pan move, the toggle-arm's own step aside: one step for the take and its automation.
    assert_eq!(a.bus.history().len(), steps + 1, "one undo step");
    a.undo();
    assert_eq!(pan_clips(&a, s), 0);
    let mut back = a.project().clone();
    back.next_id = before.next_id;
    assert_eq!(back, before, "the take and the automation gone together");
}

/// Master in Latch, armed, its fader ridden while playing: a pass open on the master's gain.
fn master_ride() -> CantusApp {
    let mut a = app(cantus_ui::starter_project());
    assert!(a.exec(Command::SetMasterAutoMode {
        mode: AutoMode::Latch
    }));
    a.set_auto_armed(true);
    frame(&mut a, bar(1), &[]);
    let key = MergeKey::of(&"master-drag");
    a.exec_merging(Command::SetMasterGain { gain_db: -9.0 }, key);
    a.end_gesture();
    frame(&mut a, bar(2), &[]);
    a
}

/// No automation clip anywhere in `p`.
fn no_automation(p: &Project) -> bool {
    p.tracks
        .iter()
        .flat_map(|t| t.clips.iter())
        .all(|c| c.automation.is_none())
}

/// The frames after another project is in place, stopped.
fn settle_stopped(a: &mut CantusApp) {
    a.engine.set_heard_for_tests(Ticks::ZERO, false);
    a.auto_write_frame();
    a.auto_write_settle();
    a.auto_write_frame();
}

/// The integrated review's #1: a pass belongs to the project it rode. *New from template*
/// in the middle of one writes nothing into the new project, nor its old value back.
#[test]
fn a_new_project_from_a_template_ends_the_pass_with_the_old_one() {
    use cantus_ui::templates::{open_template, Starter, TemplateRef};
    let mut a = master_ride();
    let template = TemplateRef::Starter(Starter::Mastering);
    let fresh = open_template(&template).expect("the starter");
    a.new_from_template(&template);
    settle_stopped(&mut a);
    assert!(
        no_automation(a.project()),
        "nothing written into the new one"
    );
    assert_eq!(a.project().master.gain_db, fresh.master.gain_db);
    assert_eq!(a.project().master.auto_mode, fresh.master.auto_mode);
}

/// #1, as a command (an import, a recovery): the ride is the old project's, kept in its
/// history; the new project gets none of it.
#[test]
fn another_song_in_place_ends_the_pass_in_the_old_one() {
    let mut a = master_ride();
    let mut other = cantus_ui::starter_project();
    other.master.gain_db = 2.5;
    assert!(a.exec(Command::ReplaceProject {
        project: Box::new(other)
    }));
    a.engine.set_heard_for_tests(bar(3), true);
    a.auto_write_frame();
    a.auto_write_settle();
    settle_stopped(&mut a);
    assert!(
        no_automation(a.project()),
        "nothing written into the new one"
    );
    assert_eq!(a.project().master.gain_db, 2.5);
    a.undo();
    assert!(!no_automation(a.project()), "the old project kept its ride");
}

/// #1: undo or redo of another song waits while a pass writes.
#[test]
fn undoing_another_song_waits_while_a_pass_writes() {
    let mut a = app(cantus_ui::starter_project());
    let other = cantus_ui::starter_project();
    assert!(a.exec(Command::ReplaceProject {
        project: Box::new(other)
    }));
    assert!(a.exec(Command::SetMasterAutoMode {
        mode: AutoMode::Latch
    }));
    a.set_auto_armed(true);
    frame(&mut a, bar(1), &[]);
    let key = MergeKey::of(&"master-drag");
    a.exec_merging(Command::SetMasterGain { gain_db: -9.0 }, key);
    a.end_gesture();
    frame(&mut a, bar(2), &[]);
    // The ride's step and the mode's go back; the song's is held.
    for _ in 0..3 {
        a.undo();
    }
    assert!(
        a.bus
            .history()
            .last()
            .is_some_and(|e| matches!(e.inverse, Command::ReplaceProject { .. })),
        "the song stays while the pass writes"
    );
}

/// What a stream holds for `kind` at `t`: its last value at or before (`None` before any).
fn held_at(
    ev: &[cantus_core::clip_lanes::CtlEvent],
    kind: cantus_core::clip_lanes::LaneKind,
    t: Ticks,
) -> Option<f32> {
    ev.iter()
        .rfind(|e| e.kind == kind && e.tick <= t)
        .map(|e| e.value)
}

/// The integrated review's #2: *Bake to notes* of a trimmed, looping clip keeps its
/// controllers where they were heard, every repetition.
#[test]
fn bake_to_notes_keeps_the_controllers_as_heard() {
    use cantus_core::clip_lanes::{compile_track, ClipLane, LaneKind, STEP};
    let mut p = cantus_ui::starter_project();
    let ti = p
        .tracks
        .iter()
        .position(|t| t.clips.iter().any(|c| !c.notes.is_empty()))
        .expect("a note track");
    let ci = p.tracks[ti]
        .clips
        .iter()
        .position(|c| !c.notes.is_empty())
        .unwrap();
    {
        let c = &mut p.tracks[ti].clips[ci];
        c.offset = Ticks::beats(1);
        c.content_length = Some(Ticks::beats(4));
        c.length = Ticks::beats(10);
        c.lanes = vec![
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
            // A pedal drawn as holds (the fix-round review: it stayed a ramp after Bake).
            ClipLane::new(
                LaneKind::Cc(64),
                [(0, 1.0), (2, 0.0), (3, 1.0), (5, 0.0)]
                    .iter()
                    .map(|&(b, v)| {
                        let mut p = AutoPoint::new(Ticks::beats(b) + Ticks(480 * (b % 2)), v);
                        p.shape = cantus_core::automation::Shape::Hold;
                        p
                    })
                    .collect(),
            ),
        ];
    }
    // The clip alone on its track, so nothing else is heard over it.
    let keep = p.tracks[ti].clips[ci].clone();
    p.tracks[ti].clips = vec![keep.clone()];
    let mut a = app(p);
    let t = a.project().tracks[ti].id;
    let before = compile_track(a.project().track(t).unwrap());
    assert!(a.bake_note_fx(t, keep.id));
    let after = compile_track(a.project().track(t).unwrap());
    let mut at = keep.start;
    while at < keep.end() {
        for kind in [LaneKind::PitchBend, LaneKind::Cc(1), LaneKind::Cc(64)] {
            let (x, y) = (held_at(&before, kind, at), held_at(&after, kind, at));
            let alike = match (x, y) {
                (Some(x), Some(y)) => (x - y).abs() < 0.01,
                (x, y) => x == y,
            };
            assert!(alike, "{kind:?} at {at:?}: {x:?} heard, {y:?} baked");
        }
        at += Ticks(STEP / 2 + 1);
    }
}

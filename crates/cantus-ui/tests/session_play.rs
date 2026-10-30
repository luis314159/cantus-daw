//! The session's conductor in the application (ADR 0076 §7–8): presses sent to the engine
//! handle become a performance, heard as its projection; the view is told what is queued
//! and what is heard; arrangement record writes what was heard as one undo step; a slot
//! records what is played into it.

use cantus_core::session::*;
use cantus_core::*;
use cantus_engine::messages::{SessionEvent, SessionMsg, SlotPhase};
use cantus_engine::{EngineHandle, EngineMessage, NoteInput};
use cantus_ui::CantusApp;

const BAR: Ticks = Ticks::beats(4);

fn app() -> (CantusApp, TrackId, Vec<SceneId>) {
    let mut p = Project::new("live");
    let t = p.make_track("Keys");
    let tid = t.id;
    p.tracks.push(t);
    let scenes: Vec<SceneId> = (0..2).map(|_| SceneId(p.alloc_id())).collect();
    for s in &scenes {
        p.session.scenes.push(Scene::new(*s, ""));
    }
    let mut c = p.make_clip("Riff", Ticks::ZERO, BAR);
    c.content_length = Some(BAR);
    for b in 0..4 {
        let n = p.make_note(Ticks::beats(b), Ticks::QUARTER, 60 + b as u8, 100);
        c.notes.push(n);
    }
    p.tracks[0].slots.push(Slot {
        scene: scenes[0],
        clip: Some(SessionClip::new(c)),
        stop: true,
    });
    (
        CantusApp::from_parts(Some(p), EngineHandle::dummy(), false),
        tid,
        scenes,
    )
}

fn send(app: &mut CantusApp, m: SessionMsg) {
    app.engine.send(EngineMessage::Session(m));
    app.sync_engine();
}

fn at(app: &mut CantusApp, tick: Ticks, playing: bool) {
    app.engine.set_heard_for_tests(tick, playing);
    app.sync_engine();
}

#[test]
fn a_launch_from_stop_plays_at_the_playhead_and_is_heard_as_arrangement() {
    let (mut app, tid, scenes) = app();
    let slot = SlotRef::new(tid, scenes[0]);
    send(
        &mut app,
        SessionMsg::Launch {
            slot,
            velocity: 1.0,
        },
    );
    let played = &app.perform.plan().perf.played;
    assert_eq!(played[0].at, Ticks::ZERO);
    let events = app.engine.take_session_events();
    assert!(events.contains(&SessionEvent::Slot {
        slot,
        phase: SlotPhase::Playing {
            since: Ticks::ZERO,
            from: Ticks::ZERO
        }
    }));
    assert!(events.contains(&SessionEvent::Track {
        track: tid,
        arrangement: false
    }));
    let heard = cantus_ui::session_play::play(&mut app, None).expect("a projection");
    assert!(!heard.track(tid).unwrap().clips.is_empty());
    // The song itself is untouched.
    assert!(app.project().track(tid).unwrap().clips.is_empty());
}

#[test]
fn a_launch_while_playing_is_queued_to_the_bar_then_heard() {
    let (mut app, tid, scenes) = app();
    let slot = SlotRef::new(tid, scenes[0]);
    at(&mut app, Ticks(500), true);
    send(
        &mut app,
        SessionMsg::Launch {
            slot,
            velocity: 1.0,
        },
    );
    let events = app.engine.take_session_events();
    assert!(events.contains(&SessionEvent::Slot {
        slot,
        phase: SlotPhase::Queued { at: BAR }
    }));
    assert!(!events.iter().any(|e| matches!(
        e,
        SessionEvent::Slot {
            phase: SlotPhase::Playing { .. },
            ..
        }
    )));
    at(&mut app, BAR, true);
    let events = app.engine.take_session_events();
    assert!(events.contains(&SessionEvent::Slot {
        slot,
        phase: SlotPhase::Playing {
            since: BAR,
            from: Ticks::ZERO
        }
    }));
    // A stop is told as stopping, with where the clip is.
    send(&mut app, SessionMsg::StopTrack { track: tid });
    let events = app.engine.take_session_events();
    assert!(events.contains(&SessionEvent::Slot {
        slot,
        phase: SlotPhase::Stopping {
            since: BAR,
            from: Ticks::ZERO,
            at: BAR * 2
        }
    }));
}

#[test]
fn arrangement_record_writes_what_was_heard_in_one_step() {
    let (mut app, tid, scenes) = app();
    let slot = SlotRef::new(tid, scenes[0]);
    at(&mut app, Ticks(500), true);
    send(&mut app, SessionMsg::SetArrangementRecord(true));
    send(
        &mut app,
        SessionMsg::Launch {
            slot,
            velocity: 1.0,
        },
    );
    at(&mut app, BAR * 3, true);
    // Stopped: the performance ends and is written.
    at(&mut app, BAR * 3, false);
    let clips = &app.project().track(tid).unwrap().clips;
    let span: i64 = clips.iter().map(|c| c.length.0).sum();
    assert_eq!(Ticks(span), BAR * 2, "bars 2 and 3, as heard");
    assert_eq!(
        app.bus.undo_label_key(),
        Some("cmd.session.commit_performance")
    );
    assert!(!app.perform.active());
}

#[test]
fn a_slot_records_what_is_played_into_it_and_plays_it_on() {
    let (mut app, tid, scenes) = app();
    let slot = SlotRef::new(tid, scenes[1]);
    at(&mut app, Ticks(100), true);
    send(&mut app, SessionMsg::Record { slot });
    assert!(app.project().track(tid).unwrap().armed, "armed to record");
    let events = app.engine.take_session_events();
    assert!(events.contains(&SessionEvent::Slot {
        slot,
        phase: SlotPhase::RecordQueued { at: BAR }
    }));
    at(&mut app, BAR, true);
    assert!(app.is_recording());
    let note = |tick: Ticks, velocity: u8| NoteInput {
        track: tid,
        tick,
        pitch: 64,
        velocity,
        release: 0,
    };
    app.ingest_note_inputs(vec![
        note(BAR + Ticks(10), 90),
        note(BAR + Ticks(400), 0),
        note(BAR + Ticks(960), 80),
        note(BAR + Ticks(1400), 0),
    ]);
    send(&mut app, SessionMsg::StopRecording { slot });
    at(&mut app, BAR + Ticks(2000), true);
    at(&mut app, BAR * 2, true);
    assert!(!app.is_recording());
    let c = app
        .project()
        .slot_clip(slot)
        .expect("the take, in the slot");
    assert_eq!(c.clip.notes.len(), 2);
    assert_eq!(c.clip.notes[0].start, Ticks(10));
    assert_eq!(c.end(), BAR);
    assert!(c.looping());
    assert_eq!(app.bus.undo_label_key(), Some("cmd.session.record"));
    // It plays on.
    assert!(app
        .perform
        .plan()
        .perf
        .played
        .iter()
        .any(|p| p.slot == slot));
    assert!(
        app.project().track(tid).unwrap().clips.is_empty(),
        "nothing in the arrangement"
    );
}

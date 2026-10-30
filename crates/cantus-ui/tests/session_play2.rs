//! The conductor's fix round (the integrator's review of the engine): a launch from stop
//! that survives the frames before the engine says it plays, the lead, a compile that runs
//! late, launching a recording slot, a queued recording dropped on stop, overdub across
//! follow actions, a fixed length, a MIDI take laid ahead of its end, a count-in, audio
//! recovered into its slot.

use cantus_core::session::*;
use cantus_core::*;
use cantus_engine::messages::{SessionEvent, SessionMsg, SlotPhase};
use cantus_engine::{EngineHandle, EngineMessage, FinishedTake, NoteInput};
use cantus_ui::session_play::lead_s;
use cantus_ui::CantusApp;

const BAR: Ticks = Ticks::beats(4);

fn app() -> (CantusApp, TrackId, Vec<SceneId>) {
    let mut p = Project::new("live");
    let t = p.make_track("Keys");
    let tid = t.id;
    p.tracks.push(t);
    let scenes: Vec<SceneId> = (0..3).map(|_| SceneId(p.alloc_id())).collect();
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

fn note(track: TrackId, tick: Ticks, velocity: u8) -> NoteInput {
    NoteInput {
        track,
        tick,
        pitch: 64,
        velocity,
        release: 0,
    }
}

/// Finding 1: a launch from stop is not taken back on the frames before the engine says
/// it plays.
#[test]
fn a_launch_from_stop_survives_until_the_engine_plays() {
    let (mut app, tid, scenes) = app();
    let slot = SlotRef::new(tid, scenes[0]);
    send(
        &mut app,
        SessionMsg::Launch {
            slot,
            velocity: 1.0,
        },
    );
    app.sync_engine();
    app.sync_engine();
    assert!(app.perform.active(), "still performing");
    assert!(app
        .perform
        .plan()
        .perf
        .played
        .iter()
        .any(|p| p.slot == slot));
    at(&mut app, Ticks(200), true);
    assert!(app.perform.active());
    // A real stop still ends it.
    at(&mut app, Ticks(300), false);
    assert!(!app.perform.active());
}

/// D1: the lead is two blocks, the worst compile and a floor; and the numbers at the
/// three buffers the report gives.
#[test]
fn the_lead_is_two_blocks_the_compile_and_a_floor() {
    let ms = |b: usize, c: f64| lead_s(b, 48_000.0, c) * 1000.0;
    assert!((ms(256, 0.0) - (2.0 * 256.0 / 48.0 + 2.0)).abs() < 1e-9);
    for b in [64, 256, 1024] {
        eprintln!(
            "lead at {b} frames: {:.2} ms (+ a 3 ms compile: {:.2} ms)",
            ms(b, 0.0),
            ms(b, 0.003)
        );
    }
    let (mut app, tid, scenes) = app();
    let slot = SlotRef::new(tid, scenes[0]);
    let mut q = app.project().slot_clip(slot).unwrap().launch;
    q.quantize = Quantize::None;
    app.exec(Command::Session(SessionCommand::SetSlotLaunch {
        slot,
        launch: q,
    }));
    at(&mut app, Ticks(100), true);
    send(
        &mut app,
        SessionMsg::Launch {
            slot,
            velocity: 1.0,
        },
    );
    let effect = app.perform.plan().perf.played[0].at;
    // At least two blocks and the floor after the playhead.
    let floor = Ticks::from_seconds(
        lead_s(app.engine.stats().block_frames.max(1), 48_000.0, 0.0),
        120.0,
    );
    assert!(effect >= Ticks(100) + floor, "{effect:?}");
}

/// D2: a compile that runs past its press's tick is planned again from where it can land,
/// before it is sent: what is planned is what is heard.
#[test]
fn a_late_compile_is_planned_again_before_it_is_sent() {
    let (mut app, tid, scenes) = app();
    let slot = SlotRef::new(tid, scenes[0]);
    let mut q = app.project().slot_clip(slot).unwrap().launch;
    q.quantize = Quantize::None;
    app.exec(Command::Session(SessionCommand::SetSlotLaunch {
        slot,
        launch: q,
    }));
    at(&mut app, Ticks(100), true);
    app.set_compile_delay_for_tests(std::time::Duration::from_secs(1));
    send(
        &mut app,
        SessionMsg::Launch {
            slot,
            velocity: 1.0,
        },
    );
    let effect = app.perform.plan().perf.played[0].at;
    // A second at 120 BPM is two beats.
    assert!(effect >= Ticks(100) + Ticks::beats(2), "{effect:?}");
}

/// Finding 4: launching the slot that records ends the recording at the bar line, and the
/// clip plays on.
#[test]
fn launching_the_recording_slot_ends_it_at_the_bar() {
    let (mut app, tid, scenes) = app();
    let slot = SlotRef::new(tid, scenes[1]);
    at(&mut app, Ticks(100), true);
    send(&mut app, SessionMsg::Record { slot });
    at(&mut app, BAR, true);
    assert!(app.is_recording());
    app.ingest_note_inputs(vec![
        note(tid, BAR + Ticks(10), 90),
        note(tid, BAR + Ticks(300), 0),
    ]);
    at(&mut app, BAR + Ticks(1000), true);
    send(
        &mut app,
        SessionMsg::Launch {
            slot,
            velocity: 1.0,
        },
    );
    at(&mut app, BAR * 2 - Ticks(100), true);
    at(&mut app, BAR * 2, true);
    assert!(!app.is_recording(), "it ended at the bar");
    assert_eq!(app.project().slot_clip(slot).unwrap().end(), BAR);
    assert!(app
        .perform
        .plan()
        .perf
        .played
        .iter()
        .any(|p| p.slot == slot && p.at == BAR * 2 && p.from == Ticks::ZERO));
}

/// Finding 5: a recording queued for its boundary goes when the transport stops first.
#[test]
fn a_queued_recording_goes_when_the_transport_stops() {
    let (mut app, tid, scenes) = app();
    let slot = SlotRef::new(tid, scenes[1]);
    at(&mut app, Ticks(100), true);
    send(&mut app, SessionMsg::Record { slot });
    at(&mut app, Ticks(200), false);
    assert!(app.perform.recording().is_none());
    // A later record is taken.
    at(&mut app, Ticks(300), true);
    send(&mut app, SessionMsg::Record { slot });
    assert!(app.perform.recording().is_some());
}

/// Finding 3: overdubbing a clip that plays itself again every bar: every note lands, in
/// the pass it was heard.
#[test]
fn overdub_follows_the_clip_through_its_follow_actions() {
    let (mut app, tid, scenes) = app();
    let slot = SlotRef::new(tid, scenes[0]);
    app.exec(Command::Session(SessionCommand::SetSlotFollow {
        slot,
        follow: FollowActions {
            on: true,
            a: FollowAction::Again,
            b: FollowAction::Again,
            chance_a: 100,
            after: FollowAfter::Linked,
            count: 1,
        },
    }));
    at(&mut app, Ticks(100), true);
    send(
        &mut app,
        SessionMsg::Launch {
            slot,
            velocity: 1.0,
        },
    );
    at(&mut app, BAR + Ticks(10), true);
    send(&mut app, SessionMsg::Record { slot });
    at(&mut app, BAR * 2, true);
    assert!(app.is_recording());
    app.ingest_note_inputs(vec![
        note(tid, BAR * 2 + Ticks(100), 90),
        note(tid, BAR * 2 + Ticks(200), 0),
        note(tid, BAR * 3 + Ticks(500), 80),
        note(tid, BAR * 3 + Ticks(600), 0),
    ]);
    at(&mut app, BAR * 3 + Ticks(10), true);
    send(&mut app, SessionMsg::StopRecording { slot });
    at(&mut app, BAR * 4, true);
    let c = app.project().slot_clip(slot).unwrap();
    assert_eq!(c.clip.notes.len(), 6, "both bars' notes, overdubbed");
    let starts: Vec<Ticks> = c.clip.notes.iter().map(|n| n.start).collect();
    assert!(starts.contains(&Ticks(100)) && starts.contains(&Ticks(500)));
}

/// A fixed length ends the recording by itself; the MIDI take is laid ahead of its end, so
/// it plays on from the bar line, and keeps its id when the take is whole.
#[test]
fn a_fixed_length_take_is_laid_ahead_and_plays_on_from_its_end() {
    let (mut app, tid, scenes) = app();
    let slot = SlotRef::new(tid, scenes[1]);
    let mut s = app.project().session.settings;
    s.fixed_bars = Some(2);
    app.exec(Command::Session(SessionCommand::SetSettings {
        settings: s,
    }));
    at(&mut app, Ticks(100), true);
    send(&mut app, SessionMsg::Record { slot });
    at(&mut app, BAR, true);
    app.ingest_note_inputs(vec![
        note(tid, BAR + Ticks(10), 90),
        note(tid, BAR + Ticks(400), 0),
    ]);
    // A frame a little before the end: the take is in the projection, launched on the end.
    at(&mut app, BAR * 3 - Ticks(100), true);
    assert!(app.is_recording());
    let launched = app
        .perform
        .plan()
        .perf
        .played
        .iter()
        .find(|p| p.slot == slot)
        .copied()
        .expect("launched ahead");
    assert_eq!((launched.at, launched.from), (BAR * 3, Ticks::ZERO));
    let heard = cantus_ui::session_play::play(&mut app, None).unwrap();
    let ahead = heard
        .track(tid)
        .unwrap()
        .clips
        .iter()
        .find(|c| c.start == BAR * 3)
        .map(|c| c.notes.len());
    assert_eq!(ahead, Some(1));
    at(&mut app, BAR * 3, true);
    assert!(!app.is_recording());
    let c = app.project().slot_clip(slot).unwrap();
    assert_eq!(c.end(), BAR * 2);
    assert_eq!(c.clip.notes.len(), 1);
    // Still launched on the bar line, not later.
    let p = app
        .perform
        .plan()
        .perf
        .played
        .iter()
        .find(|p| p.slot == slot)
        .copied()
        .unwrap();
    assert_eq!(p.at, BAR * 3);
}

/// Recording into a slot from stop counts in, and records from the playhead.
#[test]
fn a_slot_recording_from_stop_counts_in() {
    let (mut app, tid, scenes) = app();
    let slot = SlotRef::new(tid, scenes[1]);
    let mut r = app.project().recording.clone();
    r.count_in_bars = 1;
    app.exec(Command::SetRecordSettings { settings: r });
    at(&mut app, BAR * 2, false);
    send(&mut app, SessionMsg::Record { slot });
    assert!(app.is_recording());
    assert_eq!(app.perform.recording().unwrap().from, BAR * 2);
}

/// An audio take whose pass ended without its recording state (a crash, recovered from the
/// journal) comes back into its slot, not the arrangement.
#[test]
fn an_audio_take_recovered_comes_back_into_its_slot() {
    let (mut app, tid, scenes) = app();
    let slot = SlotRef::new(tid, scenes[2]);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("take.wav");
    std::fs::write(&path, b"RIFF").unwrap();
    let mut plan = cantus_ui::record_pass::PassPlan::new(BAR);
    plan.audio_tracks = vec![tid];
    plan.punch = Some((BAR, BAR * 3));
    plan.stop_at = BAR * 3;
    plan.slot = Some(slot);
    let take = FinishedTake {
        track: tid,
        path,
        sample_rate: 48_000,
        channels: 1,
        frames: 48_000 * 8,
        start_tick: 0.0,
        wraps: Vec::new(),
        wrap_shifts: Vec::new(),
        dropped: 0,
    };
    assert!(app.finish_pass(plan, vec![take], Vec::new()));
    let c = app.project().slot_clip(slot).expect("into its slot");
    assert!(c.clip.audio.is_some());
    assert_eq!(c.clip.length, BAR * 2);
    assert!(app.project().track(tid).unwrap().clips.is_empty());
    assert!(app.engine.take_session_events().iter().all(|e| !matches!(
        e,
        SessionEvent::Slot {
            phase: SlotPhase::Recording { .. },
            ..
        }
    )));
}

//! The seams of the integrated round (the integrator's review of r33/integration): the
//! session while a source is followed — a launch from stop waits for Link's quantum or the
//! master's Start, a sync start or relock is not a locate, the lead at the followed tempo, a
//! scene's tempo proposed to Link — and an MPE controller recorded into a slot.

use cantus_core::model::note_expr::ExprKind;
use cantus_core::session::*;
use cantus_core::*;
use cantus_engine::inst::mpe::{ExprInput, MpeIn};
use cantus_engine::messages::{SessionEvent, SessionMsg, SlotPhase};
use cantus_engine::transport::sync::{Lock, SyncShared};
use cantus_engine::{EngineHandle, EngineMessage};
use cantus_ui::session_play::{lead_s, FIRST_COMPILE_S};
use cantus_ui::views::transport::sync_service::SyncSource;
use cantus_ui::CantusApp;
use std::sync::atomic::Ordering;
use std::sync::Arc;

const BAR: Ticks = Ticks::beats(4);

/// One track, three scenes; a one-bar loop of notes in the first slot.
fn app(bpm: f64) -> (CantusApp, TrackId, Vec<SceneId>) {
    let mut p = Project::new("live");
    p.tempo_bpm = bpm;
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

/// `source` followed through a seam of the app's own, in `lock`, at `tempo` (0: none).
fn follow(app: &mut CantusApp, source: SyncSource, lock: Lock, tempo: f64) -> Arc<SyncShared> {
    app.settings.sync.source = source;
    app.settings.sync.quantum = 8.0;
    let seam = app.sync_seam_for_tests();
    set(&seam, lock, tempo);
    seam
}

fn set(seam: &SyncShared, lock: Lock, tempo: f64) {
    seam.set_lock(lock);
    seam.tempo_bits.store(tempo.to_bits(), Ordering::Relaxed);
}

fn send(app: &mut CantusApp, m: SessionMsg) {
    app.engine.send(EngineMessage::Session(m));
    app.sync_engine();
}

fn launch(app: &mut CantusApp, slot: SlotRef) {
    send(
        app,
        SessionMsg::Launch {
            slot,
            velocity: 1.0,
        },
    );
}

fn at(app: &mut CantusApp, tick: Ticks, playing: bool) {
    app.engine.set_heard_for_tests(tick, playing);
    app.sync_engine();
}

/// Frames for longer than a launch from stop waits with no engine (`STARTING_S`).
fn hold(app: &mut CantusApp) {
    let t = std::time::Instant::now();
    while t.elapsed() < std::time::Duration::from_millis(2_300) {
        app.sync_engine();
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

/// Seam 1: following Link (quantum 8 at 90 BPM: a start can wait over 5 s), a launch from
/// stop waits for the quantum — shown queued, still the performance — and plays from where
/// it was launched when the transport starts.
#[test]
fn a_launch_from_stop_waits_for_links_quantum() {
    let (mut app, tid, scenes) = app(90.0);
    let slot = SlotRef::new(tid, scenes[0]);
    let seam = follow(&mut app, SyncSource::Link, Lock::Queued, 0.0);
    at(&mut app, BAR * 2, false);
    launch(&mut app, slot);
    hold(&mut app);
    assert!(app.perform.active(), "still waiting for the quantum");
    let events = app.engine.take_session_events();
    assert!(
        events.iter().any(|e| matches!(
            e,
            SessionEvent::Slot {
                phase: SlotPhase::Queued { .. },
                ..
            }
        )),
        "shown queued: {events:?}"
    );
    assert!(!events.iter().any(|e| matches!(
        e,
        SessionEvent::Slot {
            phase: SlotPhase::Playing { .. },
            ..
        }
    )));
    // The quantum: Link starts the transport where it stands, in phase.
    set(&seam, Lock::Locked, 90.0);
    at(&mut app, BAR * 2 + Ticks(5), true);
    assert!(app.perform.active());
    assert_eq!(app.perform.plan().perf.played[0].at, BAR * 2);
    at(&mut app, BAR * 2 + Ticks(200), true);
    let events = app.engine.take_session_events();
    assert!(events.iter().any(|e| matches!(
        e,
        SessionEvent::Slot {
            phase: SlotPhase::Playing { .. },
            ..
        }
    )));
}

/// Seam 1: following MIDI clock, a launch from stop waits for the master's Start however
/// long; a song position while it waits moves it there; a Start from the top (behind
/// where it was launched) plays it from the top, caught up — not a "jump back".
#[test]
fn a_launch_from_stop_waits_for_the_masters_start() {
    let (mut app, tid, scenes) = app(120.0);
    let slot = SlotRef::new(tid, scenes[0]);
    let seam = follow(&mut app, SyncSource::Clock, Lock::Queued, 0.0);
    at(&mut app, BAR * 2, false);
    launch(&mut app, slot);
    hold(&mut app);
    assert!(app.perform.active(), "still waiting for the master");
    // The master's song position: the playhead stands at bar 2.
    at(&mut app, BAR, false);
    assert_eq!(app.perform.plan().perf.played[0].at, BAR);
    // Its Start from the top.
    set(&seam, Lock::Locked, 120.0);
    at(&mut app, Ticks(10), true);
    assert!(app.perform.active(), "a sync start is no locate");
    let x = app.perform.plan().perf.played[0];
    assert!(x.at < BAR, "{:?}", x.at);
    assert_eq!(x.origin, Ticks::ZERO, "caught up from the start");
}

/// Seam 5: following Link, a relock jumping the playhead back within the quantum keeps the
/// performance; a locate sent by the user still ends it.
#[test]
fn a_links_relock_is_no_locate() {
    let (mut app, tid, scenes) = app(120.0);
    let slot = SlotRef::new(tid, scenes[0]);
    let _seam = follow(&mut app, SyncSource::Link, Lock::Locked, 120.0);
    at(&mut app, Ticks(100), true);
    launch(&mut app, slot);
    at(&mut app, BAR * 2, true);
    assert!(app.perform.active());
    at(&mut app, BAR * 2 - Ticks::beats(1), true);
    assert!(app.perform.active(), "the relock is the source's");
    at(&mut app, BAR * 2 + Ticks::beats(1), true);
    app.engine.seek(Ticks::ZERO);
    at(&mut app, Ticks(30), true);
    assert!(!app.perform.active(), "the user's locate ends it");
}

/// Seam 6: following at twice the song's tempo, the lead is its seconds at the followed
/// tempo (the playhead's ticks go at it).
#[test]
fn the_lead_is_counted_at_the_followed_tempo() {
    let (mut app, tid, scenes) = app(120.0);
    let slot = SlotRef::new(tid, scenes[0]);
    let mut q = app.project().slot_clip(slot).unwrap().launch;
    q.quantize = Quantize::None;
    app.exec(Command::Session(SessionCommand::SetSlotLaunch {
        slot,
        launch: q,
    }));
    let _seam = follow(&mut app, SyncSource::Link, Lock::Locked, 240.0);
    at(&mut app, Ticks(100), true);
    launch(&mut app, slot);
    let stats = app.engine.stats();
    let rate = if stats.sample_rate > 0.0 {
        stats.sample_rate
    } else {
        app.engine.sample_rate
    };
    let lead = Ticks::from_seconds(
        lead_s(stats.block_frames.max(1), rate, FIRST_COMPILE_S),
        240.0,
    );
    let got = app.perform.plan().perf.played[0].at - Ticks(100);
    assert!(
        got >= lead && got < lead + lead / 2,
        "{got:?} against {lead:?} at the followed tempo ({:?} at the song's)",
        lead / 2
    );
}

/// Seam 7: a scene's tempo goes with the projection it plays in, at the scene's bar, for the
/// engine to propose to a followed Link session as that tick is heard (`cantus-engine`
/// `tests/session_sync.rs`); the song the performance ends with carries none.
#[test]
fn a_scenes_tempo_goes_with_its_projection() {
    let (mut app, _tid, scenes) = app(120.0);
    app.exec(Command::Session(SessionCommand::SetSceneTempo {
        id: scenes[0],
        tempo: Some(140.0),
    }));
    let _seam = follow(&mut app, SyncSource::Link, Lock::Locked, 120.0);
    at(&mut app, Ticks(500), true);
    send(
        &mut app,
        SessionMsg::LaunchScene {
            scene: scenes[0],
            velocity: 1.0,
        },
    );
    let s = app.projection_for_tests().expect("a projection");
    assert_eq!(s.scene_tempos, vec![(BAR, 140.0)]);
    at(&mut app, BAR * 2, false);
    assert!(!app.perform.active());
    assert!(app.projection_for_tests().is_none(), "the song again");
}

fn expr(track: TrackId, tick: Ticks, start: Ticks, what: MpeIn) -> ExprInput {
    ExprInput {
        track,
        tick,
        channel: 2,
        pitch: 64,
        start,
        what,
    }
}

/// A note played with expression from `start`, its bend moving, off at `off`.
fn mpe_note(app: &mut CantusApp, track: TrackId, start: Ticks, off: Ticks) {
    let bend = |tick, value| {
        expr(
            track,
            tick,
            start,
            MpeIn::Value {
                kind: ExprKind::Bend,
                value,
            },
        )
    };
    app.engine.inject_expr_inputs(&[
        expr(track, start, start, MpeIn::On { velocity: 100 }),
        bend(start + Ticks(50), 300),
        bend(start + Ticks(150), 900),
    ]);
    app.ingest_note_inputs(Vec::new());
    app.engine
        .inject_expr_inputs(&[expr(track, off, start, MpeIn::Off { release: 0 })]);
    app.ingest_note_inputs(Vec::new());
}

/// Seam 2: an MPE controller recorded into an empty slot (a fixed length, laid ahead) keeps
/// every note's expression — cut where the take's end cuts a note.
#[test]
fn a_slot_take_keeps_its_expression() {
    let (mut app, tid, scenes) = app(120.0);
    let slot = SlotRef::new(tid, scenes[1]);
    let mut s = app.project().session.settings;
    s.fixed_bars = Some(2);
    app.exec(Command::Session(SessionCommand::SetSettings {
        settings: s,
    }));
    at(&mut app, Ticks(100), true);
    send(&mut app, SessionMsg::Record { slot });
    at(&mut app, BAR, true);
    assert!(app.is_recording());
    mpe_note(&mut app, tid, BAR + Ticks(10), BAR + Ticks(400));
    // Held across the end: cut there.
    mpe_note(&mut app, tid, BAR * 3 - Ticks(100), BAR * 3 + Ticks(300));
    at(&mut app, BAR * 3 - Ticks(100), true);
    at(&mut app, BAR * 3 + Ticks(400), true);
    assert!(!app.is_recording());
    let c = app.project().slot_clip(slot).unwrap();
    assert_eq!(c.clip.notes.len(), 2);
    for n in &c.clip.notes {
        let e = n.expr.as_ref().expect("its expression");
        assert!(!e.curve(ExprKind::Bend).is_empty());
    }
    let cut = c.clip.notes.iter().max_by_key(|n| n.start).unwrap();
    assert!(cut.length < Ticks(400), "cut at the end: {:?}", cut.length);
}

/// Seam 2: overdubbed into a looping clip that plays, the notes keep their expression.
#[test]
fn an_overdub_keeps_its_expression() {
    let (mut app, tid, scenes) = app(120.0);
    let slot = SlotRef::new(tid, scenes[0]);
    at(&mut app, Ticks(100), true);
    launch(&mut app, slot);
    at(&mut app, BAR + Ticks(10), true);
    send(&mut app, SessionMsg::Record { slot });
    at(&mut app, BAR * 2, true);
    assert!(app.is_recording());
    mpe_note(&mut app, tid, BAR * 2 + Ticks(100), BAR * 2 + Ticks(500));
    at(&mut app, BAR * 2 + Ticks(600), true);
    send(&mut app, SessionMsg::StopRecording { slot });
    at(&mut app, BAR * 3, true);
    let c = app.project().slot_clip(slot).unwrap();
    let added: Vec<_> = c.clip.notes.iter().filter(|n| n.pitch == 64).collect();
    assert_eq!(added.len(), 1);
    assert!(added[0].expr.is_some(), "its expression");
}

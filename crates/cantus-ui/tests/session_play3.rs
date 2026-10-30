//! The conductor's second round (the integrator's review of the timing round): a quantised
//! launch whose compile overran plays on the bar it meant, caught up; the compile estimate
//! is a high percentile of the compiles that matter; a launch from stop that was stopped
//! before the engine played gives way to the next launch from stop.

use cantus_core::session::*;
use cantus_core::*;
use cantus_engine::messages::SessionMsg;
use cantus_engine::{EngineHandle, EngineMessage};
use cantus_ui::session_play::{Conductor, FIRST_COMPILE_S};
use cantus_ui::CantusApp;

const BAR: Ticks = Ticks::beats(4);

fn app() -> (CantusApp, SlotRef) {
    let mut p = Project::new("live");
    let t = p.make_track("Keys");
    let tid = t.id;
    p.tracks.push(t);
    let scene = SceneId(p.alloc_id());
    p.session.scenes.push(Scene::new(scene, ""));
    let mut c = p.make_clip("Riff", Ticks::ZERO, BAR);
    c.content_length = Some(BAR);
    for b in 0..4 {
        let n = p.make_note(Ticks::beats(b), Ticks::QUARTER, 60 + b as u8, 100);
        c.notes.push(n);
    }
    p.tracks[0].slots.push(Slot {
        scene,
        clip: Some(SessionClip::new(c)),
        stop: true,
    });
    (
        CantusApp::from_parts(Some(p), EngineHandle::dummy(), false),
        SlotRef::new(tid, scene),
    )
}

fn launch(app: &mut CantusApp, slot: SlotRef) {
    app.engine.send(EngineMessage::Session(SessionMsg::Launch {
        slot,
        velocity: 1.0,
    }));
    app.sync_engine();
}

fn at(app: &mut CantusApp, tick: Ticks, playing: bool) {
    app.engine.set_heard_for_tests(tick, playing);
    app.sync_engine();
}

/// N1: a bar-quantised launch pressed a beat before the bar, whose compile overran past it
/// (planned again two beats on), plays on that bar caught up — not a bar later.
#[test]
fn an_overrun_compile_plays_on_the_bar_it_meant_caught_up() {
    let (mut app, slot) = app();
    at(&mut app, BAR - Ticks::beats(1), true);
    app.set_compile_delay_for_tests(std::time::Duration::from_secs(1));
    launch(&mut app, slot);
    let x = app.perform.plan().perf.played[0];
    assert!(x.at > BAR && x.at < BAR * 2, "{:?}", x.at);
    assert_eq!(x.origin, BAR, "in step with the bar it meant");
    assert_eq!(x.from, x.at - BAR);
}

/// N4: the estimate is the 95th percentile of the last compiles — one slow compile among
/// 64 does not set the lead for the next 64 — and a launch from stop's full compile is
/// not one of them.
#[test]
fn the_compile_estimate_is_a_high_percentile_of_the_compiles_that_matter() {
    let mut c = Conductor::default();
    assert_eq!(c.compile_s(), FIRST_COMPILE_S);
    c.compiled(0.058);
    for _ in 0..63 {
        c.compiled(0.003);
    }
    assert!(c.compile_s() < 0.01, "{}", c.compile_s());
    // Few: their worst.
    let mut few = Conductor::default();
    few.compiled(0.003);
    few.compiled(0.058);
    assert_eq!(few.compile_s(), 0.058);
    // A launch from stop compiles the projection in full: not counted.
    let (mut app, slot) = app();
    launch(&mut app, slot);
    assert!(app.perform.active());
    assert_eq!(app.perform.compile_s(), FIRST_COMPILE_S);
}

/// N7: a launch from stop, the transport stopped before any frame saw it play: the next
/// launch from stop starts the transport again, at once, from the playhead.
#[test]
fn a_launch_from_stop_stopped_before_it_played_gives_way_to_the_next() {
    let (mut app, slot) = app();
    launch(&mut app, slot);
    assert!(app.perform.active());
    // Stopped before the engine said it plays (it never will: the dummy engine).
    app.engine.stop();
    app.sync_engine();
    assert!(!app.perform.active(), "the performance ended with the stop");
    let position = app.engine.position();
    launch(&mut app, slot);
    let x = app.perform.plan().perf.played[0];
    assert_eq!(
        x.at, position,
        "at once, from the playhead: the transport is started"
    );
}

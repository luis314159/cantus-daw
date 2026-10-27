//! Write, Touch and Latch in the application (ADR 0068): a pass from play to stop, driven
//! frame by frame with the dummy engine's heard position, the hand's moves as the commands
//! a drag runs — and its commit, one undo step.

mod support;
use cantus_core::*;
use cantus_ui::CantusApp;
use support::*;

/// A beat at the starter project's tempo, in ticks.
const BEAT: i64 = PPQ;

fn track(a: &CantusApp) -> TrackId {
    a.project().tracks[0].id
}

fn pan_clips(a: &CantusApp, t: TrackId) -> Vec<Clip> {
    a.project()
        .tracks
        .iter()
        .flat_map(|tr| tr.clips.iter())
        .filter(|c| {
            c.automation
                .as_ref()
                .is_some_and(|x| x.target == AutoTarget::TrackPan(t))
        })
        .cloned()
        .collect()
}

/// Play from `from` to `to` in frames of `step` ticks; `hand(tick)` says where the hand
/// puts the pan at that frame (`None`: not touching it).
fn play(a: &mut CantusApp, from: i64, to: i64, step: i64, hand: impl Fn(i64) -> Option<f32>) {
    let t = track(a);
    let key = MergeKey::of(&("test-pan", t.0));
    let mut holding = false;
    let mut tick = from;
    while tick <= to {
        a.engine.set_heard_for_tests(Ticks(tick), true);
        a.auto_write_frame();
        match hand(tick) {
            Some(pan) => {
                a.exec_merging(Command::SetTrackPan { id: t, pan }, key);
                holding = true;
            }
            None if holding => {
                a.end_gesture();
                holding = false;
            }
            None => {}
        }
        a.auto_write_settle();
        tick += step;
    }
    if holding {
        a.end_gesture();
    }
}

fn stop(a: &mut CantusApp, at: i64) {
    a.engine.set_heard_for_tests(Ticks(at), false);
    a.auto_write_frame();
}

fn armed_app(mode: AutoMode) -> CantusApp {
    let mut a = app(cantus_ui::starter_project());
    let t = track(&a);
    assert!(a.exec(Command::SetTrackAutoMode { track: t, mode }));
    a.set_auto_armed(true);
    a
}

/// Read: a hand on the pan changes the project's value (FL's override) and writes nothing.
#[test]
fn read_writes_no_automation() {
    let mut a = armed_app(AutoMode::Read);
    let t = track(&a);
    let steps = a.bus.history().len();
    play(&mut a, 0, 8 * BEAT, 40, |tick| {
        (tick > 2 * BEAT && tick < 3 * BEAT).then_some(-0.5)
    });
    stop(&mut a, 8 * BEAT);
    assert!(pan_clips(&a, t).is_empty());
    assert_eq!(a.project().track(t).unwrap().pan, -0.5);
    assert_eq!(a.bus.history().len(), steps + 1, "the drag's own step");
}

/// Disarmed, a track in Touch writes nothing either.
#[test]
fn disarmed_modes_write_nothing() {
    let mut a = armed_app(AutoMode::Touch);
    a.set_auto_armed(false);
    let t = track(&a);
    play(&mut a, 0, 4 * BEAT, 40, |tick| (tick < BEAT).then_some(0.7));
    stop(&mut a, 4 * BEAT);
    assert!(pan_clips(&a, t).is_empty());
}

/// Touch: written while held; let go, it glides back over the touch glide (250 ms: half a
/// beat at 120 BPM) to what played before, the knob is put back, and the pass is one undo
/// step with the drag folded in.
#[test]
fn touch_writes_while_held_and_glides_back() {
    let mut a = armed_app(AutoMode::Touch);
    let t = track(&a);
    let before = a.project().track(t).unwrap().pan;
    let steps = a.bus.history().len();
    play(&mut a, 0, 8 * BEAT, 40, |tick| {
        (2 * BEAT..4 * BEAT).contains(&tick).then_some(0.8)
    });
    stop(&mut a, 8 * BEAT);
    let clips = pan_clips(&a, t);
    assert_eq!(clips.len(), 1, "{clips:?}");
    let c = &clips[0];
    assert_eq!(c.start, Ticks(2 * BEAT));
    let bpm = a.project().tempo_bpm;
    let glide = (0.25 * bpm / 60.0 * PPQ as f64).round() as i64;
    assert!(
        (c.end().0 - (4 * BEAT + glide)).abs() <= 40,
        "ends where the glide does: {:?}",
        c.end()
    );
    let lane =
        cantus_core::automation::compile::lane_for(a.project(), &AutoTarget::TrackPan(t)).unwrap();
    assert!((lane.value_at((3 * BEAT) as f64) - 0.8).abs() < 1e-3);
    assert!(
        (lane.value_at((6 * BEAT) as f64) - before).abs() < 1e-3,
        "back"
    );
    assert_eq!(
        a.project().track(t).unwrap().pan,
        before,
        "the knob put back"
    );
    assert_eq!(
        a.bus.history().len(),
        steps + 1,
        "one step: the pass and its drag"
    );
    a.undo();
    assert!(pan_clips(&a, t).is_empty());
    assert_eq!(a.project().track(t).unwrap().pan, before);
}

/// Latch: from the first touch to the stop, holding the last value; the track stays in
/// Latch.
#[test]
fn latch_holds_until_the_stop() {
    let mut a = armed_app(AutoMode::Latch);
    let t = track(&a);
    play(&mut a, 0, 6 * BEAT, 40, |tick| {
        (BEAT..2 * BEAT).contains(&tick).then_some(-0.6)
    });
    stop(&mut a, 6 * BEAT);
    let clips = pan_clips(&a, t);
    assert_eq!(clips.len(), 1);
    assert_eq!(clips[0].start, Ticks(BEAT));
    assert!((clips[0].end().0 - 6 * BEAT).abs() <= 40);
    let lane =
        cantus_core::automation::compile::lane_for(a.project(), &AutoTarget::TrackPan(t)).unwrap();
    assert!((lane.value_at((5 * BEAT) as f64) + 0.6).abs() < 1e-3);
    assert_eq!(a.project().track(t).unwrap().auto_mode, AutoMode::Latch);
}

/// Write: an automated target holds where it was heard when the pass began, over the whole
/// pass, untouched; the track comes out in Touch, in the same undo step.
#[test]
fn write_overwrites_what_the_track_automates_and_turns_touch() {
    let mut a = armed_app(AutoMode::Write);
    let t = track(&a);
    let mut c = a.bus.new_clip("pan", Ticks::ZERO, Ticks(16 * BEAT));
    c.automation = Some(Automation::new(
        AutoTarget::TrackPan(t),
        vec![
            AutoPoint::new(Ticks::ZERO, 0.2),
            AutoPoint::new(Ticks(16 * BEAT), 0.9),
        ],
    ));
    assert!(a.exec(Command::InsertClip {
        track: t,
        clip: Box::new(c),
    }));
    let steps = a.bus.history().len();
    play(&mut a, 4 * BEAT, 8 * BEAT, 40, |_| None);
    stop(&mut a, 8 * BEAT);
    let lane =
        cantus_core::automation::compile::lane_for(a.project(), &AutoTarget::TrackPan(t)).unwrap();
    let held = lane.pos_at((4 * BEAT) as f64).unwrap();
    assert!((lane.pos_at((7 * BEAT) as f64).unwrap() - held).abs() < 1e-4);
    assert!(
        lane.pos_at((12 * BEAT) as f64).unwrap() > held + 0.1,
        "the old curve after"
    );
    assert_eq!(a.project().track(t).unwrap().auto_mode, AutoMode::Touch);
    assert_eq!(a.bus.history().len(), steps + 1);
}

/// While a hand holds an automated control, the engine hears the hand: the snapshot's lane
/// drives nothing.
#[test]
fn a_held_control_is_heard_not_its_curve() {
    let mut a = armed_app(AutoMode::Touch);
    let t = track(&a);
    let mut c = a.bus.new_clip("pan", Ticks::ZERO, Ticks(16 * BEAT));
    c.automation = Some(Automation::new(
        AutoTarget::TrackPan(t),
        vec![AutoPoint::new(Ticks::ZERO, 0.2)],
    ));
    assert!(a.exec(Command::InsertClip {
        track: t,
        clip: Box::new(c),
    }));
    let key = MergeKey::of(&"hold");
    a.engine.set_heard_for_tests(Ticks(BEAT), true);
    a.auto_write_frame();
    a.exec_merging(Command::SetTrackPan { id: t, pan: 0.4 }, key);
    a.engine.set_heard_for_tests(Ticks(BEAT + 40), true);
    a.auto_write_frame();
    a.auto_write_settle();
    let mut snap = std::sync::Arc::new(Snapshot::compile(a.project(), 1));
    a.latch_recorded(&mut snap);
    let lane = snap
        .automation
        .iter()
        .find(|l| l.target == AutoTarget::TrackPan(t))
        .unwrap();
    assert!(lane.segs.is_empty(), "the hand is heard");
    assert!(a.latched(&AutoTarget::TrackPan(t)));
}

/// The top bar's *All tracks*: every track and the master in one mode, one undo step.
#[test]
fn every_track_at_once() {
    let mut a = app(cantus_ui::starter_project());
    let steps = a.bus.history().len();
    assert!(a.set_all_auto_modes(AutoMode::Latch));
    assert!(a
        .project()
        .tracks
        .iter()
        .all(|t| t.auto_mode == AutoMode::Latch));
    assert_eq!(a.project().master.auto_mode, AutoMode::Latch);
    assert_eq!(a.bus.history().len(), steps + 1);
    assert!(!a.set_all_auto_modes(AutoMode::Latch), "nothing to change");
}

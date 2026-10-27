//! Write, Touch and Latch as pure data (ADR 0068): what a pass writes, what its commit
//! makes of it, and what the engine hears while it runs.

use crate::automation::compile::{compile, lane_for};
use crate::automation::range::range_of;
use crate::automation::write::{overlay, AutoMode, Live, Writer};
use crate::automation::write_commit::{commit, heard_pos, live_lanes, Commit};
use crate::automation::{AutoPoint, AutoTarget, Automation};
use crate::command::Command;
use crate::model::{Project, TrackId};
use crate::time::Ticks;

const GLIDE: i64 = 240;

fn none(_: &AutoTarget) -> bool {
    false
}

fn glide(t: i64) -> i64 {
    t + GLIDE
}

/// A project with one track whose level the tests write, and a clip on it holding the
/// level at `level` (0…1 of the fader) over four bars, when given.
fn project(level: Option<f32>) -> (Project, TrackId, AutoTarget) {
    let mut p = Project::empty("w");
    let t = p.make_track("Lead");
    let id = t.id;
    p.tracks.push(t);
    let target = AutoTarget::TrackGain(id);
    if let Some(v) = level {
        let mut c = p.make_clip("ride", Ticks::ZERO, Ticks::beats(16));
        c.automation = Some(Automation::new(
            target.clone(),
            vec![AutoPoint::new(Ticks::ZERO, v)],
        ));
        p.tracks[0].clips.push(c);
    }
    (p, id, target)
}

/// Run `cmds` on a copy of `p`.
fn applied(p: &Project, cmds: &[Command]) -> Project {
    let mut q = p.clone();
    for c in cmds {
        c.clone().apply(&mut q).expect("a commit applies");
    }
    q
}

fn committed(p: &Project, w: &Writer, host: TrackId) -> (Project, usize) {
    let mut next = 10_000;
    let mut ids = || {
        next += 1;
        next
    };
    let hostf = move |_: &AutoTarget| Some(host);
    let name = |_: &AutoTarget| "Lead · Level".to_string();
    let mut c = Commit {
        before: p,
        host: &hostf,
        name: &name,
        ids: &mut ids,
        wrote: &[host],
        master_wrote: false,
    };
    let (cmds, n) = commit(p, w, &mut c);
    (applied(p, &cmds), n)
}

/// Read mode: a hand on the control writes nothing, and a pass with no gesture changes no
/// automation (the null case).
#[test]
fn read_writes_nothing() {
    let (p, id, target) = project(Some(0.4));
    let mut w = Writer::new(0, None);
    w.value(target.clone(), AutoMode::Read, 100, 0.9);
    for t in (0..3840).step_by(40) {
        w.frame(t, &none, &glide, &|_, _| 0.4);
    }
    w.finish(3840);
    assert!(w.is_empty());
    let (q, n) = committed(&p, &w, id);
    assert_eq!(n, 0);
    assert_eq!(q, p, "no automation changed");
    // A Touch pass nobody touched writes nothing either.
    let mut w = Writer::new(0, None);
    for t in (0..3840).step_by(40) {
        w.frame(t, &none, &glide, &|_, _| 0.4);
    }
    w.finish(3840);
    assert!(w.is_empty());
}

/// A written curve played back reproduces the gesture within the stated tolerance: 1/500
/// of the control's travel (the bake's thinning), which on the fader near 0 dB is
/// measured here.
#[test]
fn a_written_curve_reproduces_the_gesture() {
    let (p, id, target) = project(None);
    let mut w = Writer::new(0, None);
    let held = |_: &AutoTarget| true;
    // A slow sine ride over two bars, sampled as a hand's frames are (every 16 ticks).
    let pos = |t: i64| 0.7 + 0.2 * cantus_dsp::math::sin(t as f32 / 1920.0 * std::f32::consts::PI);
    for t in (0..=7680).step_by(16) {
        w.value(target.clone(), AutoMode::Latch, t, pos(t));
        w.frame(t, &held, &glide, &|_, _| 0.0);
    }
    w.finish(7680);
    let (q, n) = committed(&p, &w, id);
    assert_eq!(n, 1);
    let lane = lane_for(&q, &target).expect("a lane");
    let range = range_of(&q, &target).unwrap();
    let mut worst_pos = 0f32;
    let mut worst_db = 0f32;
    for t in (0..7680).step_by(7) {
        let got = lane.pos_at(t as f64).unwrap();
        worst_pos = worst_pos.max((got - pos(t)).abs());
        worst_db = worst_db.max((range.value(got) - range.value(pos(t))).abs());
    }
    assert!(worst_pos <= 0.002 + 1e-4, "{worst_pos}");
    // The fader's taper around 0 dB: the same tolerance in decibels.
    assert!(worst_db < 0.15, "{worst_db} dB");
    eprintln!("written curve: worst {worst_pos:.5} of the travel, {worst_db:.4} dB");
}

/// Touch: written while held; let go, it glides back to the curve already there over the
/// glide time, and the old curve plays on after it.
#[test]
fn touch_returns_to_the_old_curve() {
    let (p, id, target) = project(Some(0.4));
    let mut w = Writer::new(0, None);
    let old = |t: &AutoTarget, at: i64| heard_pos(&p, t, at, 0.0);
    let mut holding = true;
    for t in (0..=3840).step_by(16) {
        if (960..1920).contains(&t) {
            w.value(target.clone(), AutoMode::Touch, t, 0.9);
        }
        holding = (960..1920).contains(&t);
        let h = holding;
        w.frame(t, &move |_| h, &glide, &old);
    }
    assert!(!holding);
    w.finish(3840);
    let (q, _) = committed(&p, &w, id);
    let lane = lane_for(&q, &target).unwrap();
    assert!(
        (lane.pos_at(500.0).unwrap() - 0.4).abs() < 1e-6,
        "before the touch"
    );
    assert!(
        (lane.pos_at(1500.0).unwrap() - 0.9).abs() < 1e-3,
        "while held"
    );
    // Halfway through the glide, halfway back.
    let mid = lane.pos_at((1920 + GLIDE / 2) as f64).unwrap();
    assert!((mid - 0.65).abs() < 0.02, "{mid}");
    assert!((lane.pos_at((1920 + GLIDE + 1) as f64).unwrap() - 0.4).abs() < 1e-6);
    assert!(
        (lane.pos_at(3000.0).unwrap() - 0.4).abs() < 1e-6,
        "the old curve on"
    );
}

/// Latch: from the first touch until the stop, holding the last value; the project's own
/// value is put back to what it was.
#[test]
fn latch_holds_until_the_stop() {
    let (mut p, id, target) = project(None);
    p.tracks[0].gain_db = -6.0;
    let before = p.clone();
    let mut w = Writer::new(0, None);
    for t in (0..=3840).step_by(16) {
        let held = (960..1100).contains(&t);
        if held {
            w.value(target.clone(), AutoMode::Latch, t, 0.95);
        }
        w.frame(t, &move |_| held, &glide, &|_, _| 0.0);
    }
    w.finish(3840);
    // The hand moved the project's own value meanwhile (FL's override).
    let mut now = p.clone();
    now.tracks[0].gain_db = 3.0;
    let mut next = 50_000;
    let mut ids = || {
        next += 1;
        next
    };
    let hostf = move |_: &AutoTarget| Some(id);
    let name = |_: &AutoTarget| "x".to_string();
    let mut c = Commit {
        before: &before,
        host: &hostf,
        name: &name,
        ids: &mut ids,
        wrote: &[],
        master_wrote: false,
    };
    let (cmds, n) = commit(&now, &w, &mut c);
    assert_eq!(n, 1);
    let q = applied(&now, &cmds);
    assert_eq!(q.tracks[0].gain_db, -6.0, "the knob put back");
    let lane = lane_for(&q, &target).unwrap();
    assert_eq!(lane.pos_at(3000.0), Some(0.95));
    assert_eq!(
        lane.value_at(500.0),
        -6.0,
        "before the clip: the project's value"
    );
    // After the clip: it holds where it left off (ADR 0027 §4).
    assert_eq!(lane.pos_at(10_000.0), Some(0.95));
}

/// Write: every armed target holds where it was when the pass began until the hand moves
/// it, from start to stop; the track comes out in Touch.
#[test]
fn write_overwrites_and_switches_to_touch() {
    let (mut p, id, target) = project(Some(0.4));
    p.tracks[0].auto_mode = AutoMode::Write;
    let mut w = Writer::new(960, None);
    w.arm(target.clone(), 960, 0.6);
    for t in (960..=2880).step_by(16) {
        w.frame(t, &none, &glide, &|_, _| 0.4);
    }
    w.finish(2880);
    let (q, n) = committed(&p, &w, id);
    assert_eq!(n, 1);
    let lane = lane_for(&q, &target).unwrap();
    assert_eq!(lane.pos_at(500.0), Some(0.4));
    assert_eq!(lane.pos_at(2000.0), Some(0.6));
    assert_eq!(
        lane.pos_at(3000.0),
        Some(0.4),
        "the old clip comes back after"
    );
    assert_eq!(q.tracks[0].auto_mode, AutoMode::Touch);
}

/// Inside a loop the last pass wins where it writes; a jump ends every run.
#[test]
fn the_last_loop_pass_wins() {
    let target = AutoTarget::MasterGain;
    let mut w = Writer::new(0, None);
    let held = |_: &AutoTarget| true;
    for t in (0..=960).step_by(16) {
        w.value(target.clone(), AutoMode::Latch, t, 0.2);
        w.frame(t, &held, &glide, &|_, _| 0.0);
    }
    // The loop came round: the second pass writes 0.8 over 0…480 only, then lets go.
    w.jump(0);
    for t in (0..=480).step_by(16) {
        w.value(target.clone(), AutoMode::Touch, t, 0.8);
        w.frame(t, &held, &glide, &|_, _| 0.0);
    }
    let runs = &w.targets[&target].runs;
    assert_eq!(runs.len(), 2);
    let laid = overlay(runs);
    let at = |t: i64| {
        laid.iter()
            .flat_map(|r| r.iter())
            .rfind(|s| s.0 <= t)
            .map(|s| s.1)
    };
    assert_eq!(at(400), Some(0.8));
    assert_eq!(at(900), Some(0.2));
}

/// With punch on, nothing is written outside the punch range.
#[test]
fn punch_bounds_the_writing() {
    let target = AutoTarget::MasterPan;
    let mut w = Writer::new(0, Some((960, 1920)));
    let held = |_: &AutoTarget| true;
    for t in (0..=2880).step_by(16) {
        w.value(target.clone(), AutoMode::Latch, t, 0.3);
        w.frame(t, &held, &glide, &|_, _| 0.5);
    }
    w.finish(2880);
    let laid = overlay(&w.targets[&target].runs);
    let all: Vec<i64> = laid.iter().flat_map(|r| r.iter().map(|s| s.0)).collect();
    assert!(all.iter().all(|t| (960..=1920).contains(t)), "{all:?}");
    assert_eq!(all.first(), Some(&960));
    assert_eq!(all.last(), Some(&1920));
}

/// While a pass writes: a held control's lane drives nothing (the hand is heard), a
/// latched one holds, a released touch glides back along the old curve.
#[test]
fn the_engine_hears_what_is_written() {
    let (p, _, target) = project(Some(0.4));
    let lanes = compile(&p);
    let mut w = Writer::new(0, None);
    w.value(target.clone(), AutoMode::Touch, 100, 0.9);
    w.frame(100, &|_| true, &glide, &|_, _| 0.4);
    assert_eq!(w.live(&target), Live::Hand);
    let mut l = lanes.clone();
    live_lanes(&mut l, &w, &p, &p, w.at.unwrap());
    assert!(l[0].segs.is_empty());
    // Let go at 200: the glide.
    w.frame(200, &none, &glide, &|_, _| 0.4);
    let mut l = lanes.clone();
    live_lanes(&mut l, &w, &p, &p, w.at.unwrap());
    assert!((l[0].pos_at(200.0).unwrap() - 0.9).abs() < 1e-6);
    assert!((l[0].pos_at(320.0).unwrap() - 0.65).abs() < 1e-3);
    assert!((l[0].pos_at(1000.0).unwrap() - 0.4).abs() < 1e-6);
    // Latched: it holds.
    let mut w = Writer::new(0, None);
    w.value(target.clone(), AutoMode::Latch, 100, 0.7);
    w.frame(100, &none, &glide, &|_, _| 0.4);
    let mut l = lanes;
    live_lanes(&mut l, &w, &p, &p, w.at.unwrap());
    assert_eq!(l[0].pos_at(5000.0), Some(0.7));
}

/// Review #2: a controller's message is told after the frame's own sample (at its own,
/// earlier time): it lands in order, never dropped; one heard before a loop's wrap but told
/// after it goes into the lap before the wrap, not the new one.
#[test]
fn late_values_land_in_order_and_before_a_jump() {
    let target = AutoTarget::MasterPan;
    let mut w = Writer::new(0, None);
    let held = |_: &AutoTarget| true;
    w.value(target.clone(), AutoMode::Latch, 0, 0.1);
    w.frame(100, &held, &glide, &|_, _| 0.0);
    // A message heard at 80, told after the frame at 100.
    w.now = Some(120);
    w.value(target.clone(), AutoMode::Latch, 80, 0.6);
    let run = &w.targets[&target].runs[0];
    assert!(run.contains(&(80, 0.6)), "{run:?}");
    assert!(run.windows(2).all(|p| p[0].0 < p[1].0), "{run:?}");
    // The loop wrapped from 1 000 to 0; a message heard at 990 is told after the wrap.
    w.frame(1000, &held, &glide, &|_, _| 0.0);
    w.jump(0);
    w.now = Some(10);
    w.value(target.clone(), AutoMode::Latch, 990, 0.9);
    let runs = &w.targets[&target].runs;
    assert_eq!(runs.len(), 2);
    assert!(
        runs[0].contains(&(990, 0.9)),
        "the lap before: {:?}",
        runs[0]
    );
    assert!(
        runs[1].iter().all(|s| s.0 < 990),
        "not the new lap: {:?}",
        runs[1]
    );
}

/// Review #10: a touch let go just before the punch-out glides to the punch-out, ending
/// where the glide had got to — not a step.
#[test]
fn a_glide_past_the_punch_out_ends_there() {
    let target = AutoTarget::MasterPan;
    let mut w = Writer::new(0, Some((0, 1000)));
    w.value(target.clone(), AutoMode::Touch, 800, 1.0);
    w.frame(800, &|_| true, &glide, &|_, _| 0.0);
    w.frame(880, &none, &glide, &|_, _| 0.0);
    let run = &w.targets[&target].runs[0];
    let end = *run.last().unwrap();
    assert_eq!(end.0, 1000);
    let want = 1.0 - (1000.0 - 880.0) / GLIDE as f32;
    assert!((end.1 - want).abs() < 1e-4, "{run:?}");
}

/// Review #11: a hosted parameter with no curve, let go in Touch, glides back in what the
/// engine hears as it does in what is written.
#[test]
fn a_hosted_parameter_glides_back_live() {
    use crate::external::ExternalPlugin;
    use crate::PluginFormat;
    let mut p = Project::new("h");
    let t = p.make_track("T");
    let strip = t.strip.expect("a strip");
    p.tracks.push(t);
    let mut slot = p.make_effect(crate::EffectKind::External);
    slot.spec.external = Some(Box::new(ExternalPlugin::new(PluginFormat::Clap, "x", "X")));
    let effect = slot.id;
    p.strip_mut(strip).unwrap().effects.push(slot);
    let target = AutoTarget::Plugin { effect, id: 3 };
    let mut w = Writer::new(0, None);
    w.value(target.clone(), AutoMode::Touch, 100, 0.9);
    w.frame(100, &|_| true, &glide, &|_, _| 0.2);
    w.frame(200, &none, &glide, &|_, _| 0.2);
    let mut lanes = compile(&p);
    live_lanes(&mut lanes, &w, &p, &p, 250);
    let lane = lanes
        .iter()
        .find(|l| l.target == target)
        .expect("a lane for the glide");
    assert!((lane.pos_at(200.0).unwrap() - 0.9).abs() < 1e-4);
    assert!((lane.pos_at(1000.0).unwrap() - 0.2).abs() < 1e-4);
}

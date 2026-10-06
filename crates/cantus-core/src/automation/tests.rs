use super::compile::{compile, lane_for};
use super::range::{fader_db_at, fader_pos_of, AutoRange, GAIN};
use super::*;
use crate::model::Project;
use crate::time::Ticks;
use crate::{Command, Snapshot};

fn pt(tick: i64, value: f32) -> AutoPoint {
    AutoPoint::new(Ticks(tick), value)
}

/// A project with one track whose strip's gain is automated by `clips`.
fn project_with(clips: Vec<(i64, i64, Vec<AutoPoint>)>) -> (Project, AutoTarget) {
    let mut p = Project::default();
    let mut t = p.make_track("t");
    let strip = t.strip.unwrap();
    let target = AutoTarget::StripGain(strip);
    for (start, len, pts) in clips {
        let c = p.make_automation_clip("a", target.clone(), Ticks(start), Ticks(len), pts);
        t.clips.push(c);
    }
    p.tracks.push(t);
    (p, target)
}

#[test]
fn shapes_start_at_zero_end_at_one_and_bend_as_told() {
    for shape in Shape::ALL {
        for t in [-1.0, -0.3, 0.0, 0.5, 1.0] {
            let a = shape.at(t, 0.0);
            assert_eq!(a, 0.0, "{shape:?} {t} starts at the first value");
            let v = shape.at(t, 0.37);
            assert!((0.0..=1.0).contains(&v), "{shape:?} {t}: {v}");
        }
    }
    assert_eq!(Shape::Curve.at(0.0, 0.25), 0.25, "no tension is a line");
    assert!(Shape::Curve.at(0.8, 0.5) < 0.2, "positive tension waits");
    assert!(Shape::Curve.at(-0.8, 0.5) > 0.8, "negative tension rushes");
    assert_eq!(
        Shape::Hold.at(0.0, 0.99),
        0.0,
        "hold stays until the next point"
    );
    let s = Shape::SCurve.at(0.6, 0.5);
    assert!((s - 0.5).abs() < 1e-6, "an S passes through its middle");
    assert!(Shape::SCurve.at(0.6, 0.2) < 0.2 && Shape::SCurve.at(0.6, 0.8) > 0.8);
    assert_eq!(
        Shape::Stairs.at(0.0, 0.99),
        1.0,
        "the last stair is the next value"
    );
}

#[test]
fn a_clip_curve_holds_before_its_first_point_and_after_its_last() {
    let a = Automation::new(AutoTarget::MasterGain, vec![pt(100, 0.2), pt(200, 0.6)]);
    assert_eq!(a.value_at(0.0), Some(0.2));
    assert!((a.value_at(150.0).unwrap() - 0.4).abs() < 1e-6);
    assert_eq!(a.value_at(10_000.0), Some(0.6));
    assert_eq!(
        Automation::new(AutoTarget::MasterGain, vec![]).value_at(0.0),
        None
    );
}

#[test]
fn the_fader_taper_is_the_mixers_and_inverts_exactly() {
    assert_eq!(fader_db_at(0.8), 0.0);
    assert_eq!(fader_db_at(0.0), crate::MIN_GAIN_DB);
    for db in [-60.0, -24.0, -6.0, 0.0, 3.0, 12.0] {
        assert!((fader_db_at(fader_pos_of(db)) - db).abs() < 1e-4, "{db}");
    }
    assert_eq!(GAIN.value(0.8), 0.0);
    let log = AutoRange {
        min: 20.0,
        max: 20_000.0,
        taper: Taper::Log,
        stepped: false,
        default: 1000.0,
    };
    assert!((log.value(0.5) - 632.455).abs() < 0.01);
    assert!((log.pos(632.455) - 0.5).abs() < 1e-5);
}

#[test]
fn a_lane_follows_the_window_and_holds_between_clips() {
    // Clip A at 1000 shows its content from 0; clip B at 5000 is trimmed from the left
    // (offset 500), so it starts mid-ramp.
    let (mut p, target) = project_with(vec![
        (1000, 1000, vec![pt(0, 0.0), pt(1000, 1.0)]),
        (5000, 500, vec![pt(0, 0.0), pt(1000, 1.0)]),
    ]);
    p.tracks[0].clips[1].offset = Ticks(500);
    let lane = lane_for(&p, &target).expect("automated");
    let at = |t: f64| lane.pos_at(t);
    assert_eq!(at(999.0), None, "before the first clip: the knob");
    assert_eq!(lane.value_at(500.0), p.strips[0].gain_db);
    assert_eq!(at(1000.0), Some(0.0));
    assert_eq!(at(1500.0), Some(0.5));
    assert_eq!(at(1999.0), Some(0.999));
    assert_eq!(at(3000.0), Some(1.0), "held where clip A ended");
    assert_eq!(at(5000.0), Some(0.5), "B starts where its window shows");
    assert_eq!(at(5250.0), Some(0.75));
    assert_eq!(at(9000.0), Some(1.0), "and holds after");
}

#[test]
fn the_clip_that_started_last_plays_and_the_long_one_comes_back() {
    let (p, target) = project_with(vec![
        (0, 4000, vec![pt(0, 0.1), pt(4000, 0.1)]),
        (1000, 1000, vec![pt(0, 0.9)]),
    ]);
    let lane = lane_for(&p, &target).unwrap();
    assert_eq!(lane.pos_at(500.0), Some(0.1));
    assert_eq!(lane.pos_at(1500.0), Some(0.9));
    assert_eq!(lane.pos_at(2500.0), Some(0.1), "the long clip resumes");
}

#[test]
fn a_looping_clip_repeats_its_curve_and_a_muted_one_is_silent() {
    let (mut p, target) = project_with(vec![(0, 3000, vec![pt(0, 0.0), pt(1000, 1.0)])]);
    p.tracks[0].clips[0].content_length = Some(Ticks(1000));
    let lane = lane_for(&p, &target).unwrap();
    assert_eq!(lane.pos_at(500.0), Some(0.5));
    assert_eq!(lane.pos_at(1500.0), Some(0.5));
    assert_eq!(lane.pos_at(2250.0), Some(0.25));
    p.tracks[0].clips[0].muted = true;
    assert!(lane_for(&p, &target).is_none());
    assert!(compile(&p).is_empty());
}

#[test]
fn split_halves_play_exactly_what_the_whole_did() {
    let pts = vec![pt(0, 0.0), pt(700, 0.8), pt(2000, 0.3)];
    let (whole, target) = project_with(vec![(0, 2000, pts.clone())]);
    let (mut halves, _) = project_with(vec![(0, 2000, pts)]);
    let mut right = halves.tracks[0].clips[0].clone();
    right.id = crate::ClipId(halves.alloc_id());
    right.start = Ticks(900);
    right.length = Ticks(1100);
    right.offset = Ticks(900);
    halves.tracks[0].clips[0].length = Ticks(900);
    halves.tracks[0].clips.push(right);
    let (a, b) = (
        lane_for(&whole, &target).unwrap(),
        lane_for(&halves, &target).unwrap(),
    );
    for t in (0..2600).step_by(37) {
        assert_eq!(a.pos_at(t as f64), b.pos_at(t as f64), "tick {t}");
    }
}

#[test]
fn the_snapshot_carries_lanes_and_no_automation_changes_nothing() {
    let (p, target) = project_with(vec![(0, 960, vec![pt(0, 0.0), pt(960, 1.0)])]);
    let s = Snapshot::compile(&p, 1);
    assert_eq!(s.automation.len(), 1);
    assert_eq!(s.automation[0].target, target);
    assert!(
        s.tracks[0].events.is_empty(),
        "an automation clip fires no notes"
    );
    let plain = Snapshot::compile(&Project::default(), 1);
    assert!(plain.automation.is_empty());
}

#[test]
fn a_target_that_vanished_is_left_out_but_its_clip_stays() {
    let mut p = Project::default();
    let mut t = p.make_track("t");
    let target = AutoTarget::Effect {
        effect: crate::EffectId(424_242),
        key: "mix".into(),
    };
    t.clips
        .push(p.make_automation_clip("a", target, Ticks(0), Ticks(960), vec![pt(0, 0.5)]));
    p.tracks.push(t);
    assert!(compile(&p).is_empty());
    assert_eq!(p.tracks[0].clips.len(), 1);
}

#[test]
fn set_clip_automation_is_undone_exactly_and_refuses_bad_curves() {
    let (mut p, target) = project_with(vec![(0, 960, vec![pt(0, 0.5)])]);
    let before = p.clone();
    let (track, clip) = (p.tracks[0].id, p.tracks[0].clips[0].id);
    let cmd = Command::SetClipAutomation {
        track,
        clip,
        automation: Some(Automation::new(
            target.clone(),
            vec![pt(0, 0.1), pt(480, 0.9)],
        )),
    };
    let inverse = cmd.apply(&mut p).unwrap();
    assert_ne!(p, before);
    inverse.apply(&mut p).unwrap();
    assert_eq!(p, before);
    let bad = Command::SetClipAutomation {
        track,
        clip,
        automation: Some(Automation {
            target,
            points: vec![pt(10, 0.5), pt(0, 2.0)],
        }),
    };
    assert!(bad.apply(&mut p).is_err());
    assert_eq!(p, before);
}

#[test]
fn the_last_tweaked_parameter_comes_from_the_command() {
    let s = crate::StripId(7);
    let cmd = Command::SetStripGain {
        id: s,
        gain_db: -3.0,
    };
    assert_eq!(AutoTarget::of_command(&cmd), Some(AutoTarget::StripGain(s)));
    let fx = Command::SetEffectParam {
        strip: s,
        effect: crate::EffectId(9),
        key: "mix".into(),
        value: 0.5,
    };
    let batch = Command::Batch {
        label_key: "x".into(),
        commands: vec![cmd, fx],
    };
    assert_eq!(
        AutoTarget::of_command(&batch),
        Some(AutoTarget::Effect {
            effect: crate::EffectId(9),
            key: "mix".into()
        })
    );
    assert_eq!(
        AutoTarget::of_command(&Command::SetMetronome { enabled: true }),
        None
    );
}

#[test]
fn articulators_keep_curves_sorted_and_in_range() {
    let mut pts = vec![pt(0, 0.1), pt(100, 0.9), pt(300, 0.4)];
    pts[0].tension = 0.5;
    edit::flip_vertical(&mut pts, &[]);
    assert!((pts[0].value - 0.9).abs() < 1e-6);
    edit::flip_horizontal(&mut pts, Ticks(0), Ticks(300));
    assert_eq!(
        pts.iter().map(|p| p.tick.0).collect::<Vec<_>>(),
        vec![0, 200, 300]
    );
    assert!(
        (pts[2].value - 0.9).abs() < 1e-6,
        "the first point is now last"
    );
    assert_eq!(pts[1].tension, -0.5, "its bend travels the other way");
    edit::scale_values(&mut pts, &[], 0.5, 0.0);
    assert!(pts.iter().all(|p| p.value == 0.5));
    let mut r = vec![pt(0, 0.5), pt(10, 0.5)];
    edit::randomize(&mut r, &[], 0.1, 42);
    let mut again = vec![pt(0, 0.5), pt(10, 0.5)];
    edit::randomize(&mut again, &[], 0.1, 42);
    assert_eq!(r, again, "seeded: undo and redo are exact");
    assert!(r.iter().all(|p| (p.value - 0.5).abs() <= 0.1));
    let lfo = edit::lfo(edit::Lfo::Sine, Ticks(0), Ticks(960), 4, 0.2, 0.8, 1);
    let a = Automation::new(AutoTarget::MasterPan, lfo);
    assert!(a.is_valid());
    assert!(
        (a.value_at(120.0).unwrap() - 0.8).abs() < 1e-6,
        "the first peak"
    );
    assert!((a.value_at(240.0).unwrap() - 0.2).abs() < 1e-6);
}

#[test]
fn a_tension_drag_puts_the_curve_under_the_pointer() {
    for mid in [0.05, 0.2, 0.5, 0.8, 0.95] {
        let t = edit::tension_for_mid(Shape::Curve, mid);
        let got = Shape::Curve.at(t, 0.5);
        assert!((got - mid).abs() < 0.02 || t.abs() == 1.0, "{mid}: {got}");
    }
}

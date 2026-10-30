//! The conductor's timing (ADR 0076 §7, the integrator's review of the engine): chained
//! legato, the launch tolerance, planning on from what was planned, the tempo in force
//! when arrangement record starts, the projection and the commit through a tempo change,
//! frozen tracks, the stall policy.

use super::*;
use crate::session::play::{open_ended, plan, plan_from, project_play, Input, Timed};

fn launch(slot: SlotRef) -> Input {
    Input::Launch {
        slot,
        velocity: 1.0,
    }
}

/// The fixture's keys column: a two-bar loop (scene 0) and two one-bar loops (1, 2).
fn column() -> (CommandBus, Vec<TrackId>, Vec<SceneId>) {
    let (mut bus, tracks, scenes) = fixture();
    for s in [scenes[1], scenes[2]] {
        let c = note_clip(&mut bus, 1);
        bus.execute(
            Command::Session(SessionCommand::InsertSlotClip {
                slot: SlotRef::new(tracks[0], s),
                clip: Box::new(c),
            }),
            Origin::User,
        )
        .unwrap();
    }
    (bus, tracks, scenes)
}

fn set_launch(bus: &mut CommandBus, slot: SlotRef, f: impl FnOnce(&mut LaunchSettings)) {
    let mut l = bus.project().slot_clip(slot).unwrap().launch;
    f(&mut l);
    bus.execute(
        Command::Session(SessionCommand::SetSlotLaunch { slot, launch: l }),
        Origin::User,
    )
    .unwrap();
}

/// Review finding 2: a legato launch after a legato launch keeps the first's phase.
#[test]
fn chained_legato_keeps_the_first_launchs_phase() {
    let (mut bus, tracks, scenes) = column();
    let s = |i: usize| SlotRef::new(tracks[0], scenes[i]);
    for i in [1, 2] {
        set_launch(&mut bus, s(i), |l| {
            l.legato = true;
            l.quantize = Quantize::Quarter;
        });
    }
    let p = plan(
        bus.project(),
        &[
            Timed::at(Ticks::ZERO, launch(s(0))),
            Timed::at(Ticks::beats(5), launch(s(1))),
            Timed::at(Ticks::beats(6), launch(s(2))),
        ],
        BAR * 4,
    );
    assert_eq!(p.perf.played[1].from, Ticks::beats(1));
    assert_eq!(
        p.perf.played[2].from,
        Ticks::beats(2),
        "beat 6 of a one-bar loop"
    );
}

/// D2: a launch pressed just after a boundary plays on it, caught up from where it can
/// take effect; one pressed later than the tolerance waits for the next.
#[test]
fn a_launch_pressed_just_late_plays_on_the_boundary_it_missed() {
    let (bus, tracks, scenes) = column();
    let slot = SlotRef::new(tracks[0], scenes[1]);
    let late = |pressed: Ticks| Timed {
        at: pressed + Ticks(400),
        now: false,
        input: launch(slot),
        pressed,
    };
    let p = plan(bus.project(), &[late(BAR + Ticks(30))], BAR * 4);
    let x = p.perf.played[0];
    assert_eq!(
        (x.at, x.from, x.origin),
        (BAR + Ticks(430), Ticks(430), BAR)
    );
    let p = plan(bus.project(), &[late(BAR + Ticks(300))], BAR * 4);
    assert_eq!(
        p.perf.played[0].at,
        BAR * 2,
        "past the tolerance: the next bar"
    );
}

/// Review finding 7 / D5: an edit while performing changes what comes, never what was
/// planned before the cut — the follow chances, a press added.
#[test]
fn planning_on_keeps_what_was_planned_before_the_cut() {
    let (mut bus, tracks, scenes) = column();
    let a = SlotRef::new(tracks[0], scenes[1]);
    let follow = FollowActions {
        on: true,
        a: FollowAction::Next,
        b: FollowAction::Any,
        chance_a: 50,
        after: FollowAfter::Linked,
        count: 1,
    };
    for s in &scenes {
        bus.execute(
            Command::Session(SessionCommand::SetSlotFollow {
                slot: SlotRef::new(tracks[0], *s),
                follow,
            }),
            Origin::User,
        )
        .unwrap();
    }
    let mut inputs = vec![Timed::at(Ticks::ZERO, launch(a))];
    let before = plan(bus.project(), &inputs, BAR * 16);
    // The chances change; a press comes later.
    for s in &scenes {
        bus.execute(
            Command::Session(SessionCommand::SetSlotFollow {
                slot: SlotRef::new(tracks[0], *s),
                follow: FollowActions {
                    chance_a: 5,
                    ..follow
                },
            }),
            Origin::User,
        )
        .unwrap();
    }
    inputs.push(Timed::at(BAR * 9, Input::StopTrack { track: tracks[0] }));
    let cut = BAR * 6 + Ticks(7);
    let after = plan_from(bus.project(), &inputs, BAR * 16, Some((&before, cut)));
    let upto = |p: &play::Plan| -> Vec<_> {
        p.perf
            .played
            .iter()
            .filter(|x| x.until <= cut)
            .copied()
            .collect()
    };
    assert!(!upto(&before).is_empty());
    assert_eq!(upto(&before), upto(&after));
    let changes = |p: &play::Plan| -> Vec<_> {
        p.changes
            .iter()
            .filter(|(t, _)| *t < cut)
            .copied()
            .collect()
    };
    assert_eq!(changes(&before), changes(&after), "told the same");
    // Planned from scratch with the new chances, the past would have been another.
    assert_ne!(upto(&plan(bus.project(), &inputs, BAR * 16)), upto(&before));
    // The stop came after the cut.
    assert!(after.perf.played.iter().all(|x| x.until <= BAR * 9));
}

/// Review finding 8: arrangement record started after a scene's tempo writes that tempo
/// where it starts; and the projection and the commit agree through a scene's tempo with
/// unwarped audio in the arrangement after it (both keep it in its file).
#[test]
fn the_tempo_in_force_comes_with_a_late_recording_and_projection_agrees_with_commit() {
    let (mut bus, tracks, scenes) = column();
    bus.execute(
        Command::Session(SessionCommand::SetSceneTempo {
            id: scenes[1],
            tempo: Some(132.0),
        }),
        Origin::User,
    )
    .unwrap();
    let sid = crate::model::AudioSourceId(bus.alloc_id());
    bus.execute(
        Command::InsertAudioSource {
            source: crate::audio::AudioSource {
                id: sid,
                name: "pad".into(),
                path: "C:/pad.wav".into(),
                sample_rate: 48_000,
                channels: 2,
                frames: 48_000 * 120,
                sha256: None,
                recorded: false,
            },
        },
        Origin::User,
    )
    .unwrap();
    let mut a = bus.new_clip("pad", BAR * 10, BAR * 4);
    a.offset = BAR;
    a.audio = Some(crate::audio::AudioRegion::of(sid));
    bus.execute(
        Command::InsertClip {
            track: tracks[1],
            clip: Box::new(a),
        },
        Origin::User,
    )
    .unwrap();
    let p = plan(
        bus.project(),
        &[Timed::at(
            BAR * 6,
            Input::LaunchScene {
                scene: scenes[1],
                velocity: 1.0,
            },
        )],
        BAR * 16,
    );
    let song = bus.project().clone();
    let rec = p
        .perf
        .within(BAR * 8, BAR * 12, |s| song.slot_clip(s).cloned());
    assert_eq!(rec.tempos, vec![(BAR * 8, 132.0)]);
    // The whole performance: what the projection plays is what the bus commits.
    let heard = project_play(&song, &p.perf);
    let cmd = edit::commit_performance(&song, &p.perf).unwrap();
    bus.execute(cmd, Origin::User).unwrap();
    let committed = bus.project();
    assert_eq!(heard.tempo_points, committed.tempo_points);
    let audio = |q: &Project| q.track(tracks[1]).unwrap().clips.clone();
    assert_eq!(audio(&heard), audio(committed));
    assert_ne!(
        audio(&heard),
        audio(&song),
        "the tempo moved it in its file"
    );
}

/// A frozen track's clips do not launch, in the planner as in the interface.
#[test]
fn a_frozen_tracks_clips_do_not_launch() {
    let (bus, tracks, scenes) = column();
    let mut p = bus.project().clone();
    p.tracks[0].frozen = Some(crate::freeze::Frozen {
        content: "0".repeat(64),
    });
    let slot = SlotRef::new(tracks[0], scenes[0]);
    let plan = plan(&p, &[Timed::at(Ticks::ZERO, launch(slot))], BAR * 4);
    assert!(plan.perf.played.is_empty() && plan.perf.held.is_empty());
}

/// D4: what loops at the horizon is carried on in the projection (a stalled interface
/// leaves it looping); a clip that ends, ends.
#[test]
fn looping_clips_are_open_ended_past_the_horizon() {
    let (bus, tracks, scenes) = column();
    let slot = SlotRef::new(tracks[0], scenes[0]);
    let p = plan(
        bus.project(),
        &[Timed::at(Ticks::ZERO, launch(slot))],
        BAR * 8,
    );
    let open = open_ended(bus.project(), &p.perf, BAR * 8, 64);
    assert_eq!(open.played[0].until, BAR * 72);
    assert_eq!(open.held[0].2, BAR * 72);
}

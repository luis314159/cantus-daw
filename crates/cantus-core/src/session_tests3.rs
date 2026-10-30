//! Playing the session (ADR 0076 §7): the conductor's plan and the projection.

use super::*;
use crate::session::play::{plan, project_play, Change, Input, Timed};

fn press(at: Ticks, input: Input) -> Timed {
    Timed::at(at, input)
}

fn launch(slot: SlotRef) -> Input {
    Input::Launch {
        slot,
        velocity: 1.0,
    }
}

/// The fixture with a second, one-bar clip under the first (scene 1) and a third (scene
/// 2) on the keys track.
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

#[test]
fn a_launch_waits_for_its_boundary_and_holds_the_track() {
    let (bus, tracks, scenes) = column();
    let a = SlotRef::new(tracks[0], scenes[0]);
    let b = SlotRef::new(tracks[0], scenes[1]);
    let p = plan(
        bus.project(),
        &[
            press(Ticks(10), launch(a)),
            press(BAR * 2 + Ticks(1), launch(b)),
        ],
        BAR * 8,
    );
    assert_eq!(p.effect, vec![Some(BAR), Some(BAR * 3)]);
    let played: Vec<_> = p
        .perf
        .played
        .iter()
        .map(|x| (x.slot, x.at, x.until))
        .collect();
    assert_eq!(played, vec![(a, BAR, BAR * 3), (b, BAR * 3, BAR * 8)]);
    assert_eq!(p.perf.held, vec![(tracks[0], BAR, BAR * 8)]);
    assert!(p.changes.contains(&(
        BAR,
        Change::Arrangement {
            track: tracks[0],
            on: false
        }
    )));
    // Projected: the arrangement of the track there is the session's.
    let q = project_play(bus.project(), &p.perf);
    let keys = q.track(tracks[0]).unwrap();
    assert_eq!(keys.clips.len(), 2);
    assert!(!q.loop_enabled);
    // What the projection plays is what a commit writes, clip for clip.
    let mut c = bus.project().clone();
    edit::commit_performance(bus.project(), &p.perf)
        .unwrap()
        .apply(&mut c)
        .unwrap();
    assert_eq!(c.tracks, q.tracks);
}

#[test]
fn legato_takes_over_where_the_other_clip_was() {
    let (mut bus, tracks, scenes) = column();
    let a = SlotRef::new(tracks[0], scenes[0]);
    let b = SlotRef::new(tracks[0], scenes[1]);
    set_launch(&mut bus, b, |l| {
        l.legato = true;
        l.quantize = Quantize::Quarter;
    });
    let p = plan(
        bus.project(),
        &[
            press(Ticks::ZERO, launch(a)),
            press(BAR + Ticks::beats(1), launch(b)),
        ],
        BAR * 4,
    );
    let second = p.perf.played[1];
    assert_eq!(second.at, BAR + Ticks::beats(1));
    // Five beats in: a one-bar loop is one beat into its second pass.
    assert_eq!(second.from, Ticks::beats(1));
}

#[test]
fn gate_toggle_and_repeat() {
    let (mut bus, tracks, scenes) = column();
    let a = SlotRef::new(tracks[0], scenes[0]);
    set_launch(&mut bus, a, |l| l.mode = LaunchMode::Gate);
    let p = plan(
        bus.project(),
        &[
            press(Ticks::ZERO, launch(a)),
            press(BAR + Ticks(5), Input::Release { slot: a }),
        ],
        BAR * 6,
    );
    assert_eq!(
        p.perf.played[0].until,
        BAR * 2,
        "a gate stops at the release's boundary"
    );
    set_launch(&mut bus, a, |l| l.mode = LaunchMode::Toggle);
    let p = plan(
        bus.project(),
        &[
            press(Ticks::ZERO, launch(a)),
            press(BAR + Ticks(5), launch(a)),
        ],
        BAR * 6,
    );
    assert_eq!(p.perf.played.len(), 1);
    assert_eq!(p.perf.played[0].until, BAR * 2, "pressed again, it stops");
    set_launch(&mut bus, a, |l| {
        l.mode = LaunchMode::Repeat;
        l.quantize = Quantize::Quarter;
    });
    let p = plan(
        bus.project(),
        &[
            press(Ticks::ZERO, launch(a)),
            press(Ticks::beats(3) + Ticks(5), Input::Release { slot: a }),
        ],
        BAR * 2,
    );
    let starts: Vec<Ticks> = p.perf.played.iter().map(|x| x.at).collect();
    assert_eq!(
        starts,
        (0..4).map(Ticks::beats).collect::<Vec<_>>(),
        "a retrigger a beat while held"
    );
    assert_eq!(p.perf.played[3].until, Ticks::beats(4));
}

#[test]
fn follow_actions_are_drawn_from_the_seed_and_where_they_happen() {
    let (mut bus, tracks, scenes) = column();
    let a = SlotRef::new(tracks[0], scenes[0]);
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
    let inputs = [press(Ticks::ZERO, launch(a))];
    let short = plan(bus.project(), &inputs, BAR * 40);
    let long = plan(bus.project(), &inputs, BAR * 80);
    let before = |p: &play::Plan| -> Vec<_> {
        p.perf
            .played
            .iter()
            .filter(|x| x.until < BAR * 40)
            .copied()
            .collect()
    };
    assert_eq!(before(&short), before(&long), "a longer horizon only adds");
    assert!(short.perf.played.len() > 20);
    let slots: std::collections::HashSet<_> =
        short.perf.played.iter().map(|x| x.slot.scene).collect();
    assert_eq!(slots.len(), 3, "the chances reach every clip");
    // Another seed, another performance.
    let mut settings = bus.project().session.settings;
    settings.seed = 7;
    bus.execute(
        Command::Session(SessionCommand::SetSettings { settings }),
        Origin::User,
    )
    .unwrap();
    assert_ne!(plan(bus.project(), &inputs, BAR * 40).perf, short.perf);
    assert!(short
        .changes
        .iter()
        .any(|(_, c)| matches!(c, Change::Followed { .. })));
}

#[test]
fn scenes_set_their_tempo_and_back_to_arrangement_ends_the_hold() {
    let (mut bus, tracks, scenes) = column();
    bus.execute(
        Command::Session(SessionCommand::SetSceneTempo {
            id: scenes[1],
            tempo: Some(140.0),
        }),
        Origin::User,
    )
    .unwrap();
    // The bass slot of scene 1 keeps playing: no stop button there.
    bus.execute(
        Command::Session(SessionCommand::SetSlotStop {
            slot: SlotRef::new(tracks[1], scenes[1]),
            stop: false,
        }),
        Origin::User,
    )
    .unwrap();
    let p = plan(
        bus.project(),
        &[
            press(
                Ticks(3),
                Input::LaunchScene {
                    scene: scenes[1],
                    velocity: 1.0,
                },
            ),
            press(BAR * 3, Input::Back { track: None }),
        ],
        BAR * 6,
    );
    assert_eq!(p.perf.tempos, vec![(BAR, 140.0)]);
    assert_eq!(p.perf.held, vec![(tracks[0], BAR, BAR * 3)]);
    assert_eq!(p.perf.played.len(), 1);
    assert_eq!(p.perf.played[0].until, BAR * 3);
    assert!(p.changes.contains(&(
        BAR * 3,
        Change::Arrangement {
            track: tracks[0],
            on: true
        }
    )));
    // The tempo track the projection plays.
    let q = project_play(bus.project(), &p.perf);
    assert_eq!(q.tempo_map().bpm_at((BAR * 2).0 as f64), 140.0);
    assert_eq!(q.tempo_map().bpm_at(0.0), 120.0);
}

/// Review of delivery 1, #5: an arrangement audio window the session cuts keeps no fade
/// counted from an edge it no longer has: the cut ends with the click-free fade, the part
/// after it starts with one and keeps the window's own fade out.
#[test]
fn a_cut_audio_window_fades_at_the_cut() {
    let (mut bus, tracks, scenes) = column();
    let sid = crate::model::AudioSourceId(bus.alloc_id());
    bus.execute(
        Command::InsertAudioSource {
            source: crate::audio::AudioSource {
                id: sid,
                name: "pad".into(),
                path: "C:/pad.wav".into(),
                sample_rate: 48_000,
                channels: 2,
                frames: 48_000 * 60,
                sha256: None,
                recorded: false,
            },
        },
        Origin::User,
    )
    .unwrap();
    let mut a = bus.new_clip("pad", Ticks::ZERO, BAR * 8);
    let mut region = crate::audio::AudioRegion::of(sid);
    region.fade_out = BAR;
    a.audio = Some(region);
    let id = a.id;
    bus.execute(
        Command::InsertClip {
            track: tracks[0],
            clip: Box::new(a),
        },
        Origin::User,
    )
    .unwrap();
    let p = plan(
        bus.project(),
        &[
            press(BAR, launch(SlotRef::new(tracks[0], scenes[0]))),
            press(BAR * 3, Input::Back { track: None }),
        ],
        BAR * 8,
    );
    let q = project_play(bus.project(), &p.perf);
    let t = q.track(tracks[0]).unwrap();
    let left = t.arranged(id).unwrap();
    assert_eq!(left.end(), BAR);
    assert_eq!(left.audio.as_ref().unwrap().fade_out, edit::CUT_FADE);
    let right = t
        .clips
        .iter()
        .find(|c| c.audio.is_some() && c.start == BAR * 3)
        .unwrap();
    let r = right.audio.as_ref().unwrap();
    assert_eq!((r.fade_in, r.fade_out), (edit::CUT_FADE, BAR));
}

/// A tempo edit keeps an unwarped session audio clip's stretch of its file in seconds.
#[test]
fn a_tempo_edit_keeps_unwarped_session_audio_in_its_seconds() {
    let (mut bus, tracks, scenes) = column();
    let sid = crate::model::AudioSourceId(bus.alloc_id());
    bus.execute(
        Command::InsertAudioSource {
            source: crate::audio::AudioSource {
                id: sid,
                name: "loop".into(),
                path: "C:/loop.wav".into(),
                sample_rate: 48_000,
                channels: 2,
                frames: 48_000 * 8,
                sha256: None,
                recorded: false,
            },
        },
        Origin::User,
    )
    .unwrap();
    let mut c = note_clip(&mut bus, 2);
    c.clip.notes.clear();
    c.clip.audio = Some(crate::audio::AudioRegion::of(sid));
    c.clip.offset = BAR;
    let slot = SlotRef::new(tracks[1], scenes[2]);
    bus.execute(
        Command::Session(SessionCommand::InsertSlotClip {
            slot,
            clip: Box::new(c),
        }),
        Origin::User,
    )
    .unwrap();
    bus.execute(Command::SetTempo { bpm: 60.0 }, Origin::User)
        .unwrap();
    let c = bus.project().slot_clip(slot).unwrap();
    assert_eq!(c.clip.offset, BAR / 2);
    assert_eq!(c.clip.period(), Some(BAR));
    bus.undo().unwrap();
    assert_eq!(bus.project().slot_clip(slot).unwrap().clip.offset, BAR);
}

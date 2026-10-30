//! The session's model and commands (ADR 0076).

use super::edit::{self, Performance, Played};
use super::*;
use crate::automation::{AutoPoint, AutoTarget};
use crate::bus::CommandBus;
use crate::command::{Command, Origin};
use crate::model::{Clip, Project, TrackId};
use crate::tempo::MetreChange;
use crate::time::{Ticks, TimeSignature};

const BAR: Ticks = Ticks::beats(4);

/// A project with two tracks, three scenes and a looping two-bar clip in track 0's first
/// slot (notes on every beat).
fn fixture() -> (CommandBus, Vec<TrackId>, Vec<SceneId>) {
    let mut bus = CommandBus::default();
    let mut tracks = Vec::new();
    for name in ["Keys", "Bass"] {
        let t = bus.new_track(name);
        tracks.push(t.id);
        let index = bus.project().tracks.len();
        bus.execute(
            Command::InsertTrack {
                index,
                track: Box::new(t),
            },
            Origin::User,
        )
        .unwrap();
    }
    let mut scenes = Vec::new();
    for i in 0..3 {
        let id = SceneId(bus.alloc_id());
        scenes.push(id);
        bus.execute(
            Command::Session(SessionCommand::InsertScene {
                index: i,
                scene: Box::new(Scene::new(id, format!("Scene {}", i + 1))),
                slots: Vec::new(),
            }),
            Origin::User,
        )
        .unwrap();
    }
    let clip = note_clip(&mut bus, 2);
    bus.execute(
        Command::Session(SessionCommand::InsertSlotClip {
            slot: SlotRef::new(tracks[0], scenes[0]),
            clip: Box::new(clip),
        }),
        Origin::User,
    )
    .unwrap();
    (bus, tracks, scenes)
}

fn note_clip(bus: &mut CommandBus, bars: i64) -> SessionClip {
    let mut ids = || bus.alloc_id();
    let mut c = edit::new_note_clip("Riff", BAR * bars, &mut ids);
    for b in 0..bars * 4 {
        let mut n = bus.new_note(Ticks::beats(b), Ticks::QUARTER / 2, 60 + b as u8, 100);
        n.start = Ticks::beats(b);
        c.clip.notes.push(n);
    }
    c
}

fn ids_of(bus: &mut CommandBus) -> impl FnMut() -> u64 + '_ {
    move || bus.alloc_id()
}

#[test]
fn a_project_without_a_session_writes_exactly_what_it_wrote() {
    let mut p = Project::default();
    let t = p.make_track("A");
    p.tracks.push(t);
    let text = serde_json::to_string(&p).unwrap();
    assert!(
        !text.contains("session") && !text.contains("slots"),
        "{text}"
    );
    let back: Project = serde_json::from_str(&text).unwrap();
    assert_eq!(back, p);
    assert!(!p.has_session());
}

#[test]
fn a_session_round_trips_through_serde() {
    let (bus, ..) = fixture();
    let text = serde_json::to_string(bus.project()).unwrap();
    let back: Project = serde_json::from_str(&text).unwrap();
    assert_eq!(&back, bus.project());
}

#[test]
fn every_slot_command_undoes_exactly() {
    let (mut bus, tracks, scenes) = fixture();
    let start = bus.project().clone();
    let a = SlotRef::new(tracks[0], scenes[0]);
    let b = SlotRef::new(tracks[1], scenes[2]);
    let launch = LaunchSettings {
        mode: LaunchMode::Gate,
        quantize: Quantize::EighthTriplet,
        legato: true,
        velocity: 0.5,
    };
    let follow = FollowActions {
        on: true,
        a: FollowAction::Next,
        b: FollowAction::Jump(scenes[2]),
        chance_a: 70,
        after: FollowAfter::Time(BAR * 2),
        count: 3,
    };
    let env = Automation::new(
        AutoTarget::TrackGain(tracks[0]),
        vec![AutoPoint::new(Ticks::ZERO, 0.2), AutoPoint::new(BAR, 0.9)],
    );
    let cmds = vec![
        SessionCommand::SetSlotLaunch { slot: a, launch },
        SessionCommand::SetSlotFollow { slot: a, follow },
        SessionCommand::SetSlotEnvelopes {
            slot: a,
            envelopes: vec![env],
        },
        SessionCommand::SetSlotRegion {
            slot: a,
            region: SlotRegion {
                start: Some(Ticks::beats(1)),
                offset: Ticks::beats(2),
                length: BAR,
                loop_length: Some(BAR),
            },
        },
        SessionCommand::SetSlotStop {
            slot: SlotRef::new(tracks[1], scenes[1]),
            stop: false,
        },
        SessionCommand::MoveSlotClip { from: a, to: b },
        SessionCommand::SetSceneName {
            id: scenes[1],
            name: "Chorus".into(),
        },
        SessionCommand::SetSceneColor {
            id: scenes[1],
            color: Some(crate::model::Color::rgb(1, 2, 3)),
        },
        SessionCommand::SetSceneTempo {
            id: scenes[1],
            tempo: Some(128.0),
        },
        SessionCommand::SetSceneMetre {
            id: scenes[1],
            metre: Some(TimeSignature::new(7, 8)),
        },
        SessionCommand::SetSceneFollow {
            id: scenes[1],
            follow,
        },
        SessionCommand::MoveScene {
            id: scenes[0],
            to: 2,
        },
        SessionCommand::SetSettings {
            settings: SessionSettings {
                launch_quantize: Quantize::Bars2,
                record_quantize: RecordQuantize::SixteenthAndTriplet,
                seed: 42,
                fixed_bars: Some(4),
            },
        },
        SessionCommand::RemoveScene { id: scenes[2] },
    ];
    let mut states = vec![start.clone()];
    for c in cmds {
        bus.execute(Command::Session(c.clone()), Origin::User)
            .unwrap_or_else(|e| panic!("{c:?}: {e}"));
        states.push(bus.project().clone());
    }
    for i in (0..states.len() - 1).rev() {
        bus.undo().unwrap();
        let (mut now, mut want) = (bus.project().clone(), states[i].clone());
        now.next_id = 0;
        want.next_id = 0;
        assert_eq!(now, want, "undo to {i}");
    }
    for want in &states[1..] {
        bus.redo().unwrap();
        let (mut now, mut want) = (bus.project().clone(), want.clone());
        now.next_id = 0;
        want.next_id = 0;
        assert_eq!(now, want);
    }
}

#[test]
fn refusals_leave_the_project_as_it_was() {
    let (mut bus, tracks, scenes) = fixture();
    let a = SlotRef::new(tracks[0], scenes[0]);
    let taken = note_clip(&mut bus, 1);
    let mut bad_start = note_clip(&mut bus, 1);
    bad_start.start = Some(BAR * 3);
    let mut linked = note_clip(&mut bus, 1);
    linked.clip.link = Some(crate::model::LinkId(5));
    let before = bus.project().clone();
    let refused = vec![
        SessionCommand::InsertSlotClip {
            slot: a,
            clip: Box::new(taken),
        },
        SessionCommand::InsertSlotClip {
            slot: SlotRef::new(tracks[1], scenes[0]),
            clip: Box::new(bad_start),
        },
        SessionCommand::InsertSlotClip {
            slot: SlotRef::new(tracks[1], scenes[0]),
            clip: Box::new(linked),
        },
        SessionCommand::RemoveSlotClip {
            slot: SlotRef::new(tracks[1], scenes[0]),
        },
        SessionCommand::RemoveSlotClip {
            slot: SlotRef::new(tracks[0], SceneId(999_999)),
        },
        SessionCommand::MoveSlotClip { from: a, to: a },
        SessionCommand::SetSlotLaunch {
            slot: a,
            launch: LaunchSettings {
                velocity: 2.0,
                ..Default::default()
            },
        },
        SessionCommand::SetSlotFollow {
            slot: a,
            follow: FollowActions {
                count: 0,
                ..FollowActions::off()
            },
        },
        SessionCommand::SetSlotRegion {
            slot: a,
            region: SlotRegion {
                start: Some(BAR * 5),
                offset: Ticks::ZERO,
                length: BAR,
                loop_length: None,
            },
        },
        SessionCommand::SetSceneTempo {
            id: scenes[0],
            tempo: Some(5.0),
        },
        SessionCommand::SetSceneMetre {
            id: scenes[0],
            metre: Some(TimeSignature::new(4, 3)),
        },
        SessionCommand::MoveScene {
            id: scenes[0],
            to: 3,
        },
        SessionCommand::SetSettings {
            settings: SessionSettings {
                launch_quantize: Quantize::Global,
                ..Default::default()
            },
        },
        SessionCommand::InsertScene {
            index: 0,
            scene: Box::new(Scene::new(scenes[1], "again")),
            slots: Vec::new(),
        },
    ];
    for c in refused {
        assert!(
            bus.execute(Command::Session(c.clone()), Origin::User)
                .is_err(),
            "{c:?} was taken"
        );
        assert!(bus.project() == &before, "{c:?} changed the project");
    }
}

#[path = "session_tests2.rs"]
mod more;

#[path = "session_tests3.rs"]
mod play_tests;

#[path = "session_tests4.rs"]
mod timing_tests;

#[path = "session_tests5.rs"]
mod second_round_tests;

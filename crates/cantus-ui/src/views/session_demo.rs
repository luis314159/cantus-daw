//! A session to look at (ADR 0077): the shots, the frame budget and the tests build the
//! same grid — `tracks` columns of clips with notes in their own colours, `scenes` rows
//! with names, a tempo and a metre on one, empty slots and a slot without its stop button —
//! and can light it as the engine would ([`showcase`]): playing, queued, stopping,
//! recording, a scene playing, a track back on its arrangement.

use crate::app::CantusApp;
use cantus_core::session::{
    FollowAction, FollowActions, FollowAfter, LaunchMode, Scene, SceneId, SessionClip, Slot,
    SlotRef,
};
use cantus_core::{Project, Ticks, TimeSignature};
use cantus_engine::messages::{ScenePhase, SessionEvent, SlotPhase};

const NAMES: [&str; 8] = ["Drums", "Bass", "Keys", "Lead", "Pad", "Perc", "FX", "Vox"];
const CLIPS: [&str; 8] = [
    "Groove", "Walk", "Chords", "Hook", "Swell", "Shaker", "Riser", "Chop",
];
const SCENES: [&str; 8] = [
    "Intro", "Verse", "Build", "Chorus", "Break", "Verse 2", "Drop", "Outro",
];

/// A project with a session of `tracks` × `scenes`.
pub fn project(tracks: usize, scenes: usize) -> Project {
    let mut p = Project::new("Session");
    let bar = Ticks::beats(4);
    let scene_ids: Vec<SceneId> = (0..scenes).map(|_| SceneId(p.alloc_id())).collect();
    for (j, id) in scene_ids.iter().enumerate() {
        let mut s = Scene::new(*id, SCENES.get(j % SCENES.len()).copied().unwrap_or(""));
        if j == 3 {
            s.tempo = Some(128.0);
            s.metre = Some(TimeSignature {
                numerator: 4,
                denominator: 4,
            });
        }
        if j == 4 {
            s.color = Some(cantus_core::Color::PALETTE[5]);
        }
        p.session.scenes.push(s);
    }
    for i in 0..tracks {
        let mut t = p.make_track(NAMES[i % NAMES.len()]);
        for (j, scene) in scene_ids.iter().enumerate() {
            // A pattern of full and empty slots, as a real set has.
            let filled = (i * 3 + j * 5) % 7 < 4 || j == 0;
            if !filled {
                if i == 2 && j == 5 {
                    t.slots.push(Slot {
                        scene: *scene,
                        clip: None,
                        stop: false,
                    });
                }
                continue;
            }
            let bars = 1 + ((i + j) % 4) as i64;
            let len = bar * bars;
            let mut c = p.make_clip(
                format!("{} {}", CLIPS[i % CLIPS.len()], j + 1),
                Ticks::ZERO,
                len,
            );
            c.content_length = Some(len);
            for n in 0..(4 * bars) {
                let pitch = 48 + ((n * 5 + i as i64 * 7 + j as i64 * 3) % 24) as u8;
                let note = p.make_note(Ticks::beats(n), Ticks(cantus_core::PPQ / 2), pitch, 100);
                c.notes.push(note);
            }
            if (i + j) % 5 == 0 {
                c.color =
                    Some(cantus_core::Color::PALETTE[(i + j) % cantus_core::Color::PALETTE.len()]);
            }
            let mut sc = SessionClip::new(c);
            if i == 1 && j == 1 {
                sc.launch.mode = LaunchMode::Gate;
                sc.launch.legato = true;
                sc.follow = FollowActions {
                    on: true,
                    a: FollowAction::Next,
                    b: FollowAction::Again,
                    chance_a: 70,
                    after: FollowAfter::Linked,
                    count: 2,
                };
            }
            t.slots.push(Slot {
                scene: *scene,
                clip: Some(sc),
                stop: true,
            });
        }
        t.slots.sort_by_key(|s| s.scene);
        // The fourth track armed: its empty slots show the record dot.
        t.armed = i == 3;
        p.tracks.push(t);
    }
    p
}

/// Light the grid as a playing set: track 0 playing scene 0, track 1 queued on scene 1,
/// track 2 stopping, track 3 recording into an empty slot (armed), scene 0 playing, scene
/// 1 queued, track 4 held silent, the transport four bars and a half in.
pub fn showcase(app: &mut CantusApp) {
    let bar = Ticks::beats(4);
    let now = bar * 4 + Ticks::beats(2);
    app.engine.set_heard_for_tests(now, true);
    let p = app.project().clone();
    let t = |i: usize| p.tracks.get(i).map(|t| t.id);
    let s = |j: usize| p.session.scenes.get(j).map(|s| s.id);
    let mut events = Vec::new();
    let mut slot = |i: usize, j: usize, phase: SlotPhase| {
        if let (Some(tr), Some(sc)) = (t(i), s(j)) {
            events.push(SessionEvent::Slot {
                slot: SlotRef::new(tr, sc),
                phase,
            });
        }
    };
    slot(
        0,
        0,
        SlotPhase::Playing {
            since: bar * 4,
            from: Ticks::ZERO,
        },
    );
    slot(1, 1, SlotPhase::Queued { at: bar * 5 });
    slot(
        2,
        0,
        SlotPhase::Stopping {
            since: bar * 2,
            from: Ticks::ZERO,
            at: bar * 5,
        },
    );
    slot(
        5,
        2,
        SlotPhase::Playing {
            since: bar * 3,
            from: Ticks::ZERO,
        },
    );
    // Track 3 records into its first empty slot.
    let empty = p.tracks.get(3).and_then(|tr| {
        p.session
            .scenes
            .iter()
            .find(|sc| tr.slot_clip(sc.id).is_none())
            .map(|sc| sc.id)
    });
    if let (Some(tr), Some(sc)) = (t(3), empty) {
        events.push(SessionEvent::Slot {
            slot: SlotRef::new(tr, sc),
            phase: SlotPhase::Recording { since: bar * 4 },
        });
    }
    if let Some(sc) = s(0) {
        events.push(SessionEvent::Scene {
            scene: sc,
            phase: ScenePhase::Playing { since: bar * 4 },
        });
    }
    if let Some(sc) = s(1) {
        events.push(SessionEvent::Scene {
            scene: sc,
            phase: ScenePhase::Queued { at: bar * 5 },
        });
    }
    for i in [0, 2, 3, 4, 5] {
        if let Some(tr) = t(i) {
            events.push(SessionEvent::Track {
                track: tr,
                arrangement: false,
            });
        }
    }
    for e in events {
        app.session_view.live.apply(e);
    }
}

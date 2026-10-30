//! DAWproject's scenes and clip slots (round 33, ADR 0076).
//!
//! The format holds a session as `<Scenes>`: each `<Scene>` a row (name, colour) whose
//! content is `<Lanes>` of `<ClipSlot track=… hasStop=…>`, each holding at most one
//! `<Clip>` — read and written as the arrangement's clips are, their time counted from the
//! slot. What the format has no place for — a clip's launch settings and follow actions, its
//! start marker, a scene's tempo, metre and follow action, the session's settings — Cantus
//! carries beside the XML as `cantus/session.json` (read back only from a file Cantus
//! wrote); clip envelopes have no place either and the report says so.

use super::read_timeline::{SlotData, Timeline};
use cantus_core::session::{
    FollowAction, FollowActions, LaunchSettings, Scene, SceneId, SessionClip, SessionSettings, Slot,
};
use cantus_core::{Project, Ticks, TimeSignature, Track};
use serde::{Deserialize, Serialize};

/// Where the session's own settings travel in the archive.
pub const SIDE: &str = "cantus/session.json";

/// The scenes the file holds, made in the project in its order (their slots come with
/// their tracks, `into_slots`).
pub fn make_scenes(project: &mut Project, t: &Timeline) {
    for s in &t.scenes {
        let mut scene = Scene::new(SceneId(project.alloc_id()), s.name.clone());
        scene.color = s.color;
        project.session.scenes.push(scene);
    }
}

/// The clip slots the file gives the track `key`.
pub fn slot_clips<'a>(t: &'a Timeline, key: &str) -> &'a [SlotData] {
    t.slots.get(key).map(Vec::as_slice).unwrap_or(&[])
}

/// Move the clips made from `slots` — `track.clips` past its first `arranged` — into the
/// track's column, with each slot's start marker and stop button; a scene's second slot
/// on one track is left out.
pub fn into_slots(project: &Project, track: &mut Track, arranged: usize, slots: &[SlotData]) {
    let mut made = track
        .clips
        .split_off(arranged.min(track.clips.len()))
        .into_iter();
    let mut column: Vec<Slot> = Vec::new();
    for s in slots {
        let clip = match &s.clip {
            Some(_) => match made.next() {
                Some(mut c) => {
                    c.start = Ticks::ZERO;
                    let mut sc = SessionClip::new(c);
                    sc.start = s.start;
                    Some(sc)
                }
                None => None,
            },
            None => None,
        };
        let Some(scene) = project.session.scenes.get(s.scene).map(|x| x.id) else {
            continue;
        };
        if column.iter().any(|x| x.scene == scene) {
            continue;
        }
        let slot = Slot {
            scene,
            clip,
            stop: s.stop,
        };
        if !slot.is_default() {
            column.push(slot);
        }
    }
    column.sort_by_key(|s| s.scene);
    track.slots = column;
}

/// A follow action with its jump written as a scene's index (ids are the file's own).
fn follow_out(f: &FollowActions, project: &Project) -> FollowActions {
    let map = |a: FollowAction| match a {
        FollowAction::Jump(id) => match project.session.scene_index(id) {
            Some(i) => FollowAction::Jump(SceneId(i as u64)),
            None => FollowAction::None,
        },
        a => a,
    };
    FollowActions {
        a: map(f.a),
        b: map(f.b),
        ..*f
    }
}

fn follow_in(f: &FollowActions, project: &Project) -> FollowActions {
    let map = |a: FollowAction| match a {
        FollowAction::Jump(SceneId(i)) => project
            .session
            .scenes
            .get(i as usize)
            .map_or(FollowAction::None, |s| FollowAction::Jump(s.id)),
        a => a,
    };
    FollowActions {
        a: map(f.a),
        b: map(f.b),
        ..*f
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
struct Side {
    #[serde(default)]
    settings: SessionSettings,
    #[serde(default)]
    scenes: Vec<SideScene>,
    #[serde(default)]
    slots: Vec<SideSlot>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct SideScene {
    tempo: Option<f64>,
    metre: Option<TimeSignature>,
    follow: FollowActions,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct SideSlot {
    scene: usize,
    track: usize,
    launch: LaunchSettings,
    follow: FollowActions,
    start: Option<Ticks>,
}

/// The side file for `project`'s session, or `None` when it holds nothing the XML lacks.
/// Also returns how many clip envelopes are left out.
pub fn side(project: &Project) -> (Option<Vec<u8>>, usize) {
    let mut side = Side {
        settings: project.session.settings,
        ..Default::default()
    };
    for s in &project.session.scenes {
        side.scenes.push(SideScene {
            tempo: s.tempo,
            metre: s.metre,
            follow: follow_out(&s.follow, project),
        });
    }
    let mut envelopes = 0;
    for (ti, t) in project.tracks.iter().enumerate() {
        for slot in &t.slots {
            let (Some(c), Some(si)) = (&slot.clip, project.session.scene_index(slot.scene)) else {
                continue;
            };
            envelopes += c.envelopes.len();
            if c.launch.is_default() && c.follow.is_off() && c.start.is_none() {
                continue;
            }
            side.slots.push(SideSlot {
                scene: si,
                track: ti,
                launch: c.launch,
                follow: follow_out(&c.follow, project),
                start: c.start,
            });
        }
    }
    let plain = side.settings.is_default()
        && side.slots.is_empty()
        && side
            .scenes
            .iter()
            .all(|s| s.tempo.is_none() && s.metre.is_none() && s.follow.is_off());
    let bytes = (!plain).then(|| serde_json::to_vec_pretty(&side).unwrap_or_default());
    (bytes, envelopes)
}

/// Apply the side file a file Cantus wrote carries (`bytes`), then put the session in the
/// form a command takes (`Project::repair_session`).
pub fn apply_side(project: &mut Project, bytes: Option<&[u8]>) {
    if let Some(side) = bytes.and_then(|b| serde_json::from_slice::<Side>(b).ok()) {
        project.session.settings = side.settings;
        for (i, s) in side.scenes.iter().enumerate() {
            let follow = follow_in(&s.follow, project);
            if let Some(scene) = project.session.scenes.get_mut(i) {
                scene.tempo = s.tempo;
                scene.metre = s.metre;
                scene.follow = follow;
            }
        }
        for s in &side.slots {
            let Some(scene) = project.session.scenes.get(s.scene).map(|x| x.id) else {
                continue;
            };
            let follow = follow_in(&s.follow, project);
            let Some(c) = project
                .tracks
                .get_mut(s.track)
                .and_then(|t| t.slots.iter_mut().find(|x| x.scene == scene))
                .and_then(|x| x.clip.as_mut())
            else {
                continue;
            };
            c.launch = s.launch;
            c.follow = follow;
            c.start = s.start;
        }
    }
    project.repair_session();
}

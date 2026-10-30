//! The session's gestures that are several commands at once (ADR 0076), each built here as
//! one `Command::Batch` — one undo step whose inverse restores everything: a clip or a
//! scene duplicated, *capture and insert scene*, clips taken from the arrangement into
//! slots and a slot's clip put into the arrangement, a column consolidated, a stretch of
//! the arrangement consolidated into a new scene, and **a performance committed to the
//! arrangement** (arrangement record).
//!
//! Each builder reads the project as it is and returns `None` when there is nothing to do
//! (a slot that is empty, a scene that is not there). Fresh ids come from `ids`, as
//! `CommandBus::alloc_id` gives them.

use super::{SceneId, SessionClip, SessionCommand, Slot, SlotRef};
use crate::automation::{AutoPoint, Automation};
use crate::command::Command;
use crate::model::{Clip, ClipId, NoteId, Project, TrackId};
use crate::time::Ticks;

/// Where fresh ids come from.
pub type Ids<'a> = &'a mut dyn FnMut() -> u64;

fn batch(label_key: &str, commands: Vec<Command>) -> Option<Command> {
    (!commands.is_empty()).then(|| Command::Batch {
        label_key: label_key.to_string(),
        commands,
    })
}

fn session(c: SessionCommand) -> Command {
    Command::Session(c)
}

/// A copy of `clip` with fresh ids — its own and its notes' — on the playing lane, in no
/// link group and no comp.
pub fn fresh_clip(clip: &Clip, ids: Ids) -> Clip {
    let mut c = clip.clone();
    c.id = ClipId(ids());
    for n in &mut c.notes {
        n.id = NoteId(ids());
    }
    c.link = None;
    c.comp = false;
    c.lane = 0;
    c
}

/// A copy of a session clip with fresh ids, everything else kept.
pub fn fresh(c: &SessionClip, ids: Ids) -> SessionClip {
    SessionClip {
        clip: fresh_clip(&c.clip, ids),
        ..c.clone()
    }
}

/// An arrangement clip taken into a slot: what its window shows, the window kept (a
/// repeating clip loops its repeat; one that plays once plays its length once). `None`
/// for an automation clip, which a slot does not hold.
pub fn from_arrangement(c: &Clip, ids: Ids) -> Option<SessionClip> {
    if c.automation.is_some() {
        return None;
    }
    let mut k = fresh_clip(c, ids);
    k.start = Ticks::ZERO;
    Some(SessionClip::new(k))
}

/// A new, empty MIDI clip for a slot: `bars_len` ticks long, looping (what a double-click
/// on an empty slot makes).
pub fn new_note_clip(name: &str, length: Ticks, ids: Ids) -> SessionClip {
    let mut c = SessionClip::new(Clip {
        id: ClipId(ids()),
        name: name.to_string(),
        start: Ticks::ZERO,
        length,
        offset: Ticks::ZERO,
        content_length: Some(length),
        muted: false,
        color: None,
        notes: Vec::new(),
        audio: None,
        lane: 0,
        automation: None,
        link: None,
        comp: false,
        lanes: Vec::new(),
        groove: None,
        skip_note_fx: false,
    });
    c.launch = Default::default();
    c
}

/// A new scene's command: `name`, at `index`, empty.
pub fn insert_scene(index: usize, name: &str, ids: Ids) -> Command {
    session(SessionCommand::InsertScene {
        index,
        scene: Box::new(super::Scene::new(SceneId(ids()), name)),
        slots: Vec::new(),
    })
}

/// Duplicate a slot's clip into slot `to` (empty): Live's Ctrl+D in the grid.
pub fn duplicate_slot(project: &Project, from: SlotRef, to: SlotRef, ids: Ids) -> Option<Command> {
    let c = project.slot_clip(from)?;
    project.session.scene(to.scene)?;
    if project.slot_clip(to).is_some() {
        return None;
    }
    let mut copy = fresh(c, ids);
    super::envelopes_for(project, &mut copy, from.track, to.track);
    batch(
        "cmd.session.duplicate_clip",
        vec![session(SessionCommand::InsertSlotClip {
            slot: to,
            clip: Box::new(copy),
        })],
    )
}

/// Duplicate a scene under itself: its settings and every slot of its row, the clips
/// copied with fresh ids.
pub fn duplicate_scene(project: &Project, id: SceneId, ids: Ids) -> Option<Command> {
    let index = project.session.scene_index(id)?;
    let mut scene = project.session.scenes[index].clone();
    scene.id = SceneId(ids());
    let slots = project
        .tracks
        .iter()
        .filter_map(|t| {
            let s = t.slot(id)?;
            Some((
                t.id,
                Slot {
                    scene: scene.id,
                    clip: s.clip.as_ref().map(|c| fresh(c, ids)),
                    stop: s.stop,
                },
            ))
        })
        .collect();
    batch(
        "cmd.session.duplicate_scene",
        vec![session(SessionCommand::InsertScene {
            index: index + 1,
            scene: Box::new(scene),
            slots,
        })],
    )
}

/// *Capture and insert scene* (Live): a new scene at `index` holding a copy of every clip
/// playing now (`playing`, as the engine reports them), the tracks playing none left
/// empty with their stop buttons.
pub fn capture_scene(
    project: &Project,
    playing: &[SlotRef],
    index: usize,
    name: &str,
    ids: Ids,
) -> Option<Command> {
    if index > project.session.scenes.len() {
        return None;
    }
    let scene = super::Scene::new(SceneId(ids()), name);
    let mut slots: Vec<(TrackId, Slot)> = Vec::new();
    for t in &project.tracks {
        let Some(r) = playing.iter().find(|r| r.track == t.id) else {
            continue;
        };
        let Some(c) = t.slot_clip(r.scene) else {
            continue;
        };
        slots.push((
            t.id,
            Slot {
                scene: scene.id,
                clip: Some(fresh(c, ids)),
                stop: true,
            },
        ));
    }
    batch(
        "cmd.session.capture_scene",
        vec![session(SessionCommand::InsertScene {
            index,
            scene: Box::new(scene),
            slots,
        })],
    )
}

/// Arrangement clips taken into the session: each track's clips, in time order, into its
/// own column's free slots from scene `first` down — new scenes added at the bottom when
/// the rows run out (Live's drag from the arrangement). Automation clips stay.
pub fn slots_from_arrangement(
    project: &Project,
    clips: &[(TrackId, ClipId)],
    first: usize,
    ids: Ids,
) -> Option<Command> {
    let scenes = &project.session.scenes;
    let mut per_track: Vec<(TrackId, Vec<&Clip>)> = Vec::new();
    for (t, c) in clips {
        let Some(clip) = project
            .track(*t)
            .and_then(|x| x.clips.iter().find(|k| k.id == *c))
        else {
            continue;
        };
        if clip.automation.is_some() {
            continue;
        }
        match per_track.iter_mut().find(|(x, _)| x == t) {
            Some((_, v)) => v.push(clip),
            None => per_track.push((*t, vec![clip])),
        }
    }
    // Rows: the scenes from `first` (free ones only), then new ones.
    let mut rows: Vec<SceneId> = scenes.iter().map(|s| s.id).collect();
    let mut out = Vec::new();
    let mut placed: Vec<(TrackId, usize, SessionClip)> = Vec::new();
    for (t, mut cs) in per_track {
        cs.sort_by_key(|c| (c.start, c.id));
        let track = project.track(t).expect("found above");
        let mut row = first.min(rows.len());
        for c in cs {
            while row < scenes.len() && track.slot_clip(rows[row]).is_some() {
                row += 1;
            }
            let sc = from_arrangement(c, ids).expect("not automation");
            placed.push((t, row, sc));
            row += 1;
        }
    }
    let needed = placed.iter().map(|(_, r, _)| r + 1).max().unwrap_or(0);
    while rows.len() < needed {
        let id = SceneId(ids());
        out.push(session(SessionCommand::InsertScene {
            index: rows.len(),
            scene: Box::new(super::Scene::new(id, "")),
            slots: Vec::new(),
        }));
        rows.push(id);
    }
    for (t, row, sc) in placed {
        out.push(session(SessionCommand::InsertSlotClip {
            slot: SlotRef::new(t, rows[row]),
            clip: Box::new(sc),
        }));
    }
    batch("cmd.session.from_arrangement", out)
}

/// The value of a curve of points at content tick `t` (held before the first and after the
/// last), as an automation curve reads it.
pub(crate) fn curve_at(points: &[AutoPoint], t: Ticks) -> Option<f32> {
    Automation {
        target: crate::automation::AutoTarget::MasterGain,
        points: points.to_vec(),
    }
    .value_at(t.0 as f64)
}

#[path = "session_perform.rs"]
mod perform;
pub use perform::{
    clear_range, commit_performance, consolidate_column, consolidate_time, perform, perform_as,
    pieces, slot_to_arrangement, warp_ahead, Performance, Piece, Played, CUT_FADE,
};

//! The session's commands (ADR 0004, 0076): every edit of the grid, each with an exact
//! inverse. `Command::Session` carries one; the builders that make several into one step
//! (`Command::Batch`) are in [`super::edit`].

use super::{
    FollowActions, LaunchSettings, Scene, SceneId, SessionClip, SessionSettings, Slot, SlotRef,
};
use crate::automation::Automation;
use crate::command::{Command, CommandError};
use crate::freeze::Touch;
use crate::model::{Color, Project, Track, TrackId};
use crate::time::{Ticks, TimeSignature};
use serde::{Deserialize, Serialize};

/// A session clip's window and start marker at once — what its loop panel edits as one
/// step (the same fields `SetClipOffset`, `SetClipBounds` and `SetClipLoop` reach one by
/// one).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlotRegion {
    /// The start marker (`None`: the loop's start, `offset`).
    pub start: Option<Ticks>,
    /// Where the loop (or, without one, the clip) starts, in content ticks.
    pub offset: Ticks,
    /// The end marker's distance from `offset`: what the clip plays without its loop.
    pub length: Ticks,
    /// The loop's length (`None`: no loop).
    pub loop_length: Option<Ticks>,
}

impl SlotRegion {
    pub fn of(c: &SessionClip) -> Self {
        SlotRegion {
            start: c.start,
            offset: c.clip.offset,
            length: c.clip.length,
            loop_length: c.clip.content_length,
        }
    }
}

/// An edit of the session. Slots are named by track and scene; refused (and nothing
/// changed) for a track or scene that is not there.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum SessionCommand {
    /// Put a clip in an empty slot (an empty slot without its stop button keeps that).
    /// Refused for an occupied slot, a clip a command would refuse (`check_clip`) or whose
    /// id the track already holds.
    InsertSlotClip {
        slot: SlotRef,
        clip: Box<SessionClip>,
    },
    /// Take a slot's clip out.
    RemoveSlotClip {
        slot: SlotRef,
    },
    /// Move a slot's clip to an empty slot, of this track or another.
    MoveSlotClip {
        from: SlotRef,
        to: SlotRef,
    },
    /// How a slot's clip launches (a drag merges into one step).
    SetSlotLaunch {
        slot: SlotRef,
        launch: LaunchSettings,
    },
    /// What follows a slot's clip.
    SetSlotFollow {
        slot: SlotRef,
        follow: FollowActions,
    },
    /// A slot's clip's window and start marker.
    SetSlotRegion {
        slot: SlotRef,
        region: SlotRegion,
    },
    /// A slot's clip's own envelopes, all at once (drawing one merges into one step).
    SetSlotEnvelopes {
        slot: SlotRef,
        envelopes: Vec<Automation>,
    },
    /// Whether an empty slot stops its track when its scene is launched.
    SetSlotStop {
        slot: SlotRef,
        stop: bool,
    },
    /// Put a scene at `index` with its row's slots (an undo of its removal, a duplicate, a
    /// capture). Refused for an id taken, an index past the end, a slot of another scene,
    /// a track twice or not there, or a clip `InsertSlotClip` would refuse.
    InsertScene {
        index: usize,
        scene: Box<Scene>,
        slots: Vec<(TrackId, Slot)>,
    },
    /// Take a scene out, with its row's slots.
    RemoveScene {
        id: SceneId,
    },
    /// Move a scene to `to` (its index once moved).
    MoveScene {
        id: SceneId,
        to: usize,
    },
    SetSceneName {
        id: SceneId,
        name: String,
    },
    SetSceneColor {
        id: SceneId,
        color: Option<Color>,
    },
    /// The tempo launching the scene sets (`None`: it leaves the tempo).
    SetSceneTempo {
        id: SceneId,
        tempo: Option<f64>,
    },
    /// The metre launching the scene sets (`None`: it leaves the metre).
    SetSceneMetre {
        id: SceneId,
        metre: Option<TimeSignature>,
    },
    SetSceneFollow {
        id: SceneId,
        follow: FollowActions,
    },
    /// The session's own settings (global and record quantisation, seed, fixed length).
    SetSettings {
        settings: SessionSettings,
    },
}

impl SessionCommand {
    /// The history's name for it (`cmd.*`, both locales).
    pub fn label_key(&self) -> &'static str {
        use SessionCommand::*;
        match self {
            InsertSlotClip { .. } => "cmd.session.clip_insert",
            RemoveSlotClip { .. } => "cmd.session.clip_remove",
            MoveSlotClip { .. } => "cmd.session.clip_move",
            SetSlotLaunch { .. } => "cmd.session.launch",
            SetSlotFollow { .. } => "cmd.session.follow",
            SetSlotRegion { .. } => "cmd.session.region",
            SetSlotEnvelopes { .. } => "cmd.session.envelopes",
            SetSlotStop { .. } => "cmd.session.stop_button",
            InsertScene { .. } => "cmd.session.scene_insert",
            RemoveScene { .. } => "cmd.session.scene_remove",
            MoveScene { .. } => "cmd.session.scene_move",
            SetSceneName { .. } => "cmd.session.scene_name",
            SetSceneColor { .. } => "cmd.session.scene_color",
            SetSceneTempo { .. } => "cmd.session.scene_tempo",
            SetSceneMetre { .. } => "cmd.session.scene_metre",
            SetSceneFollow { .. } => "cmd.session.scene_follow",
            SetSettings { .. } => "cmd.session.settings",
        }
    }

    /// Whether `self` sets what `older` set, so a gesture keeps only the newest.
    pub fn supersedes(&self, older: &SessionCommand) -> bool {
        use SessionCommand::*;
        match (self, older) {
            (SetSlotLaunch { slot: a, .. }, SetSlotLaunch { slot: b, .. })
            | (SetSlotFollow { slot: a, .. }, SetSlotFollow { slot: b, .. })
            | (SetSlotRegion { slot: a, .. }, SetSlotRegion { slot: b, .. })
            | (SetSlotEnvelopes { slot: a, .. }, SetSlotEnvelopes { slot: b, .. }) => a == b,
            (SetSceneName { id: a, .. }, SetSceneName { id: b, .. })
            | (SetSceneColor { id: a, .. }, SetSceneColor { id: b, .. })
            | (SetSceneTempo { id: a, .. }, SetSceneTempo { id: b, .. })
            | (SetSceneMetre { id: a, .. }, SetSceneMetre { id: b, .. })
            | (SetSceneFollow { id: a, .. }, SetSceneFollow { id: b, .. }) => a == b,
            (SetSettings { .. }, SetSettings { .. }) => true,
            _ => false,
        }
    }
}

fn bad(what: &str) -> CommandError {
    CommandError::InvalidValue(what.to_string())
}

/// Refuse a session clip a command would refuse: an arrangement clip's own checks, and a
/// window of the session's form — at 0, on the playing lane, in no link group or comp, a
/// note or audio clip (automation lives in its envelopes) — with valid settings.
pub fn check_clip(project: &Project, c: &SessionClip) -> Result<(), CommandError> {
    crate::command::validate_clip(&c.clip)?;
    crate::notefx_cmd::check_clip(project, &c.clip)?;
    let k = &c.clip;
    if k.start != Ticks::ZERO
        || k.lane != 0
        || k.link.is_some()
        || k.comp
        || k.automation.is_some()
        || k.offset.is_negative()
    {
        return Err(bad("session clip"));
    }
    if !c.launch.is_valid() || !c.follow.is_valid() {
        return Err(bad("session clip settings"));
    }
    if c.start.is_some_and(|s| s.is_negative() || s >= c.end()) {
        return Err(bad("session clip start"));
    }
    check_envelopes(&c.envelopes)
}

fn check_envelopes(envs: &[Automation]) -> Result<(), CommandError> {
    for (i, e) in envs.iter().enumerate() {
        if !e.is_valid() || envs[..i].iter().any(|o| o.target == e.target) {
            return Err(bad("clip envelope"));
        }
    }
    Ok(())
}

fn track_mut(project: &mut Project, id: TrackId) -> Result<&mut Track, CommandError> {
    project.track_mut(id).ok_or(CommandError::TrackNotFound(id))
}

fn need_scene(project: &Project, id: SceneId) -> Result<usize, CommandError> {
    project
        .session
        .scene_index(id)
        .ok_or_else(|| bad("scene not found"))
}

/// The track's entry for `scene`, made (empty, with its stop button) when there is none.
fn entry(t: &mut Track, scene: SceneId) -> &mut Slot {
    let i = match t.slots.binary_search_by_key(&scene, |s| s.scene) {
        Ok(i) => i,
        Err(i) => {
            t.slots.insert(
                i,
                Slot {
                    scene,
                    clip: None,
                    stop: true,
                },
            );
            i
        }
    };
    &mut t.slots[i]
}

/// Drops the entries that hold nothing worth storing: the column's canonical form.
fn tidy(t: &mut Track) {
    t.slots.retain(|s| !s.is_default());
}

fn bump(project: &mut Project, id: u64) {
    project.next_id = project.next_id.max(id.saturating_add(1));
}

/// The clip of a slot, for the setters: refused when the slot holds none.
fn clip_mut(project: &mut Project, slot: SlotRef) -> Result<&mut SessionClip, CommandError> {
    need_scene(project, slot.scene)?;
    track_mut(project, slot.track)?
        .slots
        .iter_mut()
        .find(|s| s.scene == slot.scene)
        .and_then(|s| s.clip.as_mut())
        .ok_or_else(|| bad("slot is empty"))
}

fn scene_mut(project: &mut Project, id: SceneId) -> Result<&mut Scene, CommandError> {
    project
        .session
        .scene_mut(id)
        .ok_or_else(|| bad("scene not found"))
}

/// Applies `cmd`, returning its inverse (`Command::Session`).
pub(crate) fn apply(cmd: SessionCommand, project: &mut Project) -> Result<Command, CommandError> {
    use SessionCommand::*;
    let inverse = match cmd {
        InsertSlotClip { slot, clip } => {
            need_scene(project, slot.scene)?;
            check_clip(project, &clip)?;
            let t = project
                .track(slot.track)
                .ok_or(CommandError::TrackNotFound(slot.track))?;
            if t.slot_clip(slot.scene).is_some() {
                return Err(bad("slot is taken"));
            }
            if t.clip(clip.clip.id).is_some() {
                return Err(CommandError::DuplicateId(clip.clip.id.0));
            }
            bump(project, clip.clip.id.0);
            let t = track_mut(project, slot.track)?;
            entry(t, slot.scene).clip = Some(*clip);
            RemoveSlotClip { slot }
        }
        RemoveSlotClip { slot } => {
            need_scene(project, slot.scene)?;
            let t = track_mut(project, slot.track)?;
            let clip = t
                .slots
                .iter_mut()
                .find(|s| s.scene == slot.scene)
                .and_then(|s| s.clip.take())
                .ok_or_else(|| bad("slot is empty"))?;
            tidy(t);
            InsertSlotClip {
                slot,
                clip: Box::new(clip),
            }
        }
        MoveSlotClip { from, to } => {
            need_scene(project, from.scene)?;
            need_scene(project, to.scene)?;
            let src = project
                .track(from.track)
                .ok_or(CommandError::TrackNotFound(from.track))?;
            let id = src
                .slot_clip(from.scene)
                .ok_or_else(|| bad("slot is empty"))?
                .clip
                .id;
            let dst = project
                .track(to.track)
                .ok_or(CommandError::TrackNotFound(to.track))?;
            if dst.slot_clip(to.scene).is_some() {
                return Err(bad("slot is taken"));
            }
            if from.track != to.track && dst.clip(id).is_some() {
                return Err(CommandError::DuplicateId(id.0));
            }
            // Its envelopes of the track's own parameters follow it (review 1, #6).
            let envs = &src.slot_clip(from.scene).expect("checked above").envelopes;
            if !super::envelopes_move(project, envs, from.track, to.track) {
                return Err(bad("clip envelopes cannot follow"));
            }
            let t = track_mut(project, from.track)?;
            let mut clip = entry(t, from.scene).clip.take().expect("checked above");
            tidy(t);
            super::swap_targets(&mut clip.envelopes, from.track, to.track);
            let t = track_mut(project, to.track)?;
            entry(t, to.scene).clip = Some(clip);
            MoveSlotClip { from: to, to: from }
        }
        SetSlotLaunch { slot, launch } => {
            if !launch.is_valid() {
                return Err(bad("launch settings"));
            }
            let c = clip_mut(project, slot)?;
            let old = std::mem::replace(&mut c.launch, launch);
            SetSlotLaunch { slot, launch: old }
        }
        SetSlotFollow { slot, follow } => {
            if !follow.is_valid() {
                return Err(bad("follow actions"));
            }
            let c = clip_mut(project, slot)?;
            let old = std::mem::replace(&mut c.follow, follow);
            SetSlotFollow { slot, follow: old }
        }
        SetSlotRegion { slot, region } => {
            let c = clip_mut(project, slot)?;
            let mut next = c.clone();
            next.start = region.start;
            next.clip.offset = region.offset;
            next.clip.length = region.length;
            next.clip.content_length = region.loop_length;
            if region.loop_length.is_some_and(|l| l <= Ticks::ZERO)
                || region.length <= Ticks::ZERO
                || region.offset.is_negative()
                || next
                    .start
                    .is_some_and(|s| s.is_negative() || s >= next.end())
            {
                return Err(bad("session clip region"));
            }
            let old = SlotRegion::of(c);
            *c = next;
            SetSlotRegion { slot, region: old }
        }
        SetSlotEnvelopes { slot, envelopes } => {
            check_envelopes(&envelopes)?;
            let c = clip_mut(project, slot)?;
            let old = std::mem::replace(&mut c.envelopes, envelopes);
            SetSlotEnvelopes {
                slot,
                envelopes: old,
            }
        }
        SetSlotStop { slot, stop } => {
            need_scene(project, slot.scene)?;
            let t = track_mut(project, slot.track)?;
            let e = entry(t, slot.scene);
            let old = std::mem::replace(&mut e.stop, stop);
            tidy(t);
            SetSlotStop { slot, stop: old }
        }
        InsertScene {
            index,
            scene,
            slots,
        } => insert_scene(project, index, *scene, slots)?,
        RemoveScene { id } => {
            let index = need_scene(project, id)?;
            let scene = project.session.scenes.remove(index);
            let mut slots = Vec::new();
            for t in &mut project.tracks {
                if let Some(i) = t.slots.iter().position(|s| s.scene == id) {
                    slots.push((t.id, t.slots.remove(i)));
                }
            }
            InsertScene {
                index,
                scene: Box::new(scene),
                slots,
            }
        }
        MoveScene { id, to } => {
            let from = need_scene(project, id)?;
            let len = project.session.scenes.len();
            if to >= len {
                return Err(CommandError::IndexOutOfRange { index: to, len });
            }
            let s = project.session.scenes.remove(from);
            project.session.scenes.insert(to, s);
            MoveScene { id, to: from }
        }
        SetSceneName { id, name } => {
            let s = scene_mut(project, id)?;
            SetSceneName {
                id,
                name: std::mem::replace(&mut s.name, name),
            }
        }
        SetSceneColor { id, color } => {
            let s = scene_mut(project, id)?;
            SetSceneColor {
                id,
                color: std::mem::replace(&mut s.color, color),
            }
        }
        SetSceneTempo { id, tempo } => {
            let probe = Scene {
                tempo,
                ..Scene::new(id, "")
            };
            if !probe.is_valid() {
                return Err(bad("scene tempo"));
            }
            let s = scene_mut(project, id)?;
            SetSceneTempo {
                id,
                tempo: std::mem::replace(&mut s.tempo, tempo),
            }
        }
        SetSceneMetre { id, metre } => {
            if metre.is_some_and(|m| !super::metre_is_valid(m)) {
                return Err(bad("scene metre"));
            }
            let s = scene_mut(project, id)?;
            SetSceneMetre {
                id,
                metre: std::mem::replace(&mut s.metre, metre),
            }
        }
        SetSceneFollow { id, follow } => {
            if !follow.is_valid() {
                return Err(bad("follow actions"));
            }
            let s = scene_mut(project, id)?;
            SetSceneFollow {
                id,
                follow: std::mem::replace(&mut s.follow, follow),
            }
        }
        SetSettings { settings } => {
            if !settings.is_valid() {
                return Err(bad("session settings"));
            }
            SetSettings {
                settings: std::mem::replace(&mut project.session.settings, settings),
            }
        }
    };
    Ok(Command::Session(inverse))
}

fn insert_scene(
    project: &mut Project,
    index: usize,
    scene: Scene,
    slots: Vec<(TrackId, Slot)>,
) -> Result<SessionCommand, CommandError> {
    let len = project.session.scenes.len();
    if index > len {
        return Err(CommandError::IndexOutOfRange { index, len });
    }
    if !scene.is_valid() {
        return Err(bad("scene"));
    }
    if project.session.scene(scene.id).is_some() {
        return Err(CommandError::DuplicateId(scene.id.0));
    }
    let mut ids = Vec::new();
    for (i, (track, slot)) in slots.iter().enumerate() {
        let t = project
            .track(*track)
            .ok_or(CommandError::TrackNotFound(*track))?;
        if slot.scene != scene.id || slot.is_default() || slots[..i].iter().any(|(o, _)| o == track)
        {
            return Err(bad("scene slots"));
        }
        if let Some(c) = &slot.clip {
            check_clip(project, c)?;
            let id = c.clip.id;
            if t.clip(id).is_some() || ids.contains(&(*track, id)) {
                return Err(CommandError::DuplicateId(id.0));
            }
            ids.push((*track, id));
        }
    }
    let id = scene.id;
    bump(project, id.0);
    for (_, c) in &ids {
        bump(project, c.0);
    }
    project.session.scenes.insert(index, scene);
    for (track, slot) in slots {
        let t = project.track_mut(track).expect("checked above");
        let at = t.slots.partition_point(|s| s.scene < id);
        t.slots.insert(at, slot);
    }
    Ok(SessionCommand::RemoveScene { id })
}

/// What `cmd` touches of a frozen sound (ADR 0064): a slot's track's source — a frozen
/// track's column is locked with the rest of what it plays — and the targets of the
/// envelopes it sets.
pub(crate) fn touches(cmd: &SessionCommand, project: &Project) -> Vec<Touch> {
    use SessionCommand::*;
    match cmd {
        InsertSlotClip { slot, clip } => {
            let mut v = vec![Touch::TrackSource(slot.track)];
            v.extend(
                clip.envelopes
                    .iter()
                    .map(|e| Touch::Target(e.target.clone())),
            );
            v
        }
        RemoveSlotClip { slot } => carried(project, std::iter::once(*slot)),
        SetSlotLaunch { slot, .. }
        | SetSlotFollow { slot, .. }
        | SetSlotRegion { slot, .. }
        | SetSlotStop { slot, .. } => vec![Touch::TrackSource(slot.track)],
        SetSlotEnvelopes { slot, envelopes } => {
            let mut v = vec![Touch::TrackSource(slot.track)];
            let old = project
                .slot_clip(*slot)
                .map(|c| c.envelopes.as_slice())
                .unwrap_or(&[]);
            for e in envelopes.iter().chain(old) {
                v.push(Touch::Target(e.target.clone()));
            }
            v
        }
        MoveSlotClip { from, to } => {
            let mut v = carried(project, std::iter::once(*from));
            v.push(Touch::TrackSource(to.track));
            // Where its envelopes land on the other track.
            if let Some(c) = project.slot_clip(*from) {
                let mut envs = c.envelopes.clone();
                super::swap_targets(&mut envs, from.track, to.track);
                v.extend(envs.into_iter().map(|e| Touch::Target(e.target)));
            }
            v
        }
        InsertScene { slots, .. } => {
            let mut v = Vec::new();
            for (t, s) in slots {
                v.push(Touch::TrackSource(*t));
                for e in s.clip.iter().flat_map(|c| c.envelopes.iter()) {
                    v.push(Touch::Target(e.target.clone()));
                }
            }
            v
        }
        RemoveScene { id } => carried(
            project,
            project
                .tracks
                .iter()
                .filter(|t| t.slot(*id).is_some())
                .map(|t| SlotRef::new(t.id, *id)),
        ),
        MoveScene { .. }
        | SetSceneName { .. }
        | SetSceneColor { .. }
        | SetSceneTempo { .. }
        | SetSceneMetre { .. }
        | SetSceneFollow { .. }
        | SetSettings { .. } => vec![],
    }
}

/// The touches of taking slots out: their tracks' sources and the targets their clips'
/// envelopes drive.
fn carried(project: &Project, slots: impl Iterator<Item = SlotRef>) -> Vec<Touch> {
    let mut v = Vec::new();
    for s in slots {
        v.push(Touch::TrackSource(s.track));
        for e in project.slot_clip(s).iter().flat_map(|c| c.envelopes.iter()) {
            v.push(Touch::Target(e.target.clone()));
        }
    }
    v
}

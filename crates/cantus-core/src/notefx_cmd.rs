//! The bodies of round 31's commands (ADR 0069): note-effect chains and their parameters,
//! the song's key, clips' grooves and the groove pool. Each returns its exact inverse.

use crate::command::{edit_shared, Command, CommandError};
use crate::groove::{ClipGroove, Groove, GrooveId, GrooveSource};
use crate::model::{Clip, ClipId, Project, TrackId};
use crate::notefx::{chain_is_valid, NoteEffect, NoteFxId};
use crate::scale::Key;

fn bad(what: &str) -> CommandError {
    CommandError::InvalidValue(what.to_string())
}

pub(crate) fn set_chain(
    project: &mut Project,
    track: TrackId,
    chain: Vec<NoteEffect>,
) -> Result<Command, CommandError> {
    if !chain_is_valid(&chain) {
        return Err(bad("note-effect chain"));
    }
    // Ids are the project's: another track's chain may not hold one of these.
    let taken = project
        .tracks
        .iter()
        .filter(|t| t.id != track)
        .flat_map(|t| t.note_fx.iter())
        .any(|o| chain.iter().any(|e| e.id == o.id));
    if taken {
        return Err(bad("note-effect id in use"));
    }
    let t = project
        .track_mut(track)
        .ok_or(CommandError::TrackNotFound(track))?;
    let old = std::mem::replace(&mut t.note_fx, chain);
    Ok(Command::SetTrackNoteFx { track, chain: old })
}

pub(crate) fn set_param(
    project: &mut Project,
    track: TrackId,
    effect: NoteFxId,
    key: String,
    value: f32,
) -> Result<Command, CommandError> {
    if !value.is_finite() {
        return Err(bad("note-effect value"));
    }
    let t = project
        .track_mut(track)
        .ok_or(CommandError::TrackNotFound(track))?;
    let e = t
        .note_fx
        .iter_mut()
        .find(|e| e.id == effect)
        .ok_or_else(|| bad("note effect"))?;
    let old = e
        .kind
        .get(&key)
        .ok_or_else(|| bad("note-effect parameter"))?;
    e.kind.set(&key, value);
    Ok(Command::SetNoteFxParam {
        track,
        effect,
        key,
        value: old,
    })
}

pub(crate) fn set_key(project: &mut Project, key: Option<Key>) -> Result<Command, CommandError> {
    if key.is_some_and(|k| !k.is_valid()) {
        return Err(bad("key"));
    }
    let old = std::mem::replace(&mut project.key, key);
    Ok(Command::SetProjectKey { key: old })
}

/// A clip that holds notes (not audio, not automation).
fn note_clip(c: &Clip) -> Result<(), CommandError> {
    if c.audio.is_some() || c.automation.is_some() {
        Err(bad("not a note clip"))
    } else {
        Ok(())
    }
}

pub(crate) fn set_clip_groove(
    project: &mut Project,
    track: TrackId,
    clip: ClipId,
    groove: Option<ClipGroove>,
) -> Result<Command, CommandError> {
    if let Some(g) = &groove {
        if !g.is_valid() || !g.source.exists(&project.grooves) {
            return Err(bad("groove"));
        }
    }
    let old = edit_shared(project, track, clip, |c| {
        note_clip(c)?;
        Ok(std::mem::replace(&mut c.groove, groove))
    })?;
    Ok(Command::SetClipGroove {
        track,
        clip,
        groove: old,
    })
}

pub(crate) fn set_clip_skip(
    project: &mut Project,
    track: TrackId,
    clip: ClipId,
    skip: bool,
) -> Result<Command, CommandError> {
    let old = edit_shared(project, track, clip, |c| {
        note_clip(c)?;
        Ok(std::mem::replace(&mut c.skip_note_fx, skip))
    })?;
    Ok(Command::SetClipSkipNoteFx {
        track,
        clip,
        skip: old,
    })
}

pub(crate) fn insert_groove(
    project: &mut Project,
    index: usize,
    groove: Groove,
) -> Result<Command, CommandError> {
    if !groove.is_valid() || groove.id == GrooveId(0) {
        return Err(bad("groove"));
    }
    if project.grooves.iter().any(|g| g.id == groove.id) {
        return Err(CommandError::DuplicateId(groove.id.0));
    }
    if index > project.grooves.len() {
        return Err(CommandError::IndexOutOfRange {
            index,
            len: project.grooves.len(),
        });
    }
    let id = groove.id;
    project.grooves.insert(index, groove);
    Ok(Command::RemoveGroove { id })
}

/// Whether some clip plays with pooled groove `id`.
pub(crate) fn groove_in_use(project: &Project, id: GrooveId) -> bool {
    project.tracks.iter().flat_map(|t| t.clips.iter()).any(|c| {
        c.groove
            .is_some_and(|g| g.source == GrooveSource::Pool { id })
    })
}

pub(crate) fn remove_groove(project: &mut Project, id: GrooveId) -> Result<Command, CommandError> {
    let index = project
        .grooves
        .iter()
        .position(|g| g.id == id)
        .ok_or_else(|| bad("groove"))?;
    if groove_in_use(project, id) {
        return Err(bad("groove in use"));
    }
    let groove = project.grooves.remove(index);
    Ok(Command::InsertGroove {
        index,
        groove: Box::new(groove),
    })
}

pub(crate) fn set_groove(project: &mut Project, groove: Groove) -> Result<Command, CommandError> {
    if !groove.is_valid() {
        return Err(bad("groove"));
    }
    let slot = project
        .grooves
        .iter_mut()
        .find(|g| g.id == groove.id)
        .ok_or_else(|| bad("groove"))?;
    let old = std::mem::replace(slot, groove);
    Ok(Command::SetGroove {
        groove: Box::new(old),
    })
}

/// A track coming in with a chain (`InsertTrack`: an undone removal, an import, a template):
/// valid, and its ids held by no other track.
pub(crate) fn check_new(project: &Project, chain: &[NoteEffect]) -> Result<(), CommandError> {
    if !chain_is_valid(chain) {
        return Err(bad("note-effect chain"));
    }
    if let Some(e) = chain.iter().find(|e| {
        project
            .tracks
            .iter()
            .any(|t| t.note_fx.iter().any(|o| o.id == e.id))
    }) {
        return Err(CommandError::DuplicateId(e.id.0));
    }
    Ok(())
}

/// A clip coming in (`InsertClip`, a track's clips): its groove, if any, valid, on a note
/// clip, naming a swing or a groove the pool holds — what `SetClipGroove` would accept, so a
/// paste or an undo cannot bring in a dangling or out-of-range one.
pub(crate) fn check_clip(project: &Project, c: &Clip) -> Result<(), CommandError> {
    let Some(g) = &c.groove else {
        return Ok(());
    };
    note_clip(c)?;
    if !g.is_valid() || !g.source.exists(&project.grooves) {
        return Err(bad("groove"));
    }
    Ok(())
}

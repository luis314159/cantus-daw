//! A session clip stays a session clip (ADR 0076): the content commands reach it as they
//! reach an arrangement clip (`Track::clip` finds it), and a command that would give it what
//! a slot cannot hold — a link group, a take lane, a comp, an automation curve, a place
//! other than 0, a start marker past its end — is refused, the project unchanged.

use crate::command::{Command, CommandError};
use crate::model::{ClipId, Project, TrackId};

/// The clip a plain clip command names, when it is one of a track's session clips.
fn names_slot_clip(cmd: &Command, project: &Project) -> Option<(TrackId, ClipId)> {
    use Command::*;
    let (track, clip) = match cmd {
        SetClipBounds { track, clip, .. }
        | SetClipName { track, clip, .. }
        | SetClipOffset { track, clip, .. }
        | SetClipLoop { track, clip, .. }
        | SetClipMute { track, clip, .. }
        | SetClipColor { track, clip, .. }
        | SetClipAudio { track, clip, .. }
        | SetClipPitch { track, clip, .. }
        | SetClipWarp { track, clip, .. }
        | SetClipAutomation { track, clip, .. }
        | SetClipLanes { track, clip, .. }
        | SetClipLink { track, clip, .. }
        | SetClipContent { track, clip, .. }
        | SetClipLane { track, clip, .. }
        | SetClipComp { track, clip, .. }
        | SetClipGroove { track, clip, .. }
        | SetClipSkipNoteFx { track, clip, .. }
        | InsertNotes { track, clip, .. }
        | RemoveNotes { track, clip, .. }
        | SetNotes { track, clip, .. } => (*track, *clip),
        MoveClipToTrack { from, clip, .. } => (*from, *clip),
        SetCompSeam {
            track, left, right, ..
        } => {
            let t = project.track(*track)?;
            return [*left, *right]
                .into_iter()
                .find(|c| t.scene_of_clip(*c).is_some())
                .map(|c| (*track, c));
        }
        _ => return None,
    };
    project
        .track(track)?
        .scene_of_clip(clip)
        .map(|_| (track, clip))
}

/// Runs `apply` on `cmd`; when it names a session clip, the clip is checked after and the
/// command undone and refused if it no longer has a session clip's form. Commands that
/// name none — the session's own, a batch (each of its commands comes back here) — run as
/// they are.
pub(crate) fn refuse(
    cmd: Command,
    project: &mut Project,
    apply: impl FnOnce(Command, &mut Project) -> Result<Command, CommandError>,
) -> Result<Command, CommandError> {
    let Some((track, clip)) = names_slot_clip(&cmd, project) else {
        return apply(cmd, project);
    };
    if matches!(
        cmd,
        Command::MoveClipToTrack { .. } | Command::SetCompSeam { .. }
    ) {
        return Err(CommandError::InvalidValue("a session clip".into()));
    }
    let inverse = apply(cmd, project)?;
    let ok = project.track(track).is_some_and(|t| {
        t.scene_of_clip(clip)
            .and_then(|s| t.slot_clip(s))
            .is_some_and(|c| super::cmd::check_clip(project, c).is_ok())
    });
    if !ok {
        // Undone: the project is as it was (an inverse is exact, ADR 0004).
        let _ = inverse.apply(project);
        return Err(CommandError::InvalidValue("a session clip".into()));
    }
    Ok(inverse)
}

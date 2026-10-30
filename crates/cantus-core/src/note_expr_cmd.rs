//! The expression command (ADR 0080): `Command::SetNoteExpr` sets the curves of the notes
//! it names, and its inverse — the same command with the old curves — gives them back
//! exactly. Every gesture (draw, scale, smooth, thin, flatten, clear, a recorded take's
//! curves) computes its result here or in [`super::ops`] and is stored by this one command,
//! named in the history by its [`ExprEdit`].

use super::NoteExpr;
use crate::command::{edit_shared, Command, CommandError};
use crate::model::{Clip, ClipId, NoteId, Project, TrackId};
use serde::{Deserialize, Serialize};

/// Which gesture an expression edit was: what the history names it.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExprEdit {
    /// Curves given whole (a paste, a recorded take's curves, a script).
    Set,
    /// Drawn with the pen or the line tool in the expression lane.
    Draw,
    /// Scaled about the rest (more or less bend, pressure, timbre).
    Scale,
    /// Smoothed.
    Smooth,
    /// Thinned (fewer points, the same shape).
    Thin,
    /// Movement taken out.
    Flatten,
    /// Removed.
    Clear,
}

impl ExprEdit {
    pub const ALL: [ExprEdit; 7] = [
        ExprEdit::Set,
        ExprEdit::Draw,
        ExprEdit::Scale,
        ExprEdit::Smooth,
        ExprEdit::Thin,
        ExprEdit::Flatten,
        ExprEdit::Clear,
    ];

    pub fn label_key(self) -> &'static str {
        match self {
            ExprEdit::Set => "cmd.mpe.set",
            ExprEdit::Draw => "cmd.mpe.draw",
            ExprEdit::Scale => "cmd.mpe.scale",
            ExprEdit::Smooth => "cmd.mpe.smooth",
            ExprEdit::Thin => "cmd.mpe.thin",
            ExprEdit::Flatten => "cmd.mpe.flatten",
            ExprEdit::Clear => "cmd.mpe.clear",
        }
    }
}

type Curves = Vec<(NoteId, Option<Box<NoteExpr>>)>;

/// Apply `Command::SetNoteExpr`: refused whole (nothing changed) for a note that is not in
/// the clip, a note named twice, or a malformed curve.
pub(crate) fn apply(
    project: &mut Project,
    track: TrackId,
    clip: ClipId,
    edit: ExprEdit,
    notes: Curves,
) -> Result<Command, CommandError> {
    for (i, (id, e)) in notes.iter().enumerate() {
        if notes[..i].iter().any(|(o, _)| o == id) {
            return Err(CommandError::DuplicateId(id.0));
        }
        if let Some(e) = e {
            if e.is_empty() || !e.is_valid() {
                return Err(CommandError::InvalidValue(format!("note {id} expression")));
            }
        }
    }
    let old = edit_shared(project, track, clip, |c: &mut Clip| {
        for (id, _) in &notes {
            if c.note(*id).is_none() {
                return Err(CommandError::NoteNotFound(*id));
            }
        }
        let mut old = Vec::with_capacity(notes.len());
        for (id, e) in &notes {
            if let Some(n) = c.note_mut(*id) {
                old.push((*id, std::mem::replace(&mut n.expr, e.clone())));
            }
        }
        Ok(old)
    })?;
    Ok(Command::SetNoteExpr {
        track,
        clip,
        edit,
        notes: old,
    })
}

/// The command that gives each of `notes` (in `clip` of `track`) the expression `f` makes
/// of its current one; `None` when nothing would change.
pub fn command_for(
    project: &Project,
    track: TrackId,
    clip: ClipId,
    ids: &[NoteId],
    edit: ExprEdit,
    mut f: impl FnMut(&crate::model::Note) -> Option<Box<NoteExpr>>,
) -> Option<Command> {
    // Arrangement and session clips alike (`Track::clip`, ADR 0076).
    let c = project.track(track)?.clip(clip)?;
    let mut notes = Vec::new();
    for id in ids {
        let Some(n) = c.note(*id) else { continue };
        let e = f(n).and_then(|e| (*e).normalised().boxed());
        if e != n.expr {
            notes.push((*id, e));
        }
    }
    (!notes.is_empty()).then_some(Command::SetNoteExpr {
        track,
        clip,
        edit,
        notes,
    })
}

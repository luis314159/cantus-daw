//! The note's articulation and bow (round 42, ADR 0158): how a note is played — sustained,
//! legato, staccato, spiccato, pizzicato, tremolo — and, for a bowed string, which way the
//! bow moves. Held as `Note::art`; `None` (every older note) lets the instrument decide.
//! Owned by the `strings-rec` builder; the contract declares the types and the command.
//!
//! `Command::SetNoteArt` sets the articulation of the notes it names; its inverse — the same
//! command with the old values — gives them back exactly.

use crate::command::{edit_shared, Command, CommandError};
use crate::model::{Clip, ClipId, NoteId, Project, TrackId};
use serde::{Deserialize, Serialize};

/// How a note is played. **Append only, never reorder** (a song stores the name).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Articulation {
    Sustain,
    Legato,
    Staccato,
    Spiccato,
    Pizzicato,
    Tremolo,
}

impl Articulation {
    pub const ALL: [Articulation; 6] = [
        Articulation::Sustain,
        Articulation::Legato,
        Articulation::Staccato,
        Articulation::Spiccato,
        Articulation::Pizzicato,
        Articulation::Tremolo,
    ];

    /// Its locale key (`areas/strings_rec.*`).
    pub fn name_key(self) -> &'static str {
        match self {
            Articulation::Sustain => "art.sustain",
            Articulation::Legato => "art.legato",
            Articulation::Staccato => "art.staccato",
            Articulation::Spiccato => "art.spiccato",
            Articulation::Pizzicato => "art.pizzicato",
            Articulation::Tremolo => "art.tremolo",
        }
    }
}

/// Which way the bow moves on a bowed note.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Bow {
    Down,
    Up,
}

/// A note's articulation, and its bow when it is bowed (`None`: the instrument alternates).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NoteArt {
    pub articulation: Articulation,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bow: Option<Bow>,
}

/// Apply `Command::SetNoteArt`: refused whole (nothing changed) for a note that is not in the
/// clip or a note named twice. The inverse holds the old values.
pub(crate) fn apply(
    project: &mut Project,
    track: TrackId,
    clip: ClipId,
    arts: Vec<(NoteId, Option<NoteArt>)>,
) -> Result<Command, CommandError> {
    for (i, (id, _)) in arts.iter().enumerate() {
        if arts[..i].iter().any(|(o, _)| o == id) {
            return Err(CommandError::DuplicateId(id.0));
        }
    }
    let old = edit_shared(project, track, clip, |c: &mut Clip| {
        for (id, _) in &arts {
            if c.note(*id).is_none() {
                return Err(CommandError::NoteNotFound(*id));
            }
        }
        let mut old = Vec::with_capacity(arts.len());
        for (id, a) in &arts {
            if let Some(n) = c.note_mut(*id) {
                old.push((*id, std::mem::replace(&mut n.art, *a)));
            }
        }
        Ok(old)
    })?;
    Ok(Command::SetNoteArt {
        track,
        clip,
        arts: old,
    })
}

//! r39:arrange — the arrangement's structure (round 39, ADR 0113): **sections** (named spans
//! of the song with their length, a lane above the tracks), **folders** (a named, coloured
//! row that groups tracks and folds them away) and **a mode per automation lane** (a lane
//! may read while its neighbour touches). None of it changes what renders: a folder's mute
//! and solo are its members' own, a section is a name over bars, and a lane's mode is what a
//! write pass does there. `Command::Arrange` carries each edit; each has an exact inverse.

use crate::automation::{AutoMode, AutoTarget};
use crate::command::{Command, CommandError};
use crate::model::{Color, Project, TrackId};
use crate::time::Ticks;
use serde::{Deserialize, Serialize};

/// A stable section id (from the project's id counter).
#[derive(
    Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct SectionId(pub u64);

/// A named span of the song: "Verse", 16 bars from bar 9. Sections never overlap.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Section {
    pub id: SectionId,
    pub start: Ticks,
    pub length: Ticks,
    pub name: String,
    /// From the track palette; `None` draws it in the lane's own ink.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<Color>,
}

impl Section {
    pub fn end(&self) -> Ticks {
        self.start + self.length
    }

    /// Whether `tick` falls inside the span.
    pub fn contains(&self, tick: Ticks) -> bool {
        tick >= self.start && tick < self.end()
    }
}

/// A stable folder id (from the project's id counter).
#[derive(
    Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct FolderId(pub u64);

/// A folder of tracks: a row of its own above its members ([`crate::Track::folder`]),
/// open (its members shown under it) or closed (one summary row).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TrackFolder {
    pub id: FolderId,
    pub name: String,
    pub color: Color,
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub open: bool,
}

fn yes() -> bool {
    true
}

fn is_true(b: &bool) -> bool {
    *b
}

/// One automation lane's own mode, over its track's ([`crate::Track::auto_mode`]).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LaneMode {
    pub target: AutoTarget,
    pub mode: AutoMode,
}

/// The mode a lane of `track` driving `target` writes in: its own, else the track's.
pub fn lane_mode(track: &crate::model::Track, target: &AutoTarget) -> AutoMode {
    track
        .lane_modes
        .iter()
        .find(|l| l.target == *target)
        .map_or(track.auto_mode, |l| l.mode)
}

/// The arrangement's edits (ADR 0113).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ArrangeCommand {
    /// Add a section at `index` of the list (refused where it would overlap another).
    InsertSection {
        index: usize,
        section: Section,
    },
    RemoveSection {
        id: SectionId,
    },
    /// A section's span, name or colour, whole (refused where it would overlap another).
    SetSection {
        section: Section,
    },
    InsertFolder {
        index: usize,
        folder: TrackFolder,
    },
    /// Remove a folder no track is in (empty it first: one batch).
    RemoveFolder {
        id: FolderId,
    },
    /// A folder's name, colour or open state, whole.
    SetFolder {
        folder: TrackFolder,
    },
    /// Put a track in a folder, or out of any.
    SetTrackFolder {
        track: TrackId,
        folder: Option<FolderId>,
    },
    /// A lane's own mode, or `None` to follow its track's.
    SetLaneMode {
        track: TrackId,
        target: AutoTarget,
        mode: Option<AutoMode>,
    },
    /// The icon of the user's choice before a track's name (a glyph id), or none.
    SetTrackGlyph {
        track: TrackId,
        glyph: Option<String>,
    },
}

/// The longest glyph id a track keeps (ids are short words: "kick", "vocal-female").
pub const GLYPH_ID_MAX: usize = 48;

impl ArrangeCommand {
    pub fn label_key(&self) -> &'static str {
        use ArrangeCommand::*;
        match self {
            InsertSection { .. } => "cmd.arrange.add_section",
            RemoveSection { .. } => "cmd.arrange.remove_section",
            SetSection { .. } => "cmd.arrange.set_section",
            InsertFolder { .. } => "cmd.arrange.add_folder",
            RemoveFolder { .. } => "cmd.arrange.remove_folder",
            SetFolder { .. } => "cmd.arrange.set_folder",
            SetTrackFolder { .. } => "cmd.arrange.set_track_folder",
            SetLaneMode { .. } => "cmd.arrange.set_lane_mode",
            SetTrackGlyph { .. } => "cmd.arrange.set_track_glyph",
        }
    }

    /// Whether `self` replaces `older` in a merged gesture (a section dragged, a folder
    /// renamed key by key).
    pub fn supersedes(&self, older: &ArrangeCommand) -> bool {
        use ArrangeCommand::*;
        match (self, older) {
            (SetSection { section: a }, SetSection { section: b }) => a.id == b.id,
            (SetFolder { folder: a }, SetFolder { folder: b }) => a.id == b.id,
            (SetTrackGlyph { track: a, .. }, SetTrackGlyph { track: b, .. }) => a == b,
            _ => false,
        }
    }
}

fn invalid(what: &str) -> CommandError {
    CommandError::InvalidValue(what.into())
}

/// A section that may stand in `project` beside the others (but `except`).
fn check_section(p: &Project, s: &Section, except: Option<SectionId>) -> Result<(), CommandError> {
    if s.start < Ticks::ZERO || s.length <= Ticks::ZERO {
        return Err(invalid("section span"));
    }
    let overlaps = p
        .sections
        .iter()
        .filter(|o| Some(o.id) != except)
        .any(|o| o.start < s.end() && s.start < o.end());
    if overlaps {
        return Err(invalid("sections overlap"));
    }
    Ok(())
}

/// Apply `c`; its inverse.
pub fn apply(c: ArrangeCommand, project: &mut Project) -> Result<Command, CommandError> {
    use ArrangeCommand::*;
    let inv = |c: ArrangeCommand| Ok(Command::Arrange(c));
    match c {
        InsertSection { index, section } => {
            let len = project.sections.len();
            if index > len {
                return Err(CommandError::IndexOutOfRange { index, len });
            }
            if project.sections.iter().any(|s| s.id == section.id) {
                return Err(CommandError::DuplicateId(section.id.0));
            }
            check_section(project, &section, None)?;
            let id = section.id;
            project.sections.insert(index, section);
            inv(RemoveSection { id })
        }
        RemoveSection { id } => {
            let index = section_index(project, id)?;
            let section = project.sections.remove(index);
            inv(InsertSection { index, section })
        }
        SetSection { section } => {
            let index = section_index(project, section.id)?;
            check_section(project, &section, Some(section.id))?;
            let old = std::mem::replace(&mut project.sections[index], section);
            inv(SetSection { section: old })
        }
        InsertFolder { index, folder } => {
            let len = project.folders.len();
            if index > len {
                return Err(CommandError::IndexOutOfRange { index, len });
            }
            if project.folders.iter().any(|f| f.id == folder.id) {
                return Err(CommandError::DuplicateId(folder.id.0));
            }
            let id = folder.id;
            project.folders.insert(index, folder);
            inv(RemoveFolder { id })
        }
        RemoveFolder { id } => {
            let index = folder_index(project, id)?;
            if project.tracks.iter().any(|t| t.folder == Some(id)) {
                return Err(invalid("folder has tracks"));
            }
            let folder = project.folders.remove(index);
            inv(InsertFolder { index, folder })
        }
        SetFolder { folder } => {
            let index = folder_index(project, folder.id)?;
            let old = std::mem::replace(&mut project.folders[index], folder);
            inv(SetFolder { folder: old })
        }
        SetTrackFolder { track, folder } => {
            if let Some(f) = folder {
                folder_index(project, f)?;
            }
            let t = project
                .track_mut(track)
                .ok_or(CommandError::TrackNotFound(track))?;
            let old = std::mem::replace(&mut t.folder, folder);
            inv(SetTrackFolder { track, folder: old })
        }
        SetLaneMode {
            track,
            target,
            mode,
        } => {
            let t = project
                .track_mut(track)
                .ok_or(CommandError::TrackNotFound(track))?;
            let at = t.lane_modes.iter().position(|l| l.target == target);
            let old = at.map(|i| t.lane_modes[i].mode);
            match (at, mode) {
                (Some(i), Some(m)) => t.lane_modes[i].mode = m,
                (Some(i), None) => {
                    t.lane_modes.remove(i);
                }
                (None, Some(m)) => t.lane_modes.push(LaneMode {
                    target: target.clone(),
                    mode: m,
                }),
                (None, None) => {}
            }
            inv(SetLaneMode {
                track,
                target,
                mode: old,
            })
        }
        SetTrackGlyph { track, glyph } => {
            let ok = |g: &str| {
                !g.is_empty()
                    && g.len() <= GLYPH_ID_MAX
                    && g.chars()
                        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
            };
            if glyph.as_deref().is_some_and(|g| !ok(g)) {
                return Err(invalid("glyph id"));
            }
            let t = project
                .track_mut(track)
                .ok_or(CommandError::TrackNotFound(track))?;
            let old = std::mem::replace(&mut t.glyph, glyph);
            inv(SetTrackGlyph { track, glyph: old })
        }
    }
}

fn section_index(p: &Project, id: SectionId) -> Result<usize, CommandError> {
    p.sections
        .iter()
        .position(|s| s.id == id)
        .ok_or_else(|| invalid("section"))
}

fn folder_index(p: &Project, id: FolderId) -> Result<usize, CommandError> {
    p.folders
        .iter()
        .position(|f| f.id == id)
        .ok_or_else(|| invalid("folder"))
}

impl Project {
    /// A section by id.
    pub fn section(&self, id: SectionId) -> Option<&Section> {
        self.sections.iter().find(|s| s.id == id)
    }

    /// A folder by id.
    pub fn folder(&self, id: FolderId) -> Option<&TrackFolder> {
        self.folders.iter().find(|f| f.id == id)
    }

    /// The section that holds `tick`, if any.
    pub fn section_at(&self, tick: Ticks) -> Option<&Section> {
        self.sections.iter().find(|s| s.contains(tick))
    }

    /// The tracks in `folder`, in the song's order.
    pub fn folder_members(&self, folder: FolderId) -> impl Iterator<Item = &crate::Track> {
        self.tracks.iter().filter(move |t| t.folder == Some(folder))
    }
}

#[cfg(test)]
#[path = "arrange_tests.rs"]
mod tests;

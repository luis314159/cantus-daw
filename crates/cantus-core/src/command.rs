//! The command layer (ADR 0004).
//!
//! A [`Command`] is the *only* way to mutate a [`Project`]. Applying one returns its
//! inverse, which is what makes undo/redo, auditing and AI reversibility fall out for free.

use crate::instrument::FirmusParam;
use crate::model::*;
use crate::time::{clamp_bpm, Ticks, TimeSignature};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Who issued a command. Semantics are identical; this only feeds auditing/permissions.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Origin {
    User,
    Script,
    Ai,
    Mcp,
    System,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum CommandError {
    #[error("track {0} not found")]
    TrackNotFound(TrackId),
    #[error("clip {0} not found")]
    ClipNotFound(ClipId),
    #[error("note {0} not found")]
    NoteNotFound(NoteId),
    #[error("index {index} out of range (len {len})")]
    IndexOutOfRange { index: usize, len: usize },
    #[error("duplicate id {0}")]
    DuplicateId(u64),
    #[error("invalid value: {0}")]
    InvalidValue(String),
    #[error("instrument on track {0} does not have that parameter")]
    ParamNotApplicable(TrackId),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Command {
    // --- project-wide -----------------------------------------------------------------
    SetProjectName {
        name: String,
    },
    SetTempo {
        bpm: f64,
    },
    SetTimeSignature {
        time_signature: TimeSignature,
    },
    SetLoopRegion {
        region: Option<LoopRegion>,
    },
    SetLoopEnabled {
        enabled: bool,
    },
    SetMetronome {
        enabled: bool,
    },
    SetMasterGain {
        gain_db: f32,
    },
    /// Replace the whole project (used by "open"). Inverse restores the previous one.
    ReplaceProject {
        project: Box<Project>,
    },

    // --- tracks -----------------------------------------------------------------------
    InsertTrack {
        index: usize,
        track: Box<Track>,
    },
    RemoveTrack {
        id: TrackId,
    },
    MoveTrack {
        id: TrackId,
        index: usize,
    },
    SetTrackName {
        id: TrackId,
        name: String,
    },
    SetTrackColor {
        id: TrackId,
        color: Color,
    },
    SetTrackGain {
        id: TrackId,
        gain_db: f32,
    },
    SetTrackPan {
        id: TrackId,
        pan: f32,
    },
    SetTrackMute {
        id: TrackId,
        muted: bool,
    },
    SetTrackSolo {
        id: TrackId,
        soloed: bool,
    },
    SetInstrumentParam {
        track: TrackId,
        param: FirmusParam,
        value: f32,
    },

    // --- clips ------------------------------------------------------------------------
    InsertClip {
        track: TrackId,
        clip: Box<Clip>,
    },
    RemoveClip {
        track: TrackId,
        clip: ClipId,
    },
    SetClipBounds {
        track: TrackId,
        clip: ClipId,
        start: Ticks,
        length: Ticks,
    },
    SetClipName {
        track: TrackId,
        clip: ClipId,
        name: String,
    },

    // --- notes ------------------------------------------------------------------------
    InsertNotes {
        track: TrackId,
        clip: ClipId,
        notes: Vec<Note>,
    },
    RemoveNotes {
        track: TrackId,
        clip: ClipId,
        ids: Vec<NoteId>,
    },
    /// Replace notes wholesale, matched by id. Used for move/resize/velocity edits.
    SetNotes {
        track: TrackId,
        clip: ClipId,
        notes: Vec<Note>,
    },

    /// Several commands as one undo step.
    Batch {
        label_key: String,
        commands: Vec<Command>,
    },
}

impl Command {
    /// i18n key describing the command for the undo menu.
    pub fn label_key(&self) -> &str {
        use Command::*;
        match self {
            SetProjectName { .. } => "cmd.set_project_name",
            SetTempo { .. } => "cmd.set_tempo",
            SetTimeSignature { .. } => "cmd.set_time_signature",
            SetLoopRegion { .. } => "cmd.set_loop_region",
            SetLoopEnabled { .. } => "cmd.set_loop_enabled",
            SetMetronome { .. } => "cmd.set_metronome",
            SetMasterGain { .. } => "cmd.set_master_gain",
            ReplaceProject { .. } => "cmd.replace_project",
            InsertTrack { .. } => "cmd.insert_track",
            RemoveTrack { .. } => "cmd.remove_track",
            MoveTrack { .. } => "cmd.move_track",
            SetTrackName { .. } => "cmd.set_track_name",
            SetTrackColor { .. } => "cmd.set_track_color",
            SetTrackGain { .. } => "cmd.set_track_gain",
            SetTrackPan { .. } => "cmd.set_track_pan",
            SetTrackMute { .. } => "cmd.set_track_mute",
            SetTrackSolo { .. } => "cmd.set_track_solo",
            SetInstrumentParam { .. } => "cmd.set_instrument_param",
            InsertClip { .. } => "cmd.insert_clip",
            RemoveClip { .. } => "cmd.remove_clip",
            SetClipBounds { .. } => "cmd.set_clip_bounds",
            SetClipName { .. } => "cmd.set_clip_name",
            InsertNotes { .. } => "cmd.insert_notes",
            RemoveNotes { .. } => "cmd.remove_notes",
            SetNotes { .. } => "cmd.set_notes",
            Batch { label_key, .. } => label_key,
        }
    }

    /// Apply to `project`, returning the inverse command. On error the project is
    /// unchanged (batches roll back).
    pub fn apply(self, project: &mut Project) -> Result<Command, CommandError> {
        use Command::*;
        match self {
            SetProjectName { name } => {
                let old = std::mem::replace(&mut project.name, name);
                Ok(SetProjectName { name: old })
            }
            SetTempo { bpm } => {
                let old = project.tempo_bpm;
                project.tempo_bpm = clamp_bpm(bpm);
                Ok(SetTempo { bpm: old })
            }
            SetTimeSignature { time_signature } => {
                if time_signature.numerator == 0
                    || !matches!(time_signature.denominator, 1 | 2 | 4 | 8 | 16 | 32)
                {
                    return Err(CommandError::InvalidValue(format!(
                        "time signature {time_signature}"
                    )));
                }
                let old = std::mem::replace(&mut project.time_signature, time_signature);
                Ok(SetTimeSignature {
                    time_signature: old,
                })
            }
            SetLoopRegion { region } => {
                if let Some(r) = &region {
                    if r.end <= r.start || r.start.is_negative() {
                        return Err(CommandError::InvalidValue("loop region".into()));
                    }
                }
                let old = std::mem::replace(&mut project.loop_region, region);
                Ok(SetLoopRegion { region: old })
            }
            SetLoopEnabled { enabled } => {
                let old = std::mem::replace(&mut project.loop_enabled, enabled);
                Ok(SetLoopEnabled { enabled: old })
            }
            SetMetronome { enabled } => {
                let old = std::mem::replace(&mut project.metronome, enabled);
                Ok(SetMetronome { enabled: old })
            }
            SetMasterGain { gain_db } => {
                let old = project.master.gain_db;
                project.master.gain_db = clamp_gain(gain_db)?;
                Ok(SetMasterGain { gain_db: old })
            }
            ReplaceProject { project: new } => {
                let old = std::mem::replace(project, *new);
                Ok(ReplaceProject {
                    project: Box::new(old),
                })
            }

            InsertTrack { index, track } => {
                let len = project.tracks.len();
                if index > len {
                    return Err(CommandError::IndexOutOfRange { index, len });
                }
                if project.track(track.id).is_some() {
                    return Err(CommandError::DuplicateId(track.id.0));
                }
                let id = track.id;
                project.tracks.insert(index, *track);
                Ok(RemoveTrack { id })
            }
            RemoveTrack { id } => {
                let index = project
                    .track_index(id)
                    .ok_or(CommandError::TrackNotFound(id))?;
                let track = project.tracks.remove(index);
                Ok(InsertTrack {
                    index,
                    track: Box::new(track),
                })
            }
            MoveTrack { id, index } => {
                let from = project
                    .track_index(id)
                    .ok_or(CommandError::TrackNotFound(id))?;
                let len = project.tracks.len();
                if index >= len {
                    return Err(CommandError::IndexOutOfRange { index, len });
                }
                let t = project.tracks.remove(from);
                project.tracks.insert(index, t);
                Ok(MoveTrack { id, index: from })
            }
            SetTrackName { id, name } => {
                let t = project
                    .track_mut(id)
                    .ok_or(CommandError::TrackNotFound(id))?;
                let old = std::mem::replace(&mut t.name, name);
                Ok(SetTrackName { id, name: old })
            }
            SetTrackColor { id, color } => {
                let t = project
                    .track_mut(id)
                    .ok_or(CommandError::TrackNotFound(id))?;
                let old = std::mem::replace(&mut t.color, color);
                Ok(SetTrackColor { id, color: old })
            }
            SetTrackGain { id, gain_db } => {
                let gain_db = clamp_gain(gain_db)?;
                let t = project
                    .track_mut(id)
                    .ok_or(CommandError::TrackNotFound(id))?;
                let old = std::mem::replace(&mut t.gain_db, gain_db);
                Ok(SetTrackGain { id, gain_db: old })
            }
            SetTrackPan { id, pan } => {
                if !pan.is_finite() {
                    return Err(CommandError::InvalidValue("pan".into()));
                }
                let t = project
                    .track_mut(id)
                    .ok_or(CommandError::TrackNotFound(id))?;
                let old = std::mem::replace(&mut t.pan, pan.clamp(-1.0, 1.0));
                Ok(SetTrackPan { id, pan: old })
            }
            SetTrackMute { id, muted } => {
                let t = project
                    .track_mut(id)
                    .ok_or(CommandError::TrackNotFound(id))?;
                let old = std::mem::replace(&mut t.muted, muted);
                Ok(SetTrackMute { id, muted: old })
            }
            SetTrackSolo { id, soloed } => {
                let t = project
                    .track_mut(id)
                    .ok_or(CommandError::TrackNotFound(id))?;
                let old = std::mem::replace(&mut t.soloed, soloed);
                Ok(SetTrackSolo { id, soloed: old })
            }
            SetInstrumentParam {
                track,
                param,
                value,
            } => {
                let t = project
                    .track_mut(track)
                    .ok_or(CommandError::TrackNotFound(track))?;
                let p = t
                    .instrument
                    .firmus_mut()
                    .ok_or(CommandError::ParamNotApplicable(track))?;
                let old = param.get(p);
                param.set(p, value);
                Ok(SetInstrumentParam {
                    track,
                    param,
                    value: old,
                })
            }

            InsertClip { track, clip } => {
                validate_clip(&clip)?;
                let t = project
                    .track_mut(track)
                    .ok_or(CommandError::TrackNotFound(track))?;
                if t.clip(clip.id).is_some() {
                    return Err(CommandError::DuplicateId(clip.id.0));
                }
                let id = clip.id;
                t.clips.push(*clip);
                t.clips.sort_by_key(|c| c.start);
                Ok(RemoveClip { track, clip: id })
            }
            RemoveClip { track, clip } => {
                let t = project
                    .track_mut(track)
                    .ok_or(CommandError::TrackNotFound(track))?;
                let idx = t
                    .clips
                    .iter()
                    .position(|c| c.id == clip)
                    .ok_or(CommandError::ClipNotFound(clip))?;
                let removed = t.clips.remove(idx);
                Ok(InsertClip {
                    track,
                    clip: Box::new(removed),
                })
            }
            SetClipBounds {
                track,
                clip,
                start,
                length,
            } => {
                if start.is_negative() || length <= Ticks::ZERO {
                    return Err(CommandError::InvalidValue("clip bounds".into()));
                }
                let t = project
                    .track_mut(track)
                    .ok_or(CommandError::TrackNotFound(track))?;
                let c = t.clip_mut(clip).ok_or(CommandError::ClipNotFound(clip))?;
                let (old_start, old_len) = (c.start, c.length);
                c.start = start;
                c.length = length;
                t.clips.sort_by_key(|c| c.start);
                Ok(SetClipBounds {
                    track,
                    clip,
                    start: old_start,
                    length: old_len,
                })
            }
            SetClipName { track, clip, name } => {
                let t = project
                    .track_mut(track)
                    .ok_or(CommandError::TrackNotFound(track))?;
                let c = t.clip_mut(clip).ok_or(CommandError::ClipNotFound(clip))?;
                let old = std::mem::replace(&mut c.name, name);
                Ok(SetClipName {
                    track,
                    clip,
                    name: old,
                })
            }

            InsertNotes { track, clip, notes } => {
                for n in &notes {
                    validate_note(n)?;
                }
                let t = project
                    .track_mut(track)
                    .ok_or(CommandError::TrackNotFound(track))?;
                let c = t.clip_mut(clip).ok_or(CommandError::ClipNotFound(clip))?;
                for n in &notes {
                    if c.note(n.id).is_some() {
                        return Err(CommandError::DuplicateId(n.id.0));
                    }
                }
                let ids = notes.iter().map(|n| n.id).collect();
                c.notes.extend(notes);
                c.notes.sort_by_key(|n| (n.start, n.pitch));
                Ok(RemoveNotes { track, clip, ids })
            }
            RemoveNotes { track, clip, ids } => {
                let t = project
                    .track_mut(track)
                    .ok_or(CommandError::TrackNotFound(track))?;
                let c = t.clip_mut(clip).ok_or(CommandError::ClipNotFound(clip))?;
                for id in &ids {
                    if c.note(*id).is_none() {
                        return Err(CommandError::NoteNotFound(*id));
                    }
                }
                let mut removed = Vec::with_capacity(ids.len());
                c.notes.retain(|n| {
                    if ids.contains(&n.id) {
                        removed.push(n.clone());
                        false
                    } else {
                        true
                    }
                });
                Ok(InsertNotes {
                    track,
                    clip,
                    notes: removed,
                })
            }
            SetNotes { track, clip, notes } => {
                for n in &notes {
                    validate_note(n)?;
                }
                let t = project
                    .track_mut(track)
                    .ok_or(CommandError::TrackNotFound(track))?;
                let c = t.clip_mut(clip).ok_or(CommandError::ClipNotFound(clip))?;
                let mut old = Vec::with_capacity(notes.len());
                for n in &notes {
                    old.push(
                        c.note(n.id)
                            .cloned()
                            .ok_or(CommandError::NoteNotFound(n.id))?,
                    );
                }
                for n in notes {
                    if let Some(slot) = c.note_mut(n.id) {
                        *slot = n;
                    }
                }
                c.notes.sort_by_key(|n| (n.start, n.pitch));
                Ok(SetNotes {
                    track,
                    clip,
                    notes: old,
                })
            }

            Batch {
                label_key,
                commands,
            } => {
                let mut inverses = Vec::with_capacity(commands.len());
                for cmd in commands {
                    match cmd.apply(project) {
                        Ok(inv) => inverses.push(inv),
                        Err(e) => {
                            // Roll back what was applied so far.
                            while let Some(inv) = inverses.pop() {
                                let _ = inv.apply(project);
                            }
                            return Err(e);
                        }
                    }
                }
                inverses.reverse();
                Ok(Batch {
                    label_key,
                    commands: inverses,
                })
            }
        }
    }
}

fn clamp_gain(db: f32) -> Result<f32, CommandError> {
    if !db.is_finite() {
        return Err(CommandError::InvalidValue("gain".into()));
    }
    Ok(db.clamp(MIN_GAIN_DB, MAX_GAIN_DB))
}

fn validate_clip(c: &Clip) -> Result<(), CommandError> {
    if c.start.is_negative() || c.length <= Ticks::ZERO {
        return Err(CommandError::InvalidValue("clip bounds".into()));
    }
    for n in &c.notes {
        validate_note(n)?;
    }
    Ok(())
}

fn validate_note(n: &Note) -> Result<(), CommandError> {
    if n.length <= Ticks::ZERO {
        return Err(CommandError::InvalidValue(format!("note {} length", n.id)));
    }
    if n.pitch > 127 {
        return Err(CommandError::InvalidValue(format!("note {} pitch", n.id)));
    }
    if n.velocity == 0 || n.velocity > 127 {
        return Err(CommandError::InvalidValue(format!(
            "note {} velocity",
            n.id
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project_with_track() -> (Project, TrackId, ClipId) {
        let mut p = Project::default();
        let track = p.make_track("Lead");
        let tid = track.id;
        Command::InsertTrack {
            index: 0,
            track: Box::new(track),
        }
        .apply(&mut p)
        .unwrap();
        let clip = p.make_clip("Clip", Ticks::ZERO, Ticks::beats(4));
        let cid = clip.id;
        Command::InsertClip {
            track: tid,
            clip: Box::new(clip),
        }
        .apply(&mut p)
        .unwrap();
        (p, tid, cid)
    }

    #[test]
    fn inverse_restores_state() {
        let (mut p, tid, cid) = project_with_track();
        let before = p.clone();
        let note = p.make_note(Ticks::ZERO, Ticks::QUARTER, 60, 100);
        let before_alloc = p.clone();
        let inv = Command::InsertNotes {
            track: tid,
            clip: cid,
            notes: vec![note],
        }
        .apply(&mut p)
        .unwrap();
        assert_ne!(p, before_alloc);
        inv.apply(&mut p).unwrap();
        assert_eq!(p, before_alloc);
        // Allocating an id is not a command and is not undone — by design.
        assert_ne!(p, before);
    }

    #[test]
    fn batch_rolls_back_on_error() {
        let (mut p, tid, cid) = project_with_track();
        let before = p.clone();
        let result = Command::Batch {
            label_key: "test".into(),
            commands: vec![
                Command::SetTempo { bpm: 90.0 },
                Command::RemoveNotes {
                    track: tid,
                    clip: cid,
                    ids: vec![NoteId(999)],
                },
            ],
        }
        .apply(&mut p);
        assert!(matches!(result, Err(CommandError::NoteNotFound(_))));
        assert_eq!(p, before);
    }

    #[test]
    fn remove_track_inverse_keeps_position() {
        let mut p = Project::default();
        for name in ["a", "b", "c"] {
            let t = p.make_track(name);
            let idx = p.tracks.len();
            Command::InsertTrack {
                index: idx,
                track: Box::new(t),
            }
            .apply(&mut p)
            .unwrap();
        }
        let before = p.clone();
        let id = p.tracks[1].id;
        let inv = Command::RemoveTrack { id }.apply(&mut p).unwrap();
        assert_eq!(p.tracks.len(), 2);
        inv.apply(&mut p).unwrap();
        assert_eq!(p, before);
    }

    #[test]
    fn rejects_bad_values() {
        let (mut p, tid, _) = project_with_track();
        assert!(Command::SetTrackGain {
            id: tid,
            gain_db: f32::NAN
        }
        .apply(&mut p)
        .is_err());
        assert!(Command::SetTrackGain {
            id: TrackId(4242),
            gain_db: 0.0
        }
        .apply(&mut p)
        .is_err());
        assert_eq!(
            Command::SetTempo { bpm: 5000.0 }
                .apply(&mut p)
                .ok()
                .map(|_| p.tempo_bpm),
            Some(999.0)
        );
    }
}

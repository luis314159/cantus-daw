//! The command layer (ADR 0004).
//!
//! A [`Command`] is the *only* way to mutate a [`Project`]. Applying one returns its
//! inverse, which is what makes undo/redo, auditing and AI reversibility fall out for free.

use crate::effect::EffectSlot;
use crate::instrument::{InstrumentParam, InstrumentSpec};
use crate::model::*;
use crate::pads::DrumPad;
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
    #[error("effect {0} not found")]
    EffectNotFound(EffectId),
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
    SetMasterPan {
        pan: f32,
    },
    SetMasterWidth {
        width: f32,
    },
    SetMasterPhaseInvert {
        invert: bool,
    },
    /// Project-wide, because it changes what the project renders as (ADR 0012).
    SetPanLaw {
        law: PanLaw,
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
    /// 0 is mono, 1 as recorded, 2 twice as wide.
    SetTrackWidth {
        id: TrackId,
        width: f32,
    },
    SetTrackPhaseInvert {
        id: TrackId,
        invert: bool,
    },
    /// How tall the track is drawn, as a multiple of the base row. A view thing, and a
    /// command all the same: it is stored in the project, so it is undoable like the rest.
    SetTrackHeight {
        id: TrackId,
        height: f32,
    },
    /// `None` puts the track back on the icon its instrument implies.
    SetTrackIcon {
        id: TrackId,
        icon: Option<TrackIcon>,
    },
    SetTrackMute {
        id: TrackId,
        muted: bool,
    },
    SetTrackSolo {
        id: TrackId,
        soloed: bool,
    },
    /// Replace the track's instrument wholesale — a synth becomes a kit, or the other way.
    /// The clips stay: a note is a pitch either way, and the editors' lanes follow the new
    /// instrument (`InstrumentSpec::lanes`).
    SetInstrument {
        track: TrackId,
        spec: Box<InstrumentSpec>,
    },
    /// Add a pad to a kit. Pads are kept sorted by pitch and a pitch can hold one pad, so
    /// this fails rather than shadowing an existing one.
    InsertPad {
        track: TrackId,
        pad: Box<DrumPad>,
    },
    RemovePad {
        track: TrackId,
        pitch: Pitch,
    },
    /// Set one parameter of whatever instrument the track hosts.
    ///
    /// The address is an [`InstrumentParam`], not a `FirmusParam`: a second instrument
    /// must not need a second command, or every later one needs another.
    SetInstrumentParam {
        track: TrackId,
        param: InstrumentParam,
        value: f32,
    },
    /// Rename a pad of a kit. The name lives **in the kit**, so this renames the lane in
    /// every clip on the track at once — which is the point of a pad having a name at all.
    SetPadName {
        track: TrackId,
        pad: u8,
        name: String,
    },
    /// Point a pad at a sample file (or at none). The pad's voice is set separately, so a
    /// pad can hold on to its file while it is switched back to a synthesized voice.
    SetPadSample {
        track: TrackId,
        pad: u8,
        path: Option<String>,
    },

    // --- effects ----------------------------------------------------------------------
    InsertEffect {
        track: TrackId,
        index: usize,
        slot: Box<EffectSlot>,
    },
    RemoveEffect {
        track: TrackId,
        effect: EffectId,
    },
    MoveEffect {
        track: TrackId,
        effect: EffectId,
        index: usize,
    },
    SetEffectBypass {
        track: TrackId,
        effect: EffectId,
        bypass: bool,
    },
    SetEffectParam {
        track: TrackId,
        effect: EffectId,
        key: String,
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
    /// Slip: move the content under the window without moving the window.
    SetClipOffset {
        track: TrackId,
        clip: ClipId,
        offset: Ticks,
    },
    /// Turn the clip's repeat on (`Some(period)`) or off (`None`).
    SetClipLoop {
        track: TrackId,
        clip: ClipId,
        content_length: Option<Ticks>,
    },
    SetClipMute {
        track: TrackId,
        clip: ClipId,
        muted: bool,
    },
    /// `None` puts the clip back on the track's colour.
    SetClipColor {
        track: TrackId,
        clip: ClipId,
        color: Option<Color>,
    },
    /// Move a clip to another track, keeping its id — so a drag across tracks is one
    /// reversible step and the selection survives it.
    MoveClipToTrack {
        from: TrackId,
        to: TrackId,
        clip: ClipId,
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
            SetMasterPan { .. } => "cmd.set_master_pan",
            SetMasterWidth { .. } => "cmd.set_master_width",
            SetMasterPhaseInvert { .. } => "cmd.set_master_phase",
            SetPanLaw { .. } => "cmd.set_pan_law",
            ReplaceProject { .. } => "cmd.replace_project",
            InsertTrack { .. } => "cmd.insert_track",
            RemoveTrack { .. } => "cmd.remove_track",
            MoveTrack { .. } => "cmd.move_track",
            SetTrackName { .. } => "cmd.set_track_name",
            SetTrackColor { .. } => "cmd.set_track_color",
            SetTrackGain { .. } => "cmd.set_track_gain",
            SetTrackPan { .. } => "cmd.set_track_pan",
            SetTrackWidth { .. } => "cmd.set_track_width",
            SetTrackPhaseInvert { .. } => "cmd.set_track_phase",
            SetTrackHeight { .. } => "cmd.set_track_height",
            SetTrackIcon { .. } => "cmd.set_track_icon",
            SetTrackMute { .. } => "cmd.set_track_mute",
            SetTrackSolo { .. } => "cmd.set_track_solo",
            SetInstrument { .. } => "cmd.set_instrument",
            InsertPad { .. } => "cmd.insert_pad",
            RemovePad { .. } => "cmd.remove_pad",
            SetInstrumentParam { .. } => "cmd.set_instrument_param",
            SetPadName { .. } => "cmd.set_pad_name",
            SetPadSample { .. } => "cmd.set_pad_sample",
            InsertEffect { .. } => "cmd.insert_effect",
            RemoveEffect { .. } => "cmd.remove_effect",
            MoveEffect { .. } => "cmd.move_effect",
            SetEffectBypass { .. } => "cmd.set_effect_bypass",
            SetEffectParam { .. } => "cmd.set_effect_param",
            InsertClip { .. } => "cmd.insert_clip",
            RemoveClip { .. } => "cmd.remove_clip",
            SetClipBounds { .. } => "cmd.set_clip_bounds",
            SetClipName { .. } => "cmd.set_clip_name",
            SetClipOffset { .. } => "cmd.set_clip_offset",
            SetClipLoop { .. } => "cmd.set_clip_loop",
            SetClipMute { .. } => "cmd.set_clip_mute",
            SetClipColor { .. } => "cmd.set_clip_color",
            MoveClipToTrack { .. } => "cmd.move_clip_to_track",
            InsertNotes { .. } => "cmd.insert_notes",
            RemoveNotes { .. } => "cmd.remove_notes",
            SetNotes { .. } => "cmd.set_notes",
            Batch { label_key, .. } => label_key,
        }
    }

    /// `true` when `self` sets the same parameter as `older`, so in a merged gesture the
    /// newer value simply replaces the older one (a fader drag). Anything else — inserts,
    /// removals, different targets — accumulates instead.
    pub fn supersedes(&self, older: &Command) -> bool {
        use Command::*;
        match (self, older) {
            (SetProjectName { .. }, SetProjectName { .. })
            | (SetTempo { .. }, SetTempo { .. })
            | (SetTimeSignature { .. }, SetTimeSignature { .. })
            | (SetLoopRegion { .. }, SetLoopRegion { .. })
            | (SetLoopEnabled { .. }, SetLoopEnabled { .. })
            | (SetMetronome { .. }, SetMetronome { .. })
            | (SetMasterGain { .. }, SetMasterGain { .. })
            | (SetMasterPan { .. }, SetMasterPan { .. })
            | (SetMasterWidth { .. }, SetMasterWidth { .. })
            | (SetMasterPhaseInvert { .. }, SetMasterPhaseInvert { .. })
            | (SetPanLaw { .. }, SetPanLaw { .. }) => true,
            (SetTrackName { id: a, .. }, SetTrackName { id: b, .. })
            | (SetTrackColor { id: a, .. }, SetTrackColor { id: b, .. })
            | (SetTrackGain { id: a, .. }, SetTrackGain { id: b, .. })
            | (SetTrackPan { id: a, .. }, SetTrackPan { id: b, .. })
            | (SetTrackWidth { id: a, .. }, SetTrackWidth { id: b, .. })
            | (SetTrackPhaseInvert { id: a, .. }, SetTrackPhaseInvert { id: b, .. })
            | (SetTrackMute { id: a, .. }, SetTrackMute { id: b, .. })
            | (SetTrackSolo { id: a, .. }, SetTrackSolo { id: b, .. }) => a == b,
            (SetTrackHeight { id: a, .. }, SetTrackHeight { id: b, .. }) => a == b,
            (
                SetInstrumentParam {
                    track: a,
                    param: pa,
                    ..
                },
                SetInstrumentParam {
                    track: b,
                    param: pb,
                    ..
                },
            ) => a == b && pa == pb,
            (
                SetEffectBypass {
                    track: a,
                    effect: ea,
                    ..
                },
                SetEffectBypass {
                    track: b,
                    effect: eb,
                    ..
                },
            ) => a == b && ea == eb,
            (
                SetEffectParam {
                    track: a,
                    effect: ea,
                    key: ka,
                    ..
                },
                SetEffectParam {
                    track: b,
                    effect: eb,
                    key: kb,
                    ..
                },
            ) => a == b && ea == eb && ka == kb,
            (
                SetClipBounds {
                    track: a, clip: ca, ..
                },
                SetClipBounds {
                    track: b, clip: cb, ..
                },
            )
            | (
                SetClipName {
                    track: a, clip: ca, ..
                },
                SetClipName {
                    track: b, clip: cb, ..
                },
            )
            | (
                SetClipOffset {
                    track: a, clip: ca, ..
                },
                SetClipOffset {
                    track: b, clip: cb, ..
                },
            ) => a == b && ca == cb,
            (
                SetNotes {
                    track: a,
                    clip: ca,
                    notes: na,
                },
                SetNotes {
                    track: b,
                    clip: cb,
                    notes: nb,
                },
            ) => {
                a == b
                    && ca == cb
                    && na.len() == nb.len()
                    && na.iter().all(|n| nb.iter().any(|m| m.id == n.id))
            }
            _ => false,
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
            SetMasterPan { pan } => {
                let old = std::mem::replace(&mut project.master.pan, clamp_pan(pan)?);
                Ok(SetMasterPan { pan: old })
            }
            SetMasterWidth { width } => {
                let old = std::mem::replace(&mut project.master.width, clamp_width(width)?);
                Ok(SetMasterWidth { width: old })
            }
            SetMasterPhaseInvert { invert } => {
                let old = std::mem::replace(&mut project.master.phase_invert, invert);
                Ok(SetMasterPhaseInvert { invert: old })
            }
            SetPanLaw { law } => {
                let old = std::mem::replace(&mut project.pan_law, law);
                Ok(SetPanLaw { law: old })
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
            SetTrackWidth { id, width } => {
                let width = clamp_width(width)?;
                let t = project
                    .track_mut(id)
                    .ok_or(CommandError::TrackNotFound(id))?;
                let old = std::mem::replace(&mut t.width, width);
                Ok(SetTrackWidth { id, width: old })
            }
            SetTrackPhaseInvert { id, invert } => {
                let t = project
                    .track_mut(id)
                    .ok_or(CommandError::TrackNotFound(id))?;
                let old = std::mem::replace(&mut t.phase_invert, invert);
                Ok(SetTrackPhaseInvert { id, invert: old })
            }
            SetTrackHeight { id, height } => {
                let t = project
                    .track_mut(id)
                    .ok_or(CommandError::TrackNotFound(id))?;
                let old = t.height;
                t.height = if height.is_finite() {
                    height.clamp(MIN_TRACK_HEIGHT, MAX_TRACK_HEIGHT)
                } else {
                    1.0
                };
                Ok(SetTrackHeight { id, height: old })
            }

            SetTrackIcon { id, icon } => {
                let t = project
                    .track_mut(id)
                    .ok_or(CommandError::TrackNotFound(id))?;
                let old = t.icon;
                t.icon = icon;
                Ok(SetTrackIcon { id, icon: old })
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
                let old = t
                    .instrument
                    .param(param)
                    .ok_or(CommandError::ParamNotApplicable(track))?;
                t.instrument
                    .set_param(param, value)
                    .ok_or(CommandError::ParamNotApplicable(track))?;
                Ok(SetInstrumentParam {
                    track,
                    param,
                    value: old,
                })
            }

            SetInstrument { track, spec } => {
                let t = project
                    .track_mut(track)
                    .ok_or(CommandError::TrackNotFound(track))?;
                let old = std::mem::replace(&mut t.instrument, *spec);
                Ok(SetInstrument {
                    track,
                    spec: Box::new(old),
                })
            }

            InsertPad { track, pad } => {
                let t = project
                    .track_mut(track)
                    .ok_or(CommandError::TrackNotFound(track))?;
                let kit = t
                    .instrument
                    .pulsus_mut()
                    .ok_or(CommandError::ParamNotApplicable(track))?;
                if kit.index_of_pitch(pad.pitch).is_some() {
                    return Err(CommandError::DuplicateId(pad.pitch as u64));
                }
                let pitch = pad.pitch;
                let at = kit.pads.partition_point(|p| p.pitch < pitch);
                kit.pads.insert(at, *pad);
                Ok(RemovePad { track, pitch })
            }

            RemovePad { track, pitch } => {
                let t = project
                    .track_mut(track)
                    .ok_or(CommandError::TrackNotFound(track))?;
                let kit = t
                    .instrument
                    .pulsus_mut()
                    .ok_or(CommandError::ParamNotApplicable(track))?;
                let at = kit
                    .index_of_pitch(pitch)
                    .ok_or(CommandError::ParamNotApplicable(track))?;
                let pad = kit.pads.remove(at);
                Ok(InsertPad {
                    track,
                    pad: Box::new(pad),
                })
            }

            SetPadName { track, pad, name } => {
                let t = project
                    .track_mut(track)
                    .ok_or(CommandError::TrackNotFound(track))?;
                let kit = t
                    .instrument
                    .pulsus_mut()
                    .ok_or(CommandError::ParamNotApplicable(track))?;
                let slot = kit
                    .pad_mut(pad as usize)
                    .ok_or(CommandError::ParamNotApplicable(track))?;
                let old = std::mem::replace(&mut slot.name, name);
                Ok(SetPadName {
                    track,
                    pad,
                    name: old,
                })
            }

            SetPadSample { track, pad, path } => {
                let t = project
                    .track_mut(track)
                    .ok_or(CommandError::TrackNotFound(track))?;
                let kit = t
                    .instrument
                    .pulsus_mut()
                    .ok_or(CommandError::ParamNotApplicable(track))?;
                let slot = kit
                    .pad_mut(pad as usize)
                    .ok_or(CommandError::ParamNotApplicable(track))?;
                let old = std::mem::replace(&mut slot.sample, path);
                Ok(SetPadSample {
                    track,
                    pad,
                    path: old,
                })
            }

            InsertEffect { track, index, slot } => {
                let t = project
                    .track_mut(track)
                    .ok_or(CommandError::TrackNotFound(track))?;
                let len = t.effects.len();
                if index > len {
                    return Err(CommandError::IndexOutOfRange { index, len });
                }
                if t.effect(slot.id).is_some() {
                    return Err(CommandError::DuplicateId(slot.id.0));
                }
                let id = slot.id;
                t.effects.insert(index, *slot);
                Ok(RemoveEffect { track, effect: id })
            }
            RemoveEffect { track, effect } => {
                let t = project
                    .track_mut(track)
                    .ok_or(CommandError::TrackNotFound(track))?;
                let index = t
                    .effect_index(effect)
                    .ok_or(CommandError::EffectNotFound(effect))?;
                let slot = t.effects.remove(index);
                Ok(InsertEffect {
                    track,
                    index,
                    slot: Box::new(slot),
                })
            }
            MoveEffect {
                track,
                effect,
                index,
            } => {
                let t = project
                    .track_mut(track)
                    .ok_or(CommandError::TrackNotFound(track))?;
                let from = t
                    .effect_index(effect)
                    .ok_or(CommandError::EffectNotFound(effect))?;
                let len = t.effects.len();
                if index >= len {
                    return Err(CommandError::IndexOutOfRange { index, len });
                }
                let e = t.effects.remove(from);
                t.effects.insert(index, e);
                Ok(MoveEffect {
                    track,
                    effect,
                    index: from,
                })
            }
            SetEffectBypass {
                track,
                effect,
                bypass,
            } => {
                let t = project
                    .track_mut(track)
                    .ok_or(CommandError::TrackNotFound(track))?;
                let e = t
                    .effect_mut(effect)
                    .ok_or(CommandError::EffectNotFound(effect))?;
                let old = std::mem::replace(&mut e.bypass, bypass);
                Ok(SetEffectBypass {
                    track,
                    effect,
                    bypass: old,
                })
            }
            SetEffectParam {
                track,
                effect,
                key,
                value,
            } => {
                let t = project
                    .track_mut(track)
                    .ok_or(CommandError::TrackNotFound(track))?;
                let e = t
                    .effect_mut(effect)
                    .ok_or(CommandError::EffectNotFound(effect))?;
                let old = e.spec.get(&key);
                e.spec
                    .set(&key, value)
                    .ok_or_else(|| CommandError::InvalidValue(format!("effect param {key}")))?;
                Ok(SetEffectParam {
                    track,
                    effect,
                    key,
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
            SetClipOffset {
                track,
                clip,
                offset,
            } => {
                let t = project
                    .track_mut(track)
                    .ok_or(CommandError::TrackNotFound(track))?;
                let c = t.clip_mut(clip).ok_or(CommandError::ClipNotFound(clip))?;
                let old = std::mem::replace(&mut c.offset, offset);
                Ok(SetClipOffset {
                    track,
                    clip,
                    offset: old,
                })
            }
            SetClipLoop {
                track,
                clip,
                content_length,
            } => {
                if let Some(l) = content_length {
                    if l <= Ticks::ZERO {
                        return Err(CommandError::InvalidValue("clip loop length".into()));
                    }
                }
                let t = project
                    .track_mut(track)
                    .ok_or(CommandError::TrackNotFound(track))?;
                let c = t.clip_mut(clip).ok_or(CommandError::ClipNotFound(clip))?;
                let old = std::mem::replace(&mut c.content_length, content_length);
                Ok(SetClipLoop {
                    track,
                    clip,
                    content_length: old,
                })
            }
            SetClipMute { track, clip, muted } => {
                let t = project
                    .track_mut(track)
                    .ok_or(CommandError::TrackNotFound(track))?;
                let c = t.clip_mut(clip).ok_or(CommandError::ClipNotFound(clip))?;
                let old = std::mem::replace(&mut c.muted, muted);
                Ok(SetClipMute {
                    track,
                    clip,
                    muted: old,
                })
            }
            SetClipColor { track, clip, color } => {
                let t = project
                    .track_mut(track)
                    .ok_or(CommandError::TrackNotFound(track))?;
                let c = t.clip_mut(clip).ok_or(CommandError::ClipNotFound(clip))?;
                let old = std::mem::replace(&mut c.color, color);
                Ok(SetClipColor {
                    track,
                    clip,
                    color: old,
                })
            }
            MoveClipToTrack { from, to, clip } => {
                if from == to {
                    // Not an error: a drag that ends on the track it started on is a
                    // no-op, and its inverse is itself. It still has to name a clip that
                    // exists, or a batch would half-apply and report success.
                    let t = project
                        .track(from)
                        .ok_or(CommandError::TrackNotFound(from))?;
                    t.clip(clip).ok_or(CommandError::ClipNotFound(clip))?;
                    return Ok(MoveClipToTrack { from, to, clip });
                }
                if project.track(to).is_none() {
                    return Err(CommandError::TrackNotFound(to));
                }
                let src = project
                    .track_mut(from)
                    .ok_or(CommandError::TrackNotFound(from))?;
                let idx = src
                    .clips
                    .iter()
                    .position(|c| c.id == clip)
                    .ok_or(CommandError::ClipNotFound(clip))?;
                let moved = src.clips.remove(idx);
                let dst = project
                    .track_mut(to)
                    .ok_or(CommandError::TrackNotFound(to))?;
                if dst.clip(clip).is_some() {
                    // Cannot happen with ids from `Project::alloc_id`, but a project file
                    // is a file: put the clip back rather than lose it.
                    let src = project.track_mut(from).expect("checked above");
                    src.clips.insert(idx, moved);
                    return Err(CommandError::DuplicateId(clip.0));
                }
                dst.clips.push(moved);
                dst.clips.sort_by_key(|c| c.start);
                Ok(MoveClipToTrack {
                    from: to,
                    to: from,
                    clip,
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

fn clamp_pan(pan: f32) -> Result<f32, CommandError> {
    if !pan.is_finite() {
        return Err(CommandError::InvalidValue("pan".into()));
    }
    Ok(pan.clamp(-1.0, 1.0))
}

fn clamp_width(width: f32) -> Result<f32, CommandError> {
    if !width.is_finite() {
        return Err(CommandError::InvalidValue("width".into()));
    }
    Ok(width.clamp(0.0, MAX_WIDTH))
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

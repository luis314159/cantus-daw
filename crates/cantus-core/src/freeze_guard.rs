//! The edit lock of frozen targets (ADR 0064 §3): every command classified by what it
//! touches, and a touch inside a frozen region refused.
//!
//! [`touches`] is one `match` over every [`Command`] with **no wildcard arm**: a command
//! added to the model does not build until it is said here what it touches (BACKLOG §4c:
//! "every `Command` is classified … and a new one fails the build until it is"). Most are
//! classified statically — the strip, track or automation target they change — and checked
//! against the [`Locks`] of the frozen targets before anything is applied; the few whose
//! reach is project-wide ([`Touch::Global`]: the tempo, the pan law, a track or strip
//! inserted or removed, a whole project) are applied and the frozen keys checked after,
//! undone and refused when one moved. The undo fuzz holds the invariant that results:
//! after any command, applied or refused, every frozen target's stored key is its content
//! key ([`super::consistent`]).

use super::{content_key, FreezeTarget, Locks};
use crate::automation::AutoTarget;
use crate::command::{Command, CommandError};
use crate::model::{ClipId, Project, StripId, TrackId};

/// What a command changes, as far as a frozen sound is concerned.
#[derive(Clone, Debug, PartialEq)]
pub enum Touch {
    /// A track's source: its instrument, clips, notes, input and arming.
    TrackSource(TrackId),
    /// A track's level, balance, mute, solo and place: what its strip gathers of it.
    TrackMix(TrackId),
    /// A strip's chain: polarity, swap, inserts, EQ.
    StripChain(StripId),
    /// A strip's mix: width, pan, fader, mute, solo, sends, where it goes, its place.
    StripMix(StripId),
    /// What a strip gathers: a route into it made or taken away.
    StripInput(StripId),
    /// An automation target's curve, or a modulator driving it.
    Target(AutoTarget),
    /// Project-wide: applied, then every frozen key checked.
    Global,
}

/// The touches of `cmd` on `project` as it is (before the command). A `Batch` touches
/// nothing itself: each of its commands is guarded as it is applied.
pub fn touches(cmd: &Command, project: &Project) -> Vec<Touch> {
    use Command::*;
    use Touch::*;
    let strip = |s: StripId| {
        if s == StripId::MASTER {
            Global
        } else {
            StripChain(s)
        }
    };
    let out_of = |s: StripId| project.strip(s).and_then(|x| x.output);
    let input = |s: Option<StripId>| s.map_or(Global, StripInput);
    match cmd {
        // ---- project-wide ----------------------------------------------------------------
        SetProjectName { .. }
        | SetLoopRegion { .. }
        | SetLoopEnabled { .. }
        | SetMetronome { .. }
        | SetPatternMode { .. }
        | AddMarker { .. }
        | RemoveMarker { .. }
        | SetMarker { .. }
        | SetRecordSettings { .. } => vec![],
        SetTempo { .. }
        | SetTimeSignature { .. }
        | SetTempoTrack { .. }
        | InsertTempoPoint { .. }
        | RemoveTempoPoint { .. }
        | SetTempoPoint { .. }
        | SetMetreTrack { .. }
        | SetMetreAt { .. }
        | RemoveMetreAt { .. }
        | MoveMetre { .. }
        | SetPanLaw { .. }
        | ReplaceProject { .. } => vec![Global],
        // The master is in a region only when a modulator there follows the mix.
        SetMasterGain { .. }
        | SetMasterPan { .. }
        | SetMasterWidth { .. }
        | SetMasterPhaseInvert { .. }
        | SetMasterFxEnabled { .. } => vec![Global],
        // ---- tracks ----------------------------------------------------------------------
        InsertTrack { .. } | RemoveTrack { .. } => vec![Global],
        MoveTrack { id, .. } | SetTrackGain { id, .. } | SetTrackPan { id, .. } => {
            vec![TrackMix(*id)]
        }
        SetTrackMute { id, .. } | SetTrackSolo { id, .. } => vec![TrackMix(*id)],
        SetTrackName { .. }
        | SetTrackColor { .. }
        | SetTrackHeight { .. }
        | SetTrackIcon { .. }
        | SetTrackAutomationOpen { .. }
        | SetTrackTakesOpen { .. }
        | SetTrackPitchOpen { .. } => vec![],
        SetTrackStrip { id, strip } => {
            let was = project.track(*id).and_then(|t| t.strip);
            vec![TrackMix(*id), input(was), input(*strip)]
        }
        SetTrackInput { id, .. } | SetTrackArmed { id, .. } => vec![TrackSource(*id)],
        // ---- strips ----------------------------------------------------------------------
        InsertStrip { .. } | RemoveStrip { .. } => vec![Global],
        SetStripName { .. }
        | SetStripColor { .. }
        | SetStripDock { .. }
        | SetStripGroup { .. }
        | InsertStripGroup { .. }
        | RemoveStripGroup { .. }
        | SetStripGroupName { .. }
        | SetStripGroupColor { .. }
        | SetStripGroupCollapsed { .. } => vec![],
        SetStripGain { id, .. }
        | SetStripPan { id, .. }
        | SetStripWidth { id, .. }
        | SetStripMute { id, .. }
        | SetStripSolo { id, .. }
        | MoveStrip { id, .. } => vec![StripMix(*id)],
        SetStripPhaseInvert { id, .. }
        | SetStripSwapChannels { id, .. }
        | SetStripFxEnabled { id, .. }
        | SetStripEq { id, .. } => vec![StripChain(*id)],
        SetStripOutput { id, output } => vec![StripMix(*id), input(out_of(*id)), input(*output)],
        SetStripSend { id, send } | InsertStripSend { id, send, .. } => {
            vec![StripMix(*id), StripInput(send.to)]
        }
        RemoveStripSend { id, to } => vec![StripMix(*id), StripInput(*to)],
        // ---- instruments -----------------------------------------------------------------
        SetInstrument { track, .. }
        | InsertPad { track, .. }
        | RemovePad { track, .. }
        | SetInstrumentParam { track, .. }
        | SetPadName { track, .. }
        | SetPadSample { track, .. }
        | SetPadLayers { track, .. }
        | SetInstrumentValue { track, .. }
        | ClearInstrumentValue { track, .. }
        | SetInstrumentText { track, .. }
        | SetInstrumentPluginParam { track, .. }
        | SetInstrumentPluginState { track, .. } => vec![TrackSource(*track)],
        // Where a plug-in runs never changes its sound (ADR 0063).
        SetPluginIsolation { .. } | SetInstrumentIsolation { .. } => vec![],
        // ---- inserts ---------------------------------------------------------------------
        InsertEffect { strip: s, .. }
        | RemoveEffect { strip: s, .. }
        | MoveEffect { strip: s, .. }
        | ReplaceEffect { strip: s, .. }
        | SetEffectBypass { strip: s, .. }
        | SetEffectParam { strip: s, .. }
        | ClearEffectParam { strip: s, .. }
        | SetEffectSidechain { strip: s, .. }
        | SetEffectMix { strip: s, .. }
        | SetPluginParam { strip: s, .. }
        | SetPluginState { strip: s, .. }
        | SetEffectMidiSource { strip: s, .. } => vec![strip(*s)],
        // ---- clips -----------------------------------------------------------------------
        InsertClip { track, clip } => match &clip.automation {
            Some(a) => vec![Target(a.target.clone())],
            None => {
                let mut v = vec![TrackSource(*track)];
                if let Some(link) = clip.link {
                    v.extend(link_touches(project, link));
                }
                v
            }
        },
        RemoveClip { track, clip }
        | SetClipBounds { track, clip, .. }
        | SetClipOffset { track, clip, .. }
        | SetClipLoop { track, clip, .. }
        | SetClipMute { track, clip, .. }
        | SetClipAudio { track, clip, .. }
        | SetClipPitch { track, clip, .. }
        // A warped clip's render plays in place of its source (ADR 0061).
        | SetClipWarp { track, clip, .. }
        | SetClipLane { track, clip, .. }
        | SetClipComp { track, clip, .. }
        | InsertNotes { track, clip, .. }
        | RemoveNotes { track, clip, .. }
        | SetNotes { track, clip, .. } => clip_touches(project, *track, *clip),
        SetClipContent {
            track,
            clip,
            content,
        } => {
            let mut v = clip_touches(project, *track, *clip);
            if let Some(a) = &content.automation {
                v.push(Target(a.target.clone()));
            }
            // A clip that changes kind leaves (or joins) its track's sounding clips.
            let was = project
                .track(*track)
                .and_then(|t| t.clip(*clip))
                .map(|c| c.automation.is_some());
            if was != Some(content.automation.is_some()) {
                v.push(TrackSource(*track));
            }
            v
        }
        SetClipLink { track, clip, link } => {
            let mut v = clip_touches(project, *track, *clip);
            if let Some(link) = link {
                v.extend(link_touches(project, *link));
            }
            v
        }
        SetClipName { .. } | SetClipColor { .. } => vec![],
        MoveClipToTrack { from, to, clip } => {
            let mut v = clip_touches(project, *from, *clip);
            if !v.iter().any(|t| matches!(t, Target(_))) {
                v.push(TrackSource(*to));
            }
            v
        }
        SetClipAutomation {
            track,
            clip,
            automation,
        } => {
            let mut v = clip_touches(project, *track, *clip);
            match automation {
                Some(a) => v.push(Target(a.target.clone())),
                // An automation clip made a plain one joins its track's sounding clips.
                None => v.push(TrackSource(*track)),
            }
            v
        }
        SetCompSeam { track, .. } | SetTrackComps { track, .. } => vec![TrackSource(*track)],
        // ---- audio sources ---------------------------------------------------------------
        InsertAudioSource { .. } | RemoveAudioSource { .. } => vec![],
        SetAudioSourcePath { id, .. } => project
            .tracks
            .iter()
            .filter(|t| {
                t.clips
                    .iter()
                    .filter_map(|c| c.audio.as_ref())
                    .chain(
                        t.comps
                            .iter()
                            .flat_map(|c| c.windows.iter())
                            .filter_map(|w| w.audio.as_ref()),
                    )
                    .any(|a| a.source == *id)
            })
            .map(|t| TrackSource(t.id))
            .collect(),
        // ---- modulators ------------------------------------------------------------------
        InsertModulator { modulator, .. } => routes(&modulator.routes),
        RemoveModulator { id } => project
            .modulator(*id)
            .map(|(_, _, m)| routes(&m.routes))
            .unwrap_or_default(),
        SetModulator { modulator } => {
            let mut v = routes(&modulator.routes);
            if let Some((_, _, m)) = project.modulator(modulator.id) {
                v.extend(routes(&m.routes));
            }
            v
        }
        // ---- freezing: validated where they are applied --------------------------------
        SetStripFrozen { .. } | SetTrackFrozen { .. } => vec![],
        // Each command of a batch is guarded as it is applied.
        Batch { .. } => vec![],
    }
}

fn routes(routes: &[crate::modulator::ModRoute]) -> Vec<Touch> {
    routes
        .iter()
        .map(|r| Touch::Target(r.target.clone()))
        .collect()
}

/// The touches of a link group's members: each one's own (a clip joining or leaving the
/// group shares their content).
fn link_touches(project: &Project, link: crate::model::LinkId) -> Vec<Touch> {
    crate::links::members(project, link)
        .into_iter()
        .flat_map(|(t, c)| clip_touches(project, t, c))
        .collect()
}

/// A clip's touches: an automation clip's target, else the source of its track and of
/// every track a linked sibling sits on (an edit through one changes them all).
fn clip_touches(project: &Project, track: TrackId, clip: ClipId) -> Vec<Touch> {
    let Some(c) = project.track(track).and_then(|t| t.clip(clip)) else {
        return vec![Touch::TrackSource(track)];
    };
    if let Some(a) = &c.automation {
        return vec![Touch::Target(a.target.clone())];
    }
    let mut v = vec![Touch::TrackSource(track)];
    for (t, c) in crate::links::siblings(project, (track, clip)) {
        // A sibling is the same kind of clip; an automation one is its target's.
        match project
            .track(t)
            .and_then(|x| x.clip(c))
            .and_then(|c| c.automation.as_ref())
        {
            Some(a) => v.push(Touch::Target(a.target.clone())),
            None => v.push(Touch::TrackSource(t)),
        }
    }
    v
}

/// The frozen target a touch is refused by, if any; `Global` is never refused here.
fn refused_by(project: &Project, locks: &Locks, touch: &Touch, depth: u8) -> Option<FreezeTarget> {
    use Touch::*;
    match touch {
        TrackSource(t) => locks
            .source
            .contains(t)
            .then(|| locks.owner_of_track(*t))
            .flatten(),
        TrackMix(t) => locks
            .track_mix
            .contains(t)
            .then(|| locks.owner_of_track(*t))
            .flatten(),
        StripChain(s) | StripInput(s) => locks
            .chain
            .contains(s)
            .then(|| locks.owner_of_strip(*s))
            .flatten(),
        StripMix(s) => locks
            .mix
            .contains(s)
            .then(|| locks.owner_of_strip(*s))
            .flatten(),
        Target(target) => {
            let inner = match target {
                AutoTarget::TrackGain(t) | AutoTarget::TrackPan(t) => TrackMix(*t),
                AutoTarget::StripGain(s) | AutoTarget::StripPan(s) | AutoTarget::StripWidth(s) => {
                    StripMix(*s)
                }
                AutoTarget::Send { strip, .. } => StripMix(*strip),
                AutoTarget::Effect { effect, .. } | AutoTarget::Plugin { effect, .. } => {
                    match project.effect_host(*effect) {
                        Some(h) if h != StripId::MASTER => StripChain(h),
                        _ => return None,
                    }
                }
                AutoTarget::Instrument { track, .. }
                | AutoTarget::Firmus { track, .. }
                | AutoTarget::PluginInstrument { track, .. } => TrackSource(*track),
                AutoTarget::MasterGain | AutoTarget::MasterPan | AutoTarget::MasterWidth => {
                    return None
                }
                AutoTarget::ModAmount(m) => {
                    if depth > 0 {
                        return None;
                    }
                    let (_, _, m) = project.modulator(*m)?;
                    return m
                        .routes
                        .iter()
                        .find_map(|r| refused_by(project, locks, &Target(r.target.clone()), 1));
                }
            };
            refused_by(project, locks, &inner, depth)
        }
        Global => None,
    }
}

/// Whether `touches` reach anything project-wide, or the master (in a region only when a
/// modulator there follows the mix): applied, then checked.
fn needs_check(project: &Project, touches: &[Touch]) -> bool {
    touches.iter().any(|t| match t {
        Touch::Global => true,
        Touch::Target(AutoTarget::MasterGain | AutoTarget::MasterPan | AutoTarget::MasterWidth) => {
            true
        }
        Touch::Target(AutoTarget::Effect { effect, .. } | AutoTarget::Plugin { effect, .. }) => {
            project.effect_host(*effect) == Some(StripId::MASTER)
        }
        _ => false,
    })
}

/// Apply `cmd` to `project` through `apply` (the command's own body) under the lock:
/// refused with [`CommandError::Frozen`] when it would change a frozen sound, the project
/// then as it was.
pub fn guard(
    cmd: Command,
    project: &mut Project,
    apply: fn(Command, &mut Project) -> Result<Command, CommandError>,
) -> Result<Command, CommandError> {
    // Freezing itself: a frozen state must be the target's key now.
    match &cmd {
        Command::SetStripFrozen {
            strip,
            frozen: Some(f),
        } => {
            if !f.is_valid()
                || content_key(project, FreezeTarget::Strip(*strip)).as_ref() != Some(&f.content)
            {
                return Err(CommandError::InvalidValue("frozen".into()));
            }
            return apply(cmd, project);
        }
        Command::SetTrackFrozen {
            track,
            frozen: Some(f),
        } => {
            if !f.is_valid()
                || content_key(project, FreezeTarget::Track(*track)).as_ref() != Some(&f.content)
            {
                return Err(CommandError::InvalidValue("frozen".into()));
            }
            return apply(cmd, project);
        }
        Command::Batch { .. } => return apply(cmd, project),
        _ => {}
    }
    let replaces = matches!(cmd, Command::ReplaceProject { .. });
    if !project.any_frozen() && !replaces && !inserts_frozen(&cmd) {
        return apply(cmd, project);
    }
    let touched = touches(&cmd, project);
    let locks = Locks::of(project);
    for t in &touched {
        if let Some(owner) = refused_by(project, &locks, t, 0) {
            return Err(CommandError::Frozen(owner));
        }
    }
    let check = needs_check(project, &touched);
    let inverse = apply(cmd, project)?;
    if check {
        if let Err(owner) = super::consistent(project) {
            // Undone: the project is as it was (an inverse is exact, ADR 0004).
            let _ = apply(inverse, project);
            return Err(CommandError::Frozen(owner));
        }
    }
    Ok(inverse)
}

/// A track or strip inserted with a frozen state of its own (an undo of its removal).
fn inserts_frozen(cmd: &Command) -> bool {
    match cmd {
        Command::InsertTrack { track, .. } => track.frozen.is_some(),
        Command::InsertStrip { strip, .. } => strip.frozen.is_some(),
        _ => false,
    }
}

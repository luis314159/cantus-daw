//! The commands of the mixer's round-36 features (ADR 0004, 0093): VCAs and who follows
//! them, mixer snapshots and their recall, cues, and the channels a recall leaves alone.
//! `Command::Mix` carries one; each has an exact inverse.

use crate::command::{Command, CommandError};
use crate::freeze::Touch;
use crate::mix_snap::{apply_state, changes, MixCue, MixCueId, MixSnapId, MixSnapshot, MixState};
use crate::model::{GroupId, Project, StripId};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum MixCommand {
    /// Add a snapshot at `index` (taking one, and the inverse of removing one).
    InsertSnapshot {
        index: usize,
        snapshot: Box<MixSnapshot>,
    },
    /// Remove a snapshot no cue plays.
    RemoveSnapshot {
        id: MixSnapId,
    },
    RenameSnapshot {
        id: MixSnapId,
        name: String,
    },
    /// Take the desk's state again into a snapshot (`scope` empty: all channels).
    UpdateSnapshot {
        id: MixSnapId,
        state: Box<MixState>,
    },
    /// Recall a snapshot: `scope` empty is every channel; the master when `master`; a safe
    /// channel only when `include_safe` or named by `scope`. One undo step.
    Recall {
        id: MixSnapId,
        scope: Vec<StripId>,
        master: bool,
        include_safe: bool,
    },
    /// Put a state on the desk, exactly (the inverse of a recall; the A/B swap).
    Apply {
        state: Box<MixState>,
    },
    InsertCue {
        index: usize,
        cue: MixCue,
    },
    RemoveCue {
        id: MixCueId,
    },
    SetCue {
        cue: MixCue,
    },
    /// The VCAs whose faders scale a strip (besides its group's).
    SetStripVcas {
        id: StripId,
        vcas: Vec<StripId>,
    },
    /// The VCA a group's strips follow.
    SetGroupVca {
        id: GroupId,
        vca: Option<StripId>,
    },
    /// A recall of everything leaves the channel alone.
    SetRecallSafe {
        id: StripId,
        safe: bool,
    },
    /// A solo elsewhere does not silence the channel (console feel).
    SetSoloSafe {
        id: StripId,
        safe: bool,
    },
}

impl MixCommand {
    pub fn label_key(&self) -> &'static str {
        use MixCommand::*;
        match self {
            InsertSnapshot { .. } => "cmd.mix.take_snapshot",
            RemoveSnapshot { .. } => "cmd.mix.remove_snapshot",
            RenameSnapshot { .. } => "cmd.mix.rename_snapshot",
            UpdateSnapshot { .. } => "cmd.mix.update_snapshot",
            Recall { .. } | Apply { .. } => "cmd.mix.recall",
            InsertCue { .. } => "cmd.mix.add_cue",
            RemoveCue { .. } => "cmd.mix.remove_cue",
            SetCue { .. } => "cmd.mix.set_cue",
            SetStripVcas { .. } => "cmd.mix.set_vcas",
            SetGroupVca { .. } => "cmd.mix.set_group_vca",
            SetRecallSafe { .. } => "cmd.mix.set_safe",
            SetSoloSafe { .. } => "cmd.mix.set_solo_safe",
        }
    }

    /// Whether `self` replaces `older` in a merged gesture.
    pub fn supersedes(&self, older: &MixCommand) -> bool {
        use MixCommand::*;
        match (self, older) {
            (RenameSnapshot { id: a, .. }, RenameSnapshot { id: b, .. }) => a == b,
            (SetCue { cue: a }, SetCue { cue: b }) => a.id == b.id,
            _ => false,
        }
    }
}

fn invalid(what: &str) -> CommandError {
    CommandError::InvalidValue(what.into())
}

/// Apply `c`; its inverse.
pub fn apply(c: MixCommand, project: &mut Project) -> Result<Command, CommandError> {
    use MixCommand::*;
    let inv = |c: MixCommand| Ok(Command::Mix(c));
    match c {
        InsertSnapshot { index, snapshot } => {
            let len = project.mix.snapshots.len();
            if index > len {
                return Err(CommandError::IndexOutOfRange { index, len });
            }
            if project.mix.snapshot(snapshot.id).is_some() {
                return Err(CommandError::DuplicateId(snapshot.id.0));
            }
            let id = snapshot.id;
            project.mix.snapshots.insert(index, *snapshot);
            inv(RemoveSnapshot { id })
        }
        RemoveSnapshot { id } => {
            let index = project
                .mix
                .snapshots
                .iter()
                .position(|s| s.id == id)
                .ok_or_else(|| invalid("snapshot"))?;
            if project.mix.cues.iter().any(|c| c.snapshot == id) {
                return Err(invalid("snapshot is cued"));
            }
            let snapshot = project.mix.snapshots.remove(index);
            inv(InsertSnapshot {
                index,
                snapshot: Box::new(snapshot),
            })
        }
        RenameSnapshot { id, name } => {
            let s = snap_mut(project, id)?;
            let old = std::mem::replace(&mut s.name, name);
            inv(RenameSnapshot { id, name: old })
        }
        UpdateSnapshot { id, state } => {
            let s = snap_mut(project, id)?;
            let old = std::mem::replace(&mut s.state, *state);
            inv(UpdateSnapshot {
                id,
                state: Box::new(old),
            })
        }
        Recall {
            id,
            scope,
            master,
            include_safe,
        } => {
            let state = project
                .mix
                .snapshot(id)
                .ok_or_else(|| invalid("snapshot"))?
                .state
                .clone();
            let old = apply_state(project, &state, &scope, master, include_safe, false);
            inv(Apply {
                state: Box::new(old),
            })
        }
        Apply { state } => {
            let old = apply_state(project, &state, &[], true, true, true);
            inv(Apply {
                state: Box::new(old),
            })
        }
        InsertCue { index, cue } => {
            let len = project.mix.cues.len();
            if index > len {
                return Err(CommandError::IndexOutOfRange { index, len });
            }
            if project.mix.cues.iter().any(|c| c.id == cue.id) {
                return Err(CommandError::DuplicateId(cue.id.0));
            }
            check_cue(project, &cue)?;
            let id = cue.id;
            project.mix.cues.insert(index, cue);
            inv(RemoveCue { id })
        }
        RemoveCue { id } => {
            let index = project
                .mix
                .cues
                .iter()
                .position(|c| c.id == id)
                .ok_or_else(|| invalid("cue"))?;
            let cue = project.mix.cues.remove(index);
            inv(InsertCue { index, cue })
        }
        SetCue { cue } => {
            check_cue(project, &cue)?;
            let c = project
                .mix
                .cues
                .iter_mut()
                .find(|c| c.id == cue.id)
                .ok_or_else(|| invalid("cue"))?;
            let old = std::mem::replace(c, cue);
            inv(SetCue { cue: old })
        }
        SetStripVcas { id, vcas } => {
            for (i, v) in vcas.iter().enumerate() {
                if *v == id || !project.is_vca(*v) || vcas[..i].contains(v) {
                    return Err(invalid("vca"));
                }
            }
            let s = project
                .strip_mut(id)
                .ok_or(CommandError::StripNotFound(id))?;
            let old = std::mem::replace(&mut s.vcas, vcas);
            if project.vca_cycle() {
                if let Some(s) = project.strip_mut(id) {
                    s.vcas = old;
                }
                return Err(invalid("vca cycle"));
            }
            inv(SetStripVcas { id, vcas: old })
        }
        SetGroupVca { id, vca } => {
            if project.strip_group(id).is_none() {
                return Err(CommandError::GroupNotFound(id));
            }
            if let Some(v) = vca {
                if !project.is_vca(v) {
                    return Err(invalid("vca"));
                }
            }
            let before = project.mix.group_vcas.clone();
            let at = project.mix.group_vcas.iter().position(|(g, _)| *g == id);
            let old = at.map(|i| project.mix.group_vcas[i].1);
            match (at, vca) {
                (Some(i), Some(v)) => project.mix.group_vcas[i].1 = v,
                (Some(i), None) => {
                    project.mix.group_vcas.remove(i);
                }
                (None, Some(v)) => project.mix.group_vcas.push((id, v)),
                (None, None) => {}
            }
            if project.vca_cycle() {
                // Put it back exactly, in its order (review 1, item 7).
                project.mix.group_vcas = before;
                return Err(invalid("vca cycle"));
            }
            inv(SetGroupVca { id, vca: old })
        }
        SetRecallSafe { id, safe } => {
            let s = project
                .strip_mut(id)
                .ok_or(CommandError::StripNotFound(id))?;
            let old = std::mem::replace(&mut s.recall_safe, safe);
            inv(SetRecallSafe { id, safe: old })
        }
        SetSoloSafe { id, safe } => {
            let s = project
                .strip_mut(id)
                .ok_or(CommandError::StripNotFound(id))?;
            let old = std::mem::replace(&mut s.solo_safe, safe);
            inv(SetSoloSafe { id, safe: old })
        }
    }
}

fn snap_mut(project: &mut Project, id: MixSnapId) -> Result<&mut MixSnapshot, CommandError> {
    project
        .mix
        .snapshots
        .iter_mut()
        .find(|s| s.id == id)
        .ok_or_else(|| invalid("snapshot"))
}

fn check_cue(project: &Project, cue: &MixCue) -> Result<(), CommandError> {
    if project.mix.snapshot(cue.snapshot).is_none() {
        return Err(invalid("snapshot"));
    }
    if cue.at.0 < 0 {
        return Err(invalid("cue position"));
    }
    Ok(())
}

/// What `c` touches, for the edit lock of frozen targets.
pub fn touches(c: &MixCommand, project: &Project) -> Vec<Touch> {
    use MixCommand::*;
    match c {
        Recall {
            id,
            scope,
            master,
            include_safe,
        } => match project.mix.snapshot(*id) {
            Some(s) => of_state(project, &s.state, scope, *master, *include_safe),
            None => vec![],
        },
        Apply { state } => of_state(project, state, &[], true, true),
        // A cue changes, from its bar, what its snapshot holds (levels as automation,
        // switches in the timeline): what a recall of it would change (review 1, item 1).
        // A cue's data reaches a frozen key (ADR 0092): applied, then every frozen key
        // checked (`Global`), besides what it changes now. // r36:record
        InsertCue { cue, .. } => with_check(of_cue(project, cue)),
        SetCue { cue } => {
            let mut v = of_cue(project, cue);
            if let Some(old) = project.mix.cues.iter().find(|c| c.id == cue.id) {
                v.extend(of_cue(project, old));
            }
            with_check(v) // r36:record
        }
        RemoveCue { id } => match project.mix.cues.iter().find(|c| c.id == *id) {
            Some(c) => with_check(of_cue(project, c)), // r36:record
            None => vec![],
        },
        // A snapshot a cue plays retaken, or a strip made safe from cues of everything:
        // what the cues apply changes (round 36, the integrated review). // r36:record
        UpdateSnapshot { id, .. } => {
            let played: Vec<_> = project
                .mix
                .cues
                .iter()
                .filter(|c| c.snapshot == *id)
                .collect();
            if played.is_empty() {
                vec![]
            } else {
                with_check(
                    played
                        .into_iter()
                        .flat_map(|c| of_cue(project, c))
                        .collect(),
                )
            }
        } // r36:record
        SetRecallSafe { .. } => {
            let everything: Vec<_> = project
                .mix
                .cues
                .iter()
                .filter(|c| c.scope.is_empty())
                .collect();
            if everything.is_empty() {
                vec![]
            } else {
                with_check(
                    everything
                        .into_iter()
                        .flat_map(|c| of_cue(project, c))
                        .collect(),
                )
            }
        } // r36:record
        // Who follows a VCA changes the fader stage of a bus's inputs (a freeze region's
        // strips): the mix of everything whose VCAs change (review 1, item 1).
        SetStripVcas { id, .. } => followers(project, &[*id]),
        SetGroupVca { id, .. } => {
            let ids: Vec<StripId> = project.strips_in_group(*id).map(|s| s.id).collect();
            followers(project, &ids)
        }
        // Who a solo silences changes what a strip gathers of a frozen bus: its mix.
        SetSoloSafe { id, .. } => vec![Touch::StripMix(*id)],
        // A snapshot taken or renamed plays nothing; a VCA's follow list scales the fader
        // stage, which is after what a freeze renders (ADR 0093).
        InsertSnapshot { .. } | RemoveSnapshot { .. } | RenameSnapshot { .. } => vec![],
    }
}

/// `v` and a check of every frozen key after the command (round 36). // r36:record
fn with_check(mut v: Vec<Touch>) -> Vec<Touch> {
    v.push(Touch::Global);
    v
}

/// The mix of `ids` and of every strip a VCA among them reaches.
pub(crate) fn followers(project: &Project, ids: &[StripId]) -> Vec<Touch> {
    let mut out: Vec<StripId> = ids.to_vec();
    for id in ids {
        for m in project.vca_members(*id) {
            if !out.contains(&m) {
                out.push(m);
            }
        }
    }
    out.into_iter().map(Touch::StripMix).collect()
}

fn of_cue(project: &Project, cue: &crate::mix_snap::MixCue) -> Vec<Touch> {
    match project.mix.snapshot(cue.snapshot) {
        Some(s) => of_state(project, &s.state, &cue.scope, cue.scope.is_empty(), false),
        None => vec![],
    }
}

fn of_state(
    project: &Project,
    state: &MixState,
    scope: &[StripId],
    master: bool,
    include_safe: bool,
) -> Vec<Touch> {
    let (chain, mix, m) = changes(project, state, scope, master, include_safe);
    let mut v: Vec<Touch> = chain.into_iter().map(Touch::StripChain).collect();
    v.extend(mix.into_iter().map(Touch::StripMix));
    if m {
        v.push(Touch::Global);
    }
    v
}

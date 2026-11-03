//! The bodies of round 36's external-unit commands (ADR 0094). Each returns its exact
//! inverse.

use crate::command::{Command, CommandError};
use crate::external_unit::{ExternalUnit, ExternalUnitId, Measured};
use crate::model::Project;

fn bad(what: &str) -> CommandError {
    CommandError::InvalidValue(what.to_string())
}

/// At most this many units in a project (a rack is dozens, not thousands).
pub const MAX_UNITS: usize = 128;

fn check(project: &Project, unit: &ExternalUnit, replacing: bool) -> Result<(), CommandError> {
    if !unit.is_valid() {
        return Err(bad("external unit"));
    }
    // A unit whose track is gone is inert, not invalid: a track removed leaves its units,
    // and the undo of a unit's removal must put it back whether its track is there or not
    // (the track's own undo comes later). The interface and the agents only add units to
    // tracks that exist.
    // One unit of a kind per track: the track's notes (or audio) go to one place.
    let clash = project
        .external_units
        .iter()
        .any(|u| u.id != unit.id && u.track == unit.track && u.kind == unit.kind);
    if clash {
        return Err(bad("the track already has a unit of that kind"));
    }
    if !replacing && project.external_unit(unit.id).is_some() {
        return Err(CommandError::DuplicateId(unit.id.0));
    }
    Ok(())
}

pub(crate) fn insert(
    project: &mut Project,
    index: usize,
    unit: ExternalUnit,
) -> Result<Command, CommandError> {
    if project.external_units.len() >= MAX_UNITS {
        return Err(bad("too many external units"));
    }
    if index > project.external_units.len() {
        return Err(CommandError::IndexOutOfRange {
            index,
            len: project.external_units.len(),
        });
    }
    check(project, &unit, false)?;
    let id = unit.id;
    project.external_units.insert(index, unit);
    Ok(Command::RemoveExternalUnit { id })
}

pub(crate) fn remove(project: &mut Project, id: ExternalUnitId) -> Result<Command, CommandError> {
    let index = project
        .external_units
        .iter()
        .position(|u| u.id == id)
        .ok_or_else(|| bad("external unit"))?;
    let unit = project.external_units.remove(index);
    Ok(Command::InsertExternalUnit {
        index,
        unit: Box::new(unit),
    })
}

pub(crate) fn set(project: &mut Project, unit: ExternalUnit) -> Result<Command, CommandError> {
    check(project, &unit, true)?;
    let slot = project
        .external_units
        .iter_mut()
        .find(|u| u.id == unit.id)
        .ok_or_else(|| bad("external unit"))?;
    let old = std::mem::replace(slot, unit);
    Ok(Command::SetExternalUnit {
        unit: Box::new(old),
    })
}

pub(crate) fn set_latency(
    project: &mut Project,
    id: ExternalUnitId,
    latency: Option<Measured>,
) -> Result<Command, CommandError> {
    if latency.as_ref().is_some_and(|m| !m.is_valid()) {
        return Err(bad("measured latency"));
    }
    let u = project
        .external_units
        .iter_mut()
        .find(|u| u.id == id)
        .ok_or_else(|| bad("external unit"))?;
    let old = std::mem::replace(&mut u.latency, latency);
    Ok(Command::SetExternalLatency { id, latency: old })
}

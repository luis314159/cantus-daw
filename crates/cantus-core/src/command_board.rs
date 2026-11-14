//! The bodies of Tabula's board commands (ADR 0134, provisional). Owned by the tabula
//! builder; `crate::command` dispatches here. Each returns its exact inverse:
//!
//! | Command | Inverse |
//! |---|---|
//! | `InsertPedal` | `RemovePedal` |
//! | `RemovePedal` | `InsertPedal` with its index and the macro maps removed with it |
//! | `MovePedal` | itself with the old index and path |
//! | `SetPedalOn`, `SetPedalShape` | the old value |
//! | `SetMacroMaps` | the old list |
//! | `SetMacroName` | the old name |
//!
//! Refusals: a slot that is not a board, a duplicate id, a kind that may not be a pedal
//! (`fx::tabula::pedal_allowed`), more than `MAX_PEDALS`, a map to a missing pedal or key,
//! a macro past `MACROS`. An id's kind never changes (replacing a pedal is Remove + Insert
//! in one `Batch`).
//!
//! **The wave-3 contract**: every body refuses with `"board: phase A"` until the tabula
//! builder writes it (with its undo-fuzz arm in `tests/undo_invariants.rs`).

use crate::command::{Command, CommandError};
use crate::fx::lapsus::{Lane, Pattern};
use crate::fx::tabula::{MacroMap, Path, Pedal, PedalId, PedalShape};
use crate::model::{EffectId, Project, StripId};

fn phase_a() -> CommandError {
    CommandError::InvalidValue("board: phase A".into())
}

pub(crate) fn insert_pedal(
    project: &mut Project,
    strip: StripId,
    effect: EffectId,
    index: usize,
    pedal: Pedal,
    maps: Vec<(u8, usize, MacroMap)>,
) -> Result<Command, CommandError> {
    let _ = (project, strip, effect, index, pedal, maps);
    Err(phase_a())
}

pub(crate) fn remove_pedal(
    project: &mut Project,
    strip: StripId,
    effect: EffectId,
    pedal: PedalId,
) -> Result<Command, CommandError> {
    let _ = (project, strip, effect, pedal);
    Err(phase_a())
}

pub(crate) fn move_pedal(
    project: &mut Project,
    strip: StripId,
    effect: EffectId,
    pedal: PedalId,
    index: usize,
    path: Path,
) -> Result<Command, CommandError> {
    let _ = (project, strip, effect, pedal, index, path);
    Err(phase_a())
}

pub(crate) fn set_pedal_on(
    project: &mut Project,
    strip: StripId,
    effect: EffectId,
    pedal: PedalId,
    on: bool,
) -> Result<Command, CommandError> {
    let _ = (project, strip, effect, pedal, on);
    Err(phase_a())
}

pub(crate) fn set_pedal_shape(
    project: &mut Project,
    strip: StripId,
    effect: EffectId,
    pedal: PedalId,
    shape: Option<PedalShape>,
) -> Result<Command, CommandError> {
    let _ = (project, strip, effect, pedal, shape);
    Err(phase_a())
}

pub(crate) fn set_macro_maps(
    project: &mut Project,
    strip: StripId,
    effect: EffectId,
    macro_: u8,
    maps: Vec<MacroMap>,
) -> Result<Command, CommandError> {
    let _ = (project, strip, effect, macro_, maps);
    Err(phase_a())
}

pub(crate) fn set_macro_name(
    project: &mut Project,
    strip: StripId,
    effect: EffectId,
    macro_: u8,
    name: Option<String>,
) -> Result<Command, CommandError> {
    let _ = (project, strip, effect, macro_, name);
    Err(phase_a())
}

/// `Command::SetEffectIr` with `pedal: Some(_)`: the response of a board's pedal whose kind
/// takes one (`EffectKind::takes_ir`).
pub(crate) fn set_pedal_ir(
    project: &mut Project,
    strip: StripId,
    effect: EffectId,
    ir: Option<Box<crate::fx::aula::IrFile>>,
    pedal: Option<PedalId>,
) -> Result<Command, CommandError> {
    let _ = (project, strip, effect, ir, pedal);
    Err(phase_a())
}

/// `Command::SetLapsusPattern` with `pedal: Some(_)`: a board's Lapsus pedal's bank.
pub(crate) fn set_pedal_lapsus(
    project: &mut Project,
    strip: StripId,
    effect: EffectId,
    lane: Lane,
    slot: u8,
    pattern: Option<Box<Pattern>>,
    pedal: Option<PedalId>,
) -> Result<Command, CommandError> {
    let _ = (project, strip, effect, lane, slot, pattern, pedal);
    Err(phase_a())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The contract: every body refuses, so nothing reaches a project half-built.
    #[test]
    fn every_board_command_refuses_until_phase_a() {
        let mut p = Project::new("board");
        let r = insert_pedal(
            &mut p,
            StripId::MASTER,
            EffectId(1),
            0,
            Pedal::new(PedalId(0), crate::EffectSpec::new(crate::EffectKind::Delay)),
            Vec::new(),
        );
        assert_eq!(r, Err(phase_a()));
    }
}

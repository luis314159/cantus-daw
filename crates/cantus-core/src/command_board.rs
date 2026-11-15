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
//! | `SetEffectIr`/`SetLapsusPattern` with a pedal | the pedal's old response / pattern |
//!
//! Refusals: a slot that is not a board, a duplicate id, a kind that may not be a pedal
//! (`fx::tabula::pedal_allowed`), a pedal holding a board, more than `MAX_PEDALS`, a map to
//! a missing pedal or key (or a value that is not a number, or a target another macro
//! moves), a macro past `MACROS`. An id's kind never changes (replacing a pedal is Remove +
//! Insert in one `Batch`).
//!
//! **The canonical board** every command leaves: `EffectSpec::board` is `None` exactly when
//! the board has no pedal and no macro, and the macro list never ends in a macro with no
//! name and no maps — so a command and its inverse give back the very bytes it started
//! from.

use crate::command::{slot_mut, Command, CommandError};
use crate::effect::{EffectKind, EffectSlot};
use crate::fx::lapsus::{Lane, Pattern};
use crate::fx::tabula::{
    pedal_allowed, Board, Macro, MacroMap, Path, Pedal, PedalId, PedalShape, MACROS, MAX_PEDALS,
};
use crate::model::{EffectId, Project, StripId};

fn refuse(why: &str) -> CommandError {
    CommandError::InvalidValue(format!("board: {why}"))
}

/// The board slot `effect` on `strip`; refused when it is not a board.
fn board_slot(
    project: &mut Project,
    strip: StripId,
    effect: EffectId,
) -> Result<&mut EffectSlot, CommandError> {
    let e = slot_mut(project, strip, effect)?;
    if e.spec.kind != EffectKind::Board {
        return Err(refuse("not a board"));
    }
    Ok(e)
}

/// The board of a slot, made when it has none (put back by [`settle`]).
fn board_of(e: &mut EffectSlot) -> &mut Board {
    e.spec.board.get_or_insert_with(Default::default)
}

/// The canonical form: no trailing empty macro, and no board at all when it is empty.
fn settle(e: &mut EffectSlot) {
    if let Some(b) = e.spec.board.as_deref_mut() {
        while b.macros.last().is_some_and(|m| *m == Macro::default()) {
            b.macros.pop();
        }
        if b.pedals.is_empty() && b.macros.is_empty() {
            e.spec.board = None;
        }
    }
}

/// Whether `map` may be one of macro `macro_`'s targets on `board`: its pedal is there, its
/// key is that pedal's, its ends are numbers, and no other macro moves that target.
fn map_ok(board: &Board, macro_: usize, map: &MacroMap) -> bool {
    let target = board
        .pedal(map.pedal)
        .is_some_and(|p| p.spec.kind.param(&map.key).is_some());
    let other = board
        .macro_of(map.pedal, &map.key)
        .is_some_and(|m| m != macro_);
    target && !other && map.from.is_finite() && map.to.is_finite()
}

pub(crate) fn insert_pedal(
    project: &mut Project,
    strip: StripId,
    effect: EffectId,
    index: usize,
    pedal: Pedal,
    maps: Vec<(u8, usize, MacroMap)>,
) -> Result<Command, CommandError> {
    let e = board_slot(project, strip, effect)?;
    if !pedal_allowed(pedal.spec.kind) {
        return Err(refuse("a kind that may not be a pedal"));
    }
    if pedal.spec.board.is_some() {
        return Err(refuse("a board inside a pedal"));
    }
    let b = crate::fx::tabula::board(&e.spec);
    if b.pedal(pedal.id).is_some() {
        return Err(refuse("a pedal id the board holds"));
    }
    if b.pedals.len() >= MAX_PEDALS {
        return Err(refuse("a board holds sixteen pedals"));
    }
    // The maps come back with the pedal they move, each at its place in its macro's list.
    let mut probe = b.clone();
    let at = index.min(probe.pedals.len());
    let id = pedal.id;
    probe.pedals.insert(at, pedal);
    for (m, pos, map) in &maps {
        let m = usize::from(*m);
        if m >= MACROS || map.pedal != id || !map_ok(&probe, m, map) {
            return Err(refuse("a macro map"));
        }
        if probe.macros.len() <= m {
            probe.macros.resize_with(m + 1, Macro::default);
        }
        let list = &mut probe.macros[m].maps;
        if list
            .iter()
            .any(|x| x.pedal == map.pedal && x.key == map.key)
        {
            return Err(refuse("a macro map twice"));
        }
        list.insert((*pos).min(list.len()), map.clone());
    }
    *board_of(e) = probe;
    settle(e);
    Ok(Command::RemovePedal {
        strip,
        effect,
        pedal: id,
    })
}

pub(crate) fn remove_pedal(
    project: &mut Project,
    strip: StripId,
    effect: EffectId,
    pedal: PedalId,
) -> Result<Command, CommandError> {
    let e = board_slot(project, strip, effect)?;
    let Some(index) = crate::fx::tabula::board(&e.spec).index_of(pedal) else {
        return Err(refuse("no such pedal"));
    };
    let b = board_of(e);
    let gone = b.pedals.remove(index);
    let mut maps = Vec::new();
    for (m, mac) in b.macros.iter_mut().enumerate() {
        let mut pos = 0;
        mac.maps.retain(|x| {
            let keep = x.pedal != pedal;
            if !keep {
                // Each at the place it had: put back in this order, each lands there.
                maps.push((m as u8, pos, x.clone()));
            }
            pos += 1;
            keep
        });
    }
    settle(e);
    Ok(Command::InsertPedal {
        strip,
        effect,
        index,
        pedal: Box::new(gone),
        maps,
    })
}

pub(crate) fn move_pedal(
    project: &mut Project,
    strip: StripId,
    effect: EffectId,
    pedal: PedalId,
    index: usize,
    path: Path,
) -> Result<Command, CommandError> {
    let e = board_slot(project, strip, effect)?;
    let Some(old) = crate::fx::tabula::board(&e.spec).index_of(pedal) else {
        return Err(refuse("no such pedal"));
    };
    let b = board_of(e);
    let mut p = b.pedals.remove(old);
    let old_path = std::mem::replace(&mut p.path, path);
    b.pedals.insert(index.min(b.pedals.len()), p);
    Ok(Command::MovePedal {
        strip,
        effect,
        pedal,
        index: old,
        path: old_path,
    })
}

/// The pedal `pedal` of the board slot, to change in place.
fn pedal_mut(
    project: &mut Project,
    strip: StripId,
    effect: EffectId,
    pedal: PedalId,
) -> Result<&mut Pedal, CommandError> {
    let e = board_slot(project, strip, effect)?;
    e.spec
        .board
        .as_deref_mut()
        .and_then(|b| b.pedal_mut(pedal))
        .ok_or_else(|| refuse("no such pedal"))
}

pub(crate) fn set_pedal_on(
    project: &mut Project,
    strip: StripId,
    effect: EffectId,
    pedal: PedalId,
    on: bool,
) -> Result<Command, CommandError> {
    let p = pedal_mut(project, strip, effect, pedal)?;
    let old = std::mem::replace(&mut p.on, on);
    Ok(Command::SetPedalOn {
        strip,
        effect,
        pedal,
        on: old,
    })
}

pub(crate) fn set_pedal_shape(
    project: &mut Project,
    strip: StripId,
    effect: EffectId,
    pedal: PedalId,
    shape: Option<PedalShape>,
) -> Result<Command, CommandError> {
    let p = pedal_mut(project, strip, effect, pedal)?;
    let old = std::mem::replace(&mut p.shape, shape);
    Ok(Command::SetPedalShape {
        strip,
        effect,
        pedal,
        shape: old,
    })
}

pub(crate) fn set_macro_maps(
    project: &mut Project,
    strip: StripId,
    effect: EffectId,
    macro_: u8,
    maps: Vec<MacroMap>,
) -> Result<Command, CommandError> {
    let m = usize::from(macro_);
    if m >= MACROS {
        return Err(refuse("a macro past the rail"));
    }
    let e = board_slot(project, strip, effect)?;
    let b = crate::fx::tabula::board(&e.spec);
    for (i, map) in maps.iter().enumerate() {
        let twice = maps[..i]
            .iter()
            .any(|x| x.pedal == map.pedal && x.key == map.key);
        if twice || !map_ok(b, m, map) {
            return Err(refuse("a macro map"));
        }
    }
    let b = board_of(e);
    if b.macros.len() <= m {
        b.macros.resize_with(m + 1, Macro::default);
    }
    let old = std::mem::replace(&mut b.macros[m].maps, maps);
    settle(e);
    Ok(Command::SetMacroMaps {
        strip,
        effect,
        macro_,
        maps: old,
    })
}

pub(crate) fn set_macro_name(
    project: &mut Project,
    strip: StripId,
    effect: EffectId,
    macro_: u8,
    name: Option<String>,
) -> Result<Command, CommandError> {
    let m = usize::from(macro_);
    if m >= MACROS {
        return Err(refuse("a macro past the rail"));
    }
    // A name of nothing but spaces is no name: "Macro n" again.
    let name = name.filter(|n| !n.trim().is_empty());
    let e = board_slot(project, strip, effect)?;
    let b = board_of(e);
    if b.macros.len() <= m {
        b.macros.resize_with(m + 1, Macro::default);
    }
    let old = std::mem::replace(&mut b.macros[m].name, name);
    settle(e);
    Ok(Command::SetMacroName {
        strip,
        effect,
        macro_,
        name: old,
    })
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
    let Some(id) = pedal else {
        return Err(refuse("no pedal"));
    };
    let p = pedal_mut(project, strip, effect, id)?;
    if !p.spec.kind.takes_ir() && ir.is_some() {
        return Err(CommandError::InvalidValue("impulse response".into()));
    }
    if ir.as_ref().is_some_and(|f| f.path.trim().is_empty()) {
        return Err(CommandError::InvalidValue("impulse response path".into()));
    }
    let old = std::mem::replace(&mut p.spec.ir, ir);
    Ok(Command::SetEffectIr {
        strip,
        effect,
        ir: old,
        pedal,
    })
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
    let Some(id) = pedal else {
        return Err(refuse("no pedal"));
    };
    let p = pedal_mut(project, strip, effect, id)?;
    if p.spec.kind != EffectKind::TimeShaper {
        return Err(CommandError::InvalidValue("time pattern".into()));
    }
    if usize::from(slot) >= crate::fx::lapsus::SLOTS {
        return Err(CommandError::InvalidValue(format!("pattern slot {slot}")));
    }
    let pattern = pattern.map(|x| Box::new(x.sanitized(lane)));
    let bank = p.spec.lapsus.get_or_insert_with(Default::default);
    let old = match pattern {
        Some(x) => bank.lane_mut(lane).insert(slot, *x),
        None => bank.lane_mut(lane).remove(&slot),
    };
    if bank.is_empty() {
        p.spec.lapsus = None;
    }
    Ok(Command::SetLapsusPattern {
        strip,
        effect,
        lane,
        slot,
        pattern: old.map(Box::new),
        pedal,
    })
}

/// A board's spec after `cmds` — the board commands, built for the strip and the effect
/// given — exactly as they would leave it in a project, checks and refusals included: for a
/// board that lives outside a project (the exported plug-in's, ADR 0020). The whole list or
/// nothing.
pub fn apply_to_spec(
    spec: &crate::EffectSpec,
    cmds: impl FnOnce(StripId, EffectId) -> Vec<Command>,
) -> Result<crate::EffectSpec, CommandError> {
    let mut p = Project::new("board");
    let id = EffectId(1);
    p.master.effects.push(EffectSlot {
        id,
        bypass: false,
        mix: 1.0,
        sidechain: None,
        midi_from: None,
        spec: spec.clone(),
    });
    for c in cmds(StripId::MASTER, id) {
        c.apply(&mut p)?;
    }
    Ok(p.master.effects.remove(0).spec)
}

/// The pedal ids a project still names on board `effect` outside the board itself — its
/// automation lanes' `p{id}.…` keys — so a fresh id never takes one of them over
/// (`fx::tabula::next_id`).
pub fn referenced_ids(project: &Project, effect: EffectId) -> Vec<PedalId> {
    let mut out = Vec::new();
    for t in &project.tracks {
        for c in &t.clips {
            if let Some(a) = &c.automation {
                if let crate::AutoTarget::Effect { effect: e, key } = &a.target {
                    if *e == effect {
                        if let Some((id, _)) = crate::fx::tabula::split_key(key) {
                            out.push(id);
                        }
                    }
                }
            }
        }
    }
    out
}

#[cfg(test)]
#[path = "command_board_tests.rs"]
mod tests;

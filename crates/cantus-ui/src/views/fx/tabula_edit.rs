//! Tabula's edits from its window (ADR 0134 §2): each gesture on the board is one step of
//! undo — a footswitch, a pedal added, taken out, moved or onto a lane, a shape, a macro's
//! maps or name, a randomised board. In Cantus they run through the project's bus; in the
//! exported plug-in the host applies them to its own board (`FxHost::board_commands`).

use super::host::FxHost;
use cantus_core::fx::tabula::{
    board, next_id, MacroMap, Path, Pedal, PedalId, PedalShape, MACROS, MAX_PEDALS,
};
use cantus_core::{Command, EffectId, EffectKind, StripId};

/// Run the commands `build` makes as one step of undo (`label_key` names a batch).
pub fn run(
    host: &mut dyn FxHost,
    label_key: &str,
    build: impl Fn(StripId, EffectId) -> Vec<Command>,
) {
    if let Some((app, strip, effect)) = host.daw() {
        let mut cmds = build(strip, effect);
        let cmd = match cmds.len() {
            0 => return,
            1 => cmds.pop().expect("one"),
            _ => Command::Batch {
                label_key: label_key.into(),
                commands: cmds,
            },
        };
        app.exec(cmd);
        return;
    }
    host.board_commands(label_key, &build);
}

/// A fresh pedal id for the board `host` edits: past every pedal's and every id its
/// project's lanes still name (a leftover lane never attaches to another unit).
pub fn fresh_id(host: &mut dyn FxHost) -> Option<PedalId> {
    let referenced = match host.daw() {
        Some((app, _, effect)) => cantus_core::command_board::referenced_ids(app.project(), effect),
        None => Vec::new(),
    };
    next_id(board(host.spec()), referenced)
}

/// A pedal of `kind` added to the board: at the chain's end (after the split when the board
/// has one), sounding at once (its unit's starting preset, ADR 0071). `false` when the board
/// is full or the kind may not be a pedal.
pub fn add(host: &mut dyn FxHost, kind: EffectKind) -> Option<PedalId> {
    if !cantus_core::fx::tabula::pedal_allowed(kind) {
        return None;
    }
    let spec = host.spec().clone();
    let b = board(&spec);
    if b.pedals.len() >= MAX_PEDALS {
        return None;
    }
    let id = fresh_id(host)?;
    let path = if b.has_split() || b.pedals.iter().any(|p| p.path == Path::Post) {
        Path::Post
    } else {
        Path::Pre
    };
    let index = b.pedals.len();
    let mut pedal = Pedal::new(id, cantus_core::starting::starting_spec(kind));
    pedal.path = path;
    run(host, "cmd.insert_pedal", |strip, effect| {
        vec![Command::InsertPedal {
            strip,
            effect,
            index,
            pedal: Box::new(pedal.clone()),
            maps: Vec::new(),
        }]
    });
    Some(id)
}

pub fn remove(host: &mut dyn FxHost, pedal: PedalId) {
    run(host, "cmd.remove_pedal", |strip, effect| {
        vec![Command::RemovePedal {
            strip,
            effect,
            pedal,
        }]
    });
}

/// A pedal to `index` of the signal order and onto `path`: one step.
pub fn move_to(host: &mut dyn FxHost, pedal: PedalId, index: usize, path: Path) {
    run(host, "cmd.move_pedal", |strip, effect| {
        vec![Command::MovePedal {
            strip,
            effect,
            pedal,
            index,
            path,
        }]
    });
}

pub fn footswitch(host: &mut dyn FxHost, pedal: PedalId, on: bool) {
    run(host, "cmd.pedal_on", |strip, effect| {
        vec![Command::SetPedalOn {
            strip,
            effect,
            pedal,
            on,
        }]
    });
}

pub fn shape(host: &mut dyn FxHost, pedal: PedalId, shape: Option<PedalShape>) {
    run(host, "cmd.pedal_shape", |strip, effect| {
        vec![Command::SetPedalShape {
            strip,
            effect,
            pedal,
            shape,
        }]
    });
}

/// Map `key` of `pedal` to macro `m` (taken off any other macro that moved it), sweeping it
/// from where it stands to the far end of its range that is further: one step.
pub fn learn(host: &mut dyn FxHost, m: u8, pedal: PedalId, key: &str) {
    let spec = host.spec().clone();
    let b = board(&spec);
    let pk = cantus_core::fx::tabula::pedal_key(pedal, key);
    let Some(info) = spec.param(&pk) else {
        return;
    };
    let now = spec.get(&pk);
    let to = if now - info.min > info.max - now {
        info.min
    } else {
        info.max
    };
    let map = MacroMap {
        pedal,
        key: key.into(),
        from: now,
        to,
    };
    let mut lists: Vec<(u8, Vec<MacroMap>)> = Vec::new();
    for i in 0..MACROS as u8 {
        let mut maps = b
            .macro_at(i as usize)
            .map(|x| x.maps.clone())
            .unwrap_or_default();
        let before = maps.len();
        maps.retain(|x| !(x.pedal == pedal && x.key == key));
        if i == m {
            maps.push(map.clone());
            lists.push((i, maps));
        } else if maps.len() != before {
            lists.push((i, maps));
        }
    }
    // The map taken off another macro first, so the target is never moved by two.
    lists.sort_by_key(|(i, _)| u8::from(*i == m));
    run(host, "cmd.macro_maps", |strip, effect| {
        lists
            .iter()
            .map(|(i, maps)| Command::SetMacroMaps {
                strip,
                effect,
                macro_: *i,
                maps: maps.clone(),
            })
            .collect()
    });
}

/// Every pedal's face knobs given new values within musical ranges, from `seed`: one step.
pub fn randomise(host: &mut dyn FxHost, seed: u64) {
    let spec = host.spec().clone();
    let mut keys: Vec<(PedalId, &str)> = Vec::new();
    for p in spec.pedals() {
        if p.spec.kind == EffectKind::Pedal {
            let m =
                super::fragor::model_of(p.spec.get(cantus_core::fx::fragor::SLOTS[0])).unwrap_or(0);
            keys.extend(cantus_core::fx::fragor::KNOBS[m].iter().map(|k| (p.id, *k)));
        } else if let Some(f) = super::tabula_faces::of(p.spec.kind) {
            keys.extend(f.knobs.iter().map(|k| (p.id, *k)));
        }
    }
    let values = cantus_core::fx::tabula_random::randomise(&spec, &keys, seed);
    run(host, "cmd.tabula_randomise", |strip, effect| {
        values
            .iter()
            .map(|(key, value)| Command::SetEffectParam {
                strip,
                effect,
                key: key.clone(),
                value: *value,
            })
            .collect()
    });
}

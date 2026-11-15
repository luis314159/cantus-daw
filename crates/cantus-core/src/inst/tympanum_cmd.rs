//! Tympanum's edits as commands (ADR 0124, ADR 0004): every one a batch of the commands
//! every native instrument has (`SetInstrumentText`, `SetInstrumentValue`,
//! `ClearInstrumentValue`, `SetInstrumentOutput`), so each has its exact inverse by
//! construction and is one undo step.
//!
//! - [`change_kit`]: another kit, **its routes and its mixer carried** — each output on a
//!   strip goes to the new kit's output of the same name, else of the same family (ADR
//!   0123 §1), with its `o{n}.` keys; a piece's `p{n}.` keys go to the new kit's piece of
//!   the same kind; the old kit's bleed cells and mic moves, which mean nothing in the new
//!   one, are cleared.
//! - [`move_mic`], [`set_bleed`]: a modelled kit's mic moved, one bleed cell set.

use super::super::tympanum_kit::{self as kitdef, KitDef};
use super::{bleed_key, KitRef, KIT_KEY};
use crate::command::Command;
use crate::inst::sampler::split_indexed;
use crate::inst_out::{output_key, OutputId, OutputSlot};
use crate::model::{Project, TrackId};

/// A kit's definition, if Cantus knows it now (a modelled kit's table; a recorded kit the
/// app's loader has read).
pub fn definition(kit: &KitRef) -> Option<KitDef> {
    match kit {
        KitRef::Modelled(id) => kitdef::modelled_kit(id),
        KitRef::Pack { .. } => kitdef::recorded(&kit.text()).map(|k| k.def.clone()),
    }
}

/// The command that gives `track`'s Tympanum the kit `to`, carrying what can be carried.
/// `None`: not a Tympanum, or already that kit.
///
/// The old kit's outputs are its definition's mics, or — its pack missing — the slots the
/// song keeps (each with its name and family), so a song whose kit is gone still carries
/// its routes by name. A new kit Cantus cannot read (not installed, unreadable) changes
/// only the kit's name: the routes and the mixer stay as they are, nothing dropped.
/// The app reads a pack's kit first (`cantus_ui`'s `tympanum::kit_change`), so a kit not
/// registered yet is not mistaken for one that cannot be read.
pub fn change_kit(project: &Project, track: TrackId, to: &KitRef) -> Option<Command> {
    let n = project.track(track)?.instrument.native()?;
    if n.kind != crate::InstrumentKind::AcousticDrums {
        return None;
    }
    let from = KitRef::of(n);
    if &from == to {
        return None;
    }
    let mut commands = vec![Command::SetInstrumentText {
        track,
        key: KIT_KEY.into(),
        text: Some(to.text()),
    }];
    let (old, Some(new)) = (definition(&from), definition(to)) else {
        return Some(Command::Batch {
            label_key: "cmd.tympanum_kit".into(),
            commands,
        });
    };
    let new = Some(new);
    let olds: Vec<(OutputId, String, crate::inst_out::OutputFamily)> = match &old {
        Some(o) => o
            .mics
            .iter()
            .map(|m| (m.output, m.name.clone(), m.family))
            .collect(),
        None => n
            .outputs
            .iter()
            .map(|s| (s.id, s.name.clone(), s.family))
            .collect(),
    };
    // Outputs: each old output → the new kit's output of the same name, else family.
    // Names first for every output, then families, so a family match never takes the
    // output another one's own name asks for.
    let mut map: Vec<(OutputId, OutputId)> = Vec::new();
    if let Some(new) = &new {
        let mut taken: Vec<OutputId> = Vec::new();
        for (id, name, _) in &olds {
            if let Some(x) = new
                .mics
                .iter()
                .find(|x| x.name.eq_ignore_ascii_case(name) && !taken.contains(&x.output))
            {
                taken.push(x.output);
                map.push((*id, x.output));
            }
        }
        for (id, _, family) in &olds {
            if map.iter().any(|(a, _)| a == id) {
                continue;
            }
            if let Some(x) = new
                .mics
                .iter()
                .find(|x| x.family == *family && !taken.contains(&x.output))
            {
                taken.push(x.output);
                map.push((*id, x.output));
            }
        }
    }
    let to_new = |id: OutputId| map.iter().find(|(a, _)| *a == id).map(|(_, b)| *b);
    for s in &n.outputs {
        commands.push(Command::SetInstrumentOutput {
            track,
            id: s.id,
            slot: None,
        });
    }
    for s in &n.outputs {
        if let (Some(id), Some(new)) = (to_new(s.id), &new) {
            let Some(m) = new.mics.iter().find(|x| x.output == id) else {
                continue;
            };
            commands.push(Command::SetInstrumentOutput {
                track,
                id,
                slot: Some(OutputSlot {
                    id,
                    name: m.name.clone(),
                    family: m.family,
                    strip: s.strip,
                }),
            });
        }
    }
    // Keys: outputs' by the map, pieces' by kind, the rest of the old kit cleared.
    let piece_to = |p: usize| -> Option<usize> {
        let (old, new) = (old.as_ref()?, new.as_ref()?);
        let kind = old.pieces.get(p)?.kind;
        let nth = old.pieces[..p].iter().filter(|x| x.kind == kind).count();
        new.pieces
            .iter()
            .enumerate()
            .filter(|(_, x)| x.kind == kind)
            .nth(nth)
            .map(|(i, _)| i)
    };
    let mut sets: Vec<(String, f32)> = Vec::new();
    for (key, v) in &n.params {
        let carried = if let Some((id, f)) = output_key(key) {
            to_new(OutputId(id as u16)).map(|o| crate::inst_out::key(o, f))
        } else if bleed_key(key).is_some() {
            None
        } else if let Some((m, _)) = split_indexed(key, b'm', super::MAX_MICS) {
            let _ = m;
            None
        } else if let Some((p, field)) = split_indexed(key, b'p', super::MAX_PIECES) {
            piece_to(p).map(|q| format!("p{q}.{field}"))
        } else {
            continue;
        };
        commands.push(Command::ClearInstrumentValue {
            track,
            key: key.clone(),
        });
        if let Some(k) = carried {
            sets.push((k, *v));
        }
    }
    for (key, value) in sets {
        commands.push(Command::SetInstrumentValue { track, key, value });
    }
    Some(Command::Batch {
        label_key: "cmd.tympanum_kit".into(),
        commands,
    })
}

/// Move a modelled kit's mic `mic` to `(x, y, z)` cm from its place, aimed `az`, `el`
/// degrees from its aim: one step.
pub fn move_mic(track: TrackId, mic: usize, at: [f32; 5]) -> Command {
    let fields = ["x", "y", "z", "az", "el"];
    Command::Batch {
        label_key: "cmd.tympanum_mic".into(),
        commands: fields
            .iter()
            .zip(at)
            .map(|(f, value)| Command::SetInstrumentValue {
                track,
                key: format!("m{mic}.{f}"),
                value,
            })
            .collect(),
    }
}

/// One cell of the piece × mic bleed matrix: its level (dB), polarity and power.
pub fn set_bleed(
    track: TrackId,
    piece: usize,
    mic: usize,
    db: f32,
    inv: bool,
    on: bool,
) -> Command {
    let k = |f: &str| format!("b{piece}.{mic}.{f}");
    Command::Batch {
        label_key: "cmd.tympanum_bleed".into(),
        commands: vec![
            Command::SetInstrumentValue {
                track,
                key: k("db"),
                value: db,
            },
            Command::SetInstrumentValue {
                track,
                key: k("inv"),
                value: f32::from(u8::from(inv)),
            },
            Command::SetInstrumentValue {
                track,
                key: k("on"),
                value: f32::from(u8::from(on)),
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inst_out::OutputFamily;
    use crate::{InstrumentKind, InstrumentSpec, NativeSpec};

    #[test]
    fn a_kit_change_carries_routes_by_name_then_family_and_undoes_exactly() {
        let mut p = Project::new("t");
        let oh = p.make_strip("OH");
        let oh_id = oh.id;
        p.strips.push(oh);
        let mut t = p.make_track("Drums");
        let mut spec = NativeSpec::new(InstrumentKind::AcousticDrums);
        // The studio kit's Overheads (9) on strip 77, its level −3 dB; a bleed cell set.
        spec.outputs.push(OutputSlot {
            id: OutputId(9),
            name: "Overheads".into(),
            family: OutputFamily::Overheads,
            strip: oh_id,
        });
        spec.set("o9.level", -3.0);
        spec.set("b1.2.db", -6.0);
        spec.set("p1.tune", 2.0);
        t.instrument = InstrumentSpec::Native(spec);
        let id = t.id;
        p.tracks.push(t);
        // A recorded kit whose overheads are named "OH".
        let mut def = kitdef::modelled_kit("studio").unwrap();
        def.kit = KitRef::Pack {
            pack: "cmd-test".into(),
            kit: "kit".into(),
        };
        def.mics
            .retain(|m| matches!(m.family, OutputFamily::Kick | OutputFamily::Overheads));
        for (i, m) in def.mics.iter_mut().enumerate() {
            m.output = OutputId(i as u16);
        }
        def.mics[2].name = "OH".into();
        kitdef::register(kitdef::RecordedKit {
            def,
            hits: Default::default(),
            log: Vec::new(),
        })
        .unwrap();
        let to = KitRef::Pack {
            pack: "cmd-test".into(),
            kit: "kit".into(),
        };
        let cmd = change_kit(&p, id, &to).expect("a change");
        let mut after = p.clone();
        let inverse = cmd.apply(&mut after).expect("applies");
        let n = after.track(id).unwrap().instrument.native().unwrap();
        assert_eq!(KitRef::of(n), to);
        assert_eq!(n.outputs.len(), 1);
        assert_eq!(n.outputs[0].id, OutputId(2));
        assert_eq!(n.outputs[0].strip, oh_id);
        assert_eq!(n.get("o2.level"), -3.0);
        assert!(!n.is_stored("b1.2.db"));
        assert_eq!(
            n.get("p1.tune"),
            2.0,
            "the snare's tune goes to the new snare"
        );
        inverse.apply(&mut after).expect("undoes");
        assert_eq!(after, p);
    }

    /// Review of 40a27bb7, finding 1: a song whose pack is gone still carries its routes by
    /// the slots' names; a kit Cantus cannot read changes the name alone, keeping the mixer.
    #[test]
    fn an_unknown_old_kit_carries_by_its_slots_and_an_unknown_new_kit_keeps_everything() {
        let mut p = Project::new("t");
        let oh = p.make_strip("Overheads");
        let oh_id = oh.id;
        p.strips.push(oh);
        let mut t = p.make_track("Drums");
        let mut spec = NativeSpec::new(InstrumentKind::AcousticDrums);
        spec.set_text(super::KIT_KEY, Some("pack:gone-pack/kit".into()))
            .unwrap();
        spec.outputs.push(OutputSlot {
            id: OutputId(4),
            name: "Overheads".into(),
            family: OutputFamily::Overheads,
            strip: oh_id,
        });
        spec.set("o4.level", -2.0);
        t.instrument = InstrumentSpec::Native(spec);
        let id = t.id;
        p.tracks.push(t);
        let studio = KitRef::Modelled("studio".into());
        let def = kitdef::modelled_kit("studio").unwrap();
        let oh_new = def
            .mics
            .iter()
            .find(|m| m.name == "Overheads")
            .unwrap()
            .output;
        let cmd = change_kit(&p, id, &studio).expect("a change");
        let mut after = p.clone();
        let inverse = cmd.apply(&mut after).unwrap();
        let n = after.track(id).unwrap().instrument.native().unwrap();
        assert_eq!(n.outputs.len(), 1);
        assert_eq!(n.outputs[0].id, oh_new);
        assert_eq!(n.outputs[0].strip, oh_id);
        assert_eq!(
            n.get(&crate::inst_out::key(
                oh_new,
                crate::inst_out::OutputField::Level
            )),
            -2.0
        );
        inverse.apply(&mut after).unwrap();
        assert_eq!(after, p);
        // To a kit nobody can read: the name changes, the slot and its level stay.
        let gone = KitRef::Pack {
            pack: "never-installed".into(),
            kit: "kit".into(),
        };
        let mut after = p.clone();
        change_kit(&p, id, &gone)
            .unwrap()
            .apply(&mut after)
            .unwrap();
        let n = after.track(id).unwrap().instrument.native().unwrap();
        assert_eq!(KitRef::of(n), gone);
        assert_eq!(
            n.outputs,
            p.track(id).unwrap().instrument.native().unwrap().outputs
        );
        assert_eq!(n.get("o4.level"), -2.0);
    }
}

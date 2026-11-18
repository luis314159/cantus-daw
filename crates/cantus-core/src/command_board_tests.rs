//! Tabula's board commands: each undone and redone exactly, and what a board refuses.

use crate::command::{Command, Origin};
use crate::fx::tabula::{guitar_rig, MacroMap, Path, Pedal, PedalId, PedalShape};
use crate::model::{EffectId, Project, StripId};
use crate::{CommandBus, EffectKind, EffectSpec};

/// A project with the guitar rig on the master's chain.
fn rig() -> (CommandBus, EffectId) {
    let mut p = Project::new("board");
    let mut slot = p.make_effect(EffectKind::Board);
    slot.spec = guitar_rig();
    let id = slot.id;
    p.master.effects.push(slot);
    (CommandBus::new(p), id)
}

fn map(pedal: u16, key: &str) -> MacroMap {
    MacroMap {
        pedal: PedalId(pedal),
        key: key.into(),
        from: 0.1,
        to: 0.9,
    }
}

/// Run `cmd`, undo it and redo it: undo gives back the project exactly, redo the result.
fn round_trip(bus: &mut CommandBus, cmd: Command) {
    let before = bus.project().clone();
    bus.execute(cmd.clone(), Origin::User)
        .unwrap_or_else(|e| panic!("{cmd:?}: {e:?}"));
    let after = bus.project().clone();
    assert_ne!(after, before, "{cmd:?} changed nothing");
    assert!(bus.undo().is_some());
    assert_eq!(*bus.project(), before, "undo of {cmd:?} is exact");
    assert!(bus.redo().is_some());
    assert_eq!(*bus.project(), after, "redo of {cmd:?} is exact");
}

#[test]
fn every_board_command_round_trips_exactly() {
    let (mut bus, e) = rig();
    let s = StripId::MASTER;
    let cmds = [
        Command::SetMacroMaps {
            strip: s,
            effect: e,
            macro_: 2,
            maps: vec![map(0, "od.drive"), map(2, "time_ms")],
        },
        Command::SetMacroName {
            strip: s,
            effect: e,
            macro_: 2,
            name: Some("Drive".into()),
        },
        // Removing a mapped pedal takes its maps with it; the undo puts both back.
        Command::RemovePedal {
            strip: s,
            effect: e,
            pedal: PedalId(0),
        },
        Command::InsertPedal {
            strip: s,
            effect: e,
            index: 1,
            pedal: Box::new(Pedal::new(PedalId(7), EffectSpec::new(EffectKind::Phaser))),
            maps: vec![(5, 0, map(7, "rate"))],
        },
        Command::MovePedal {
            strip: s,
            effect: e,
            pedal: PedalId(2),
            index: 0,
            path: Path::B,
        },
        Command::SetPedalOn {
            strip: s,
            effect: e,
            pedal: PedalId(1),
            on: false,
        },
        Command::SetPedalShape {
            strip: s,
            effect: e,
            pedal: PedalId(3),
            shape: Some(PedalShape::Double),
        },
        Command::SetEffectParam {
            strip: s,
            effect: e,
            key: "p2.feedback".into(),
            value: 0.6,
        },
        Command::ClearEffectParam {
            strip: s,
            effect: e,
            key: "p2.time_ms".into(),
        },
    ];
    // Each is left applied (redone), so the next one meets a board that has changed.
    for cmd in cmds {
        round_trip(&mut bus, cmd);
    }
}

/// A pedal's response and its Lapsus bank go to that pedal, and come back on undo.
#[test]
fn a_pedals_response_and_patterns_are_its_own() {
    let (mut bus, e) = rig();
    let s = StripId::MASTER;
    for (id, kind) in [
        (8, EffectKind::Aula),
        (9, EffectKind::TimeShaper),
        (10, EffectKind::Ducker), // fxb:cedo
    ] {
        bus.execute(
            Command::InsertPedal {
                strip: s,
                effect: e,
                index: 4,
                pedal: Box::new(Pedal::new(PedalId(id), EffectSpec::new(kind))),
                maps: Vec::new(),
            },
            Origin::User,
        )
        .unwrap();
    }
    round_trip(
        &mut bus,
        Command::SetEffectIr {
            strip: s,
            effect: e,
            ir: Some(Box::new(crate::IrFile::new("irs/spring.wav"))),
            pedal: Some(PedalId(8)),
        },
    );
    round_trip(
        &mut bus,
        Command::SetLapsusPattern {
            strip: s,
            effect: e,
            lane: crate::fx::lapsus::Lane::Time,
            slot: 3,
            pattern: Some(Box::new(crate::fx::lapsus::Pattern {
                len: 2,
                points: vec![crate::fx::lapsus::Point::new(0.5, 0.5)],
                name: None,
            })),
            pedal: Some(PedalId(9)),
        },
    );
    // fxb:cedo — a Cedo pedal's drawn shape, sanitized on the way in, undone exactly.
    round_trip(
        &mut bus,
        Command::SetCedoShape {
            strip: s,
            effect: e,
            shape: Some(Box::new(crate::fx::cedo::Shape {
                points: vec![
                    crate::fx::lapsus::Point::new(0.6, 2.0),
                    crate::fx::lapsus::Point::new(0.0, 1.0),
                ],
            })),
            pedal: Some(PedalId(10)),
        },
    );
    let shape = crate::fx::tabula::board(&bus.project().master.effects[0].spec)
        .pedals
        .iter()
        .find(|p| p.id == PedalId(10))
        .unwrap()
        .spec
        .cedo
        .clone()
        .expect("redone: the shape drawn");
    assert_eq!(
        shape.points,
        vec![
            crate::fx::lapsus::Point::new(0.0, 1.0),
            crate::fx::lapsus::Point::new(0.6, 1.0),
        ],
        "sanitized: in order, in range"
    );
    // Onto a pedal of another kind: refused.
    for cmd in [
        Command::SetCedoShape {
            strip: s,
            effect: e,
            shape: None,
            pedal: Some(PedalId(9)),
        },
        Command::SetEffectIr {
            strip: s,
            effect: e,
            ir: Some(Box::new(crate::IrFile::new("irs/spring.wav"))),
            pedal: Some(PedalId(2)),
        },
        Command::SetLapsusPattern {
            strip: s,
            effect: e,
            lane: crate::fx::lapsus::Lane::Time,
            slot: 3,
            pattern: None,
            pedal: Some(PedalId(8)),
        },
    ] {
        assert!(bus.execute(cmd, Origin::User).is_err());
    }
}

/// Taking every pedal off leaves no board at all (`None`), and the undo the rig.
#[test]
fn an_emptied_board_is_none_and_comes_back() {
    let (mut bus, e) = rig();
    let rig = bus.project().master.effects[0].spec.clone();
    for id in 0..4 {
        bus.execute(
            Command::RemovePedal {
                strip: StripId::MASTER,
                effect: e,
                pedal: PedalId(id),
            },
            Origin::User,
        )
        .unwrap();
    }
    assert!(bus.project().master.effects[0].spec.board.is_none());
    for _ in 0..4 {
        bus.undo();
    }
    assert_eq!(bus.project().master.effects[0].spec, rig);
}

#[test]
fn what_a_board_refuses() {
    let (mut bus, e) = rig();
    let s = StripId::MASTER;
    let insert = |id: u16, kind| Command::InsertPedal {
        strip: s,
        effect: e,
        index: 0,
        pedal: Box::new(Pedal::new(PedalId(id), EffectSpec::new(kind))),
        maps: Vec::new(),
    };
    let before = bus.project().clone();
    let mut nested = Pedal::new(PedalId(9), EffectSpec::new(EffectKind::Delay));
    nested.spec.board = Some(Box::default());
    let refused = [
        insert(0, EffectKind::Delay),    // an id the board holds
        insert(9, EffectKind::Board),    // no nesting
        insert(9, EffectKind::External), // no hosted plug-in
        insert(9, EffectKind::Analyzer), // its rings are the slot's
        Command::InsertPedal {
            strip: s,
            effect: e,
            index: 0,
            pedal: Box::new(nested),
            maps: Vec::new(),
        },
        Command::InsertPedal {
            strip: s,
            effect: e,
            index: 0,
            pedal: Box::new(Pedal::new(PedalId(9), EffectSpec::new(EffectKind::Delay))),
            maps: vec![(0, 0, map(9, "rate"))], // a key a delay has not
        },
        Command::RemovePedal {
            strip: s,
            effect: e,
            pedal: PedalId(42),
        },
        Command::SetMacroMaps {
            strip: s,
            effect: e,
            macro_: 8,
            maps: Vec::new(),
        },
        Command::SetMacroMaps {
            strip: s,
            effect: e,
            macro_: 0,
            maps: vec![map(1, "no_such_key")],
        },
        Command::SetMacroMaps {
            strip: s,
            effect: e,
            macro_: 0,
            maps: vec![map(0, "od.drive"), map(0, "od.drive")],
        },
        Command::SetMacroMaps {
            strip: s,
            effect: e,
            macro_: 0,
            maps: vec![MacroMap {
                from: f32::NAN,
                ..map(0, "od.drive")
            }],
        },
        Command::SetEffectParam {
            strip: s,
            effect: e,
            key: "p42.out".into(),
            value: 1.0,
        },
    ];
    for cmd in refused {
        assert!(bus.execute(cmd.clone(), Origin::User).is_err(), "{cmd:?}");
    }
    assert_eq!(*bus.project(), before, "a refusal leaves nothing behind");
    // One macro per target.
    bus.execute(
        Command::SetMacroMaps {
            strip: s,
            effect: e,
            macro_: 0,
            maps: vec![map(0, "od.drive")],
        },
        Origin::User,
    )
    .unwrap();
    assert!(bus
        .execute(
            Command::SetMacroMaps {
                strip: s,
                effect: e,
                macro_: 1,
                maps: vec![map(0, "od.drive")],
            },
            Origin::User
        )
        .is_err());
    // The seventeenth.
    for id in 4..16 {
        bus.execute(insert(id, EffectKind::Tremolo), Origin::User)
            .unwrap();
    }
    assert!(bus
        .execute(insert(16, EffectKind::Tremolo), Origin::User)
        .is_err());
    // A slot that is not a board.
    let mut p = Project::new("x");
    let slot = p.make_effect(EffectKind::Delay);
    let id = slot.id;
    p.master.effects.push(slot);
    let mut bus = CommandBus::new(p);
    assert!(bus
        .execute(
            Command::SetPedalOn {
                strip: s,
                effect: id,
                pedal: PedalId(0),
                on: false
            },
            Origin::User
        )
        .is_err());
}

/// A name of spaces is no name, and naming a macro on an empty board makes the board.
#[test]
fn macro_names_are_canonical() {
    let mut p = Project::new("x");
    let slot = p.make_effect(EffectKind::Board);
    let e = slot.id;
    p.master.effects.push(slot);
    let mut bus = CommandBus::new(p);
    let name = |n: Option<&str>| Command::SetMacroName {
        strip: StripId::MASTER,
        effect: e,
        macro_: 3,
        name: n.map(Into::into),
    };
    round_trip(&mut bus, name(Some("Wah")));
    bus.execute(name(Some("Wah")), Origin::User).unwrap();
    let b = bus.project().master.effects[0].spec.board.clone().unwrap();
    assert_eq!(b.macros.len(), 4);
    assert_eq!(b.macros[3].name.as_deref(), Some("Wah"));
    bus.execute(name(Some("   ")), Origin::User).unwrap();
    assert!(
        bus.project().master.effects[0].spec.board.is_none(),
        "no name, no maps, no pedals: no board"
    );
}

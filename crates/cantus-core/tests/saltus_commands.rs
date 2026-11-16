//! Saltus, the pitch unit (ADR 0135, provisional): every control is a `SetEffectParam` with
//! an exact inverse — on an insert and on a pedal of Tabula's board (`p{id}.` keys) — and
//! its starting preset names only its own keys, inside their ranges. The undo fuzz reaches
//! the same commands at random (`undo_invariants.rs`, every kind's table); this holds each
//! key of Saltus's at once, at the ends of its range and past them. // fx3:saltus

use cantus_core::fx::saltus::PARAMS;
use cantus_core::fx::tabula::{pedal_key, Board, Pedal, PedalId};
use cantus_core::starting::{start_of, Start};
use cantus_core::*;

fn song() -> (CommandBus, StripId, EffectId, EffectId) {
    let mut p = Project::new("Pitch");
    let strip = p.strips[0].id;
    let insert = p.make_effect(EffectKind::PitchShift);
    let mut board = p.make_effect(EffectKind::Board);
    board.spec.board = Some(Box::new(Board {
        pedals: vec![Pedal::new(
            PedalId(3),
            EffectSpec::new(EffectKind::PitchShift),
        )],
        macros: Vec::new(),
    }));
    let (a, b) = (insert.id, board.id);
    let s = p.strips.iter_mut().find(|s| s.id == strip).unwrap();
    s.effects.push(insert);
    s.effects.push(board);
    (CommandBus::new(p), strip, a, b)
}

#[test]
fn every_key_on_an_insert_and_on_a_pedal_undoes_exactly() {
    let (mut bus, strip, insert, board) = song();
    let start = bus.project().clone();
    let mut applied = 0;
    for info in PARAMS {
        for (effect, key) in [
            (insert, info.key.to_string()),
            (board, pedal_key(PedalId(3), info.key)),
        ] {
            for value in [info.min, info.max, info.min - 5.0, info.max + 5.0] {
                let before = bus.project().clone();
                let r = bus.execute(
                    Command::SetEffectParam {
                        strip,
                        effect,
                        key: key.clone(),
                        value,
                    },
                    Origin::User,
                );
                if r.is_err() {
                    assert_eq!(
                        bus.project(),
                        &before,
                        "{key} = {value}: refused, untouched"
                    );
                    continue;
                }
                applied += 1;
                bus.undo();
                assert_eq!(bus.project(), &before, "{key} = {value}: undone exactly");
                bus.redo();
                bus.undo();
                assert_eq!(bus.project(), &before, "{key} = {value}: redone, undone");
            }
            bus.execute(
                Command::ClearEffectParam {
                    strip,
                    effect,
                    key: key.clone(),
                },
                Origin::User,
            )
            .ok();
        }
    }
    assert!(applied >= PARAMS.len() * 4, "only {applied} applied");
    while bus.undo().is_some() {}
    assert_eq!(bus.project(), &start, "undo all: the song as it was");
}

#[test]
fn the_starting_preset_sets_only_its_own_keys_in_range() {
    let Start::Preset(p) = start_of(EffectKind::PitchShift) else {
        panic!("Saltus starts on a preset (ADR 0071)");
    };
    assert_eq!(p.label_key, "preset.saltus.twelve_string");
    for (k, v) in p.params {
        let info = PARAMS.iter().find(|i| i.key == *k).expect(k);
        assert!(info.min <= *v && *v <= info.max, "{k} = {v}");
    }
}

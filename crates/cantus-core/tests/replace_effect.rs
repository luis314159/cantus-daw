//! Swapping one insert for another in its place (round 20, ADR 0029): `ReplaceEffect`
//! keeps the slot's position in the chain, is one step of the history, and undoes exactly.
//! What the new slot carries over (mix, bypass, a key, a note source) is the caller's
//! choice; the command refuses what the new kind cannot take, as its own commands do.

use cantus_core::*;

fn bus() -> (CommandBus, StripId, TrackId) {
    let mut p = Project::new("swap");
    let strip = p.strips[0].id;
    let t = p.make_track("Lead");
    let track = t.id;
    p.tracks.push(t);
    (CommandBus::new(p), strip, track)
}

fn insert(bus: &mut CommandBus, strip: StripId, kind: EffectKind) -> EffectId {
    let slot = bus.new_effect(kind);
    let id = slot.id;
    let index = bus.project().effects_of(strip).unwrap().len();
    bus.execute(
        Command::InsertEffect {
            strip,
            index,
            slot: Box::new(slot),
        },
        Origin::User,
    )
    .unwrap();
    id
}

fn kinds(bus: &CommandBus, strip: StripId) -> Vec<EffectKind> {
    bus.project()
        .effects_of(strip)
        .unwrap()
        .iter()
        .map(|e| e.spec.kind)
        .collect()
}

#[test]
fn a_replaced_insert_keeps_its_place_and_undoes_in_one_step() {
    let (mut bus, strip, _) = bus();
    insert(&mut bus, strip, EffectKind::Eq);
    let comp = insert(&mut bus, strip, EffectKind::Compressor);
    insert(&mut bus, strip, EffectKind::Reverb);
    let mut slot = bus.new_effect(EffectKind::Limiter);
    slot.mix = 0.4;
    slot.bypass = true;
    let new_id = slot.id;
    let before = bus.project().clone();
    bus.execute(
        Command::ReplaceEffect {
            strip,
            effect: comp,
            slot: Box::new(slot),
        },
        Origin::User,
    )
    .unwrap();
    assert_eq!(
        kinds(&bus, strip),
        [EffectKind::Eq, EffectKind::Limiter, EffectKind::Reverb]
    );
    let e = bus.project().effect(strip, new_id).unwrap();
    assert_eq!((e.mix, e.bypass), (0.4, true));
    assert!(bus.project().effect(strip, comp).is_none());
    let after = bus.project().clone();
    bus.undo().unwrap();
    assert_eq!(*bus.project(), before, "one undo puts the old insert back");
    bus.redo().unwrap();
    assert_eq!(*bus.project(), after);
}

#[test]
fn the_master_chain_swaps_too() {
    let (mut bus, _, _) = bus();
    let m = StripId::MASTER;
    let lim = insert(&mut bus, m, EffectKind::Limiter);
    let slot = bus.new_effect(EffectKind::Maximizer);
    bus.execute(
        Command::ReplaceEffect {
            strip: m,
            effect: lim,
            slot: Box::new(slot),
        },
        Origin::User,
    )
    .unwrap();
    assert_eq!(kinds(&bus, m), [EffectKind::Maximizer]);
}

#[test]
fn a_replacement_refuses_what_its_kind_cannot_take() {
    let (mut bus, strip, track) = bus();
    let other = bus.project().strips[1].id;
    let eq = insert(&mut bus, strip, EffectKind::Eq);
    let dup = insert(&mut bus, strip, EffectKind::Reverb);
    let before = bus.project().clone();
    let refused = |bus: &mut CommandBus, slot: EffectSlot| {
        let r = bus.execute(
            Command::ReplaceEffect {
                strip,
                effect: eq,
                slot: Box::new(slot),
            },
            Origin::User,
        );
        assert!(r.is_err(), "refused: {r:?}");
    };
    // A key into a kind that has no detector.
    let mut s = bus.new_effect(EffectKind::Reverb);
    s.sidechain = Some(other);
    refused(&mut bus, s);
    // Notes into a kind that hears none.
    let mut s = bus.new_effect(EffectKind::Eq);
    s.midi_from = Some(track);
    refused(&mut bus, s);
    // A key from itself.
    let mut s = bus.new_effect(EffectKind::Compressor);
    s.sidechain = Some(strip);
    refused(&mut bus, s);
    // An id already in the chain.
    let mut s = bus.new_effect(EffectKind::Gate);
    s.id = dup;
    refused(&mut bus, s);
    // A slot that is not there.
    let s = bus.new_effect(EffectKind::Gate);
    let r = bus.execute(
        Command::ReplaceEffect {
            strip,
            effect: EffectId(9_999),
            slot: Box::new(s),
        },
        Origin::User,
    );
    assert!(r.is_err());
    // Nothing was left behind (the ids handed out aside).
    let mut now = bus.project().clone();
    now.next_id = before.next_id;
    assert_eq!(now, before);
}

#[test]
fn a_key_and_a_note_source_carry_over_where_the_new_kind_takes_them() {
    let (mut bus, strip, track) = bus();
    let other = bus.project().strips[1].id;
    let comp = insert(&mut bus, strip, EffectKind::Compressor);
    bus.execute(
        Command::SetEffectSidechain {
            strip,
            effect: comp,
            source: Some(other),
        },
        Origin::User,
    )
    .unwrap();
    let mut gate = bus.new_effect(EffectKind::Gate);
    gate.sidechain = Some(other);
    gate.midi_from = Some(track);
    let gid = gate.id;
    bus.execute(
        Command::ReplaceEffect {
            strip,
            effect: comp,
            slot: Box::new(gate),
        },
        Origin::User,
    )
    .unwrap();
    let e = bus.project().effect(strip, gid).unwrap();
    assert_eq!((e.sidechain, e.midi_from), (Some(other), Some(track)));
}

//! The master's insert chain (ADR 0026): every effect command addresses it as
//! `StripId::MASTER`, through the one resolver (`Project::effects_of`), and undoes exactly.

use cantus_core::*;

fn bus_with_strip() -> (CommandBus, StripId) {
    let mut p = Project::new("master");
    let strip = p.strips[0].id;
    let t = p.make_track("Lead");
    p.tracks.push(t);
    (CommandBus::new(p), strip)
}

/// Run `cmd`, check it changed the project, undo it and check the project is exactly as it
/// was, redo it and check it is exactly as after.
fn round_trip(bus: &mut CommandBus, cmd: Command) {
    let before = bus.project().clone();
    bus.execute(cmd.clone(), Origin::User)
        .unwrap_or_else(|e| panic!("{cmd:?}: {e}"));
    let after = bus.project().clone();
    bus.undo().expect("undo");
    assert_eq!(*bus.project(), before, "undo of {cmd:?}");
    bus.redo().expect("redo");
    assert_eq!(*bus.project(), after, "redo of {cmd:?}");
}

fn insert(bus: &mut CommandBus, kind: EffectKind) -> EffectId {
    let slot = bus.new_effect(kind);
    let id = slot.id;
    let index = bus.project().master.effects.len();
    round_trip(
        bus,
        Command::InsertEffect {
            strip: StripId::MASTER,
            index,
            slot: Box::new(slot),
        },
    );
    id
}

#[test]
fn the_resolver_names_the_master_and_never_a_strip() {
    let mut p = Project::new("r");
    let lim = p.make_effect(EffectKind::Limiter);
    let lid = lim.id;
    p.master.effects.push(lim);
    let eq = p.make_effect(EffectKind::Eq);
    let eid = eq.id;
    let s = p.strips[2].id;
    p.strip_mut(s).unwrap().effects.push(eq);
    assert_eq!(p.effects_of(StripId::MASTER).unwrap().len(), 1);
    assert_eq!(p.effect(StripId::MASTER, lid).unwrap().id, lid);
    assert!(p.effect(StripId::MASTER, eid).is_none());
    assert_eq!(p.effect(s, eid).unwrap().id, eid);
    assert_eq!(p.effect_host(lid), Some(StripId::MASTER));
    assert_eq!(p.effect_host(eid), Some(s));
    assert!(
        p.strip(StripId::MASTER).is_none(),
        "the master is not a strip"
    );
    assert_eq!(p.fx_enabled(StripId::MASTER), Some(true));
    assert!(p.effects_of(StripId(9_999)).is_none());
}

#[test]
fn every_effect_command_acts_on_the_master_and_undoes_exactly() {
    let (mut bus, strip) = bus_with_strip();
    let lim = insert(&mut bus, EffectKind::Limiter);
    let comp = insert(&mut bus, EffectKind::Compressor);
    let gate = insert(&mut bus, EffectKind::Gate);
    assert_eq!(bus.project().master.effects.len(), 3);
    assert!(bus.project().strips.iter().all(|s| s.effects.is_empty()));
    let m = StripId::MASTER;
    round_trip(
        &mut bus,
        Command::MoveEffect {
            strip: m,
            effect: lim,
            index: 2,
        },
    );
    assert_eq!(bus.project().master.effects[2].id, lim);
    round_trip(
        &mut bus,
        Command::SetEffectBypass {
            strip: m,
            effect: comp,
            bypass: true,
        },
    );
    round_trip(
        &mut bus,
        Command::SetEffectParam {
            strip: m,
            effect: lim,
            key: "ceiling".into(),
            value: -6.0,
        },
    );
    assert_eq!(
        bus.project().effect(m, lim).unwrap().spec.get("ceiling"),
        -6.0
    );
    round_trip(
        &mut bus,
        Command::ClearEffectParam {
            strip: m,
            effect: lim,
            key: "ceiling".into(),
        },
    );
    round_trip(
        &mut bus,
        Command::SetEffectMix {
            strip: m,
            effect: comp,
            mix: 0.4,
        },
    );
    // A key into a master insert, from any strip: the master is always processed last.
    round_trip(
        &mut bus,
        Command::SetEffectSidechain {
            strip: m,
            effect: comp,
            source: Some(strip),
        },
    );
    assert_eq!(
        bus.project().effect(m, comp).unwrap().sidechain,
        Some(strip)
    );
    let track = bus.project().tracks[0].id;
    round_trip(
        &mut bus,
        Command::SetEffectMidiSource {
            strip: m,
            effect: gate,
            source: Some(track),
        },
    );
    round_trip(&mut bus, Command::SetMasterFxEnabled { enabled: false });
    assert!(!bus.project().master.fx_enabled);
    round_trip(
        &mut bus,
        Command::RemoveEffect {
            strip: m,
            effect: gate,
        },
    );
    assert!(bus.project().effect(m, gate).is_none());
    // Undo everything: the master is bare again.
    while bus.undo().is_some() {}
    assert!(bus.project().master.effects.is_empty());
    assert!(bus.project().master.fx_enabled);
}

#[test]
fn hosted_plug_in_commands_reach_the_master() {
    let (mut bus, _) = bus_with_strip();
    let mut slot = bus.new_effect(EffectKind::External);
    let plugin = ExternalPlugin::new(PluginFormat::Clap, "org.test.plugin", "Test");
    slot.spec = EffectSpec::external(plugin);
    let id = slot.id;
    round_trip(
        &mut bus,
        Command::InsertEffect {
            strip: StripId::MASTER,
            index: 0,
            slot: Box::new(slot),
        },
    );
    round_trip(
        &mut bus,
        Command::SetPluginParam {
            strip: StripId::MASTER,
            effect: id,
            id: 3,
            value: Some(0.25),
        },
    );
    let mut params = std::collections::BTreeMap::new();
    params.insert(1, 0.5);
    round_trip(
        &mut bus,
        Command::SetPluginState {
            strip: StripId::MASTER,
            effect: id,
            state: PluginState::new(vec![1, 2, 3]),
            params,
        },
    );
}

#[test]
fn the_master_is_never_a_key_and_its_keys_hold_their_strips() {
    let (mut bus, strip) = bus_with_strip();
    let comp = insert(&mut bus, EffectKind::Compressor);
    // A strip's insert keyed from the master: refused, nothing left behind.
    let slot = bus.new_effect(EffectKind::Compressor);
    let sc = slot.id;
    bus.execute(
        Command::InsertEffect {
            strip,
            index: 0,
            slot: Box::new(slot),
        },
        Origin::User,
    )
    .unwrap();
    let before = bus.project().clone();
    for (host, effect) in [(strip, sc), (StripId::MASTER, comp)] {
        let r = bus.execute(
            Command::SetEffectSidechain {
                strip: host,
                effect,
                source: Some(StripId::MASTER),
            },
            Origin::User,
        );
        assert!(
            matches!(r, Err(CommandError::RoutingCycle(_))),
            "{host:?}: {r:?}"
        );
        assert_eq!(*bus.project(), before);
    }
    // A strip the master is keyed from cannot be removed.
    let free = bus.project().strips.last().unwrap().id;
    bus.execute(
        Command::SetEffectSidechain {
            strip: StripId::MASTER,
            effect: comp,
            source: Some(free),
        },
        Origin::User,
    )
    .unwrap();
    let before = bus.project().clone();
    let r = bus.execute(Command::RemoveStrip { id: free }, Origin::User);
    assert!(matches!(r, Err(CommandError::StripInUse(_))), "{r:?}");
    assert_eq!(*bus.project(), before);
}

#[test]
fn the_master_holds_as_many_inserts_as_a_strip() {
    let (mut bus, _) = bus_with_strip();
    for _ in 0..MAX_INSERTS {
        insert(&mut bus, EffectKind::Eq);
    }
    let slot = bus.new_effect(EffectKind::Eq);
    let before = bus.project().clone();
    let r = bus.execute(
        Command::InsertEffect {
            strip: StripId::MASTER,
            index: 0,
            slot: Box::new(slot),
        },
        Origin::User,
    );
    assert!(matches!(r, Err(CommandError::TooManyInserts(_))), "{r:?}");
    assert_eq!(*bus.project(), before);
}

#[test]
fn the_snapshot_carries_the_master_chain_after_the_strips() {
    let mut p = Project::new("s");
    let eq = p.make_effect(EffectKind::Eq);
    let s = p.strips[0].id;
    p.strip_mut(s).unwrap().effects.push(eq.clone());
    let lim = p.make_effect(EffectKind::Limiter);
    p.master.effects.push(lim.clone());
    p.master.fx_enabled = false;
    let snap = Snapshot::compile(&p, 1);
    assert_eq!(snap.master_effects, vec![lim.clone()]);
    assert!(!snap.master_fx_enabled);
    let ids: Vec<EffectId> = snap.effect_slots().map(|e| e.id).collect();
    assert_eq!(ids, vec![eq.id, lim.id]);
}

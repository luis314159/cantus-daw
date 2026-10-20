//! Where a hosted plug-in runs (round 29, ADR 0063): the per-plug-in choice stored in the
//! project, its commands and their exact inverses.

use cantus_core::*;

fn project_with_hosted() -> (CommandBus, StripId, EffectId, TrackId) {
    let mut p = Project::new("iso");
    let mut t = p.make_track("Synth");
    let mut plugin = ExternalPlugin::new(PluginFormat::Vst3, "ABCD", "Synth");
    plugin.params.insert(0, 0.5);
    t.instrument = InstrumentSpec::External(plugin);
    let track = t.id;
    p.tracks.push(t);
    let strip = p.strips[0].id;
    let mut slot = p.make_effect(EffectKind::External);
    slot.spec = EffectSpec::external(ExternalPlugin::new(PluginFormat::Clap, "org.x", "X"));
    let effect = slot.id;
    p.strips[0].effects.push(slot);
    (CommandBus::new(p), strip, effect, track)
}

fn insert_isolation(bus: &CommandBus, effect: EffectId) -> Option<PluginIsolation> {
    bus.project()
        .all_effects()
        .find(|(_, e)| e.id == effect)
        .and_then(|(_, e)| e.spec.external.as_deref())
        .and_then(|p| p.isolation)
}

#[test]
fn an_inserts_isolation_is_set_and_undone_exactly() {
    let (mut bus, strip, effect, _) = project_with_hosted();
    assert_eq!(insert_isolation(&bus, effect), None, "follows the default");
    let before = bus.project().clone();
    bus.execute(
        Command::SetPluginIsolation {
            strip,
            effect,
            isolation: Some(PluginIsolation::Individually),
        },
        Origin::User,
    )
    .unwrap();
    assert_eq!(
        insert_isolation(&bus, effect),
        Some(PluginIsolation::Individually)
    );
    bus.execute(
        Command::SetPluginIsolation {
            strip,
            effect,
            isolation: Some(PluginIsolation::Within),
        },
        Origin::User,
    )
    .unwrap();
    assert_eq!(
        insert_isolation(&bus, effect),
        Some(PluginIsolation::Within)
    );
    // Two steps: each choice is its own undo step.
    bus.undo().unwrap();
    assert_eq!(
        insert_isolation(&bus, effect),
        Some(PluginIsolation::Individually)
    );
    bus.undo().unwrap();
    assert_eq!(bus.project(), &before);
}

#[test]
fn a_tracks_instrument_isolation_is_set_and_undone_exactly() {
    let (mut bus, _, _, track) = project_with_hosted();
    let before = bus.project().clone();
    bus.execute(
        Command::SetInstrumentIsolation {
            track,
            isolation: Some(PluginIsolation::Together),
        },
        Origin::User,
    )
    .unwrap();
    let now = bus
        .project()
        .track(track)
        .and_then(|t| t.instrument.external())
        .and_then(|p| p.isolation);
    assert_eq!(now, Some(PluginIsolation::Together));
    bus.undo().unwrap();
    assert_eq!(bus.project(), &before);
}

#[test]
fn isolation_is_refused_where_there_is_no_hosted_plugin() {
    let (mut bus, strip, _, _) = project_with_hosted();
    let mut p = bus.project().clone();
    let slot = p.make_effect(EffectKind::Eq);
    let eq = slot.id;
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
    assert!(bus
        .execute(
            Command::SetPluginIsolation {
                strip,
                effect: eq,
                isolation: Some(PluginIsolation::Within),
            },
            Origin::User,
        )
        .is_err());
    assert_eq!(bus.project(), &before, "a refusal changes nothing");
    let mut q = Project::new("no instrument");
    let t = q.make_track("Keys");
    let id = t.id;
    q.tracks.push(t);
    let mut bus2 = CommandBus::new(q);
    assert!(bus2
        .execute(
            Command::SetInstrumentIsolation {
                track: id,
                isolation: Some(PluginIsolation::Within),
            },
            Origin::User,
        )
        .is_err());
}

#[test]
fn a_plugin_that_follows_the_default_saves_the_same_bytes_as_before() {
    let plugin = ExternalPlugin::new(PluginFormat::Clap, "org.x", "X");
    let json = serde_json::to_string(&plugin).unwrap();
    assert!(!json.contains("isolation"), "{json}");
    let mut chosen = plugin.clone();
    chosen.isolation = Some(PluginIsolation::ByVendor);
    let json = serde_json::to_string(&chosen).unwrap();
    let back: ExternalPlugin = serde_json::from_str(&json).unwrap();
    assert_eq!(back.isolation, Some(PluginIsolation::ByVendor));
}

#[test]
fn every_isolation_has_its_command_name() {
    let (_, strip, effect, track) = project_with_hosted();
    for c in [
        Command::SetPluginIsolation {
            strip,
            effect,
            isolation: None,
        },
        Command::SetInstrumentIsolation {
            track,
            isolation: None,
        },
    ] {
        assert_eq!(c.label_key(), "cmd.set_plugin_isolation");
    }
}

//! r41:ids — the plug-in host when another project takes the open one's place (ADR 0149):
//! ids start again from 1, so an instance loaded for the old project's id never serves the
//! new project's slot of that id when the plug-ins differ, and a state parked for the id
//! goes back only into the plug-in it came from. With the real helper and test plug-ins.

mod support;

use cantus_core::*;
use support::*;

/// Which plug-in serves `effect` now: its format's id.
fn serving(rig: &Rig, effect: EffectId) -> Option<String> {
    rig.host.hosted(effect).map(|h| h.info.id.clone())
}

/// Project A with the CLAP gain on its first strip; project B the same song with the VST3
/// gain in that place under the same id.
fn pair(isolation: PluginIsolation) -> (Rig, EffectId, Project, StripId) {
    let project = demo_project();
    let strip = strip_of(&project, 0);
    let mut rig = Rig::new(project, host());
    let id = rig.insert(strip, GAIN_ID, isolation);
    rig.settle();
    assert_eq!(serving(&rig, id).as_deref(), Some(GAIN_ID));
    let mut b = rig.bus.project().clone();
    let mut slot = rig
        .host
        .make_slot_isolated(&mut rig.bus, strip, &info(VST3_GAIN_ID), Some(isolation))
        .expect("the VST3 gain loads");
    slot.id = id;
    if let Some(p) = slot.spec.external.as_deref_mut() {
        p.isolation = Some(isolation);
    }
    let effects = &mut b.strip_mut(strip).expect("its strip").effects;
    let at = effects.iter().position(|e| e.id == id).expect("the gain");
    effects[at] = slot;
    (rig, id, b, strip)
}

#[test]
fn another_plug_in_under_the_same_id_is_loaded_for_it_and_back() {
    let _g = serial();
    for isolation in [PluginIsolation::Within, PluginIsolation::Individually] {
        let (mut rig, id, b, _) = pair(isolation);
        rig.exec(Command::ReplaceProject {
            project: Box::new(b),
        });
        rig.settle();
        assert_eq!(
            serving(&rig, id).as_deref(),
            Some(VST3_GAIN_ID),
            "{isolation:?}: the new project's plug-in serves its slot, not the old one's"
        );
        rig.bus.undo().expect("undone");
        rig.resync();
        rig.settle();
        assert_eq!(
            serving(&rig, id).as_deref(),
            Some(GAIN_ID),
            "{isolation:?}: undone, the old project's plug-in again"
        );
    }
}

#[test]
fn forgetting_the_project_reloads_every_plug_in_from_the_project() {
    let _g = serial();
    let (mut rig, id, b, strip) = pair(PluginIsolation::Individually);
    // Opened: the host and the engine forget the old project, as `reset_bus_to` does.
    rig.host.forget_project();
    rig.engine.forget_project();
    rig.bus.reset(b);
    rig.resync();
    rig.settle();
    assert_eq!(serving(&rig, id).as_deref(), Some(VST3_GAIN_ID));
    let held = rig.processor.held_effects();
    assert!(
        held.iter()
            .any(|(s, e, k)| *s == strip && *e == id && *k == EffectKind::External),
        "the engine runs the new plug-in's unit in its place: {held:?}"
    );
    assert!(rig.engine.knows_effect(id));
}

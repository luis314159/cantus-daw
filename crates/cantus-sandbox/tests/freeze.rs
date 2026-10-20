//! A frozen chain with a hosted plug-in (round 29, ADR 0064, with ADR 0063's hosting): the
//! freeze renders through `render_units` — fresh instances, where the plug-in runs live —
//! and plays the live mix to the bit; the plug-in of a frozen chain is unloaded, and comes
//! back with what was set when the chain thaws.

mod support;

use cantus_core::freeze::{self, FreezeTarget};
use cantus_core::*;
use cantus_engine::freeze::render::prepare;
use cantus_engine::RenderProgress;
use std::time::Duration;
use support::*;

fn patient_host() -> cantus_host::PluginHost {
    let host = host();
    if let Some(p) = host.sandboxes() {
        p.set_deadline_floor(Duration::from_secs(5));
    }
    host
}

#[test]
fn a_frozen_chain_unloads_its_hosted_plug_in_and_plays_the_live_mix() {
    let _g = serial();
    for iso in [PluginIsolation::Within, PluginIsolation::Together] {
        let project = demo_project();
        let strip = strip_of(&project, 0);
        let mut rig = Rig::new(project, patient_host());
        let id = rig.insert(strip, GAIN_ID, iso);
        rig.set_param(strip, id, P_GAIN, -6.0);
        rig.set_param(strip, id, P_LATENCY, 37.0);
        rig.settle();
        // Offline reads: the bits are compared, a busy machine must not make a block late.
        rig.processor.set_offline(true);
        let live = rig.play(48_000, 256);
        assert!(peak(&live) > 0.01);
        let dir = tempfile::tempdir().unwrap();
        rig.engine.frozen.set_app_dir(dir.path().to_path_buf());
        let target = FreezeTarget::Strip(strip);
        let p = rig.bus.project().clone();
        let job = prepare(&mut rig.engine, &p, target).expect("prepared");
        let units = rig.host.render_units(&p, SR);
        let insts = rig.host.render_instruments(&p, SR);
        assert_eq!(units.len(), 1, "the plug-in, a fresh instance");
        job.run(units, insts, &RenderProgress::default())
            .expect("rendered");
        rig.host.finish_render();
        rig.engine.frozen.forget_missing();
        rig.exec(freeze::freeze_commands(&p, target).unwrap());
        assert!(
            rig.host.hosted(id).is_none(),
            "{iso:?}: unloaded while frozen"
        );
        let frozen = rig.play(48_000, 256);
        assert_eq!(differing(&live, &frozen), 0, "{iso:?}: frozen = live");
        rig.exec(freeze::unfreeze_command(target));
        assert!(rig.host.hosted(id).is_some(), "{iso:?}: back");
        rig.settle();
        let again = rig.play(48_000, 256);
        assert_eq!(differing(&live, &again), 0, "{iso:?}: thawed = live");
    }
}

/// Review 2: a change made in the window of a plug-in a freeze locks (its render not at
/// hand, so it plays live and is loaded) is refused — the plug-in is given back the
/// project's value, the refusal is said, and a save keeps what the project holds.
#[test]
fn a_window_change_a_freeze_refuses_is_given_back_and_said() {
    let _g = serial();
    let project = demo_project();
    let strip = strip_of(&project, 0);
    let mut rig = Rig::new(project, patient_host());
    let id = rig.insert(strip, GAIN_ID, PluginIsolation::Within);
    rig.set_param(strip, id, P_GAIN, -6.0);
    rig.settle();
    // Frozen, its render missing: it plays live, its plug-in loaded, its chain locked.
    let dir = tempfile::tempdir().unwrap();
    rig.engine.frozen.set_app_dir(dir.path().to_path_buf());
    let target = FreezeTarget::Strip(strip);
    let p = rig.bus.project().clone();
    rig.exec(freeze::freeze_commands(&p, target).unwrap());
    assert!(rig.host.hosted(id).is_some(), "loaded: it plays live");
    rig.notices.clear();
    // The window turns the gain.
    rig.host.record(
        &mut rig.bus,
        id,
        cantus_engine::FxParamEvent::Value(P_GAIN, -18.0),
    );
    rig.frame();
    let stored = |rig: &Rig| {
        rig.bus
            .project()
            .all_effects()
            .find(|(_, e)| e.id == id)
            .and_then(|(_, e)| e.spec.external.as_ref().map(|x| x.params.clone()))
            .unwrap()
    };
    assert_eq!(stored(&rig).get(&P_GAIN), Some(&-6.0), "refused");
    rig.settle();
    let (_, live) = rig.host.capture(id).unwrap();
    assert_eq!(
        live.get(&P_GAIN),
        Some(&-6.0),
        "the plug-in given back the value"
    );
    assert!(
        rig.notices
            .contains(&cantus_host::HostNotice::Frozen(target)),
        "said: {:?}",
        rig.notices
    );
    // A save writes what the project holds of a frozen region's plug-in.
    let saved = rig.host.project_for_save(rig.bus.project());
    assert_eq!(
        saved
            .all_effects()
            .find(|(_, e)| e.id == id)
            .map(|(_, e)| e.spec.clone()),
        rig.bus
            .project()
            .all_effects()
            .find(|(_, e)| e.id == id)
            .map(|(_, e)| e.spec.clone())
    );
}

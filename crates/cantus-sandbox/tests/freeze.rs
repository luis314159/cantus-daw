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

// ---- review 3 ---------------------------------------------------------------------

/// The test gain's state with its gain at `db` (its first value, after the magic).
fn with_gain(state: &[u8], db: f64) -> Vec<u8> {
    let mut s = state.to_vec();
    s[4..12].copy_from_slice(&db.to_le_bytes());
    s
}

/// A rig with the test gain on the first strip at −6 dB, its strip frozen without its
/// render at hand (it plays live, loaded, locked).
fn locked_rig() -> (Rig, EffectId, FreezeTarget, tempfile::TempDir) {
    let project = demo_project();
    let strip = strip_of(&project, 0);
    let mut rig = Rig::new(project, patient_host());
    let id = rig.insert(strip, GAIN_ID, PluginIsolation::Within);
    rig.set_param(strip, id, P_GAIN, -6.0);
    rig.settle();
    // What a freeze writes first: the plug-in's state, as it is, into the project.
    let (state, params) = rig.host.capture(id).unwrap();
    rig.exec(Command::SetPluginState {
        strip,
        effect: id,
        state,
        params,
    });
    let dir = tempfile::tempdir().unwrap();
    rig.engine.frozen.set_app_dir(dir.path().to_path_buf());
    let target = FreezeTarget::Strip(strip);
    let p = rig.bus.project().clone();
    rig.exec(freeze::freeze_commands(&p, target).unwrap());
    assert!(rig.host.hosted(id).is_some(), "loaded: it plays live");
    (rig, id, target, dir)
}

#[test]
fn a_change_the_host_never_heard_of_is_neither_saved_nor_rendered() {
    // The plug-in turned behind the host's back (its own preset, say) to −18 dB: a save
    // keeps what the project holds, and the render is made with it (review 3: the test
    // before gave the value back first, so it could not tell).
    let _g = serial();
    let (mut rig, id, target, _dir) = locked_rig();
    rig.processor.set_offline(true);
    let p = rig.bus.project().clone();
    let reference = rig.play(48_000, 256);
    let (stored, _) = rig.host.capture(id).unwrap();
    rig.host
        .load_state_for_test(id, &with_gain(stored.bytes(), -18.0));
    let (_, live) = rig.host.capture(id).unwrap();
    assert_eq!(live.get(&P_GAIN), Some(&-18.0), "the instance holds −18");
    let saved = rig.host.project_for_save(&p);
    assert_eq!(
        saved
            .all_effects()
            .find(|(_, e)| e.id == id)
            .and_then(|(_, e)| e.spec.external.as_ref().map(|x| x.params.clone()))
            .and_then(|m| m.get(&P_GAIN).copied()),
        Some(-6.0),
        "the save keeps the project's"
    );
    let job = prepare(&mut rig.engine, &p, target).expect("prepared");
    let units = rig.host.render_units_stored(&p, SR);
    job.run(units, Vec::new(), &RenderProgress::default())
        .expect("rendered");
    rig.host.finish_render();
    rig.engine.frozen.forget_missing();
    rig.resync();
    rig.settle();
    let frozen = rig.play(48_000, 256);
    assert_eq!(differing(&reference, &frozen), 0, "rendered at −6 dB");
}

#[test]
fn a_state_of_its_own_in_a_locked_plug_in_is_given_back_and_said() {
    // Its own browser loads something that moves no value (a sample): refused as an edit
    // is, given back, said (review 3: it went unheard).
    let _g = serial();
    let (mut rig, id, target, _dir) = locked_rig();
    let (stored, _) = rig.host.capture(id).unwrap();
    // The same values, a tag of its own.
    let mut other = stored.bytes().to_vec();
    let len = other.len();
    other.truncate(len - 4);
    other.extend_from_slice(&4u32.to_le_bytes());
    other.extend_from_slice(b"kick");
    rig.notices.clear();
    rig.host.state_dirty_for_test(&mut rig.bus, id, &other);
    rig.frame();
    let (now, _) = rig.host.capture(id).unwrap();
    assert_eq!(
        now.bytes(),
        stored.bytes(),
        "given back what the project holds"
    );
    assert!(
        rig.notices
            .contains(&cantus_host::HostNotice::Frozen(target)),
        "said: {:?}",
        rig.notices
    );
}

#[test]
fn a_locked_plug_in_comes_back_as_the_project_holds_it_not_as_it_was_parked() {
    // Turned behind the host's back, then unloaded (its render now at hand: it rests,
    // its state parked), then loaded again while the freeze still holds it (the render
    // gone): it comes back at the project's −6 dB, not the parked −18.
    let _g = serial();
    let (mut rig, id, target, _dir) = locked_rig();
    let p = rig.bus.project().clone();
    let (stored, _) = rig.host.capture(id).unwrap();
    rig.host
        .load_state_for_test(id, &with_gain(stored.bytes(), -18.0));
    let job = prepare(&mut rig.engine, &p, target).expect("prepared");
    let units = rig.host.render_units_stored(&p, SR);
    job.run(units, Vec::new(), &RenderProgress::default())
        .expect("rendered");
    rig.host.finish_render();
    rig.engine.frozen.forget_missing();
    rig.resync();
    assert!(rig.host.hosted(id).is_none(), "unloaded while it rests");
    std::fs::remove_file(&job.path).unwrap();
    rig.engine.frozen.let_go_all();
    rig.engine.frozen.forget_missing();
    rig.resync();
    assert!(rig.host.hosted(id).is_some(), "back: it plays live");
    let (_, live) = rig.host.capture(id).unwrap();
    assert_eq!(live.get(&P_GAIN), Some(&-6.0));
}

#[test]
fn a_drag_a_freeze_refuses_gives_back_once_at_its_end() {
    let _g = serial();
    let (mut rig, id, _, _dir) = locked_rig();
    use cantus_engine::FxParamEvent::{Begin, End, Value};
    let before = rig.host.give_backs();
    rig.host.record(&mut rig.bus, id, Begin(P_GAIN));
    for k in 0..12 {
        rig.host
            .record(&mut rig.bus, id, Value(P_GAIN, -7.0 - k as f64));
        rig.frame();
    }
    assert_eq!(rig.host.give_backs(), before, "nothing reloaded mid-drag");
    rig.host.record(&mut rig.bus, id, End(P_GAIN));
    rig.frame();
    assert_eq!(rig.host.give_backs(), before + 1, "once, at its end");
    let (_, live) = rig.host.capture(id).unwrap();
    assert_eq!(live.get(&P_GAIN), Some(&-6.0));
}

#[test]
fn edits_count_what_windows_do_never_what_automation_does() {
    // A freeze rendering compares these (review 3: it compared the states' bytes, which
    // automation and modulators move, and never landed).
    let _g = serial();
    let project = demo_project();
    let strip = strip_of(&project, 0);
    let mut rig = Rig::new(project, patient_host());
    let id = rig.insert(strip, GAIN_ID, PluginIsolation::Within);
    let mut p = rig.bus.project().clone();
    let ride = p.make_automation_clip(
        "ride",
        cantus_core::automation::AutoTarget::Plugin {
            effect: id,
            id: P_GAIN,
        },
        Ticks::ZERO,
        Ticks::beats(8),
        vec![
            cantus_core::automation::AutoPoint::new(Ticks::ZERO, 0.2),
            cantus_core::automation::AutoPoint::new(Ticks::beats(8), 0.9),
        ],
    );
    p.tracks[0].clips.push(ride);
    rig.bus = CommandBus::new(p);
    rig.resync();
    rig.settle();
    let before = rig.host.edits_of(&[id], &[]);
    let _ = rig.play(24_000, 256);
    assert_eq!(
        rig.host.edits_of(&[id], &[]),
        before,
        "automation is no edit"
    );
    rig.host.record(
        &mut rig.bus,
        id,
        cantus_engine::FxParamEvent::Value(P_GAIN, -3.0),
    );
    assert_eq!(rig.host.edits_of(&[id], &[]), before + 1, "a window's is");
}

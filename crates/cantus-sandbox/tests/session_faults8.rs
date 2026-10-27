//! Round 31 (item 0): what a helper says is judged by the order it said it in, never by
//! when it reaches the interface's frame — through real helpers and the whole host (ADR
//! 0063). A value's echo that reaches the interface after a save asked for the state, but
//! that the helper saw before it saved, is in that state: the state stays current.

mod support;

use cantus_core::*;
use std::time::{Duration, Instant};
use support::*;

/// A host whose real-time blocks wait as long as it takes.
fn patient_host() -> cantus_host::PluginHost {
    let host = host();
    if let Some(p) = host.sandboxes() {
        p.set_deadline_floor(Duration::from_secs(5));
    }
    host
}

/// The gain a saved state holds (the test plug-in's state: a tag, then its gain).
fn gain_in(p: &Project, id: EffectId) -> Option<f64> {
    p.all_effects()
        .find(|(_, s)| s.id == id)
        .and_then(|(_, s)| s.spec.external.as_ref())
        .map(|x| {
            let b = x.state.bytes();
            f64::from_le_bytes(b[4..12].try_into().unwrap())
        })
}

fn frames_until(rig: &mut Rig, until: impl Fn(&mut Rig) -> bool) {
    let start = Instant::now();
    while !until(rig) {
        assert!(start.elapsed() < Duration::from_secs(10), "in time");
        rig.frame();
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// A value set and a save asked for in the same frame: the value's echo (with the answer to
/// the call that set it) reaches the interface after the save was asked for — on a slow
/// machine it did so even after a frame between them (GitHub's macOS runner, the late-state
/// test) — but the helper saw it before it saved. The state holds the value, and stays
/// current: the next save takes it without asking anew and names nothing.
#[test]
fn a_value_set_just_before_a_save_leaves_its_state_current() {
    let _g = serial();
    let project = demo_project();
    let strip = strip_of(&project, 0);
    let mut rig = Rig::new(project, patient_host());
    // Loaded, not yet processing: a value set is flushed to it on its helper's main thread,
    // in turn before the save (once it processes, a value goes with its next block).
    let id = rig.insert(strip, GAIN_ID, PluginIsolation::Individually);
    // Set, then asked for at once: no frame between them.
    rig.set_param(strip, id, P_GAIN, -6.0);
    rig.host.ask_states();
    frames_until(&mut rig, |r| r.host.states_in());
    let saved = rig.host.project_for_save(rig.bus.project());
    assert!(
        rig.host.saved_stale().is_empty(),
        "{:?}",
        rig.host.saved_stale()
    );
    assert_eq!(gain_in(&saved, id), Some(-6.0), "the value set is in it");
    // Frames go on: the echo has been seen, and it was before the save.
    let start = Instant::now();
    while start.elapsed() < Duration::from_millis(300) {
        rig.frame();
        std::thread::sleep(Duration::from_millis(5));
    }
    let again = rig.host.project_for_save(rig.bus.project());
    assert!(
        rig.host.saved_stale().is_empty(),
        "{:?}, last changed by: {:?}",
        rig.host.saved_stale(),
        rig.host.why_changed(id)
    );
    assert_eq!(gain_in(&again, id), Some(-6.0));
}

/// A value the plug-in moves while its state is being saved is not taken as held by that
/// state: the state is not current, and the next save asks anew (the order rule's other
/// side — nothing newer than a state is ever called current).
#[test]
fn a_value_moved_after_a_state_leaves_it_not_current() {
    let _g = serial();
    let project = demo_project();
    let strip = strip_of(&project, 0);
    let mut rig = Rig::new(project, patient_host());
    let id = rig.insert(strip, GAIN_ID, PluginIsolation::Individually);
    rig.settle();
    rig.host.ask_states();
    frames_until(&mut rig, |r| r.host.states_in());
    rig.host.project_for_save(rig.bus.project());
    assert!(rig.host.saved_stale().is_empty());
    // Set after the state was in: the next save without asking names it.
    rig.set_param(strip, id, P_GAIN, -12.0);
    let start = Instant::now();
    while start.elapsed() < Duration::from_millis(300) {
        rig.frame();
        std::thread::sleep(Duration::from_millis(5));
    }
    rig.host.project_for_save(rig.bus.project());
    let stale = rig.host.saved_stale().to_vec();
    assert_eq!(stale.len(), 1, "a value set after the state: {stale:?}");
    assert!(rig.host.why_changed(id).is_some());
}

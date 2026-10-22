//! The final review of round 29 (`final-B`), through real helpers and the whole host (ADR
//! 0063): a render waits for a plug-in that is leaving and never lacks one silently; a
//! state that moves while it plays never keeps a freeze's render from beginning; a plug-in
//! let go while its helper is in a dialog is bounded, and shows its card meanwhile; an
//! instrument picked again plays while the one before it leaves.

mod support;

use cantus_core::*;
use cantus_host::bridge::sandbox::Limits;
use cantus_host::{HostNotice, RenderFor, RenderLoss};
use std::time::{Duration, Instant};
use support::*;

fn patient_host() -> cantus_host::PluginHost {
    let host = host();
    if let Some(p) = host.sandboxes() {
        p.set_deadline_floor(Duration::from_secs(5));
    }
    host
}

/// One frame as the application runs it: the host's frame, and a sync again when a plug-in
/// let go has its state in.
fn frame(rig: &mut Rig) {
    rig.frame();
    if rig.host.take_resync() {
        rig.resync();
    }
}

fn peak(x: &[f32]) -> f32 {
    x.iter().fold(0.0f32, |m, v| m.max(v.abs()))
}

/// The gain's state `state` with its tag (which is not a parameter) set to `tag`.
fn tagged(state: &[u8], tag: &[u8]) -> Vec<u8> {
    // The tag is the state's tail: its length (4 bytes), then its bytes.
    let n = state.len();
    let len = u32::from_le_bytes(state[n - 4..].try_into().unwrap_or([0; 4])) as usize;
    assert_eq!(len, 0, "an untagged state");
    let mut out = state[..n - 4].to_vec();
    out.extend_from_slice(&(tag.len() as u32).to_le_bytes());
    out.extend_from_slice(tag);
    out
}

/// A render asked for while a plug-in is leaving — a device change let it go, its state as
/// it is now on its way — waits for it; one that will not wait says it lacks it; once it is
/// back the render has it. A save waits for it too (review final-B #1, #4).
#[test]
fn a_render_waits_for_a_plugin_that_is_leaving_and_never_lacks_it_silently() {
    let _g = serial();
    let project = demo_project();
    let strip = strip_of(&project, 0);
    let mut rig = Rig::new(project, patient_host());
    let id = rig.insert(strip, GAIN_ID, PluginIsolation::Together);
    // Its next state takes 3 s.
    rig.set_param(strip, id, P_FAULT, SLOW_SAVE);
    rig.settle();
    rig.host.reload_all(SR);
    rig.resync();
    assert!(rig.host.hosted(id).is_none(), "leaving");
    assert!(!rig.host.states_in(), "a save waits for its state too");
    let p = rig.bus.project().clone();
    assert!(
        !rig.host.render_ready(RenderFor::Export, &p, SR, false),
        "the render waits for the plug-in leaving"
    );
    // A render that does not wait lacks it — and says so.
    let units = rig.host.render_units(&p, SR);
    let failures = rig.host.render_failures();
    rig.host.finish_render();
    assert!(units.is_empty());
    assert_eq!(failures.len(), 1, "{failures:?}");
    assert_eq!(failures[0].kind, RenderLoss::NeverLoaded);
    // Waited for: once it is back, the render has it.
    let start = Instant::now();
    loop {
        let p = rig.bus.project().clone();
        if rig.host.render_ready(RenderFor::Export, &p, SR, false) {
            break;
        }
        assert!(start.elapsed() < Duration::from_secs(20), "it came back");
        frame(&mut rig);
        std::thread::sleep(Duration::from_millis(5));
    }
    let p = rig.bus.project().clone();
    let units = rig.host.render_units(&p, SR);
    let failures = rig.host.render_failures();
    rig.host.finish_render();
    assert_eq!(units.len(), 1, "the render has it");
    assert!(failures.is_empty(), "{failures:?}");
}

/// A plug-in in process whose state moves at every look (as one that carries what it last
/// heard does while the song plays) beside an isolated one: the freeze's render still
/// begins — the isolated instance is not made anew for a state that is not its own (review
/// final-B #10).
#[test]
fn a_moving_state_never_keeps_a_freeze_from_its_render() {
    let _g = serial();
    let project = demo_project();
    let strip = strip_of(&project, 0);
    let mut rig = Rig::new(project, patient_host());
    let inside = rig.insert(strip, GAIN_ID, PluginIsolation::Within);
    let _outside = rig.insert(strip, GAIN_ID, PluginIsolation::Together);
    rig.settle();
    let base = rig.bus.project().clone();
    let state = rig.plugin(inside).state.bytes().to_vec();
    assert!(!state.is_empty(), "the plug-in in process said its state");
    let with = |tag: &[u8]| {
        let mut p = base.clone();
        if let Some(e) = p
            .strip_mut(strip)
            .and_then(|s| s.effects.iter_mut().find(|e| e.id == inside))
            .and_then(|e| e.spec.external.as_deref_mut())
        {
            e.state = PluginState::new(tagged(&state, tag));
        }
        p
    };
    let start = Instant::now();
    let mut n = 0u32;
    loop {
        let p = with(if n.is_multiple_of(2) { b"a" } else { b"b" });
        if rig.host.render_ready(RenderFor::Freeze, &p, SR, true) {
            break;
        }
        n += 1;
        assert!(
            start.elapsed() < Duration::from_secs(15),
            "the freeze's render begins though a state moves"
        );
        rig.frame();
        std::thread::sleep(Duration::from_millis(5));
    }
    rig.host.finish_render();
}

/// Two plug-ins of one helper let go (a device change) while it is in a dialog of the
/// first's (its state loading, 6 s): the first shows its card meanwhile, and neither waits
/// past the state limit — each is parked as it was last said, said so, and loaded again,
/// in a helper of its own, while the dialog is still open (review final-B #4, #11).
#[test]
fn plugins_let_go_during_a_dialog_are_bounded_and_show_its_card() {
    let _g = serial();
    let project = demo_project();
    let strip = strip_of(&project, 0);
    let host = patient_host();
    if let Some(pool) = host.sandboxes() {
        pool.set_limits(Limits {
            state: Duration::from_millis(3_000),
            ..pool.limits()
        });
    }
    let mut rig = Rig::new(project, host);
    // Their helper loads every state in a nested loop of 6 s (as a plug-in's dialog does).
    std::env::set_var("CANTUS_TEST_GAIN_PUMP_LOAD_MS", "6000");
    let a = rig.insert(strip, GAIN_ID, PluginIsolation::Together);
    let b = rig.insert(strip, GAIN_ID, PluginIsolation::Together);
    std::env::remove_var("CANTUS_TEST_GAIN_PUMP_LOAD_MS");
    let state = rig.plugin(a).state.bytes().to_vec();
    let params = rig.plugin(a).params.clone();
    rig.exec(Command::SetPluginState {
        strip,
        effect: a,
        state: PluginState::new(tagged(&state, b"dialog")),
        params,
    });
    std::thread::sleep(Duration::from_millis(200));
    rig.host.reload_all(SR);
    rig.resync();
    let start = Instant::now();
    let mut card = false;
    let kept = |r: &Rig| {
        r.notices
            .iter()
            .filter(|n| matches!(n, HostNotice::StateKept { .. }))
            .count()
    };
    while start.elapsed() < Duration::from_millis(4_500) {
        card |= rig.host.waiting().0.iter().any(|(e, _)| *e == a);
        if kept(&rig) == 2 && rig.host.hosted(a).is_some() && rig.host.hosted(b).is_some() {
            break;
        }
        frame(&mut rig);
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(
        card,
        "the plug-in in its dialog showed its card while it left"
    );
    assert_eq!(
        kept(&rig),
        2,
        "each kept as it was last said, and said: {:?}",
        rig.notices
    );
    assert!(
        rig.host.hosted(a).is_some() && rig.host.hosted(b).is_some(),
        "loaded again"
    );
}

/// An instrument picked again (the same plug-in, to start afresh) plays as soon as it is
/// loaded, while the one before it is still leaving — its state on its way (review final-B
/// #4).
#[test]
fn an_instrument_picked_again_plays_while_the_one_before_it_leaves() {
    let _g = serial();
    let mut p = Project::new("again");
    let mut keys = p.make_track("Keys");
    let mut clip = p.make_clip("Chord", Ticks::ZERO, Ticks::beats(16));
    for (i, pitch) in [60u8, 64, 67, 72].iter().cycle().take(16).enumerate() {
        let n = p.make_note(Ticks::QUARTER * i as i64, Ticks::QUARTER, *pitch, 100);
        clip.notes.push(n);
    }
    keys.clips.push(clip);
    let track = keys.id;
    p.tracks.push(keys);
    let host = patient_host();
    if let Some(pool) = host.sandboxes() {
        pool.set_limits(Limits {
            state: Duration::from_secs(6),
            ..pool.limits()
        });
    }
    let mut rig = Rig::new(p, host);
    let pick = |rig: &mut Rig| {
        let spec = rig
            .host
            .make_instrument_isolated(track, &info(SYNTH_ID), Some(PluginIsolation::Individually))
            .expect("the synth loads");
        rig.exec(Command::SetInstrument {
            track,
            spec: Box::new(spec),
        });
    };
    pick(&mut rig);
    rig.settle();
    // The first one quiet, and its next state never comes (its helper hangs in it).
    for (id, value) in [(S_GAIN, -60.0), (S_FAULT, HANG_SAVE)] {
        rig.exec(Command::SetInstrumentPluginParam {
            track,
            id,
            value: Some(value),
        });
    }
    rig.settle();
    pick(&mut rig);
    let start = Instant::now();
    let mut heard = 0.0f32;
    while heard < 0.01 && start.elapsed() < Duration::from_millis(3_000) {
        frame(&mut rig);
        rig.engine.seek(Ticks::ZERO);
        rig.engine.play();
        let mut out = Vec::new();
        for _ in 0..24 {
            out.extend(rig.block(256));
        }
        rig.engine.stop();
        heard = peak(&out);
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(
        heard > 0.01,
        "the new one plays while the old one leaves: {heard}"
    );
}

/// The final review's parking claim (final-B #15) is held in `session_faults6.rs`'s
/// `a_plugin_let_go_parks_its_state_as_it_is_now_or_says_so`: a state moved quietly
/// (`TAG_QUIETLY`) comes back with the plug-in.
#[test]
fn a_quiet_tag_is_only_the_plugins_own_doing() {
    let _g = serial();
    let project = demo_project();
    let strip = strip_of(&project, 0);
    let mut rig = Rig::new(project, patient_host());
    let id = rig.insert(strip, GAIN_ID, PluginIsolation::Within);
    rig.set_param(strip, id, P_FAULT, TAG_QUIETLY);
    rig.settle();
    rig.host.ask_states();
    assert!(rig.host.states_in());
    let saved = rig.host.project_for_save(rig.bus.project());
    let state = saved
        .all_effects()
        .find(|(_, e)| e.id == id)
        .and_then(|(_, e)| e.spec.external.as_deref())
        .map(|p| p.state.bytes().to_vec())
        .unwrap_or_default();
    assert!(state.ends_with(b"heard"), "its own state carries it");
    assert!(
        !rig.plugin(id).state.bytes().ends_with(b"heard"),
        "the host was told nothing"
    );
}

//! ADR 0063 §10, the way a user meets it: two plug-ins in one helper, the first aborts in its
//! `process` while the helper's main thread is held inside the second's code
//! (`CANTUS_TEST_SANDBOX_LAG_IN`, a loaded machine's main thread put off the processor
//! there). Nothing in the process says which thread aborted: the card says Cantus could not
//! tell which one crashed, and offers no process of its own to either; *Reload all* runs each
//! alone for the session (the project unchanged, *Together* still where they run, the tip
//! saying alone); the first's
//! next abort is its own — one card, for it alone, counted — and the second plays on. A
//! process of its own: the helper inherits the knob from the environment.
//!
//! Windows only: there an abort is a fail-fast and no handler runs. On Linux and macOS the
//! helper's signal handler names the aborting thread's plug-in, so no end has two candidates.
#![cfg(windows)]

mod hosted_support;
mod support;

use cantus_core::*;
use cantus_engine::{EngineHandle, Processor};
use cantus_host::{Format, HostNotice};
use cantus_ui::views::plugin_notice::{self, Answer, Target};
use cantus_ui::CantusApp;
use hosted_support::*;

/// The test gain's fault parameter, and its abort in `process`.
const P_FAULT: u32 = 4;
const ABORT_PROCESS: f64 = 1.0;

/// One audio block, then the host's per-frame work as the app does it: its plug-ins' news,
/// a crash told to the card.
fn frame(a: &mut CantusApp, processor: &mut Processor) {
    let mut buf = vec![0f32; 512];
    processor.process(&mut buf, 2);
    a.engine.poll();
    for n in a.plugins.idle(&mut a.bus, &mut a.engine) {
        if matches!(n, HostNotice::PluginCrashed { .. }) {
            plugin_notice::on_notice(a, n);
        }
    }
    a.sync_engine();
    a.pump_hosted_opens();
}

fn frames_until(
    a: &mut CantusApp,
    processor: &mut Processor,
    secs: u64,
    done: impl Fn(&CantusApp) -> bool,
) -> bool {
    let until = std::time::Instant::now() + std::time::Duration::from_secs(secs);
    while std::time::Instant::now() < until {
        frame(a, processor);
        if done(a) {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    false
}

fn abort(a: &mut CantusApp, processor: &mut Processor, strip: StripId, fx: EffectId) {
    a.set_plugin_param(strip, fx, P_FAULT, 0.0, false);
    for _ in 0..4 {
        frame(a, processor);
    }
    a.set_plugin_param(strip, fx, P_FAULT, ABORT_PROCESS, false);
}

#[test]
fn an_end_nothing_chooses_between_is_said_and_settled_by_the_next() {
    let _one = serial();
    let dir = tempfile::tempdir().unwrap();
    // The helper's main thread 30 ms in each call into its second plug-in (instance 2).
    std::env::set_var("CANTUS_TEST_SANDBOX_LAG_IN", "2:30");
    cantus_ui::i18n::default_thread_locale(cantus_ui::i18n::Locale::En);
    let (engine, mut processor) = EngineHandle::manual(48_000.0);
    let mut a = CantusApp::from_parts_with(
        Some(cantus_ui::starter_project()),
        engine,
        false,
        Some(PluginIsolation::Together),
    );
    a.plugins
        .set_sandboxes(cantus_host::bridge::Sandboxes::new(sandbox_exe()));
    let info = know(&mut a, &gain_library(), dir.path(), "Cantus Test Gain.clap");
    let strip = first_strip(&a);
    let first = insert(&mut a, strip, &info);
    let second = insert(&mut a, strip, &info);
    let (t1, t2) = (Target::Insert(first), Target::Insert(second));
    assert!(frames_until(&mut a, &mut processor, 30, |a| a
        .hosted_working(t1)
        && a.hosted_working(t2)));
    a.engine.seek(Ticks::ZERO);
    a.engine.play();
    for _ in 0..64 {
        frame(&mut a, &mut processor);
    }

    // The first aborts while the main thread is in the second's code: both stop, and the
    // card says nothing chose between them. Either can be its doing: no process of its
    // own is offered.
    let mut unsettled = false;
    for _ in 0..5 {
        abort(&mut a, &mut processor, strip, first);
        assert!(
            frames_until(&mut a, &mut processor, 15, |a| a
                .plugin_crashes
                .waiting
                .len()
                == 2),
            "both stopped: {}",
            a.status.text
        );
        if a.plugins.crash_suspect(first) {
            unsettled = true;
            break;
        }
        // The main thread was between calls: laid on the first at once. Again.
        assert!(a.plugins.crash_culprit(first));
        plugin_notice::answer(&mut a, Answer::Reload);
        assert!(frames_until(&mut a, &mut processor, 30, |a| a
            .hosted_working(t1)
            && a.hosted_working(t2)));
    }
    assert!(
        unsettled,
        "the knob held the main thread in the second's code"
    );
    assert!(a.plugins.crash_suspect(second));
    assert!(!a.plugins.crash_culprit(first) && !a.plugins.crash_culprit(second));
    let counted = a.plugins.crashes_this_session(Format::Clap, &info.id);
    let card = plugin_notice::card(&a).expect("a card");
    assert_eq!(card.sentence(), "2 plug-ins stopped working");
    assert!(
        card.body.contains("could not tell which one crashed"),
        "{}",
        card.body
    );
    assert!(card.body.contains("each runs alone"), "{}", card.body);
    assert!(card.body.contains("kept playing"), "{}", card.body);
    let labels: Vec<&str> = card.buttons.iter().map(|(l, _)| l.as_str()).collect();
    assert_eq!(labels, ["Reload all", "Keep them off"]);

    // Reloaded from the card: each alone, where the project still says *Together*.
    plugin_notice::answer(&mut a, Answer::Reload);
    assert!(frames_until(&mut a, &mut processor, 30, |a| a
        .hosted_working(t1)
        && a.hosted_working(t2)));
    for fx in [first, second] {
        let h = a.plugins.hosted(fx).expect("loaded");
        assert!(h.apart(), "runs alone");
        assert_eq!(h.isolation(), PluginIsolation::Together);
        let tip = plugin_notice::tip(&a, Target::Insert(fx));
        assert!(tip.contains("Alone this session"), "{tip}");
    }
    let project_says = a
        .project()
        .all_effects()
        .find(|(_, e)| e.id == second)
        .and_then(|(_, e)| e.spec.external.as_deref())
        .map(|p| p.isolation);
    // As this computer says (*Together* here), still: nothing in the project moved.
    assert_eq!(project_says, Some(None));
    for _ in 0..64 {
        frame(&mut a, &mut processor);
    }

    // Its next abort is its own: one card, for it, counted; the second plays on.
    abort(&mut a, &mut processor, strip, first);
    assert!(frames_until(&mut a, &mut processor, 15, |a| !a
        .plugin_crashes
        .waiting
        .is_empty()));
    for _ in 0..40 {
        frame(&mut a, &mut processor);
    }
    let waiting: Vec<Target> = a.plugin_crashes.waiting.iter().map(|c| c.target).collect();
    assert_eq!(waiting, [t1], "the first alone stopped");
    assert!(a.plugins.crash_culprit(first));
    assert!(!a.plugins.crash_suspect(first));
    assert!(a.hosted_working(t2), "the second plays on");
    assert_eq!(
        a.plugins.crashes_this_session(Format::Clap, &info.id),
        counted + 1
    );
    let card = plugin_notice::card(&a).expect("a card");
    assert!(
        !card.body.contains("could not tell"),
        "laid on it: {}",
        card.body
    );
    std::env::remove_var("CANTUS_TEST_SANDBOX_LAG_IN");
}

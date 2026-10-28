//! Review of ADR 0073, #3: switching to Cantus's view while an isolated plug-in's own window
//! is still on its way (its helper busy: the window "opens when it is free") cancels that
//! window — it never appears later beside Cantus's view. A process of its own: the test
//! gain's slow `load_state` knob is read from the environment the helper inherits.

mod hosted_support;
mod support;

use cantus_core::*;
use cantus_engine::EngineHandle;
use cantus_ui::hosted_view::HostedView;
use cantus_ui::views::plugin_notice::Target;
use cantus_ui::CantusApp;
use hosted_support::*;

#[test]
fn a_window_still_on_its_way_is_cancelled_by_the_switch() {
    let _one = serial();
    let dir = tempfile::tempdir().unwrap();
    // Every `load_state` of the gain runs a nested message loop for 3 s: its helper is busy.
    std::env::set_var("CANTUS_TEST_GAIN_PUMP_LOAD_MS", "3000");
    cantus_ui::i18n::default_thread_locale(cantus_ui::i18n::Locale::En);
    let mut a = CantusApp::from_parts_with(
        Some(cantus_ui::starter_project()),
        EngineHandle::dummy(),
        false,
        Some(PluginIsolation::Together),
    );
    a.plugins
        .set_sandboxes(cantus_host::bridge::Sandboxes::new(sandbox_exe()));
    let info = know(&mut a, &gain_library(), dir.path(), "Cantus Test Gain.clap");
    let strip = first_strip(&a);
    let fx = insert(&mut a, strip, &info);
    let target = Target::Insert(fx);
    assert!(pump_until(&mut a, 30.0, |a| a.hosted_working(target)
        && a.fx.hosted.views.pending().is_empty()));
    // Cantus's view; then a state to load (its helper busy for 3 s), its own window asked
    // for meanwhile — and Cantus's view chosen again before the helper is free.
    let _ = pump_until(&mut a, 6.0, |a| a.native_open(target));
    a.set_hosted_view(target, HostedView::Cantus);
    assert!(!a.native_open(target));
    let (state, params) = a.plugins.capture(fx).expect("loaded");
    a.set_plugin_param(strip, fx, 0, -12.0, false);
    a.sync_engine();
    a.set_plugin_state(strip, fx, state, params);
    a.sync_engine();
    a.set_hosted_view(target, HostedView::Native);
    let asked_while_busy = !a.native_open(target);
    a.set_hosted_view(target, HostedView::Cantus);
    assert!(a.cantus_view_open(target));
    // The helper free again and long after: its window was closed as it came.
    let appeared = pump_until(&mut a, 8.0, |a| a.native_open(target));
    assert!(
        !appeared,
        "its own window appeared beside Cantus's view (asked while busy: {asked_while_busy})"
    );
    assert!(a.cantus_view_open(target));
    assert!(a.hosted_working(target), "{}", a.status.text);
}

/// Second review of ADR 0073, #2: a second press on the plug-in's window key while its
/// window is still on its way (its helper busy) is harmless — the window still comes, as
/// before; only switching to Cantus's view cancels it.
#[test]
fn a_second_press_while_its_window_is_on_its_way_does_not_cancel_it() {
    let _one = serial();
    let dir = tempfile::tempdir().unwrap();
    std::env::set_var("CANTUS_TEST_GAIN_PUMP_LOAD_MS", "3000");
    cantus_ui::i18n::default_thread_locale(cantus_ui::i18n::Locale::En);
    let mut a = CantusApp::from_parts_with(
        Some(cantus_ui::starter_project()),
        EngineHandle::dummy(),
        false,
        Some(PluginIsolation::Together),
    );
    a.plugins
        .set_sandboxes(cantus_host::bridge::Sandboxes::new(sandbox_exe()));
    let info = know(&mut a, &gain_library(), dir.path(), "Cantus Test Gain.clap");
    let strip = first_strip(&a);
    let fx = insert(&mut a, strip, &info);
    let target = Target::Insert(fx);
    assert!(pump_until(&mut a, 30.0, |a| a.hosted_working(target)
        && a.fx.hosted.views.pending().is_empty()));
    let _ = pump_until(&mut a, 6.0, |a| a.native_open(target));
    a.plugins.close_window(fx);
    // Its helper busy loading a state; the window asked for, then asked for again.
    let (state, params) = a.plugins.capture(fx).expect("loaded");
    a.set_plugin_param(strip, fx, 0, -12.0, false);
    a.sync_engine();
    a.set_plugin_state(strip, fx, state, params);
    a.sync_engine();
    a.toggle_plugin_window(fx);
    let busy = !a.native_open(target);
    a.toggle_plugin_window(fx);
    let came = pump_until(&mut a, 10.0, |a| a.native_open(target));
    if cfg!(windows) {
        assert!(
            came,
            "the window on its way came (asked while busy: {busy})"
        );
    }
    a.plugins.close_window(fx);
}

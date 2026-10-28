//! A hosted plug-in's views **out of process** (ADR 0073 on ADR 0063): the test gain and
//! the test synth run in this build's `cantus-sandbox`; an insert or a pick still loading
//! in its helper opens in its view once it is in, and switching views — its own window,
//! the helper's, and Cantus's — keeps the project, the plug-in's state and its automation.

mod hosted_support;
mod support;

use cantus_core::*;
use cantus_engine::EngineHandle;
use cantus_ui::hosted_view::HostedView;
use cantus_ui::views::plugin_notice::Target;
use cantus_ui::CantusApp;
use hosted_support::*;

/// An app whose plug-ins run together in a helper of this build.
fn isolated_app() -> CantusApp {
    cantus_ui::i18n::default_thread_locale(cantus_ui::i18n::Locale::En);
    let mut app = CantusApp::from_parts_with(
        Some(cantus_ui::starter_project()),
        EngineHandle::dummy(),
        false,
        Some(PluginIsolation::Together),
    );
    app.plugins
        .set_sandboxes(cantus_host::bridge::Sandboxes::new(sandbox_exe()));
    assert!(app.plugins.isolation_available(), "the helper is built");
    app
}

/// Its own window opened — in its helper, which may open it a little later (it was busy):
/// on Windows, always. Elsewhere, with no display, the open fell back to Cantus's view, or
/// the helper's refusal is on the status line.
fn native_or_fallback(a: &mut CantusApp, target: Target) {
    let opened = pump_until(a, 10.0, |a| a.native_open(target));
    if opened {
        assert!(!a.cantus_view_open(target), "one view at a time");
    } else {
        if cfg!(windows) {
            panic!("Windows opens the helper's window: {}", a.status.text);
        }
        assert!(
            a.cantus_view_open(target) || a.status.error,
            "{}",
            a.status.text
        );
    }
}

/// The insert's state and values as its helper says them now.
fn fresh_state(
    a: &mut CantusApp,
    fx: EffectId,
) -> (PluginState, std::collections::BTreeMap<u32, f64>) {
    a.plugins.ask_states_of(&[fx], &[]);
    let until = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !a.plugins.states_in_of(&[fx], &[]) {
        let _ = a.plugins.idle(&mut a.bus, &mut a.engine);
        assert!(std::time::Instant::now() < until, "its helper answers");
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    a.plugins.capture(fx).expect("loaded")
}

#[test]
fn an_isolated_insert_opens_once_loaded_and_switching_keeps_its_state() {
    let _one = serial();
    let dir = tempfile::tempdir().unwrap();
    // Its first activation in the helper takes 2.5 s (the test gain's slow-start knob, read
    // by the helper this app starts): the insert is always seen loading.
    std::env::set_var(
        "CANTUS_TEST_GAIN_SLOW_ACTIVATE_ONCE",
        dir.path().join("slow-once"),
    );
    std::env::set_var("CANTUS_TEST_GAIN_SLOW_MS", "2500");
    let mut a = isolated_app();
    let info = know(&mut a, &gain_library(), dir.path(), "Cantus Test Gain.clap");
    let strip = first_strip(&a);
    let fx = insert(&mut a, strip, &info);
    let target = Target::Insert(fx);
    assert_eq!(a.plugins.isolation(fx), Some(PluginIsolation::Together));
    assert!(a.hosted_loading(target), "loading in its helper");
    assert!(
        a.fx.hosted.views.pending().contains(&target),
        "its own window waits for its helper"
    );
    assert!(a.cantus_view_open(target), "its loading card meanwhile");
    assert!(!a.native_open(target));
    assert!(
        pump_until(&mut a, 30.0, |a| !a.hosted_loading(target)
            && a.fx.hosted.views.pending().is_empty()),
        "loaded in its helper"
    );
    assert!(a.hosted_working(target));
    native_or_fallback(&mut a, target);
    if a.native_open(target) {
        assert!(
            !a.cantus_view_open(target),
            "the window took the card's place"
        );
    }
    // A value moved and automated; then the switch, both ways, twice.
    a.set_plugin_param(strip, fx, 0, -3.0, false);
    a.automate(AutoTarget::Plugin { effect: fx, id: 0 });
    // The value reached its helper before the state is taken.
    assert!(pump_until(&mut a, 10.0, |a| a
        .plugins
        .hosted(fx)
        .and_then(|h| h.param_value(0))
        == Some(-3.0)));
    let state = fresh_state(&mut a, fx);
    let project = a.project().clone();
    let revision = a.bus.revision();
    for view in [
        HostedView::Cantus,
        HostedView::Native,
        HostedView::Cantus,
        HostedView::Native,
    ] {
        a.set_hosted_view(target, view);
        let _ = pump_until(&mut a, 0.2, |_| false);
        let _ = support::paint_editors(&mut a, egui::vec2(1400.0, 900.0));
        assert_eq!(a.bus.revision(), revision, "no command: {view:?}");
        assert_eq!(*a.project(), project);
        assert!(a.hosted_working(target), "still loaded where it ran");
        assert_eq!(
            fresh_state(&mut a, fx),
            state,
            "its state, as its helper says it"
        );
    }
    a.plugins.close_window(fx);
}

#[test]
fn an_isolated_instrument_with_cantus_as_its_default_opens_there() {
    use cantus_ui::picker::{PickChoice, PickPurpose};
    let _one = serial();
    let dir = tempfile::tempdir().unwrap();
    let mut a = isolated_app();
    a.settings_path = dir.path().join("settings.json");
    let info = know(
        &mut a,
        &synth_library(),
        dir.path(),
        "Cantus Test Synth.clap",
    );
    a.settings
        .hosted_views
        .set(PluginFormat::Clap, &info.id, Some(HostedView::Cantus));
    let track = a.project().tracks[0].id;
    a.apply_pick(
        PickPurpose::ReplaceInstrument { track },
        PickChoice::Hosted(Box::new(info)),
    );
    let target = Target::Instrument(track);
    assert!(
        a.cantus_view_open(target),
        "by its default, at once — loading or not"
    );
    assert!(pump_until(&mut a, 30.0, |a| a.hosted_working(target)));
    assert!(a.hosted_view_switchable(target));
    let state = a.plugins.capture_instrument(track).expect("loaded");
    let revision = a.bus.revision();
    a.set_hosted_view(target, HostedView::Native);
    native_or_fallback(&mut a, target);
    a.set_hosted_view(target, HostedView::Cantus);
    assert!(a.cantus_view_open(target) && !a.native_open(target));
    let ctx = support::ctx();
    let text = support::paint(&ctx, egui::vec2(1400.0, 900.0), 3, |ui| {
        let c = ui.ctx().clone();
        cantus_ui::views::plugin_view_inst::window(&mut a, &c);
    });
    assert!(
        text.contains("| TEMPO SEEN |"),
        "its meters, from the helper: {text}"
    );
    assert_eq!(a.bus.revision(), revision);
    assert_eq!(a.plugins.capture_instrument(track), Some(state));
    a.plugins.close_instrument_window(track);
}

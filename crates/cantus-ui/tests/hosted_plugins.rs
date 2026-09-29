//! Hosted plug-ins in the application (ADR 0019): a project whose plug-in is missing opens,
//! says so, passes sound and saves its record back untouched; the generic editor shows a
//! real plug-in's parameters in the plug-in's own words.

mod support;
use cantus_core::*;
use support::*;

fn missing_plugin_project() -> (Project, StripId, EffectId, ExternalPlugin) {
    let mut p = cantus_ui::starter_project();
    let strip = p.tracks[0].strip.unwrap();
    let mut plugin = ExternalPlugin::new(PluginFormat::Clap, "org.example.nowhere", "Far Away EQ");
    plugin.vendor = "Elsewhere Audio".into();
    plugin.state = PluginState::new(vec![9, 8, 7, 6]);
    plugin.params.insert(3, -1.5);
    let mut slot = p.make_effect(EffectKind::External);
    slot.spec = EffectSpec::external(plugin.clone());
    let id = slot.id;
    p.strip_mut(strip).unwrap().effects.push(slot);
    (p, strip, id, plugin)
}

#[test]
fn a_missing_plugin_opens_says_so_and_saves_back_untouched() {
    let (p, strip, id, plugin) = missing_plugin_project();
    let mut a = app(p);
    assert!(a.plugins.missing(id).is_some(), "not on this computer");
    a.selected_strip = Some(strip);
    let ctx = ctx();
    // Tall enough that the inserts, at the bottom of the inspector, are on screen.
    let text = paint(&ctx, egui::vec2(420.0, 6000.0), 3, |ui| {
        cantus_ui::views::inspector::show(&mut a, ui)
    });
    assert!(text.contains("Far Away EQ"), "{text}");
    assert!(text.contains("CLAP"), "the format badge: {text}");
    assert!(text.contains(&cantus_ui::i18n::t("host.missing")), "{text}");

    a.open_effect_editor(strip, id);
    let text = paint_editors(&mut a, egui::vec2(1200.0, 800.0));
    assert!(text.contains("is not on this computer"), "{text}");

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("missing.cantus");
    a.save_to_path(&path);
    let back = cantus_project::load(&path, true).unwrap().project;
    let kept = back
        .strip(strip)
        .and_then(|s| s.effect(id))
        .and_then(|e| e.spec.external.as_deref())
        .unwrap();
    assert_eq!(*kept, plugin);
}

/// The test CLAP plug-in, built from source where the host's tests build it.
fn test_plugin() -> std::path::PathBuf {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let target = root.join("target").join("test-plugins");
    let status =
        std::process::Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
            .args(["build", "-p", "cantus-test-clap", "--target-dir"])
            .arg(&target)
            .current_dir(&root)
            .status()
            .expect("cargo");
    assert!(status.success());
    target.join("debug").join(if cfg!(windows) {
        "cantus_test_clap.dll"
    } else if cfg!(target_os = "macos") {
        "libcantus_test_clap.dylib"
    } else {
        "libcantus_test_clap.so"
    })
}

#[test]
fn the_generic_editor_shows_the_plugins_parameters_in_its_own_words() {
    let mut a = app(cantus_ui::starter_project());
    cantus_ui::views::plugins::demo_setup(&mut a, &test_plugin(), true);
    let (strip, effect) = match a.fx.open {
        Some((s, e, k)) => {
            assert_eq!(k, EffectKind::External);
            (s, e)
        }
        None => panic!("the editor did not open"),
    };
    assert!(a.plugins.hosted(effect).is_some(), "loaded");
    let text = paint_editors(&mut a, egui::vec2(1400.0, 900.0));
    // The frame's header sets the name in capitals, and the cells their labels.
    let lower = text.to_lowercase();
    assert!(lower.contains("cantus test gain"), "{text}");
    assert!(text.contains("GAIN"), "{text}");
    assert!(lower.contains("latency"), "{text}");
    // The pill sets the unit a hair apart from the number, not after a space.
    assert!(
        text.replace(' ', "").contains("-4.5dB"),
        "the plug-in's own text for the value: {text}"
    );
    assert!(
        !lower.contains("gesture"),
        "a hidden parameter is not shown: {text}"
    );
    assert!(
        text.contains(&cantus_ui::i18n::t("host.editor.open_window")),
        "{text}"
    );
    // A change from the editor is a command: one undo step.
    let before = a.bus.history().len();
    a.set_plugin_param(strip, effect, 0, -9.0, true);
    a.set_plugin_param(strip, effect, 0, -10.0, true);
    a.end_gesture();
    assert_eq!(a.bus.history().len(), before + 1);
    a.undo();
    let v = a
        .project()
        .strip(strip)
        .and_then(|s| s.effect(effect))
        .and_then(|e| e.spec.external.as_deref())
        .map(|p| p.params[&0]);
    assert_eq!(v, Some(-4.5));
}

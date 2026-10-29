//! *Units wear their own faces* on the boot path (round 32, ADR 0074): the desktop app goes
//! `run` → `boot::Boot` → `CantusApp::assemble`, never `CantusApp::new`, so the choice kept
//! in the settings is applied there — and again whenever it changes. Its own test binary:
//! it points `CANTUS_DATA_DIR` at a temporary folder.

use cantus_core::EffectKind;
use cantus_engine::EngineHandle;
use cantus_ui::app::Files;
use cantus_ui::kit::skin;
use cantus_ui::CantusApp;

fn assemble() -> CantusApp {
    let config = cantus_ui::app::config_dir();
    let files = Files::read(&config, Some(cantus_ui::i18n::Locale::En));
    CantusApp::assemble(
        &egui::Context::default(),
        files,
        None,
        EngineHandle::dummy(),
        None,
        None,
        None,
        false,
    )
}

#[test]
fn the_boot_path_applies_the_units_faces_switch_from_the_settings() {
    let data = tempfile::tempdir().unwrap();
    std::env::set_var("CANTUS_DATA_DIR", data.path());
    let config = cantus_ui::app::config_dir();
    std::fs::create_dir_all(&config).unwrap();

    // A new installation: on.
    let mut app = assemble();
    assert!(app.settings.unit_faces, "on by default");
    assert!(skin::enabled());
    assert_eq!(skin::of_effect(EffectKind::Eq).key, "fx.eq");

    // Turned off and kept: the next start wears the plain look.
    app.settings.unit_faces = false;
    app.apply_unit_faces();
    assert!(!skin::enabled(), "applied when it changes");
    app.store_settings();
    skin::set_enabled(true);
    let app = assemble();
    assert!(!app.settings.unit_faces, "kept in the settings");
    assert!(!skin::enabled(), "applied by assemble");
    assert_eq!(skin::of_effect(EffectKind::Eq).key, "plain");
    skin::set_enabled(true);
}

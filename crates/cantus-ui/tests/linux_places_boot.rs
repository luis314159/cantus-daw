//! Linux (ADR 0065, review of round 30): the app the boot path assembles keeps every file
//! in the XDG places — its settings in `$XDG_CONFIG_HOME/cantus`; autosaves, recordings of
//! unsaved projects and caches in `$XDG_DATA_HOME/cantus` — and none in the folder older
//! builds used. Its own test binary: it sets the process's environment.
#![cfg(target_os = "linux")]

use cantus_engine::EngineHandle;
use cantus_ui::app::{config_dir, data_dir, Files};
use cantus_ui::CantusApp;

#[test]
fn the_assembled_app_keeps_its_files_in_the_xdg_places() {
    let home = tempfile::tempdir().unwrap();
    std::env::remove_var("CANTUS_DATA_DIR");
    std::env::set_var("HOME", home.path());
    std::env::set_var("XDG_CONFIG_HOME", home.path().join("cfg"));
    std::env::set_var("XDG_DATA_HOME", home.path().join("data"));
    let (config, data) = (config_dir(), data_dir());
    assert_eq!(config, home.path().join("cfg/cantus"));
    assert_eq!(data, home.path().join("data/cantus"));

    let files = Files::read(&config, Some(cantus_ui::i18n::Locale::En));
    let ctx = egui::Context::default();
    let app = CantusApp::assemble(
        &ctx,
        files,
        None,
        EngineHandle::dummy(),
        None,
        None,
        None,
        false,
    );
    assert!(
        app.autosave_dir.starts_with(&data),
        "{:?}",
        app.autosave_dir
    );
    assert!(
        app.recordings_dir().starts_with(&data),
        "{:?}",
        app.recordings_dir()
    );
    // Nothing written where builds before round 30 kept everything.
    assert!(
        !home.path().join("data/Cantus").exists(),
        "the old folder came back"
    );
}

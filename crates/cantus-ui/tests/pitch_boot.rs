//! The pitch editor on the boot path (round 28, ADR 0054): the desktop app goes `run` →
//! `boot::Boot` → `CantusApp::assemble`, never `CantusApp::new`, so the analyses' cache
//! folder is set there. Its own test binary: it points `CANTUS_DATA_DIR` at a temporary
//! folder, and nothing else in the process reads it meanwhile.

use cantus_engine::EngineHandle;
use cantus_ui::app::Files;
use cantus_ui::CantusApp;

#[test]
fn the_boot_path_keeps_analyses_in_the_data_folder() {
    let data = tempfile::tempdir().unwrap();
    std::env::set_var("CANTUS_DATA_DIR", data.path());
    let files = Files::read(data.path(), Some(cantus_ui::i18n::Locale::En));
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
    assert_eq!(
        app.engine.samples.pitch().disk_cache(),
        Some(data.path().join("cache").join("pitch"))
    );
}

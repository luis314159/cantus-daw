//! The MPE input from the boot path (round 33, ADR 0080): the desktop app goes `run` →
//! `boot::Boot` → `CantusApp::assemble`, so the MPE ports and zones of the settings must be
//! published there. Its own test binary, one test: it points `CANTUS_DATA_DIR` at a
//! temporary folder.

use cantus_core::Project;
use cantus_ui::CantusApp;

#[test]
fn the_mpe_ports_reach_the_engine_from_the_boot_path() {
    let data = tempfile::tempdir().unwrap();
    std::env::set_var("CANTUS_DATA_DIR", data.path());
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
    let mut files = cantus_ui::app::Files::read(data.path(), Some(cantus_ui::i18n::Locale::En));
    files.settings.mpe_ports = vec!["Seaboard".into()];
    let ctx = egui::Context::default();
    let _app = CantusApp::assemble(
        &ctx,
        files,
        Some(Project::new("boot")),
        cantus_engine::EngineHandle::dummy(),
        None,
        None,
        None,
        false,
    );
    assert_eq!(
        cantus_ui::mpe::input::last_published(),
        Some(None),
        "assemble publishes the MPE input: none, as the marked port is not open here"
    );
}

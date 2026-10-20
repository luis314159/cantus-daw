//! Freezing on the boot path (round 29, ADR 0064): the desktop app goes `run` →
//! `boot::Boot` → `CantusApp::assemble`, never `CantusApp::new`, so the frozen renders'
//! folder and low-latency monitoring are wired there. Its own test binary: it points
//! `CANTUS_DATA_DIR` at a temporary folder, and nothing else in the process reads it.

use cantus_engine::EngineHandle;
use cantus_ui::app::Files;
use cantus_ui::CantusApp;

#[test]
fn the_boot_path_points_renders_at_the_data_folder_and_tells_the_engine_low_latency() {
    let data = tempfile::tempdir().unwrap();
    std::env::set_var("CANTUS_DATA_DIR", data.path());
    let mut files = Files::read(data.path(), Some(cantus_ui::i18n::Locale::En));
    assert!(!files.settings.low_latency, "off on a new installation");
    assert_eq!(
        files.settings.low_latency_ms,
        cantus_ui::settings::DEFAULT_LOW_LATENCY_MS
    );
    files.settings.low_latency = true;
    let ctx = egui::Context::default();
    let (engine, mut proc) = EngineHandle::manual(48_000.0);
    let app = CantusApp::assemble(&ctx, files, None, engine, None, None, None, false);
    assert_eq!(
        app.engine.frozen.app_dir(),
        Some(data.path().join("cache").join("freeze").as_path())
    );
    let mut buf = vec![0f32; 128 * 2];
    proc.process(&mut buf, 2);
    assert!(
        proc.stats().low_latency.on,
        "the setting reached the engine"
    );
}

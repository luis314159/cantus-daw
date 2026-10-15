//! The boot path designs Chorda's reference keyboard beside the window (ADR 0059): a binary
//! of its own, so nothing else has asked for it first.

use cantus_engine::EngineHandle;
use cantus_ui::app::Files;
use cantus_ui::CantusApp;

#[test]
fn the_boot_path_starts_designing_the_keyboard() {
    let data = tempfile::tempdir().unwrap();
    std::env::set_var("CANTUS_DATA_DIR", data.path());
    let files = Files::read(data.path(), Some(cantus_ui::i18n::Locale::En));
    let ctx = egui::Context::default();
    let engine = EngineHandle::dummy();
    let fs = engine.sample_rate as f32;
    assert!(
        !cantus_engine::inst::chorda::is_warm(fs),
        "nothing asked for it before"
    );
    let _app = CantusApp::assemble(&ctx, files, None, engine, None, None, None, false);
    assert!(
        cantus_engine::inst::chorda::is_warm(fs),
        "the reference keyboard at {fs} Hz is designed or asked for"
    );
}

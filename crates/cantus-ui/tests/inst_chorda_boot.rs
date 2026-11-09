//! Chorda's reference keyboard on the boot path (ADR 0059, amended in round 38): the app
//! `CantusApp::assemble` puts together on the song Cantus opens with (no piano in it) designs
//! nothing at start — that was 7 s of CPU over ten threads after the window, for a piano
//! most sessions never load — and opening the instrument picker asks for it, so it is ready
//! by the time the piano is chosen. A binary of its own, so nothing else has asked first.

use cantus_engine::EngineHandle;
use cantus_ui::app::Files;
use cantus_ui::picker::PickPurpose;
use cantus_ui::CantusApp;

#[test]
fn the_boot_path_leaves_the_keyboard_to_the_instrument_picker() {
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
    let song = cantus_ui::demos::startup_project();
    let mut app = CantusApp::assemble(&ctx, files, Some(song), engine, None, None, None, false);
    assert!(
        !cantus_engine::inst::chorda::is_warm(fs),
        "the start asks for no piano the song does not have"
    );
    // An effect's picker asks for nothing; the instrument picker does.
    app.open_picker(PickPurpose::AddEffect {
        strip: cantus_core::StripId::MASTER,
    });
    assert!(!cantus_engine::inst::chorda::is_warm(fs));
    app.open_picker(PickPurpose::AddInstrument { index: None });
    assert!(
        cantus_engine::inst::chorda::is_warm(fs),
        "the reference keyboard at {fs} Hz is designed or asked for once the picker opens"
    );
}

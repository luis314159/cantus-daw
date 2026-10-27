//! Round 31 (ADR 0070) on the boot path: the desktop app goes `run` → `boot::Boot` →
//! `CantusApp::assemble`, never `CantusApp::new`, so the user's automatic crossfades are
//! told to the command bus there. Its own test binary: it points `CANTUS_DATA_DIR` at a
//! temporary folder, and nothing else in the process reads it.

use cantus_core::crossfade::XfadeCurve;
use cantus_engine::EngineHandle;
use cantus_ui::app::Files;
use cantus_ui::CantusApp;

fn data() -> &'static std::path::Path {
    static DIR: std::sync::OnceLock<tempfile::TempDir> = std::sync::OnceLock::new();
    DIR.get_or_init(|| {
        let d = tempfile::tempdir().unwrap();
        std::env::set_var("CANTUS_DATA_DIR", d.path());
        d
    })
    .path()
}

fn assembled(edit: impl FnOnce(&mut Files)) -> CantusApp {
    let mut files = Files::read(data(), Some(cantus_ui::i18n::Locale::En));
    edit(&mut files);
    let ctx = egui::Context::default();
    let (engine, _proc) = EngineHandle::manual(48_000.0);
    CantusApp::assemble(&ctx, files, None, engine, None, None, None, false)
}

#[test]
fn the_boot_path_tells_the_bus_the_users_automatic_crossfades() {
    let files = Files::read(data(), Some(cantus_ui::i18n::Locale::En));
    assert!(files.settings.auto_crossfade, "on for a new installation");
    assert!(
        !files.settings.insert_neutral,
        "starting presets for a new installation"
    );
    let app = assembled(|_| {});
    assert_eq!(app.bus.auto_crossfades(), Some(XfadeCurve::EqualPower));
    let app = assembled(|f| f.settings.crossfade_curve = XfadeCurve::SCurve);
    assert_eq!(app.bus.auto_crossfades(), Some(XfadeCurve::SCurve));
    let app = assembled(|f| f.settings.auto_crossfade = false);
    assert_eq!(app.bus.auto_crossfades(), None, "off, as the user left it");
}

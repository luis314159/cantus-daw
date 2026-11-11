//! The arrangement's header column in a small window (round 39, fixes; BACKLOG §8): drawn
//! while the window is still small — a first frame, a start in a small window — the
//! headers are drawn narrower, but the width the user left is kept, and the column comes
//! back to it when the window grows. Through `CantusApp::assemble`, as the app starts.
//!
//! Its own test binary: it points `CANTUS_DATA_DIR` at a temporary folder.

use cantus_engine::EngineHandle;
use cantus_ui::app::Files;
use cantus_ui::CantusApp;
use std::path::Path;

fn data() -> &'static Path {
    static DIR: std::sync::OnceLock<tempfile::TempDir> = std::sync::OnceLock::new();
    DIR.get_or_init(|| {
        let d = tempfile::tempdir().unwrap();
        std::env::set_var("CANTUS_DATA_DIR", d.path());
        std::fs::write(
            d.path().join("settings.json"),
            r#"{"audio_setup":{"setup_done":true}}"#,
        )
        .unwrap();
        d
    })
    .path()
}

fn frame(app: &mut CantusApp, ctx: &egui::Context, size: egui::Vec2, t: &mut f64) {
    *t += 1.0 / 60.0;
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
        time: Some(*t),
        predicted_dt: 1.0 / 60.0,
        ..Default::default()
    };
    ctx.run_ui(input, |ui| app.frame(ui)).textures_delta.clear();
}

#[test]
fn a_small_first_window_keeps_the_headers_width_for_when_it_grows() {
    let files = Files::read(data(), Some(cantus_ui::i18n::Locale::En));
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    let (engine, _proc) = EngineHandle::manual(48_000.0);
    let project = cantus_ui::demos::startup_project();
    let mut app = CantusApp::assemble(&ctx, files, Some(project), engine, None, None, None, false);
    // The width the user left the headers at.
    app.arrangement.header_width = 300.0;
    let mut t = 0.0;
    // A start in a small window: the column is drawn narrower than that.
    for _ in 0..10 {
        frame(&mut app, &ctx, egui::vec2(560.0, 420.0), &mut t);
    }
    let small = app.arrangement.header_shown();
    assert!(
        small < 300.0,
        "the arrangement was drawn, narrower than its headers' width: {small}"
    );
    assert_eq!(app.arrangement.header_width, 300.0, "the stored width kept");
    // The window grows: the user's width again.
    for _ in 0..10 {
        frame(&mut app, &ctx, egui::vec2(1920.0, 1080.0), &mut t);
    }
    assert_eq!(app.arrangement.header_width, 300.0);
    assert_eq!(
        app.arrangement.header_shown(),
        300.0,
        "drawn at the user's width"
    );
}

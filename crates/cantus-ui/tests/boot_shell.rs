//! Final review (reach 2): the shipped window is `boot::Shell`, not `CantusApp` itself —
//! once running, each of its frames is exactly `CantusApp`'s own (`eframe::App::ui`), so the
//! two can never drift: the taskbar's light or dark (the Automatic icon) is read there too.

use cantus_engine::EngineHandle;
use cantus_ui::boot::Shell;
use cantus_ui::{starter_project, CantusApp};

#[test]
fn the_running_shell_runs_the_apps_whole_frame() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = CantusApp::from_parts(Some(starter_project()), EngineHandle::dummy(), false);
    app.settings_path = dir.path().join("settings.json");
    app.layouts_path = dir.path().join("layouts.json");
    app.appearance_path = dir.path().join("appearance.json");
    app.keymap_path = dir.path().join("keymap.toml");
    assert!(!app.taskbar.polled());
    let mut shell = Shell::Running(Box::new(app));
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    for _ in 0..2 {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1280.0, 720.0),
            )),
            ..Default::default()
        };
        ctx.run_ui(input, |root| {
            let mut frame = eframe::Frame::_new_kittest();
            eframe::App::ui(&mut shell, root, &mut frame);
        })
        .textures_delta
        .clear();
    }
    let Shell::Running(app) = &shell else {
        panic!("still running");
    };
    assert!(
        app.taskbar.polled(),
        "the running window never read the taskbar (the Automatic icon)"
    );
}

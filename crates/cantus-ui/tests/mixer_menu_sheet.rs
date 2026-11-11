//! The mixer's menus have a sheet (round 39, `shots`): since `des-controls` a list inside a
//! popup paints its rows only and leaves the sheet to the popup's frame, and the mixer's
//! menus open in a bare area with no frame — the Snapshots menu was words floating over the
//! arrangement (beta.14's pictures showed it). Drawn through the app as a user opens it.

use cantus_engine::EngineHandle;
use cantus_ui::{welcome_project, CantusApp};

#[test]
fn the_snapshots_menu_is_drawn_on_its_sheet() {
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    let mut app = CantusApp::from_parts(Some(welcome_project()), EngineHandle::dummy(), false);
    let tmp = std::env::temp_dir().join(format!("cantus-menu-sheet-{}", std::process::id()));
    app.settings_path = tmp.join("settings.json");
    app.layouts_path = tmp.join("layouts.json");
    app.keymap_path = tmp.join("keymap.toml");
    app.appearance_path = tmp.join("appearance.json");
    app.attach(&ctx);
    app.central = cantus_ui::app::CentralView::Mixer;
    let at = egui::pos2(300.0, 160.0);
    cantus_ui::views::mixer::open_snapshots(&mut app, at);
    let canvas = cantus_ui::raster::shoot(&ctx, egui::vec2(1280.0, 720.0), 1.0, 4, None, |root| {
        app.frame(root)
    });
    let menu = cantus_ui::kit::Style::current().t.menu;
    // Inside the sheet's top padding, left of every word: the sheet's face.
    let [r, g, b, _] = canvas.pixel(at.x as usize + 8, at.y as usize + 3);
    let near = |a: u8, b: u8| (a as i32 - b as i32).abs() <= 3;
    assert!(
        near(r, menu.r()) && near(g, menu.g()) && near(b, menu.b()),
        "the menu's sheet is painted: {:?} under it, the sheet is {:?}",
        (r, g, b),
        menu
    );
}

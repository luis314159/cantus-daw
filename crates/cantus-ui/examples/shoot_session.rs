//! The Session pane drawn without a screen (round 33, ADR 0077): the grid playing (a clip
//! playing, one queued, one stopping, one recording, a scene playing and one queued), the
//! clip panel of a gate clip with follow actions, an envelope drawn, the learn mode, a
//! controller grid's session ring, an empty session — in Slate (Cantus Night) and Paper,
//! at 100 % and 200 %; and the whole window with the Session pane in front.
//!
//! ```text
//! cargo run -p cantus-ui --example shoot_session -- [out-dir] [case]
//! ```

use cantus_core::automation::{AutoPoint, AutoTarget, Automation};
use cantus_core::session::{SessionCommand, SlotRef};
use cantus_engine::EngineHandle;
use cantus_ui::kit::appearance::Mode;
use cantus_ui::raster::shoot;
use cantus_ui::views::session;
use cantus_ui::CantusApp;
use std::path::{Path, PathBuf};

fn context(dark: bool) -> egui::Context {
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    let theme = if dark {
        cantus_ui::theme_family::cantus_night()
    } else {
        cantus_ui::theme::paper()
    };
    cantus_ui::theme::apply(&theme, &ctx);
    // The application's own scroll bars (solid, six points), as the window has them.
    cantus_ui::views::workspace_style::install(&ctx);
    ctx
}

/// The app on a demo session, lit, the cursor on track 2's gate clip (its panel open).
fn app(tracks: usize, scenes: usize) -> CantusApp {
    let mut app = CantusApp::from_parts(
        Some(session::demo::project(tracks, scenes)),
        EngineHandle::dummy(),
        false,
    );
    session::demo::showcase(&mut app);
    let t = app.project().tracks[1].id;
    let sc = app.project().session.scenes[1].id;
    let slot = SlotRef::new(t, sc);
    app.session_view.cursor = Some(slot);
    app.session_view.focused = true;
    // An envelope on the clip: the track's volume rising over its loop.
    let len = app
        .project()
        .slot_clip(slot)
        .map(|c| c.pass())
        .unwrap_or(cantus_core::Ticks::beats(4));
    let env = Automation::new(
        AutoTarget::TrackGain(t),
        vec![
            AutoPoint::new(cantus_core::Ticks::ZERO, 0.3),
            AutoPoint::new(cantus_core::Ticks(len.0 / 2), 0.8),
            AutoPoint::new(len, 0.55),
        ],
    );
    app.exec(cantus_core::Command::Session(
        SessionCommand::SetSlotEnvelopes {
            slot,
            envelopes: vec![env],
        },
    ));
    app.session_view.env.target = Some(AutoTarget::TrackGain(t));
    app
}

fn pane(out: &Path, name: &str, dark: bool, zoom: f32, size: (f32, f32), mut app: CantusApp) {
    let ctx = context(dark);
    let theme = if dark { "slate" } else { "paper" };
    let path = out.join(format!(
        "session-{name}-{theme}-{}.png",
        (zoom * 100.0) as u32
    ));
    shoot(
        &ctx,
        egui::vec2(size.0, size.1),
        zoom,
        4,
        Some(&path),
        |ui| session::show(&mut app, ui),
    );
    println!("{}", path.display());
}

fn window(out: &Path, dark: bool) {
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    let tmp = std::env::temp_dir().join("cantus-shoot-session");
    let mut app = app(8, 8);
    app.settings_path = tmp.join("settings.json");
    app.layouts_path = tmp.join("layouts.json");
    app.keymap_path = tmp.join("keymap.toml");
    app.appearance_path = tmp.join("appearance.json");
    app.appearance.mode = if dark { Mode::Dark } else { Mode::Light };
    let mode = app.appearance.mode;
    let preset = if dark { "cantus_night" } else { "cantus_paper" };
    app.appearance.assign(mode, format!("preset:{preset}"));
    app.attach(&ctx);
    app.show_pane(cantus_ui::views::workspace::Pane::Session);
    let theme = if dark { "slate" } else { "paper" };
    let path = out.join(format!("session-window-{theme}-1600x900.png"));
    cantus_ui::raster::shoot(
        &ctx,
        egui::vec2(1600.0, 900.0),
        1.0,
        8,
        Some(&path),
        |root| app.frame(root),
    );
    println!("{}", path.display());
}

/// Settings ▸ MIDI with four inputs (MPE marked on one, a Launchpad on another): a
/// 1600 × 900 screen at 100 %, a 1080p one at 200 % (960 × 540 points).
fn midi(out: &Path, dark: bool, zoom: f32) {
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    let tmp = std::env::temp_dir().join("cantus-shoot-session");
    let mut app = app(8, 8);
    app.settings_path = tmp.join("settings.json");
    app.layouts_path = tmp.join("layouts.json");
    app.keymap_path = tmp.join("keymap.toml");
    app.appearance_path = tmp.join("appearance.json");
    app.appearance.mode = if dark { Mode::Dark } else { Mode::Light };
    let mode = app.appearance.mode;
    let preset = if dark { "cantus_night" } else { "cantus_paper" };
    app.appearance.assign(mode, format!("preset:{preset}"));
    app.attach(&ctx);
    app.settings.ui_scale = zoom;
    app.settings.mpe_ports = vec!["Seaboard RISE 2".into()];
    cantus_ui::views::workspace_settings::demo_midi(
        &mut app,
        &[
            "Keystation 49",
            "MIDIIN2 (LPMiniMK3 MIDI)",
            "Seaboard RISE 2",
            "Launch Control XL",
        ],
        &["Drum Machine", "Launch Control XL"],
    );
    let theme = if dark { "slate" } else { "paper" };
    let pct = (zoom * 100.0).round() as u32;
    let path = out.join(format!("settings-midi-{theme}-{pct}.png"));
    let size = if zoom > 1.0 {
        egui::vec2(1920.0 / zoom, 1080.0 / zoom)
    } else {
        egui::vec2(1600.0, 900.0)
    };
    cantus_ui::raster::shoot(&ctx, size, zoom, 8, Some(&path), |root| app.frame(root));
    println!("{}", path.display());
}

fn main() {
    let out = PathBuf::from(
        std::env::args()
            .nth(1)
            .unwrap_or_else(|| "target/shoot-session".into()),
    );
    let only = std::env::args().nth(2);
    std::fs::create_dir_all(&out).expect("out dir");
    let want = |c: &str| only.as_deref().is_none_or(|o| c.starts_with(o));
    for dark in [true, false] {
        for zoom in [1.0f32, 2.0] {
            if want("grid") {
                pane(&out, "grid", dark, zoom, (1400.0, 720.0), app(8, 8));
            }
        }
        if want("learn") {
            let mut a = app(6, 6);
            a.controllers.learn_mode = true;
            pane(&out, "learn", dark, 1.0, (1200.0, 560.0), a);
        }
        if want("ring") {
            let mut a = app(10, 10);
            a.session_view.panel_open = false;
            a.session_view.rings = vec![(1, 1, 8, 8)];
            pane(&out, "ring", dark, 1.0, (1400.0, 560.0), a);
        }
        if want("empty") {
            let mut p = session::demo::project(4, 0);
            p.session.scenes.clear();
            let a = CantusApp::from_parts(Some(p), EngineHandle::dummy(), false);
            pane(&out, "empty", dark, 1.0, (900.0, 360.0), a);
        }
        if want("scene") {
            let mut a = app(8, 8);
            let sc = a.project().session.scenes[3].id;
            let t = a.project().tracks[0].id;
            a.session_view.cursor = Some(SlotRef::new(t, sc));
            a.session_view.scene_chosen = true;
            pane(&out, "scene", dark, 1.0, (1400.0, 720.0), a);
        }
        if want("narrow") {
            pane(&out, "narrow", dark, 1.0, (720.0, 520.0), app(8, 8));
        }
        if want("window") {
            window(&out, dark);
        }
        if want("midi") {
            for zoom in [1.0, 2.0] {
                midi(&out, dark, zoom);
            }
        }
    }
}

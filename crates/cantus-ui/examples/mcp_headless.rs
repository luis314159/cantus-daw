//! Cantus's MCP door with no window (round 34, ADR 0081): a real `CantusApp` — the
//! application's own catalogue, permissions and pump — turned by headless frames, its door
//! open for external agents. For checking real MCP clients against it (MCP Inspector's CLI,
//! `claude mcp add` / `claude mcp list`, the stdio bridge) without opening the DAW:
//!
//! ```text
//! cargo run -p cantus-ui --example mcp_headless -- <settings folder> <port> [seconds]
//! ```
//!
//! It prints `READY <port> <token>` once the door is open, then serves for `seconds`
//! (default 120). The settings folder gets the token and the door's file, as the app's own
//! does, so the bridge started with `CANTUS_DATA_DIR=<settings folder>` finds it.

use cantus_core::Project;
use cantus_ui::CantusApp;
use std::time::{Duration, Instant};

fn main() {
    let mut args = std::env::args().skip(1);
    let dir = std::path::PathBuf::from(args.next().expect("a settings folder"));
    let port: u16 = args.next().and_then(|p| p.parse().ok()).unwrap_or(39571);
    let seconds: u64 = args.next().and_then(|s| s.parse().ok()).unwrap_or(120);
    std::fs::create_dir_all(&dir).expect("the settings folder");
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
    let mut p = Project::new("headless");
    let t = p.make_track("Bass");
    p.tracks.push(t);
    let mut app = CantusApp::from_parts(Some(p), cantus_engine::EngineHandle::dummy(), false);
    app.settings_path = dir.join("settings.json");
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    cantus_ui::ai_mcp::wire(&mut app, &ctx);
    app.ai.door.dir = Some(dir.clone());
    app.settings.ai.external_agents = true;
    app.settings.ai.port = port;
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1400.0, 900.0),
        )),
        ..Default::default()
    };
    let frame = |app: &mut CantusApp| {
        ctx.run_ui(input.clone(), |ui| app.frame(ui))
            .textures_delta
            .clear();
    };
    frame(&mut app);
    let Some(open) = app.ai.door.port() else {
        eprintln!("the door did not open (port {port} taken?)");
        std::process::exit(1);
    };
    let token = cantus_mcp::home::token(&dir).expect("the token");
    println!("READY {open} {token}");
    let end = Instant::now() + Duration::from_secs(seconds);
    while Instant::now() < end {
        frame(&mut app);
        std::thread::sleep(Duration::from_millis(15));
    }
}

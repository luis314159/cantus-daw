//! The MCP server from the boot path (round 34, ADR 0081): the desktop app goes `run` →
//! `boot::Boot` → `CantusApp::assemble`, so the server and its pump must be made there,
//! and the pump must turn in the real frame (`CantusApp::frame`). Its own test binary, one
//! test: it points `CANTUS_DATA_DIR` at a temporary folder.

use cantus_core::Project;
use cantus_mcp::client::TestClient;
use cantus_ui::CantusApp;
use serde_json::json;

#[test]
fn the_server_is_made_by_assemble_and_served_by_the_frame() {
    let data = tempfile::tempdir().unwrap();
    std::env::set_var("CANTUS_DATA_DIR", data.path());
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
    let files = cantus_ui::app::Files::read(data.path(), Some(cantus_ui::i18n::Locale::En));
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    let mut app = CantusApp::assemble(
        &ctx,
        files,
        Some(Project::new("boot")),
        cantus_engine::EngineHandle::dummy(),
        None,
        None,
        None,
        false,
    );
    assert!(
        app.ai.is_wired(),
        "assemble makes the MCP server and its pump"
    );
    let server = app.ai.server().unwrap().clone();
    let mut client = TestClient::legacy(server, "2025-11-25");
    // The tool table is the application's registry, the server's own tools included.
    let list = client.request("tools/list", json!({}), &mut || {}).unwrap();
    let names: Vec<String> = list["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap().to_string())
        .collect();
    let registry: Vec<&str> = cantus_ui::ai_tools::registry()
        .iter()
        .map(|t| t.name)
        .collect();
    assert_eq!(names, registry);
    assert!(names.iter().any(|n| n == "wait_job"));
    // The whole list as agents see it — names, titles, descriptions, annotations, schemas
    // — changes only on purpose: `CANTUS_TOOLS_DUMP=1` writes it again, and the diff is
    // reviewed with the change that made it.
    let golden =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/mcp-tools-list.json");
    let now = serde_json::to_string_pretty(&list).unwrap() + "\n";
    if std::env::var_os("CANTUS_TOOLS_DUMP").is_some() {
        std::fs::write(&golden, &now).unwrap();
    }
    let kept = std::fs::read_to_string(&golden)
        .unwrap_or_default()
        .replace("\r\n", "\n");
    assert!(
        kept == now,
        "tools/list changed: run with CANTUS_TOOLS_DUMP=1 and review the diff of {}",
        golden.display()
    );
    // A call is answered by the window's own frame, nothing else turning the pump.
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1400.0, 900.0),
        )),
        ..Default::default()
    };
    let r = client.call_tool("list_jobs", json!({}), &mut || {
        ctx.run_ui(input.clone(), |ui| app.frame(ui))
            .textures_delta
            .clear();
    });
    assert!(!r.is_error, "{}", r.text);
    assert_eq!(r.structured.unwrap()["jobs"], json!([]));
    let seen = app.ai.agents().unwrap().activity().count();
    assert_eq!(seen, 1, "the frame's pump served it");
}

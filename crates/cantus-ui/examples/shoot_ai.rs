//! The window's side of agents (round 34, ADR 0081), screenshotted without a screen:
//!
//! - `card`: the whole window with an agent connected (the top bar's chip) and its call
//!   waiting on the confirmation card;
//! - `activity`: the AI activity open — the agent, its calls (one refused), the call waiting;
//! - `settings`: Settings ▸ AI with external agents allowed;
//! - `bar[-none][-narrow]`: the top bar alone at 1920 px (or 1100), with the chip (or with
//!   no agent connected, the chip hidden).
//!
//! Each in Cantus Night and Paper, English and Spanish.
//!
//! `cargo run -p cantus-ui --example shoot_ai -- <out-dir> [filter…]`

use cantus_ai::{EditPlan, Said, ToolClass, ToolCx, ToolDef, ToolError, ToolRun};
use cantus_engine::EngineHandle;
use cantus_mcp::client::TestClient;
use cantus_ui::i18n::Locale;
use cantus_ui::views::transport;
use cantus_ui::{welcome_project, CantusApp};
use egui::vec2;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

fn schema() -> Value {
    json!({"type": "object", "properties": {}})
}

/// A destructive call for the card to ask about (the shot's own: the project's tools word
/// theirs the same way).
fn remove_first(cx: &mut ToolCx<'_>, _: &Value) -> Result<EditPlan, ToolError> {
    let t = cx.project().tracks[0].clone();
    Ok(EditPlan {
        command: cantus_core::Command::RemoveTrack { id: t.id },
        said: Said::new("workflow.history.remove_track").name("name", t.name),
        result: json!({}),
    })
}

static REMOVE: ToolDef = ToolDef {
    name: "remove_track",
    title: "Remove a track",
    description: "Shot.",
    class: ToolClass::Destructive,
    input_schema: schema,
    output_schema: None,
    run: ToolRun::Edit(remove_first),
};

fn tools() -> Vec<&'static ToolDef> {
    let mut v = cantus_ui::ai_tools::registry();
    v.push(&REMOVE);
    v
}

fn main() {
    let mut args = std::env::args().skip(1);
    let dir = PathBuf::from(args.next().unwrap_or_else(|| "shots".into()));
    let filters: Vec<String> = args.collect();
    std::fs::create_dir_all(&dir).expect("output dir");
    let whats = [
        "card",
        "activity",
        "settings",
        "bar",
        "bar-narrow",
        "bar-none",
        "bar-none-narrow",
    ];
    for what in whats {
        for theme in ["night", "paper"] {
            for lang in [Locale::En, Locale::Es] {
                let name = format!("ai-{what}-{theme}-{}", lang.code());
                if !filters.is_empty() && !filters.iter().any(|f| name.contains(f.as_str())) {
                    continue;
                }
                cantus_ui::i18n::set_locale(lang);
                let path = dir.join(format!("{name}.png"));
                shoot(&path, what, theme);
                println!("wrote {}", path.display());
            }
        }
    }
    // The waiting calls' threads end with the process.
    std::process::exit(0);
}

fn context(theme: &str) -> egui::Context {
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    let t = if theme == "paper" {
        cantus_ui::theme::paper()
    } else {
        cantus_ui::theme_family::cantus_night()
    };
    cantus_ui::theme::apply(&t, &ctx);
    ctx
}

fn app_for(theme: &str, ctx: &egui::Context) -> CantusApp {
    use cantus_ui::kit::appearance::Mode;
    let mut app = CantusApp::from_parts(Some(welcome_project()), EngineHandle::dummy(), false);
    let tmp = std::env::temp_dir().join("cantus-shoot-ai");
    app.settings_path = tmp.join("settings.json");
    app.layouts_path = tmp.join("layouts.json");
    app.keymap_path = tmp.join("keymap.toml");
    app.appearance_path = tmp.join("appearance.json");
    let (preset, mode) = if theme == "paper" {
        ("paper", Mode::Light)
    } else {
        ("cantus_night", Mode::Dark)
    };
    app.appearance.mode = mode;
    app.appearance.assign(mode, format!("preset:{preset}"));
    let catalog = cantus_mcp::Catalog {
        tools,
        ..cantus_ui::ai_mcp::catalog::app()
    };
    cantus_ui::ai_mcp::wire_with(&mut app, ctx, catalog);
    app.ai.door.dir = Some(tmp.join("home"));
    app
}

/// An agent called a few times, one call refused, and one waits on the card. The frames
/// that serve it run on a context of their own: the shot's keeps its font atlas to upload.
fn agent_at_work(app: &mut CantusApp, theme: &str) {
    let scratch = context(theme);
    let ctx = &scratch;
    let server = app.ai.server().unwrap().clone();
    let mut c = TestClient::raw(server.clone());
    c.name = "Claude Code".into();
    c.request(
        "initialize",
        json!({"protocolVersion": "2025-11-25", "capabilities": {},
               "clientInfo": {"name": "Claude Code", "version": "2.1"}}),
        &mut || {},
    )
    .unwrap();
    let frame = |app: &mut CantusApp| {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                vec2(1600.0, 1000.0),
            )),
            ..Default::default()
        };
        ctx.run_ui(input, |ui| app.frame(ui)).textures_delta.clear();
    };
    for tool in ["list_jobs", "list_ai_actions"] {
        c.call_tool(tool, json!({}), &mut || frame(app));
    }
    let _ = c.call_tool(
        "begin_change",
        json!({"label": "write the verse"}),
        &mut || frame(app),
    );
    app.settings.ai.denied_tools = vec!["undo_last_ai_action".into()];
    let _ = c.call_tool("undo_last_ai_action", json!({}), &mut || frame(app));
    app.settings.ai.denied_tools.clear();
    // The call that waits: on its own thread, as a transport's would.
    let session = c.session().clone();
    std::thread::spawn(move || {
        let mut w = TestClient::raw_on(server, session);
        w.name = "Claude Code".into();
        let _ = w.call_tool("remove_track", json!({}), &mut || {});
    });
    let started = std::time::Instant::now();
    while app.ai.agents().is_some_and(|a| a.asks().next().is_none()) {
        frame(app);
        assert!(
            started.elapsed().as_secs() < 10,
            "the call reaches the card"
        );
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

fn shoot(path: &Path, what: &str, theme: &str) {
    let ctx = context(theme);
    let mut app = app_for(theme, &ctx);
    app.attach(&ctx);
    match what {
        bar if bar.starts_with("bar") => {
            if !bar.contains("none") {
                agent_at_work(&mut app, theme);
            }
            let width = if bar.contains("narrow") {
                1100.0
            } else {
                1920.0
            };
            let size = vec2(width, 54.0);
            cantus_ui::raster::shoot(&ctx, size, 1.0, 6, Some(path), |root| {
                egui::Panel::top(egui::Id::new("toolbar"))
                    .frame(egui::Frame::NONE)
                    .exact_size(transport::HEIGHT)
                    .show(root, |ui| transport::show(&mut app, ui));
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE.fill(cantus_ui::kit::Style::current().t.panel))
                    .show(root, |_| {});
            });
            return;
        }
        "settings" => {
            app.settings.ai.external_agents = true;
            app.settings.ai.allow_files = true;
            app.settings.ai.folders = vec![PathBuf::from("D:/Music/Samples")];
            cantus_ui::views::workspace_settings::open(
                &mut app,
                cantus_ui::views::workspace_settings::SettingsPage::Ai,
            );
        }
        "activity" => {
            agent_at_work(&mut app, theme);
            app.ai.activity_open = true;
        }
        _ => agent_at_work(&mut app, theme),
    }
    cantus_ui::raster::shoot(&ctx, vec2(1600.0, 1000.0), 1.0, 8, Some(path), |root| {
        app.frame(root)
    });
}

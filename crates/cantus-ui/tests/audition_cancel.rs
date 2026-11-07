//! Round 37's fixes, item 4 of the second review: an agent's `audition_preset` cancelled
//! while it renders answers Cancelled and plays nothing. Through `CantusApp::assemble`, its
//! MCP server and the manual engine; the job is held right after its render by the test's
//! gate (`ai_tools::library::RENDERED_HOOK`), cancelled with `cancel_job`, then let go. Its
//! own binary: the gate is process-wide, and it points `CANTUS_DATA_DIR` at a temporary
//! folder.

use cantus_core::*;
use cantus_engine::EngineHandle;
use cantus_mcp::client::{TestClient, ToolCall};
use cantus_mcp::Answer;
use cantus_ui::CantusApp;
use serde_json::{json, Value};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

fn call(app: &mut CantusApp, c: &mut TestClient, name: &str, args: Value) -> ToolCall {
    c.call_tool(name, args, &mut || {
        cantus_ui::ai_mcp::pump(app);
        app.poll_preset_audition();
        if let Some(agents) = app.ai.agents_mut() {
            let ids: Vec<u64> = agents.asks().map(|x| x.id).collect();
            for id in ids {
                agents.answer(id, Answer::Allow);
            }
        }
    })
}

#[test]
fn an_agents_audition_cancelled_while_it_renders_answers_cancelled_and_plays_nothing() {
    let data = tempfile::tempdir().unwrap();
    std::env::set_var("CANTUS_DATA_DIR", data.path());
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
    let files = cantus_ui::app::Files::read(data.path(), Some(cantus_ui::i18n::Locale::En));
    let ctx = egui::Context::default();
    let (engine, mut proc) = EngineHandle::manual(48_000.0);
    let mut app = CantusApp::assemble(
        &ctx,
        files,
        Some(Project::new("Cancel")),
        engine,
        None,
        None,
        None,
        false,
    );
    let mut c = TestClient::legacy(app.ai.server().unwrap().clone(), "2025-11-25");
    // The gate: the worker says it has rendered, then waits to be let go.
    let (entered_tx, entered) = mpsc::channel::<()>();
    let (release, release_rx) = mpsc::channel::<()>();
    let entered_tx = Mutex::new(entered_tx);
    let release_rx = Mutex::new(release_rx);
    *cantus_ui::ai_tools::library::RENDERED_HOOK.lock().unwrap() = Some(Arc::new(move || {
        let _ = entered_tx.lock().unwrap().send(());
        let _ = release_rx
            .lock()
            .unwrap()
            .recv_timeout(Duration::from_secs(60));
    }));
    let found = call(
        &mut app,
        &mut c,
        "find_presets",
        json!({"unit": "unda", "limit": 1}),
    );
    let name = found.structured.unwrap()["presets"][0]["name"]
        .as_str()
        .unwrap()
        .to_string();
    // The audition: held after its render, past the call's inline wait — a job handle.
    let r = call(
        &mut app,
        &mut c,
        "audition_preset",
        json!({"unit": "unda", "preset": name}),
    );
    assert!(!r.is_error, "{}", r.text);
    let v = r.structured.unwrap();
    let job = v["job"]
        .as_str()
        .expect("a job handle while it is held")
        .to_string();
    entered
        .recv_timeout(Duration::from_secs(60))
        .expect("the render finished and reached the gate");
    assert!(
        app.presets.audition.rendering(),
        "the browser's audition waits for it"
    );
    // The agent cancels it while it is held, then it is let go.
    let cancel = call(&mut app, &mut c, "cancel_job", json!({"job": job}));
    assert!(!cancel.is_error, "{}", cancel.text);
    release.send(()).unwrap();
    let w = call(
        &mut app,
        &mut c,
        "wait_job",
        json!({"job": job, "timeout_s": 30}),
    );
    let said = format!("{} {}", w.text, w.structured.clone().unwrap_or(Value::Null));
    assert!(
        said.to_lowercase().contains("cancel"),
        "the job answered Cancelled: {said}"
    );
    assert!(
        w.structured.as_ref().is_none_or(|s| s
            .get("result")
            .is_none_or(|r| r.get("loudness_lufs").is_none())),
        "no measurement of a cancelled audition: {said}"
    );
    // Nothing reaches the player: the browser's audition lets go of it, and the engine
    // plays silence.
    let start = Instant::now();
    while app.presets.audition.rendering() && start.elapsed() < Duration::from_secs(10) {
        app.poll_preset_audition();
        std::thread::sleep(Duration::from_millis(5));
    }
    let a = &app.presets.audition;
    assert!(!a.rendering() && a.heard.is_none() && a.until.is_none());
    assert!(
        a.last.is_none() && a.renders == 0,
        "nothing was handed to the player"
    );
    app.sync_engine();
    let mut loudest = 0f32;
    for _ in 0..60 {
        let mut buf = vec![0f32; 1024];
        proc.process(&mut buf, 2);
        loudest = buf.iter().fold(loudest, |m, x| m.max(x.abs()));
    }
    assert_eq!(loudest, 0.0, "the engine played nothing");
    *cantus_ui::ai_tools::library::RENDERED_HOOK.lock().unwrap() = None;
}

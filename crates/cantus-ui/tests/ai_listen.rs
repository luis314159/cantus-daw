//! The listening tools and the mastering assistant through the MCP server, with the real
//! application as their host (round 34, ADRs 0083–0084): an agent hears the song as
//! numbers, sets tracks to loudness targets in one undo step, and masters for a platform —
//! the result read back from a real render.

use cantus_core::*;
use cantus_mcp::client::{TestClient, ToolCall};
use cantus_mcp::Answer;
use cantus_ui::CantusApp;
use serde_json::{json, Value};

/// Two Firmus parts, eight bars at 120 BPM.
fn song() -> Project {
    let mut p = Project::new("ai-listen");
    p.tempo_bpm = 120.0;
    for (name, wave, root) in [
        ("Lead", Waveform::Saw, 60u8),
        ("Bass", Waveform::Square, 36),
    ] {
        let mut t = p.make_track(name);
        let f = t.instrument.firmus_mut().unwrap();
        f.osc1_wave = wave;
        let mut c = p.make_clip(name, Ticks::ZERO, Ticks::beats(32));
        for b in 0..32i64 {
            let n = p.make_note(
                Ticks::beats(b),
                Ticks::QUARTER,
                root + [0u8, 3, 7, 5][(b % 4) as usize],
                100,
            );
            c.notes.push(n);
        }
        t.clips.push(c);
        p.tracks.push(t);
    }
    p
}

fn app() -> (CantusApp, TestClient) {
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
    let mut app = CantusApp::from_parts(Some(song()), cantus_engine::EngineHandle::dummy(), false);
    let ctx = egui::Context::default();
    cantus_ui::ai_mcp::wire_with(&mut app, &ctx, cantus_ui::ai_mcp::catalog::app());
    let client = TestClient::legacy(app.ai.server().unwrap().clone(), "2025-11-25");
    (app, client)
}

fn call(app: &mut CantusApp, c: &mut TestClient, name: &str, args: Value) -> ToolCall {
    c.call_tool(name, args, &mut || {
        cantus_ui::ai_mcp::pump(app);
        if let Some(agents) = app.ai.agents_mut() {
            let ids: Vec<u64> = agents.asks().map(|x| x.id).collect();
            for id in ids {
                agents.answer(id, Answer::Allow);
            }
        }
    })
}

/// A tool's answer, waiting on its job when it answered with a handle.
fn answer(app: &mut CantusApp, c: &mut TestClient, name: &str, args: Value) -> Value {
    let r = call(app, c, name, args);
    assert!(!r.is_error, "{name}: {}", r.text);
    let mut v = r.structured.expect("structured");
    for _ in 0..40 {
        let Some(job) = v.get("job").and_then(Value::as_str).map(str::to_string) else {
            return v;
        };
        if v["state"] == "done" {
            return v["result"].clone();
        }
        let w = call(app, c, "wait_job", json!({ "job": job, "timeout_s": 30 }));
        assert!(!w.is_error, "{name}: {}", w.text);
        v = w.structured.expect("structured");
    }
    panic!("{name} never finished");
}

#[test]
fn the_listening_tools_are_listed_with_their_prompt() {
    let (mut app, mut c) = app();
    let list = c
        .request("tools/list", json!({}), &mut || {
            cantus_ui::ai_mcp::pump(&mut app)
        })
        .unwrap();
    let names: Vec<&str> = list["tools"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|t| t["name"].as_str())
        .collect();
    for want in [
        "audition",
        "analyse_tracks",
        "masking_matrix",
        "set_mix_targets",
        "master_assist",
        "apply_master_chain",
        "replace_master_chain",
    ] {
        assert!(names.contains(&want), "{want} missing from {names:?}");
    }
    let prompts = c
        .request("prompts/list", json!({}), &mut || {
            cantus_ui::ai_mcp::pump(&mut app)
        })
        .unwrap();
    assert!(
        prompts.to_string().contains("master_for_streaming"),
        "{prompts}"
    );
    let got = c
        .request(
            "prompts/get",
            json!({ "name": "master_for_streaming", "arguments": { "platform": "apple_music" } }),
            &mut || cantus_ui::ai_mcp::pump(&mut app),
        )
        .unwrap();
    assert!(got.to_string().contains("-16"), "{got}");
    assert!(
        got.to_string().contains("replace_master_chain"),
        "the prompt names the destructive tool: {got}"
    );
}

#[test]
fn an_agent_listens_sets_targets_and_masters_through_the_application() {
    let (mut app, mut c) = app();
    let mut before = app.project().clone();
    let heard = answer(&mut app, &mut c, "audition", json!({ "wav": false }));
    assert!(heard["loudness"]["integrated_lufs"].is_number(), "{heard}");
    // Mix targets: one undo step, landed within a tenth of a LU.
    let lead = app.project().tracks[0].id.0;
    let _ = answer(
        &mut app,
        &mut c,
        "set_mix_targets",
        json!({ "targets": [{ "track_id": lead, "lufs": -22.0 }] }),
    );
    let t = answer(
        &mut app,
        &mut c,
        "analyse_tracks",
        json!({ "tracks": [lead] }),
    );
    let got = t["tracks"][0]["loudness"]["integrated_lufs"]
        .as_f64()
        .unwrap();
    assert!((got - -22.0).abs() < 0.1, "{got}");
    // The assistant, then the master read back from a real render.
    let prop = answer(
        &mut app,
        &mut c,
        "master_assist",
        json!({ "platform": "spotify" }),
    );
    let r = call(
        &mut app,
        &mut c,
        "apply_master_chain",
        json!({ "proposal_id": prop["proposal_id"] }),
    );
    assert!(!r.is_error, "{}", r.text);
    let after = answer(&mut app, &mut c, "audition", json!({ "wav": false }));
    let lufs = after["loudness"]["integrated_lufs"].as_f64().unwrap();
    let tp = after["loudness"]["true_peak_dbtp"].as_f64().unwrap();
    assert!((lufs - -14.0).abs() <= 0.5, "{lufs}");
    assert!(tp <= -1.0 + 1e-3, "{tp}");
    // Every step the agent's own; the user's Undo takes both back exactly.
    app.undo();
    app.undo();
    let mut now = app.project().clone();
    (before.next_id, now.next_id) = (0, 0);
    assert_eq!(now, before);
}

/// The panel is drawn by `CantusApp::frame` itself (the boot's path): opened as the master's
/// menu opens it, it shows; a proposal from its own worker comes back through the frames;
/// *Apply* is one undo step on the master.
#[test]
fn the_assistant_panel_runs_through_the_applications_frame() {
    use cantus_ui::ai_tools::listen::panel;
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
    let mut app = CantusApp::from_parts(Some(song()), cantus_engine::EngineHandle::dummy(), false);
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    panel::open(&mut app);
    panel::start(&mut app);
    assert!(
        app.listen.panel.running.is_some(),
        "{:?}",
        app.listen.panel.error
    );
    for _ in 0..3000 {
        if app.listen.panel.done.is_some() {
            break;
        }
        let _ = cantus_ui::raster::shoot(&ctx, egui::vec2(1280.0, 800.0), 1.0, 1, None, |root| {
            app.frame(root)
        });
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let done = app
        .listen
        .panel
        .done
        .as_ref()
        .expect("the worker answered through the frames");
    assert!((done.proposal.predicted.integrated_lufs.unwrap() - -14.0).abs() <= 0.5);
    let before = app.bus.history().len();
    panel::apply(&mut app);
    assert_eq!(app.bus.history().len(), before + 1, "one undo step");
    assert!(!app.project().master.effects.is_empty());
}

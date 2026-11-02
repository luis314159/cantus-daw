//! The dancer for agents (round 35, ADR 0087, ADR 0082), through the in-process MCP
//! server: the catalogue lists it with its keys, moves and moods by name and its move map;
//! `add_track` makes one; `automate` draws its `move`; `set_instrument_param` sets a mood;
//! what was made reads back from the project and through `get_unit`.

use cantus_core::inst::dancer::{self, Move};
use cantus_core::*;
use cantus_mcp::client::{TestClient, ToolCall};
use cantus_mcp::Answer;
use cantus_ui::CantusApp;
use serde_json::{json, Value};

fn app_with(p: Project) -> (CantusApp, TestClient) {
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
    let mut app = CantusApp::from_parts(Some(p), cantus_engine::EngineHandle::dummy(), false);
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

fn ok(app: &mut CantusApp, c: &mut TestClient, name: &str, args: Value) -> Value {
    let r = call(app, c, name, args.clone());
    assert!(!r.is_error, "{name}({args}): {}", r.text);
    r.structured.unwrap_or(Value::Null)
}

#[test]
fn the_catalogue_describes_the_dancer_with_its_moves_moods_and_map() {
    let (mut app, mut c) = app_with(Project::new("Dance"));
    let list = ok(&mut app, &mut c, "list_units", json!({}));
    assert!(list["units"]
        .as_array()
        .unwrap()
        .iter()
        .any(|u| u["id"] == "dancer" && u["type"] == "instrument"));
    let u = ok(&mut app, &mut c, "get_unit", json!({"id": "dancer"}));
    let params = u["params"].as_array().unwrap();
    assert_eq!(params.len(), dancer::PARAMS.len());
    let mv = params.iter().find(|p| p["key"] == "move").unwrap();
    assert_eq!(mv["taper"], "stepped");
    let words = mv["choices"].to_string() + &mv.to_string();
    assert!(
        words.contains("To the beat") && words.contains("Clap"),
        "{mv}"
    );
    let view = params.iter().find(|p| p["key"] == "view.mode").unwrap();
    assert_eq!(view["automatable"], false);
    let map = u["move_map"].as_array().unwrap();
    assert_eq!(map.len(), 16);
    assert_eq!(map[0]["key"], dancer::BASE_KEY);
    assert_eq!(map[0]["note"], "C3");
    assert_eq!(map[5]["move"], "clap");
    assert_eq!(u["moves"].as_array().unwrap().len(), 17);
    assert_eq!(u["moods"].as_array().unwrap()[6]["name"], "Cool");
    assert!(!u["characters"].as_array().unwrap().is_empty());
    assert!(u["playable"].as_str().unwrap().contains("move map"));
    assert!(
        !u["what"].as_str().unwrap().contains("  "),
        "no run of spaces"
    );
}

#[test]
fn an_agent_makes_a_dancer_automates_its_move_and_reads_it_back() {
    let (mut app, mut c) = app_with(Project::new("Dance"));
    app.settings.ai.allow_edits = true;
    let made = ok(
        &mut app,
        &mut c,
        "add_track",
        json!({"kind": "dancer", "name": "Trino"}),
    );
    let track = TrackId(made["track_id"].as_u64().unwrap());
    let spec = app
        .project()
        .track(track)
        .unwrap()
        .instrument
        .native()
        .unwrap()
        .clone();
    assert_eq!(spec.kind, InstrumentKind::Dancer);
    assert_eq!(
        spec.text(dancer::CHARACTER_KEY),
        Some(dancer::DEFAULT_CHARACTER),
        "an agent's dancer names its character as the picker's does"
    );
    // Clap over bars 3–4, back to the routine at bar 5.
    let clap = dancer::move_value(Move::Clap);
    ok(
        &mut app,
        &mut c,
        "automate",
        json!({"target": {"track_id": track.0, "param": "move"},
               "points": [{"at": "3:1", "value": clap}, {"at": "5:1", "value": 0}]}),
    );
    ok(
        &mut app,
        &mut c,
        "set_instrument_param",
        json!({"track_id": track.0, "params": {"mood": 6, "energy": 0.9}}),
    );
    let t = app.project().track(track).unwrap();
    let auto = t
        .clips
        .iter()
        .find_map(|c| c.automation.as_ref().map(|a| (c, a)))
        .expect("an automation clip");
    assert_eq!(
        auto.1.target,
        AutoTarget::Instrument {
            track,
            key: "move".into()
        }
    );
    let n = t.instrument.native().unwrap();
    assert_eq!(n.get("mood"), 6.0);
    assert_eq!(n.get("energy"), 0.9);
    // Through the server too: the track says what it hosts and holds.
    let got = ok(&mut app, &mut c, "get_track", json!({"track_id": track.0}));
    assert!(got.to_string().contains("dancer"), "{got}");
}

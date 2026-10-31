//! The pump with the real host (round 34, ADR 0081): an agent's edit through the
//! application is one undo step with `Origin::Mcp`, named "AI: …" in the history, undone
//! exactly by the user's Undo; the user's settings decide by class.

use cantus_ai::{EditPlan, Said, ToolClass, ToolCx, ToolDef, ToolError, ToolRun};
use cantus_core::{Command, Origin, Project};
use cantus_mcp::client::{TestClient, ToolCall};
use cantus_mcp::{Answer, Catalog};
use cantus_ui::CantusApp;
use serde_json::{json, Value};

fn schema() -> Value {
    json!({"type": "object", "properties": {"bpm": {"type": "number"}}, "required": ["bpm"]})
}

fn set_tempo(_: &mut ToolCx<'_>, args: &Value) -> Result<EditPlan, ToolError> {
    let bpm = args["bpm"]
        .as_f64()
        .ok_or_else(|| ToolError::bad("bpm", "a number"))?;
    Ok(EditPlan {
        command: Command::SetTempo { bpm },
        // A key the history can word: the server's own sentence for an undo.
        said: Said::new("ai.said.undo_last").arg("what", bpm),
        result: json!({"bpm": bpm}),
    })
}

/// A tool that reaches the application through the host, as `cantus-ui`'s tools do.
fn where_am_i(cx: &mut ToolCx<'_>, _: &Value) -> Result<Value, ToolError> {
    let app = cx
        .host
        .as_any_mut()
        .downcast_mut::<CantusApp>()
        .ok_or_else(|| ToolError::Failed("not the application".into()))?;
    Ok(
        json!({"tracks": app.project().tracks.len(), "same_bus": app.bus.revision() == cx.bus().revision()}),
    )
}

static TEST_TOOLS: &[ToolDef] = &[
    ToolDef {
        name: "test_set_tempo",
        title: "Set the tempo",
        description: "Test.",
        class: ToolClass::Edit,
        input_schema: schema,
        output_schema: None,
        run: ToolRun::Edit(set_tempo),
    },
    ToolDef {
        name: "test_where",
        title: "Where",
        description: "Test.",
        class: ToolClass::Transport,
        input_schema: schema,
        output_schema: None,
        run: ToolRun::Host(where_am_i),
    },
];

fn tools() -> Vec<&'static ToolDef> {
    let mut v = cantus_ui::ai_tools::registry();
    v.extend(TEST_TOOLS);
    v
}

fn app() -> (CantusApp, TestClient) {
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
    let mut p = Project::new("ai");
    let t = p.make_track("Bass");
    p.tracks.push(t);
    let mut app = CantusApp::from_parts(Some(p), cantus_engine::EngineHandle::dummy(), false);
    let ctx = egui::Context::default();
    let catalog = Catalog {
        tools,
        ..cantus_ui::ai_mcp::catalog::app()
    };
    cantus_ui::ai_mcp::wire_with(&mut app, &ctx, catalog);
    let client = TestClient::legacy(app.ai.server().unwrap().clone(), "2025-11-25");
    (app, client)
}

fn call(
    app: &mut CantusApp,
    c: &mut TestClient,
    name: &str,
    args: Value,
    answer: Option<Answer>,
) -> ToolCall {
    c.call_tool(name, args, &mut || {
        cantus_ui::ai_mcp::pump(app);
        if let (Some(a), Some(agents)) = (answer, app.ai.agents_mut()) {
            let ids: Vec<u64> = agents.asks().map(|x| x.id).collect();
            for id in ids {
                agents.answer(id, a);
            }
        }
    })
}

#[test]
fn an_agents_edit_is_one_named_undo_step_the_users_undo_takes_back() {
    let (mut app, mut c) = app();
    let mut before = app.project().clone();
    let r = call(
        &mut app,
        &mut c,
        "test_set_tempo",
        json!({"bpm": 97}),
        Some(Answer::Allow),
    );
    assert!(!r.is_error, "{}", r.text);
    assert_eq!(app.project().tempo_bpm, 97.0);
    let top = app.bus.history().last().unwrap();
    assert_eq!(top.origin, Origin::Mcp);
    let rows = cantus_ui::views::history::rows(&app, 0..app.bus.history().len() + 1);
    assert_eq!(
        rows.last().unwrap().name,
        "AI: Undo its last step: 97",
        "the history says it is the agent's"
    );
    app.undo();
    let mut now = app.project().clone();
    (before.next_id, now.next_id) = (0, 0);
    assert_eq!(now, before, "the user's Undo restores the project exactly");
}

#[test]
fn the_users_settings_decide() {
    let (mut app, mut c) = app();
    // Edits allowed in Settings: no confirmation.
    app.settings.ai.allow_edits = true;
    let r = call(&mut app, &mut c, "test_set_tempo", json!({"bpm": 98}), None);
    assert!(!r.is_error, "{}", r.text);
    // A tool refused by name.
    app.settings.ai.denied_tools = vec!["test_set_tempo".into()];
    let r = call(&mut app, &mut c, "test_set_tempo", json!({"bpm": 99}), None);
    assert!(
        r.is_error && r.text.contains("does not allow"),
        "{}",
        r.text
    );
    // The AI area off: everything refused.
    app.settings.ai.enabled = false;
    let r = call(&mut app, &mut c, "list_jobs", json!({}), None);
    assert!(r.is_error && r.text.contains("switched off"), "{}", r.text);
    assert_eq!(app.project().tempo_bpm, 98.0);
}

#[test]
fn a_host_tool_reaches_the_application_and_its_very_bus() {
    let (mut app, mut c) = app();
    let r = call(&mut app, &mut c, "test_where", json!({"bpm": 1}), None);
    assert!(!r.is_error, "{}", r.text);
    let s = r.structured.unwrap();
    assert_eq!(s["tracks"], 1);
    assert_eq!(s["same_bus"], true);
}

#[test]
fn an_agent_undoes_its_own_step_through_the_applications_undo() {
    let (mut app, mut c) = app();
    app.settings.ai.allow_edits = true;
    call(
        &mut app,
        &mut c,
        "test_set_tempo",
        json!({"bpm": 101}),
        None,
    );
    let r = call(
        &mut app,
        &mut c,
        "undo_last_ai_action",
        json!({}),
        Some(Answer::Allow),
    );
    assert!(!r.is_error, "{}", r.text);
    assert_eq!(app.project().tempo_bpm, 120.0);
    assert!(app.bus.can_redo(), "the user can redo it");
}

#[test]
fn ai_settings_are_additive_in_the_settings_file() {
    let old: cantus_ui::settings::Settings = serde_json::from_str("{}").unwrap();
    assert_eq!(old.ai, cantus_ui::ai_mcp::AiSettings::default());
    assert!(
        old.ai.enabled && !old.ai.external_agents && !old.ai.allow_edits && !old.ai.allow_files
    );
    let mut s = old.clone();
    s.ai.allow_edits = true;
    s.ai.folders = vec!["C:/music".into()];
    let back: cantus_ui::settings::Settings =
        serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
    assert_eq!(back.ai, s.ai);
}

/// A tool's sentence is worded whole in the interface's language: nested phrases, bars and
/// the user's names (verbatim, in the language's quotes).
#[test]
fn a_tools_sentence_is_worded_whole_in_each_language() {
    use cantus_mcp::AgentHost;
    let (app, _c) = app();
    let said = Said::new("ai.history.step")
        .nested("what", Said::new("ai.said.undo_last").bars("what", 1, 4));
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::Es);
    assert_eq!(app.say(&said), "IA: Deshacer su último paso: compases 1–4");
    let named = Said::new("ai.history.step").name("what", "Piano");
    assert_eq!(app.say(&named), "IA: «Piano»");
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
    assert_eq!(app.say(&said), "AI: Undo its last step: bars 1–4");
    assert_eq!(app.say(&named), "AI: “Piano”");
}

/// The integrated review: a kit an agent adds — a new Pulsus track, or an instrument
/// swapped for Pulsus — has its pad names in the interface's language, as one the user
/// adds; undone, nothing of it is left.
#[test]
fn an_agents_kit_speaks_the_interfaces_language() {
    use cantus_ui::i18n::{set_thread_locale, t, Locale};
    let (mut app, mut c) = app();
    set_thread_locale(Locale::Es);
    app.settings.ai.allow_edits = true;
    let before = app.bus.project().clone();
    let r = call(
        &mut app,
        &mut c,
        "add_track",
        json!({"kind": "pulsus"}),
        None,
    );
    assert!(!r.is_error, "{}", r.text);
    let kick = t("pad.name.kick");
    assert_ne!(kick, "Kick", "a Spanish name to look for");
    let pads = |app: &CantusApp, track: usize| -> Vec<String> {
        match &app.bus.project().tracks[track].instrument {
            cantus_core::InstrumentSpec::Pulsus(kit) => {
                kit.pads.iter().map(|p| p.name.clone()).collect()
            }
            other => panic!("not a kit: {other:?}"),
        }
    };
    let added = app.bus.project().tracks.len() - 1;
    assert!(pads(&app, added).contains(&kick), "{:?}", pads(&app, added));
    assert!(!pads(&app, added).iter().any(|n| n == "Kick"));
    // An instrument swapped for Pulsus (destructive: allowed on the card).
    let first = app.bus.project().tracks[0].id.0;
    let r = call(
        &mut app,
        &mut c,
        "set_instrument",
        json!({"track_id": first, "kind": "pulsus"}),
        Some(Answer::Allow),
    );
    assert!(!r.is_error, "{}", r.text);
    assert!(pads(&app, 0).contains(&kick), "{:?}", pads(&app, 0));
    // Undone both: the project as it was.
    app.bus.undo();
    app.bus.undo();
    assert_eq!(
        app.bus.project().tracks.len(),
        before.tracks.len(),
        "the kit's track undone"
    );
    set_thread_locale(Locale::En);
}

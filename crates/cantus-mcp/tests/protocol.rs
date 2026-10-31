//! Protocol conformance, per version, written from the specification's pages
//! (modelcontextprotocol.io/specification/{2025-06-18,2025-11-25,2026-07-28}: basic,
//! lifecycle/versioning, server/tools, resources, prompts, discover), through the
//! in-process client — the same `Server::handle` every transport calls.

mod common;

use cantus_mcp::jsonrpc;
use cantus_mcp::server::{tool_result, SUPPORTED_VERSIONS};
use cantus_mcp::{Session, ToolAnswer};
use common::Rig;
use serde_json::{json, Value};

const LEGACY: [&str; 2] = ["2025-06-18", "2025-11-25"];

fn code(e: &Value) -> i64 {
    e["code"].as_i64().unwrap()
}

#[test]
fn a_handshake_agrees_each_legacy_version_as_asked() {
    let rig = Rig::new();
    for v in LEGACY {
        let mut c = cantus_mcp::client::TestClient::raw(rig.server.clone());
        let r = c
            .request(
                "initialize",
                json!({"protocolVersion": v, "capabilities": {}, "clientInfo": {"name": "x", "version": "1"}}),
                &mut || {},
            )
            .unwrap();
        assert_eq!(r["protocolVersion"], v);
        assert_eq!(r["serverInfo"]["name"], "cantus");
        assert_eq!(r["serverInfo"]["title"], "Cantus");
        assert!(r["instructions"].as_str().unwrap().contains("undo"));
        assert_eq!(r["capabilities"]["tools"]["listChanged"], false);
        assert!(r["capabilities"]["resources"].is_object());
        assert!(r["capabilities"]["prompts"].is_object());
        assert!(
            r.get("resultType").is_none(),
            "a legacy result has no resultType"
        );
        assert!(c
            .session()
            .id
            .as_deref()
            .is_some_and(|s| s.starts_with("session-")));
    }
}

#[test]
fn a_handshake_for_another_version_gets_the_newest_legacy_one() {
    let rig = Rig::new();
    for asked in ["2024-11-05", "2026-07-28", "2099-01-01"] {
        let mut c = cantus_mcp::client::TestClient::raw(rig.server.clone());
        let r = c
            .request(
                "initialize",
                json!({"protocolVersion": asked, "capabilities": {}}),
                &mut || {},
            )
            .unwrap();
        assert_eq!(r["protocolVersion"], "2025-11-25", "{asked}");
    }
}

#[test]
fn before_a_handshake_only_ping_and_modern_requests_are_served() {
    let rig = Rig::new();
    let mut c = cantus_mcp::client::TestClient::raw(rig.server.clone());
    assert_eq!(c.request("ping", json!({}), &mut || {}).unwrap(), json!({}));
    let e = c.request("tools/list", json!({}), &mut || {}).unwrap_err();
    assert_eq!(code(&e), jsonrpc::INVALID_PARAMS);
    assert!(e["message"].as_str().unwrap().contains("protocolVersion"));
}

#[test]
fn tools_list_is_deterministic_and_complete() {
    let mut rig = Rig::new();
    for v in LEGACY {
        let mut c = rig.legacy(v);
        let a = c.request("tools/list", json!({}), &mut || {}).unwrap();
        let b = c.request("tools/list", json!({}), &mut || {}).unwrap();
        assert_eq!(a, b, "the same list, in the same order");
        let names: Vec<&str> = a["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap())
            .collect();
        let expected: Vec<&str> = common::tools().iter().map(|t| t.name).collect();
        assert_eq!(names, expected, "the registry's order");
        for t in a["tools"].as_array().unwrap() {
            assert!(t["title"].is_string() && t["description"].is_string());
            assert_eq!(t["inputSchema"]["type"], "object");
            assert!(t["annotations"]["readOnlyHint"].is_boolean());
        }
        assert!(a.get("nextCursor").is_none(), "one page");
    }
    let mut m = rig.modern();
    let r = m.request("tools/list", json!({}), &mut || {}).unwrap();
    assert_eq!(r["resultType"], "complete");
    rig.pump();
}

#[test]
fn a_tool_call_answers_text_and_structure_and_errors_are_tool_errors() {
    let mut rig = Rig::new();
    for v in LEGACY {
        let mut c = rig.legacy(v);
        let r = rig.call(&mut c, "get_tempo", json!({}), None);
        assert!(!r.is_error, "{}", r.text);
        let s = r.structured.clone().unwrap();
        assert_eq!(s["bpm"], 120.0);
        let text: Value = serde_json::from_str(&r.text).unwrap();
        assert_eq!(text, s, "the text block is the structure, serialised");
        // A bad argument is the tool's error, which the model can correct.
        let r = rig.call(
            &mut c,
            "set_tempo",
            json!({"bpm": 5}),
            Some(cantus_mcp::Answer::Allow),
        );
        assert!(r.is_error);
        assert!(r.text.contains("bpm"), "{}", r.text);
        assert!(r.structured.is_none());
    }
}

#[test]
fn unknown_tools_methods_and_malformed_calls_are_protocol_errors() {
    let mut rig = Rig::new();
    let mut c = rig.legacy("2025-11-25");
    let agents = &mut rig.agents;
    let host = &mut rig.host;
    let mut pump = || agents.pump(host);
    let e = c
        .request("tools/call", json!({"name": "nope"}), &mut pump)
        .unwrap_err();
    assert_eq!(code(&e), jsonrpc::INVALID_PARAMS);
    assert!(e["message"].as_str().unwrap().contains("nope"));
    let e = c
        .request(
            "tools/call",
            json!({"name": "get_tempo", "arguments": [1]}),
            &mut pump,
        )
        .unwrap_err();
    assert_eq!(code(&e), jsonrpc::INVALID_PARAMS);
    let e = c.request("tools/call", json!({}), &mut pump).unwrap_err();
    assert_eq!(code(&e), jsonrpc::INVALID_PARAMS);
    let e = c
        .request("sampling/createMessage", json!({}), &mut pump)
        .unwrap_err();
    assert_eq!(code(&e), jsonrpc::METHOD_NOT_FOUND);
    let e = c
        .request("logging/setLevel", json!({"level": "info"}), &mut pump)
        .unwrap_err();
    assert_eq!(
        code(&e),
        jsonrpc::METHOD_NOT_FOUND,
        "logging is not offered"
    );
    // Notifications are never answered, known or not.
    c.notify("notifications/cancelled", json!({"requestId": 1}));
    c.notify("notifications/whatever", json!({}));
}

#[test]
fn malformed_bytes_are_refused_with_their_codes() {
    let rig = Rig::new();
    let mut s = Session::new();
    let parse = |b: &[u8], s: &mut Session| -> Value {
        serde_json::from_slice(&rig.server.handle_bytes(s, b).unwrap()).unwrap()
    };
    assert_eq!(
        code(&parse(b"{not json", &mut s)["error"]),
        jsonrpc::PARSE_ERROR
    );
    let batch = br#"[{"jsonrpc":"2.0","id":1,"method":"ping"}]"#;
    assert_eq!(
        code(&parse(batch, &mut s)["error"]),
        jsonrpc::INVALID_REQUEST
    );
    let big = vec![b' '; rig.server.limits().max_request_bytes + 1];
    let e = parse(&big, &mut s);
    assert_eq!(code(&e["error"]), jsonrpc::INVALID_REQUEST);
    assert!(e["error"]["message"].as_str().unwrap().contains("limit"));
    assert_eq!(e["id"], Value::Null);
    let note = br#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#;
    assert!(rig.server.handle_bytes(&mut s, note).is_none());
    assert!(s.initialized);
    let answer = br#"{"jsonrpc":"2.0","id":9,"result":{}}"#;
    assert!(
        rig.server.handle_bytes(&mut s, answer).is_none(),
        "a response is not answered"
    );
}

#[test]
fn the_stateless_generation_needs_its_meta_and_says_complete() {
    let mut rig = Rig::new();
    let mut m = rig.modern();
    let d = m.request("server/discover", json!({}), &mut || {}).unwrap();
    assert_eq!(d["resultType"], "complete");
    assert_eq!(d["supportedVersions"], json!(SUPPORTED_VERSIONS));
    assert!(d["capabilities"]["tools"].is_object());
    assert_eq!(
        d["_meta"]["io.modelcontextprotocol/serverInfo"]["name"],
        "cantus"
    );
    assert!(d["instructions"].is_string());
    assert!(m.session().id.is_none(), "no session is opened");
    let r = rig.call(&mut m, "get_tempo", json!({}), None);
    assert!(!r.is_error);
    assert_eq!(r.raw["resultType"], "complete");
    assert_eq!(
        r.raw["_meta"]["io.modelcontextprotocol/serverInfo"]["version"],
        "0.0.0-test"
    );

    // A request without clientCapabilities is malformed: -32602.
    let mut raw = cantus_mcp::client::TestClient::raw(rig.server.clone());
    let e = raw
        .request(
            "tools/list",
            json!({"_meta": {"io.modelcontextprotocol/protocolVersion": "2026-07-28"}}),
            &mut || {},
        )
        .unwrap_err();
    assert_eq!(code(&e), jsonrpc::INVALID_PARAMS);
    // A version this server does not speak: -32022 with what it speaks.
    let e = raw
        .request(
            "tools/list",
            json!({"_meta": {"io.modelcontextprotocol/protocolVersion": "1900-01-01",
                             "io.modelcontextprotocol/clientCapabilities": {}}}),
            &mut || {},
        )
        .unwrap_err();
    assert_eq!(code(&e), jsonrpc::UNSUPPORTED_PROTOCOL_VERSION);
    assert_eq!(e["data"]["supported"], json!(SUPPORTED_VERSIONS));
    assert_eq!(e["data"]["requested"], "1900-01-01");
    // A legacy version in _meta without its handshake is not spoken statelessly.
    let e = raw
        .request(
            "ping",
            json!({"_meta": {"io.modelcontextprotocol/protocolVersion": "2025-11-25",
                             "io.modelcontextprotocol/clientCapabilities": {}}}),
            &mut || {},
        )
        .unwrap_err();
    assert_eq!(code(&e), jsonrpc::UNSUPPORTED_PROTOCOL_VERSION);
}

#[test]
fn structured_content_is_any_value_only_in_the_stateless_generation() {
    let arr = ToolAnswer::Done(json!([1, 2]));
    assert!(tool_result(arr.clone(), false)
        .get("structuredContent")
        .is_none());
    assert_eq!(tool_result(arr, true)["structuredContent"], json!([1, 2]));
    let err = tool_result(ToolAnswer::Error("no".into()), true);
    assert_eq!(err["isError"], true);
    assert_eq!(err["content"][0], json!({"type": "text", "text": "no"}));
}

#[test]
fn resources_list_read_and_say_when_missing_in_each_generation() {
    let mut rig = Rig::new();
    let mut c = rig.legacy("2025-11-25");
    let mut m = rig.modern();
    let agents = &mut rig.agents;
    let host = &mut rig.host;
    let mut pump = || agents.pump(host);
    let l = c.request("resources/list", json!({}), &mut pump).unwrap();
    assert_eq!(l["resources"][0]["uri"], "cantus://project");
    assert_eq!(l["resources"][0]["mimeType"], "application/json");
    let t = c
        .request("resources/templates/list", json!({}), &mut pump)
        .unwrap();
    assert_eq!(
        t["resourceTemplates"][0]["uriTemplate"],
        "cantus://tracks/{id}"
    );
    let r = c
        .request(
            "resources/read",
            json!({"uri": "cantus://project"}),
            &mut pump,
        )
        .unwrap();
    assert_eq!(
        r["contents"][0]["text"],
        json!({"name": "test"}).to_string()
    );
    let w = c
        .request(
            "resources/read",
            json!({"uri": "cantus://renders/a.wav"}),
            &mut pump,
        )
        .unwrap();
    assert_eq!(w["contents"][0]["blob"], "UklGRg==");
    assert_eq!(w["contents"][0]["mimeType"], "audio/wav");
    let e = c
        .request("resources/read", json!({"uri": "cantus://nope"}), &mut pump)
        .unwrap_err();
    assert_eq!(code(&e), jsonrpc::RESOURCE_NOT_FOUND_LEGACY);
    let e = c
        .request(
            "resources/read",
            json!({"uri": "cantus://tracks/999999"}),
            &mut pump,
        )
        .unwrap_err();
    assert_eq!(
        code(&e),
        jsonrpc::RESOURCE_NOT_FOUND_LEGACY,
        "a template's missing item too"
    );
    let e = m
        .request("resources/read", json!({"uri": "cantus://nope"}), &mut pump)
        .unwrap_err();
    assert_eq!(code(&e), jsonrpc::INVALID_PARAMS, "2026-07-28 says -32602");
}

#[test]
fn a_template_reads_the_item_it_names() {
    let mut rig = Rig::new();
    let id = rig.host.first_track().0;
    let mut c = rig.legacy("2025-06-18");
    let agents = &mut rig.agents;
    let host = &mut rig.host;
    let uri = format!("cantus://tracks/{id}");
    let r = c
        .request("resources/read", json!({"uri": uri}), &mut || {
            agents.pump(host)
        })
        .unwrap();
    assert_eq!(r["contents"][0]["uri"], uri);
    assert_eq!(
        r["contents"][0]["text"],
        json!({"name": "Bass"}).to_string()
    );
}

#[test]
fn prompts_are_listed_and_worded_in_the_interface_language() {
    let mut rig = Rig::new();
    let mut c = rig.legacy("2025-11-25");
    let agents = &mut rig.agents;
    let host = &mut rig.host;
    let mut pump = || agents.pump(host);
    let l = c.request("prompts/list", json!({}), &mut pump).unwrap();
    let p = &l["prompts"][0];
    assert_eq!(p["name"], "explain_mix");
    assert_eq!(
        p["title"], "ai.prompt.explain.title",
        "localised by the host"
    );
    assert_eq!(p["arguments"][0]["required"], true);
    let g = c
        .request(
            "prompts/get",
            json!({"name": "explain_mix", "arguments": {"track": "Bass"}}),
            &mut pump,
        )
        .unwrap();
    assert_eq!(g["messages"][0]["role"], "user");
    assert_eq!(
        g["messages"][0]["content"]["text"],
        "ai.prompt.explain.text track=Bass"
    );
    let e = c
        .request("prompts/get", json!({"name": "explain_mix"}), &mut pump)
        .unwrap_err();
    assert_eq!(
        code(&e),
        jsonrpc::INVALID_PARAMS,
        "a required argument missing"
    );
    let e = c
        .request("prompts/get", json!({"name": "nope"}), &mut pump)
        .unwrap_err();
    assert_eq!(code(&e), jsonrpc::INVALID_PARAMS);
}

#[test]
fn a_client_name_is_shown_clean_and_never_trusted() {
    let mut rig = Rig::new();
    let mut c = cantus_mcp::client::TestClient::raw(rig.server.clone());
    c.request(
        "initialize",
        json!({"protocolVersion": "2025-11-25", "capabilities": {},
               "clientInfo": {"name": "Evil\nClient\u{7}", "version": "1"}}),
        &mut || {},
    )
    .unwrap();
    let r = rig.call(&mut c, "get_tempo", json!({}), None);
    assert!(!r.is_error);
    let caller = rig.agents.activity().last().unwrap().caller.clone();
    assert_eq!(caller.name, "EvilClient");
    assert!(
        caller.key.starts_with("session-"),
        "the key is the session, not the name"
    );
}

/// Review 3 #1: every output schema's root is an object, as 2025-06-18 and 2025-11-25 ask
/// (`Tool.outputSchema: { type: "object", … }`); every input schema's too.
#[test]
fn every_schema_in_tools_list_has_an_object_root() {
    let rig = Rig::new();
    let mut c = rig.legacy("2025-11-25");
    let list = c.request("tools/list", json!({}), &mut || {}).unwrap();
    for t in list["tools"].as_array().unwrap() {
        assert_eq!(t["inputSchema"]["type"], "object", "{}", t["name"]);
        if let Some(out) = t.get("outputSchema") {
            assert_eq!(out["type"], "object", "{}", t["name"]);
        }
    }
}

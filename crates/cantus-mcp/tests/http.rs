//! The Streamable HTTP door end to end, on a real socket: the token, `Host` and `Origin`,
//! sessions, the version header, the limits, and a call that edits the project.

mod common;

use cantus_mcp::http::{is_loopback, origin_allowed, HttpConfig, HttpDoor};
use cantus_mcp::http_client::Client;
use cantus_mcp::http_wire::Message;
use common::{Host, Rig};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

/// A door with its pump turning on a thread; `finish` stops it and gives the host back.
struct Open {
    door: HttpDoor,
    stop: Arc<AtomicBool>,
    pump: std::thread::JoinHandle<(Host, cantus_mcp::Agents)>,
}

impl Open {
    fn new(configure: impl FnOnce(&mut Rig)) -> Open {
        let mut rig = Rig::new();
        rig.host.policy.external_agents = true;
        configure(&mut rig);
        let door = HttpDoor::open(
            rig.server.clone(),
            HttpConfig {
                port: 0,
                tokens: vec![("default".into(), TOKEN.into())],
                max_connections: 8,
                ..HttpConfig::default()
            },
        )
        .unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let s = stop.clone();
        let Rig {
            mut agents,
            mut host,
            ..
        } = rig;
        let pump = std::thread::spawn(move || {
            while !s.load(Ordering::SeqCst) {
                agents.pump(&mut host);
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            (host, agents)
        });
        Open { door, stop, pump }
    }

    fn client(&self) -> Client {
        Client::new(self.door.addr())
    }

    fn finish(self) -> Host {
        self.finish_all().0
    }

    fn finish_all(self) -> (Host, cantus_mcp::Agents) {
        self.stop.store(true, Ordering::SeqCst);
        let both = self.pump.join().unwrap();
        self.door.close();
        both
    }
}

fn post(c: &mut Client, headers: &[(&str, &str)], body: Value) -> Message {
    let auth = format!("Bearer {TOKEN}");
    let mut h = vec![
        ("Authorization", auth.as_str()),
        ("Content-Type", "application/json"),
    ];
    h.extend_from_slice(headers);
    c.send("POST", &h, body.to_string().as_bytes()).unwrap()
}

fn json_of(m: &Message) -> Value {
    serde_json::from_slice(&m.body).unwrap()
}

fn init() -> Value {
    json!({"jsonrpc": "2.0", "id": 1, "method": "initialize",
           "params": {"protocolVersion": "2025-11-25", "capabilities": {}, "clientInfo": {"name": "curl"}}})
}

#[test]
fn the_token_is_required_and_compared_whole() {
    let open = Open::new(|_| {});
    let mut c = open.client();
    let m = c
        .send(
            "POST",
            &[("Content-Type", "application/json")],
            init().to_string().as_bytes(),
        )
        .unwrap();
    assert_eq!(m.start.0, "401");
    assert!(m.header("www-authenticate").unwrap().starts_with("Bearer"));
    let wrong = format!("Bearer {}x", &TOKEN[..TOKEN.len() - 1]);
    let m = c
        .send(
            "POST",
            &[
                ("Authorization", &wrong),
                ("Content-Type", "application/json"),
            ],
            b"{}",
        )
        .unwrap();
    assert_eq!(m.start.0, "401");
    let m = post(&mut c, &[], init());
    assert_eq!(m.start.0, "200");
    open.finish();
}

#[test]
fn host_and_origin_guard_against_web_pages() {
    let open = Open::new(|_| {});
    let mut c = open.client();
    let m = post(&mut c, &[("Origin", "https://evil.example")], init());
    assert_eq!(m.start.0, "403");
    let m = post(&mut c, &[("Origin", "null")], init());
    assert_eq!(m.start.0, "403");
    let m = post(&mut c, &[("Origin", "http://localhost:5173")], init());
    assert_eq!(m.start.0, "200", "a page on the loopback may");
    // A rebound name: the request's Host is not the loopback.
    let auth = format!("Bearer {TOKEN}");
    let m = c
        .send(
            "POST",
            &[
                ("Host", "evil.example:39571"),
                ("Authorization", &auth),
                ("Content-Type", "application/json"),
            ],
            init().to_string().as_bytes(),
        )
        .unwrap();
    assert_eq!(m.start.0, "403");
    open.finish();
}

#[test]
fn sessions_open_answer_end_and_are_404_once_ended() {
    let open = Open::new(|_| {});
    let mut c = open.client();
    let m = post(&mut c, &[], init());
    let sid = m.header("mcp-session-id").unwrap().to_string();
    assert!(sid.starts_with("session-"));
    assert_eq!(json_of(&m)["result"]["protocolVersion"], "2025-11-25");
    let note = json!({"jsonrpc": "2.0", "method": "notifications/initialized"});
    let m = post(&mut c, &[("Mcp-Session-Id", &sid)], note);
    assert_eq!(m.start.0, "202");
    assert!(m.body.is_empty());
    let list = json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"});
    let m = post(
        &mut c,
        &[
            ("Mcp-Session-Id", &sid),
            ("MCP-Protocol-Version", "2025-11-25"),
        ],
        list.clone(),
    );
    assert_eq!(m.start.0, "200");
    assert!(json_of(&m)["result"]["tools"].as_array().unwrap().len() > 3);
    // No session and no version: 400, so a client probing falls back to initialize.
    let m = post(&mut c, &[], list.clone());
    assert_eq!(m.start.0, "400");
    let m = post(&mut c, &[("Mcp-Session-Id", "session-nope")], list.clone());
    assert_eq!(m.start.0, "404");
    let auth = format!("Bearer {TOKEN}");
    let m = c
        .send(
            "DELETE",
            &[("Authorization", &auth), ("Mcp-Session-Id", &sid)],
            b"",
        )
        .unwrap();
    assert_eq!(m.start.0, "204");
    let m = post(&mut c, &[("Mcp-Session-Id", &sid)], list);
    assert_eq!(m.start.0, "404");
    assert_eq!(open.door.take_ended(), vec![sid]);
    open.finish();
}

#[test]
fn the_user_ending_a_session_turns_its_next_request_away() {
    let open = Open::new(|_| {});
    let mut c = open.client();
    let sid = post(&mut c, &[], init())
        .header("mcp-session-id")
        .unwrap()
        .to_string();
    assert!(open.door.end_session(&sid));
    let list = json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"});
    assert_eq!(
        post(&mut c, &[("Mcp-Session-Id", &sid)], list).start.0,
        "404"
    );
    open.finish();
}

#[test]
fn methods_types_limits_and_versions_are_checked() {
    let open = Open::new(|_| {});
    let mut c = open.client();
    let auth = format!("Bearer {TOKEN}");
    let m = c.send("GET", &[("Authorization", &auth)], b"").unwrap();
    assert_eq!(m.start.0, "405");
    assert!(m.header("allow").unwrap().contains("POST"));
    let m = c
        .send(
            "POST",
            &[("Authorization", &auth), ("Content-Type", "text/plain")],
            b"{}",
        )
        .unwrap();
    assert_eq!(m.start.0, "415");
    let m = post(&mut c, &[("MCP-Protocol-Version", "1999-01-01")], init());
    assert_eq!(m.start.0, "400");
    assert_eq!(json_of(&m)["error"]["code"], -32022);
    let modern = |v: &str| {
        json!({"jsonrpc": "2.0", "id": 3, "method": "tools/list", "params": {"_meta": {
            "io.modelcontextprotocol/protocolVersion": v,
            "io.modelcontextprotocol/clientCapabilities": {}}}})
    };
    let m = post(
        &mut c,
        &[("MCP-Protocol-Version", "2025-11-25")],
        modern("2026-07-28"),
    );
    assert_eq!(m.start.0, "400");
    assert_eq!(
        json_of(&m)["error"]["code"],
        -32020,
        "header and _meta disagree"
    );
    let m = post(
        &mut c,
        &[("MCP-Protocol-Version", "2026-07-28")],
        modern("2026-07-28"),
    );
    assert_eq!(m.start.0, "200");
    assert_eq!(json_of(&m)["result"]["resultType"], "complete");
    assert!(
        m.header("mcp-session-id").is_none(),
        "stateless: no session"
    );
    // A body over the limit is refused from its head, unread.
    use std::io::{BufReader, Write};
    let mut raw = std::net::TcpStream::connect(open.door.addr()).unwrap();
    write!(
        raw,
        "POST /mcp HTTP/1.1\r\nHost: 127.0.0.1\r\nAuthorization: {auth}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n",
        (4 << 20) + 1
    )
    .unwrap();
    let m = cantus_mcp::http_wire::read(&mut BufReader::new(raw), 1 << 20, None).unwrap();
    assert_eq!(m.start.0, "413");
    open.finish();
}

#[test]
fn an_edit_through_the_door_is_one_undo_step() {
    let open = Open::new(|rig| rig.host.policy.allow_edits = true);
    let mut c = open.client();
    let call = |v: &str, id: u64, args: Value| {
        json!({"jsonrpc": "2.0", "id": id, "method": "tools/call", "params": {
            "name": "set_tempo", "arguments": args,
            "_meta": {"io.modelcontextprotocol/protocolVersion": v,
                      "io.modelcontextprotocol/clientCapabilities": {},
                      "io.modelcontextprotocol/clientInfo": {"name": "agent-a"}}}})
    };
    let m = post(
        &mut c,
        &[("MCP-Protocol-Version", "2026-07-28")],
        call("2026-07-28", 1, json!({"bpm": 93})),
    );
    let r = json_of(&m);
    assert_eq!(r["result"]["isError"], false, "{r}");
    assert_eq!(r["result"]["structuredContent"]["bpm"], 93.0);
    let mut host = open.finish();
    assert_eq!(host.bus.project().tempo_bpm, 93.0);
    assert_eq!(host.bus.history().len(), 1);
    host.bus.undo();
    assert_eq!(host.bus.project().tempo_bpm, 120.0);
}

#[test]
fn external_agents_switched_off_are_refused_at_the_door() {
    let open = Open::new(|rig| rig.host.policy.external_agents = false);
    let mut c = open.client();
    let sid = post(&mut c, &[], init())
        .header("mcp-session-id")
        .unwrap()
        .to_string();
    let call = json!({"jsonrpc": "2.0", "id": 2, "method": "tools/call",
                      "params": {"name": "get_tempo", "arguments": {}}});
    let r = json_of(&post(&mut c, &[("Mcp-Session-Id", &sid)], call));
    assert_eq!(r["result"]["isError"], true);
    assert!(r["result"]["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("external agents"));
    open.finish();
}

#[test]
fn a_closed_door_listens_no_more() {
    let open = Open::new(|_| {});
    let addr = open.door.addr();
    open.finish();
    assert!(
        std::net::TcpStream::connect_timeout(&addr, std::time::Duration::from_millis(300)).is_err()
    );
}

#[test]
fn loopback_names_and_origins() {
    for h in [
        "127.0.0.1",
        "127.0.0.1:39571",
        "localhost",
        "LOCALHOST:1",
        "[::1]:80",
        "[::1]",
    ] {
        assert!(is_loopback(h), "{h}");
    }
    for h in [
        "evil.example",
        "127.0.0.2",
        "localhost.evil.example",
        "10.0.0.1:80",
    ] {
        assert!(!is_loopback(h), "{h}");
    }
    assert!(origin_allowed("http://localhost:3000"));
    assert!(origin_allowed("https://127.0.0.1"));
    for o in [
        "null",
        "file://",
        "http://evil.example",
        "http://localhost@evil.example",
        "ws://localhost",
        "http://localhost/x",
    ] {
        assert!(!origin_allowed(o), "{o}");
    }
}

/// Review 2 #11: each client's token, and each bridge process, is an agent of its own.
#[test]
fn each_client_token_and_each_bridge_is_its_own_agent() {
    const CURSOR: &str = "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210";
    let open = Open::new(|_| {});
    open.door.set_tokens(vec![
        ("default".into(), TOKEN.into()),
        ("cursor".into(), CURSOR.into()),
    ]);
    let ping = |id: u64| {
        json!({"jsonrpc": "2.0", "id": id, "method": "tools/call", "params": {
            "name": "get_tempo", "arguments": {},
            "_meta": {"io.modelcontextprotocol/protocolVersion": "2026-07-28",
                      "io.modelcontextprotocol/clientCapabilities": {}}}})
    };
    let mut c = open.client();
    let mut send = |token: &str, instance: Option<&str>, id: u64| {
        let auth = format!("Bearer {token}");
        let mut h = vec![
            ("Authorization", auth.as_str()),
            ("Content-Type", "application/json"),
        ];
        if let Some(i) = instance {
            h.push(("Mcp-Cantus-Instance", i));
        }
        let m = c.send("POST", &h, ping(id).to_string().as_bytes()).unwrap();
        assert_eq!(m.start.0, "200");
    };
    send(TOKEN, None, 1);
    send(CURSOR, None, 2);
    send(TOKEN, Some("bridge-a"), 3);
    send(TOKEN, Some("bridge-b"), 4);
    send(TOKEN, Some("bad instance!"), 5);
    let (_, agents) = open.finish_all();
    let keys: Vec<String> = agents.activity().map(|a| a.caller.key.clone()).collect();
    assert_eq!(
        keys,
        [
            "http:default",
            "http:cursor",
            "http:default#bridge-a",
            "http:default#bridge-b",
            "http:default"
        ]
    );
}

/// Review 3 #2: a request that would have panicked a connection (a chunk size of 2⁶⁴−1)
/// is refused; however many come, the door's slots come back and it still serves.
#[test]
fn malformed_requests_never_use_up_the_door() {
    use std::io::{Read, Write};
    let open = Open::new(|_| {});
    for _ in 0..20 {
        let mut raw = std::net::TcpStream::connect(open.door.addr()).unwrap();
        let _ = raw.write_all(
            b"POST /mcp HTTP/1.1\r\nHost: 127.0.0.1\r\nTransfer-Encoding: chunked\r\n\r\n2\r\n{}\r\nffffffffffffffff\r\nx",
        );
        let mut answer = Vec::new();
        let _ = raw.read_to_end(&mut answer);
    }
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while open.door.connections() > 0 {
        assert!(std::time::Instant::now() < deadline, "slots given back");
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let mut c = open.client();
    assert_eq!(post(&mut c, &[], init()).start.0, "200");
    open.finish();
}

/// Review 3 #13: past 32 bridge instances per token, more are the token's own agent.
#[test]
fn bridge_instances_per_token_are_bounded() {
    let open = Open::new(|_| {});
    let mut c = open.client();
    let ping = json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {
        "name": "get_tempo", "arguments": {},
        "_meta": {"io.modelcontextprotocol/protocolVersion": "2026-07-28",
                  "io.modelcontextprotocol/clientCapabilities": {}}}});
    for i in 0..33 {
        let auth = format!("Bearer {TOKEN}");
        let inst = format!("bridge-{i}");
        let h = [
            ("Authorization", auth.as_str()),
            ("Content-Type", "application/json"),
            ("Mcp-Cantus-Instance", inst.as_str()),
        ];
        assert_eq!(
            c.send("POST", &h, ping.to_string().as_bytes())
                .unwrap()
                .start
                .0,
            "200"
        );
    }
    let (_, agents) = open.finish_all();
    let keys: std::collections::BTreeSet<String> =
        agents.activity().map(|a| a.caller.key.clone()).collect();
    assert_eq!(keys.len(), 33);
    assert!(keys.contains("http:default"), "the 33rd is the token's own");
    assert!(!keys.contains("http:default#bridge-32"));
}

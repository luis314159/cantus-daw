//! The third review's findings (round 34, mcp: the HTTP door, the bridge and the second fix
//! round), each held by a test that fails without its fix. (#1 and #2 are in protocol.rs,
//! http.rs and http_wire.rs; #8 in cantus-rt's private.rs and home.rs; #11 in cantus-ai;
//! #15 in ids.rs.)

mod common;

use cantus_core::{Command, Origin};
use cantus_mcp::client::TestClient;
use cantus_mcp::http::{HttpConfig, HttpDoor};
use cantus_mcp::http_client::Client;
use cantus_mcp::{Answer, Outcome, Session};
use common::Rig;
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::time::{Duration, Instant};

const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const OTHER: &str = "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210";

fn door(rig: &Rig, head: Duration) -> HttpDoor {
    HttpDoor::open(
        rig.server.clone(),
        HttpConfig {
            port: 0,
            tokens: vec![
                ("default".into(), TOKEN.into()),
                ("cursor".into(), OTHER.into()),
            ],
            max_connections: 4,
            head_time: head,
            body_time: head,
            ..HttpConfig::default()
        },
    )
    .unwrap()
}

fn wait_until(what: &str, mut f: impl FnMut() -> bool) {
    let end = Instant::now() + Duration::from_secs(10);
    while !f() {
        assert!(Instant::now() < end, "{what}");
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// #3: a client dripping its head is cut off at the head's time; an unauthenticated
/// request is answered before its body is read.
#[test]
fn a_slow_drip_holds_no_slot_and_no_body_is_read_unauthenticated() {
    let rig = Rig::new();
    // The head's time short, the body's long: an answer before the body's time is one
    // that never waited for the body, however slow the machine.
    let d = HttpDoor::open(
        rig.server.clone(),
        HttpConfig {
            port: 0,
            tokens: vec![("default".into(), TOKEN.into())],
            max_connections: 4,
            head_time: Duration::from_millis(300),
            body_time: Duration::from_secs(20),
            ..HttpConfig::default()
        },
    )
    .unwrap();
    let mut drip = std::net::TcpStream::connect(d.addr()).unwrap();
    drip.write_all(b"POST /mcp HTTP/1.1\r\nHo").unwrap();
    wait_until("the slow client is cut off", || d.connections() == 0);
    // A huge announced body, no token: 401 at once, the body never waited for.
    let mut raw = std::net::TcpStream::connect(d.addr()).unwrap();
    raw.write_all(b"POST /mcp HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: 1000000\r\n\r\n")
        .unwrap();
    let started = Instant::now();
    let mut answer = Vec::new();
    raw.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    let _ = raw.read_to_end(&mut answer);
    assert!(String::from_utf8_lossy(&answer).starts_with("HTTP/1.1 401"));
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "not after the body's time"
    );
}

/// #3: a call waiting on the card whose client hangs up frees its slot and its card.
#[test]
fn a_client_that_hangs_up_takes_its_card_and_slot_with_it() {
    let mut rig = Rig::new();
    rig.host.policy.external_agents = true;
    let d = door(&rig, Duration::from_secs(10));
    let addr = d.addr();
    let t = std::thread::spawn(move || {
        let mut s = std::net::TcpStream::connect(addr).unwrap();
        let body = json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {
            "name": "set_tempo", "arguments": {"bpm": 90},
            "_meta": {"io.modelcontextprotocol/protocolVersion": "2026-07-28",
                      "io.modelcontextprotocol/clientCapabilities": {}}}})
        .to_string();
        write!(
            s,
            "POST /mcp HTTP/1.1\r\nHost: 127.0.0.1\r\nAuthorization: Bearer {TOKEN}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        )
        .unwrap();
        s
    });
    let s = t.join().unwrap();
    wait_until("the call is on the card", || {
        rig.pump();
        rig.agents.asks().count() == 1
    });
    drop(s);
    wait_until("the card and the slot go", || {
        rig.pump();
        rig.agents.asks().count() == 0 && d.connections() == 0
    });
    assert!(rig.agents.activity().any(|a| a.outcome == Outcome::Dropped));
    assert_eq!(rig.host.bus.project().tempo_bpm, 120.0);
}

/// #4: edits allowed when a job started but not when it ends: its edit is asked.
#[test]
fn a_jobs_edit_is_asked_when_edits_stopped_being_allowed() {
    let mut rig = Rig::new();
    rig.host.policy.allow_edits = true;
    rig.agents.config.inline_job_wait = Duration::ZERO;
    let mut c = rig.legacy("2025-11-25");
    let r = rig.call(&mut c, "measure_tempo", json!({"bpm": 75, "ms": 100}), None);
    let job = r.structured.unwrap()["job"].as_str().unwrap().to_string();
    rig.host.policy.allow_edits = false;
    let w = rig.call(&mut c, "wait_job", json!({"job": job}), Some(Answer::Deny));
    assert!(w.is_error && w.text.contains("said no"), "{}", w.text);
    assert_eq!(rig.host.bus.project().tempo_bpm, 120.0);
}

/// #5: cancelling a job withdraws its edit's card.
#[test]
fn cancelling_a_job_withdraws_its_card() {
    let mut rig = Rig::new();
    rig.agents.config.inline_job_wait = Duration::ZERO;
    let mut c = rig.legacy("2025-11-25");
    let r = rig.call(
        &mut c,
        "measure_tempo",
        json!({"bpm": 76, "ms": 50}),
        Some(Answer::Allow),
    );
    let job = r.structured.unwrap()["job"].as_str().unwrap().to_string();
    rig.host
        .bus
        .execute(
            Command::SetProjectName {
                name: "moved".into(),
            },
            Origin::User,
        )
        .unwrap();
    wait_until("the edit is on the card", || {
        rig.pump();
        rig.agents.asks().count() == 1
    });
    let x = rig.call(&mut c, "cancel_job", json!({"job": job}), None);
    assert!(!x.is_error, "{}", x.text);
    assert_eq!(rig.agents.asks().count(), 0, "the card is withdrawn");
    let w = rig.call(&mut c, "wait_job", json!({"job": job}), None);
    assert!(w.is_error && w.text.contains("cancelled"), "{}", w.text);
}

/// #6: a change a running job will join stays open past its idle time.
#[test]
fn a_change_stays_open_while_its_job_runs() {
    let mut rig = Rig::new();
    rig.host.policy.allow_edits = true;
    rig.agents.config.inline_job_wait = Duration::ZERO;
    let mut m = rig.modern();
    let r = rig.call(&mut m, "begin_change", json!({"label": "x"}), None);
    let ch = r.structured.unwrap()["change"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(
        !rig.call(&mut m, "set_tempo", json!({"bpm": 80, "change": ch}), None)
            .is_error
    );
    // Idle at once from the moment the job runs (before, the change could end on a pump
    // the call had not reached yet: a race, not the case held).
    let agents = &mut rig.agents;
    let host = &mut rig.host;
    let args = json!({"bpm": 81, "ms": 150, "change": ch});
    let r = m.call_tool("measure_tempo", args, &mut || {
        agents.pump(host);
        if agents.jobs().running(None) > 0 {
            agents.config.change_idle = Duration::ZERO;
        }
    });
    let job = r.structured.unwrap()["job"].as_str().unwrap().to_string();
    let w = rig.call(&mut m, "wait_job", json!({"job": job}), None);
    assert!(!w.is_error, "{}", w.text);
    assert_eq!(
        rig.host.bus.history().len(),
        1,
        "the job's edit joined the change"
    );
}

/// #7: the bridge sends the token only to a door that proves it holds it.
#[test]
fn the_bridge_never_sends_the_token_to_a_stranger() {
    let dir = std::env::temp_dir().join(format!("cantus-r3-stranger-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let token = cantus_mcp::home::token(&dir).unwrap();
    // Something else on the port: it answers everything with 200, and records what it got.
    let stranger = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = stranger.local_addr().unwrap().port();
    cantus_mcp::home::write_door(
        &dir,
        &cantus_mcp::home::DoorFile {
            port,
            pid: 1,
            version: "x".into(),
        },
    )
    .unwrap();
    let heard = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
    let h = heard.clone();
    std::thread::spawn(move || {
        for s in stranger.incoming().flatten() {
            let mut s = s;
            let mut buf = [0u8; 4096];
            s.set_read_timeout(Some(Duration::from_millis(500))).ok();
            if let Ok(n) = s.read(&mut buf) {
                h.lock()
                    .unwrap()
                    .push_str(&String::from_utf8_lossy(&buf[..n]));
            }
            let _ = s.write_all(b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\n\r\n");
        }
    });
    let bridge = cantus_mcp::bridge::Bridge::new(cantus_mcp::bridge::Finder { dir: dir.clone() });
    let r = bridge
        .message(r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{}}}"#)
        .unwrap();
    assert_eq!(r["error"]["code"], cantus_mcp::bridge::NOT_CANTUS, "{r}");
    assert!(
        !heard.lock().unwrap().contains(&token),
        "the token never left"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// #9: a session is used only with the token that opened it.
#[test]
fn a_session_belongs_to_its_token() {
    let mut rig = Rig::new();
    rig.host.policy.external_agents = true;
    let d = door(&rig, Duration::from_secs(10));
    let init = json!({"jsonrpc": "2.0", "id": 1, "method": "initialize",
                      "params": {"protocolVersion": "2025-11-25", "capabilities": {}}});
    let post = |token: &str, sid: Option<&str>, body: &Value| {
        let mut c = Client::new(d.addr());
        let auth = format!("Bearer {token}");
        let mut h = vec![
            ("Authorization", auth.as_str()),
            ("Content-Type", "application/json"),
        ];
        if let Some(s) = sid {
            h.push(("Mcp-Session-Id", s));
        }
        c.send("POST", &h, body.to_string().as_bytes()).unwrap()
    };
    let m = post(TOKEN, None, &init);
    let sid = m.header("mcp-session-id").unwrap().to_string();
    let ping = json!({"jsonrpc": "2.0", "id": 2, "method": "ping"});
    assert_eq!(
        post(OTHER, Some(&sid), &ping).start.0,
        "404",
        "another token"
    );
    assert_eq!(post(TOKEN, Some(&sid), &ping).start.0, "200");
}

/// #10: a plan that changed the project while planning is refused, never passed on.
#[test]
fn planning_that_edits_is_refused() {
    let mut rig = Rig::new();
    rig.host.policy.allow_edits = true;
    let mut c = rig.legacy("2025-11-25");
    let r = rig.call(&mut c, "sneaky_plan", json!({}), None);
    assert!(
        r.is_error && r.text.contains("not an edit tool"),
        "{}",
        r.text
    );
    assert_ne!(
        rig.host.bus.project().tempo_bpm,
        63.0,
        "its plan was not executed"
    );
}

/// #12: a call cancelled while it waits on the card is dropped, its card withdrawn.
#[test]
fn a_cancelled_call_leaves_the_card() {
    let mut rig = Rig::new();
    let mut c = rig.legacy("2025-11-25");
    let session = c.session().clone();
    let mut canceller = TestClient::raw_on(rig.server.clone(), session);
    let agents = &mut rig.agents;
    let host = &mut rig.host;
    let mut sent = false;
    let r = c.call_tool("set_tempo", json!({"bpm": 91}), &mut || {
        agents.pump(host);
        if !sent && agents.asks().count() == 1 {
            // The test client numbers its requests from 1: initialize was 1, this is 2.
            canceller.notify("notifications/cancelled", json!({"requestId": 2}));
            sent = true;
        }
    });
    assert!(r.is_error && r.text.contains("cancelled"), "{}", r.text);
    for _ in 0..5 {
        agents.pump(host);
    }
    assert_eq!(agents.asks().count(), 0);
    assert_eq!(host.bus.project().tempo_bpm, 120.0);
}

/// #13: the agents remembered at once are bounded.
#[test]
fn the_agents_remembered_are_bounded() {
    let mut rig = Rig::new();
    rig.agents.config.max_agents = 1;
    let mut a = rig.legacy("2025-11-25");
    let mut b = rig.legacy("2025-11-25");
    assert!(!rig.call(&mut a, "get_tempo", json!({}), None).is_error);
    let r = rig.call(&mut b, "get_tempo", json!({}), None);
    assert!(
        r.is_error && r.text.contains("too many agents"),
        "{}",
        r.text
    );
    let _ = Session::new();
}

/// #14: the undo card is never shown for a step that is not the agent's.
#[test]
fn no_undo_card_for_another_agents_step() {
    let mut rig = Rig::new();
    rig.host.policy.allow_edits = true;
    let mut a = rig.legacy("2025-11-25");
    let mut b = rig.legacy("2025-11-25");
    rig.call(&mut a, "set_tempo", json!({"bpm": 99}), None);
    let r = rig.call(&mut b, "undo_last_ai_action", json!({}), None);
    assert!(r.is_error && r.text.contains("another agent"), "{}", r.text);
    assert!(
        !rig.agents.activity().any(|x| x.outcome == Outcome::Asked),
        "never asked"
    );
}

/// The integrator, after tools: a host tool's card reads its own sentence, worded from its
/// arguments, the user's name quoted.
#[test]
fn a_host_tools_card_reads_its_own_sentence() {
    let mut rig = Rig::new();
    let mut c = rig.legacy("2025-11-25");
    let agents = &mut rig.agents;
    let host = &mut rig.host;
    let mut said = None;
    let r = c.call_tool("export_named", json!({"file": "Song.wav"}), &mut || {
        agents.pump(host);
        let first = agents.asks().map(|a| (a.id, a.said.clone())).next();
        if let Some((id, s)) = first {
            said = Some(s);
            agents.answer(id, Answer::Allow);
        }
    });
    assert!(!r.is_error, "{}", r.text);
    assert_eq!(
        said.as_deref(),
        Some("ai.test.export file=ai.quoted name=Song.wav")
    );
}

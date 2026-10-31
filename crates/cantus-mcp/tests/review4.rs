//! The fourth review's findings, each held by a test that fails without its fix (round 34).
//! (#1, #2, #4, #8 are in cantus-rt's private.rs, home.rs and tests/bridge.rs; #5 in
//! schema.rs; #6 in cantus-ai's tool.rs; #10 in cantus-ai's tool.rs and cantus-ui.)

mod common;

use cantus_mcp::client::TestClient;
use common::Rig;
use serde_json::json;
use std::time::{Duration, Instant};

/// #7: a job whose starting call is cancelled before it is answered is cancelled too —
/// nobody holds its handle, so its edit must never land.
#[test]
fn a_job_whose_starting_call_is_cancelled_is_cancelled() {
    let mut rig = Rig::new();
    rig.host.policy.allow_edits = true;
    rig.agents.config.inline_job_wait = Duration::from_secs(10);
    let mut c = rig.legacy("2025-11-25");
    let session = c.session().clone();
    let mut canceller = TestClient::raw_on(rig.server.clone(), session);
    let agents = &mut rig.agents;
    let host = &mut rig.host;
    let mut sent = false;
    let r = c.call_tool("measure_tempo", json!({"bpm": 77, "ms": 300}), &mut || {
        agents.pump(host);
        if !sent && agents.jobs().running(None) == 1 {
            // initialize was request 1; this call is 2.
            canceller.notify("notifications/cancelled", json!({"requestId": 2}));
            sent = true;
        }
    });
    assert!(r.is_error && r.text.contains("cancelled"), "{}", r.text);
    let end = Instant::now() + Duration::from_secs(5);
    while agents.jobs().running(None) > 0 {
        assert!(Instant::now() < end, "the job ends");
        agents.pump(host);
        std::thread::sleep(Duration::from_millis(10));
    }
    // Past the job's own time: its edit never came.
    std::thread::sleep(Duration::from_millis(400));
    for _ in 0..5 {
        agents.pump(host);
    }
    assert_eq!(
        host.bus.project().tempo_bpm,
        120.0,
        "the job's edit never landed"
    );
    let last = agents.activity().last().unwrap();
    assert!(
        format!("{:?}", last.outcome).contains("cancelled"),
        "{:?}",
        last.outcome
    );
}

const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn door(rig: &Rig, token: &str, instance_idle: Duration) -> cantus_mcp::http::HttpDoor {
    cantus_mcp::http::HttpDoor::open(
        rig.server.clone(),
        cantus_mcp::http::HttpConfig {
            port: 0,
            tokens: vec![("default".into(), token.into())],
            instance_idle,
            ..Default::default()
        },
    )
    .unwrap()
}

fn settings(name: &str, port: u16) -> (std::path::PathBuf, String) {
    let dir = std::env::temp_dir().join(format!("cantus-r4-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let token = cantus_mcp::home::token(&dir).unwrap();
    let door = cantus_mcp::home::DoorFile {
        port,
        pid: 1,
        version: "x".into(),
    };
    cantus_mcp::home::write_door(&dir, &door).unwrap();
    (dir, token)
}

/// Copy one direction of a stream until it ends.
fn pipe(mut from: std::net::TcpStream, mut to: std::net::TcpStream) {
    std::thread::spawn(move || {
        let _ = std::io::copy(&mut from, &mut to);
        let _ = to.shutdown(std::net::Shutdown::Write);
    });
}

/// #3: the door proves itself on the very connection the token then goes over — never a
/// proof remembered from an earlier one. Here the port answers the real door on its first
/// connection and a stranger on every later one (Cantus gone, something else bound in its
/// place): the stranger never hears the token.
#[test]
fn a_proof_holds_only_for_its_own_connection() {
    let rig = Rig::new();
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = listener.local_addr().unwrap().port();
    let (dir, token) = settings("proof", port);
    let real = door(&rig, &token, Duration::from_secs(3600));
    let heard = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
    let h = heard.clone();
    let real_addr = real.addr();
    std::thread::spawn(move || {
        for (n, s) in listener.incoming().flatten().enumerate() {
            if n == 0 {
                let up = std::net::TcpStream::connect(real_addr).unwrap();
                pipe(s.try_clone().unwrap(), up.try_clone().unwrap());
                pipe(up, s);
                continue;
            }
            // The stranger: records what it hears, answers every request 204.
            let h = h.clone();
            std::thread::spawn(move || {
                let mut s = s;
                s.set_read_timeout(Some(Duration::from_millis(300))).ok();
                let mut buf = [0u8; 8192];
                while let Ok(k) = std::io::Read::read(&mut s, &mut buf) {
                    if k == 0 {
                        break;
                    }
                    h.lock()
                        .unwrap()
                        .push_str(&String::from_utf8_lossy(&buf[..k]));
                    let _ = std::io::Write::write_all(
                        &mut s,
                        b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\n\r\n",
                    );
                }
            });
        }
    });
    let bridge = cantus_mcp::bridge::Bridge::new(cantus_mcp::bridge::Finder { dir: dir.clone() });
    let r = bridge
        .message(r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{}}}"#)
        .unwrap();
    assert!(r.get("result").is_some(), "the real door answered: {r}");
    let r = bridge
        .message(r#"{"jsonrpc":"2.0","id":2,"method":"ping"}"#)
        .unwrap();
    assert_eq!(r["error"]["code"], cantus_mcp::bridge::NOT_CANTUS, "{r}");
    std::thread::sleep(Duration::from_millis(100));
    let heard = heard.lock().unwrap().clone();
    assert!(
        heard.contains("Mcp-Cantus-Proof"),
        "the stranger was asked to prove"
    );
    assert!(
        !heard.contains(&token),
        "the token never reached the stranger"
    );
    real.close();
    let _ = std::fs::remove_dir_all(&dir);
}

/// #12: a request the client cancelled before the bridge sent it is never sent (nor
/// answered: the client expects nothing).
#[test]
fn a_request_cancelled_before_it_was_sent_is_never_sent() {
    let stranger = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = stranger.local_addr().unwrap().port();
    let (dir, _) = settings("cancel", port);
    // Counts the connections made to it, and closes each at once.
    let made = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let m = made.clone();
    std::thread::spawn(move || {
        for s in stranger.incoming().flatten() {
            m.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            drop(s);
        }
    });
    let bridge = cantus_mcp::bridge::Bridge::new(cantus_mcp::bridge::Finder { dir: dir.clone() });
    bridge.queued(&json!(7));
    bridge.cancel(&json!(7));
    let r = bridge.message(
        r#"{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"play","arguments":{}}}"#,
    );
    assert!(r.is_none(), "{r:?}");
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(
        made.load(std::sync::atomic::Ordering::SeqCst),
        0,
        "no connection was even opened for it"
    );
    // Only that one: the next with the same id is sent (and answered, here an error).
    let r = bridge.message(r#"{"jsonrpc":"2.0","id":7,"method":"ping"}"#);
    assert!(r.is_some_and(|v| v.get("error").is_some()));
    assert!(made.load(std::sync::atomic::Ordering::SeqCst) >= 1);
    let _ = std::fs::remove_dir_all(&dir);
}

/// #9: a bridge instance unheard of past its idle time gives its place up — the 33rd
/// after a while is an agent of its own, not the token's shared one.
#[test]
fn idle_bridge_instances_give_their_place_up() {
    let rig = Rig::new();
    let d = door(&rig, TOKEN, Duration::from_secs(4));
    let auth = format!("Bearer {TOKEN}");
    let ping = |instance: &str| {
        let mut c = cantus_mcp::http_client::Client::new(d.addr());
        let h = [
            ("Authorization", auth.as_str()),
            ("Content-Type", "application/json"),
            ("Accept", "application/json, text/event-stream"),
            ("Mcp-Cantus-Instance", instance),
        ];
        let m = c
            .send("POST", &h, br#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#)
            .unwrap();
        assert_eq!(m.start.0, "200", "{}", String::from_utf8_lossy(&m.body));
    };
    let started = std::time::Instant::now();
    for i in 0..33 {
        ping(&format!("bridge-{i}"));
    }
    if started.elapsed() > Duration::from_secs(3) {
        eprintln!("this machine is too slow to time the idle instances: not run");
        d.close();
        return;
    }
    assert_eq!(d.instances(), 32, "at most 32 at once");
    std::thread::sleep(Duration::from_millis(4200));
    ping("bridge-late");
    assert_eq!(
        d.instances(),
        1,
        "the idle ones forgotten, the new one kept"
    );
    d.close();
}

/// #11: a refusal sent while the client is still sending its body reaches it — the door
/// shuts its side and drains, so the close is not a reset that discards the answer.
#[test]
fn a_refusal_reaches_a_client_still_sending() {
    let rig = Rig::new();
    let d = door(&rig, TOKEN, Duration::from_secs(3600));
    for _ in 0..5 {
        let mut s = std::net::TcpStream::connect(d.addr()).unwrap();
        let body = vec![b' '; 256 << 10];
        let head = format!(
            "POST /mcp HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n",
            body.len()
        );
        std::io::Write::write_all(&mut s, head.as_bytes()).unwrap();
        // Part of the body, as a client that does not wait for an interim answer.
        let _ = std::io::Write::write_all(&mut s, &body[..48 << 10]);
        s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        let mut answer = Vec::new();
        let _ = std::io::Read::read_to_end(&mut s, &mut answer);
        let text = String::from_utf8_lossy(&answer);
        assert!(text.starts_with("HTTP/1.1 401"), "{text:?}");
    }
    d.close();
}

/// #5: a client that writes every optional argument, `null` when unset, is not refused for
/// it; a misspelt one still is.
#[test]
fn an_optional_argument_sent_as_null_is_one_left_out() {
    let mut rig = Rig::new();
    let mut c = rig.legacy("2025-11-25");
    let args = json!({"steps": 1, "ms": null, "panic": null, "bpm": null});
    let r = rig.call(&mut c, "count_typed", args, None);
    assert!(!r.is_error, "{}", r.text);
    let r = rig.call(
        &mut c,
        "count_typed",
        json!({"steps": 1, "mss": null}),
        None,
    );
    assert!(r.is_error && r.text.contains("mss"), "{}", r.text);
}

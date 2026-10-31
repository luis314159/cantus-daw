//! The fifth review's findings in cantus-mcp, each held by a test that fails without its
//! fix (round 34). (#1, #2, #10, #13 are in cantus-rt's private.rs and home.rs; #3 in
//! xtask's package tests; #4, #5, #12 in cantus-ui; #11 in cantus-ai's tool.rs.)

mod common;

use common::Rig;
use serde_json::json;
use std::io::{Read, Write};
use std::time::{Duration, Instant};

const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn door(rig: &Rig) -> cantus_mcp::http::HttpDoor {
    cantus_mcp::http::HttpDoor::open(
        rig.server.clone(),
        cantus_mcp::http::HttpConfig {
            port: 0,
            tokens: vec![("default".into(), TOKEN.into())],
            ..Default::default()
        },
    )
    .unwrap()
}

/// #6: a refused client that keeps trickling a byte at a time does not hold its slot past
/// the drain's total time (it held it for hours: the drain was bounded in bytes alone).
#[test]
fn a_trickling_refused_client_is_let_go_in_about_a_second() {
    let rig = Rig::new();
    let d = door(&rig);
    let mut s = std::net::TcpStream::connect(d.addr()).unwrap();
    s.write_all(
        b"POST /mcp HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: 100000\r\n\r\n",
    )
    .unwrap();
    let mut w = s.try_clone().unwrap();
    let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let st = stop.clone();
    let trickle = std::thread::spawn(move || {
        while !st.load(std::sync::atomic::Ordering::SeqCst) {
            if w.write_all(b" ").is_err() {
                break;
            }
            std::thread::sleep(Duration::from_millis(150));
        }
    });
    s.set_read_timeout(Some(Duration::from_secs(20))).unwrap();
    let mut answer = Vec::new();
    let _ = s.read_to_end(&mut answer);
    assert!(String::from_utf8_lossy(&answer).starts_with("HTTP/1.1 401"));
    // Still trickling: the door lets the connection go after its drain's total time.
    let started = Instant::now();
    let held = loop {
        if d.connections() == 0 {
            break None;
        }
        let took = started.elapsed();
        if took > cantus_mcp::http::LINGER + Duration::from_secs(4) {
            break Some(took);
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    stop.store(true, std::sync::atomic::Ordering::SeqCst);
    let _ = trickle.join();
    assert_eq!(
        held, None,
        "the slot was still held while the client trickled"
    );
    d.close();
}

/// #8: something that accepts on Cantus's port and never answers the proof is "not
/// Cantus" in seconds — not after the answer's 660 s.
#[test]
fn a_silent_port_is_not_cantus_in_seconds() {
    let silent = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let addr = silent.local_addr().unwrap();
    let held = std::thread::spawn(move || {
        // Accepts, reads nothing, says nothing, until the client goes.
        let (mut s, _) = silent.accept().unwrap();
        let mut buf = [0u8; 1024];
        s.set_read_timeout(Some(Duration::from_secs(30))).ok();
        while let Ok(n) = s.read(&mut buf) {
            if n == 0 {
                break;
            }
        }
    });
    let mut c = cantus_mcp::http_client::Client::new(addr);
    c.prove = Some(TOKEN.into());
    c.timeout = Duration::from_secs(60);
    c.proof_timeout = Duration::from_millis(300);
    let started = Instant::now();
    let err = c.send("POST", &[], b"{}").unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::PermissionDenied, "{err}");
    assert!(started.elapsed() < Duration::from_secs(10));
    drop(c);
    let _ = held.join();
}

/// #9: a cancellation is remembered only for a request still queued; one for a request
/// already sent (or never seen) is not, and a new session forgets them all.
#[test]
fn the_bridges_cancellations_never_pile_up() {
    let dir = std::env::temp_dir().join(format!("cantus-r5-cancel-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let bridge = cantus_mcp::bridge::Bridge::new(cantus_mcp::bridge::Finder { dir: dir.clone() });
    for i in 0..100 {
        bridge.cancel(&json!(i));
    }
    assert_eq!(bridge.cancellations(), 0, "none of them was queued");
    bridge.queued(&json!("a"));
    bridge.queued(&json!("b"));
    bridge.cancel(&json!("a"));
    bridge.cancel(&json!("b"));
    assert_eq!(bridge.cancellations(), 2);
    // "a" taken by a worker: dropped unsent, its cancellation spent.
    assert!(bridge
        .message(r#"{"jsonrpc":"2.0","id":"a","method":"ping"}"#)
        .is_none());
    assert_eq!(bridge.cancellations(), 1);
    // A new session forgets the rest.
    let _ = bridge.message(
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{}}}"#,
    );
    assert_eq!(bridge.cancellations(), 0);
    // Review 6 #8: a reply the client sends (an id, no method) is never queued — no worker
    // would take it off again.
    for i in 0..50 {
        bridge.queue_line(&json!({"jsonrpc": "2.0", "id": i, "result": {}}));
    }
    assert_eq!(bridge.waiting(), 0);
    bridge.queue_line(&json!({"jsonrpc": "2.0", "id": 9, "method": "ping"}));
    assert_eq!(bridge.waiting(), 1);
    let _ = std::fs::remove_dir_all(&dir);
}

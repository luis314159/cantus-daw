//! The stdio bridge, the real program (`cantus-mcp`), as an MCP client starts it: through
//! a running door to the project, and with no Cantus running.

mod common;

use cantus_mcp::home::{self, DoorFile};
use cantus_mcp::http::{HttpConfig, HttpDoor};
use common::Rig;
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

struct Bridge {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout: BufReader<ChildStdout>,
}

impl Bridge {
    fn start(dir: &std::path::Path) -> Bridge {
        let mut child = Command::new(env!("CARGO_BIN_EXE_cantus-mcp"))
            .env("CANTUS_DATA_DIR", dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let stdin = child.stdin.take();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        Bridge {
            child,
            stdin,
            stdout,
        }
    }

    fn send(&mut self, v: Value) {
        let s = self.stdin.as_mut().unwrap();
        writeln!(s, "{v}").unwrap();
        s.flush().unwrap();
    }

    fn ask(&mut self, v: Value) -> Value {
        self.send(v);
        let mut line = String::new();
        self.stdout.read_line(&mut line).unwrap();
        assert!(
            line.ends_with('\n') && !line.trim_end().contains('\n'),
            "one line"
        );
        serde_json::from_str(&line).unwrap()
    }

    fn finish(mut self) {
        drop(self.stdin.take());
        let status = self.child.wait().unwrap();
        assert!(status.success());
    }
}

fn temp(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("cantus-bridge-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn with_no_cantus_running_every_request_says_what_to_do() {
    let dir = temp("none");
    let mut b = Bridge::start(&dir);
    let r = b.ask(json!({"jsonrpc": "2.0", "id": 1, "method": "initialize",
                         "params": {"protocolVersion": "2025-11-25", "capabilities": {}}}));
    assert_eq!(r["id"], 1);
    assert_eq!(r["error"]["code"], cantus_mcp::bridge::NOT_RUNNING);
    assert!(r["error"]["message"]
        .as_str()
        .unwrap()
        .contains("Allow external agents"));
    // A notification gets nothing, and the next request its own error.
    b.send(json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));
    let r = b.ask(json!({"jsonrpc": "2.0", "id": "x", "method": "tools/list"}));
    assert_eq!(r["id"], "x");
    assert_eq!(r["error"]["code"], cantus_mcp::bridge::NOT_RUNNING);
    // A door file left by a Cantus that is gone: the same, cleanly.
    home::write_door(
        &dir,
        &DoorFile {
            port: 1,
            pid: 1,
            version: "x".into(),
        },
    )
    .unwrap();
    home::regenerate_token(&dir).unwrap();
    let r = b.ask(json!({"jsonrpc": "2.0", "id": 2, "method": "ping"}));
    assert_eq!(r["error"]["code"], cantus_mcp::bridge::NOT_RUNNING);
    b.finish();
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn through_the_door_a_client_lists_calls_and_ends_its_session() {
    let dir = temp("door");
    home::token(&dir).unwrap();
    let mut rig = Rig::new();
    rig.host.policy.external_agents = true;
    rig.host.policy.allow_edits = true;
    let door = HttpDoor::open(
        rig.server.clone(),
        HttpConfig {
            port: 0,
            tokens: cantus_mcp::home::tokens(&dir).unwrap(),
            max_connections: 8,
            ..HttpConfig::default()
        },
    )
    .unwrap();
    home::write_door(
        &dir,
        &DoorFile {
            port: door.port(),
            pid: std::process::id(),
            version: "t".into(),
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
        host
    });
    let mut b = Bridge::start(&dir);
    let r = b.ask(json!({"jsonrpc": "2.0", "id": 1, "method": "initialize",
                         "params": {"protocolVersion": "2025-06-18", "capabilities": {},
                                    "clientInfo": {"name": "Claude Code"}}}));
    assert_eq!(r["result"]["protocolVersion"], "2025-06-18", "{r}");
    b.send(json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));
    let r = b.ask(json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}));
    assert!(r["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .any(|t| t["name"] == "set_tempo"));
    let r = b.ask(json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call",
                         "params": {"name": "set_tempo", "arguments": {"bpm": 88}}}));
    assert_eq!(r["result"]["isError"], false, "{r}");
    assert_eq!(door.sessions(), 1);
    b.finish();
    assert_eq!(
        door.sessions(),
        0,
        "the bridge ended its session as its client left"
    );
    stop.store(true, Ordering::SeqCst);
    let host = pump.join().unwrap();
    assert_eq!(host.bus.project().tempo_bpm, 88.0);
    door.close();
    let _ = std::fs::remove_dir_all(&dir);
}

/// Review 3 #12: a call waiting on the user's card does not hold back a ping.
#[test]
fn a_waiting_call_does_not_hold_back_a_ping() {
    let dir = temp("side-by-side");
    home::token(&dir).unwrap();
    let mut rig = Rig::new();
    rig.host.policy.external_agents = true;
    let door = HttpDoor::open(
        rig.server.clone(),
        HttpConfig {
            port: 0,
            tokens: cantus_mcp::home::tokens(&dir).unwrap(),
            max_connections: 8,
            ..HttpConfig::default()
        },
    )
    .unwrap();
    home::write_door(
        &dir,
        &DoorFile {
            port: door.port(),
            pid: std::process::id(),
            version: "t".into(),
        },
    )
    .unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let deny = Arc::new(AtomicBool::new(false));
    let (s, d) = (stop.clone(), deny.clone());
    let Rig {
        mut agents,
        mut host,
        ..
    } = rig;
    let pump = std::thread::spawn(move || {
        while !s.load(Ordering::SeqCst) {
            agents.pump(&mut host);
            if d.load(Ordering::SeqCst) {
                let ids: Vec<u64> = agents.asks().map(|a| a.id).collect();
                for id in ids {
                    agents.answer(id, cantus_mcp::Answer::Deny);
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    });
    let mut b = Bridge::start(&dir);
    let r = b.ask(json!({"jsonrpc": "2.0", "id": 1, "method": "initialize",
                         "params": {"protocolVersion": "2025-11-25", "capabilities": {}}}));
    assert!(r.get("result").is_some(), "{r}");
    b.send(json!({"jsonrpc": "2.0", "id": 2, "method": "tools/call",
                  "params": {"name": "set_tempo", "arguments": {"bpm": 88}}}));
    std::thread::sleep(std::time::Duration::from_millis(200));
    let r = b.ask(json!({"jsonrpc": "2.0", "id": 3, "method": "ping"}));
    assert_eq!(
        r["id"], 3,
        "the ping is answered while the call waits on the card"
    );
    deny.store(true, Ordering::SeqCst);
    let mut line = String::new();
    b.stdout.read_line(&mut line).unwrap();
    let r: Value = serde_json::from_str(&line).unwrap();
    assert_eq!(r["id"], 2);
    assert_eq!(r["result"]["isError"], true);
    b.finish();
    stop.store(true, Ordering::SeqCst);
    pump.join().unwrap();
    door.close();
    let _ = std::fs::remove_dir_all(&dir);
}

/// Review 4 #1: a settings folder another user could swap is never read, and the bridge
/// says so (not "Cantus is not running"); our own folder left group-writable by a umask of
/// 002 is tightened and read.
#[cfg(unix)]
#[test]
fn a_settings_folder_that_is_not_private_is_named() {
    use std::os::unix::fs::PermissionsExt;
    let base = temp("open");
    let dir = base.join("cantus");
    let mcp = home::secrets(&dir);
    std::fs::create_dir_all(&mcp).unwrap();
    std::fs::set_permissions(&mcp, std::fs::Permissions::from_mode(0o775)).unwrap();
    let mut b = Bridge::start(&dir);
    let r = b.ask(json!({"jsonrpc": "2.0", "id": 1, "method": "ping"}));
    assert_eq!(r["error"]["code"], cantus_mcp::bridge::NOT_RUNNING, "{r}");
    let mode = std::fs::metadata(&mcp).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o700, "tightened");
    // The settings folder, the `mcp` folder's parent, open to all and not sticky.
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o777)).unwrap();
    let r = b.ask(json!({"jsonrpc": "2.0", "id": 2, "method": "ping"}));
    assert_eq!(r["error"]["code"], cantus_mcp::bridge::NOT_PRIVATE, "{r}");
    assert!(r["error"]["message"]
        .as_str()
        .unwrap()
        .contains("not private"));
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
    b.finish();
    let _ = std::fs::remove_dir_all(&base);
}

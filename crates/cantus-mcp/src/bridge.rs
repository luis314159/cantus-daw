//! The stdio bridge (ADR 0081 §10): `cantus-mcp`, a small program beside `cantus.exe` that an
//! MCP client starts and talks to over stdin/stdout (newline-delimited JSON-RPC, MCP's
//! stdio transport). It forwards each message to the running application's HTTP door —
//! found through `mcp-door.json` and `mcp-token` in the user's settings folder — and writes
//! each answer back on one line. When Cantus is not running, or its door is closed, every
//! request is answered with a clean MCP error that says what to do.
//!
//! - The token goes only over a connection on which the door has proved it holds it: a
//!   fresh nonce answered with its HMAC under the token, on that same connection, before
//!   anything else is sent (review 3 #7, review 4 #3) — whatever else listens on Cantus's
//!   port never sees the token.
//! - The settings folder must be private (this user's, nobody else may write in it);
//!   otherwise every request says so (review 4 #1).
//! - Requests run side by side on a bounded pool, the rest queued — the stdin thread only
//!   reads, so a `ping` or a `notifications/cancelled` is never held back by a call waiting
//!   on the user's card; a request cancelled before it was sent is never sent (review 3
//!   #12, review 4 #12); `initialize` runs alone, first.

use crate::home::{self, DoorFile};
use crate::http_client::Client;
use crate::jsonrpc;
use serde_json::Value;
use std::collections::HashSet;
use std::io::{self, BufRead, Write};
use std::net::{Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{mpsc, Arc, Mutex};

/// The bridge's own error code (outside JSON-RPC's reserved range, as MCP asks of new
/// codes): Cantus cannot be reached.
pub const NOT_RUNNING: i64 = -31000;
/// The token the bridge read was refused (it was regenerated): set up again.
pub const BAD_TOKEN: i64 = -31001;
/// Something that is not Cantus answers on its port: the token was not sent.
pub const NOT_CANTUS: i64 = -31002;
/// The settings folder is not private: nothing is read from it.
pub const NOT_PRIVATE: i64 = -31003;

pub const NOT_RUNNING_WORDS: &str = "Cantus is not running, or it does not allow external agents — open Cantus and turn on Settings ▸ AI ▸ Allow external agents";
pub const NOT_CANTUS_WORDS: &str = "something that is not Cantus answers on Cantus's port, so the token was not sent: close it, or choose another port in Cantus's Settings ▸ AI";

/// Requests served at once; more wait in the queue.
const POOL: usize = 16;

/// Where the bridge looks.
pub struct Finder {
    pub dir: PathBuf,
}

/// Why the door cannot be found.
pub enum Unfound {
    /// The settings folder is not private (why).
    NotPrivate(String),
    /// No door file, or no token: Cantus is not running or its door is closed.
    Closed,
}

impl Finder {
    /// The door the app wrote, with the token — only from a private folder.
    pub fn find(&self) -> Result<(SocketAddr, String), Unfound> {
        cantus_rt::private::secure_dir(&home::secrets(&self.dir)).map_err(Unfound::NotPrivate)?;
        let DoorFile { port, .. } = home::read_door(&self.dir).ok_or(Unfound::Closed)?;
        let token = home::token_if_any(&self.dir).ok_or(Unfound::Closed)?;
        Ok((SocketAddr::from((Ipv4Addr::LOCALHOST, port)), token))
    }
}

fn error_line(id: Value, code: i64, message: &str) -> Value {
    jsonrpc::error(id, jsonrpc::RpcError::new(code, message))
}

/// What the bridge keeps across messages.
#[derive(Default)]
struct State {
    session: Option<String>,
    version: Option<String>,
}

/// The bridge: shared by its pool.
pub struct Bridge {
    finder: Finder,
    state: Mutex<State>,
    /// This process, to the door: two bridges are two agents (review 2 #11).
    instance: String,
    /// Requests queued, not yet taken by a worker, and those of them the client cancelled:
    /// a cancellation is kept only for a request still queued, and goes when a worker
    /// takes it or a new session starts — never a set that grows (review 5 #9).
    queue: Mutex<Queue>,
}

#[derive(Default)]
struct Queue {
    waiting: HashSet<String>,
    cancelled: HashSet<String>,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

impl Bridge {
    pub fn new(finder: Finder) -> Self {
        Bridge {
            finder,
            state: Mutex::new(State::default()),
            instance: crate::ids::fresh("bridge"),
            queue: Mutex::new(Queue::default()),
        }
    }

    /// A request goes into the queue (what `run` does before handing it to the pool).
    pub fn queued(&self, request_id: &Value) {
        lock(&self.queue).waiting.insert(request_id.to_string());
    }

    /// A cancellation the client sent: its request, if still queued, is never sent; one
    /// for a request already sent (or never seen) is the server's business — forwarded,
    /// not remembered.
    pub fn cancel(&self, request_id: &Value) {
        let mut q = lock(&self.queue);
        let id = request_id.to_string();
        if q.waiting.contains(&id) {
            q.cancelled.insert(id);
        }
    }

    /// Cancellations remembered now (for tests: bounded by the queue).
    pub fn cancellations(&self) -> usize {
        lock(&self.queue).cancelled.len()
    }

    /// Requests queued now, not yet taken (for tests).
    pub fn waiting(&self) -> usize {
        lock(&self.queue).waiting.len()
    }

    /// A line `run` hands to the pool: queued only if it is a request (an `id` and a
    /// `method`) — a reply the client sends to the server has no method, and no worker
    /// would ever take it off the queue (review 6 #8).
    pub fn queue_line(&self, parsed: &Value) {
        if parsed.get("method").is_some() {
            if let Some(id) = parsed.get("id") {
                self.queued(id);
            }
        }
    }

    /// One line from the client → the line to write back, if any.
    pub fn message(&self, line: &str) -> Option<Value> {
        let value: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(e) => {
                let words = format!("not JSON: {e}");
                return Some(error_line(Value::Null, jsonrpc::PARSE_ERROR, &words));
            }
        };
        let id = value.get("id").cloned();
        let is_request = id.is_some() && value.get("method").is_some();
        if let Some(i) = id.as_ref().filter(|_| is_request) {
            // Taken off the queue: no longer waiting, its cancellation (if any) spent.
            let mut q = lock(&self.queue);
            let key = i.to_string();
            q.waiting.remove(&key);
            if q.cancelled.remove(&key) {
                // Cancelled before it was sent: the client expects no answer.
                return None;
            }
        }
        let fail = |code: i64, words: &str| -> Option<Value> {
            is_request.then(|| error_line(id.clone().unwrap_or(Value::Null), code, words))
        };
        if value.get("method").and_then(Value::as_str) == Some("initialize") {
            // A new session: nothing of the last one is remembered.
            *lock(&self.queue) = Queue::default();
            let mut st = lock(&self.state);
            st.session = None;
            st.version = value
                .pointer("/params/protocolVersion")
                .and_then(Value::as_str)
                .map(str::to_string);
        }
        let (addr, token) = match self.finder.find() {
            Ok(found) => found,
            Err(Unfound::NotPrivate(why)) => {
                let words = format!(
                    "the settings folder is not private, so nothing is read from it: {why}"
                );
                return fail(NOT_PRIVATE, &words);
            }
            Err(Unfound::Closed) => return fail(NOT_RUNNING, NOT_RUNNING_WORDS),
        };
        let auth = format!("Bearer {token}");
        let (session, version) = {
            let st = lock(&self.state);
            (st.session.clone(), st.version.clone())
        };
        let mut headers: Vec<(&str, &str)> = vec![
            ("Authorization", &auth),
            ("Content-Type", "application/json"),
            ("Accept", "application/json, text/event-stream"),
            ("Mcp-Cantus-Instance", &self.instance),
        ];
        if let Some(s) = &session {
            headers.push(("Mcp-Session-Id", s));
        }
        let meta_version = value
            .pointer("/params/_meta/io.modelcontextprotocol~1protocolVersion")
            .and_then(Value::as_str)
            .map(str::to_string);
        let version = meta_version.or(version);
        if let Some(v) = &version {
            headers.push(("MCP-Protocol-Version", v));
        }
        let body = serde_json::to_vec(&value).expect("JSON");
        let mut client = Client::new(addr);
        client.prove = Some(token.clone());
        let answer = match client.send("POST", &headers, &body) {
            Ok(m) => m,
            Err(e) if e.kind() == io::ErrorKind::PermissionDenied => {
                return fail(NOT_CANTUS, NOT_CANTUS_WORDS)
            }
            Err(_) => return fail(NOT_RUNNING, NOT_RUNNING_WORDS),
        };
        if let Some(s) = answer.header("mcp-session-id") {
            lock(&self.state).session = Some(s.to_string());
        }
        // The version agreed, from initialize's answer.
        let parsed: Option<Value> = serde_json::from_slice(&answer.body).ok();
        if let Some(v) = parsed
            .as_ref()
            .and_then(|p| p.pointer("/result/protocolVersion"))
            .and_then(Value::as_str)
        {
            lock(&self.state).version = Some(v.to_string());
        }
        match answer.start.0.as_str() {
            "202" | "204" => None,
            "401" => fail(
                BAD_TOKEN,
                "Cantus refused the token (it was regenerated?): copy the setup again from Settings ▸ AI",
            ),
            "403" => fail(NOT_RUNNING, NOT_RUNNING_WORDS),
            "404" if session.is_some() => {
                lock(&self.state).session = None;
                fail(
                    NOT_RUNNING,
                    "the session ended (Cantus restarted, or the agent was disconnected): reconnect this MCP server",
                )
            }
            _ => match parsed {
                Some(p) => is_request.then_some(p),
                None => fail(
                    jsonrpc::INTERNAL_ERROR,
                    "Cantus answered something that is not JSON",
                ),
            },
        }
    }

    /// The client hung up: end the session in Cantus (over a proved connection).
    pub fn close(&self) {
        let Some(session) = lock(&self.state).session.take() else {
            return;
        };
        let Ok((addr, token)) = self.finder.find() else {
            return;
        };
        let auth = format!("Bearer {token}");
        let mut c = Client::new(addr);
        c.prove = Some(token.clone());
        let _ = c.send(
            "DELETE",
            &[("Authorization", &auth), ("Mcp-Session-Id", &session)],
            b"",
        );
    }
}

fn write_line(out: &Mutex<Box<dyn Write + Send>>, answer: &Value) -> io::Result<()> {
    // One message per line: serde_json never writes a raw newline.
    let text = serde_json::to_string(answer).expect("JSON");
    let mut o = lock(out);
    o.write_all(text.as_bytes())?;
    o.write_all(b"\n")?;
    o.flush()
}

/// Serve stdin → Cantus → stdout until stdin ends.
pub fn run(
    input: impl BufRead,
    output: impl Write + Send + 'static,
    finder: Finder,
) -> io::Result<()> {
    let bridge = Arc::new(Bridge::new(finder));
    let out: Arc<Mutex<Box<dyn Write + Send>>> = Arc::new(Mutex::new(Box::new(output)));
    let (tx, rx) = mpsc::channel::<String>();
    let rx = Arc::new(Mutex::new(rx));
    let busy = Arc::new(AtomicUsize::new(0));
    let pool: Vec<_> = (0..POOL)
        .map(|_| {
            let (b, o, r, busy) = (bridge.clone(), out.clone(), rx.clone(), busy.clone());
            std::thread::spawn(move || loop {
                let next = lock(&r).recv();
                let Ok(line) = next else { break };
                if let Some(answer) = b.message(&line) {
                    let _ = write_line(&o, &answer);
                }
                busy.fetch_sub(1, Ordering::SeqCst);
            })
        })
        .collect();
    for line in input.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let parsed: Option<Value> = serde_json::from_str(&line).ok();
        let method = parsed
            .as_ref()
            .and_then(|v| v.get("method"))
            .and_then(Value::as_str);
        let is_request = parsed.as_ref().is_some_and(|v| v.get("id").is_some());
        if method == Some("notifications/cancelled") {
            if let Some(id) = parsed.as_ref().and_then(|v| v.pointer("/params/requestId")) {
                bridge.cancel(id);
            }
        }
        if method == Some("initialize") {
            // Alone and first: everything before it answered.
            while busy.load(Ordering::SeqCst) > 0 {
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
        }
        if method == Some("initialize") || !is_request {
            if let Some(answer) = bridge.message(&line) {
                write_line(&out, &answer)?;
            }
            continue;
        }
        busy.fetch_add(1, Ordering::SeqCst);
        if let Some(v) = parsed.as_ref() {
            bridge.queue_line(v);
        }
        if tx.send(line).is_err() {
            break;
        }
    }
    drop(tx);
    for t in pool {
        let _ = t.join();
    }
    bridge.close();
    Ok(())
}

/// The bridge's `--version` and `--help`.
pub fn usage() -> String {
    format!(
        "cantus-mcp {} — Cantus's MCP server for clients that start a program (stdio).\n\
         It reaches the running Cantus; turn on Settings ▸ AI ▸ Allow external agents there.\n\
         Settings folder: {}\n",
        env!("CARGO_PKG_VERSION"),
        home::dir().display()
    )
}

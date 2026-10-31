//! An in-process MCP client for tests (ADR 0081 §4): it drives the protocol end to end —
//! the same [`Server::handle`] every transport calls, the door, the UI thread's pump — with
//! no socket. The request runs on a thread of its own, as a transport's would, while the
//! test's thread (which owns the application, as the UI thread does) turns the pump.
//!
//! ```ignore
//! let mut client = TestClient::legacy(server, "2025-11-25");
//! let r = client.call_tool("list_jobs", json!({}), &mut || agents.pump(&mut host));
//! assert!(!r.is_error);
//! ```

use crate::server::{Server, Session, META_CAPABILITIES, META_CLIENT, META_VERSION};
use serde_json::{json, Value};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::{Duration, Instant};

/// A tool call's result, read.
#[derive(Clone, Debug, PartialEq)]
pub struct ToolCall {
    pub is_error: bool,
    /// The first text block.
    pub text: String,
    pub structured: Option<Value>,
    /// The whole `result`.
    pub raw: Value,
}

/// A client of one connection.
pub struct TestClient {
    server: Server,
    session: Option<Session>,
    next_id: u64,
    /// `Some(version)` for a 2026-07-28 client, which sends `_meta` on every request.
    modern: Option<String>,
    pub name: String,
    /// The longest a request may take before the test fails.
    pub timeout: Duration,
}

impl TestClient {
    /// A client that has not said anything yet.
    pub fn raw(server: Server) -> Self {
        Self::raw_on(server, Session::new())
    }

    /// A client that has not said anything yet, on a session of the caller's making (an
    /// external one, a shared credential).
    pub fn raw_on(server: Server, session: Session) -> Self {
        TestClient {
            server,
            session: Some(session),
            next_id: 1,
            modern: None,
            name: "test-client".into(),
            timeout: Duration::from_secs(60),
        }
    }

    /// A handshake client (2025-06-18, 2025-11-25), initialised.
    pub fn legacy(server: Server, version: &str) -> Self {
        Self::legacy_on(server, version, Session::new())
    }

    /// [`Self::legacy`] on a given session.
    pub fn legacy_on(server: Server, version: &str, session: Session) -> Self {
        let mut c = Self::raw_on(server, session);
        let r = c
            .request(
                "initialize",
                json!({
                    "protocolVersion": version,
                    "capabilities": {},
                    "clientInfo": {"name": c.name, "version": "1.0"}
                }),
                &mut || {},
            )
            .expect("initialize answers");
        assert_eq!(
            r["protocolVersion"], version,
            "the version is agreed as asked"
        );
        c.notify("notifications/initialized", json!({}));
        c
    }

    /// A stateless client (2026-07-28).
    pub fn modern(server: Server) -> Self {
        Self::modern_on(server, Session::new())
    }

    /// [`Self::modern`] on a given session (its credential is whose calls these are).
    pub fn modern_on(server: Server, session: Session) -> Self {
        let mut c = Self::raw_on(server, session);
        c.modern = Some(crate::server::MODERN_VERSIONS[0].into());
        c
    }

    /// The session the server keeps for this connection.
    pub fn session(&self) -> &Session {
        self.session.as_ref().expect("no request in flight")
    }

    /// Send one message; turn `pump` until the server answers.
    pub fn send(&mut self, msg: Value, pump: &mut dyn FnMut()) -> Option<Value> {
        let server = self.server.clone();
        let mut session = self.session.take().expect("one request at a time");
        let (tx, rx) = mpsc::channel();
        std::thread::Builder::new()
            .name("cantus-mcp-test-client".into())
            .spawn(move || {
                let out = server.handle(&mut session, msg);
                let _ = tx.send((session, out));
            })
            .expect("a thread");
        let started = Instant::now();
        loop {
            match rx.recv_timeout(Duration::from_millis(1)) {
                Ok((session, out)) => {
                    self.session = Some(session);
                    return out;
                }
                Err(RecvTimeoutError::Timeout) => {
                    pump();
                    assert!(
                        started.elapsed() < self.timeout,
                        "the server did not answer within {:?}",
                        self.timeout
                    );
                }
                Err(RecvTimeoutError::Disconnected) => panic!("the server's thread panicked"),
            }
        }
    }

    /// `params` with this client's `_meta` when it is a stateless one.
    fn with_meta(&self, mut params: Value) -> Value {
        if let (Some(v), Some(obj)) = (&self.modern, params.as_object_mut()) {
            let meta = obj.entry("_meta").or_insert_with(|| json!({}));
            meta[META_VERSION] = json!(v);
            meta[META_CLIENT] = json!({"name": self.name, "version": "1.0"});
            meta[META_CAPABILITIES] = json!({});
        }
        params
    }

    /// A request: `Ok(result)` or `Err(error)`.
    pub fn request(
        &mut self,
        method: &str,
        params: Value,
        pump: &mut dyn FnMut(),
    ) -> Result<Value, Value> {
        let id = self.next_id;
        self.next_id += 1;
        let params = self.with_meta(params);
        let msg = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        let answer = self.send(msg, pump).expect("a request is answered");
        assert_eq!(answer["jsonrpc"], "2.0");
        assert_eq!(answer["id"], id, "the answer carries the request's id");
        match answer.get("error") {
            Some(e) => Err(e.clone()),
            None => Ok(answer["result"].clone()),
        }
    }

    /// A notification: never answered.
    pub fn notify(&mut self, method: &str, params: Value) {
        let params = self.with_meta(params);
        let msg = json!({"jsonrpc": "2.0", "method": method, "params": params});
        assert_eq!(
            self.send(msg, &mut || {}),
            None,
            "a notification is not answered"
        );
    }

    /// `tools/call`, read. Panics on a protocol error (use [`Self::request`] to see one).
    pub fn call_tool(&mut self, name: &str, args: Value, pump: &mut dyn FnMut()) -> ToolCall {
        let raw = self
            .request("tools/call", json!({"name": name, "arguments": args}), pump)
            .unwrap_or_else(|e| panic!("tools/call {name}: protocol error {e}"));
        ToolCall {
            is_error: raw["isError"].as_bool().expect("isError is said"),
            text: raw["content"][0]["text"].as_str().unwrap_or("").to_string(),
            structured: raw.get("structuredContent").cloned(),
            raw,
        }
    }
}

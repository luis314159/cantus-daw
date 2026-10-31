//! The Streamable HTTP door (ADR 0081 §9; MCP's transports page, 2025-06-18 → 2026-07-28):
//! one endpoint, `POST /mcp`, on `127.0.0.1` only, answering JSON (Cantus opens no SSE
//! stream, so `GET` is 405 as the specification allows).
//!
//! Every request is checked before it is read as MCP:
//! - **`Host`** must be the loopback (`127.0.0.1`, `localhost`, `[::1]`) and **`Origin`**, when
//!   sent, a loopback origin — otherwise 403 (DNS rebinding: a web page cannot reach it);
//! - **`Authorization: Bearer <token>`** must carry one of the install's tokens — the
//!   default one (the bridge's, a hand-made setup) or a client's own from *Copy setup for…* —
//!   otherwise 401;
//! - `Content-Type: application/json`, a body within the server's limit (413);
//! - `MCP-Protocol-Version`, when sent, a version spoken (400), and equal to a stateless
//!   request's `_meta` version (400, -32020 *HeaderMismatch*).
//!
//! Session clients get `Mcp-Session-Id` from `initialize` and send it back; an unknown or
//! ended one is 404 (they initialise again); `DELETE /mcp` ends a session. A stateless
//! request's agent is its credential: the token it carried (`http:<client>`), and for the
//! stdio bridge its process too (`Mcp-Cantus-Instance`), so two clients — or two bridges —
//! are never one agent, and one agent is the same on every request (review 1 #3, review 2
//! #11).

use crate::http_wire::{self, Message, WireError};
use crate::jsonrpc::{self, RpcError};
use crate::server::{Server, Session, META_VERSION, SUPPORTED_VERSIONS};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{self, BufReader, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// The header a bridge process names itself by (its own random id).
pub const INSTANCE_HEADER: &str = "mcp-cantus-instance";
/// How long an idle session lives.
const SESSION_IDLE: Duration = Duration::from_secs(3600);
const MAX_SESSIONS: usize = 64;

/// The door's settings.
#[derive(Clone, Debug)]
pub struct HttpConfig {
    /// 0: any free port (tests).
    pub port: u16,
    /// The tokens it accepts, each with the name its agents go by (`default`, a client's).
    pub tokens: Vec<(String, String)>,
    pub max_connections: usize,
    /// How long a request's head may take (the wait for it included), then its body.
    pub head_time: Duration,
    pub body_time: Duration,
    /// A bridge instance unheard of this long is forgotten (its place goes to a new one).
    pub instance_idle: Duration,
}

impl Default for HttpConfig {
    fn default() -> Self {
        HttpConfig {
            port: crate::home::DEFAULT_PORT,
            tokens: Vec::new(),
            max_connections: 16,
            head_time: Duration::from_secs(15),
            body_time: Duration::from_secs(30),
            instance_idle: Duration::from_secs(3600),
        }
    }
}

struct Shared {
    server: Server,
    tokens: Mutex<Vec<(String, String)>>,
    /// Bridge instances seen per token, and when last (review 3 #13, review 4 #9).
    instances: Mutex<HashMap<String, Vec<(String, Instant)>>>,
    instance_idle: Duration,
    sessions: Mutex<HashMap<String, (Session, Instant)>>,
    /// Sessions that ended (deleted, expired, revoked): the pump forgets their agents.
    ended: Mutex<Vec<String>>,
    stop: AtomicBool,
    connections: AtomicUsize,
    max_connections: usize,
    head_time: Duration,
    body_time: Duration,
}

/// The door, open. Closing it (or dropping it) stops listening and ends its sessions.
pub struct HttpDoor {
    addr: SocketAddr,
    shared: Arc<Shared>,
    thread: Option<JoinHandle<()>>,
}

impl HttpDoor {
    /// Open the door on `127.0.0.1:port`.
    pub fn open(server: Server, config: HttpConfig) -> io::Result<HttpDoor> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, config.port))?;
        let addr = listener.local_addr()?;
        let shared = Arc::new(Shared {
            server,
            tokens: Mutex::new(config.tokens),
            instances: Mutex::new(HashMap::new()),
            sessions: Mutex::new(HashMap::new()),
            ended: Mutex::new(Vec::new()),
            stop: AtomicBool::new(false),
            connections: AtomicUsize::new(0),
            max_connections: config.max_connections.max(1),
            head_time: config.head_time,
            body_time: config.body_time,
            instance_idle: config.instance_idle,
        });
        let s = shared.clone();
        let thread = std::thread::Builder::new()
            .name("cantus-mcp-http".into())
            .spawn(move || accept(listener, s))?;
        Ok(HttpDoor {
            addr,
            shared,
            thread: Some(thread),
        })
    }

    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    pub fn port(&self) -> u16 {
        self.addr.port()
    }

    /// End a session (the user disconnected its agent): its next request is 404.
    pub fn end_session(&self, id: &str) -> bool {
        let gone = lock(&self.shared.sessions).remove(id).is_some();
        if gone {
            lock(&self.shared.ended).push(id.to_string());
        }
        gone
    }

    /// The tokens it accepts from now on (a client's own was made; the tokens were
    /// regenerated).
    pub fn set_tokens(&self, tokens: Vec<(String, String)>) {
        *lock(&self.shared.tokens) = tokens;
    }

    /// End every session (the user revoked every agent, or changed the token).
    pub fn end_all(&self) {
        let ids: Vec<String> = lock(&self.shared.sessions)
            .drain()
            .map(|(k, _)| k)
            .collect();
        lock(&self.shared.ended).extend(ids);
    }

    /// The sessions that ended since the last call (their agents are forgotten).
    pub fn take_ended(&self) -> Vec<String> {
        std::mem::take(&mut *lock(&self.shared.ended))
    }

    /// Connections being served now.
    pub fn connections(&self) -> usize {
        self.shared.connections.load(Ordering::SeqCst)
    }

    /// Sessions open now.
    pub fn sessions(&self) -> usize {
        lock(&self.shared.sessions).len()
    }

    /// Bridge instances remembered now, over every token.
    pub fn instances(&self) -> usize {
        lock(&self.shared.instances).values().map(Vec::len).sum()
    }

    /// Stop listening.
    pub fn close(mut self) {
        self.shut();
    }

    fn shut(&mut self) {
        self.shared.stop.store(true, Ordering::SeqCst);
        // Wake the accepting thread.
        let _ = TcpStream::connect_timeout(&self.addr, Duration::from_millis(200));
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
        self.end_all();
    }
}

impl Drop for HttpDoor {
    fn drop(&mut self) {
        self.shut();
    }
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

fn accept(listener: TcpListener, shared: Arc<Shared>) {
    for stream in listener.incoming() {
        if shared.stop.load(Ordering::SeqCst) {
            break;
        }
        let Ok(mut stream) = stream else { continue };
        if shared.connections.load(Ordering::SeqCst) >= shared.max_connections {
            let _ = http_wire::respond(
                &mut stream,
                503,
                &[("Connection", "close"), ("Retry-After", "1")],
                b"",
            );
            continue;
        }
        let slot = Slot::take(shared.clone());
        let _ = std::thread::Builder::new()
            .name("cantus-mcp-http-conn".into())
            .spawn(move || {
                // A fault in one connection is its own: the slot is given back by the
                // guard whatever happens (review 3 #2), and the door goes on.
                let s = slot.shared.clone();
                let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let _ = connection(stream, &s);
                }));
                drop(slot);
            });
        // A thread that could not start dropped its closure, and the slot with it.
    }
}

/// One connection's place among `max_connections`, given back when dropped.
struct Slot {
    shared: Arc<Shared>,
}

impl Slot {
    fn take(shared: Arc<Shared>) -> Slot {
        shared.connections.fetch_add(1, Ordering::SeqCst);
        Slot { shared }
    }
}

impl Drop for Slot {
    fn drop(&mut self) {
        self.shared.connections.fetch_sub(1, Ordering::SeqCst);
    }
}

/// A stream read against one deadline, however the bytes trickle in.
struct Timed {
    stream: TcpStream,
    deadline: Instant,
}

impl io::Read for Timed {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let left = self.deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Err(io::Error::new(io::ErrorKind::TimedOut, "too slow"));
        }
        self.stream.set_read_timeout(Some(left))?;
        self.stream.read(buf)
    }
}

/// Whether the client is still there (it has not closed its side), looked at without
/// waiting while its call waits on the UI thread.
fn still_there(stream: &TcpStream) -> bool {
    if stream.set_nonblocking(true).is_err() {
        return true;
    }
    let mut one = [0u8; 1];
    let there = match stream.peek(&mut one) {
        Ok(0) => false,
        Ok(_) => true,
        Err(e) => e.kind() == io::ErrorKind::WouldBlock,
    };
    let _ = stream.set_nonblocking(false);
    there
}

fn connection(stream: TcpStream, shared: &Shared) -> io::Result<()> {
    stream.set_nodelay(true)?;
    let mut writer = stream.try_clone()?;
    let watch = stream.try_clone()?;
    let mut reader = BufReader::new(Timed {
        stream,
        deadline: Instant::now() + shared.head_time,
    });
    let max = shared.server.limits().max_request_bytes;
    loop {
        if shared.stop.load(Ordering::SeqCst) {
            return Ok(());
        }
        // A request's head (and the wait for it) and then its body each within their time:
        // no slow drip holds a slot (review 3 #3).
        reader.get_mut().deadline = Instant::now() + shared.head_time;
        let bad = |w: &mut TcpStream, status: u16, why: &str| {
            let body = error_body(Value::Null, RpcError::new(jsonrpc::INVALID_REQUEST, why));
            respond_json(w, status, &[("Connection", "close")], &body)
        };
        let mut msg = match http_wire::read_head(&mut reader) {
            Ok(m) => m,
            Err(WireError::Closed) | Err(WireError::Io(_)) => return Ok(()),
            Err(WireError::Bad(status, why)) => return bad(&mut writer, status, why),
        };
        // Who is asking, before a byte of the body is read (review 3 #3).
        let admitted = match admit(shared, &msg) {
            Ok(a) => a,
            // The proof, on a request with no body: answered, the connection kept — the
            // bridge sends its request on this same stream (review 4 #3).
            Err(Turned::Keep((status, headers, body))) => {
                let h: Vec<(&str, &str)> = headers.iter().map(|(a, b)| (*a, b.as_str())).collect();
                http_wire::respond(&mut writer, status, &h, &body)?;
                continue;
            }
            Err(Turned::Close((status, headers, body))) => {
                let mut h: Vec<(&str, &str)> =
                    headers.iter().map(|(a, b)| (*a, b.as_str())).collect();
                h.push(("Connection", "close"));
                if !body.is_empty() {
                    h.push(("Content-Type", "application/json"));
                }
                http_wire::respond(&mut writer, status, &h, &body)?;
                linger(&writer);
                return Ok(());
            }
        };
        reader.get_mut().deadline = Instant::now() + shared.body_time;
        match http_wire::read_body(&mut reader, &mut msg, max, Some(&mut writer)) {
            Ok(()) => {}
            Err(WireError::Closed) | Err(WireError::Io(_)) => return Ok(()),
            Err(WireError::Bad(status, why)) => return bad(&mut writer, status, why),
        }
        let close = msg.closes();
        let alive = || still_there(&watch);
        let (status, headers, body) = serve(shared, &msg, &admitted, &alive);
        let mut h: Vec<(&str, &str)> = headers.iter().map(|(a, b)| (*a, b.as_str())).collect();
        if close {
            h.push(("Connection", "close"));
        }
        if body.is_empty() {
            http_wire::respond(&mut writer, status, &h, b"")?;
        } else {
            h.push(("Content-Type", "application/json"));
            http_wire::respond(&mut writer, status, &h, &body)?;
        }
        if close {
            return Ok(());
        }
    }
}

/// Close after a refusal without losing it: our side shut, what the client still sends
/// read and dropped briefly (bounded), so the close is not a reset that discards the
/// answer (RFC 9112 §9.6; review 4 #11).
fn linger(stream: &TcpStream) {
    linger_for(stream, LINGER);
}

/// The longest a refusal's drain holds its slot, whatever the client trickles (review 5
/// #6: bounded in bytes alone, a byte every 150 ms held it for hours).
pub const LINGER: Duration = Duration::from_secs(1);

fn linger_for(stream: &TcpStream, total: Duration) {
    use std::io::Read;
    let _ = stream.shutdown(std::net::Shutdown::Write);
    let end = Instant::now() + total;
    let mut sink = [0u8; 4096];
    let mut left: usize = 64 << 10;
    let mut s = stream;
    while left > 0 {
        let now = Instant::now();
        if now >= end {
            break;
        }
        let wait = (end - now).min(Duration::from_millis(200));
        let _ = s.set_read_timeout(Some(wait.max(Duration::from_millis(1))));
        match s.read(&mut sink) {
            Ok(0) | Err(_) => break,
            Ok(n) => left = left.saturating_sub(n),
        }
    }
}

fn respond_json(
    w: &mut impl Write,
    status: u16,
    h: &[(&str, &str)],
    body: &[u8],
) -> io::Result<()> {
    let mut h = h.to_vec();
    h.push(("Content-Type", "application/json"));
    http_wire::respond(w, status, &h, body)
}

fn error_body(id: Value, e: RpcError) -> Vec<u8> {
    serde_json::to_vec(&jsonrpc::error(id, e)).expect("JSON")
}

type Answer = (u16, Vec<(&'static str, String)>, Vec<u8>);

fn plain(status: u16, message: &str) -> Answer {
    let body = error_body(
        Value::Null,
        RpcError::new(jsonrpc::INVALID_REQUEST, message),
    );
    (status, Vec::new(), body)
}

/// A loopback host name (`Host` without its port, or an `Origin`'s host).
pub fn is_loopback(host: &str) -> bool {
    let h = host.trim().to_ascii_lowercase();
    let h = if let Some(rest) = h.strip_prefix('[') {
        rest.split(']').next().unwrap_or("").to_string()
    } else {
        h.rsplit_once(':')
            .filter(|(_, port)| port.chars().all(|c| c.is_ascii_digit()))
            .map(|(name, _)| name.to_string())
            .unwrap_or(h)
    };
    h == "127.0.0.1" || h == "localhost" || h == "::1"
}

/// An `Origin` a loopback page sends (`http://localhost:5173`): its scheme is http(s) and
/// its host the loopback. `null` and every other origin are refused.
pub fn origin_allowed(origin: &str) -> bool {
    let o = origin.trim().to_ascii_lowercase();
    let Some(rest) = o
        .strip_prefix("http://")
        .or_else(|| o.strip_prefix("https://"))
    else {
        return false;
    };
    !rest.contains('/') && !rest.contains('@') && is_loopback(rest)
}

/// Equal in time whatever the bytes (the token).
fn same(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Who a request is, once let in.
struct Admitted {
    /// The credential its agent goes by (`http:<token>` or `http:<token>#<instance>`).
    credential: String,
}

/// The token a credential was let in with.
fn token_of(credential: &str) -> &str {
    credential.split('#').next().unwrap_or(credential)
}

/// HMAC-SHA-256 (RFC 2104).
pub fn hmac_sha256(key: &[u8], message: &[u8]) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let mut block = [0u8; 64];
    if key.len() > 64 {
        block[..32].copy_from_slice(&Sha256::digest(key));
    } else {
        block[..key.len()].copy_from_slice(key);
    }
    let pad = |byte: u8| block.map(|b| b ^ byte);
    let inner = Sha256::new()
        .chain_update(pad(0x36))
        .chain_update(message)
        .finalize();
    Sha256::new()
        .chain_update(pad(0x5c))
        .chain_update(inner)
        .finalize()
        .into()
}

/// The door's proof that it holds the default token, for a nonce the bridge chose: the
/// bridge sends the token only to a door that answers it (review 3 #7).
pub fn proof(token: &str, nonce: &str) -> String {
    hmac_sha256(
        token.as_bytes(),
        format!("cantus-mcp-proof:{nonce}").as_bytes(),
    )
    .iter()
    .map(|b| format!("{b:02x}"))
    .collect()
}

/// The header a bridge asks the proof with (`Mcp-Cantus-Proof: <nonce>`), and the door
/// answers it in.
pub const PROOF_HEADER: &str = "mcp-cantus-proof";

/// Distinct bridge processes kept apart per token; more share the token's own agent
/// (review 3 #13).
const MAX_INSTANCES: usize = 32;

/// Let a request in, or the answer that turns it away: the loopback `Host`, a loopback
/// `Origin` if any, the endpoint, a proof asked for, the bearer token.
/// How a request is turned away: and the connection closed, or (a proof) kept.
enum Turned {
    Close(Answer),
    Keep(Answer),
}

impl From<Answer> for Turned {
    fn from(a: Answer) -> Self {
        Turned::Close(a)
    }
}

fn admit(shared: &Shared, msg: &Message) -> Result<Admitted, Turned> {
    let path = msg.start.1.split('?').next().unwrap_or("");
    if !msg.header("host").is_some_and(is_loopback) {
        return Err(plain(403, "this door answers only on the loopback (Host)").into());
    }
    if let Some(origin) = msg.header("origin") {
        if !origin_allowed(origin) {
            return Err(plain(403, "the request's Origin is not allowed").into());
        }
    }
    if path != "/mcp" {
        return Err(plain(404, "the MCP endpoint is /mcp").into());
    }
    if let Some(nonce) = msg.header(PROOF_HEADER) {
        let ok = (16..=128).contains(&nonce.len()) && nonce.bytes().all(|b| b.is_ascii_hexdigit());
        if !ok {
            return Err(plain(400, "a proof's nonce is 16 to 128 hex digits").into());
        }
        let default = lock(&shared.tokens)
            .iter()
            .find(|(n, _)| n == "default")
            .map(|(_, t)| t.clone());
        let body_announced = msg
            .header("content-length")
            .is_some_and(|l| l.trim() != "0")
            || msg.header("transfer-encoding").is_some();
        return Err(match default {
            Some(t) if !body_announced => Turned::Keep((
                204,
                vec![("Mcp-Cantus-Proof", proof(&t, nonce))],
                Vec::new(),
            )),
            Some(_) => plain(400, "a proof request carries no body").into(),
            None => plain(404, "no default token").into(),
        });
    }
    let bearer = msg
        .header("authorization")
        .and_then(|a| {
            a.strip_prefix("Bearer ")
                .or_else(|| a.strip_prefix("bearer "))
        })
        .map(str::trim);
    // Every token compared, whichever matches (no early exit on a match's position).
    let named = bearer.and_then(|t| {
        lock(&shared.tokens)
            .iter()
            .fold(None, |found, (name, token)| {
                if same(t.as_bytes(), token.as_bytes()) {
                    Some(name.clone())
                } else {
                    found
                }
            })
    });
    let Some(token) = named else {
        let mut a = plain(
            401,
            "a bearer token is needed: copy it from Cantus's Settings ▸ AI",
        );
        a.1.push(("WWW-Authenticate", "Bearer realm=\"cantus\"".into()));
        return Err(a.into());
    };
    let instance = msg.header(INSTANCE_HEADER).filter(|i| {
        !i.is_empty() && i.len() <= 64 && i.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
    });
    // A bounded number of instances per token: past it, they are the token's own agent.
    let instance = instance.filter(|i| {
        let mut all = lock(&shared.instances);
        let seen = all.entry(token.clone()).or_default();
        let now = Instant::now();
        // Instances unheard of past their idle time give their place up (review 4 #9).
        seen.retain(|(_, at)| now.saturating_duration_since(*at) < shared.instance_idle);
        if let Some(entry) = seen.iter_mut().find(|(s, _)| s == i) {
            entry.1 = now;
            return true;
        }
        if seen.len() < MAX_INSTANCES {
            seen.push((i.to_string(), now));
            return true;
        }
        false
    });
    let credential = match instance {
        Some(i) => format!("http:{token}#{i}"),
        None => format!("http:{token}"),
    };
    Ok(Admitted { credential })
}

impl Admitted {
    /// `http:<token>`: what a session opened by it is bound to.
    fn token_credential(&self) -> &str {
        token_of(&self.credential)
    }
}

fn serve(shared: &Shared, msg: &Message, who: &Admitted, alive: &dyn Fn() -> bool) -> Answer {
    match msg.start.0.as_str() {
        "POST" => post(shared, msg, who, alive),
        "DELETE" => match msg.header("mcp-session-id") {
            None => plain(400, "DELETE needs an Mcp-Session-Id"),
            Some(id) => {
                let mut s = lock(&shared.sessions);
                // A session ends only by the token it was opened with (review 3 #9).
                let mine = s.get(id).is_some_and(|(sess, _)| {
                    token_of(&sess.credential) == token_of(&who.credential)
                });
                if mine {
                    s.remove(id);
                    drop(s);
                    lock(&shared.ended).push(id.to_string());
                    (204, Vec::new(), Vec::new())
                } else {
                    plain(404, "no such session")
                }
            }
        },
        "GET" => {
            let mut a = plain(
                405,
                "Cantus answers every request in its response; it opens no stream",
            );
            a.1.push(("Allow", "POST, DELETE".into()));
            a
        }
        _ => {
            let mut a = plain(405, "the MCP endpoint takes POST");
            a.1.push(("Allow", "POST, DELETE".into()));
            a
        }
    }
}

fn post(shared: &Shared, msg: &Message, who: &Admitted, alive: &dyn Fn() -> bool) -> Answer {
    let json_type = msg
        .header("content-type")
        .is_some_and(|c| c.to_ascii_lowercase().starts_with("application/json"));
    if !json_type {
        return plain(415, "the body must be application/json");
    }
    let header_version = msg.header("mcp-protocol-version").map(str::to_string);
    if let Some(v) = &header_version {
        if !SUPPORTED_VERSIONS.contains(&v.as_str()) {
            let e = RpcError::new(
                jsonrpc::UNSUPPORTED_PROTOCOL_VERSION,
                "Unsupported protocol version",
            )
            .with_data(json!({"supported": SUPPORTED_VERSIONS, "requested": v}));
            return (400, Vec::new(), error_body(Value::Null, e));
        }
    }
    let value: Value = match serde_json::from_slice(&msg.body) {
        Ok(v) => v,
        Err(e) => {
            let body = error_body(
                Value::Null,
                RpcError::new(jsonrpc::PARSE_ERROR, format!("not JSON: {e}")),
            );
            return (400, Vec::new(), body);
        }
    };
    let meta_version = value
        .pointer(&format!(
            "/params/_meta/{}",
            META_VERSION.replace('/', "~1")
        ))
        .and_then(Value::as_str)
        .map(str::to_string);
    if let (Some(h), Some(m)) = (&header_version, &meta_version) {
        if h != m {
            let id = value.get("id").cloned().unwrap_or(Value::Null);
            let e = RpcError::new(-32020, "the MCP-Protocol-Version header and _meta disagree")
                .with_data(json!({"header": h, "meta": m}));
            return (400, Vec::new(), error_body(id, e));
        }
    }
    let is_initialize = value.get("method").and_then(Value::as_str) == Some("initialize");
    let session_header = msg.header("mcp-session-id").map(str::to_string);
    let mut session = match &session_header {
        Some(id) if !is_initialize => {
            let mut s = lock(&shared.sessions);
            match s.get_mut(id) {
                // Only with the token it was opened with (review 3 #9).
                Some((sess, at)) if token_of(&sess.credential) == who.token_credential() => {
                    *at = Instant::now();
                    sess.clone()
                }
                _ => return plain(404, "no such session: initialize again"),
            }
        }
        _ => Session::external(&who.credential),
    };
    let had_id = session.id.clone();
    let answer = shared.server.handle_with(&mut session, value, alive);
    let mut headers = Vec::new();
    if let Some(id) = session.id.clone() {
        if had_id.is_none() {
            headers.push(("Mcp-Session-Id", id.clone()));
            keep(shared, id, session);
        } else if let Some(entry) = lock(&shared.sessions).get_mut(&id) {
            // Still open (a session ended while its request ran stays ended).
            entry.0 = session;
        }
    }
    let Some(answer) = answer else {
        return (202, headers, Vec::new());
    };
    let status = match answer.pointer("/error/code").and_then(Value::as_i64) {
        Some(
            jsonrpc::PARSE_ERROR | jsonrpc::INVALID_REQUEST | jsonrpc::UNSUPPORTED_PROTOCOL_VERSION,
        ) => 400,
        // A request with no version and no session: modern clients probing fall back on 400.
        Some(jsonrpc::INVALID_PARAMS)
            if meta_version.is_none() && had_id.is_none() && !is_initialize =>
        {
            400
        }
        _ => 200,
    };
    (status, headers, serde_json::to_vec(&answer).expect("JSON"))
}

/// Keep a session, the oldest going past the cap; idle ones expire.
fn keep(shared: &Shared, id: String, session: Session) {
    let mut s = lock(&shared.sessions);
    let now = Instant::now();
    let expired: Vec<String> = s
        .iter()
        .filter(|(_, (_, at))| now.saturating_duration_since(*at) > SESSION_IDLE)
        .map(|(k, _)| k.clone())
        .collect();
    for k in &expired {
        s.remove(k);
    }
    s.insert(id, (session, now));
    while s.len() > MAX_SESSIONS {
        let oldest = s
            .iter()
            .min_by_key(|(_, (_, at))| *at)
            .map(|(k, _)| k.clone())
            .expect("not empty");
        s.remove(&oldest);
        lock(&shared.ended).push(oldest);
    }
    drop(s);
    lock(&shared.ended).extend(expired);
}

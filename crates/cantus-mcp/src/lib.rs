//! # cantus-mcp
//!
//! The Model Context Protocol server of Cantus (round 34, ADR 0081), written by hand:
//! JSON-RPC 2.0 over ordinary threads, no async runtime in the application.
//!
//! - [`Server`] speaks the protocol — both generations: the handshake one (2025-06-18,
//!   2025-11-25) and the stateless 2026-07-28 — for every transport.
//! - The [`door`] carries what needs the project to the UI thread and wakes the window.
//! - [`Agents`] is the UI thread's side: permissions by class ([`permissions`]), the
//!   confirmation, one undo step per call with `Origin::Mcp`, changes that group a turn,
//!   jobs on worker threads, the AI activity; [`generic`] holds the server's own tools.
//! - [`client::TestClient`] drives it all end to end in a test, with no socket.
//!
//! Nothing here touches the audio thread; the project is reached only through the
//! application's command bus, on the UI thread (ADRs 0004, 0005).

#![forbid(unsafe_code)]

pub mod agents;
mod agents_call;
mod agents_catalog;
mod agents_generic;
pub mod bridge;
pub mod catalog;
pub mod client;
pub mod door;
pub mod generic;
pub mod home;
pub mod http;
pub mod http_client;
pub mod http_wire;
mod ids;
pub mod jobs;
pub mod jsonrpc;
pub mod permissions;
pub mod schema;
pub mod server;

pub use agents::{Activity, AgentHost, Agents, AiAction, Answer, Ask, Config, Outcome, UndoAll};
pub use agents_catalog::base64;
pub use catalog::Catalog;
pub use door::{Caller, Door, Inbox, ToolAnswer, Waker};
pub use permissions::{Grants, Policy};
pub use server::{Limits, Server, ServerInfo, Session};

/// A server and its pump, joined by a door: `wake` is called whenever a request waits for
/// the UI thread (the application passes `egui::Context::request_repaint`).
pub fn start(info: ServerInfo, catalog: Catalog, wake: Waker) -> (Server, Agents) {
    let (door, inbox) = door::door(wake);
    let server = Server::new(info, catalog, door, Limits::default());
    (server, Agents::new(inbox, catalog))
}

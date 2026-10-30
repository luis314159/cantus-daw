//! # cantus-link
//!
//! Ableton Link in Rust (round 33, ADR 0078): tempo, phase and start/stop shared with every
//! Link app on the local network — discovery over UDP multicast (`224.76.78.75:20808`),
//! the measurement of the session's shared clock by ping and pong, the timeline, start/stop
//! (Link 3) and the merging of sessions, wire-compatible with the Link SDK.
//!
//! **Licence.** The SDK (github.com/Ableton/link) is GPL-2.0-or-later (or proprietary).
//! This crate follows its source and is GPL-3.0-or-later like the application; it never
//! enters the MPL-2.0 engine crates (ADR 0047). The engine sees only its own abstract
//! timeline; the application's bridge (`cantus-ui`'s `sync_link.rs`) copies this crate's
//! [`Session`] into it.
//!
//! The parts, and the SDK files each follows:
//!
//! - [`wire`] — the messages, byte for byte (`discovery/v1/Messages.hpp`,
//!   `discovery/Payload.hpp`, `link/v1/Messages.hpp`, `link/PayloadEntries.hpp`, …).
//! - [`timeline`] — beats, tempo, timelines, the ghost transform and phase (`link/Beats.hpp`,
//!   `Tempo.hpp`, `Timeline.hpp`, `GhostXForm.hpp`, `Phase.hpp`, `Link.ipp`).
//! - [`measure`] — the clock measurement and the ping responder (`Measurement.hpp`,
//!   `PingResponder.hpp`, `MeasurementService.hpp`, `Median.hpp`).
//! - [`node`] — the participant without I/O: peers, sessions, timelines (`Controller.hpp`,
//!   `Sessions.hpp`, `Peers.hpp`, `PeerGateway.hpp`, `UdpMessenger.hpp`).
//! - [`net`] — the sockets (`platforms/asio/Context.hpp`); [`link`] — the handle.

pub mod link;
pub mod measure;
pub mod net;
pub mod node;
pub mod timeline;
pub mod wire;

pub use link::{random_id, Config, Link, Session};
pub use timeline::{Beats, GhostXForm, StartStop, Tempo, Timeline};

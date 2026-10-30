//! A Link node without I/O: what it knows, what it does with each message and each moment,
//! and the messages it wants sent (ADR 0078 §2). The runtime (`net.rs`) feeds it from real
//! sockets; the tests feed it from a simulated network with delays, losses and clocks that
//! disagree — the same code either way.
//!
//! It follows the SDK's `Controller.hpp`, `Sessions.hpp`, `Peers.hpp`,
//! `discovery/PeerGateway.hpp` and `discovery/UdpMessenger.hpp`:
//!
//! - **Discovery.** Every 250 ms (`ttl` 5 s over a ratio of 20), and at most every 50 ms when
//!   the state changes, the node multicasts *alive* with its state on each interface; an
//!   *alive* heard is answered with a unicast *response*; a peer not heard for its time to
//!   live is forgotten; leaving sends *bye-bye*.
//! - **Sessions.** A node founds a session (its own id) with a ghost clock starting at zero.
//!   On seeing a peer of another session it measures that session's ghost clock; the
//!   **older** session — the larger ghost time, by more than 0.5 s, or the smaller id when
//!   within it — wins and the node joins it, taking its timeline. A joined session is
//!   measured again every 30 s. With no peer left, the node founds a new session, the beat
//!   carried over.
//! - **The timeline.** Within a session the timeline with the **larger beat origin** is the
//!   newest and wins; a tempo change makes one past the old origin. **Start/stop** goes by
//!   the newer timestamp.

use crate::measure::{self, Measurement, Step};
use crate::timeline::{Beats, GhostXForm, StartStop, Timeline};
use crate::wire::{self, NodeId, PeerState};
use std::net::{Ipv4Addr, SocketAddrV4};

// What the node does with what it hears, and the edits its owner makes.
#[path = "node_rx.rs"]
mod rx;

/// Link's multicast group and port (`discovery/IpInterface.hpp`).
pub const MULTICAST: SocketAddrV4 = SocketAddrV4::new(Ipv4Addr::new(224, 76, 78, 75), 20808);
/// A peer's time to live, seconds, and the broadcasts per time to live.
pub const TTL: u8 = 5;
pub const TTL_RATIO: i64 = 20;
/// The quickest two broadcasts follow each other, µs.
pub const MIN_BROADCAST_US: i64 = 50_000;
/// Two sessions' ghost clocks closer than this are the same age (`SESSION_EPS`).
pub const SESSION_EPS_US: i64 = 500_000;
/// The most peers a node keeps (by node and interface) and the most other sessions it
/// tracks: a flood of ids is dropped past them (review r33 #10).
pub const MAX_PEERS: usize = 128;
pub const MAX_OTHERS: usize = 32;
/// A joined session is measured again this often.
pub const REMEASURE_US: i64 = 30_000_000;

/// Which socket a message leaves by.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Via {
    /// The interface's discovery sender (multicast and responses).
    Discovery,
    /// The interface's measurement socket (pings and pongs).
    Measure,
}

/// A message to send.
#[derive(Clone, Debug, PartialEq)]
pub struct Send {
    pub iface: usize,
    pub via: Via,
    pub to: SocketAddrV4,
    pub bytes: Vec<u8>,
}

/// A peer, as heard on one interface.
#[derive(Copy, Clone, Debug)]
struct Peer {
    state: PeerState,
    iface: usize,
    expires: i64,
}

/// Another session seen, its newest timeline and its measured clock.
#[derive(Copy, Clone, Debug)]
struct Other {
    id: NodeId,
    timeline: Timeline,
}

/// The node.
#[derive(Debug)]
pub struct Node {
    id: NodeId,
    session: NodeId,
    /// The session's timeline and start/stop, in ghost time.
    timeline: Timeline,
    start_stop: StartStop,
    xform: GhostXForm,
    enabled: bool,
    /// Each interface's measurement endpoint (`mep4`), by interface index.
    ifaces: Vec<Option<SocketAddrV4>>,
    /// Where alive and bye-bye go: Link's group, or a test's port.
    group: SocketAddrV4,
    peers: Vec<Peer>,
    others: Vec<Other>,
    measuring: Vec<Measurement>,
    remeasure_at: Option<i64>,
    last_broadcast: i64,
    next_broadcast: i64,
    out: Vec<Send>,
    /// Session-peer count changes and joins, for the owner to read.
    pub joins: u64,
}

impl Node {
    /// A node with `id` playing at `bpm`, its clock at `now`.
    pub fn new(id: NodeId, bpm: f64, now: i64) -> Node {
        let xform = GhostXForm::founder(now);
        Node {
            id,
            session: id,
            timeline: Timeline {
                tempo: crate::timeline::Tempo::new(bpm),
                beat_origin: Beats::ZERO,
                time_origin: 0,
            }
            .clamped(),
            start_stop: StartStop::default(),
            xform,
            enabled: false,
            ifaces: Vec::new(),
            group: MULTICAST,
            peers: Vec::new(),
            others: Vec::new(),
            measuring: Vec::new(),
            remeasure_at: None,
            last_broadcast: i64::MIN / 2,
            next_broadcast: now,
            out: Vec::new(),
            joins: 0,
        }
    }

    pub fn id(&self) -> NodeId {
        self.id
    }
    pub fn session(&self) -> NodeId {
        self.session
    }
    pub fn timeline(&self) -> Timeline {
        self.timeline
    }
    pub fn start_stop(&self) -> StartStop {
        self.start_stop
    }
    pub fn xform(&self) -> GhostXForm {
        self.xform
    }
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// The interfaces it speaks on, each with its measurement endpoint.
    pub fn set_interfaces(&mut self, ifaces: Vec<Option<SocketAddrV4>>) {
        self.peers.retain(|p| p.iface < ifaces.len());
        self.ifaces = ifaces;
        self.next_broadcast = i64::MIN / 2;
    }

    /// Interface `i` (opened by a scan) answers pings at `mep`.
    pub fn set_interface(&mut self, i: usize, mep: Option<SocketAddrV4>) {
        if self.ifaces.len() <= i {
            self.ifaces.resize(i + 1, None);
        }
        self.ifaces[i] = mep;
        self.next_broadcast = i64::MIN / 2;
    }

    /// Interface `i` is gone: the peers heard on it forgotten (a peer heard on another
    /// interface too stays).
    pub fn drop_interface(&mut self, i: usize, now: i64) {
        if let Some(m) = self.ifaces.get_mut(i) {
            *m = None;
        }
        let before = self.peer_count();
        self.peers.retain(|p| p.iface != i);
        self.measuring.retain(|m| m.iface != i);
        self.membership_changed(before, now);
    }

    /// Multicast to `group` instead of Link's (a test's own port).
    pub fn set_group(&mut self, group: SocketAddrV4) {
        self.group = group;
    }

    /// How many peers and other sessions it keeps (the tests' caps).
    #[doc(hidden)]
    pub fn state_sizes(&self) -> (usize, usize) {
        (self.peers.len(), self.others.len())
    }

    /// How many other nodes share its session (`uniqueSessionPeerCount`).
    pub fn peer_count(&self) -> usize {
        // Without a heap: a node heard on two interfaces counts once (at most `MAX_PEERS`).
        let ours = |p: &Peer| p.state.session == self.session;
        self.peers
            .iter()
            .enumerate()
            .filter(|(i, p)| {
                ours(p)
                    && !self.peers[..*i]
                        .iter()
                        .any(|q| ours(q) && q.state.node == p.state.node)
            })
            .count()
    }

    /// The session's beat at host time `host`.
    pub fn beat_at(&self, host: i64) -> Beats {
        self.timeline.beats_at(self.xform.host_to_ghost(host))
    }

    /// The messages it wants sent, oldest first.
    pub fn take_out(&mut self) -> Vec<Send> {
        std::mem::take(&mut self.out)
    }

    /// Start or stop taking part. Enabling founds a new session, the beat carried over
    /// (`updateEnabled` → `resetState`); disabling says bye-bye.
    pub fn enable(&mut self, on: bool, new_id: NodeId, now: i64) {
        if on == self.enabled {
            return;
        }
        if on {
            self.enabled = true;
            self.reset(new_id, now);
        } else {
            self.byebye();
            self.enabled = false;
            self.peers.clear();
            self.others.clear();
            self.measuring.clear();
        }
    }

    fn byebye(&mut self) {
        let state = self.peer_state(0);
        for i in 0..self.ifaces.len() {
            if let Some(b) = wire::encode_discovery(wire::BYEBYE, 0, &state).finish() {
                self.out.push(Send {
                    iface: i,
                    via: Via::Discovery,
                    to: self.group,
                    bytes: b.to_vec(),
                });
            }
        }
    }

    /// Found a session of its own (`resetState`), the beat at `now` kept.
    fn reset(&mut self, new_id: NodeId, now: i64) {
        log::debug!("link: {:02x?} founds a session as {:02x?}", self.id, new_id);
        let beat = self.beat_at(now);
        self.id = new_id;
        self.session = new_id;
        self.xform = GhostXForm::founder(now);
        self.timeline = Timeline {
            tempo: self.timeline.tempo,
            beat_origin: beat,
            time_origin: self.xform.host_to_ghost(now),
        };
        self.start_stop = StartStop::default();
        self.peers.clear();
        self.others.clear();
        self.measuring.clear();
        self.remeasure_at = None;
        self.changed();
    }

    /// Its state changed: broadcast as soon as the 50 ms rule lets it.
    fn changed(&mut self) {
        self.next_broadcast = self
            .next_broadcast
            .min(self.last_broadcast + MIN_BROADCAST_US);
    }

    fn peer_state(&self, iface: usize) -> PeerState {
        PeerState {
            node: self.id,
            session: self.session,
            timeline: self.timeline,
            start_stop: self.start_stop,
            measurement: self.ifaces.get(iface).copied().flatten(),
        }
    }

    fn send_state(&mut self, kind: u8, iface: usize, to: SocketAddrV4) {
        let state = self.peer_state(iface);
        if let Some(b) = wire::encode_discovery(kind, TTL, &state).finish() {
            self.out.push(Send {
                iface,
                via: Via::Discovery,
                to,
                bytes: b.to_vec(),
            });
        }
    }

    /// Time passes: broadcasts, timeouts, the peers whose time ran out, measurements again.
    pub fn tick(&mut self, now: i64) {
        if !self.enabled {
            return;
        }
        if now >= self.next_broadcast {
            for i in 0..self.ifaces.len() {
                self.send_state(wire::ALIVE, i, self.group);
            }
            self.last_broadcast = now;
            self.next_broadcast = now + i64::from(TTL) * 1_000_000 / TTL_RATIO;
        }
        let before = self.peer_count();
        let n = self.peers.len();
        self.peers.retain(|p| p.expires >= now);
        if self.peers.len() != n {
            // A session nobody is heard in any more is forgotten with its last peer.
            let peers = &self.peers;
            self.others
                .retain(|o| peers.iter().any(|p| p.state.session == o.id));
            self.membership_changed(before, now);
        }
        let mut i = 0;
        while i < self.measuring.len() {
            match self.measuring[i].tick(now) {
                Step::Ping(bytes) => {
                    let m = &self.measuring[i];
                    self.out.push(Send {
                        iface: m.iface,
                        via: Via::Measure,
                        to: m.endpoint,
                        bytes,
                    });
                    i += 1;
                }
                Step::Failed => {
                    let m = self.measuring.swap_remove(i);
                    self.measured(m.session, None, now);
                }
                _ => i += 1,
            }
        }
        if let Some(at) = self.remeasure_at {
            if now >= at {
                self.remeasure_at = Some(now + REMEASURE_US);
                self.measure_session(self.session, now);
            }
        }
    }

    fn membership_changed(&mut self, before: usize, now: i64) {
        let after = self.peer_count();
        if before > 0 && after == 0 {
            // Alone again: a session of its own (`SessionPeerCounter`).
            let id = derive_id(&self.id, now);
            self.reset(id, now);
        }
        if before != after {
            self.joins += 1;
        }
    }
}

/// A fresh id from an old one and the time, when the runtime gives none (deterministic,
/// so a simulated network replays).
pub fn derive_id(old: &NodeId, now: i64) -> NodeId {
    let mut x = u64::from_be_bytes(*old) ^ (now as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    x ^= x >> 33;
    x = x.wrapping_mul(0xff51_afd7_ed55_8ccd);
    x ^= x >> 33;
    x.to_be_bytes()
}

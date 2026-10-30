//! Link's wire format, byte for byte as the SDK writes it (ADR 0078 §2).
//!
//! Every number is big-endian (`discovery/NetworkByteStreamSerializable.hpp`). A message is
//! an eight-byte protocol header, a message header and a **payload**: a run of entries, each
//! a four-character key (`u32`), a size (`u32`) and that many bytes (`discovery/Payload.hpp`);
//! a reader skips the keys it does not know, so a newer peer's extra entries (Link Audio's
//! `aep4`/`aep6`) cost nothing.
//!
//! - **Discovery** (`discovery/v1/Messages.hpp`): `_asdp_v\x01`, then the type (alive 1,
//!   response 2, bye-bye 3), the time to live in seconds, a group id (always 0) and the
//!   sender's eight-byte node id. Its payload is the node's state (`link/NodeState.hpp`,
//!   `link/PeerState.hpp`): `tmln` (tempo as µs per beat, beat origin in µbeats, time
//!   origin in µs: three `i64`), `sess` (the session id, eight bytes), `stst` (playing as
//!   one byte, beats `i64`, timestamp `i64`) and `mep4` (the measurement endpoint: an IPv4
//!   address `u32` and a port `u16`).
//! - **Measurement** (`link/v1/Messages.hpp`, `link/PayloadEntries.hpp`): `_link_v\x01`,
//!   then the type (ping 1, pong 2) and a payload of `__ht` (host time), `__gt` (ghost
//!   time), `_pgt` (the previous ghost time) and `sess`, each an `i64` of µs but the last.

use crate::timeline::{Beats, StartStop, Tempo, Timeline};
use std::net::{Ipv4Addr, SocketAddrV4};

/// The largest message either protocol sends (`kMaxMessageSize`).
pub const MAX_MESSAGE: usize = 512;
/// Discovery's protocol header.
pub const DISCOVERY_HEADER: [u8; 8] = *b"_asdp_v\x01";
/// Measurement's protocol header.
pub const LINK_HEADER: [u8; 8] = *b"_link_v\x01";

/// Discovery's message types.
pub const ALIVE: u8 = 1;
pub const RESPONSE: u8 = 2;
pub const BYEBYE: u8 = 3;
/// Measurement's.
pub const PING: u8 = 1;
pub const PONG: u8 = 2;

/// The payload keys (four characters, big-endian).
pub const KEY_TIMELINE: u32 = u32::from_be_bytes(*b"tmln");
pub const KEY_SESSION: u32 = u32::from_be_bytes(*b"sess");
pub const KEY_START_STOP: u32 = u32::from_be_bytes(*b"stst");
pub const KEY_MEP4: u32 = u32::from_be_bytes(*b"mep4");
pub const KEY_HOST_TIME: u32 = u32::from_be_bytes(*b"__ht");
pub const KEY_GHOST_TIME: u32 = u32::from_be_bytes(*b"__gt");
pub const KEY_PREV_GHOST_TIME: u32 = u32::from_be_bytes(*b"_pgt");

/// A node's (or a session's) id: eight random bytes (`link/NodeId.hpp`). A session is
/// named by the id of the node that founded it.
pub type NodeId = [u8; 8];

/// Why a message was not read.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum WireError {
    /// Not one of ours (another protocol, or a version we do not speak).
    Header,
    /// Cut short, or an entry's size disagrees with its key.
    Malformed,
}

/// A writer into a fixed message buffer: what does not fit is refused, never truncated.
#[derive(Debug)]
pub struct Writer {
    buf: [u8; MAX_MESSAGE],
    len: usize,
    overflow: bool,
}

impl Default for Writer {
    fn default() -> Self {
        Writer {
            buf: [0; MAX_MESSAGE],
            len: 0,
            overflow: false,
        }
    }
}

impl Writer {
    pub fn bytes(&mut self, b: &[u8]) {
        if self.len + b.len() > MAX_MESSAGE {
            self.overflow = true;
            return;
        }
        self.buf[self.len..self.len + b.len()].copy_from_slice(b);
        self.len += b.len();
    }
    pub fn u8(&mut self, v: u8) {
        self.bytes(&[v]);
    }
    pub fn u16(&mut self, v: u16) {
        self.bytes(&v.to_be_bytes());
    }
    pub fn u32(&mut self, v: u32) {
        self.bytes(&v.to_be_bytes());
    }
    pub fn i64(&mut self, v: i64) {
        self.bytes(&v.to_be_bytes());
    }
    /// One payload entry: key, size, value.
    pub fn entry(&mut self, key: u32, value: &[u8]) {
        self.u32(key);
        self.u32(value.len() as u32);
        self.bytes(value);
    }
    /// The message, or `None` if it outgrew [`MAX_MESSAGE`] (the SDK throws there).
    pub fn finish(&self) -> Option<&[u8]> {
        (!self.overflow && self.len < MAX_MESSAGE).then(|| &self.buf[..self.len])
    }
}

/// A reader over a received message.
#[derive(Debug)]
pub struct Reader<'a> {
    b: &'a [u8],
}

impl<'a> Reader<'a> {
    pub fn new(b: &'a [u8]) -> Self {
        Reader { b }
    }
    pub fn take(&mut self, n: usize) -> Result<&'a [u8], WireError> {
        if self.b.len() < n {
            return Err(WireError::Malformed);
        }
        let (h, t) = self.b.split_at(n);
        self.b = t;
        Ok(h)
    }
    pub fn u8(&mut self) -> Result<u8, WireError> {
        Ok(self.take(1)?[0])
    }
    pub fn u16(&mut self) -> Result<u16, WireError> {
        let s = self.take(2)?;
        Ok(u16::from_be_bytes([s[0], s[1]]))
    }
    pub fn u32(&mut self) -> Result<u32, WireError> {
        let s = self.take(4)?;
        Ok(u32::from_be_bytes([s[0], s[1], s[2], s[3]]))
    }
    pub fn i64(&mut self) -> Result<i64, WireError> {
        let s = self.take(8)?;
        let mut a = [0u8; 8];
        a.copy_from_slice(s);
        Ok(i64::from_be_bytes(a))
    }
    pub fn id(&mut self) -> Result<NodeId, WireError> {
        let s = self.take(8)?;
        let mut a = [0u8; 8];
        a.copy_from_slice(s);
        Ok(a)
    }
    pub fn is_empty(&self) -> bool {
        self.b.is_empty()
    }
    pub fn rest(&self) -> &'a [u8] {
        self.b
    }
}

/// Each entry of a payload in turn, `(key, value)`; an entry running past the end is an
/// error (`Payload with incorrect size`).
pub fn entries(payload: &[u8]) -> impl Iterator<Item = Result<(u32, &[u8]), WireError>> {
    let mut r = Reader::new(payload);
    let mut failed = false;
    std::iter::from_fn(move || {
        if failed || r.is_empty() {
            return None;
        }
        let e = (|| {
            let key = r.u32()?;
            let size = r.u32()? as usize;
            Ok((key, r.take(size)?))
        })();
        failed = e.is_err();
        Some(e)
    })
}

fn timeline_bytes(t: &Timeline) -> [u8; 24] {
    let mut v = [0u8; 24];
    v[..8].copy_from_slice(&t.tempo.micros_per_beat().to_be_bytes());
    v[8..16].copy_from_slice(&t.beat_origin.micro().to_be_bytes());
    v[16..].copy_from_slice(&t.time_origin.to_be_bytes());
    v
}

fn read_timeline(v: &[u8]) -> Result<Timeline, WireError> {
    let mut r = Reader::new(v);
    let mpb = r.i64()?;
    let beat = r.i64()?;
    let time = r.i64()?;
    if !r.is_empty() || mpb <= 0 {
        return Err(WireError::Malformed);
    }
    Ok(Timeline {
        tempo: Tempo::from_micros_per_beat(mpb),
        beat_origin: Beats::from_micro(beat),
        time_origin: time,
    })
}

/// What a node tells the others about itself (`PeerState`).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct PeerState {
    pub node: NodeId,
    pub session: NodeId,
    pub timeline: Timeline,
    pub start_stop: StartStop,
    /// Where it answers pings (`mep4`); `None` from a peer that sent none.
    pub measurement: Option<SocketAddrV4>,
}

/// A discovery message as read.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Discovery {
    pub kind: u8,
    pub ttl: u8,
    pub group: u16,
    pub node: NodeId,
    /// The state, for alive and response (bye-bye carries none).
    pub state: Option<PeerState>,
}

/// Encode a discovery message: `kind` with `ttl` seconds, from `state.node`; a bye-bye
/// carries an empty payload and a zero time to live, as the SDK's.
pub fn encode_discovery(kind: u8, ttl: u8, state: &PeerState) -> Writer {
    let mut w = Writer::default();
    w.bytes(&DISCOVERY_HEADER);
    w.u8(kind);
    w.u8(if kind == BYEBYE { 0 } else { ttl });
    w.u16(0);
    w.bytes(&state.node);
    if kind != BYEBYE {
        w.entry(KEY_TIMELINE, &timeline_bytes(&state.timeline));
        w.entry(KEY_SESSION, &state.session);
        let mut ss = [0u8; 17];
        ss[0] = u8::from(state.start_stop.playing);
        ss[1..9].copy_from_slice(&state.start_stop.beats.micro().to_be_bytes());
        ss[9..].copy_from_slice(&state.start_stop.timestamp.to_be_bytes());
        w.entry(KEY_START_STOP, &ss);
        if let Some(ep) = state.measurement {
            let mut m = [0u8; 6];
            m[..4].copy_from_slice(&u32::from(*ep.ip()).to_be_bytes());
            m[4..].copy_from_slice(&ep.port().to_be_bytes());
            w.entry(KEY_MEP4, &m);
        }
    }
    w
}

/// Read a discovery message.
pub fn decode_discovery(b: &[u8]) -> Result<Discovery, WireError> {
    if b.len() < 20 || b[..8] != DISCOVERY_HEADER {
        return Err(WireError::Header);
    }
    let mut r = Reader::new(&b[8..]);
    let kind = r.u8()?;
    let ttl = r.u8()?;
    let group = r.u16()?;
    let node = r.id()?;
    if kind == BYEBYE {
        return Ok(Discovery {
            kind,
            ttl,
            group,
            node,
            state: None,
        });
    }
    let mut state = PeerState {
        node,
        session: [0; 8],
        timeline: Timeline::default(),
        start_stop: StartStop::default(),
        measurement: None,
    };
    for e in entries(r.rest()) {
        let (key, v) = e?;
        match key {
            KEY_TIMELINE => state.timeline = read_timeline(v)?,
            KEY_SESSION => {
                if v.len() != 8 {
                    return Err(WireError::Malformed);
                }
                state.session.copy_from_slice(v);
            }
            KEY_START_STOP => {
                let mut r = Reader::new(v);
                let playing = r.u8()? != 0;
                let beats = Beats::from_micro(r.i64()?);
                let timestamp = r.i64()?;
                if !r.is_empty() {
                    return Err(WireError::Malformed);
                }
                state.start_stop = StartStop {
                    playing,
                    beats,
                    timestamp,
                };
            }
            KEY_MEP4 => {
                let mut r = Reader::new(v);
                let ip = Ipv4Addr::from(r.u32()?);
                let port = r.u16()?;
                if !r.is_empty() {
                    return Err(WireError::Malformed);
                }
                state.measurement = Some(SocketAddrV4::new(ip, port));
            }
            _ => {}
        }
    }
    Ok(Discovery {
        kind,
        ttl,
        group,
        node,
        state: Some(state),
    })
}

/// A measurement message as read: its type and the entries it carried (0 when absent, as
/// the SDK's defaults).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Measure {
    pub kind: u8,
    pub session: Option<NodeId>,
    pub host_time: i64,
    pub ghost_time: i64,
    pub prev_ghost_time: i64,
    /// The payload's bytes after the header (a ping's, echoed in the pong).
    pub payload_len: usize,
}

/// A ping: the host time now and, after the first answer, the ghost time it brought.
pub fn encode_ping(host_time: i64, prev_ghost: Option<i64>) -> Writer {
    let mut w = Writer::default();
    w.bytes(&LINK_HEADER);
    w.u8(PING);
    w.entry(KEY_HOST_TIME, &host_time.to_be_bytes());
    if let Some(g) = prev_ghost {
        w.entry(KEY_PREV_GHOST_TIME, &g.to_be_bytes());
    }
    w
}

/// The pong to a ping whose payload is `ping_payload`: the session, the ghost time now,
/// then the ping's own entries echoed (`PingResponder::reply`).
pub fn encode_pong(session: &NodeId, ghost_now: i64, ping_payload: &[u8]) -> Writer {
    let mut w = Writer::default();
    w.bytes(&LINK_HEADER);
    w.u8(PONG);
    w.entry(KEY_SESSION, session);
    w.entry(KEY_GHOST_TIME, &ghost_now.to_be_bytes());
    w.bytes(ping_payload);
    w
}

/// The largest ping payload a responder answers: a host time and a previous ghost time.
pub const MAX_PING_PAYLOAD: usize = 32;

/// Read a measurement message; its payload (for an echo) is `&b[9..]`.
pub fn decode_measure(b: &[u8]) -> Result<Measure, WireError> {
    if b.len() < 9 || b[..8] != LINK_HEADER {
        return Err(WireError::Header);
    }
    let mut m = Measure {
        kind: b[8],
        payload_len: b.len() - 9,
        ..Measure::default()
    };
    for e in entries(&b[9..]) {
        let (key, v) = e?;
        let int = || -> Result<i64, WireError> {
            let mut r = Reader::new(v);
            let x = r.i64()?;
            if r.is_empty() {
                Ok(x)
            } else {
                Err(WireError::Malformed)
            }
        };
        match key {
            KEY_HOST_TIME => m.host_time = int()?,
            KEY_GHOST_TIME => m.ghost_time = int()?,
            KEY_PREV_GHOST_TIME => m.prev_ghost_time = int()?,
            KEY_SESSION => {
                if v.len() != 8 {
                    return Err(WireError::Malformed);
                }
                let mut id = [0u8; 8];
                id.copy_from_slice(v);
                m.session = Some(id);
            }
            _ => {}
        }
    }
    Ok(m)
}

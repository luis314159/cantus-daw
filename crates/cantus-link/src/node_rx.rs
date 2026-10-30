//! The node's ears and hands: what it does with each message it hears, and the edits its
//! owner makes to the session (a tempo, a start or stop, the beat at a time).

use super::*;

impl Node {
    /// A message on interface `iface`'s discovery sockets, from `from`.
    pub fn on_discovery(&mut self, iface: usize, from: SocketAddrV4, bytes: &[u8], now: i64) {
        if !self.enabled {
            return;
        }
        let Ok(d) = wire::decode_discovery(bytes) else {
            return;
        };
        if d.node == self.id || d.group != 0 {
            return;
        }
        match d.kind {
            wire::ALIVE => {
                self.send_state(wire::RESPONSE, iface, from);
                if let Some(s) = d.state {
                    self.saw_peer(s, iface, d.ttl, now);
                }
            }
            wire::RESPONSE => {
                if let Some(s) = d.state {
                    self.saw_peer(s, iface, d.ttl, now);
                }
            }
            wire::BYEBYE => {
                let before = self.peer_count();
                self.peers
                    .retain(|p| !(p.state.node == d.node && p.iface == iface));
                self.membership_changed(before, now);
            }
            _ => {}
        }
    }

    /// A peer's state (`Peers::sawPeerOnGateway` and the controller's callbacks).
    fn saw_peer(&mut self, state: PeerState, iface: usize, ttl: u8, now: i64) {
        if !self.sane(&state, now) {
            return;
        }
        let before = self.peer_count();
        let expires = now.saturating_add(i64::from(ttl) * 1_000_000);
        let full = self.peers.len() >= MAX_PEERS;
        match self
            .peers
            .iter_mut()
            .find(|p| p.state.node == state.node && p.iface == iface)
        {
            Some(p) => {
                p.state = state;
                p.expires = expires;
            }
            // A flood of new ids stops at the cap (review r33 #10).
            None if full => return,
            None => self.peers.push(Peer {
                state,
                iface,
                expires,
            }),
        }
        if state.session == self.session {
            // The newest timeline of our session wins (`Sessions::updateTimeline`).
            if state.timeline.beat_origin > self.timeline.beat_origin {
                self.timeline = state.timeline.clamped();
                self.changed();
            }
            if state.start_stop.timestamp > self.start_stop.timestamp {
                self.start_stop = state.start_stop;
                self.changed();
            }
        } else {
            let room = self.others.len() < MAX_OTHERS;
            match self.others.iter_mut().find(|o| o.id == state.session) {
                Some(o) => {
                    if state.timeline.beat_origin > o.timeline.beat_origin {
                        o.timeline = state.timeline;
                    }
                }
                // Only a session it can measure (a peer said where it answers pings), and
                // no more than the cap: one that never can would stay for good.
                None if state.measurement.is_some() && room => {
                    self.others.push(Other {
                        id: state.session,
                        timeline: state.timeline,
                    });
                    self.measure_session(state.session, now);
                }
                None => {}
            }
        }
        self.membership_changed(before, now);
    }

    /// Whether a peer's state is one to believe: its numbers within 2^53 (beats, times —
    /// what a double holds exactly), and a start/stop of our session not stamped past a
    /// minute from now (one stamped at the end of time froze it — review r33 #10).
    fn sane(&self, s: &PeerState, now: i64) -> bool {
        const LIMIT: u64 = 1 << 53;
        let small = |x: i64| x.unsigned_abs() < LIMIT;
        if !(small(s.timeline.beat_origin.micro())
            && small(s.timeline.time_origin)
            && small(s.start_stop.beats.micro())
            && small(s.start_stop.timestamp))
        {
            return false;
        }
        if !s.timeline.tempo.bpm().is_finite() {
            return false;
        }
        if s.session == self.session {
            let ghost_now = self.xform.host_to_ghost(now);
            if s.start_stop.timestamp > ghost_now.saturating_add(60_000_000) {
                return false;
            }
        }
        true
    }

    /// Measure `session`'s clock through one of its peers: its founder if heard, else the
    /// first (`launchSessionMeasurement`).
    pub(super) fn measure_session(&mut self, session: NodeId, now: i64) {
        if self.measuring.iter().any(|m| m.session == session) {
            return;
        }
        let peer = self
            .peers
            .iter()
            .filter(|p| p.state.session == session && p.state.measurement.is_some())
            .min_by_key(|p| p.state.node != session)
            .copied();
        let Some(p) = peer else {
            return;
        };
        let Some(ep) = p.state.measurement else {
            return;
        };
        let (m, ping) = Measurement::start(session, p.state.node, ep, p.iface, now);
        self.out.push(Send {
            iface: p.iface,
            via: Via::Measure,
            to: ep,
            bytes: ping,
        });
        self.measuring.push(m);
    }

    /// A message on interface `iface`'s measurement socket.
    pub fn on_measure(&mut self, iface: usize, from: SocketAddrV4, bytes: &[u8], now: i64) {
        if !self.enabled {
            return;
        }
        let Ok(m) = wire::decode_measure(bytes) else {
            return;
        };
        if m.kind == wire::PING {
            let ghost = self.xform.host_to_ghost(now);
            if let Some(pong) = measure::answer(bytes, &self.session, ghost) {
                self.out.push(Send {
                    iface,
                    via: Via::Measure,
                    to: from,
                    bytes: pong,
                });
            }
            return;
        }
        if m.kind != wire::PONG {
            return;
        }
        let Some(i) = self.measuring.iter().position(|x| x.endpoint == from) else {
            return;
        };
        match self.measuring[i].pong(&m, now) {
            Step::Ping(bytes) => self.out.push(Send {
                iface,
                via: Via::Measure,
                to: from,
                bytes,
            }),
            Step::Done(intercept) => {
                let done = self.measuring.swap_remove(i);
                self.measured(done.session, Some(intercept), now);
            }
            Step::Failed => {
                let done = self.measuring.swap_remove(i);
                self.measured(done.session, None, now);
            }
            Step::Wait => {}
        }
    }

    /// A measurement ended (`Sessions::handleSuccessfulMeasurement` / `…Failed…`).
    pub(super) fn measured(&mut self, session: NodeId, intercept: Option<i64>, now: i64) {
        let Some(intercept) = intercept else {
            if session == self.session {
                self.remeasure_at = Some(now.saturating_add(REMEASURE_US));
            } else {
                self.others.retain(|o| o.id != session);
                self.peers.retain(|p| p.state.session != session);
            }
            return;
        };
        // A peer's pongs set this number: an absurd one (past 2^53 µs, centuries) is a
        // failed measurement, never a clock (round 2, D3: it overflowed, and could be joined
        // or become our own).
        if intercept.unsigned_abs() >= 1 << 53 {
            return self.measured(session, None, now);
        }
        let xform = GhostXForm {
            slope: 1.0,
            intercept,
        };
        if session == self.session {
            if xform != self.xform {
                self.xform = xform;
                self.changed();
            }
            return;
        }
        let Some(k) = self.others.iter().position(|o| o.id == session) else {
            return;
        };
        let cur = self.xform.host_to_ghost(now);
        let new = xform.host_to_ghost(now);
        let diff = new.saturating_sub(cur);
        if diff > SESSION_EPS_US
            || (diff.unsigned_abs() < SESSION_EPS_US as u64 && session < self.session)
        {
            // The other session is older: join it, with its timeline.
            log::debug!(
                "link: {:02x?} joins {:02x?} (diff {diff} µs)",
                self.id,
                session
            );
            let other = self.others.swap_remove(k);
            let before = self.peer_count();
            self.others.push(Other {
                id: self.session,
                timeline: self.timeline,
            });
            self.session = other.id;
            self.timeline = other.timeline.clamped();
            self.xform = xform;
            self.start_stop = StartStop::default();
            // The newest start/stop of the session we join, as its peers tell it.
            for p in &self.peers {
                if p.state.session == self.session
                    && p.state.start_stop.timestamp > self.start_stop.timestamp
                {
                    self.start_stop = p.state.start_stop;
                }
            }
            self.remeasure_at = Some(now + REMEASURE_US);
            self.joins += 1;
            self.changed();
            self.membership_changed(before.max(1), now);
        }
    }

    // ---- the owner's edits --------------------------------------------------------------

    /// A new tempo from `at` (host µs) on: a new timeline past the old origin.
    pub fn set_tempo(&mut self, bpm: f64, at: i64) {
        let g = self.xform.host_to_ghost(at);
        let tl = crate::timeline::retempo(&self.timeline, bpm, g);
        if tl != self.timeline {
            self.timeline = tl;
            self.changed();
        }
    }

    /// Start or stop the session at host time `at`, decided `now`.
    pub fn set_playing(&mut self, playing: bool, at: i64, now: i64) {
        let ss = StartStop {
            playing,
            beats: self.timeline.beats_at(self.xform.host_to_ghost(at)),
            timestamp: self.xform.host_to_ghost(now),
        };
        if ss.timestamp > self.start_stop.timestamp {
            self.start_stop = ss;
            self.changed();
        }
    }

    /// Alone, make the session's `beat` fall at host time `at` (`requestBeatAtTime` with
    /// nobody to respect: the beat is forced, the phase moving with it); the new timeline's
    /// origin is past the old one, so a peer that comes later adopts it. With others in the
    /// session nothing changes: phase within any `_quantum` stays theirs.
    pub fn request_beat_at(&mut self, beat: f64, at: i64, _quantum: f64, now: i64) {
        let want = Beats::new(beat);
        let tl = self.timeline;
        let g = self.xform.host_to_ghost(at);
        // Phase is the session's own, counted from its beat 0 — what the SDK's client
        // timeline counts from (review r33 #5: counted from the beat origin, a tempo change
        // put the requested beat 1.7 beats off).
        if self.peer_count() > 0 {
            // Others play: the timeline is theirs too. The SDK renumbers only the asking
            // client's own beats then, which this node's owner (it maps phase itself) has
            // no use for — so nothing moves (round 2, D7: renumbering by our quantum would
            // shift a peer on another quantum).
            return;
        }
        // Alone: `beat` falls at `at`, the phase moving with it.
        let (beat_at, time) = (want, g);
        // The same line through (`beat_at`, `time`), its origin past the old one so every
        // peer adopts it.
        let through = Timeline {
            tempo: tl.tempo,
            beat_origin: beat_at,
            time_origin: time,
        };
        let gnow = self.xform.host_to_ghost(now);
        let origin = through
            .beats_at(gnow)
            .max(tl.beat_origin + Beats::from_micro(1));
        let line = Timeline {
            tempo: tl.tempo,
            beat_origin: origin,
            time_origin: through.time_of(origin),
        };
        if line != self.timeline {
            self.timeline = line;
            self.changed();
        }
    }
}

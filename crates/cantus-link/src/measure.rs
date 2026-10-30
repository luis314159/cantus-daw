//! Measuring a session's clock (`link/Measurement.hpp`, `link/PingResponder.hpp`,
//! `link/MeasurementService.hpp`, `link/Median.hpp`).
//!
//! The measurer pings a peer of the session with its host time; the peer answers with the
//! session's ghost time and the ping echoed; the measurer pings again at once with the
//! ghost time it got. Each answer gives up to two estimates of *ghost − host*:
//! `gt − (ht_now + ht_sent)/2` and `(gt + gt_prev)/2 − ht_sent`. After more than 100 the
//! median is the intercept of the session's ghost transform (slope 1). A ping that gets no
//! answer in 50 ms is sent again, five times in a row at most; then the measurement fails.
//! (The SDK counts the five over the whole measurement, so on a network losing a few per
//! cent of packets nearly every measurement of its ~150 round trips fails; counting them
//! in a row is the one place this crate is kinder than the SDK, and it changes nothing on
//! the wire.)

use crate::wire::{self, NodeId};
use std::net::SocketAddrV4;

/// Estimates a measurement collects before it finishes (`kNumberDataPoints`).
pub const DATA_POINTS: usize = 100;
/// Pings sent again on silence before a measurement fails (`kNumberMeasurements`).
pub const RETRIES: usize = 5;
/// How long a ping waits for its pong, µs.
pub const PING_TIMEOUT_US: i64 = 50_000;

/// One measurement under way.
#[derive(Clone, Debug)]
pub struct Measurement {
    /// The session being measured, and the peer pinged for it.
    pub session: NodeId,
    pub peer: NodeId,
    pub endpoint: SocketAddrV4,
    /// The interface the peer was seen on (the socket the pings leave by).
    pub iface: usize,
    data: Vec<f64>,
    retries: usize,
    deadline: i64,
}

/// What a measurement step asks for.
#[derive(Clone, Debug, PartialEq)]
pub enum Step {
    /// Send this ping to the endpoint.
    Ping(Vec<u8>),
    /// Done: the session's ghost transform intercept (ghost − host, µs).
    Done(i64),
    /// Failed (silence, or a pong from another session).
    Failed,
    /// Nothing to do.
    Wait,
}

impl Measurement {
    /// Start measuring `session` through `peer` at `endpoint`: the first ping.
    pub fn start(
        session: NodeId,
        peer: NodeId,
        endpoint: SocketAddrV4,
        iface: usize,
        now: i64,
    ) -> (Measurement, Vec<u8>) {
        let m = Measurement {
            session,
            peer,
            endpoint,
            iface,
            data: Vec::with_capacity(DATA_POINTS + 2),
            retries: 0,
            deadline: now + PING_TIMEOUT_US,
        };
        let ping = wire::encode_ping(now, None);
        (m, ping.finish().map(<[u8]>::to_vec).unwrap_or_default())
    }

    /// The time now has passed: a ping again, or the end.
    pub fn tick(&mut self, now: i64) -> Step {
        if now < self.deadline {
            return Step::Wait;
        }
        if self.retries < RETRIES {
            self.retries += 1;
            self.deadline = now + PING_TIMEOUT_US;
            let ping = wire::encode_ping(now, None);
            return Step::Ping(ping.finish().map(<[u8]>::to_vec).unwrap_or_default());
        }
        Step::Failed
    }

    /// A pong came: an estimate or two, and the next ping (or the result).
    pub fn pong(&mut self, m: &wire::Measure, now: i64) -> Step {
        if m.session != Some(self.session) {
            return Step::Failed;
        }
        let (gt, pgt, ht) = (m.ghost_time, m.prev_ghost_time, m.host_time);
        let next = wire::encode_ping(now, Some(gt))
            .finish()
            .map(<[u8]>::to_vec)
            .unwrap_or_default();
        // In floating point: a peer's times are not to be trusted to stay in range.
        if gt != 0 && ht != 0 {
            self.data.push(gt as f64 - (now as f64 + ht as f64) * 0.5);
            if pgt != 0 {
                self.data.push((gt as f64 + pgt as f64) * 0.5 - ht as f64);
            }
        }
        self.retries = 0;
        if self.data.len() > DATA_POINTS {
            return Step::Done(crate::timeline::llround(median(&mut self.data)));
        }
        self.deadline = now.saturating_add(PING_TIMEOUT_US);
        Step::Ping(next)
    }
}

/// The median (`Median.hpp`): the middle value, or the mean of the two middle ones.
pub fn median(v: &mut [f64]) -> f64 {
    v.sort_by(f64::total_cmp);
    let n = v.len();
    if n == 0 {
        return 0.0;
    }
    if n.is_multiple_of(2) {
        (v[n / 2] + v[(n - 1) / 2]) / 2.0
    } else {
        v[n / 2]
    }
}

/// The answer to a ping (`PingResponder`): our session and its ghost time now, the ping's
/// own entries after them. `None` for anything else, or a ping too large to be one.
pub fn answer(bytes: &[u8], session: &NodeId, ghost_now: i64) -> Option<Vec<u8>> {
    let m = wire::decode_measure(bytes).ok()?;
    if m.kind != wire::PING || m.payload_len > wire::MAX_PING_PAYLOAD {
        return None;
    }
    let pong = wire::encode_pong(session, ghost_now, &bytes[9..]);
    pong.finish().map(<[u8]>::to_vec)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_median_is_the_middle() {
        assert_eq!(median(&mut [3.0, 1.0, 2.0]), 2.0);
        assert_eq!(median(&mut [4.0, 1.0, 3.0, 2.0]), 2.5);
    }

    /// A measurement against a responder whose ghost clock is host + 1 000 000 µs, the
    /// network 300 µs each way: the intercept found is the true one within a microsecond
    /// (the delays are symmetric, so they cancel).
    #[test]
    fn a_measurement_finds_the_offset() {
        let session = [7u8; 8];
        let ep = SocketAddrV4::new([127, 0, 0, 1].into(), 9);
        let mut now = 5_000_000i64;
        let (mut m, mut ping) = Measurement::start(session, session, ep, 0, now);
        for _ in 0..200 {
            now += 300;
            let pong = answer(&ping, &session, now + 1_000_000).expect("a pong");
            now += 300;
            let got = wire::decode_measure(&pong).unwrap();
            match m.pong(&got, now) {
                Step::Ping(p) => ping = p,
                Step::Done(x) => {
                    assert!((x - 1_000_000).abs() <= 1, "intercept {x}");
                    return;
                }
                s => panic!("unexpected {s:?}"),
            }
        }
        panic!("never finished");
    }

    #[test]
    fn silence_fails_after_five_retries() {
        let ep = SocketAddrV4::new([127, 0, 0, 1].into(), 9);
        let (mut m, _) = Measurement::start([1; 8], [1; 8], ep, 0, 0);
        let mut pings = 0;
        let mut t = 0;
        loop {
            t += PING_TIMEOUT_US;
            match m.tick(t) {
                Step::Ping(_) => pings += 1,
                Step::Failed => break,
                s => panic!("{s:?}"),
            }
        }
        assert_eq!(pings, RETRIES);
    }
}

//! Link nodes on a simulated network (ADR 0078 §5): clocks that disagree by seconds,
//! delays that differ each way, losses — the node code is the real one, only the wire is
//! simulated, so every run replays exactly.

use cantus_link::node::{Node, Send, Via, MULTICAST};
use std::collections::VecDeque;
use std::net::{Ipv4Addr, SocketAddrV4};

/// A small deterministic random source.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
}

struct Peer {
    node: Node,
    /// Host clock = real time + offset (µs).
    offset: i64,
    up: bool,
}

struct Msg {
    at: i64,
    from: usize,
    send: Send,
}

struct Net {
    peers: Vec<Peer>,
    queue: VecDeque<Msg>,
    t: i64,
    rng: Rng,
    /// Delay range each way (µs) and the share of messages lost.
    delay: (i64, i64),
    loss: f64,
}

fn sender(i: usize) -> SocketAddrV4 {
    SocketAddrV4::new(Ipv4Addr::new(10, 0, 0, i as u8 + 1), 5000)
}
fn measurer(i: usize) -> SocketAddrV4 {
    SocketAddrV4::new(Ipv4Addr::new(10, 0, 0, i as u8 + 1), 6000)
}

impl Net {
    fn new(seed: u64) -> Net {
        Net {
            peers: Vec::new(),
            queue: VecDeque::new(),
            t: 1_000_000,
            rng: Rng(seed | 1),
            delay: (200, 2_000),
            loss: 0.0,
        }
    }

    /// A peer at `bpm`, its clock `offset` µs from real time, enabled now.
    fn add(&mut self, bpm: f64, offset: i64) -> usize {
        let i = self.peers.len();
        let now = self.t + offset;
        let mut node = Node::new([i as u8 + 1; 8], bpm, now);
        node.set_interfaces(vec![Some(measurer(i))]);
        node.enable(true, [i as u8 + 0x40; 8], now);
        self.peers.push(Peer {
            node,
            offset,
            up: true,
        });
        i
    }

    fn host(&self, i: usize) -> i64 {
        self.t + self.peers[i].offset
    }

    fn collect(&mut self, i: usize) {
        for send in self.peers[i].node.take_out() {
            if self.rng.next() < self.loss {
                continue;
            }
            let d = self.delay.0 + ((self.delay.1 - self.delay.0) as f64 * self.rng.next()) as i64;
            self.queue.push_back(Msg {
                at: self.t + d,
                from: i,
                send,
            });
        }
    }

    fn deliver(&mut self, m: Msg) {
        let targets: Vec<usize> = match (m.send.via, m.send.to) {
            (Via::Discovery, to) if to == MULTICAST => {
                (0..self.peers.len()).filter(|&j| j != m.from).collect()
            }
            (Via::Discovery, to) => (0..self.peers.len()).filter(|&j| sender(j) == to).collect(),
            (Via::Measure, to) => (0..self.peers.len())
                .filter(|&j| measurer(j) == to)
                .collect(),
        };
        for j in targets {
            if !self.peers[j].up {
                continue;
            }
            let now = self.host(j);
            match m.send.via {
                Via::Discovery => {
                    self.peers[j]
                        .node
                        .on_discovery(0, sender(m.from), &m.send.bytes, now)
                }
                Via::Measure => {
                    self.peers[j]
                        .node
                        .on_measure(0, measurer(m.from), &m.send.bytes, now)
                }
            }
            self.collect(j);
        }
    }

    /// Run `ms` milliseconds in 0.25 ms steps.
    fn run(&mut self, ms: i64) {
        let end = self.t + ms * 1000;
        while self.t < end {
            self.t += 250;
            let mut due = Vec::new();
            let mut keep = VecDeque::new();
            while let Some(m) = self.queue.pop_front() {
                if m.at <= self.t {
                    due.push(m);
                } else {
                    keep.push_back(m);
                }
            }
            self.queue = keep;
            for m in due {
                self.deliver(m);
            }
            for i in 0..self.peers.len() {
                if self.peers[i].up {
                    let now = self.host(i);
                    self.peers[i].node.tick(now);
                    self.collect(i);
                }
            }
        }
    }

    /// The worst disagreement between the up peers' beats at the same real moment, ms.
    fn phase_error_ms(&self) -> f64 {
        let ups: Vec<usize> = (0..self.peers.len())
            .filter(|&i| self.peers[i].up)
            .collect();
        let bpm = self.peers[ups[0]].node.timeline().tempo.bpm();
        let beats: Vec<f64> = ups
            .iter()
            .map(|&i| self.peers[i].node.beat_at(self.host(i)).floating())
            .collect();
        let lo = beats.iter().cloned().fold(f64::MAX, f64::min);
        let hi = beats.iter().cloned().fold(f64::MIN, f64::max);
        (hi - lo) * 60_000.0 / bpm
    }

    fn sessions_agree(&self) -> bool {
        let ups: Vec<&Peer> = self.peers.iter().filter(|p| p.up).collect();
        ups.iter()
            .all(|p| p.node.session() == ups[0].node.session())
    }
}

#[test]
fn two_peers_share_tempo_and_phase() {
    let mut net = Net::new(7);
    net.add(120.0, 0);
    net.add(90.0, 3_456_789_000);
    net.run(4_000);
    assert!(net.sessions_agree(), "one session");
    assert_eq!(net.peers[0].node.peer_count(), 1);
    assert_eq!(net.peers[1].node.peer_count(), 1);
    // The older session (peer 0's) won: its tempo.
    let t0 = net.peers[0].node.timeline().tempo.bpm();
    let t1 = net.peers[1].node.timeline().tempo.bpm();
    assert!(
        (t0 - 120.0).abs() < 1e-9 && (t1 - 120.0).abs() < 1e-9,
        "{t0} {t1}"
    );
    let e = net.phase_error_ms();
    println!("two peers: phase error {e:.4} ms");
    assert!(e <= 1.0, "phase error {e} ms");
    // A tempo change from either side reaches the other, the phase kept.
    let now = net.host(1);
    net.peers[1].node.set_tempo(133.0, now);
    net.run(1_000);
    let t0 = net.peers[0].node.timeline().tempo.bpm();
    assert!((t0 - 133.0).abs() < 1e-3, "{t0}");
    let e = net.phase_error_ms();
    println!("after a tempo change: phase error {e:.4} ms");
    assert!(e <= 1.0, "phase error {e} ms");
}

#[test]
fn start_and_stop_reach_every_peer() {
    let mut net = Net::new(11);
    net.add(120.0, 0);
    net.add(120.0, -50_000_000);
    net.run(3_000);
    let at = net.host(0) + 100_000;
    let now = net.host(0);
    net.peers[0].node.set_playing(true, at, now);
    net.run(500);
    assert!(net.peers[1].node.start_stop().playing);
    let now = net.host(1);
    net.peers[1].node.set_playing(false, now, now);
    net.run(500);
    assert!(!net.peers[0].node.start_stop().playing);
}

#[test]
fn three_peers_join_and_leave() {
    let mut net = Net::new(3);
    net.add(100.0, 10_000_000);
    net.add(140.0, 0);
    net.run(2_000);
    net.add(70.0, 999_000_000);
    net.run(3_000);
    assert!(net.sessions_agree());
    for p in &net.peers {
        assert_eq!(p.node.peer_count(), 2);
        assert!((p.node.timeline().tempo.bpm() - 100.0).abs() < 1e-9);
    }
    let e = net.phase_error_ms();
    println!("three peers: phase error {e:.4} ms");
    assert!(e <= 1.0, "phase error {e} ms");
    // One leaves politely (bye-bye): the others count one fewer at once.
    let now = net.host(2);
    net.peers[2].node.enable(false, [9; 8], now);
    net.collect(2);
    net.run(100);
    net.peers[2].up = false;
    assert_eq!(net.peers[0].node.peer_count(), 1);
    // One goes silent: forgotten after its time to live; the last one alone founds anew.
    net.peers[1].up = false;
    net.run(7_000);
    assert_eq!(net.peers[0].node.peer_count(), 0);
}

#[test]
fn a_lossy_network_still_converges() {
    let mut net = Net::new(99);
    net.loss = 0.1;
    net.delay = (100, 5_000);
    net.add(120.0, 0);
    net.add(128.0, 77_000_000);
    net.add(128.0, -5_000_000);
    net.run(8_000);
    assert!(net.sessions_agree());
    let e = net.phase_error_ms();
    println!("10 % loss, 0.1–5 ms delays: phase error {e:.4} ms");
    assert!(e <= 1.0, "phase error {e} ms");
}

/// Review r33 #5: a beat asked for at a time lands there in the session's own numbering —
/// phase counted from the session's beat 0, as the SDK's client timeline counts it — also
/// after a tempo change has moved the beat origin off the quantum (it landed 1.7 beats
/// late) and after a new session founded with the beat carried over.
#[test]
fn a_requested_beat_lands_in_the_sessions_phase() {
    use cantus_link::timeline::{phase, Beats};
    let mut n = Node::new([1; 8], 120.0, 0);
    n.set_interfaces(vec![None]);
    n.enable(true, [2; 8], 1_000);
    let q = Beats::new(4.0);
    let check = |n: &Node, at: i64, what: &str| {
        let b = n.beat_at(at);
        let ph = phase(b, q).micro();
        assert!(
            ph <= 1 || ph >= q.micro() - 1,
            "{what}: beat {} at the time",
            b.floating()
        );
    };
    // A tempo change 10.3 beats in: the origin is no longer on a bar.
    n.set_tempo(133.0, 5_150_000);
    let at = 9_000_000;
    n.request_beat_at(0.0, at, 4.0, 8_000_000);
    check(&n, at, "after a tempo change");
    // Leave and come back: a new session, the beat carried on.
    n.enable(false, [3; 8], 9_500_000);
    n.enable(true, [4; 8], 9_600_000);
    let at = 12_345_678;
    n.request_beat_at(0.0, at, 4.0, 11_000_000);
    check(&n, at, "after a new session");
    // A beat other than 0.
    let at = 20_000_000;
    n.request_beat_at(2.0, at, 4.0, 19_000_000);
    let b = n.beat_at(at);
    let ph = phase(b, q).floating();
    assert!((ph - 2.0).abs() < 1e-5, "beat 2 asked: phase {ph}");
}

/// Review r33 #10: hostile or broken peers — absurd beats, times and tempi, a start/stop
/// stamped at the end of time, a flood of ids, pongs with extreme times — never panic
/// (overflow checks are on in tests) and never grow the node past its caps; a sane peer
/// still joins and start/stop still works afterwards.
#[test]
fn hostile_packets_neither_panic_nor_grow_the_node() {
    use cantus_link::timeline::{Beats, StartStop, Tempo, Timeline};
    use cantus_link::wire::{self, PeerState};
    let mut n = Node::new([1; 8], 120.0, 0);
    n.set_interfaces(vec![Some(measurer(0))]);
    n.enable(true, [2; 8], 1_000);
    let extremes = [i64::MIN, i64::MIN + 1, -1, 0, 1, i64::MAX - 1, i64::MAX];
    let mut k = 0u64;
    for &a in &extremes {
        for &b in &extremes {
            for mpb in [1i64, 7, 500_000, i64::MAX] {
                k += 1;
                let state = PeerState {
                    node: (k + 100).to_be_bytes(),
                    session: if k.is_multiple_of(2) {
                        n.session()
                    } else {
                        (k + 7).to_be_bytes()
                    },
                    timeline: Timeline {
                        tempo: Tempo::from_micros_per_beat(mpb),
                        beat_origin: Beats::from_micro(a),
                        time_origin: b,
                    },
                    start_stop: StartStop {
                        playing: true,
                        beats: Beats::from_micro(b),
                        timestamp: a,
                    },
                    measurement: k.is_multiple_of(3).then(|| measurer(1)),
                };
                let msg = wire::encode_discovery(wire::ALIVE, 5, &state);
                n.on_discovery(0, sender(1), msg.finish().unwrap(), 2_000 + k as i64);
                let _ = n.beat_at(a);
                n.tick(3_000 + k as i64);
                // Pongs with extreme times, to whatever is being measured.
                let pong = wire::encode_pong(
                    &state.session,
                    a,
                    &wire::encode_ping(b, Some(a)).finish().unwrap()[9..],
                );
                n.on_measure(0, measurer(1), pong.finish().unwrap(), b);
            }
        }
    }
    let (peers, others) = n.state_sizes();
    assert!(peers <= cantus_link::node::MAX_PEERS, "{peers} peers");
    assert!(others <= cantus_link::node::MAX_OTHERS, "{others} sessions");
    // A flood of new ids.
    for i in 0..10_000u64 {
        let state = PeerState {
            node: (1_000_000 + i).to_be_bytes(),
            session: (1_000_000 + i).to_be_bytes(),
            timeline: Timeline::default(),
            start_stop: StartStop::default(),
            measurement: None,
        };
        let msg = wire::encode_discovery(wire::ALIVE, 5, &state);
        n.on_discovery(0, sender(1), msg.finish().unwrap(), 10_000);
    }
    let (peers, others) = n.state_sizes();
    assert!(
        peers <= cantus_link::node::MAX_PEERS,
        "{peers} peers after a flood"
    );
    assert!(
        others <= cantus_link::node::MAX_OTHERS,
        "{others} sessions after a flood"
    );
    // Our own start/stop still moves (a timestamp at the end of time did not freeze it).
    let now = 20_000_000;
    n.set_playing(true, now, now);
    assert!(n.start_stop().playing);
    n.set_playing(false, now + 1, now + 1);
    assert!(!n.start_stop().playing, "start/stop frozen");
}

/// Round 2, D3: a measured session whose pongs carry absurd ghost times (gt at either end
/// of i64): the median intercept is absurd, refused — no overflow (debug panicked, release
/// wrapped), no session joined on it (gt = i64::MAX made it the "oldest"), our own clock
/// untouched.
#[test]
fn an_absurd_measurement_is_refused() {
    use cantus_link::timeline::{StartStop, Timeline};
    use cantus_link::wire::{self, PeerState};
    for gt in [i64::MAX, i64::MIN, i64::MAX - 1, i64::MIN + 1] {
        let mut n = Node::new([1; 8], 120.0, 0);
        n.set_interfaces(vec![Some(measurer(0))]);
        n.enable(true, [2; 8], 1_000);
        let ours = n.session();
        let xform = n.xform();
        let other = [9u8; 8];
        let state = PeerState {
            node: other,
            session: other,
            timeline: Timeline::default(),
            start_stop: StartStop::default(),
            measurement: Some(measurer(1)),
        };
        let msg = wire::encode_discovery(wire::ALIVE, 5, &state);
        n.on_discovery(0, sender(1), msg.finish().unwrap(), 2_000);
        for k in 0..101i64 {
            let ping = wire::encode_ping(3_000 + k, Some(gt));
            let pong = wire::encode_pong(&other, gt, &ping.finish().unwrap()[9..]);
            n.on_measure(0, measurer(1), pong.finish().unwrap(), 3_000 + k);
        }
        n.tick(10_000);
        assert_eq!(
            n.session(),
            ours,
            "gt {gt}: a session joined on an absurd clock"
        );
        assert_eq!(n.xform(), xform, "gt {gt}: our clock moved");
    }
}

/// Round 2, D7: with others in the session a requested beat never renumbers the shared
/// timeline (the SDK moves only the asking client's own numbering; a peer on another
/// quantum would have seen its bar's phase jump).
#[test]
fn a_request_with_others_leaves_the_session_alone() {
    let mut net = Net::new(5);
    net.add(120.0, 0);
    net.add(120.0, 1_000_000);
    net.run(3_000);
    assert_eq!(net.peers[0].node.peer_count(), 1);
    let before = (net.peers[0].node.timeline(), net.peers[1].node.timeline());
    let at = net.host(0) + 200_000;
    let now = net.host(0);
    net.peers[0].node.request_beat_at(1.0, at, 3.0, now);
    net.run(500);
    let after = (net.peers[0].node.timeline(), net.peers[1].node.timeline());
    assert_eq!(before, after, "the session's timeline moved");
}

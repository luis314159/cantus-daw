//! The Link handle: a node on its own threads, and what its owner reads and asks
//! (ADR 0078 §2). Nothing here runs on an audio thread; the application's bridge reads the
//! session here and hands the engine an abstract timeline (`cantus_engine::transport::sync`).

use crate::net::{self, Heard, Iface};
use crate::node::{self, Node, Via};
use crate::timeline::{Beats, Timeline};
use crate::wire::NodeId;
use std::net::{Ipv4Addr, SocketAddrV4};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Where the handle speaks: Link's group (everyone's) or a test's own port, on every
/// interface or the loopback only, the interfaces looked for again every `rescan`.
#[derive(Copy, Clone, Debug)]
pub struct Config {
    pub group: SocketAddrV4,
    pub only_loopback: bool,
    /// How often the interfaces are scanned again (the SDK's 30 s): one plugged in, a
    /// network joined after Link was turned on, takes part from the next scan.
    pub rescan: Duration,
    /// What lists the interfaces (`net::interfaces`; a test's own).
    pub scan: fn(bool) -> Vec<Ipv4Addr>,
}

/// The SDK's rescan period.
pub const RESCAN: Duration = Duration::from_secs(30);

impl Default for Config {
    fn default() -> Self {
        Config {
            group: node::MULTICAST,
            only_loopback: false,
            rescan: RESCAN,
            scan: net::interfaces,
        }
    }
}

impl Config {
    /// The machine only, on `port` (the tests: never Link's own group, never the network).
    pub fn local(port: u16) -> Config {
        Config {
            group: SocketAddrV4::new(*node::MULTICAST.ip(), port),
            only_loopback: true,
            ..Config::default()
        }
    }
}

/// The session as its owner sees it, in **host** time (µs of the clock the handle was
/// given).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Session {
    pub enabled: bool,
    pub peers: usize,
    pub tempo: f64,
    /// The session's timeline, mapped to host time.
    pub timeline: Timeline,
    pub playing: bool,
    /// When the last start or stop takes effect (host µs) and at which session beat.
    pub start_stop_at: i64,
    pub start_stop_beat: f64,
}

impl Session {
    /// The session's beat at host time `t`.
    pub fn beat_at(&self, t: i64) -> f64 {
        self.timeline.beats_at(t).floating()
    }
    /// The phase within `quantum` at host time `t` (`phaseAtTime`).
    pub fn phase_at(&self, t: i64, quantum: f64) -> f64 {
        crate::timeline::phase(self.timeline.beats_at(t), Beats::new(quantum)).floating()
    }
    /// The host time the session's beat `beat` falls at.
    pub fn time_at(&self, beat: f64) -> i64 {
        self.timeline.time_of(Beats::new(beat))
    }
}

struct Inner {
    stop: Arc<AtomicBool>,
    threads: Vec<std::thread::JoinHandle<()>>,
}

/// An open interface: its sockets, and the threads listening on them with their own stop.
struct Slot {
    iface: Iface,
    stop: Arc<AtomicBool>,
    threads: Vec<std::thread::JoinHandle<()>>,
}

/// Close every slot of `slots`: every listener told, then waited for.
fn close_all(slots: impl IntoIterator<Item = Slot>) {
    let slots: Vec<Slot> = slots.into_iter().collect();
    // Every listener told first, then waited for: one receive timeout in all (round 2, D5:
    // one after another, a timeout each).
    for s in &slots {
        s.stop.store(true, Ordering::Relaxed);
    }
    for s in slots {
        for t in s.threads {
            let _ = t.join();
        }
    }
}

/// What the run loop needs to open an interface.
#[derive(Clone)]
struct Opener {
    config: Config,
    clock: fn() -> i64,
    node: Arc<Mutex<Node>>,
    tx: mpsc::SyncSender<Heard>,
    open: Arc<std::sync::atomic::AtomicUsize>,
}

impl Opener {
    fn open(&self, addr: Ipv4Addr, index: usize) -> Option<Slot> {
        let iface = match Iface::open(addr, self.config.group, self.config.only_loopback) {
            Ok(i) => i,
            Err(e) => {
                log::warn!("link: cannot open {addr}: {e}");
                return None;
            }
        };
        let stop = Arc::new(AtomicBool::new(false));
        match iface.listen(
            index,
            self.clock,
            self.node.clone(),
            stop.clone(),
            self.tx.clone(),
        ) {
            Ok(threads) => Some(Slot {
                iface,
                stop,
                threads,
            }),
            Err(e) => {
                log::warn!("link: cannot listen on {addr}: {e}");
                None
            }
        }
    }

    /// Bring `slots` to the interfaces there now: those gone closed (their peers
    /// forgotten), the new ones opened in a free slot -- the others keep their index, so
    /// what the node knows of each stays right.
    fn rescan(&self, slots: &mut Vec<Option<Slot>>) {
        let now = (self.config.scan)(self.config.only_loopback);
        let mut gone = Vec::new();
        for (i, slot) in slots.iter_mut().enumerate() {
            if slot.as_ref().is_some_and(|s| !now.contains(&s.iface.addr)) {
                if let Some(s) = slot.take() {
                    log::info!("link: {} is gone", s.iface.addr);
                    gone.push(s);
                }
                if let Ok(mut n) = self.node.lock() {
                    n.drop_interface(i, (self.clock)());
                }
            }
        }
        close_all(gone);
        // What is open is counted when this ends (round 2, D6: with none, the handle said
        // "on, 0 peers" and only the log knew).
        let open = self.open.clone();
        let count = |slots: &Vec<Option<Slot>>| slots.iter().flatten().count();
        for addr in now {
            if slots.iter().flatten().any(|s| s.iface.addr == addr) {
                continue;
            }
            let i = slots
                .iter()
                .position(Option::is_none)
                .unwrap_or(slots.len());
            if let Some(s) = self.open(addr, i) {
                let mep = s.iface.measure_endpoint();
                if i == slots.len() {
                    slots.push(Some(s));
                } else {
                    slots[i] = Some(s);
                }
                if let Ok(mut n) = self.node.lock() {
                    n.set_interface(i, mep);
                }
                log::info!("link: speaking on {addr}");
            }
        }
        open.store(count(slots), Ordering::Relaxed);
    }
}

/// A Link participant.
pub struct Link {
    clock: fn() -> i64,
    config: Config,
    node: Arc<Mutex<Node>>,
    run: Mutex<Option<Inner>>,
    /// How many interfaces are open now (0: none could be — said to the user).
    open: Arc<std::sync::atomic::AtomicUsize>,
}

impl std::fmt::Debug for Link {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Link")
            .field("config", &self.config)
            .finish()
    }
}

/// Eight bytes nobody else picks: the hasher's random keys, the time and the process.
pub fn random_id() -> NodeId {
    use std::hash::{BuildHasher, Hasher};
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    h.write_u128(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos()),
    );
    h.write_u32(std::process::id());
    h.finish().to_be_bytes()
}

impl Link {
    /// A participant at `bpm`, off until [`Self::enable`], reading time from `clock` (host
    /// µs; the engine's clock, so the timeline it hands on is in the audio thread's time).
    pub fn new(bpm: f64, clock: fn() -> i64, config: Config) -> Link {
        Link {
            clock,
            config,
            node: Arc::new(Mutex::new(Node::new(random_id(), bpm, clock()))),
            run: Mutex::new(None),
            open: Arc::default(),
        }
    }

    /// Join the network (or leave it). With no interface to open yet (no network), the
    /// handle stays on and takes part from the scan that finds one.
    pub fn enable(&self, on: bool) -> std::io::Result<()> {
        let mut run = self.run.lock().unwrap_or_else(|e| e.into_inner());
        if on == run.is_some() {
            return Ok(());
        }
        if !on {
            if let Ok(mut n) = self.node.lock() {
                n.enable(false, random_id(), (self.clock)());
            }
            // The loop sends the bye-bye the node left (it wakes every 5 ms), then stops.
            std::thread::sleep(Duration::from_millis(20));
            if let Some(inner) = run.take() {
                inner.stop.store(true, Ordering::Relaxed);
                for t in inner.threads {
                    let _ = t.join();
                }
            }
            return Ok(());
        }
        let stop = Arc::new(AtomicBool::new(false));
        let (tx, rx) = mpsc::sync_channel::<Heard>(net::HEARD_QUEUE);
        let opener = Opener {
            config: self.config,
            clock: self.clock,
            node: self.node.clone(),
            tx,
            open: self.open.clone(),
        };
        {
            let mut n = self.node.lock().unwrap_or_else(|e| e.into_inner());
            n.set_group(self.config.group);
            n.set_interfaces(Vec::new());
            n.enable(true, random_id(), (self.clock)());
        }
        let mut slots = Vec::new();
        opener.rescan(&mut slots);
        if slots.iter().flatten().next().is_none() {
            log::warn!(
                "link: no interface yet; looking again every {:?}",
                self.config.rescan
            );
        }
        let stop2 = stop.clone();
        let thread = std::thread::Builder::new()
            .name("cantus-link".into())
            .spawn(move || run_loop(opener, slots, rx, stop2))?;
        *run = Some(Inner {
            stop,
            threads: vec![thread],
        });
        Ok(())
    }

    /// How many interfaces it speaks on now: 0 while on means none could be opened (no
    /// network, the port refused) — it keeps looking every `rescan`.
    pub fn interfaces_open(&self) -> usize {
        self.open.load(Ordering::Relaxed)
    }

    pub fn is_enabled(&self) -> bool {
        self.run.lock().map(|r| r.is_some()).unwrap_or(false)
    }

    /// The session now, in host time.
    pub fn session(&self) -> Session {
        let n = self.node.lock().unwrap_or_else(|e| e.into_inner());
        let x = n.xform();
        let tl = n.timeline();
        let host_tl = Timeline {
            tempo: tl.tempo,
            beat_origin: tl.beat_origin,
            time_origin: x.ghost_to_host(tl.time_origin),
        };
        let ss = n.start_stop();
        Session {
            enabled: n.is_enabled(),
            peers: n.peer_count(),
            tempo: tl.tempo.bpm(),
            timeline: host_tl,
            playing: ss.playing,
            start_stop_at: x.ghost_to_host(tl.time_of(ss.beats)),
            start_stop_beat: ss.beats.floating(),
        }
    }

    /// The tempo from host time `at` on.
    pub fn set_tempo(&self, bpm: f64, at: i64) {
        if let Ok(mut n) = self.node.lock() {
            n.set_tempo(bpm, at);
        }
    }

    /// Start or stop the session at host time `at`.
    pub fn set_playing(&self, playing: bool, at: i64) {
        if let Ok(mut n) = self.node.lock() {
            let now = (self.clock)();
            n.set_playing(playing, at, now);
        }
    }

    /// Alone, the session's `beat` at host time `at`; with others in the session, nothing
    /// (their phase stays theirs).
    pub fn request_beat_at(&self, beat: f64, at: i64, quantum: f64) {
        if let Ok(mut n) = self.node.lock() {
            let now = (self.clock)();
            n.request_beat_at(beat, at, quantum, now);
        }
    }
}

impl Drop for Link {
    fn drop(&mut self) {
        let _ = self.enable(false);
    }
}

/// The node's loop: what the sockets heard, the node's clock, what it wants sent, and the
/// interfaces scanned again every `rescan`.
fn run_loop(
    opener: Opener,
    mut slots: Vec<Option<Slot>>,
    rx: mpsc::Receiver<Heard>,
    stop: Arc<AtomicBool>,
) {
    let clock = opener.clock;
    let node = opener.node.clone();
    let mut scanned = std::time::Instant::now();
    while !stop.load(Ordering::Relaxed) {
        if scanned.elapsed() >= opener.config.rescan {
            scanned = std::time::Instant::now();
            opener.rescan(&mut slots);
        }
        let first = rx.recv_timeout(Duration::from_millis(5)).ok();
        let out = {
            let Ok(mut n) = node.lock() else { break };
            for h in first.into_iter().chain(rx.try_iter()) {
                if slots.get(h.iface).is_none_or(Option::is_none) {
                    continue;
                }
                log::trace!(
                    "link: {} bytes from {} ({:?})",
                    h.bytes.len(),
                    h.from,
                    h.via
                );
                match h.via {
                    Via::Discovery => n.on_discovery(h.iface, h.from, &h.bytes, h.at),
                    Via::Measure => n.on_measure(h.iface, h.from, &h.bytes, h.at),
                }
            }
            n.tick(clock());
            n.take_out()
        };
        for mut s in out {
            if let Some(Some(slot)) = slots.get(s.iface) {
                if s.via == Via::Measure {
                    net::restamp_ping(&mut s.bytes, clock());
                }
                log::trace!("link: {} bytes to {} ({:?})", s.bytes.len(), s.to, s.via);
                slot.iface.send(s.via, s.to, &s.bytes);
            }
        }
    }
    close_all(slots.into_iter().flatten());
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Round 2, D5: closing several interfaces stops every listener first, then waits:
    /// about one receive timeout (100 ms) in all, not one per interface.
    #[test]
    fn closing_several_interfaces_waits_once() {
        let node = Arc::new(Mutex::new(Node::new([1; 8], 120.0, 0)));
        let (tx, _rx) = mpsc::sync_channel::<Heard>(16);
        let opener = Opener {
            config: Config::local(20_941),
            clock: || 0,
            node,
            tx,
            open: Arc::default(),
        };
        let slots: Vec<Slot> = (0..5)
            .map(|i| {
                let mut o = opener.clone();
                o.config = Config::local(20_941 + i as u16);
                o.open(Ipv4Addr::LOCALHOST, i).expect("a loopback slot")
            })
            .collect();
        // Let every listener sit in its receive.
        std::thread::sleep(Duration::from_millis(30));
        let t = std::time::Instant::now();
        close_all(slots);
        let ms = t.elapsed().as_millis();
        println!("five interfaces closed in {ms} ms");
        assert!(ms < 160, "{ms} ms");
    }
}

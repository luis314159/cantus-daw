//! MIDI out from the engine (ADR 0079 §4): a thread of its own — never the audio thread —
//! that takes the clock and time-code messages the audio thread stamped with their sample,
//! and sends each when its audio reaches the speakers: the sample's host time through the
//! audio thread's own line ([`super::HostLine`]), plus the device's output latency, plus
//! the port's offset the user set (a synth with a slow MIDI input, a long cable chain).
//!
//! It sleeps on the system's high-resolution timer (Rust's `thread::sleep` uses Windows'
//! `CREATE_WAITABLE_TIMER_HIGH_RESOLUTION` timer and `nanosleep` elsewhere) until 0.5 ms
//! before a message is due, then yields until the moment; it never runs under MMCSS, so
//! the yield costs nothing the audio needs.

use super::{OutKind, Stamped, SyncShared};
use crate::midi::OutPort;
use rtrb::Consumer;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// One output port's choices.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PortChoice {
    pub name: String,
    pub clock: bool,
    pub mtc: bool,
    /// Sent this much later (positive) or earlier than the audio, ms.
    pub offset_ms: f32,
}

/// What the thread measured: how late each message left against its due time.
#[derive(Debug, Default)]
pub struct OutStats {
    pub sent: AtomicU64,
    /// The worst and the latest lateness, µs (negative: early).
    pub worst_us: AtomicI64,
    pub last_us: AtomicI64,
    /// Ports that could not be opened (busy, gone), counted.
    pub refused: AtomicU64,
}

/// Opens an output by name (the system's; a test's own).
pub type Connect = crate::midi::pool::Connect;

/// The system's outputs, by name.
pub fn connect_system(name: &str) -> Option<Box<dyn OutPort>> {
    let out = midir::MidiOutput::new("cantus-sync").ok()?;
    // The n-th port of a name (an external instrument's ordinal, ADR 0094). r36:external
    let (name, nth) = crate::external_out::split_key(name);
    let port = out
        .ports()
        .into_iter()
        .filter(|p| out.port_name(p).ok().as_deref() == Some(name))
        .nth(nth)?;
    match out.connect(&port, "cantus-sync-out") {
        Ok(c) => Some(Box::new(c)),
        Err(e) => {
            log::warn!("midi: cannot open {name} for clock: {e}");
            None
        }
    }
}

/// The running thread.
pub struct MidiOut {
    choices: Arc<Mutex<Vec<PortChoice>>>,
    /// The external instruments' ports: route number → port name (ADR 0094). r36:external
    routes: Arc<Mutex<Vec<(u64, String, u8)>>>,
    stop: Arc<AtomicBool>,
    pub stats: Arc<OutStats>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl std::fmt::Debug for MidiOut {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MidiOut").finish()
    }
}

/// A message on its way to one port, named by the port's id (stable while the port stays
/// chosen, whatever the order: review r33 #9).
struct Due {
    at_us: f64,
    port: u64,
    msg: Stamped,
}

/// An open port: its id, the choice it was opened for, the connection.
struct Open {
    id: u64,
    choice: PortChoice,
    port: Option<Box<dyn OutPort>>,
}

/// The port id of a due message for an external instrument (ADR 0094): its route says which.
const NOTE_PORT: u64 = u64::MAX; // r36:external

/// How close to a message's moment the thread sleeps (the high-resolution timer's own
/// lateness), then yields to it: the yielding is this short, not a millisecond a message.
const SPIN_US: f64 = 500.0;

/// How long the thread sleeps between looks when it has no port and no external instrument
/// to serve (round 38): a port chosen in Settings is opened within this.
const IDLE: Duration = Duration::from_millis(20);

impl MidiOut {
    /// Start sending what `ring` brings, to the ports [`Self::set_ports`] names.
    /// Its ports come from `pool`, which controller feedback shares (one connection per
    /// port: Windows' class driver serves one client).
    pub fn start(
        ring: Consumer<Stamped>,
        shared: Arc<SyncShared>,
        pool: Arc<crate::midi::pool::OutPool>,
    ) -> MidiOut {
        let choices: Arc<Mutex<Vec<PortChoice>>> = Arc::default();
        let routes: Arc<Mutex<Vec<(u64, String, u8)>>> = Arc::default(); // r36:external
        let r2 = routes.clone(); // r36:external
        let stop = Arc::new(AtomicBool::new(false));
        let stats = Arc::new(OutStats::default());
        let (c2, s2, st2) = (choices.clone(), stop.clone(), stats.clone());
        let thread = std::thread::Builder::new()
            .name("cantus-midi-out".into())
            .spawn(move || run(ring, shared, pool, c2, r2, s2, st2))
            .ok();
        MidiOut {
            choices,
            routes, // r36:external
            stop,
            stats,
            thread,
        }
    }

    /// The ports external instruments' notes go to: `(route, port name, ordinal)`, the route
    /// being what the processor stamped each note with — the unit's id (ADR 0094).
    /// r36:external
    pub fn set_unit_routes(&self, routes: Vec<(u64, String, u8)>) {
        if let Ok(mut r) = self.routes.lock() {
            *r = routes;
        }
    }

    /// The routes as last set (the interface's tests read the wiring back). r36:external
    pub fn unit_routes(&self) -> Vec<(u64, String, u8)> {
        self.routes.lock().map(|r| r.clone()).unwrap_or_default()
    }

    /// The ports and what each takes (Settings ▸ MIDI).
    pub fn set_ports(&self, ports: Vec<PortChoice>) {
        if let Ok(mut c) = self.choices.lock() {
            *c = ports;
        }
    }
}

impl Drop for MidiOut {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

fn run(
    mut ring: Consumer<Stamped>,
    shared: Arc<SyncShared>,
    pool: Arc<crate::midi::pool::OutPool>,
    choices: Arc<Mutex<Vec<PortChoice>>>,
    unit_routes: Arc<Mutex<Vec<(u64, String, u8)>>>, // r36:external
    stop: Arc<AtomicBool>,
    stats: Arc<OutStats>,
) {
    // The external instruments' ports (ADR 0094): route → its connection; a due message
    // whose port is NOTE_PORT goes to its route's. r36:external
    let mut unit_ports = crate::external_out::UnitPorts::default();
    let mut open: Vec<Open> = Vec::new();
    let mut ids = 0u64;
    let mut due: Vec<Due> = Vec::with_capacity(256);
    let mut line = shared.host_line.read().unwrap_or_default();
    while !stop.load(Ordering::Relaxed) {
        // The ports as chosen now: (re)opened when the choice changed; a port chosen again
        // keeps its id and its connection.
        if let Ok(c) = choices.try_lock() {
            if open.len() != c.len() || open.iter().zip(c.iter()).any(|(o, n)| o.choice != *n) {
                let mut next = Vec::with_capacity(c.len());
                for ch in c.iter() {
                    let kept = open
                        .iter()
                        .position(|o| o.choice.name == ch.name)
                        .map(|i| open.swap_remove(i));
                    let (id, port) = match kept {
                        Some(o) => (o.id, o.port),
                        None => {
                            ids += 1;
                            (ids, None)
                        }
                    };
                    let port = match port {
                        Some(p) => Some(p),
                        None if ch.clock || ch.mtc => {
                            let p = pool.get(&ch.name).map(|p| {
                                Box::new(crate::midi::pool::Pooled(p)) as Box<dyn OutPort>
                            });
                            if p.is_none() {
                                stats.refused.fetch_add(1, Ordering::Relaxed);
                            }
                            p
                        }
                        None => None,
                    };
                    next.push(Open {
                        id,
                        choice: ch.clone(),
                        port,
                    });
                }
                open = next;
                pool.prune();
                // What waits for a port no longer chosen, or for a kind it no longer takes,
                // is dropped.
                due.retain(|d| {
                    d.port == NOTE_PORT // r36:external
                        || open.iter().any(|o| {
                        o.id == d.port
                            && match d.msg.kind {
                                OutKind::Clock => o.choice.clock,
                                OutKind::Mtc => o.choice.mtc,
                                OutKind::Note => false, // r36:external
                            }
                    })
                });
            }
        }
        // r36:external — the unit routes as set now: a port is opened (through the pool, which
        // controller feedback and clock share) for each route; a route gone is retired, so the
        // note-offs for it still go out (never dropped).
        if let Ok(r) = unit_routes.try_lock() {
            let refused = unit_ports.update(&r, |key| {
                pool.get(key)
                    .map(|p| Box::new(crate::midi::pool::Pooled(p)) as Box<dyn OutPort>)
            });
            stats.refused.fetch_add(refused, Ordering::Relaxed);
        }
        if unit_ports.prune(|route| {
            due.iter()
                .any(|d| d.port == NOTE_PORT && d.msg.route == route)
        }) {
            pool.prune();
        }
        if let Some(l) = shared.host_line.read() {
            line = l;
        }
        while let Ok(m) = ring.pop() {
            let at = line.host_of(m.at) + line.latency_us;
            if m.kind == OutKind::Note {
                // r36:external: to the one port its route names, at its sample's moment.
                if unit_ports.reaches(m.route) {
                    due.push(Due {
                        at_us: at,
                        port: NOTE_PORT,
                        msg: m,
                    });
                }
                continue;
            }
            for o in &open {
                let takes = match m.kind {
                    OutKind::Clock => o.choice.clock,
                    OutKind::Mtc => o.choice.mtc,
                    OutKind::Note => false, // r36:external
                };
                if takes && o.port.is_some() {
                    due.push(Due {
                        at_us: at + f64::from(o.choice.offset_ms) * 1000.0,
                        port: o.id,
                        msg: m,
                    });
                }
            }
        }
        // Stable by time: messages due together keep the order the audio thread made.
        due.sort_by(|a, b| a.at_us.total_cmp(&b.at_us));
        let now = super::host_micros() as f64;
        let Some(first) = due.first() else {
            // Nothing due. With a port or an external instrument to serve, look again within
            // a millisecond (the audio thread stamps messages a latency ahead); with none, the
            // audio thread sends nothing, and a port chosen later is opened within IDLE —
            // a thread that woke a thousand times a second for nothing was 1 % of a core at
            // rest (round 38, the scorecard).
            let serving = open.iter().any(|o| o.port.is_some()) || !unit_ports.is_empty();
            std::thread::sleep(if serving {
                Duration::from_millis(1)
            } else {
                IDLE
            });
            continue;
        };
        let wait = first.at_us - now;
        if wait > SPIN_US {
            std::thread::sleep(Duration::from_micros(((wait - SPIN_US) as u64).min(2_000)));
            continue;
        }
        while (super::host_micros() as f64) < first.at_us {
            std::thread::yield_now();
        }
        let sent_at = super::host_micros() as f64;
        let mut k = 0;
        while k < due.len() && due[k].at_us <= sent_at {
            let d = &due[k];
            let target = if d.port == NOTE_PORT {
                unit_ports.port(d.msg.route) // r36:external
            } else {
                open.iter_mut()
                    .find(|o| o.id == d.port)
                    .and_then(|o| o.port.as_mut())
            };
            if let Some(p) = target {
                p.write(d.msg.msg());
                let late = (super::host_micros() as f64 - d.at_us) as i64;
                stats.sent.fetch_add(1, Ordering::Relaxed);
                stats.last_us.store(late, Ordering::Relaxed);
                stats.worst_us.fetch_max(late, Ordering::Relaxed);
            }
            k += 1;
        }
        due.drain(..k);
    }
}

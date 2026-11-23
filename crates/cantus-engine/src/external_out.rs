//! The external instruments' MIDI ports on the MIDI-out thread (ADR 0094 §3).
//!
//! A unit's notes are stamped with its **route** — the unit's id, never reused — and the
//! interface registers `(route, port name, ordinal)` for every enabled instrument
//! (`MidiOut::set_unit_routes`). A route that leaves the table is **retired**, not closed:
//! its connection stays for [`RETIRE`] so the note-offs the engine sends when a unit goes
//! (removed, switched off, moved to another port) still reach the synth that holds the
//! notes. Ports come from the pool clock out and controller feedback share; the second port
//! of a name is opened by its ordinal ([`pool_key`]).

use crate::midi::OutPort;
use std::time::{Duration, Instant};

/// How long a retired route's port stays open for its last note-offs.
pub const RETIRE: Duration = Duration::from_secs(3);

/// The pool's key of a port: its name, or for the n-th port of that name (n > 0) the name and
/// the ordinal after a unit separator (`connect_system` reads it back).
pub fn pool_key(name: &str, ordinal: u8) -> String {
    if ordinal == 0 {
        name.to_string()
    } else {
        format!("{name}\u{1F}{ordinal}")
    }
}

/// A name and ordinal from a pool key.
pub fn split_key(key: &str) -> (&str, usize) {
    match key.split_once('\u{1F}') {
        Some((n, o)) => (n, o.parse().unwrap_or(0)),
        None => (key, 0),
    }
}

/// One registered route.
pub(crate) struct UnitPort {
    pub route: u64,
    pub key: String,
    pub port: Option<Box<dyn OutPort>>,
}

/// The routes and their connections, live and retired.
#[derive(Default)]
pub(crate) struct UnitPorts {
    live: Vec<UnitPort>,
    retired: Vec<(UnitPort, Instant)>,
    table: Vec<(u64, String, u8)>,
}

impl UnitPorts {
    /// No route is set and none is retiring: no external instrument's note can be due.
    pub fn is_empty(&self) -> bool {
        self.table.is_empty() && self.live.is_empty() && self.retired.is_empty()
    }

    /// The table as the interface set it now: new routes opened through `open`, routes gone
    /// retired. Returns how many ports could not be opened.
    pub fn update(
        &mut self,
        table: &[(u64, String, u8)],
        mut open: impl FnMut(&str) -> Option<Box<dyn OutPort>>,
    ) -> u64 {
        if table == self.table.as_slice() {
            return 0;
        }
        let mut refused = 0;
        let mut next = Vec::with_capacity(table.len());
        for (route, name, ordinal) in table {
            let key = pool_key(name, *ordinal);
            let kept = self
                .live
                .iter()
                .position(|u| u.route == *route && u.key == key)
                .map(|i| self.live.swap_remove(i))
                .or_else(|| {
                    self.retired
                        .iter()
                        .position(|(u, _)| u.route == *route && u.key == key)
                        .map(|i| self.retired.swap_remove(i).0)
                });
            let port = match kept {
                Some(u) if u.port.is_some() => u.port,
                _ => {
                    let p = open(&key);
                    if p.is_none() {
                        refused += 1;
                    }
                    p
                }
            };
            next.push(UnitPort {
                route: *route,
                key,
                port,
            });
        }
        let now = Instant::now();
        for gone in self.live.drain(..) {
            self.retired.push((gone, now));
        }
        self.live = next;
        self.table = table.to_vec();
        refused
    }

    /// Whether notes for `route` can go anywhere (live or retired, with a connection).
    pub fn reaches(&self, route: u64) -> bool {
        self.live
            .iter()
            .chain(self.retired.iter().map(|(u, _)| u))
            .any(|u| u.route == route && u.port.is_some())
    }

    /// The connection of `route`.
    pub fn port(&mut self, route: u64) -> Option<&mut Box<dyn OutPort>> {
        if let Some(i) = self.live.iter().position(|u| u.route == route) {
            return self.live[i].port.as_mut();
        }
        self.retired
            .iter_mut()
            .find(|(u, _)| u.route == route)
            .and_then(|(u, _)| u.port.as_mut())
    }

    /// Close retired routes older than [`RETIRE`] that nothing waits for.
    pub fn prune(&mut self, waiting: impl Fn(u64) -> bool) -> bool {
        let before = self.retired.len();
        self.retired
            .retain(|(u, at)| at.elapsed() < RETIRE || waiting(u.route));
        self.retired.len() != before
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[derive(Clone, Default)]
    struct Probe(Arc<Mutex<Vec<Vec<u8>>>>);
    impl OutPort for Probe {
        fn write(&mut self, bytes: &[u8]) -> bool {
            self.0.lock().unwrap().push(bytes.to_vec());
            true
        }
    }

    #[test]
    fn keys_name_the_second_port_of_a_name_by_its_ordinal() {
        assert_eq!(pool_key("Synth", 0), "Synth");
        assert_eq!(split_key(&pool_key("Synth", 2)), ("Synth", 2));
        assert_eq!(split_key("Plain"), ("Plain", 0));
    }

    #[test]
    fn a_route_that_leaves_the_table_still_reaches_its_port_for_the_note_offs() {
        let probe = Probe::default();
        let mut ports = UnitPorts::default();
        let p2 = probe.clone();
        let refused = ports.update(&[(7, "Synth".into(), 0)], move |_| {
            Some(Box::new(p2.clone()) as Box<dyn OutPort>)
        });
        assert_eq!(refused, 0);
        assert!(ports.reaches(7));
        // The unit is removed: its route is retired, not closed.
        ports.update(&[], |_| None);
        assert!(ports.reaches(7), "retired: its note-offs still go");
        ports.port(7).unwrap().write(&[0x80, 60, 0]);
        assert_eq!(probe.0.lock().unwrap().as_slice(), &[vec![0x80, 60, 0]]);
        // While something waits for it, it is kept past its time; then it closes.
        assert!(!ports.prune(|_| true));
        assert!(ports.reaches(7));
    }

    #[test]
    fn the_second_port_of_a_name_is_opened_by_its_key() {
        let mut ports = UnitPorts::default();
        let mut asked = vec![];
        ports.update(&[(1, "Synth".into(), 0), (2, "Synth".into(), 1)], |k| {
            asked.push(k.to_string());
            None
        });
        assert_eq!(asked, vec!["Synth".to_string(), "Synth\u{1F}1".to_string()]);
    }
}

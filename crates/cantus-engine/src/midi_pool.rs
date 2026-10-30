//! One connection per MIDI output port, shared (integrated review #9): Windows' MIDI 1.0
//! class driver serves one client per port, so the engine's clock and time code
//! (`transport::sync::out`) and the controllers' feedback (`MidiOutputs`) must not each open
//! their own — the second open fails and one of them is lost (a Launchpad blinking to
//! Cantus's clock while its lights are fed back). Both take their port from one pool;
//! writes are whole messages under the port's own lock (never the audio thread's).

use super::OutPort;
use std::sync::{Arc, Mutex};

/// A port as the pool hands it out: written by whoever holds a handle.
pub type SharedPort = Arc<Mutex<Box<dyn OutPort>>>;

/// Opens an output by name.
pub type Connect = fn(&str) -> Option<Box<dyn OutPort>>;

/// The open outputs, by name.
pub struct OutPool {
    open: Mutex<Vec<(String, SharedPort)>>,
    connect: Connect,
}

impl std::fmt::Debug for OutPool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OutPool").finish()
    }
}

impl OutPool {
    /// A pool opening its ports with `connect` (the system's, `sync::out::connect_system`;
    /// a test's own).
    pub fn new(connect: Connect) -> Arc<OutPool> {
        Arc::new(OutPool {
            open: Mutex::new(Vec::new()),
            connect,
        })
    }

    /// The port called `name`, opened the first time it is asked for.
    pub fn get(&self, name: &str) -> Option<SharedPort> {
        let mut open = self.open.lock().ok()?;
        if let Some((_, p)) = open.iter().find(|(n, _)| n == name) {
            return Some(p.clone());
        }
        let p: SharedPort = Arc::new(Mutex::new((self.connect)(name)?));
        open.push((name.to_string(), p.clone()));
        Some(p)
    }

    /// Close the ports nobody holds any more (the pool's own handle the last).
    pub fn prune(&self) {
        if let Ok(mut open) = self.open.lock() {
            open.retain(|(_, p)| Arc::strong_count(p) > 1);
        }
    }

    /// How many ports are open (the tests).
    pub fn open_count(&self) -> usize {
        self.open.lock().map_or(0, |o| o.len())
    }
}

/// A handle written through as any port.
pub struct Pooled(pub SharedPort);

impl OutPort for Pooled {
    fn write(&mut self, bytes: &[u8]) -> bool {
        self.0.lock().map(|mut p| p.write(bytes)).unwrap_or(false)
    }
}

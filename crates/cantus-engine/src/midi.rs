//! Hardware MIDI input (midir). Each port's callback runs on its own OS thread and pushes
//! into one shared ring buffer that the audio thread drains at the start of every block.

use crate::messages::MidiEvent;
use midir::{Ignore, MidiInput, MidiInputConnection};
use rtrb::Producer;
use std::sync::{Arc, Mutex};

/// Cloneable sender for MIDI threads. The mutex is only ever contended between MIDI
/// callbacks, never touched by the audio thread.
#[derive(Clone)]
pub struct MidiSender {
    inner: Arc<Mutex<Producer<MidiEvent>>>,
}

impl std::fmt::Debug for MidiSender {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("MidiSender")
    }
}

impl MidiSender {
    pub fn new(producer: Producer<MidiEvent>) -> Self {
        MidiSender {
            inner: Arc::new(Mutex::new(producer)),
        }
    }

    /// Returns `false` if the queue was full (the event is dropped).
    pub fn send(&self, ev: MidiEvent) -> bool {
        match self.inner.lock() {
            Ok(mut p) => p.push(ev).is_ok(),
            Err(_) => false,
        }
    }
}

/// Parse a raw MIDI message into the subset the engine understands.
pub fn parse(bytes: &[u8]) -> Option<MidiEvent> {
    let status = *bytes.first()?;
    match status & 0xF0 {
        0x80 | 0x90 | 0xA0 | 0xB0 | 0xE0 => Some(MidiEvent {
            status,
            data1: *bytes.get(1)?,
            data2: *bytes.get(2)?,
        }),
        0xC0 | 0xD0 => Some(MidiEvent {
            status,
            data1: *bytes.get(1)?,
            data2: 0,
        }),
        _ => None, // system / realtime messages
    }
}

pub struct MidiInputs {
    _connections: Vec<MidiInputConnection<()>>,
    pub names: Vec<String>,
}

impl std::fmt::Debug for MidiInputs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MidiInputs")
            .field("names", &self.names)
            .finish()
    }
}

/// Names of the MIDI input ports currently present.
pub fn list_ports() -> Vec<String> {
    let Ok(input) = MidiInput::new("cantus-scan") else {
        return Vec::new();
    };
    input
        .ports()
        .iter()
        .filter_map(|p| input.port_name(p).ok())
        .collect()
}

impl MidiInputs {
    /// Open every input port. Failures are logged and skipped: a broken driver must never
    /// stop the DAW from starting.
    pub fn open_all(sender: MidiSender) -> MidiInputs {
        let mut connections = Vec::new();
        let mut names = Vec::new();
        let ports = match MidiInput::new("cantus-scan") {
            Ok(i) => i.ports(),
            Err(e) => {
                log::warn!("midi: cannot initialise input: {e}");
                return MidiInputs {
                    _connections: connections,
                    names,
                };
            }
        };
        for port in ports {
            let Ok(mut input) = MidiInput::new("cantus") else {
                continue;
            };
            input.ignore(Ignore::All);
            let name = input
                .port_name(&port)
                .unwrap_or_else(|_| "unknown".to_string());
            let tx = sender.clone();
            match input.connect(
                &port,
                "cantus-in",
                move |_stamp, bytes, _| {
                    if let Some(ev) = parse(bytes) {
                        tx.send(ev);
                    }
                },
                (),
            ) {
                Ok(c) => {
                    log::info!("midi: opened input {name}");
                    connections.push(c);
                    names.push(name);
                }
                Err(e) => log::warn!("midi: cannot open {name}: {e}"),
            }
        }
        MidiInputs {
            _connections: connections,
            names,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_channel_messages_and_ignores_system() {
        assert_eq!(
            parse(&[0x91, 60, 100]),
            Some(MidiEvent {
                status: 0x91,
                data1: 60,
                data2: 100
            })
        );
        assert_eq!(
            parse(&[0x80, 60, 0]),
            Some(MidiEvent::note_off(60)).map(|mut e| {
                e.status = 0x80;
                e
            })
        );
        assert_eq!(
            parse(&[0xC0, 5]),
            Some(MidiEvent {
                status: 0xC0,
                data1: 5,
                data2: 0
            })
        );
        assert_eq!(parse(&[0xF8]), None);
        assert_eq!(parse(&[0x90]), None);
        assert_eq!(parse(&[]), None);
    }
}

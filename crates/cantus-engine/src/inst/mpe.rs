//! What the engine's MPE input says to the rest of Cantus (round 33, ADR 0080): which
//! hardware ports are MPE controllers, and the per-note values it reports while recording.
//!
//! **Ports.** Every MIDI input port's messages meet in one ring, so a port's MPE-ness is
//! decided where its callback hands a message to the engine ([`for_engine`]): a port marked
//! MPE passes its channels as they are, and every other port's channel messages are put
//! on channel 1 — the lower zone's master, or no zone — so an ordinary keyboard on channel 5
//! is never read as a member channel. With no port marked nothing changes: the engine
//! never read the channel before. The marks are a lock-free word the interface publishes
//! ([`set_mpe_ports`]) and every callback reads.

use crate::messages::MidiEvent;
use cantus_core::model::note_expr::ExprKind;
use cantus_core::{Ticks, TrackId};
use std::sync::atomic::{AtomicU64, Ordering};

/// The ports marked MPE, by their index in the open order (the first 64).
static MPE_PORTS: AtomicU64 = AtomicU64::new(0);

/// The word of marks for the ports of `names` (the open order) whose name is in `mpe`.
pub fn word_of(names: &[String], mpe: &[String]) -> u64 {
    let mut word = 0u64;
    for (i, n) in names.iter().enumerate().take(64) {
        if mpe.iter().any(|m| m == n) {
            word |= 1 << i;
        }
    }
    word
}

/// Mark the ports of `names` (the open order) whose name is in `mpe` as MPE controllers;
/// returns the word published.
pub fn set_mpe_ports(names: &[String], mpe: &[String]) -> u64 {
    let word = word_of(names, mpe);
    MPE_PORTS.store(word, Ordering::Release);
    word
}

/// Whether port `port` is marked MPE.
pub fn is_mpe_port(port: usize) -> bool {
    port < 64 && MPE_PORTS.load(Ordering::Acquire) & (1 << port) != 0
}

/// A message from port `port` as the engine should read it: as it is from an MPE port, or
/// when no port is MPE; a channel message from any other port moved to channel 1.
/// Lock-free, allocation-free (it runs in the MIDI callback).
#[inline]
pub fn for_engine(port: usize, ev: MidiEvent) -> Option<MidiEvent> {
    for_engine_with(MPE_PORTS.load(Ordering::Acquire), port, ev)
}

/// [`for_engine`] with the marks given.
#[inline]
///
/// While a port is marked, what would reach the MPE layout from any other port is held back
/// (review D7): its RPN and NRPN selections and data entry (CC 6, 38, 98–101 — an unmarked
/// MPE device or an echoed MCM would re-zone the controller), and its panics (CC 120,
/// 123) go on tagged [`PLAIN_PANIC`], so they let go of its own plain notes and not the
/// MPE controller's. `None`: dropped.
pub fn for_engine_with(word: u64, port: usize, ev: MidiEvent) -> Option<MidiEvent> {
    if word == 0 || (port < 64 && word & (1 << port) != 0) || ev.status >= 0xF0 {
        return Some(ev);
    }
    if ev.status & 0xF0 == 0xB0 {
        match ev.data1 {
            6 | 38 | 98..=101 => return None,
            120 | 123 => {
                return Some(MidiEvent {
                    status: 0xB0,
                    data1: ev.data1,
                    data2: PLAIN_PANIC,
                })
            }
            _ => {}
        }
    }
    Some(MidiEvent {
        status: ev.status & 0xF0,
        ..ev
    })
}

/// The value a non-MPE port's all-sound-off or all-notes-off carries while a port is marked
/// MPE (the standard says 0): the engine releases the plain live notes only.
pub const PLAIN_PANIC: u8 = 127;

/// What happened to a note played on an MPE controller.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum MpeIn {
    On {
        velocity: u8,
    },
    Off {
        release: u8,
    },
    /// A value, in `NoteExpr`'s integer unit.
    Value {
        kind: ExprKind,
        value: i64,
    },
}

/// A note played on an MPE controller while recording, as the interface records it: which
/// note — its member channel, key and the tick its note-on was heard at, so two notes of
/// one key on two channels are never confused (review fix 7) — the tick this was heard at,
/// and what happened.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ExprInput {
    pub track: TrackId,
    pub tick: Ticks,
    pub channel: u8,
    pub pitch: u8,
    pub start: Ticks,
    pub what: MpeIn,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_marked_ports_keep_their_channels() {
        let names = vec!["Keys".to_string(), "Seaboard".to_string()];
        let ev = MidiEvent {
            status: 0x93,
            data1: 60,
            data2: 100,
        };
        assert_eq!(
            for_engine_with(0, 0, ev),
            Some(ev),
            "no port marked: nothing changes"
        );
        let w = word_of(&names, &["Seaboard".into()]);
        assert_eq!(w, 0b10);
        assert_eq!(for_engine_with(w, 1, ev), Some(ev));
        assert_eq!(for_engine_with(w, 0, ev).unwrap().status, 0x90);
        // Review D7: another port's MCM never reaches the layout; its panic is its own.
        let cc = |d1: u8, d2: u8| MidiEvent {
            status: 0xB4,
            data1: d1,
            data2: d2,
        };
        for d1 in [101, 100, 6, 38, 98, 99] {
            assert_eq!(for_engine_with(w, 0, cc(d1, 0)), None, "CC {d1}");
        }
        assert_eq!(
            for_engine_with(w, 1, cc(6, 3)),
            Some(cc(6, 3)),
            "the MPE port's own"
        );
        assert_eq!(
            for_engine_with(w, 0, cc(123, 0)).unwrap().data2,
            PLAIN_PANIC
        );
        assert_eq!(
            for_engine_with(0, 0, cc(6, 3)),
            Some(cc(6, 3)),
            "no MPE: untouched"
        );
        let clock = MidiEvent {
            status: 0xF8,
            data1: 0,
            data2: 0,
        };
        assert_eq!(for_engine_with(w, 0, clock), Some(clock));
    }
}

//! System messages from the input ports, for sync (round 33, ADR 0079 §2–§3): MIDI clock
//! (`F8`, Start, Continue, Stop, the song position pointer) and time code (quarter frames,
//! full frames) reach the sync code with **their arrival time**, never through the audio
//! thread's note path. Each port is told what it gives (Settings ▸ MIDI); what a port is not
//! taken for is dropped here, as every system message was before.
//!
//! **The arrival time.** midir stamps each message (µs from a port-dependent origin: the
//! driver's own stamp on ALSA and CoreMIDI, WinMM's millisecond on Windows). The stamp is
//! mapped to the host clock by the smallest *host − stamp* seen on the port — the message
//! that was delivered fastest — relaxed by 2 µs a message so a drift between the two clocks
//! is followed. A callback woken late then no longer moves the time a clock is filtered at.

use rtrb::{Consumer, Producer, RingBuffer};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Mutex;

use super::FILTER_PORTS;

/// A port gives MIDI clock.
pub const TAKES_CLOCK: u8 = 1;
/// A port gives time code.
pub const TAKES_MTC: u8 = 2;
/// A port's notes (and key pressure) are kept from the engine and the controller map.
pub const BLOCKS_NOTES: u8 = 4;
/// A port's controllers, programs, channel pressure and pitch bend are.
pub const BLOCKS_CONTROLLERS: u8 = 8;
/// How many system messages wait for the sync thread.
pub const SYNC_QUEUE: usize = 4096;

/// A system message as it came: the port (its index among the open ones), the host time
/// it arrived (µs, `transport::sync::host_micros`), its bytes.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct SyncMsg {
    pub port: u8,
    pub at: i64,
    pub len: u8,
    pub bytes: [u8; 10],
}

impl SyncMsg {
    pub fn msg(&self) -> &[u8] {
        &self.bytes[..usize::from(self.len)]
    }
}

/// Where the ports' system messages go.
#[derive(Debug)]
pub struct SyncInput {
    takes: [AtomicU8; FILTER_PORTS],
    tx: Mutex<Producer<SyncMsg>>,
    rx: Mutex<Option<Consumer<SyncMsg>>>,
}

impl Default for SyncInput {
    fn default() -> Self {
        let (tx, rx) = RingBuffer::new(SYNC_QUEUE);
        SyncInput {
            takes: std::array::from_fn(|_| AtomicU8::new(0)),
            tx: Mutex::new(tx),
            rx: Mutex::new(Some(rx)),
        }
    }
}

impl SyncInput {
    /// What port `index` gives ([`TAKES_CLOCK`], [`TAKES_MTC`]).
    pub fn set_takes(&self, index: usize, takes: u8) {
        if let Some(t) = self.takes.get(index) {
            t.store(takes, Ordering::Relaxed);
        }
    }

    /// Whether port `index` lets a channel message with `status` through (Settings ▸ MIDI:
    /// a port's notes and controllers can each be turned off).
    pub fn admits(&self, index: usize, status: u8) -> bool {
        let t = self.takes(index);
        match status & 0xF0 {
            0x80 | 0x90 | 0xA0 => t & BLOCKS_NOTES == 0,
            0xB0 | 0xC0 | 0xD0 | 0xE0 => t & BLOCKS_CONTROLLERS == 0,
            _ => true,
        }
    }

    pub fn takes(&self, index: usize) -> u8 {
        self.takes
            .get(index)
            .map_or(0, |t| t.load(Ordering::Relaxed))
    }

    /// The receiving end, once (the sync thread's).
    pub fn take_receiver(&self) -> Option<Consumer<SyncMsg>> {
        self.rx.lock().ok()?.take()
    }

    /// A port's message: `true` if it was a system message (kept for sync, or dropped as
    /// every system message is) — the caller's note path then never sees it.
    pub fn route(&self, index: usize, at: i64, bytes: &[u8]) -> bool {
        let Some(&status) = bytes.first() else {
            return false;
        };
        if status < 0xF0 {
            return false;
        }
        let kind = match status {
            0xF8 | 0xFA | 0xFB | 0xFC | 0xF2 => TAKES_CLOCK,
            0xF1 => TAKES_MTC,
            0xF0 if bytes.len() >= 10 && bytes[1..5] == [0x7F, 0x7F, 0x01, 0x01] => TAKES_MTC,
            _ => 0,
        };
        if kind != 0 && self.takes(index) & kind != 0 {
            let mut m = SyncMsg {
                port: index as u8,
                at,
                len: bytes.len().min(10) as u8,
                ..SyncMsg::default()
            };
            m.bytes[..usize::from(m.len)].copy_from_slice(&bytes[..usize::from(m.len)]);
            if let Ok(mut tx) = self.tx.lock() {
                let _ = tx.push(m);
            }
        }
        true
    }
}

/// A port's mapping from midir's stamps to the host clock.
#[derive(Copy, Clone, Debug, Default)]
pub struct StampClock {
    offset: Option<i64>,
}

impl StampClock {
    /// The host time a message stamped `stamp` (µs) arrived, now being `now`.
    pub fn arrival(&mut self, stamp: u64, now: i64) -> i64 {
        let d = now - stamp as i64;
        let off = match self.offset {
            Some(o) => (o + 2).min(d),
            None => d,
        };
        self.offset = Some(off);
        (stamp as i64 + off).min(now)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_port_gives_only_what_it_is_taken_for() {
        let s = SyncInput::default();
        let mut rx = s.take_receiver().unwrap();
        assert!(s.take_receiver().is_none());
        // Not taken: dropped, and still kept from the note path.
        assert!(s.route(0, 1, &[0xF8]));
        assert!(rx.pop().is_err());
        s.set_takes(0, TAKES_CLOCK);
        assert!(s.route(0, 2, &[0xF8]));
        assert!(s.route(0, 3, &[0xF1, 0x12]));
        assert!(!s.route(0, 4, &[0x90, 60, 100]));
        let m = rx.pop().unwrap();
        assert_eq!((m.port, m.at, m.msg()), (0, 2, &[0xF8][..]));
        assert!(rx.pop().is_err(), "time code was not taken");
        // Notes and controllers turned off on a port.
        assert!(s.admits(0, 0x90) && s.admits(0, 0xB0));
        s.set_takes(0, TAKES_CLOCK | BLOCKS_NOTES);
        assert!(!s.admits(0, 0x93) && !s.admits(0, 0x80) && s.admits(0, 0xE0));
        s.set_takes(0, BLOCKS_CONTROLLERS);
        assert!(s.admits(0, 0x90) && !s.admits(0, 0xB1) && !s.admits(0, 0xE0));
        s.set_takes(0, TAKES_MTC);
        let ff = [0xF0, 0x7F, 0x7F, 0x01, 0x01, 0x21, 0, 0, 0, 0xF7];
        assert!(s.route(0, 5, &ff));
        assert_eq!(rx.pop().unwrap().msg(), &ff);
    }

    /// A late callback (its host time late) does not move the arrival: the fastest one
    /// sets the line.
    #[test]
    fn a_late_callback_keeps_the_stamps_time() {
        let mut c = StampClock::default();
        assert_eq!(c.arrival(1_000, 501_000), 501_000);
        // 3 ms late: still at the stamp's own time.
        assert_eq!(c.arrival(21_000, 524_000), 521_002);
        // Faster than before: the line moves to it.
        assert_eq!(c.arrival(41_000, 540_500), 540_500);
    }
}

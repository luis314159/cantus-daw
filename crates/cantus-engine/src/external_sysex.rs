//! SysEx from a control surface's port (ADR 0095 §6).
//!
//! The MIDI callbacks send every message from `F0` up to the clock and time-code follower.
//! A port a surface takes whole (`FilterSet::take_port`) is the surface's, SysEx included: a
//! Mackie Control's *Host Connection Query* (`F0 00 00 66 14 01 …`) must reach the surface,
//! which answers it. The callback keeps such a message in a fixed buffer and pushes it into a
//! ring of its own port (no lock, no allocation), and wakes the interface.

use crate::midi::MidiTap;
use rtrb::Producer;

/// The longest SysEx kept from a surface (a Mackie connection query is 18 bytes).
pub const SYSEX_MAX: usize = 64;
/// SysEx messages a surface's port keeps for the interface.
pub const SYSEX_QUEUE: usize = 64;

/// One SysEx message from a port.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Sysex {
    pub port: u8,
    pub len: u8,
    pub bytes: [u8; SYSEX_MAX],
}

impl Sysex {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes[..usize::from(self.len)]
    }
}

/// A SysEx (or other system) message from port `port`: taken (`true`) when the port is a
/// surface's, kept for the interface if it is SysEx short enough; `false` sends it on to
/// the follower as before. Lock-free, allocation-free.
pub fn route_sysex(bytes: &[u8], port: usize, tap: &MidiTap, ring: &mut Producer<Sysex>) -> bool {
    if bytes.first() != Some(&0xF0) || !tap.filter().takes_whole(port) {
        return false;
    }
    if bytes.len() <= SYSEX_MAX {
        let mut b = [0u8; SYSEX_MAX];
        b[..bytes.len()].copy_from_slice(bytes);
        let _ = ring.push(Sysex {
            port: port.min(u8::MAX as usize) as u8,
            len: bytes.len() as u8,
            bytes: b,
        });
        tap.wake();
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::midi::FilterSet;

    /// TouchMCU's *Host Connection Query* (`F0 00 00 66 14 01`, a 7-byte serial, a 4-byte
    /// challenge, `F7`) from a surface's port reaches the surface's ring; from any other port
    /// it goes on to the follower; a clock byte from the surface's port is not taken.
    #[test]
    fn a_surfaces_sysex_reaches_the_surface_and_others_go_on_as_before() {
        let tap = MidiTap::new(None);
        let mut set = FilterSet::default();
        set.take_port(2);
        tap.filter().publish(&set);
        let (mut tx, mut rx) = rtrb::RingBuffer::<Sysex>::new(SYSEX_QUEUE);
        let query = [
            0xF0, 0x00, 0x00, 0x66, 0x14, 0x01, b'A', b'B', b'C', b'1', b'2', b'3', b'4', 0x10,
            0x20, 0x30, 0x40, 0xF7,
        ];
        assert!(route_sysex(&query, 2, &tap, &mut tx));
        assert_eq!(rx.pop().unwrap().bytes(), &query);
        assert!(
            !route_sysex(&query, 1, &tap, &mut tx),
            "another port's: the follower's"
        );
        assert!(rx.pop().is_err());
        assert!(
            !route_sysex(&[0xF8], 2, &tap, &mut tx),
            "a clock is not SysEx"
        );
    }
}

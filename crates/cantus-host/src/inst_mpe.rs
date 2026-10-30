//! **MPE out** for a hosted instrument that takes MIDI only (round 33, ADR 0080): each
//! sounding note on a member channel of a lower zone of its own, its bend, pressure and
//! timbre as that channel's pitch bend (±48 semitones), channel pressure and CC 74, the
//! zone announced first — its MCM and the members' bend range — as MPE 1.1 asks of a
//! sender. Channels are handed out least recently used first (§3.3), so a release tail
//! keeps its channel as long as it can; with all fifteen sounding, the oldest note gives
//! its channel up (it is released first).
//!
//! Fixed arrays only: it runs on the audio thread.

use cantus_core::model::note_expr::{midi2, ExprKind};

/// The zone's member channels: 1…15 (channel 2…16).
const MEMBERS: u8 = 15;

/// One MIDI channel message to send, `(status with channel, data1, data2)`.
pub type Msg = (u8, u8, u8);

#[derive(Debug, Default)]
pub struct MpeOut {
    /// The note each member channel sounds (its plug-in id), if any.
    busy: [Option<i32>; 16],
    /// When each was taken or let go last (a counter), for least recently used.
    used: [u64; 16],
    clock: u64,
    /// The zone was announced since the last reset.
    pub announced: bool,
    /// The last message of each kind sent on each channel (`None`: at rest).
    left: [[Option<Msg>; 3]; 16],
}

impl MpeOut {
    /// The zone's announcement: the MCM on channel 1 and RPN 0 = 48 on each member, each
    /// RPN closed with the null RPN.
    pub fn announce(&mut self, mut send: impl FnMut(Msg)) {
        self.announced = true;
        for m in [
            (0xB0, 101, 0),
            (0xB0, 100, 6),
            (0xB0, 6, MEMBERS),
            (0xB0, 101, 127),
            (0xB0, 100, 127),
        ] {
            send(m);
        }
        for ch in 1..=MEMBERS {
            let cc = 0xB0 | ch;
            for m in [
                (cc, 101, 0),
                (cc, 100, 0),
                (cc, 6, 48),
                (cc, 38, 0),
                (cc, 101, 127),
                (cc, 100, 127),
            ] {
                send(m);
            }
        }
    }

    /// A channel for note `id`; when every member sounds, the one taken longest ago, whose
    /// note is returned to be released first.
    pub fn take(&mut self, id: i32) -> (u8, Option<i32>) {
        self.clock += 1;
        let free = (1..=MEMBERS)
            .filter(|&c| self.busy[c as usize].is_none())
            .min_by_key(|&c| (self.used[c as usize], c));
        let (ch, stolen) = match free {
            Some(c) => (c, None),
            None => {
                let c = (1..=MEMBERS)
                    .min_by_key(|&c| (self.used[c as usize], c))
                    .unwrap_or(1);
                (c, self.busy[c as usize])
            }
        };
        self.busy[ch as usize] = Some(id);
        self.used[ch as usize] = self.clock;
        (ch, stolen)
    }

    /// Note `id` let go: its channel is free (and remembers when).
    pub fn free(&mut self, id: i32) {
        self.clock += 1;
        for c in 1..=MEMBERS as usize {
            if self.busy[c] == Some(id) {
                self.busy[c] = None;
                self.used[c] = self.clock;
            }
        }
    }

    /// A value sent on `channel`.
    pub fn sent(&mut self, channel: u8, kind: ExprKind, msg: Msg) {
        let rest = Self::message(channel, kind, kind.read(kind.rest()) as f32);
        self.left[channel as usize & 15][kind as usize] = (msg != rest).then_some(msg);
    }

    /// The message that puts `kind` back to rest on `channel`, when its last note left it
    /// elsewhere.
    pub fn back_to_rest(&mut self, channel: u8, kind: ExprKind) -> Option<Msg> {
        self.left[channel as usize & 15][kind as usize]
            .take()
            .map(|_| Self::message(channel, kind, kind.read(kind.rest()) as f32))
    }

    /// A seek, a panic, a new route: every member channel's state is unknown now (the plug-in
    /// keeps what it last heard), so its first use puts all three back to rest (review D1).
    pub fn reset(&mut self) {
        *self = MpeOut::default();
        for ch in 1..=MEMBERS as usize {
            self.left[ch] = [Some((0, 0, 0)); 3];
        }
    }

    /// A per-note value as its channel's message: the bend in semitones, pressure and
    /// timbre 0…1; `None` for what MPE does not carry per note.
    pub fn message(channel: u8, kind: ExprKind, value: f32) -> Msg {
        let raw = kind.store(f64::from(value));
        match kind {
            ExprKind::Bend => {
                let b = midi2::bend_to_14(raw, 48);
                (0xE0 | channel, (b & 127) as u8, (b >> 7) as u8)
            }
            ExprKind::Pressure => (0xD0 | channel, midi2::unit_to_7(raw), 0),
            ExprKind::Timbre => (0xB0 | channel, 74, midi2::unit_to_7(raw)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channels_go_least_recently_used_and_the_oldest_gives_way() {
        let mut m = MpeOut::default();
        let mut msgs = Vec::new();
        m.announce(|x| msgs.push(x));
        assert_eq!(&msgs[..3], &[(0xB0, 101, 0), (0xB0, 100, 6), (0xB0, 6, 15)]);
        assert_eq!(msgs.len(), 5 + 15 * 6);
        let (a, _) = m.take(10);
        let (b, _) = m.take(11);
        assert_ne!(a, b);
        m.free(10);
        // A free channel let go just now is the last choice: 3 is fresher.
        let (c, _) = m.take(12);
        assert_ne!(c, a);
        for id in 13..13 + 13 {
            m.take(id);
        }
        // All fifteen sound: the oldest (note 11 on b) gives way.
        assert_eq!(m.take(99), (b, Some(11)));
        assert_eq!(MpeOut::message(3, ExprKind::Bend, 12.0), (0xE3, 0, 80));
        assert_eq!(MpeOut::message(3, ExprKind::Timbre, 1.0), (0xB3, 74, 127));
    }
}

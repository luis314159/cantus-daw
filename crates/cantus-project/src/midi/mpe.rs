//! **MPE in Standard MIDI Files** (round 33, ADR 0080).
//!
//! *Import.* A track whose channels are an MPE zone — said by its MCM (RPN 6 on channel 1
//! or 16), or guessed: notes on two or more member channels, each holding one note at a
//! time, at least one of them bent while it sounds — is one Cantus track: its member
//! channels' notes are recombined onto the master channel, each with the curves its own
//! channel played (pitch bend at the zone's bend range, channel pressure, CC 74), the value
//! a channel held when the note began its first point. Key pressure, on any track, is the
//! pressure curve of the note it names (it was dropped before).
//!
//! *Export.* A track whose notes carry expression is written as a lower zone: its MCM and
//! the members' bend range (RPN 0, ±48) at the start, every note on a member channel of
//! its own (the least recently used free one, as MPE §3.3 asks of a sender), its bend,
//! pressure and timbre as that channel's pitch bend, channel pressure and CC 74 — before its
//! note-on where it starts away from rest, then as the curve moves.

use super::smf::{Event, TimedEvent};
use cantus_core::model::note_expr::zones::{MpeZones, Role, Zone};
use cantus_core::model::note_expr::{midi2, ops, ExprKind, ExprPoint, NoteExpr};

/// The MPE reading of one file track.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Layout {
    pub zones: MpeZones,
    /// Each channel's bend range in semitones (the members' 48, the masters' 2, unless an
    /// RPN 0 said otherwise).
    pub bend: [u8; 16],
}

impl Layout {
    /// The channel a note or controller of `channel` belongs to: a member's is its zone's
    /// master.
    pub fn track_channel(&self, channel: u8) -> u8 {
        match self.zones.role(channel) {
            Role::Member { upper: false } => 0,
            Role::Member { upper: true } => 15,
            _ => channel,
        }
    }

    pub fn is_member(&self, channel: u8) -> bool {
        matches!(self.zones.role(channel), Role::Member { .. })
    }
}

/// The track's MPE layout: from its MCM, else guessed from how it plays; `None` for an
/// ordinary track.
pub fn layout_of(track: &[TimedEvent], near: u64) -> Option<Layout> {
    let mut rpn = [(127u8, 127u8); 16];
    let mut zones: Option<MpeZones> = None;
    let mut bend = [48u8; 16];
    bend[0] = 2;
    bend[15] = 2;
    let mut told = [false; 16];
    // Guessing: which channels play notes, whether any holds two at once, which bend.
    let mut held = [0u32; 16];
    let mut poly = [false; 16];
    let mut noted = [false; 16];
    let mut bent = [false; 16];
    // Parts of their own: a program change on a channel; how many member notes sound at once.
    let mut programmed = [false; 16];
    let (mut together, mut most) = (0u32, 0u32);
    for te in track {
        match te.event {
            Event::Program { channel, .. } => programmed[channel as usize & 15] = true,
            Event::Controller {
                channel,
                controller,
                value,
            } => {
                let c = channel as usize & 15;
                match controller {
                    101 => rpn[c].0 = value,
                    100 => rpn[c].1 = value,
                    6 => match rpn[c] {
                        (0, 6) if c == 0 || c == 15 => {
                            zones = Some(zones.unwrap_or_default().after_mcm(c as u8, value));
                        }
                        (0, 0) => {
                            bend[c] = value.clamp(1, 96);
                            told[c] = true;
                        }
                        _ => {}
                    },
                    _ => {}
                }
            }
            Event::NoteOn {
                channel, velocity, ..
            } if velocity > 0 => {
                let c = channel as usize & 15;
                noted[c] = true;
                held[c] += 1;
                poly[c] |= held[c] > 1;
                if c != 0 {
                    together += 1;
                    most = most.max(together);
                }
            }
            Event::NoteOn { channel, .. } | Event::NoteOff { channel, .. } => {
                let c = channel as usize & 15;
                if c != 0 && held[c] > 0 {
                    together -= 1;
                }
                held[c] = held[c].saturating_sub(1);
            }
            Event::PitchBend { channel, value } if value != 8192 => {
                let c = channel as usize & 15;
                bent[c] |= held[c] > 0;
            }
            _ => {}
        }
    }
    let zones = match zones {
        Some(z) if z.is_on() => z,
        _ => {
            // Guess a lower zone only on MPE's own signature (review D6): its notes come
            // each with its channel's own setup — a bend, pressure or CC 74 at the note-on
            // (within `near` ticks), which a controller sends for every note it hands a
            // channel — on two or more channels, each one note at a time, none with a
            // program of its own. A rotation may start anywhere and skip channels (a
            // LinnStrument's channel per row), and a line may be one note at a time. Parts
            // of an ordinary file bend now and then, not at every note.
            let _ = (most, bent);
            let members: Vec<usize> = (1..16).filter(|&c| noted[c]).collect();
            if members.len() < 2 || members.iter().any(|&c| poly[c] || programmed[c]) {
                return None;
            }
            let (notes, set) = onset_setup(track, near, &members);
            if notes < 4 || set * 4 < notes * 3 {
                return None;
            }
            MpeZones {
                lower: Some(Zone::new(*members.last()? as u8)),
                upper: None,
            }
        }
    };
    // A bend range said on one member holds for the zone's members (MPE §2.4).
    for upper in [false, true] {
        let ms: Vec<u8> = zones.members(upper).collect();
        if let Some(&c) = ms.iter().find(|&&c| told[c as usize]) {
            let r = bend[c as usize];
            for &m in &ms {
                if !told[m as usize] {
                    bend[m as usize] = r;
                }
            }
        }
    }
    Some(Layout { zones, bend })
}

/// How many note-ons the `channels` play, and how many of them come with their channel's
/// own bend, pressure or CC 74 within `near` ticks.
fn onset_setup(track: &[TimedEvent], near: u64, channels: &[usize]) -> (usize, usize) {
    let mut ons: Vec<(usize, u64)> = Vec::new();
    let mut setup: Vec<Vec<u64>> = vec![Vec::new(); 16];
    for te in track {
        match te.event {
            Event::NoteOn {
                channel, velocity, ..
            } if velocity > 0 => ons.push((channel as usize & 15, te.tick)),
            Event::PitchBend { channel, .. }
            | Event::ChannelPressure { channel, .. }
            | Event::Controller {
                channel,
                controller: 74,
                ..
            } => setup[channel as usize & 15].push(te.tick),
            _ => {}
        }
    }
    let mine: Vec<_> = ons.iter().filter(|(c, _)| channels.contains(c)).collect();
    let set = mine
        .iter()
        .filter(|(c, t)| {
            let s = &setup[*c];
            let i = s.partition_point(|x| *x + near < *t);
            s.get(i).is_some_and(|x| *x <= t + near)
        })
        .count();
    (mine.len(), set)
}

/// A member channel's expression event: `(file tick, channel, kind, stored value)`.
pub type Sample = (u64, u8, ExprKind, i64);

/// The expression a member channel's (or any channel's key pressure) event carries.
pub fn sample_of(
    layout: Option<&Layout>,
    e: &TimedEvent,
) -> Option<(u8, Option<u8>, ExprKind, i64)> {
    match e.event {
        Event::PolyPressure {
            channel,
            key,
            value,
        } => Some((
            channel,
            Some(key),
            ExprKind::Pressure,
            midi2::unit_of_7(value),
        )),
        Event::PitchBend { channel, value } if layout.is_some_and(|l| l.is_member(channel)) => {
            let range = layout.map_or(48, |l| l.bend[channel as usize & 15]);
            Some((
                channel,
                None,
                ExprKind::Bend,
                midi2::bend_of_14(value, range),
            ))
        }
        Event::ChannelPressure { channel, value }
            if layout.is_some_and(|l| l.is_member(channel)) =>
        {
            Some((channel, None, ExprKind::Pressure, midi2::unit_of_7(value)))
        }
        Event::Controller {
            channel,
            controller: 74,
            value,
        } if layout.is_some_and(|l| l.is_member(channel)) => {
            Some((channel, None, ExprKind::Timbre, midi2::unit_of_7(value)))
        }
        _ => None,
    }
}

/// One note's curves from its channel's samples (sorted by tick): the value each kind held
/// at the note-on, then every change until the note-off; ticks from the note's start in
/// Cantus ticks (`to` converts a file tick). Key pressure counts only for its own key.
pub fn curves(
    samples: &[(u64, u8, Option<u8>, ExprKind, i64)],
    channel: u8,
    key: u8,
    freed: u64,
    on: u64,
    off: u64,
    to: impl Fn(u64) -> i64,
) -> Option<Box<NoteExpr>> {
    // Only the note's own lifetime: from when its channel was let go by the note before
    // (what is sent between is this note's start) to its note-off (review fix 4). The
    // samples are in time order: the slice is found, not scanned.
    // A value on the tick the note before let go is that note's last, not this one's
    // first — unless this note begins on that very tick (review D-nit).
    let from = if freed > 0 && freed < on {
        freed + 1
    } else {
        freed
    };
    let a = samples.partition_point(|s| s.0 < from);
    let z = samples.partition_point(|s| s.0 < off);
    let window = &samples[a..z.max(a)];
    let mut e = NoteExpr::default();
    let start = to(on);
    for kind in ExprKind::ALL {
        let mine = window
            .iter()
            .filter(|s| s.1 == channel && s.3 == kind && s.2.is_none_or(|k| k == key));
        let mut c: Vec<ExprPoint> = Vec::new();
        let mut before = None;
        for s in mine {
            if s.0 <= on {
                before = Some(s.4);
            } else {
                if c.is_empty() {
                    if let Some(v) = before {
                        c.push(ExprPoint(0, v));
                    }
                }
                c.push(ExprPoint(to(s.0) - start, s.4));
            }
        }
        if c.is_empty() {
            if let Some(v) = before {
                // Held all through: a key pressure at the note-on, a bend set before it.
                if v != kind.rest() {
                    c.push(ExprPoint(0, v));
                }
            }
        }
        // Thinned as a recorded take is (a cent, a thousandth), so a long played note
        // keeps its whole curve under the point limit (review fix 6).
        *e.curve_mut(kind) = ops::thin(&c, ops::default_tolerance(kind));
    }
    e.normalised().boxed()
}

/// Hands out member channels for an export: the least recently used free one.
#[derive(Debug)]
pub struct Rotation {
    members: Vec<u8>,
    /// For each member: the tick it is busy until, and when it was last let go.
    busy_until: [u64; 16],
    freed: [u64; 16],
}

impl Rotation {
    /// Over these member channels.
    pub fn of(members: &[u8]) -> Rotation {
        Rotation {
            members: members.to_vec(),
            busy_until: [0; 16],
            freed: [0; 16],
        }
    }

    pub fn new(zones: &MpeZones) -> Rotation {
        Rotation {
            members: zones.members(false).collect(),
            busy_until: [0; 16],
            freed: [0; 16],
        }
    }

    /// A channel for a note sounding over `[on, off)` (file ticks), its release tail
    /// counted until `off`: the free one let go longest ago; when every one is busy, the
    /// one that frees first (its note is then cut, as on any MPE sender out of channels).
    pub fn take(&mut self, on: u64, off: u64) -> u8 {
        let free = self
            .members
            .iter()
            .copied()
            .filter(|&c| self.busy_until[c as usize] <= on)
            .min_by_key(|&c| (self.freed[c as usize], c));
        let c = free.unwrap_or_else(|| {
            self.members
                .iter()
                .copied()
                .min_by_key(|&c| (self.busy_until[c as usize], c))
                .unwrap_or(1)
        });
        self.busy_until[c as usize] = off;
        self.freed[c as usize] = off;
        c
    }
}

/// The messages that announce a zone: the MCM on its manager `master` (channel 1 for the
/// lower zone, 16 for the upper) with the members' count, then RPN 0 = 48 on each of its
/// `members` only — no other track's channel is touched — each RPN closed with the null.
pub fn announce_zone(master: u8, members: &[u8]) -> Vec<Event> {
    let cc = |channel: u8, controller: u8, value: u8| Event::Controller {
        channel,
        controller,
        value,
    };
    let n = members.len().clamp(1, 15) as u8;
    let mut out = vec![
        cc(master, 101, 0),
        cc(master, 100, 6),
        cc(master, 6, n),
        cc(master, 101, 127),
        cc(master, 100, 127),
    ];
    for &m in members {
        out.extend([
            cc(m, 101, 0),
            cc(m, 100, 0),
            cc(m, 6, 48),
            cc(m, 38, 0),
            cc(m, 101, 127),
            cc(m, 100, 127),
        ]);
    }
    out
}

/// The messages that announce a lower zone of `members` at the members' bend range of 48:
/// the MCM on the master, then RPN 0 on each member, each RPN closed with the null RPN.
pub fn announce(members: u8) -> Vec<Event> {
    let cc = |channel: u8, controller: u8, value: u8| Event::Controller {
        channel,
        controller,
        value,
    };
    let mut out = vec![
        cc(0, 101, 0),
        cc(0, 100, 6),
        cc(0, 6, members.clamp(1, 15)),
        cc(0, 101, 127),
        cc(0, 100, 127),
    ];
    for m in MpeZones::default().after_mcm(0, members).members(false) {
        out.extend([
            cc(m, 101, 0),
            cc(m, 100, 0),
            cc(m, 6, 48),
            cc(m, 38, 0),
            cc(m, 101, 127),
            cc(m, 100, 127),
        ]);
    }
    out
}

/// A curve's value as the member channel's message (bend at ±48).
pub fn message(channel: u8, kind: ExprKind, raw: i64) -> Event {
    match kind {
        ExprKind::Bend => Event::PitchBend {
            channel,
            value: midi2::bend_to_14(raw, 48),
        },
        ExprKind::Pressure => Event::ChannelPressure {
            channel,
            value: midi2::unit_to_7(raw),
        },
        ExprKind::Timbre => Event::Controller {
            channel,
            controller: 74,
            value: midi2::unit_to_7(raw),
        },
    }
}

/// For each note `(on, off, key, velocity, release, channel)`, the latest note-off on its
/// channel at or before its note-on (0 when none): where its own values may start.
pub fn freed(notes: &[(u64, u64, u8, u8, u8, u8)]) -> Vec<u64> {
    use std::cmp::Reverse;
    use std::collections::BinaryHeap;
    let mut order: Vec<usize> = (0..notes.len()).collect();
    order.sort_by_key(|&i| notes[i].0);
    let mut pending: Vec<BinaryHeap<Reverse<u64>>> = vec![BinaryHeap::new(); 16];
    let mut last = [0u64; 16];
    let mut out = vec![0; notes.len()];
    for i in order {
        let (on, off, ch) = (notes[i].0, notes[i].1, notes[i].5 as usize & 15);
        while let Some(Reverse(o)) = pending[ch].peek().copied() {
            if o > on {
                break;
            }
            last[ch] = last[ch].max(o);
            pending[ch].pop();
        }
        out[i] = last[ch];
        pending[ch].push(Reverse(off));
    }
    out
}

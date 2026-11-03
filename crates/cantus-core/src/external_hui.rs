//! Digidesign/Mackie HUI (ADR 0095): zones and ports over plain MIDI.
//!
//! **The source** of every number here is *MACKIE HUI MIDI protocol — the results of a 2-day
//! reverse-engineering session by theageman* (2010, © 2011 SSEI, www.ssei-online.de), as
//! published at <https://forum.bome.com/uploads/short-url/aX7AWqj6FpgMiEMQi0kACDDTSSi.pdf>;
//! its pages are cited beside each table, and `tests/external_surface.rs` holds the bytes it
//! prints. Mackie never published the protocol.
//!
//! HUI hangs every switch on a *zone* (`00`–`1d`) and a *port* (0–7). The unit selects with
//! `B0 0F zz`, then `B0 2F yp` (y `40` pressed, `00` released); the host lights an LED with
//! `B0 0C zz`, `B0 2C yp` (pp. 9–10; "the led … has the same zone/port pair as the
//! corresponding button", p. 7). Faders are `B0 0z hi`, `B0 2z lo`, value = hi << 7 + lo, both
//! ways (pp. 7, 11). A V-pot turns as `B0 4p vv` and the jog wheel as `B0 0D vv`, where
//! **vv > 40 is +(vv − 40) and vv < 40 is −vv** (pp. 10–11). A ring is `B0 1y vv` (v 1–b one
//! LED, 6 the centre; + 40 the LED under it, p. 6). A strip's four-character display is
//! `F0 00 00 66 05 00 10 yy gg hh ii jj F7` (p. 4). The **ping** is the host's: `90 00 00`
//! about every second, the unit answers `90 00 7F`, and without pings for about 2 s it goes
//! offline — its faders then stop answering (p. 4, p. 10).
//!
//! **Mapped** (the zone table, p. 14): the eight channel strips; channel and bank ◂ ▸ (zone
//! 0a); shift, undo and save (zone 08); cursor ◂ ▸ and scrub (zone 0d); the transport (zone
//! 0e) and loop and return-to-zero (zone 0f); the jog wheel. Other zones are read and logged
//! by zone and port (`Decoded::unmapped`).

use crate::external_surface::*;

/// A channel strip's ports: zones 00–07 (p. 14: "fader select mute solo auto v-sel insert
/// rec/rdy").
pub mod port {
    pub const FADER: u8 = 0;
    pub const SELECT: u8 = 1;
    pub const MUTE: u8 = 2;
    pub const SOLO: u8 = 3;
    pub const AUTO: u8 = 4;
    pub const VSEL: u8 = 5;
    pub const INSERT: u8 = 6;
    pub const REC: u8 = 7;
}

/// Zone 08, keyboard shortcuts (p. 14: "ctrl/clt shift/ad editmode undo alt/fine option/a
/// edittool save").
pub const KEYS_ZONE: u8 = 0x08;
pub mod keys {
    pub const SHIFT: u8 = 1;
    pub const UNDO: u8 = 3;
    pub const SAVE: u8 = 7;
}

/// Zone 0a, channel selection (p. 14: "<- chanl, <- bank, chanl ->, bank ->"; p. 7: "the
/// led for the 'next bank' button is zone 0a, port 3").
pub const CHANNEL_ZONE: u8 = 0x0A;
pub mod channel {
    pub const LEFT: u8 = 0;
    pub const BANK_LEFT: u8 = 1;
    pub const RIGHT: u8 = 2;
    pub const BANK_RIGHT: u8 = 3;
}

/// Zone 0d, cursor movement/mode/scrub/shuttle (p. 14: "down left mode right up scrub
/// shuttle").
pub const CURSOR_ZONE: u8 = 0x0D;
pub mod cursor {
    pub const LEFT: u8 = 1;
    pub const RIGHT: u8 = 3;
    pub const SCRUB: u8 = 5;
}

/// Zone 0e, the transport's big switches (p. 14: "talkback rewind fast fwd stop play
/// record").
pub const TRANSPORT_ZONE: u8 = 0x0E;
pub mod transport {
    pub const TALKBACK: u8 = 0;
    pub const REWIND: u8 = 1;
    pub const FORWARD: u8 = 2;
    pub const STOP: u8 = 3;
    pub const PLAY: u8 = 4;
    pub const RECORD: u8 = 5;
}

/// Zone 0f, loop/rtz/end (p. 14: "|<rtz end>| on line loop qck pnch").
pub const LOOP_ZONE: u8 = 0x0F;
pub mod looping {
    pub const RTZ: u8 = 0;
    pub const END: u8 = 1;
    pub const LOOP: u8 = 3;
}

/// The jog wheel's controller (p. 11: `b0 0d vv`).
pub const JOG: u8 = 0x0D;

/// HUI's relative value as signed ticks: above `40` is positive by the excess, below is
/// negative by the value (p. 10).
pub fn delta_of(v: u8) -> i8 {
    let v = v & 0x7F;
    if v > 0x40 {
        (v - 0x40) as i8
    } else {
        -(v as i8)
    }
}

/// Signed ticks as the value a HUI sends (the emulation's and the tests').
pub fn value_of(delta: i8) -> u8 {
    let a = delta.unsigned_abs().min(0x3F);
    if delta > 0 {
        0x40 + a
    } else {
        a
    }
}

/// The event of a zone's port, if it is mapped.
pub fn event_of(zone: u8, port: u8, down: bool) -> Option<SurfaceEvent> {
    let button = |b| Some(SurfaceEvent::Button { button: b, down });
    match zone {
        0..=7 => match port {
            port::FADER => Some(SurfaceEvent::Touch {
                channel: zone,
                on: down,
            }),
            port::SELECT => button(Button::Select(zone)),
            port::MUTE => button(Button::Mute(zone)),
            port::SOLO => button(Button::Solo(zone)),
            port::REC => button(Button::Rec(zone)),
            port::VSEL => button(Button::VPush(zone)),
            _ => None,
        },
        KEYS_ZONE => match port {
            keys::SHIFT => button(Button::Shift),
            keys::UNDO => button(Button::Undo),
            keys::SAVE => button(Button::Save),
            _ => None,
        },
        CHANNEL_ZONE => match port {
            channel::LEFT => button(Button::ChannelLeft),
            channel::BANK_LEFT => button(Button::BankLeft),
            channel::RIGHT => button(Button::ChannelRight),
            channel::BANK_RIGHT => button(Button::BankRight),
            _ => None,
        },
        CURSOR_ZONE => match port {
            cursor::LEFT => button(Button::Left),
            cursor::RIGHT => button(Button::Right),
            cursor::SCRUB => button(Button::Scrub),
            _ => None,
        },
        TRANSPORT_ZONE => match port {
            transport::REWIND => button(Button::Rewind),
            transport::FORWARD => button(Button::Forward),
            transport::STOP => button(Button::Stop),
            transport::PLAY => button(Button::Play),
            transport::RECORD => button(Button::Record),
            _ => None,
        },
        LOOP_ZONE => match port {
            looping::LOOP => button(Button::Cycle),
            looping::RTZ => button(Button::ReturnToZero),
            _ => None,
        },
        _ => None,
    }
}

/// What the host's decoder makes of a message: events, whether it was the unit's answer to
/// a ping, and the zones and ports that are not mapped (for the log).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Decoded {
    pub events: Vec<SurfaceEvent>,
    /// The unit answered a ping (`90 00 7F`).
    pub pong: bool,
    pub unmapped: Vec<(u8, u8)>,
}

/// The host's decoder: it remembers the zone the unit selected and the high half of each
/// fader.
#[derive(Clone, Debug, Default)]
pub struct Decoder {
    zone: u8,
    hi: [u8; 8],
}

impl Decoder {
    pub fn feed(&mut self, bytes: &[u8]) -> Decoded {
        let mut out = Decoded::default();
        let Some((&status, rest)) = bytes.split_first() else {
            return out;
        };
        let d1 = rest.first().copied().unwrap_or(0) & 0x7F;
        let d2 = rest.get(1).copied().unwrap_or(0) & 0x7F;
        match status & 0xF0 {
            0x90 if d1 == 0 && d2 == 0x7F => out.pong = true,
            0xB0 => match d1 {
                0x0F => self.zone = d2,
                0x2F => {
                    let (port, down) = (d2 & 0x07, d2 & 0x40 != 0);
                    match event_of(self.zone, port, down) {
                        Some(e) => out.events.push(e),
                        None => out.unmapped.push((self.zone, port)),
                    }
                }
                0x00..=0x07 => self.hi[usize::from(d1)] = d2,
                0x20..=0x27 => {
                    let ch = d1 - 0x20;
                    let v = u16::from(self.hi[usize::from(ch)]) << 7 | u16::from(d2);
                    out.events.push(SurfaceEvent::Fader {
                        channel: ch,
                        pos: from14(v),
                    });
                }
                0x40..=0x47 => out.events.push(SurfaceEvent::VPot {
                    channel: d1 - 0x40,
                    delta: delta_of(d2),
                }),
                JOG => out.events.push(SurfaceEvent::Jog {
                    delta: delta_of(d2),
                }),
                _ => {}
            },
            _ => {}
        }
        out
    }
}

// ---- host to unit ----------------------------------------------------------------------

/// The ping the host sends about every second (p. 4).
pub fn ping() -> Vec<u8> {
    vec![0x90, 0x00, 0x00]
}

/// A fader to `v` (14 bits): the high half then the low (p. 7).
pub fn fader(ch: u8, v: u16) -> [Vec<u8>; 2] {
    [
        vec![0xB0, ch, (v >> 7) as u8 & 0x7F],
        vec![0xB0, 0x20 + ch, (v & 0x7F) as u8],
    ]
}

/// A switch LED: select the zone, then the port with its state (p. 7).
pub fn led(zone: u8, port: u8, on: bool) -> [Vec<u8>; 2] {
    [
        vec![0xB0, 0x0C, zone],
        vec![0xB0, 0x2C, (port & 7) | if on { 0x40 } else { 0 }],
    ]
}

/// A V-pot's ring: position 0 (off) to 11 (one LED), and the LED under it (p. 6).
pub fn ring(ch: u8, pos: u8, under: bool) -> Vec<u8> {
    vec![0xB0, 0x10 + ch, (pos & 0x0F) | if under { 0x40 } else { 0 }]
}

/// The four-character name display of a channel strip (p. 4).
pub fn name(ch: u8, text: &str) -> Vec<u8> {
    let mut v = vec![0xF0, 0x00, 0x00, 0x66, 0x05, 0x00, 0x10, ch];
    let mut chars: Vec<u8> = text
        .bytes()
        .map(|c| {
            if c.is_ascii_graphic() || c == b' ' {
                c
            } else {
                b'?'
            }
        })
        .take(4)
        .collect();
    while chars.len() < 4 {
        chars.push(b' ');
    }
    v.extend(chars);
    v.push(0xF7);
    v
}

/// What to send a unit now: the ping when one is due, and only what changed since the last
/// render (everything after the unit came back online).
pub fn render(set: &mut SurfaceSet, dev: usize, view: &SurfaceView) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    let now = set.now_ms;
    {
        let d = &mut set.devices[dev];
        d.sent.online = true;
        if d.last_ping_ms.is_none_or(|t| now >= t + PING_EVERY_MS) {
            out.push(ping());
            d.last_ping_ms = Some(now);
        }
        if d.first_ping_ms.is_none() {
            d.first_ping_ms = Some(now);
        }
        // No answer for LOST_AFTER_MS while pinging: the unit is gone (its cable, its power).
        let since = d.last_pong_ms.or(d.first_ping_ms);
        if d.alive && since.is_some_and(|t| now > t + LOST_AFTER_MS) {
            d.alive = false;
            d.lost += 1;
        }
    }
    let flip = set.flip;
    struct Ch {
        fader: Option<u16>,
        ring: u8,
        leds: [(u8, bool); 4],
        name: String,
    }
    let mut chs = Vec::new();
    for c in 0..STRIPS {
        let s = bound(set, dev, c, view);
        chs.push(Ch {
            fader: s.map(|s| {
                if flip {
                    to14((s.pan + 1.0) / 2.0)
                } else {
                    fader_value(s.gain_db)
                }
            }),
            ring: s.map_or(0, |s| ring_of(s.pan)),
            leds: [
                (port::SELECT, s.is_some_and(|s| s.selected)),
                (port::MUTE, s.is_some_and(|s| s.mute)),
                (port::SOLO, s.is_some_and(|s| s.solo)),
                (port::REC, s.is_some_and(|s| s.arm)),
            ],
            name: s.map_or("    ".into(), |s| s.name.clone()),
        });
    }
    let d = &mut set.devices[dev];
    for (c, ch) in chs.iter().enumerate() {
        if let Some(v) = ch.fader {
            let echo = d.from_unit[c].take();
            if d.motor_may_move(c, now) && d.sent.faders[c] != Some(v) {
                if echo.is_none_or(|e| e.abs_diff(v) > 1) {
                    out.extend(fader(c as u8, v));
                }
                d.sent.faders[c] = Some(v);
            }
        }
        if d.sent.rings[c] != Some(ch.ring) {
            out.push(ring(c as u8, ch.ring, ch.ring == 6));
            d.sent.rings[c] = Some(ch.ring);
        }
        for (p, on) in ch.leds {
            let key = u16::from(c as u8) << 8 | u16::from(p);
            if d.sent.leds.get(&key) != Some(&u8::from(on)) {
                out.extend(led(c as u8, p, on));
                d.sent.leds.insert(key, u8::from(on));
            }
        }
        let text = name(c as u8, &ch.name);
        let code: u32 = text[8..12]
            .iter()
            .fold(0u32, |a, b| a.wrapping_mul(131).wrapping_add(u32::from(*b)));
        if d.sent.names[c] != Some(code) {
            out.push(text);
            d.sent.names[c] = Some(code);
        }
    }
    let t = &view.transport;
    for (zone, p, on) in [
        (TRANSPORT_ZONE, transport::STOP, !t.playing),
        (TRANSPORT_ZONE, transport::PLAY, t.playing),
        (TRANSPORT_ZONE, transport::RECORD, t.recording),
        (LOOP_ZONE, looping::LOOP, t.looping),
        (CURSOR_ZONE, cursor::SCRUB, t.scrubbing),
    ] {
        let key = u16::from(zone) << 8 | u16::from(p);
        if d.sent.leds.get(&key) != Some(&u8::from(on)) {
            out.extend(led(zone, p, on));
            d.sent.leds.insert(key, u8::from(on));
        }
    }
    out
}

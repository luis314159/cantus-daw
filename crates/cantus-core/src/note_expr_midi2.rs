//! MIDI 1.0 MPE and **MIDI 2.0** values for a note's expression (ADR 0080): the
//! translation layer between the curves' integers and what each protocol carries, and the
//! Universal MIDI Packets of MIDI 2.0's per-note messages, encoded and decoded.
//!
//! A curve holds MIDI 2.0's resolution (see the module above), so a MIDI 2.0 controller's
//! 32-bit per-note pressure is stored as it came, and a MIDI 1.0 controller's 7 or 14 bits
//! are scaled up with the *Min-Center-Max* rule of the MIDI 2.0 translation spec (M2-115,
//! §3), which keeps the centre exactly on the centre and the top on the top. The transport
//! (UMP endpoints: Windows MIDI Services, ALSA's UMP, CoreMIDI's) is not here: midir has no
//! UMP yet — what it would take is written down in ADR 0080.

use super::{ExprKind, BEND_UNIT, UNIT_MAX};

/// A MIDI 1.0 value of `src` bits scaled up to `dst` bits (M2-115 §3.3, Min-Center-Max).
pub fn scale_up(value: u32, src: u32, dst: u32) -> u32 {
    if src >= dst {
        return value >> (src - dst);
    }
    let shift = dst - src;
    let shifted = value << shift;
    let center = 1u32 << (src - 1);
    if value <= center {
        return shifted;
    }
    // The bits below the top bit, repeated to fill what the shift left empty.
    let rep_bits = src - 1;
    let rep_mask = (1u32 << rep_bits) - 1;
    let mut rep = value & rep_mask;
    rep = if shift > rep_bits {
        rep << (shift - rep_bits)
    } else {
        rep >> (rep_bits - shift)
    };
    let mut out = shifted;
    while rep != 0 {
        out |= rep;
        rep >>= rep_bits;
    }
    out
}

/// A value of `src` bits scaled down to `dst` bits (the top bits, M2-115 §3.4).
pub fn scale_down(value: u32, src: u32, dst: u32) -> u32 {
    if src <= dst {
        value << (dst - src)
    } else {
        value >> (src - dst)
    }
}

/// A member channel's 14-bit pitch bend (8192 the centre) at a bend range of `range`
/// semitones, as a curve's bend: exact (the 14 bits fit the curve's unit).
pub fn bend_of_14(raw: u16, range: u8) -> i64 {
    (i64::from(raw.min(16383)) - 8192) * i64::from(range) * (BEND_UNIT / 8192)
}

/// A curve's bend as a member channel's 14-bit pitch bend at `range` semitones (clamped
/// at the range's ends), rounded to the nearest step.
pub fn bend_to_14(raw: i64, range: u8) -> u16 {
    let range = i64::from(range.max(1));
    let step = range * (BEND_UNIT / 8192);
    let q = if raw >= 0 {
        (raw + step / 2) / step
    } else {
        (raw - step / 2) / step
    };
    (q + 8192).clamp(0, 16383) as u16
}

/// A 7-bit pressure or timbre (CC 74) as a curve's value.
pub fn unit_of_7(v: u8) -> i64 {
    i64::from(scale_up(u32::from(v.min(127)), 7, 32))
}

/// A curve's pressure or timbre as 7 bits.
pub fn unit_to_7(raw: i64) -> u8 {
    scale_down(raw.clamp(0, UNIT_MAX) as u32, 32, 7) as u8
}

/// A curve's bend as MIDI 2.0's per-note pitch bend (32 bits, `0x8000_0000` the centre)
/// at a per-note bend range of `range` semitones.
pub fn bend_to_32(raw: i64, range: u8) -> u32 {
    let range = i128::from(range.max(1));
    let v = 0x8000_0000i128 + div_round(i128::from(raw) << 31, range * i128::from(BEND_UNIT));
    v.clamp(0, i128::from(u32::MAX)) as u32
}

/// MIDI 2.0's per-note pitch bend as a curve's bend.
pub fn bend_of_32(v: u32, range: u8) -> i64 {
    let d = i128::from(v) - 0x8000_0000i128;
    div_round(d * i128::from(range) * i128::from(BEND_UNIT), 1i128 << 31) as i64
}

/// `a / b` rounded to the nearest, halves away from zero (`b > 0`).
fn div_round(a: i128, b: i128) -> i128 {
    if a >= 0 {
        (a + b / 2) / b
    } else {
        (a - b / 2) / b
    }
}

/// A note's sounding pitch as MIDI 2.0's *Pitch 7.25* (registered per-note controller 3):
/// the key in the top 7 bits, the bend in the 25 below. Exact: the curve's unit is half a
/// Pitch 7.25 step.
pub fn pitch_725(key: u8, bend: i64) -> u32 {
    let v = (i64::from(key.min(127)) << 25) + bend * 2;
    v.clamp(0, i64::from(u32::MAX)) as u32
}

/// A Pitch 7.25 heard on a note of key `key`: the bend it means.
pub fn bend_of_725(key: u8, v: u32) -> i64 {
    (i64::from(v) - (i64::from(key.min(127)) << 25)) / 2
}

/// The 32-bit value MIDI 2.0 carries for a curve's pressure or timbre (the same integer).
pub fn unit_to_32(raw: i64) -> u32 {
    raw.clamp(0, UNIT_MAX) as u32
}

/// A MIDI 2.0 per-note message, as Cantus reads and writes it.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Ump {
    /// 16-bit velocity; `pitch` the attribute *Pitch 7.9* when one was given.
    NoteOn {
        group: u8,
        channel: u8,
        key: u8,
        velocity: u16,
        pitch: Option<u16>,
    },
    NoteOff {
        group: u8,
        channel: u8,
        key: u8,
        velocity: u16,
    },
    /// Polyphonic (per-note) pressure, 32 bits.
    PolyPressure {
        group: u8,
        channel: u8,
        key: u8,
        value: u32,
    },
    /// A registered per-note controller (3 = Pitch 7.25, 74 = timbre/brightness, …).
    RegisteredPerNote {
        group: u8,
        channel: u8,
        key: u8,
        index: u8,
        value: u32,
    },
    /// An assignable per-note controller.
    AssignablePerNote {
        group: u8,
        channel: u8,
        key: u8,
        index: u8,
        value: u32,
    },
    /// Per-note pitch bend, 32 bits, `0x8000_0000` the centre.
    PerNoteBend {
        group: u8,
        channel: u8,
        key: u8,
        value: u32,
    },
}

/// The registered per-note controller of MIDI 2.0's *Pitch 7.25*.
pub const RPNC_PITCH_725: u8 = 3;
/// The registered per-note controller MPE's timbre maps to (Sound Controller 5, CC 74).
pub const RPNC_TIMBRE: u8 = 74;
/// The note-on attribute type of *Pitch 7.9*.
const ATTR_PITCH_79: u8 = 3;

impl Ump {
    /// The two words of the packet (message type 4, a MIDI 2.0 channel voice message).
    pub fn encode(self) -> [u32; 2] {
        let w0 = |group: u8, op: u8, channel: u8, b2: u8, b3: u8| {
            (0x4u32 << 28)
                | (u32::from(group & 15) << 24)
                | (u32::from(op & 15) << 20)
                | (u32::from(channel & 15) << 16)
                | (u32::from(b2 & 127) << 8)
                | u32::from(b3)
        };
        match self {
            Ump::NoteOn {
                group,
                channel,
                key,
                velocity,
                pitch,
            } => match pitch {
                Some(p) => [
                    w0(group, 0x9, channel, key, ATTR_PITCH_79),
                    (u32::from(velocity) << 16) | u32::from(p),
                ],
                None => [w0(group, 0x9, channel, key, 0), u32::from(velocity) << 16],
            },
            Ump::NoteOff {
                group,
                channel,
                key,
                velocity,
            } => [w0(group, 0x8, channel, key, 0), u32::from(velocity) << 16],
            Ump::PolyPressure {
                group,
                channel,
                key,
                value,
            } => [w0(group, 0xA, channel, key, 0), value],
            Ump::RegisteredPerNote {
                group,
                channel,
                key,
                index,
                value,
            } => [w0(group, 0x0, channel, key, index), value],
            Ump::AssignablePerNote {
                group,
                channel,
                key,
                index,
                value,
            } => [w0(group, 0x1, channel, key, index), value],
            Ump::PerNoteBend {
                group,
                channel,
                key,
                value,
            } => [w0(group, 0x6, channel, key, 0), value],
        }
    }

    /// A packet read back; `None` for anything that is not one of these messages.
    pub fn decode(w: [u32; 2]) -> Option<Ump> {
        if w[0] >> 28 != 0x4 {
            return None;
        }
        let group = ((w[0] >> 24) & 15) as u8;
        let op = ((w[0] >> 20) & 15) as u8;
        let channel = ((w[0] >> 16) & 15) as u8;
        let key = ((w[0] >> 8) & 127) as u8;
        let b3 = (w[0] & 0xFF) as u8;
        Some(match op {
            0x9 => Ump::NoteOn {
                group,
                channel,
                key,
                velocity: (w[1] >> 16) as u16,
                pitch: (b3 == ATTR_PITCH_79).then_some((w[1] & 0xFFFF) as u16),
            },
            0x8 => Ump::NoteOff {
                group,
                channel,
                key,
                velocity: (w[1] >> 16) as u16,
            },
            0xA => Ump::PolyPressure {
                group,
                channel,
                key,
                value: w[1],
            },
            0x0 => Ump::RegisteredPerNote {
                group,
                channel,
                key,
                index: b3,
                value: w[1],
            },
            0x1 => Ump::AssignablePerNote {
                group,
                channel,
                key,
                index: b3,
                value: w[1],
            },
            0x6 => Ump::PerNoteBend {
                group,
                channel,
                key,
                value: w[1],
            },
            _ => return None,
        })
    }

    /// What a per-note message means for a note's curves: the kind and its stored value
    /// (`bend_range` for a per-note pitch bend). `None` for a note on or off.
    pub fn expression(self, bend_range: u8) -> Option<(ExprKind, i64)> {
        match self {
            Ump::PolyPressure { value, .. } => Some((ExprKind::Pressure, i64::from(value))),
            Ump::PerNoteBend { value, .. } => Some((ExprKind::Bend, bend_of_32(value, bend_range))),
            Ump::RegisteredPerNote {
                key, index, value, ..
            } => match index {
                RPNC_PITCH_725 => Some((ExprKind::Bend, bend_of_725(key, value))),
                RPNC_TIMBRE => Some((ExprKind::Timbre, i64::from(value))),
                _ => None,
            },
            Ump::AssignablePerNote { index, value, .. } if index == RPNC_TIMBRE => {
                Some((ExprKind::Timbre, i64::from(value)))
            }
            _ => None,
        }
    }

    /// The per-note message that carries a curve's value for a note of `key` on `channel`:
    /// the bend as Pitch 7.25 (absolute, no range to agree on), pressure as poly pressure,
    /// timbre as registered per-note controller 74.
    pub fn of_expression(group: u8, channel: u8, key: u8, kind: ExprKind, raw: i64) -> Ump {
        match kind {
            ExprKind::Bend => Ump::RegisteredPerNote {
                group,
                channel,
                key,
                index: RPNC_PITCH_725,
                value: pitch_725(key, raw),
            },
            ExprKind::Pressure => Ump::PolyPressure {
                group,
                channel,
                key,
                value: unit_to_32(raw),
            },
            ExprKind::Timbre => Ump::RegisteredPerNote {
                group,
                channel,
                key,
                index: RPNC_TIMBRE,
                value: unit_to_32(raw),
            },
        }
    }
}

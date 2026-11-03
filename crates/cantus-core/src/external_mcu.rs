//! Mackie Control (MCU) and extender (XT), byte for byte (ADR 0095).
//!
//! The layout is the public Mackie Control protocol (the Logic Control reference every
//! host implements): a strip is a MIDI channel's pitch bend (the motor fader, 14 bits), its
//! buttons are notes (note on 0x7F pressed, 0x00 released; the host lights the LED with the
//! same note: 0x7F on, 0x00 off, 0x01 blinking), its fader touch is a note too (0x68 + the
//! strip, 0x70 for the master), a V-pot turns as controller 0x10 + the strip (bit 6 the
//! direction, bits 0–5 the amount) and its ring is controller 0x30 + the strip; a meter is
//! a channel pressure (strip in the high nibble, 0–12 in the low); the jog wheel is
//! controller 0x3C; the two-digit and ten-digit seven-segment displays are controllers
//! 0x40–0x4B; the scribble strips are SysEx `F0 00 00 66 <14 | 15> 12 <offset> <ASCII> F7`,
//! the top line from offset 0, the bottom from 0x38, seven characters a strip.
//!
//! This file only turns bytes into [`SurfaceEvent`]s and a [`SurfaceView`] into bytes: what
//! a press *means* lives in `external_surface`. `tests/external_surface.rs` holds the
//! literal bytes of every message kind.

use crate::external_surface::*;

pub const MANUFACTURER: [u8; 3] = [0x00, 0x00, 0x66];
pub const ID_MCU: u8 = 0x14;
pub const ID_XT: u8 = 0x15;

/// The notes of the unit's buttons (and of their LEDs).
pub mod note {
    pub const REC: u8 = 0x00; // 0x00–0x07
    pub const SOLO: u8 = 0x08; // 0x08–0x0F
    pub const MUTE: u8 = 0x10; // 0x10–0x17
    pub const SELECT: u8 = 0x18; // 0x18–0x1F
    pub const VPUSH: u8 = 0x20; // 0x20–0x27
    pub const BANK_LEFT: u8 = 0x2E;
    pub const BANK_RIGHT: u8 = 0x2F;
    pub const CHANNEL_LEFT: u8 = 0x30;
    pub const CHANNEL_RIGHT: u8 = 0x31;
    pub const FLIP: u8 = 0x32;
    pub const NAME_VALUE: u8 = 0x34;
    pub const TIME_FORMAT: u8 = 0x35;
    pub const SHIFT: u8 = 0x46;
    pub const OPTION: u8 = 0x47;
    pub const CONTROL: u8 = 0x48;
    pub const ALT: u8 = 0x49;
    pub const SAVE: u8 = 0x50;
    pub const UNDO: u8 = 0x51;
    pub const MARKER: u8 = 0x54;
    pub const CYCLE: u8 = 0x56;
    pub const CLICK: u8 = 0x59;
    pub const REWIND: u8 = 0x5B;
    pub const FORWARD: u8 = 0x5C;
    pub const STOP: u8 = 0x5D;
    pub const PLAY: u8 = 0x5E;
    pub const RECORD: u8 = 0x5F;
    pub const UP: u8 = 0x60;
    pub const DOWN: u8 = 0x61;
    pub const LEFT: u8 = 0x62;
    pub const RIGHT: u8 = 0x63;
    pub const SCRUB: u8 = 0x65;
    /// Fader touch of strip `n` is `TOUCH + n`; the master's is `TOUCH + 8`.
    pub const TOUCH: u8 = 0x68;
    /// The time display's two mode LEDs.
    pub const SMPTE_LED: u8 = 0x71;
    pub const BEATS_LED: u8 = 0x72;
}

/// The button a note is.
pub fn button_of(n: u8) -> Button {
    use note::*;
    let strip = |base: u8| n - base;
    match n {
        0x00..=0x07 => Button::Rec(strip(REC)),
        0x08..=0x0F => Button::Solo(strip(SOLO)),
        0x10..=0x17 => Button::Mute(strip(MUTE)),
        0x18..=0x1F => Button::Select(strip(SELECT)),
        0x20..=0x27 => Button::VPush(strip(VPUSH)),
        BANK_LEFT => Button::BankLeft,
        BANK_RIGHT => Button::BankRight,
        CHANNEL_LEFT => Button::ChannelLeft,
        CHANNEL_RIGHT => Button::ChannelRight,
        FLIP => Button::Flip,
        NAME_VALUE => Button::NameValue,
        TIME_FORMAT => Button::TimeFormat,
        SHIFT => Button::Shift,
        OPTION => Button::Option,
        CONTROL => Button::Control,
        ALT => Button::Alt,
        SAVE => Button::Save,
        UNDO => Button::Undo,
        MARKER => Button::Marker,
        CYCLE => Button::Cycle,
        CLICK => Button::Click,
        REWIND => Button::Rewind,
        FORWARD => Button::Forward,
        STOP => Button::Stop,
        PLAY => Button::Play,
        RECORD => Button::Record,
        LEFT => Button::Left,
        RIGHT => Button::Right,
        SCRUB => Button::Scrub,
        other => Button::Other(other),
    }
}

/// The note of a button, for its LED.
pub fn note_of(b: Button) -> Option<u8> {
    use note::*;
    Some(match b {
        Button::Rec(c) => REC + c,
        Button::Solo(c) => SOLO + c,
        Button::Mute(c) => MUTE + c,
        Button::Select(c) => SELECT + c,
        Button::VPush(c) => VPUSH + c,
        Button::BankLeft => BANK_LEFT,
        Button::BankRight => BANK_RIGHT,
        Button::ChannelLeft => CHANNEL_LEFT,
        Button::ChannelRight => CHANNEL_RIGHT,
        Button::Flip => FLIP,
        Button::NameValue => NAME_VALUE,
        Button::TimeFormat => TIME_FORMAT,
        Button::Shift => SHIFT,
        Button::Option => OPTION,
        Button::Control => CONTROL,
        Button::Alt => ALT,
        Button::Save => SAVE,
        Button::Undo => UNDO,
        Button::Marker => MARKER,
        Button::Cycle => CYCLE,
        Button::Click => CLICK,
        Button::Rewind => REWIND,
        Button::Forward => FORWARD,
        Button::Stop => STOP,
        Button::Play => PLAY,
        Button::Record => RECORD,
        Button::Left => LEFT,
        Button::Right => RIGHT,
        Button::Scrub => SCRUB,
        Button::Other(n) => n,
        Button::ReturnToZero => return None,
    })
}

/// A V-pot or jog value as signed ticks: bit 6 is counter-clockwise, bits 0–5 the amount.
pub fn delta_of(v: u8) -> i8 {
    let amount = (v & 0x3F) as i8;
    if v & 0x40 != 0 {
        -amount
    } else {
        amount
    }
}

/// Signed ticks as the value a unit sends (for the emulation and the tests).
pub fn value_of_delta(d: i8) -> u8 {
    let a = d.unsigned_abs().min(0x3F);
    if d < 0 {
        0x40 | a
    } else {
        a
    }
}

/// What a unit sent, as events.
pub fn decode(bytes: &[u8]) -> Option<SurfaceEvent> {
    let (&status, rest) = bytes.split_first()?;
    let d1 = *rest.first()? & 0x7F;
    let d2 = rest.get(1).copied().unwrap_or(0) & 0x7F;
    match status & 0xF0 {
        0xE0 => {
            let ch = status & 0x0F;
            (usize::from(ch) <= MASTER).then(|| SurfaceEvent::Fader {
                channel: ch,
                pos: from14(u16::from(d2) << 7 | u16::from(d1)),
            })
        }
        0x90 | 0x80 => {
            let down = status & 0xF0 == 0x90 && d2 > 0;
            if (note::TOUCH..=note::TOUCH + MASTER as u8).contains(&d1) {
                return Some(SurfaceEvent::Touch {
                    channel: d1 - note::TOUCH,
                    on: down,
                });
            }
            Some(SurfaceEvent::Button {
                button: button_of(d1),
                down,
            })
        }
        0xB0 => match d1 {
            0x10..=0x17 => Some(SurfaceEvent::VPot {
                channel: d1 - 0x10,
                delta: delta_of(d2),
            }),
            0x3C => Some(SurfaceEvent::Jog {
                delta: delta_of(d2),
            }),
            _ => None,
        },
        _ => None,
    }
}

// ---- host to unit ----------------------------------------------------------------------

pub fn device_id(kind: Kind) -> u8 {
    match kind {
        Kind::McuExtender(_) => ID_XT,
        _ => ID_MCU,
    }
}

fn sysex(kind: Kind, command: u8, body: &[u8]) -> Vec<u8> {
    let mut v = vec![0xF0];
    v.extend_from_slice(&MANUFACTURER);
    v.push(device_id(kind));
    v.push(command);
    v.extend_from_slice(body);
    v.push(0xF7);
    v
}

/// The motor fader of channel `ch` (0–7, 8 the master) to `v` (14 bits).
pub fn fader(ch: u8, v: u16) -> Vec<u8> {
    vec![0xE0 | ch, (v & 0x7F) as u8, (v >> 7) as u8 & 0x7F]
}

/// A button's LED: 0x7F on, 0x00 off, 0x01 blinking.
pub fn led(n: u8, state: u8) -> Vec<u8> {
    vec![0x90, n, state]
}

/// A V-pot's ring: mode 0 a single dot, 1 boost/cut, 2 wrap, 3 spread; position 0–11; the
/// centre LED.
pub fn ring(ch: u8, mode: u8, pos: u8, centre: bool) -> Vec<u8> {
    vec![
        0xB0,
        0x30 + ch,
        (mode & 3) << 4 | (pos & 0x0F) | if centre { 0x40 } else { 0 },
    ]
}

/// A meter's level, 0–12 (0xE sets the overload light, 0xF clears it).
pub fn meter(ch: u8, level: u8) -> Vec<u8> {
    vec![0xD0, (ch & 7) << 4 | (level & 0x0F)]
}

/// Text on the scribble strips from `offset` (0–0x37 the top line, 0x38–0x6F the bottom).
pub fn lcd(kind: Kind, offset: u8, text: &[u8]) -> Vec<u8> {
    let mut body = vec![offset];
    body.extend(text.iter().map(|c| c & 0x7F));
    sysex(kind, 0x12, &body)
}

/// The value of a seven-segment digit: bits 0–5 the character (ASCII 0x20–0x3F as they are,
/// 0x40–0x5F as 0x00–0x1F), bit 6 the dot (TouchMCU, *Mackie Control protocol*, "Bit 6 is the
/// Dot on/off toggle").
pub fn digit_value(c: u8, dot: bool) -> u8 {
    (c & 0x3F) | if dot { 0x40 } else { 0 }
}

/// A digit of the ten-digit timecode display: `i` 0 is the **rightmost** (controller 0x40),
/// 9 the leftmost (0x49) — TouchMCU's table lists them "from right to left", CC 64–73.
pub fn time_digit(i: u8, c: u8, dot: bool) -> Vec<u8> {
    vec![0xB0, 0x40 + i.min(9), digit_value(c, dot)]
}

/// A digit of the two-digit assignment display: `i` 0 the right one (controller 0x4A), 1 the
/// left (0x4B) — TouchMCU, CC 74–75.
pub fn assignment_digit(i: u8, c: u8, dot: bool) -> Vec<u8> {
    vec![0xB0, 0x4A + i.min(1), digit_value(c, dot)]
}

/// The device query a host sends when a unit is connected.
pub fn device_query(kind: Kind) -> Vec<u8> {
    sysex(kind, 0x00, &[])
}

/// The host's answer to a unit's connection query (`01` + 7-byte serial + 4-byte
/// challenge): `02` + the serial + the four response bytes. `None` for anything else.
pub fn connection_reply(kind: Kind, query: &[u8]) -> Option<Vec<u8>> {
    let body = query.strip_prefix(&[0xF0, 0x00, 0x00, 0x66, device_id(kind), 0x01])?;
    let body = body.strip_suffix(&[0xF7])?;
    if body.len() != 11 {
        return None;
    }
    let (serial, c) = body.split_at(7);
    let (c0, c1, c2, c3) = (c[0] as i32, c[1] as i32, c[2] as i32, c[3] as i32);
    let r = [
        (c0 + (c1 ^ 0x0A) - c3) & 0x7F,
        ((c2 >> 4) ^ (c0 + c3)) & 0x7F,
        ((c3 - (c2 << 2)) ^ (c0 | c1)) & 0x7F,
        (c1 - c2 + (0xF0 ^ (c3 << 4))) & 0x7F,
    ];
    let mut reply = serial.to_vec();
    reply.extend(r.iter().map(|x| *x as u8));
    Some(sysex(kind, 0x02, &reply))
}

/// The ten time digits (rightmost first) of `text`, each with whether its dot is lit: digits
/// and letters count, a `.` or `:` lights the dot of the digit before it.
pub fn time_digits(text: &str) -> [(u8, bool); 10] {
    let mut out = [(b' ', false); 10];
    let mut chars: Vec<(u8, bool)> = Vec::new();
    for c in text.bytes() {
        if c == b'.' || c == b':' {
            if let Some(last) = chars.last_mut() {
                last.1 = true;
            }
        } else {
            chars.push((c, false));
        }
    }
    for (i, d) in chars.iter().rev().take(10).enumerate() {
        out[i] = *d;
    }
    out
}

/// What to send a unit now: only what changed since the last render (everything after
/// [`Device::refresh`]).
pub fn render(set: &mut SurfaceSet, dev: usize, view: &SurfaceView) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    let kind = set.devices[dev].kind;
    let main = kind.is_main();
    if !set.devices[dev].sent.online {
        out.push(device_query(kind));
        set.devices[dev].sent.online = true;
    }
    let (flip, show_pan, bank) = (set.flip, set.show_pan, set.bank);
    let mut names = Vec::with_capacity(56);
    let mut values = Vec::with_capacity(56);
    // The channels first (they borrow the set), then the device's cache.
    struct Ch {
        fader: Option<u16>,
        ring: Option<(u8, bool)>,
        leds: [u8; 4],
        meter: u8,
    }
    let mut chs: Vec<Ch> = Vec::new();
    for c in 0..STRIPS {
        let s = bound(set, dev, c, view);
        let strip_name = s.map_or("      ".to_string(), |s| name_text(&s.name));
        let strip_value = s.map_or("      ".to_string(), |s| {
            if show_pan {
                pan_text(s.pan)
            } else {
                gain_text(s.gain_db)
            }
        });
        names.extend_from_slice(strip_name.as_bytes());
        names.push(b' ');
        values.extend_from_slice(strip_value.as_bytes());
        values.push(b' ');
        chs.push(Ch {
            fader: s.map(|s| {
                if flip {
                    to14((s.pan + 1.0) / 2.0)
                } else {
                    fader_value(s.gain_db)
                }
            }),
            ring: s.map(|s| {
                if flip {
                    (1 + (fader_pos_of_db(s.gain_db) * 10.0).round() as u8, false)
                } else {
                    (ring_of(s.pan), s.pan == 0.0)
                }
            }),
            leds: s.map_or([0; 4], |s| {
                [on(s.arm), on(s.solo), on(s.mute), on(s.selected)]
            }),
            meter: s.map_or(0, |s| meter_step(s.meter)),
        });
    }
    let master = main.then(|| bound(set, dev, MASTER, view)).flatten();
    let now = set.now_ms;
    let d = &mut set.devices[dev];
    let refresh_meters = now >= d.last_meter_ms + METER_REFRESH_MS;
    if refresh_meters {
        d.last_meter_ms = now;
    }
    for (c, ch) in chs.iter().enumerate() {
        let want = ch.fader;
        if let Some(v) = want {
            let echo = d.from_unit[c].take();
            if d.motor_may_move(c, now) && d.sent.faders[c] != Some(v) {
                if echo.is_none_or(|e| e.abs_diff(v) > 1) {
                    out.push(self::fader(c as u8, v));
                }
                d.sent.faders[c] = Some(v);
            }
        }
        let r = ch.ring;
        let key = r.map(|(p, centre)| p | if centre { 0x80 } else { 0 });
        if d.sent.rings[c] != Some(key.unwrap_or(0xFF)) {
            match r {
                Some((p, centre)) => out.push(self::ring(c as u8, 0, p, centre)),
                None => out.push(self::ring(c as u8, 0, 0, false)),
            }
            d.sent.rings[c] = Some(key.unwrap_or(0xFF));
        }
        for (k, base) in [note::REC, note::SOLO, note::MUTE, note::SELECT]
            .iter()
            .enumerate()
        {
            let n = base + c as u8;
            if d.sent.leds.get(&u16::from(n)) != Some(&ch.leds[k]) {
                out.push(led(n, ch.leds[k]));
                d.sent.leds.insert(u16::from(n), ch.leds[k]);
            }
        }
        // A meter decays by itself: a lit one is sent again every METER_REFRESH_MS.
        if refresh_meters && ch.meter > 0 {
            d.sent.meters[c] = None;
        }
        if d.sent.meters[c] != Some(ch.meter) {
            out.push(meter(c as u8, ch.meter));
            d.sent.meters[c] = Some(ch.meter);
        }
    }
    if let Some(m) = master {
        let v = fader_value(m.gain_db);
        let echo = d.from_unit[MASTER].take();
        if d.motor_may_move(MASTER, now) && d.sent.faders[MASTER] != Some(v) {
            if echo.is_none_or(|e| e.abs_diff(v) > 1) {
                out.push(self::fader(MASTER as u8, v));
            }
            d.sent.faders[MASTER] = Some(v);
        }
    }
    for (i, line) in [(0u8, names), (0x38, values)].into_iter().enumerate() {
        if d.sent.lcd[i] != line.1 {
            out.push(lcd(kind, line.0, &line.1));
            d.sent.lcd[i] = line.1;
        }
    }
    if main {
        let t = &view.transport;
        let mut set_led = |n: u8, v: u8, out: &mut Vec<Vec<u8>>| {
            if d.sent.leds.get(&u16::from(n)) != Some(&v) {
                out.push(led(n, v));
                d.sent.leds.insert(u16::from(n), v);
            }
        };
        set_led(note::PLAY, on(t.playing), &mut out);
        set_led(note::STOP, on(!t.playing), &mut out);
        set_led(note::RECORD, on(t.recording), &mut out);
        set_led(note::CYCLE, on(t.looping), &mut out);
        set_led(note::CLICK, on(t.click), &mut out);
        set_led(note::SCRUB, on(t.scrubbing), &mut out);
        set_led(note::FLIP, on(flip), &mut out);
        set_led(note::NAME_VALUE, on(show_pan), &mut out);
        set_led(note::BEATS_LED, on(t.beats), &mut out);
        set_led(note::SMPTE_LED, on(!t.beats), &mut out);
        // The two-digit assignment display shows the bank (strips shown / 8 + 1) and the
        // ten-digit one the position.
        // Cache slots 0–9 the timecode digits (0 rightmost), 10–11 the assignment's.
        let page = format!("{:02}", (bank / STRIPS + 1).min(99));
        let mut codes: [(u8, bool); 12] = [(b' ', false); 12];
        for (i, dgt) in time_digits(&t.time).iter().enumerate() {
            codes[i] = *dgt;
        }
        codes[10] = (page.as_bytes()[1], false);
        codes[11] = (page.as_bytes()[0], false);
        for (i, (c, dot)) in codes.iter().enumerate() {
            let code = digit_value(*c, *dot);
            if d.sent.time[i] != Some(code) {
                out.push(if i < 10 {
                    time_digit(i as u8, *c, *dot)
                } else {
                    assignment_digit((i - 10) as u8, *c, *dot)
                });
                d.sent.time[i] = Some(code);
            }
        }
    }
    out
}

fn on(b: bool) -> u8 {
    if b {
        0x7F
    } else {
        0x00
    }
}

fn fader_pos_of_db(db: f32) -> f32 {
    crate::automation::range::fader_pos_of(db)
}

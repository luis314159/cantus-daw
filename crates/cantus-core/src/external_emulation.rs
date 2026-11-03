//! Virtual Mackie Control and HUI units (ADR 0095): the device side of both protocols, so
//! every surface feature is tested, and demonstrated, with no hardware.
//!
//! A [`VirtualMcu`] or [`VirtualHui`] behaves as the physical unit does at its MIDI port:
//! it **receives** the host's bytes and keeps what a unit would show (motor faders, LEDs,
//! rings, meters, scribble strips, displays), and it **sends** the bytes a hand would make
//! (a press, a fader move and its touch, a V-pot turn, the jog wheel). It decodes the
//! host's messages by the protocol's byte layout directly, not through the host's own codec
//! (`external_mcu`, `external_hui`), so the two are written against the same public layout
//! and meet only in the bytes. A unit's motors do not move a fader that is being touched.
//!
//! The same units serve as the *virtual device pair* of the settings page (a surface to
//! try the setup with when none is plugged in): the interface feeds the host's output to
//! [`VirtualMcu::receive`] and the unit's reply bytes to the host's input.

/// The serial the emulated MCU gives in its connection query.
pub const SERIAL: [u8; 7] = *b"CANTUS1";
/// The challenge the emulated MCU gives.
pub const CHALLENGE: [u8; 4] = [0x11, 0x22, 0x33, 0x44];

/// A virtual Mackie Control (or extender) unit.
#[derive(Clone, Debug)]
pub struct VirtualMcu {
    pub extender: bool,
    /// The motor faders (14 bits), channels 0–7 and the master as 8.
    pub faders: [u16; 9],
    pub touched: [bool; 9],
    /// LEDs by note: 0x7F on, 0 off, 1 blinking.
    pub leds: [u8; 128],
    /// V-pot rings (controller 0x30 + ch value).
    pub rings: [u8; 8],
    /// Meters: the last level (0–12) of each strip, 0xE/0xF overload on/off.
    pub meters: [u8; 8],
    /// The scribble strips: 2 × 56 characters.
    pub lcd: [u8; 112],
    /// The seven-segment codes, controllers 0x40–0x4B.
    pub digits: [u8; 12],
    /// The unit has been answered (a connection reply with the right response, or a host
    /// that talked without the handshake: `talked`).
    pub verified: bool,
    pub talked: bool,
}

impl VirtualMcu {
    pub fn new(extender: bool) -> VirtualMcu {
        VirtualMcu {
            extender,
            faders: [0; 9],
            touched: [false; 9],
            leds: [0; 128],
            rings: [0; 8],
            meters: [0; 8],
            lcd: [b' '; 112],
            digits: [0x20; 12],
            verified: false,
            talked: false,
        }
    }

    fn id(&self) -> u8 {
        if self.extender {
            0x15
        } else {
            0x14
        }
    }

    /// The host's bytes; the bytes the unit answers with, if any.
    pub fn receive(&mut self, b: &[u8]) -> Vec<Vec<u8>> {
        let mut reply = Vec::new();
        let Some(&status) = b.first() else {
            return reply;
        };
        self.talked = true;
        let d1 = b.get(1).copied().unwrap_or(0);
        let d2 = b.get(2).copied().unwrap_or(0);
        match status {
            0xF0 => {
                // F0 00 00 66 <id> <command> ... F7
                if b.len() >= 6 && b[1..4] == [0x00, 0x00, 0x66] && b[4] == self.id() {
                    match b[5] {
                        0x00 => {
                            let mut q = vec![0xF0, 0x00, 0x00, 0x66, self.id(), 0x01];
                            q.extend_from_slice(&SERIAL);
                            q.extend_from_slice(&CHALLENGE);
                            q.push(0xF7);
                            reply.push(q);
                        }
                        0x02 => {
                            // The response the unit expects to its own challenge.
                            let c = CHALLENGE.map(i32::from);
                            let want = [
                                (c[0] + (c[1] ^ 0x0A) - c[3]) & 0x7F,
                                ((c[2] >> 4) ^ (c[0] + c[3])) & 0x7F,
                                ((c[3] - (c[2] << 2)) ^ (c[0] | c[1])) & 0x7F,
                                (c[1] - c[2] + (0xF0 ^ (c[3] << 4))) & 0x7F,
                            ];
                            let got = &b[6 + 7..b.len() - 1];
                            self.verified = got.iter().map(|x| i32::from(*x)).eq(want);
                        }
                        0x12 if b.len() > 7 => {
                            let off = usize::from(b[6]);
                            for (i, c) in b[7..b.len() - 1].iter().enumerate() {
                                if let Some(slot) = self.lcd.get_mut(off + i) {
                                    *slot = *c;
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            s if s & 0xF0 == 0xE0 => {
                let ch = usize::from(s & 0x0F);
                if ch <= 8 && !self.touched[ch] {
                    self.faders[ch] = u16::from(d2 & 0x7F) << 7 | u16::from(d1 & 0x7F);
                }
            }
            s if s & 0xF0 == 0x90 => {
                if let Some(l) = self.leds.get_mut(usize::from(d1 & 0x7F)) {
                    *l = d2;
                }
            }
            s if s & 0xF0 == 0xB0 => match d1 {
                0x30..=0x37 => self.rings[usize::from(d1 - 0x30)] = d2,
                0x40..=0x4B => self.digits[usize::from(d1 - 0x40)] = d2,
                _ => {}
            },
            s if s & 0xF0 == 0xD0 => self.meters[usize::from(d1 >> 4) & 7] = d1 & 0x0F,
            _ => {}
        }
        reply
    }

    /// The text of a scribble line (0 top, 1 bottom) of channel `ch`.
    pub fn scribble(&self, line: usize, ch: usize) -> String {
        let at = line * 56 + ch * 7;
        String::from_utf8_lossy(&self.lcd[at..at + 7]).into_owned()
    }

    /// The time display as text (digits and dots, left to right).
    pub fn time_text(&self) -> String {
        let mut s = String::new();
        // Controllers 0x40 (rightmost) … 0x49 (leftmost): the timecode.
        for code in self.digits[0..10].iter().rev() {
            s.push(seven(*code));
            if code & 0x40 != 0 {
                s.push('.');
            }
        }
        s
    }

    /// The assignment display's two digits.
    pub fn assignment(&self) -> String {
        // 0x4B the left digit, 0x4A the right.
        [self.digits[11], self.digits[10]]
            .iter()
            .map(|c| seven(*c))
            .collect()
    }

    // ---- what a hand does ----------------------------------------------------------

    /// A button pressed (and, with `release`, let go): the notes it sends.
    pub fn press(&self, note: u8, release: bool) -> Vec<Vec<u8>> {
        let mut v = vec![vec![0x90, note, 0x7F]];
        if release {
            v.push(vec![0x90, note, 0x00]);
        }
        v
    }

    /// A fader touched, moved to `v` (14 bits) and let go: touch, the bend, release. The
    /// unit's own motor position follows its fader.
    pub fn move_fader(&mut self, ch: usize, v: u16, release: bool) -> Vec<Vec<u8>> {
        self.touched[ch] = true;
        self.faders[ch] = v;
        let mut out = vec![
            vec![0x90, 0x68 + ch as u8, 0x7F],
            vec![0xE0 | ch as u8, (v & 0x7F) as u8, (v >> 7) as u8],
        ];
        if release {
            self.touched[ch] = false;
            out.push(vec![0x90, 0x68 + ch as u8, 0x00]);
        }
        out
    }

    /// A touched fader let go: the note off (and the motor may move it again).
    pub fn release(&mut self, ch: usize) -> Vec<Vec<u8>> {
        self.touched[ch] = false;
        vec![vec![0x90, 0x68 + ch as u8, 0x00]]
    }

    /// A V-pot turned by `delta` ticks.
    pub fn turn(&self, ch: u8, delta: i8) -> Vec<u8> {
        vec![0xB0, 0x10 + ch, v_of(delta)]
    }

    /// The jog wheel turned by `delta` ticks.
    pub fn jog(&self, delta: i8) -> Vec<u8> {
        vec![0xB0, 0x3C, v_of(delta)]
    }
}

/// A seven-segment code as the character it shows: 0x20–0x3F are ASCII as they are, 0x00–0x1F
/// are the letters 0x40–0x5F.
fn seven(code: u8) -> char {
    let c = code & 0x3F;
    (if c < 0x20 { c + 0x40 } else { c }) as char
}

/// HUI's relative value: clockwise is 0x40 + the amount, counter-clockwise the amount alone
/// (theageman's notes, p. 10: "If vv>40 then delta=vv-40. If vv<40 then delta=-vv").
fn hui_v(d: i8) -> u8 {
    let a = d.unsigned_abs().min(0x3F);
    if d > 0 {
        0x40 + a
    } else {
        a
    }
}

/// Mackie's relative value: bit 6 the direction (set: counter-clockwise), bits 0–5 the amount.
fn v_of(d: i8) -> u8 {
    let a = d.unsigned_abs().min(0x3F);
    if d < 0 {
        0x40 | a
    } else {
        a
    }
}

/// A virtual HUI.
#[derive(Clone, Debug)]
pub struct VirtualHui {
    pub faders: [u16; 8],
    pub touched: [bool; 8],
    /// LEDs by (zone, port).
    pub leds: std::collections::BTreeMap<(u8, u8), bool>,
    pub rings: [u8; 8],
    /// The four-character names of the strips.
    pub names: [String; 8],
    /// A ping answered (the host's `90 00 00` seen).
    pub pinged: bool,
    zone: u8,
    hi: [u8; 8],
}

impl Default for VirtualHui {
    fn default() -> Self {
        Self::new()
    }
}

impl VirtualHui {
    pub fn new() -> VirtualHui {
        VirtualHui {
            faders: [0; 8],
            touched: [false; 8],
            leds: Default::default(),
            rings: [0; 8],
            names: Default::default(),
            pinged: false,
            zone: 0,
            hi: [0; 8],
        }
    }

    /// The host's bytes; the unit's answer, if any (a ping is answered).
    pub fn receive(&mut self, b: &[u8]) -> Vec<Vec<u8>> {
        let mut reply = Vec::new();
        let status = b.first().copied().unwrap_or(0);
        let d1 = b.get(1).copied().unwrap_or(0);
        let d2 = b.get(2).copied().unwrap_or(0);
        match status & 0xF0 {
            0x90 if d1 == 0 && d2 == 0 => {
                self.pinged = true;
                reply.push(vec![0x90, 0x00, 0x7F]);
            }
            0xB0 => match d1 {
                0x0C => self.zone = d2,
                0x2C => {
                    self.leds.insert((self.zone, d2 & 7), d2 & 0x40 != 0);
                }
                0x00..=0x07 => self.hi[usize::from(d1)] = d2,
                0x20..=0x27 => {
                    let ch = usize::from(d1 - 0x20);
                    if !self.touched[ch] {
                        self.faders[ch] = u16::from(self.hi[ch]) << 7 | u16::from(d2);
                    }
                }
                0x10..=0x17 => self.rings[usize::from(d1 - 0x10)] = d2,
                _ => {}
            },
            0xF0 if b.len() == 13 && b[1..7] == [0x00, 0x00, 0x66, 0x05, 0x00, 0x10] => {
                let ch = usize::from(b[7]) & 7;
                self.names[ch] = String::from_utf8_lossy(&b[8..12]).into_owned();
            }
            _ => {}
        }
        reply
    }

    /// Is the (zone, port) LED on?
    pub fn led(&self, zone: u8, port: u8) -> bool {
        self.leds.get(&(zone, port)).copied().unwrap_or(false)
    }

    /// A switch pressed (and let go): zone select then port.
    pub fn press(&self, zone: u8, port: u8, release: bool) -> Vec<Vec<u8>> {
        let mut v = vec![vec![0xB0, 0x0F, zone], vec![0xB0, 0x2F, port | 0x40]];
        if release {
            v.push(vec![0xB0, 0x0F, zone]);
            v.push(vec![0xB0, 0x2F, port]);
        }
        v
    }

    /// A fader touched, moved to `v` (14 bits) and let go.
    pub fn move_fader(&mut self, ch: usize, v: u16, release: bool) -> Vec<Vec<u8>> {
        self.touched[ch] = true;
        self.faders[ch] = v;
        let mut out = self.press(ch as u8, 0, false);
        out.push(vec![0xB0, ch as u8, (v >> 7) as u8 & 0x7F]);
        out.push(vec![0xB0, 0x20 + ch as u8, (v & 0x7F) as u8]);
        if release {
            self.touched[ch] = false;
            out.push(vec![0xB0, 0x0F, ch as u8]);
            out.push(vec![0xB0, 0x2F, 0]);
        }
        out
    }

    /// A touched fader let go.
    pub fn release(&mut self, ch: usize) -> Vec<Vec<u8>> {
        self.touched[ch] = false;
        vec![vec![0xB0, 0x0F, ch as u8], vec![0xB0, 0x2F, 0]]
    }

    pub fn turn(&self, ch: u8, delta: i8) -> Vec<u8> {
        vec![0xB0, 0x40 + ch, hui_v(delta)]
    }

    pub fn jog(&self, delta: i8) -> Vec<u8> {
        vec![0xB0, 0x0D, hui_v(delta)]
    }
}

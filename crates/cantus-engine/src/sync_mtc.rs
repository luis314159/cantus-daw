//! MIDI time code (ADR 0079 §3): the four rates, drop-frame, the quarter-frame and
//! full-frame messages, and chasing another device's code.
//!
//! From the MIDI 1.0 *MIDI Time Code* recommended practice (RP-004/RP-008):
//!
//! - **Quarter frames** `F1 0nnn dddd`: eight pieces — frames low/high nibble, seconds,
//!   minutes, hours, the last carrying the rate in bits 5–6 (`0rrh`: 0 = 24, 1 = 25,
//!   2 = 29.97 drop-frame, 3 = 30) — one every quarter of a frame, so a whole time takes
//!   two frames. The time they carry is the frame at which piece 0 was sent; a reader who
//!   has piece 7 is 1¾ frames past it.
//! - **Full frame** `F0 7F 7F 01 01 hh mm ss ff F7` (`hh` with the rate bits): a locate.
//! - **Drop-frame** (29.97): frame numbers 0 and 1 are skipped at the start of every minute
//!   except each tenth, so the code keeps to the clock (17 982 frames in ten minutes).

use super::clock::Dll;
use super::{SourceKind, Timeline};

/// The four rates.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum Rate {
    Fps24,
    #[default]
    Fps25,
    /// 29.97 frames a second, drop-frame.
    Df2997,
    Fps30,
}

impl Rate {
    pub const ALL: [Rate; 4] = [Rate::Fps24, Rate::Fps25, Rate::Df2997, Rate::Fps30];

    /// The rate's two bits.
    pub fn code(self) -> u8 {
        match self {
            Rate::Fps24 => 0,
            Rate::Fps25 => 1,
            Rate::Df2997 => 2,
            Rate::Fps30 => 3,
        }
    }
    pub fn from_code(c: u32) -> Rate {
        match c & 3 {
            0 => Rate::Fps24,
            1 => Rate::Fps25,
            2 => Rate::Df2997,
            _ => Rate::Fps30,
        }
    }
    /// Frames counted per second of code (30 for drop-frame: its labels go to 29).
    pub fn nominal(self) -> i64 {
        match self {
            Rate::Fps24 => 24,
            Rate::Fps25 => 25,
            Rate::Df2997 | Rate::Fps30 => 30,
        }
    }
    /// Frames per second of real time.
    pub fn fps(self) -> f64 {
        match self {
            Rate::Fps24 => 24.0,
            Rate::Fps25 => 25.0,
            Rate::Df2997 => 30_000.0 / 1001.0,
            Rate::Fps30 => 30.0,
        }
    }
    /// Its label: 24, 25, 29.97 df, 30.
    pub fn label(self) -> &'static str {
        match self {
            Rate::Fps24 => "24",
            Rate::Fps25 => "25",
            Rate::Df2997 => "29.97 df",
            Rate::Fps30 => "30",
        }
    }
}

/// A time code: hours, minutes, seconds, frames.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Timecode {
    pub h: u8,
    pub m: u8,
    pub s: u8,
    pub f: u8,
    pub rate: Rate,
}

/// Frames in ten minutes and in a minute, drop-frame.
const DF_TEN: i64 = 17_982;
const DF_MIN: i64 = 1_798;

impl Timecode {
    /// The code of frame number `n` (counted from 00:00:00:00) at `rate`, wrapping at 24 h.
    pub fn from_frames(n: i64, rate: Rate) -> Timecode {
        let day = 24 * 3600 * rate.fps().round() as i64;
        let day = if rate == Rate::Df2997 {
            144 * DF_TEN
        } else {
            day
        };
        let mut n = n.rem_euclid(day);
        if rate == Rate::Df2997 {
            let tens = n / DF_TEN;
            let m = n % DF_TEN;
            n += 18 * tens + if m >= 2 { 2 * ((m - 2) / DF_MIN) } else { 0 };
        }
        let fps = rate.nominal();
        Timecode {
            f: (n % fps) as u8,
            s: ((n / fps) % 60) as u8,
            m: ((n / (fps * 60)) % 60) as u8,
            h: (n / (fps * 3600)) as u8,
            rate,
        }
    }

    /// The frame number of this code.
    pub fn frames(&self) -> i64 {
        let fps = self.rate.nominal();
        let mins = 60 * i64::from(self.h) + i64::from(self.m);
        let n = (mins * 60 + i64::from(self.s)) * fps + i64::from(self.f);
        if self.rate == Rate::Df2997 {
            n - 2 * (mins - mins / 10)
        } else {
            n
        }
    }

    /// The code at `seconds` of real time (the frame in progress).
    pub fn at_seconds(seconds: f64, rate: Rate) -> Timecode {
        Timecode::from_frames((seconds * rate.fps() + 1e-9).floor() as i64, rate)
    }

    /// Its start, in seconds of real time.
    pub fn seconds(&self) -> f64 {
        self.frames() as f64 / self.rate.fps()
    }

    /// Quarter-frame piece `p` (0–7).
    pub fn quarter_frame(&self, p: u8) -> [u8; 2] {
        let d = match p & 7 {
            0 => self.f & 0x0F,
            1 => self.f >> 4 & 0x01,
            2 => self.s & 0x0F,
            3 => self.s >> 4 & 0x03,
            4 => self.m & 0x0F,
            5 => self.m >> 4 & 0x03,
            6 => self.h & 0x0F,
            _ => (self.h >> 4 & 0x01) | self.rate.code() << 1,
        };
        [0xF1, (p & 7) << 4 | d]
    }

    /// The full-frame message (a locate).
    pub fn full_frame(&self) -> [u8; 10] {
        [
            0xF0,
            0x7F,
            0x7F,
            0x01,
            0x01,
            self.rate.code() << 5 | (self.h & 0x1F),
            self.m,
            self.s,
            self.f,
            0xF7,
        ]
    }

    /// Read a full-frame message.
    pub fn from_full_frame(b: &[u8]) -> Option<Timecode> {
        if b.len() < 10 || b[..5] != [0xF0, 0x7F, 0x7F, 0x01, 0x01] || b[9] != 0xF7 {
            return None;
        }
        Some(Timecode {
            h: b[5] & 0x1F,
            m: b[6] & 0x3F,
            s: b[7] & 0x3F,
            f: b[8] & 0x1F,
            rate: Rate::from_code(u32::from(b[5] >> 5)),
        })
    }

    /// `hh:mm:ss:ff` (`;` before the frames for drop-frame).
    pub fn label(&self) -> String {
        let sep = if self.rate == Rate::Df2997 { ';' } else { ':' };
        format!(
            "{:02}:{:02}:{:02}{sep}{:02}",
            self.h, self.m, self.s, self.f
        )
    }
}

/// Collects quarter frames into whole codes.
#[derive(Copy, Clone, Debug, Default)]
pub struct QfReader {
    nibbles: [u8; 8],
    /// Which pieces came, in order since piece 0.
    next: u8,
}

impl QfReader {
    /// A quarter frame's data byte: the code its sequence carries once piece 7 completes
    /// an unbroken run 0–7.
    pub fn feed(&mut self, data: u8) -> Option<Timecode> {
        let p = data >> 4 & 7;
        if p == 0 {
            self.next = 0;
        }
        if p != self.next {
            self.next = 8; // broken: wait for the next piece 0
            return None;
        }
        self.nibbles[usize::from(p)] = data & 0x0F;
        self.next += 1;
        if p != 7 {
            return None;
        }
        let n = &self.nibbles;
        Some(Timecode {
            f: n[0] | (n[1] & 1) << 4,
            s: n[2] | (n[3] & 3) << 4,
            m: n[4] | (n[5] & 3) << 4,
            h: n[6] | (n[7] & 1) << 4,
            rate: Rate::from_code(u32::from(n[7] >> 1)),
        })
    }

    /// The piece expected next (8 when waiting for a piece 0).
    pub fn expects(&self) -> u8 {
        self.next
    }
}

/// How long the code may fall silent before chasing stops, µs (it freewheels until then).
pub const FREEWHEEL_US: i64 = 250_000;
/// A whole code this far (frames) from the count is the master locating: a jump.
pub const RELOCATE_FRAMES: f64 = 2.0;
/// A whole code past this (frames) puts the count back on it, without a jump.
pub const RESYNC_FRAMES: f64 = 0.125;
/// The chaser's loop bandwidth, Hz.
pub const CHASE_HZ: f64 = 1.0;

/// Chasing a master's time code.
#[derive(Clone, Debug, Default)]
pub struct MtcChaser {
    reader: QfReader,
    dll: Option<Dll>,
    rate: Rate,
    /// The song time (seconds of code) of the newest quarter frame, and its filtered time.
    seconds: f64,
    at: f64,
    /// Whether a whole code has been read since the last locate (positions are known).
    known: bool,
    last: Option<i64>,
    /// The last piece heard (0–7), and how many quarter frames in a row came far from
    /// their predicted moment.
    last_piece: Option<u8>,
    outliers: u8,
    epoch: u32,
}

/// A quarter frame further than this share of a quarter from its predicted moment does not
/// move the loop (a burst, a late delivery); four in a row restart it (the master's speed
/// changed).
const OUTLIER: f64 = 0.25;
const OUTLIERS: u8 = 4;

impl MtcChaser {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn rate(&self) -> Rate {
        self.rate
    }

    /// A message from the master's port at host time `host`.
    pub fn feed(&mut self, host: i64, bytes: &[u8]) {
        match bytes {
            [0xF1, data, ..] => self.quarter(host, *data),
            [0xF0, ..] => {
                if let Some(tc) = Timecode::from_full_frame(bytes) {
                    self.rate = tc.rate;
                    self.seconds = tc.seconds();
                    self.at = host as f64;
                    self.known = false;
                    self.dll = None;
                    self.last = None;
                    self.last_piece = None;
                    self.epoch = self.epoch.wrapping_add(1);
                }
            }
            _ => {}
        }
    }

    fn quarter(&mut self, host: i64, data: u8) {
        let quarter_us = 1e6 / (4.0 * self.rate.fps());
        let t = host as f64;
        if self.last.is_some_and(|l| host - l > FREEWHEEL_US) {
            self.dll = None;
            self.known = false;
        }
        // Quarters since the last one heard, by piece number (round 2, D1): the pieces
        // count 0–7 whatever the delivery's jitter or bunching; elapsed time only tells
        // apart whole runs of eight lost (a dropout).
        let piece = data >> 4 & 7;
        let n = match (self.last, self.last_piece) {
            (Some(l), Some(lp)) => {
                let d = i64::from((piece + 8 - lp) % 8);
                let by_time = (host - l) as f64 / quarter_us;
                let runs = ((by_time - d as f64) / 8.0).round().max(0.0) as i64;
                let n = d + 8 * runs;
                if n == 0 {
                    8
                } else {
                    n
                }
            }
            _ => 1,
        };
        self.last_piece = Some(piece);
        let filtered = match self.dll.as_mut() {
            Some(d) => {
                // The loop expects the next quarter: past the ones lost, then this one. A
                // quarter frame far from that moment is believed for its count only.
                let pred = d.next + (n - 1) as f64 * d.period;
                if (t - pred).abs() > OUTLIER * quarter_us {
                    self.outliers += 1;
                    d.next = pred + d.period;
                    if self.outliers >= OUTLIERS {
                        self.outliers = 0;
                        self.dll = Some(Dll::new(t, quarter_us));
                        t
                    } else {
                        pred
                    }
                } else {
                    self.outliers = 0;
                    d.next = pred;
                    d.feed(t, CHASE_HZ)
                }
            }
            None => {
                self.dll = Some(Dll::new(t, quarter_us));
                t
            }
        };
        self.last = Some(host);
        if self.known {
            self.seconds += n as f64 / (4.0 * self.rate.fps());
            self.at = filtered;
        }
        if let Some(tc) = self.reader.feed(data) {
            self.rate = tc.rate;
            // Piece 7 is 1¾ frames after the code's frame.
            let now = tc.seconds() + 1.75 / tc.rate.fps();
            let off = now - self.seconds;
            if !self.known || off.abs() > RELOCATE_FRAMES / tc.rate.fps() {
                // Not known yet, or the master located: a jump.
                if self.known {
                    self.epoch = self.epoch.wrapping_add(1);
                }
                self.seconds = now;
                self.at = filtered;
                self.known = true;
                if let Some(d) = self.dll.as_mut() {
                    d.period = 1e6 / (4.0 * tc.rate.fps());
                }
            } else if off.abs() > RESYNC_FRAMES / tc.rate.fps() {
                // A count gone astray (a quarter miscounted through a dropout): back on the
                // code's own place, no jump.
                self.seconds = now;
            }
        }
    }

    /// Whether the code runs, as of `now`.
    pub fn running(&self, now: i64) -> bool {
        self.known && self.last.is_some_and(|l| now - l <= FREEWHEEL_US)
    }

    /// The timeline to publish at `now`: song seconds of code (the offset not taken off).
    pub fn timeline(&self, now: i64) -> Timeline {
        let running = self.running(now);
        let quarter_us = 1e6 / (4.0 * self.rate.fps());
        let rate = self
            .dll
            .filter(|d| d.events() > 8)
            .map_or(1.0, |d| (quarter_us / d.period).clamp(0.9, 1.1));
        Timeline {
            kind: SourceKind::Mtc,
            tempo: 0.0,
            host: if running { self.at.round() as i64 } else { now },
            position: self.seconds,
            rate,
            playing: running,
            quantum: 0.0,
            start_stop: true,
            epoch: self.epoch,
        }
    }
}

/// The time-code generator's place: the next quarter frame, and the frame its run of
/// eight began on.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct QfWriter {
    /// The frame (number at the rate) the current run of eight carries.
    pub frame: i64,
    /// The next piece, 0–7.
    pub piece: u8,
}

impl QfWriter {
    /// A run starting at the first frame at or after `seconds` of code.
    pub fn at(seconds: f64, rate: Rate) -> QfWriter {
        QfWriter {
            frame: (seconds * rate.fps() - 1e-9).ceil() as i64,
            piece: 0,
        }
    }

    /// The code seconds the next piece is due at.
    pub fn due(&self, rate: Rate) -> f64 {
        (self.frame as f64 + f64::from(self.piece) / 4.0) / rate.fps()
    }

    /// The next piece's message, and the writer moved past it.
    pub fn take(&mut self, rate: Rate) -> [u8; 2] {
        let msg = Timecode::from_frames(self.frame, rate).quarter_frame(self.piece);
        self.piece += 1;
        if self.piece == 8 {
            self.piece = 0;
            self.frame += 2;
        }
        msg
    }
}

//! The Standard MIDI File, byte for byte: chunks, variable-length quantities, running
//! status. No musical knowledge lives here — [`super::export`] and [`super::import`] turn
//! a project into these events and back.
//!
//! Written by hand rather than taken from a crate: the format is small and fixed (the
//! 1996 MMA specification), and owning the parser is what lets it be lenient where real
//! files are sloppy (a missing end-of-track, running status across a meta event) and
//! strict where sloppiness would be a lie (a chunk longer than the file).

use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MidiError {
    #[error("not a MIDI file")]
    NotMidi,
    #[error("the file ends early (byte {0})")]
    Truncated(usize),
    #[error("a data byte came with no status to run on (byte {0})")]
    NoRunningStatus(usize),
    #[error("MIDI file format {0} is not one of 0, 1 or 2")]
    UnknownFormat(u16),
    #[error("the file has no tracks")]
    NoTracks,
    #[error("the file's time division is zero")]
    ZeroDivision,
    #[error("nothing to export")]
    Empty,
}

/// How the file counts time.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Division {
    /// Ticks per quarter note — musical time, what nearly every file uses.
    Ppq(u16),
    /// SMPTE: frames per second (24, 25, 29 for 29.97 drop-frame, 30) and ticks per frame.
    /// Absolute time, with no beats in it.
    Smpte { fps: u8, ticks_per_frame: u8 },
}

impl Division {
    /// Ticks per second for an SMPTE division.
    pub fn smpte_ticks_per_second(self) -> Option<f64> {
        match self {
            Division::Ppq(_) => None,
            Division::Smpte {
                fps,
                ticks_per_frame,
            } => {
                let fps = if fps == 29 { 29.97 } else { fps as f64 };
                Some(fps * ticks_per_frame as f64)
            }
        }
    }
}

/// One event of a track.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    NoteOff {
        channel: u8,
        key: u8,
        velocity: u8,
    },
    /// A note-on with velocity 0 is kept as it came; [`Event::is_note_off`] says what it is.
    NoteOn {
        channel: u8,
        key: u8,
        velocity: u8,
    },
    PolyPressure {
        channel: u8,
        key: u8,
        value: u8,
    },
    Controller {
        channel: u8,
        controller: u8,
        value: u8,
    },
    Program {
        channel: u8,
        program: u8,
    },
    ChannelPressure {
        channel: u8,
        value: u8,
    },
    /// 14 bits, 8192 is the centre.
    PitchBend {
        channel: u8,
        value: u16,
    },
    /// Microseconds per quarter note.
    Tempo(u32),
    /// Numerator, denominator as a power of two, MIDI clocks per metronome click, and
    /// notated 32nds per quarter.
    TimeSignature {
        numerator: u8,
        denominator_pow: u8,
        clocks: u8,
        n32: u8,
    },
    /// Sharps (positive) or flats, and minor.
    KeySignature {
        sharps: i8,
        minor: bool,
    },
    /// Meta 0x03: the sequence name in the first track of a type 1 file, the track name
    /// elsewhere.
    TrackName(String),
    InstrumentName(String),
    /// Meta 0x01 and 0x05–0x07: text, lyric, marker, cue point. Kept with its type.
    Text {
        kind: u8,
        text: String,
    },
    EndOfTrack,
    /// Any other meta event, kept whole.
    OtherMeta {
        kind: u8,
        data: Vec<u8>,
    },
    /// A system-exclusive message (F0 or F7), kept whole.
    SysEx(Vec<u8>),
}

impl Event {
    pub fn is_note_off(&self) -> bool {
        matches!(
            self,
            Event::NoteOff { .. } | Event::NoteOn { velocity: 0, .. }
        )
    }

    pub fn is_note_on(&self) -> bool {
        matches!(self, Event::NoteOn { velocity, .. } if *velocity > 0)
    }

    pub fn channel(&self) -> Option<u8> {
        match self {
            Event::NoteOff { channel, .. }
            | Event::NoteOn { channel, .. }
            | Event::PolyPressure { channel, .. }
            | Event::Controller { channel, .. }
            | Event::Program { channel, .. }
            | Event::ChannelPressure { channel, .. }
            | Event::PitchBend { channel, .. } => Some(*channel),
            _ => None,
        }
    }
}

/// An event at an absolute tick of its track.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TimedEvent {
    pub tick: u64,
    pub event: Event,
}

/// A whole file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Smf {
    pub format: u16,
    pub division: Division,
    /// Each track's events, by absolute tick, in file order.
    pub tracks: Vec<Vec<TimedEvent>>,
}

// ---- reading ------------------------------------------------------------------------

struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn u8(&mut self) -> Result<u8, MidiError> {
        let b = *self
            .bytes
            .get(self.pos)
            .ok_or(MidiError::Truncated(self.pos))?;
        self.pos += 1;
        Ok(b)
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8], MidiError> {
        let end = self
            .pos
            .checked_add(n)
            .filter(|e| *e <= self.bytes.len())
            .ok_or(MidiError::Truncated(self.bytes.len()))?;
        let s = &self.bytes[self.pos..end];
        self.pos = end;
        Ok(s)
    }

    fn u32(&mut self) -> Result<u32, MidiError> {
        let b = self.take(4)?;
        Ok(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    }

    /// A variable-length quantity: seven bits a byte, high bit set on all but the last,
    /// four bytes at most.
    fn vlq(&mut self) -> Result<u32, MidiError> {
        let mut v = 0u32;
        for _ in 0..4 {
            let b = self.u8()?;
            v = (v << 7) | (b & 0x7F) as u32;
            if b & 0x80 == 0 {
                return Ok(v);
            }
        }
        // A fifth continuation byte: the quantity is malformed. Say where.
        Err(MidiError::Truncated(self.pos))
    }

    fn done(&self) -> bool {
        self.pos >= self.bytes.len()
    }
}

/// Parse a Standard MIDI File.
pub fn read(bytes: &[u8]) -> Result<Smf, MidiError> {
    let mut r = Reader { bytes, pos: 0 };
    // A RIFF-wrapped file (`.rmi`) carries the SMF in its `data` chunk.
    if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"RMID") {
        if let Some(at) = bytes.windows(4).position(|w| w == b"MThd") {
            return read(&bytes[at..]);
        }
    }
    if r.take(4).map_err(|_| MidiError::NotMidi)? != b"MThd" {
        return Err(MidiError::NotMidi);
    }
    let header_len = r.u32()? as usize;
    if header_len < 6 {
        return Err(MidiError::NotMidi);
    }
    let header = r.take(header_len)?;
    let format = u16::from_be_bytes([header[0], header[1]]);
    let ntracks = u16::from_be_bytes([header[2], header[3]]) as usize;
    let div = u16::from_be_bytes([header[4], header[5]]);
    if format > 2 {
        return Err(MidiError::UnknownFormat(format));
    }
    let division = if div & 0x8000 != 0 {
        let fps = (-((div >> 8) as u8 as i8)) as u8;
        Division::Smpte {
            fps,
            ticks_per_frame: (div & 0xFF) as u8,
        }
    } else {
        Division::Ppq(div)
    };
    match division {
        Division::Ppq(0) => return Err(MidiError::ZeroDivision),
        Division::Smpte {
            ticks_per_frame: 0, ..
        }
        | Division::Smpte { fps: 0, .. } => return Err(MidiError::ZeroDivision),
        _ => {}
    }
    let mut tracks = Vec::with_capacity(ntracks.min(256));
    // Every track the header promises; bytes after the last one (padding, a trailer some
    // tools append) are not the file's business.
    while !r.done() && (tracks.len() < ntracks || ntracks == 0) {
        let id = r.take(4)?;
        let len = r.u32()? as usize;
        let body = r.take(len)?;
        if id == b"MTrk" {
            tracks.push(read_track(body)?);
        }
        // Any other chunk is skipped, as the specification asks.
    }
    if tracks.is_empty() {
        return Err(MidiError::NoTracks);
    }
    Ok(Smf {
        format,
        division,
        tracks,
    })
}

fn read_track(body: &[u8]) -> Result<Vec<TimedEvent>, MidiError> {
    let mut r = Reader {
        bytes: body,
        pos: 0,
    };
    let mut out = Vec::new();
    let mut tick = 0u64;
    let mut running: Option<u8> = None;
    while !r.done() {
        tick += r.vlq()? as u64;
        let first = r.u8()?;
        let event = match first {
            0xFF => {
                let kind = r.u8()?;
                let len = r.vlq()? as usize;
                let data = r.take(len)?;
                let ev = meta(kind, data);
                if ev == Event::EndOfTrack {
                    out.push(TimedEvent { tick, event: ev });
                    return Ok(out);
                }
                ev
            }
            0xF0 | 0xF7 => {
                let len = r.vlq()? as usize;
                let data = r.take(len)?;
                let mut msg = Vec::with_capacity(len + 1);
                if first == 0xF0 {
                    msg.push(0xF0);
                }
                msg.extend_from_slice(data);
                // System messages cancel running status.
                running = None;
                Event::SysEx(msg)
            }
            _ => {
                let (status, d1) = if first & 0x80 != 0 {
                    running = Some(first);
                    (first, r.u8()?)
                } else {
                    // Running status: this byte is the first data byte of another
                    // message like the last one.
                    let status = running.ok_or(MidiError::NoRunningStatus(r.pos - 1))?;
                    (status, first)
                };
                channel_event(status, d1, &mut r)?
            }
        };
        out.push(TimedEvent { tick, event });
    }
    // No end-of-track: the chunk's end is taken as one.
    Ok(out)
}

fn channel_event(status: u8, d1: u8, r: &mut Reader) -> Result<Event, MidiError> {
    let channel = status & 0x0F;
    let d1 = d1 & 0x7F;
    Ok(match status & 0xF0 {
        0x80 => Event::NoteOff {
            channel,
            key: d1,
            velocity: r.u8()? & 0x7F,
        },
        0x90 => Event::NoteOn {
            channel,
            key: d1,
            velocity: r.u8()? & 0x7F,
        },
        0xA0 => Event::PolyPressure {
            channel,
            key: d1,
            value: r.u8()? & 0x7F,
        },
        0xB0 => Event::Controller {
            channel,
            controller: d1,
            value: r.u8()? & 0x7F,
        },
        0xC0 => Event::Program {
            channel,
            program: d1,
        },
        0xD0 => Event::ChannelPressure { channel, value: d1 },
        _ => {
            let msb = r.u8()? & 0x7F;
            Event::PitchBend {
                channel,
                value: d1 as u16 | ((msb as u16) << 7),
            }
        }
    })
}

fn meta(kind: u8, data: &[u8]) -> Event {
    let text = || String::from_utf8_lossy(data).into_owned();
    match (kind, data.len()) {
        (0x2F, _) => Event::EndOfTrack,
        (0x51, 3) => Event::Tempo(u32::from_be_bytes([0, data[0], data[1], data[2]])),
        (0x58, 4) => Event::TimeSignature {
            numerator: data[0],
            denominator_pow: data[1],
            clocks: data[2],
            n32: data[3],
        },
        (0x59, 2) => Event::KeySignature {
            sharps: data[0] as i8,
            minor: data[1] != 0,
        },
        (0x03, _) => Event::TrackName(text()),
        (0x04, _) => Event::InstrumentName(text()),
        (0x01 | 0x05 | 0x06 | 0x07, _) => Event::Text { kind, text: text() },
        _ => Event::OtherMeta {
            kind,
            data: data.to_vec(),
        },
    }
}

// ---- writing ------------------------------------------------------------------------

fn put_vlq(out: &mut Vec<u8>, mut v: u32) {
    // Largest a quantity can carry in four bytes; a delta beyond it cannot be written.
    v = v.min(0x0FFF_FFFF);
    let mut buf = [0u8; 4];
    let mut n = 0;
    loop {
        buf[n] = (v & 0x7F) as u8;
        n += 1;
        v >>= 7;
        if v == 0 {
            break;
        }
    }
    for (i, b) in buf[..n].iter().enumerate().rev() {
        out.push(b | if i > 0 { 0x80 } else { 0 });
    }
}

fn put_meta(out: &mut Vec<u8>, kind: u8, data: &[u8]) {
    out.push(0xFF);
    out.push(kind);
    put_vlq(out, data.len() as u32);
    out.extend_from_slice(data);
}

fn put_event(out: &mut Vec<u8>, ev: &Event) {
    let ch = |status: u8, channel: u8| status | (channel & 0x0F);
    match ev {
        Event::NoteOff {
            channel,
            key,
            velocity,
        } => out.extend_from_slice(&[ch(0x80, *channel), key & 0x7F, velocity & 0x7F]),
        Event::NoteOn {
            channel,
            key,
            velocity,
        } => out.extend_from_slice(&[ch(0x90, *channel), key & 0x7F, velocity & 0x7F]),
        Event::PolyPressure {
            channel,
            key,
            value,
        } => out.extend_from_slice(&[ch(0xA0, *channel), key & 0x7F, value & 0x7F]),
        Event::Controller {
            channel,
            controller,
            value,
        } => out.extend_from_slice(&[ch(0xB0, *channel), controller & 0x7F, value & 0x7F]),
        Event::Program { channel, program } => {
            out.extend_from_slice(&[ch(0xC0, *channel), program & 0x7F])
        }
        Event::ChannelPressure { channel, value } => {
            out.extend_from_slice(&[ch(0xD0, *channel), value & 0x7F])
        }
        Event::PitchBend { channel, value } => out.extend_from_slice(&[
            ch(0xE0, *channel),
            (value & 0x7F) as u8,
            ((value >> 7) & 0x7F) as u8,
        ]),
        Event::Tempo(us) => {
            let b = (*us).min(0xFF_FFFF).to_be_bytes();
            put_meta(out, 0x51, &b[1..]);
        }
        Event::TimeSignature {
            numerator,
            denominator_pow,
            clocks,
            n32,
        } => put_meta(out, 0x58, &[*numerator, *denominator_pow, *clocks, *n32]),
        Event::KeySignature { sharps, minor } => {
            put_meta(out, 0x59, &[*sharps as u8, *minor as u8])
        }
        Event::TrackName(s) => put_meta(out, 0x03, s.as_bytes()),
        Event::InstrumentName(s) => put_meta(out, 0x04, s.as_bytes()),
        Event::Text { kind, text } => put_meta(out, *kind, text.as_bytes()),
        Event::EndOfTrack => put_meta(out, 0x2F, &[]),
        Event::OtherMeta { kind, data } => put_meta(out, *kind, data),
        Event::SysEx(msg) => {
            // F0 and its length, then the rest of the message (the F0 is not counted).
            let body = msg.strip_prefix(&[0xF0]).unwrap_or(msg);
            out.push(if msg.first() == Some(&0xF0) {
                0xF0
            } else {
                0xF7
            });
            put_vlq(out, body.len() as u32);
            out.extend_from_slice(body);
        }
    }
}

/// Write a Standard MIDI File.
///
/// Each track's events are written in the order given, which must be by tick; an
/// end-of-track is added where a track lacks one, and nothing follows it. Every event
/// carries its own status byte: running status saves a few bytes and is the one thing an
/// old reader gets wrong, so the writer does not use it (the reader accepts it).
pub fn write(smf: &Smf) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(b"MThd");
    out.extend_from_slice(&6u32.to_be_bytes());
    out.extend_from_slice(&smf.format.to_be_bytes());
    out.extend_from_slice(&(smf.tracks.len().min(u16::MAX as usize) as u16).to_be_bytes());
    let div: u16 = match smf.division {
        Division::Ppq(p) => p & 0x7FFF,
        Division::Smpte {
            fps,
            ticks_per_frame,
        } => (((-(fps as i8)) as u8 as u16) << 8) | ticks_per_frame as u16,
    };
    out.extend_from_slice(&div.to_be_bytes());
    for track in &smf.tracks {
        let mut body = Vec::new();
        let mut last = 0u64;
        let mut ended = false;
        for te in track {
            let delta = te.tick.saturating_sub(last);
            last = last.max(te.tick);
            put_vlq(&mut body, delta.min(u32::MAX as u64) as u32);
            put_event(&mut body, &te.event);
            if te.event == Event::EndOfTrack {
                ended = true;
                break;
            }
        }
        if !ended {
            put_vlq(&mut body, 0);
            put_event(&mut body, &Event::EndOfTrack);
        }
        out.extend_from_slice(b"MTrk");
        out.extend_from_slice(&(body.len() as u32).to_be_bytes());
        out.extend_from_slice(&body);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variable_length_quantities_match_the_specification() {
        // The specification's own table.
        for (v, bytes) in [
            (0x00u32, vec![0x00u8]),
            (0x40, vec![0x40]),
            (0x7F, vec![0x7F]),
            (0x80, vec![0x81, 0x00]),
            (0x2000, vec![0xC0, 0x00]),
            (0x3FFF, vec![0xFF, 0x7F]),
            (0x4000, vec![0x81, 0x80, 0x00]),
            (0x1F_FFFF, vec![0xFF, 0xFF, 0x7F]),
            (0x20_0000, vec![0x81, 0x80, 0x80, 0x00]),
            (0x0FFF_FFFF, vec![0xFF, 0xFF, 0xFF, 0x7F]),
        ] {
            let mut out = Vec::new();
            put_vlq(&mut out, v);
            assert_eq!(out, bytes, "{v:#x}");
            let mut r = Reader {
                bytes: &bytes,
                pos: 0,
            };
            assert_eq!(r.vlq().unwrap(), v);
        }
    }

    #[test]
    fn a_track_without_an_end_gets_one_and_the_length_is_right() {
        let on = Event::NoteOn {
            channel: 0,
            key: 60,
            velocity: 100,
        };
        let off = Event::NoteOff {
            channel: 0,
            key: 60,
            velocity: 64,
        };
        let smf = Smf {
            format: 0,
            division: Division::Ppq(96),
            tracks: vec![vec![
                TimedEvent { tick: 0, event: on },
                TimedEvent {
                    tick: 96,
                    event: off,
                },
            ]],
        };
        let bytes = write(&smf);
        // Header, then MTrk and a length that is exactly the rest of the file.
        let len = u32::from_be_bytes([bytes[18], bytes[19], bytes[20], bytes[21]]) as usize;
        assert_eq!(&bytes[14..18], b"MTrk");
        assert_eq!(bytes.len(), 22 + len);
        assert_eq!(&bytes[bytes.len() - 3..], &[0xFF, 0x2F, 0x00]);
        let back = read(&bytes).unwrap();
        assert_eq!(back.tracks[0].len(), 3);
        assert_eq!(back.tracks[0][2].event, Event::EndOfTrack);
        assert_eq!(back.tracks[0][1].tick, 96);
    }

    #[test]
    fn smpte_division_survives_a_round_trip() {
        let smf = Smf {
            format: 1,
            division: Division::Smpte {
                fps: 25,
                ticks_per_frame: 40,
            },
            tracks: vec![vec![]],
        };
        let back = read(&write(&smf)).unwrap();
        assert_eq!(back.division, smf.division);
        assert_eq!(back.division.smpte_ticks_per_second(), Some(1000.0));
    }

    #[test]
    fn garbage_is_refused_with_a_reason() {
        assert_eq!(read(b"hello"), Err(MidiError::NotMidi));
        assert_eq!(read(b""), Err(MidiError::NotMidi));
        // A header that promises a track the file does not hold.
        let mut bytes = b"MThd\0\0\0\x06\0\x01\0\x01\0\x60MTrk\0\0\x10\0".to_vec();
        bytes.extend_from_slice(&[0, 0x90]);
        assert!(matches!(read(&bytes), Err(MidiError::Truncated(_))));
        // A data byte before any status.
        let bytes = b"MThd\0\0\0\x06\0\0\0\x01\0\x60MTrk\0\0\0\x04\0\x3C\x40\0";
        assert!(matches!(read(bytes), Err(MidiError::NoRunningStatus(_))));
    }
}

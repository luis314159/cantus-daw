//! The MIDI Tuning Standard (round 42, ADR 0155): every message of the MMA's tuning
//! specification (CA-020, CA-021, RP-020), read and written.
//!
//! Layouts from the MMA's own published page (archived 2010, quoted as facts in
//! `research/instruments/microtuning-verified.md` §4); where the spec is loose, ODDSound's
//! 0BSD `libMTSClient` and FluidSynth were read for behaviour (no code copied):
//!
//! - a **frequency word** is three 7-bit bytes — the 12-TET semitone below, then 14 bits of
//!   the fraction of 100 cents above it, `/16384` as the spec says; `7F 7F 7F` is "no change";
//! - **checksums**: CA-020's XOR of every byte between `F0` and the checksum, `& 7F`, is
//!   written; on read the two other conventions in the wild (FluidSynth's bulk dump without
//!   the device and name; Scale Workshop's inverted value) are recognised, and a mismatch is
//!   reported but the tuning still loads.
//!
//! Pitches here are fractional MIDI keys (69.0 = A440), so a key's offset in a
//! [`TuningTable`] is its pitch less the key.

use crate::tuning::TuningTable;
use std::fmt;

/// `F0 7E` (non-real-time) or `F0 7F` (real-time) …, `08` = MIDI tuning.
const NON_REAL_TIME: u8 = 0x7E;
const REAL_TIME: u8 = 0x7F;
const TUNING: u8 = 0x08;
/// The device ID that means every device.
pub const ALL_DEVICES: u8 = 0x7F;
/// Every MIDI channel in a scale/octave message's bitmap.
pub const ALL_CHANNELS: u16 = 0xFFFF;

/// The highest pitch a frequency word holds (`7F 7F 7E`).
pub const PITCH_TOP: f64 = 127.0 + 16382.0 / 16384.0;

/// A frequency word's pitch as a fractional MIDI key; `None` for `7F 7F 7F` ("no change").
pub fn decode_word(w: [u8; 3]) -> Option<f64> {
    if w == [0x7F, 0x7F, 0x7F] {
        return None;
    }
    let frac = (u32::from(w[1] & 0x7F) << 7) | u32::from(w[2] & 0x7F);
    Some(f64::from(w[0] & 0x7F) + f64::from(frac) / 16384.0)
}

/// A pitch (fractional MIDI key) as a frequency word, rounded to the nearest step and kept
/// in the word's range (never the reserved `7F 7F 7F`).
pub fn encode_word(pitch: f64) -> [u8; 3] {
    if !pitch.is_finite() || pitch <= 0.0 {
        return [0, 0, 0];
    }
    let p = pitch.min(PITCH_TOP);
    let mut semi = p.floor() as u32;
    let mut frac = ((p - f64::from(semi)) * 16384.0).round() as u32;
    if frac >= 16384 {
        semi += 1;
        frac = 0;
    }
    if semi > 127 || (semi == 127 && frac > 16382) {
        return [0x7F, 0x7F, 0x7E];
    }
    [semi as u8, (frac >> 7) as u8, (frac & 0x7F) as u8]
}

/// The frequency of a pitch: `440·2^((p − 69)/12)`.
pub fn pitch_hz(pitch: f64) -> f64 {
    440.0 * cantus_dsp::math::exp2((pitch - 69.0) / 12.0)
}

/// Which convention a dump's checksum follows.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Checksum {
    /// CA-020: the XOR of every byte from the sub-ID#1 through the data, `& 7F`.
    Spec,
    /// FluidSynth's bulk dump: without the device ID and the name.
    FluidSynth,
    /// Scale Workshop's: the spec's value inverted (the XOR seeded with `FF`).
    ScaleWorkshop,
    /// None of them: reported, the tuning still loads.
    Mismatch,
}

/// One MIDI tuning message.
#[derive(Clone, Debug, PartialEq)]
pub enum MtsMessage {
    /// `00` (bank `None`) or `03`: a request for a dump.
    DumpRequest {
        device: u8,
        bank: Option<u8>,
        program: u8,
    },
    /// `01` (bank `None`) or `04`: all 128 keys' pitches, `None` where unchanged.
    KeyDump {
        device: u8,
        bank: Option<u8>,
        program: u8,
        name: String,
        keys: Box<[Option<f64>; 128]>,
        checksum: Checksum,
    },
    /// `02` (real-time, bank `None`) or `07`: some keys' new pitches.
    NoteChange {
        device: u8,
        realtime: bool,
        bank: Option<u8>,
        program: u8,
        changes: Vec<(u8, Option<f64>)>,
    },
    /// `05`/`06`: a scale/octave dump, each pitch class's cents from 12-TET.
    OctaveDump {
        device: u8,
        bank: u8,
        program: u8,
        name: String,
        cents: [f64; 12],
        two_byte: bool,
        checksum: Checksum,
    },
    /// `08`/`09`: scale/octave tuning for the channels of `channels` (bit 0 = channel 1).
    Octave {
        device: u8,
        realtime: bool,
        channels: u16,
        cents: [f64; 12],
        two_byte: bool,
    },
}

/// Why bytes are not a tuning message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MtsError {
    /// Not a universal SysEx of the MIDI tuning sub-ID.
    NotTuning,
    /// A sub-ID#2 the spec does not define.
    UnknownMessage(u8),
    /// The message's length is not its layout's.
    Length { want: usize, got: usize },
    /// A data byte with its top bit set.
    NotSevenBit,
}

impl fmt::Display for MtsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MtsError::NotTuning => write!(f, "not a MIDI tuning message"),
            MtsError::UnknownMessage(s) => write!(f, "unknown MIDI tuning message {s:02X}"),
            MtsError::Length { want, got } => {
                write!(f, "a MIDI tuning message of {got} bytes where {want} were due")
            }
            MtsError::NotSevenBit => write!(f, "a data byte past 7 bits"),
        }
    }
}

impl std::error::Error for MtsError {}

/// The XOR of `bytes`, `& 7F`.
fn xor(bytes: &[u8]) -> u8 {
    bytes.iter().fold(0u8, |a, b| a ^ b) & 0x7F
}

fn name_of(b: &[u8]) -> String {
    b.iter()
        .map(|&c| if (0x20..0x7F).contains(&c) { char::from(c) } else { ' ' })
        .collect::<String>()
        .trim_end()
        .to_string()
}

fn name_bytes(name: &str) -> [u8; 16] {
    let mut out = [b' '; 16];
    for (o, c) in out.iter_mut().zip(name.chars().filter(|c| c.is_ascii() && !c.is_ascii_control())) {
        *o = c as u8;
    }
    out
}

/// 1-byte scale/octave offset: `00` = −64 c, `40` = 0, `7F` = +63 c.
fn cents_1(v: u8) -> f64 {
    f64::from(v & 0x7F) - 64.0
}

/// 2-byte offset: 14 bits, `40 00` = 0, steps of 200/16384 c (−100 … +100).
fn cents_2(hi: u8, lo: u8) -> f64 {
    let v = (i32::from(hi & 0x7F) << 7) | i32::from(lo & 0x7F);
    f64::from(v - 8192) * 200.0 / 16384.0
}

fn enc_cents_1(c: f64) -> u8 {
    (64.0 + c).round().clamp(0.0, 127.0) as u8
}

fn enc_cents_2(c: f64) -> [u8; 2] {
    let v = (8192.0 + c * 16384.0 / 200.0).round().clamp(0.0, 16383.0) as u16;
    [(v >> 7) as u8, (v & 0x7F) as u8]
}

/// Read one MIDI tuning message (`F0 … F7`; the framing bytes may be absent).
pub fn parse(bytes: &[u8]) -> Result<MtsMessage, MtsError> {
    let b = bytes.strip_prefix(&[0xF0]).unwrap_or(bytes);
    let b = b.strip_suffix(&[0xF7]).unwrap_or(b);
    if b.iter().any(|&x| x > 0x7F) {
        return Err(MtsError::NotSevenBit);
    }
    if b.len() < 4 || !(b[0] == NON_REAL_TIME || b[0] == REAL_TIME) || b[2] != TUNING {
        return Err(MtsError::NotTuning);
    }
    let (realtime, device, sub) = (b[0] == REAL_TIME, b[1], b[3]);
    let len = |want: usize| -> Result<(), MtsError> {
        // `want` counts the bytes between F0 and F7.
        if b.len() == want {
            Ok(())
        } else {
            Err(MtsError::Length { want, got: b.len() })
        }
    };
    let keys_of = |data: &[u8]| -> Box<[Option<f64>; 128]> {
        let mut keys = Box::new([None; 128]);
        for (k, w) in keys.iter_mut().zip(data.chunks_exact(3)) {
            *k = decode_word([w[0], w[1], w[2]]);
        }
        keys
    };
    match sub {
        0x00 => {
            len(5)?;
            Ok(MtsMessage::DumpRequest {
                device,
                bank: None,
                program: b[4],
            })
        }
        0x03 => {
            len(6)?;
            Ok(MtsMessage::DumpRequest {
                device,
                bank: Some(b[4]),
                program: b[5],
            })
        }
        0x01 => {
            len(406)?;
            let (program, name, data, chk) = (b[4], &b[5..21], &b[21..405], b[405]);
            let spec = xor(&b[..405]);
            let fluid = xor(&[NON_REAL_TIME, TUNING, 0x01, program]) ^ xor(data);
            let checksum = if chk == spec {
                Checksum::Spec
            } else if chk == fluid {
                Checksum::FluidSynth
            } else if chk == spec ^ 0x7F {
                Checksum::ScaleWorkshop
            } else {
                Checksum::Mismatch
            };
            Ok(MtsMessage::KeyDump {
                device,
                bank: None,
                program,
                name: name_of(name),
                keys: keys_of(data),
                checksum,
            })
        }
        0x04 => {
            len(407)?;
            let (bank, program, name, data, chk) = (b[4], b[5], &b[6..22], &b[22..406], b[406]);
            Ok(MtsMessage::KeyDump {
                device,
                bank: Some(bank),
                program,
                name: name_of(name),
                keys: keys_of(data),
                checksum: checksum_of(&b[..406], chk),
            })
        }
        0x02 | 0x07 => {
            let banked = sub == 0x07;
            let head = if banked { 7 } else { 6 };
            if b.len() < head {
                return Err(MtsError::Length {
                    want: head,
                    got: b.len(),
                });
            }
            let ll = usize::from(b[head - 1]);
            len(head + 4 * ll)?;
            let changes = b[head..]
                .chunks_exact(4)
                .map(|c| (c[0], decode_word([c[1], c[2], c[3]])))
                .collect();
            Ok(MtsMessage::NoteChange {
                device,
                // `02` is real-time by definition; `07` says so by its sub-ID#1.
                realtime: if banked { realtime } else { true },
                bank: banked.then_some(b[4]),
                program: b[head - 2],
                changes,
            })
        }
        0x05 | 0x06 => {
            let two_byte = sub == 0x06;
            let n = if two_byte { 24 } else { 12 };
            len(22 + n + 1)?;
            let (bank, program, name) = (b[4], b[5], &b[6..22]);
            let data = &b[22..22 + n];
            Ok(MtsMessage::OctaveDump {
                device,
                bank,
                program,
                name: name_of(name),
                cents: octave_cents(data, two_byte),
                two_byte,
                checksum: checksum_of(&b[..22 + n], b[22 + n]),
            })
        }
        0x08 | 0x09 => {
            let two_byte = sub == 0x09;
            let n = if two_byte { 24 } else { 12 };
            len(7 + n)?;
            let channels =
                (u16::from(b[4] & 0x03) << 14) | (u16::from(b[5]) << 7) | u16::from(b[6]);
            Ok(MtsMessage::Octave {
                device,
                realtime,
                channels,
                cents: octave_cents(&b[7..7 + n], two_byte),
                two_byte,
            })
        }
        s => Err(MtsError::UnknownMessage(s)),
    }
}

fn checksum_of(body: &[u8], chk: u8) -> Checksum {
    let spec = xor(body);
    if chk == spec {
        Checksum::Spec
    } else if chk == spec ^ 0x7F {
        Checksum::ScaleWorkshop
    } else {
        Checksum::Mismatch
    }
}

fn octave_cents(data: &[u8], two_byte: bool) -> [f64; 12] {
    let mut out = [0.0; 12];
    for (i, o) in out.iter_mut().enumerate() {
        *o = if two_byte {
            cents_2(data[2 * i], data[2 * i + 1])
        } else {
            cents_1(data[i])
        };
    }
    out
}

impl MtsMessage {
    /// Apply the message to 128 pitches (fractional MIDI keys). A scale/octave message sets
    /// each key from 12-TET, as the spec says ("offset from the original preset").
    pub fn apply(&self, pitches: &mut [f64; 128]) {
        match self {
            MtsMessage::DumpRequest { .. } => {}
            MtsMessage::KeyDump { keys, .. } => {
                for (p, k) in pitches.iter_mut().zip(keys.iter()) {
                    if let Some(v) = k {
                        *p = *v;
                    }
                }
            }
            MtsMessage::NoteChange { changes, .. } => {
                for &(k, v) in changes {
                    if let (Some(p), Some(v)) = (pitches.get_mut(usize::from(k)), v) {
                        *p = v;
                    }
                }
            }
            MtsMessage::OctaveDump { cents, .. } | MtsMessage::Octave { cents, .. } => {
                for (k, p) in pitches.iter_mut().enumerate() {
                    *p = k as f64 + cents[k % 12] / 100.0;
                }
            }
        }
    }
}

/// 12-TET's pitches, key by key: what messages are applied to.
pub fn equal_pitches() -> [f64; 128] {
    std::array::from_fn(|k| k as f64)
}

/// 128 pitches as a [`TuningTable`] (each key's pitch less the key).
pub fn table_of(pitches: &[f64; 128]) -> TuningTable {
    TuningTable(std::array::from_fn(|k| (pitches[k] - k as f64) as f32))
}

/// A table's pitches; an unmapped key (`NAN`) is `None` ("no change", `7F 7F 7F`).
pub fn pitches_of(table: &TuningTable) -> [Option<f64>; 128] {
    std::array::from_fn(|k| {
        let o = table.0[k];
        (!o.is_nan()).then(|| k as f64 + f64::from(o))
    })
}

fn framed(body: Vec<u8>) -> Vec<u8> {
    let mut m = Vec::with_capacity(body.len() + 2);
    m.push(0xF0);
    m.extend(body);
    m.push(0xF7);
    m
}

fn words(table: &TuningTable) -> Vec<u8> {
    let mut out = Vec::with_capacity(384);
    for p in pitches_of(table) {
        out.extend(match p {
            Some(p) => encode_word(p),
            None => [0x7F, 0x7F, 0x7F],
        });
    }
    out
}

/// `01`: the bulk dump of a table (408 bytes), CA-020's checksum.
pub fn write_key_dump(device: u8, program: u8, name: &str, table: &TuningTable) -> Vec<u8> {
    let mut b = vec![NON_REAL_TIME, device & 0x7F, TUNING, 0x01, program & 0x7F];
    b.extend(name_bytes(name));
    b.extend(words(table));
    b.push(xor(&b));
    framed(b)
}

/// `04`: the key-based dump of a table in `bank` (409 bytes).
pub fn write_bank_key_dump(
    device: u8,
    bank: u8,
    program: u8,
    name: &str,
    table: &TuningTable,
) -> Vec<u8> {
    let mut b = vec![
        NON_REAL_TIME,
        device & 0x7F,
        TUNING,
        0x04,
        bank & 0x7F,
        program & 0x7F,
    ];
    b.extend(name_bytes(name));
    b.extend(words(table));
    b.push(xor(&b));
    framed(b)
}

/// `02`: real-time single-note changes for `changes` (at most 127 per message; more are
/// split across messages).
pub fn write_note_changes(device: u8, program: u8, changes: &[(u8, f64)]) -> Vec<Vec<u8>> {
    changes
        .chunks(127)
        .map(|chunk| {
            let mut b = vec![
                REAL_TIME,
                device & 0x7F,
                TUNING,
                0x02,
                program & 0x7F,
                chunk.len() as u8,
            ];
            for &(k, p) in chunk {
                b.push(k & 0x7F);
                b.extend(encode_word(p));
            }
            framed(b)
        })
        .collect()
}

/// `02` for every key of a table, in two messages of 64 (an unmapped key keeps its pitch).
pub fn write_table_changes(device: u8, program: u8, table: &TuningTable) -> Vec<Vec<u8>> {
    let changes: Vec<(u8, f64)> = pitches_of(table)
        .iter()
        .enumerate()
        .filter_map(|(k, p)| p.map(|p| (k as u8, p)))
        .collect();
    changes
        .chunks(64)
        .flat_map(|c| write_note_changes(device, program, c))
        .collect()
}

/// `08`/`09`: scale/octave tuning (cents from 12-TET per pitch class, C first).
pub fn write_octave(
    device: u8,
    realtime: bool,
    channels: u16,
    cents: &[f64; 12],
    two_byte: bool,
) -> Vec<u8> {
    let mut b = vec![
        if realtime { REAL_TIME } else { NON_REAL_TIME },
        device & 0x7F,
        TUNING,
        if two_byte { 0x09 } else { 0x08 },
        ((channels >> 14) & 0x03) as u8,
        ((channels >> 7) & 0x7F) as u8,
        (channels & 0x7F) as u8,
    ];
    for &c in cents {
        if two_byte {
            b.extend(enc_cents_2(c));
        } else {
            b.push(enc_cents_1(c));
        }
    }
    framed(b)
}

/// `05`/`06`: a scale/octave dump in `bank`.
pub fn write_octave_dump(
    device: u8,
    bank: u8,
    program: u8,
    name: &str,
    cents: &[f64; 12],
    two_byte: bool,
) -> Vec<u8> {
    let mut b = vec![
        NON_REAL_TIME,
        device & 0x7F,
        TUNING,
        if two_byte { 0x06 } else { 0x05 },
        bank & 0x7F,
        program & 0x7F,
    ];
    b.extend(name_bytes(name));
    for &c in cents {
        if two_byte {
            b.extend(enc_cents_2(c));
        } else {
            b.push(enc_cents_1(c));
        }
    }
    b.push(xor(&b));
    framed(b)
}

/// `00`/`03`: a dump request.
pub fn write_dump_request(device: u8, bank: Option<u8>, program: u8) -> Vec<u8> {
    let mut b = vec![NON_REAL_TIME, device & 0x7F, TUNING];
    match bank {
        None => b.extend([0x00, program & 0x7F]),
        Some(bank) => b.extend([0x03, bank & 0x7F, program & 0x7F]),
    }
    framed(b)
}

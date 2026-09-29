//! DX7 SysEx: single voices and 32-voice cartridges, read and written from the documented
//! format (the DX7's owner's manual and its published bulk-dump layout; nothing here comes
//! from Dexed), and a DX voice turned into a Modulus patch. Parsing happens here, on the
//! UI side; the patch reaches the project as one batch of commands, one undo step.
//!
//! | Message | Bytes |
//! |---|---|
//! | one voice | `F0 43 0n 00 01 1B`, 155 data bytes, checksum, `F7` (163) |
//! | 32 voices | `F0 43 0n 09 20 00`, 32 × 128 packed bytes, checksum, `F7` (4104) |
//!
//! The checksum is the two's complement of the data's sum, seven bits. Operators are stored
//! 6 first. Out-of-range values are clamped and reported; a wrong checksum is reported but
//! the voices still load (cartridges in the wild often carry one).

/// A modulator at DX output level 99 and full envelope: about 4π radians of peak phase
/// deviation (Chowning and Bristow's table gives about 13 for level 99).
pub const DX_FULL_INDEX: f32 = 4.0 * std::f32::consts::PI;

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct DxOp {
    pub rates: [u8; 4],
    pub levels: [u8; 4],
    pub break_point: u8,
    pub left_depth: u8,
    pub right_depth: u8,
    pub left_curve: u8,
    pub right_curve: u8,
    pub rate_scaling: u8,
    pub ams: u8,
    pub velocity: u8,
    pub output: u8,
    pub fixed: bool,
    pub coarse: u8,
    pub fine: u8,
    pub detune: u8,
}

/// One DX7 voice; `ops[0]` is operator 1.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DxVoice {
    pub ops: [DxOp; 6],
    pub pitch_rates: [u8; 4],
    pub pitch_levels: [u8; 4],
    /// 0…31 (algorithm 1…32).
    pub algorithm: u8,
    pub feedback: u8,
    pub key_sync: bool,
    pub lfo_speed: u8,
    pub lfo_delay: u8,
    pub lfo_pmd: u8,
    pub lfo_amd: u8,
    pub lfo_sync: bool,
    pub lfo_wave: u8,
    pub pms: u8,
    pub transpose: u8,
    /// Ten characters.
    pub name: [u8; 10],
}

impl DxVoice {
    pub fn name(&self) -> String {
        String::from_utf8_lossy(&self.name).trim_end().to_string()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Parsed {
    Voice(DxVoice),
    Bank(Vec<DxVoice>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SysexError {
    NotSysex,
    NotYamaha,
    Format(u8),
    Length { expected: usize, found: usize },
}

/// What a read found, and what it had to fix.
#[derive(Clone, Debug, PartialEq)]
pub struct Read {
    pub parsed: Parsed,
    /// Values clamped into range.
    pub clamped: usize,
    pub checksum_ok: bool,
}

fn checksum(data: &[u8]) -> u8 {
    (0u8.wrapping_sub(data.iter().fold(0u8, |a, b| a.wrapping_add(*b)))) & 0x7F
}

/// Parse one message (a `.syx` file of one voice or one cartridge).
pub fn parse(bytes: &[u8]) -> Result<Read, SysexError> {
    if bytes.first() != Some(&0xF0) {
        return Err(SysexError::NotSysex);
    }
    if bytes.get(1) != Some(&0x43) {
        return Err(SysexError::NotYamaha);
    }
    let format = *bytes.get(3).ok_or(SysexError::NotSysex)?;
    let len = match format {
        0 => 155,
        9 => 4096,
        f => return Err(SysexError::Format(f)),
    };
    let need = 6 + len + 2;
    if bytes.len() < need {
        return Err(SysexError::Length {
            expected: need,
            found: bytes.len(),
        });
    }
    let data = &bytes[6..6 + len];
    let checksum_ok = checksum(data) == bytes[6 + len];
    let mut clamped = 0;
    let parsed = if format == 0 {
        Parsed::Voice(read_voice155(data, &mut clamped))
    } else {
        Parsed::Bank(
            data.chunks(128)
                .map(|c| unpack_voice(c, &mut clamped))
                .collect(),
        )
    };
    Ok(Read {
        parsed,
        clamped,
        checksum_ok,
    })
}

fn clamp(v: u8, max: u8, n: &mut usize) -> u8 {
    if v > max {
        *n += 1;
        max
    } else {
        v
    }
}

fn read_op155(d: &[u8], n: &mut usize) -> DxOp {
    let c = |i: usize, max: u8, n: &mut usize| clamp(d[i], max, n);
    DxOp {
        rates: [c(0, 99, n), c(1, 99, n), c(2, 99, n), c(3, 99, n)],
        levels: [c(4, 99, n), c(5, 99, n), c(6, 99, n), c(7, 99, n)],
        break_point: c(8, 99, n),
        left_depth: c(9, 99, n),
        right_depth: c(10, 99, n),
        left_curve: c(11, 3, n),
        right_curve: c(12, 3, n),
        rate_scaling: c(13, 7, n),
        ams: c(14, 3, n),
        velocity: c(15, 7, n),
        output: c(16, 99, n),
        fixed: c(17, 1, n) == 1,
        coarse: c(18, 31, n),
        fine: c(19, 99, n),
        detune: c(20, 14, n),
    }
}

/// The 155-byte unpacked voice.
pub fn read_voice155(d: &[u8], n: &mut usize) -> DxVoice {
    let mut v = DxVoice::default();
    for i in 0..6 {
        v.ops[5 - i] = read_op155(&d[i * 21..i * 21 + 21], n);
    }
    let c = |i: usize, max: u8, n: &mut usize| clamp(d[i], max, n);
    for k in 0..4 {
        v.pitch_rates[k] = c(126 + k, 99, n);
        v.pitch_levels[k] = c(130 + k, 99, n);
    }
    v.algorithm = c(134, 31, n);
    v.feedback = c(135, 7, n);
    v.key_sync = c(136, 1, n) == 1;
    v.lfo_speed = c(137, 99, n);
    v.lfo_delay = c(138, 99, n);
    v.lfo_pmd = c(139, 99, n);
    v.lfo_amd = c(140, 99, n);
    v.lfo_sync = c(141, 1, n) == 1;
    v.lfo_wave = c(142, 5, n);
    v.pms = c(143, 7, n);
    v.transpose = c(144, 48, n);
    for k in 0..10 {
        v.name[k] = d[145 + k] & 0x7F;
    }
    v
}

pub fn write_voice155(v: &DxVoice) -> [u8; 155] {
    let mut d = [0u8; 155];
    for i in 0..6 {
        let o = &v.ops[5 - i];
        let b = &mut d[i * 21..i * 21 + 21];
        b[..4].copy_from_slice(&o.rates);
        b[4..8].copy_from_slice(&o.levels);
        b[8..21].copy_from_slice(&[
            o.break_point,
            o.left_depth,
            o.right_depth,
            o.left_curve,
            o.right_curve,
            o.rate_scaling,
            o.ams,
            o.velocity,
            o.output,
            o.fixed as u8,
            o.coarse,
            o.fine,
            o.detune,
        ]);
    }
    d[126..130].copy_from_slice(&v.pitch_rates);
    d[130..134].copy_from_slice(&v.pitch_levels);
    d[134..145].copy_from_slice(&[
        v.algorithm,
        v.feedback,
        v.key_sync as u8,
        v.lfo_speed,
        v.lfo_delay,
        v.lfo_pmd,
        v.lfo_amd,
        v.lfo_sync as u8,
        v.lfo_wave,
        v.pms,
        v.transpose,
    ]);
    d[145..].copy_from_slice(&v.name);
    d
}

/// The 128-byte packed voice of a cartridge.
pub fn unpack_voice(d: &[u8], n: &mut usize) -> DxVoice {
    let mut v = DxVoice::default();
    for i in 0..6 {
        let b = &d[i * 17..i * 17 + 17];
        let c = |x: u8, max: u8, n: &mut usize| clamp(x, max, n);
        v.ops[5 - i] = DxOp {
            rates: [
                c(b[0], 99, n),
                c(b[1], 99, n),
                c(b[2], 99, n),
                c(b[3], 99, n),
            ],
            levels: [
                c(b[4], 99, n),
                c(b[5], 99, n),
                c(b[6], 99, n),
                c(b[7], 99, n),
            ],
            break_point: c(b[8], 99, n),
            left_depth: c(b[9], 99, n),
            right_depth: c(b[10], 99, n),
            left_curve: b[11] & 3,
            right_curve: (b[11] >> 2) & 3,
            rate_scaling: b[12] & 7,
            detune: c((b[12] >> 3) & 15, 14, n),
            ams: b[13] & 3,
            velocity: (b[13] >> 2) & 7,
            output: c(b[14], 99, n),
            fixed: b[15] & 1 == 1,
            coarse: (b[15] >> 1) & 31,
            fine: c(b[16], 99, n),
        };
    }
    let c = |i: usize, max: u8, n: &mut usize| clamp(d[i], max, n);
    for k in 0..4 {
        v.pitch_rates[k] = c(102 + k, 99, n);
        v.pitch_levels[k] = c(106 + k, 99, n);
    }
    v.algorithm = d[110] & 31;
    v.feedback = d[111] & 7;
    v.key_sync = (d[111] >> 3) & 1 == 1;
    v.lfo_speed = c(112, 99, n);
    v.lfo_delay = c(113, 99, n);
    v.lfo_pmd = c(114, 99, n);
    v.lfo_amd = c(115, 99, n);
    v.lfo_sync = d[116] & 1 == 1;
    v.lfo_wave = clamp((d[116] >> 1) & 7, 5, n);
    v.pms = (d[116] >> 4) & 7;
    v.transpose = c(117, 48, n);
    for k in 0..10 {
        v.name[k] = d[118 + k] & 0x7F;
    }
    v
}

pub fn pack_voice(v: &DxVoice) -> [u8; 128] {
    let mut d = [0u8; 128];
    for i in 0..6 {
        let o = &v.ops[5 - i];
        let b = &mut d[i * 17..i * 17 + 17];
        b[..4].copy_from_slice(&o.rates);
        b[4..8].copy_from_slice(&o.levels);
        b[8] = o.break_point;
        b[9] = o.left_depth;
        b[10] = o.right_depth;
        b[11] = o.left_curve | (o.right_curve << 2);
        b[12] = o.rate_scaling | (o.detune << 3);
        b[13] = o.ams | (o.velocity << 2);
        b[14] = o.output;
        b[15] = o.fixed as u8 | (o.coarse << 1);
        b[16] = o.fine;
    }
    d[102..106].copy_from_slice(&v.pitch_rates);
    d[106..110].copy_from_slice(&v.pitch_levels);
    d[110] = v.algorithm;
    d[111] = v.feedback | ((v.key_sync as u8) << 3);
    d[112..116].copy_from_slice(&[v.lfo_speed, v.lfo_delay, v.lfo_pmd, v.lfo_amd]);
    d[116] = v.lfo_sync as u8 | (v.lfo_wave << 1) | (v.pms << 4);
    d[117] = v.transpose;
    d[118..].copy_from_slice(&v.name);
    d
}

fn message(format: u8, count: [u8; 2], data: &[u8]) -> Vec<u8> {
    let mut m = vec![0xF0, 0x43, 0x00, format, count[0], count[1]];
    m.extend_from_slice(data);
    m.push(checksum(data));
    m.push(0xF7);
    m
}

/// A single-voice message.
pub fn voice_sysex(v: &DxVoice) -> Vec<u8> {
    message(0, [0x01, 0x1B], &write_voice155(v))
}

/// A 32-voice cartridge (missing voices are the init voice).
pub fn bank_sysex(voices: &[DxVoice]) -> Vec<u8> {
    let mut data = Vec::with_capacity(4096);
    for i in 0..32 {
        let v = voices.get(i).cloned().unwrap_or_else(init_voice);
        data.extend_from_slice(&pack_voice(&v));
    }
    message(9, [0x20, 0x00], &data)
}

/// The DX7's INIT VOICE: operator 1 alone, a sine at full level.
pub fn init_voice() -> DxVoice {
    let op = |output: u8| DxOp {
        rates: [99; 4],
        levels: [99, 99, 99, 0],
        break_point: 39,
        output,
        coarse: 1,
        detune: 7,
        ..Default::default()
    };
    DxVoice {
        ops: [op(99), op(0), op(0), op(0), op(0), op(0)],
        pitch_rates: [99; 4],
        pitch_levels: [50; 4],
        algorithm: 0,
        key_sync: true,
        lfo_speed: 35,
        lfo_sync: true,
        pms: 3,
        transpose: 24,
        name: *b"INIT VOICE",
        ..Default::default()
    }
}

#[path = "fm_dx_map.rs"]
mod map;
pub use map::{lfo_hz, to_spec};

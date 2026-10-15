//! A small, strict WAV reader and a 24-bit stereo writer — what a pack's build step needs
//! to mix a multi-microphone drum kit (round 28: DrumGizmo's DRSKit ships one 13-channel
//! 32-bit float file per hit) into the stereo files Exemplar plays.
//!
//! Reads RIFF/WAVE with PCM integers (8, 16, 24 or 32 bits) or IEEE float (32 or 64 bits),
//! any number of channels, `WAVE_FORMAT_EXTENSIBLE` included; every other chunk is skipped.
//! A file that ends inside a chunk it needs, or whose format is anything else, is refused
//! with the reason: a pack is built from what it says it holds, or not at all.

use std::io::{Read, Write};
use std::path::Path;

/// Interleaved samples, `channels` per frame.
#[derive(Clone, Debug, PartialEq)]
pub struct Audio {
    pub channels: usize,
    pub rate: u32,
    pub samples: Vec<f32>,
}

impl Audio {
    pub fn frames(&self) -> usize {
        self.samples.len() / self.channels.max(1)
    }
}

fn u16_at(b: &[u8], i: usize) -> u16 {
    u16::from_le_bytes([b[i], b[i + 1]])
}

fn u32_at(b: &[u8], i: usize) -> u32 {
    u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]])
}

/// Read a WAV file whole.
pub fn read(path: &Path) -> Result<Audio, String> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .and_then(|mut f| f.read_to_end(&mut bytes))
        .map_err(|e| format!("{}: {e}", path.display()))?;
    parse(&bytes).map_err(|e| format!("{}: {e}", path.display()))
}

/// Parse a WAV file's bytes.
pub fn parse(b: &[u8]) -> Result<Audio, String> {
    if b.len() < 12 || &b[0..4] != b"RIFF" || &b[8..12] != b"WAVE" {
        return Err("not a RIFF/WAVE file".into());
    }
    let mut at = 12;
    let mut fmt: Option<(u16, usize, u32, u16)> = None;
    while at + 8 <= b.len() {
        let id = &b[at..at + 4];
        let size = u32_at(b, at + 4) as usize;
        let body = at + 8;
        let end = body.checked_add(size).ok_or("a chunk's size overflows")?;
        if id == b"fmt " {
            if end > b.len() || size < 16 {
                return Err("the format chunk is cut short".into());
            }
            let mut tag = u16_at(b, body);
            let channels = u16_at(b, body + 2) as usize;
            let rate = u32_at(b, body + 4);
            let bits = u16_at(b, body + 14);
            if tag == 0xFFFE {
                // WAVE_FORMAT_EXTENSIBLE: the real tag is the sub-format GUID's first two bytes.
                if size < 40 {
                    return Err("an extensible format chunk is cut short".into());
                }
                tag = u16_at(b, body + 24);
            }
            if channels == 0 || rate == 0 {
                return Err("no channels or no sample rate".into());
            }
            fmt = Some((tag, channels, rate, bits));
        } else if id == b"data" {
            let Some((tag, channels, rate, bits)) = fmt else {
                return Err("the audio comes before its format".into());
            };
            if end > b.len() {
                return Err("the audio is cut short".into());
            }
            let data = &b[body..end];
            let samples = decode(data, tag, bits)?;
            if samples.len() % channels != 0 {
                return Err("the audio ends inside a frame".into());
            }
            return Ok(Audio {
                channels,
                rate,
                samples,
            });
        }
        // Chunks are padded to an even size.
        at = end + (size & 1);
    }
    Err("no audio chunk".into())
}

fn decode(d: &[u8], tag: u16, bits: u16) -> Result<Vec<f32>, String> {
    Ok(match (tag, bits) {
        (1, 8) => d.iter().map(|&x| (x as f32 - 128.0) / 128.0).collect(),
        (1, 16) => d
            .chunks_exact(2)
            .map(|c| i16::from_le_bytes([c[0], c[1]]) as f32 / 32_768.0)
            .collect(),
        (1, 24) => d
            .chunks_exact(3)
            .map(|c| {
                let v = i32::from_le_bytes([0, c[0], c[1], c[2]]) >> 8;
                v as f32 / 8_388_608.0
            })
            .collect(),
        (1, 32) => d
            .chunks_exact(4)
            .map(|c| (i32::from_le_bytes([c[0], c[1], c[2], c[3]]) as f64 / 2_147_483_648.0) as f32)
            .collect(),
        (3, 32) => d
            .chunks_exact(4)
            .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect(),
        (3, 64) => d
            .chunks_exact(8)
            .map(|c| f64::from_le_bytes(c.try_into().expect("eight bytes")) as f32)
            .collect(),
        _ => return Err(format!("format {tag} at {bits} bits is not read")),
    })
}

/// A sample as a 24-bit integer: rounded to the nearest step, clamped to the range.
/// Returns the value and whether it was clamped.
pub fn to_i24(x: f32) -> (i32, bool) {
    let v = (x as f64 * 8_388_608.0).round();
    if v > 8_388_607.0 {
        (8_388_607, true)
    } else if v < -8_388_608.0 {
        (-8_388_608, true)
    } else if v.is_nan() {
        (0, true)
    } else {
        (v as i32, false)
    }
}

/// Write interleaved stereo `samples` as a 24-bit PCM WAV. Returns how many samples were
/// clamped (past full scale, or not a number).
pub fn write_stereo_24(path: &Path, rate: u32, samples: &[f32]) -> Result<u64, String> {
    let frames = samples.len() / 2;
    let data_len = frames * 6;
    let mut out = Vec::with_capacity(44 + data_len);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&((36 + data_len) as u32).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&rate.to_le_bytes());
    out.extend_from_slice(&(rate * 6).to_le_bytes());
    out.extend_from_slice(&6u16.to_le_bytes());
    out.extend_from_slice(&24u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&(data_len as u32).to_le_bytes());
    let mut clamped = 0u64;
    for &x in &samples[..frames * 2] {
        let (v, c) = to_i24(x);
        clamped += c as u64;
        out.extend_from_slice(&v.to_le_bytes()[..3]);
    }
    let mut f = std::fs::File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
    f.write_all(&out)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(clamped)
}

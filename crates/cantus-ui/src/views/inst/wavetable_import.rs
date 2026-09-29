//! Unda's table import, on the UI side (the audio thread never reads a file): what a WAV
//! says about its frame size, and Surge's `.wt` turned into a WAV the engine decodes like
//! any other. Formats are described in `research/instruments/wavetable.md` §8.4; nothing
//! here is copied from another program.
//!
//! A WAV's frame size, in the order it is looked for:
//! 1. a `clm ` chunk (Serum: `<!>2048 …`): the number after `<!>`;
//! 2. a `uhWT` chunk (u-he): 2048;
//! 3. a `srge` chunk (Surge): its second little-endian `i32`;
//! 4. an `smpl` chunk: the first loop's length (none: 2048);
//! 5. a `cue ` chunk with two or more points: their spacing;
//! 6. no chunk: 2048 when the length is a multiple of it, otherwise the whole file as
//!    one cycle (a single-cycle waveform, up to 4096 samples) or 2048 (the rest).

use std::path::{Path, PathBuf};

/// What an import found out about a file.
#[derive(Clone, Debug, PartialEq)]
pub struct Found {
    /// The WAV the engine will read (the file itself, or a converted copy).
    pub path: PathBuf,
    /// Samples per cycle.
    pub frame: usize,
    /// Cycles in the file (before any thinning to 256).
    pub frames: usize,
    /// Where the frame size came from (a locale key's last part: `clm`, `uhwt`, …).
    pub source: &'static str,
}

/// Why a file could not be imported (a locale key under `unda.import.`).
#[derive(Clone, Debug, PartialEq)]
pub enum ImportError {
    Unreadable,
    NotWav,
    Empty,
    BadWt,
}

impl ImportError {
    pub fn key(&self) -> &'static str {
        match self {
            ImportError::Unreadable => "unda.import.unreadable",
            ImportError::NotWav => "unda.import.not_wav",
            ImportError::Empty => "unda.import.empty",
            ImportError::BadWt => "unda.import.bad_wt",
        }
    }
}

fn u16le(b: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(b.get(at..at + 2)?.try_into().ok()?))
}

fn u32le(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

/// A RIFF/WAVE file's chunks, as `(id, body)`.
pub fn chunks(b: &[u8]) -> Option<Vec<([u8; 4], &[u8])>> {
    if b.get(0..4)? != b"RIFF" || b.get(8..12)? != b"WAVE" {
        return None;
    }
    let mut out = Vec::new();
    let mut at = 12;
    while at + 8 <= b.len() {
        let id: [u8; 4] = b[at..at + 4].try_into().ok()?;
        let len = u32le(b, at + 4)? as usize;
        let body = &b[at + 8..(at + 8 + len).min(b.len())];
        out.push((id, body));
        at += 8 + len + (len & 1);
    }
    Some(out)
}

/// The samples per channel a WAV holds, from its `fmt ` and `data` chunks.
fn sample_count(ch: &[([u8; 4], &[u8])]) -> Option<usize> {
    let fmt = ch.iter().find(|c| &c.0 == b"fmt ")?.1;
    let channels = u16le(fmt, 2)? as usize;
    let bits = u16le(fmt, 14)? as usize;
    let data = ch.iter().find(|c| &c.0 == b"data")?.1;
    Some(data.len() / (channels.max(1) * (bits / 8).max(1)))
}

/// The frame size a WAV's chunks declare, and which chunk said so.
pub fn declared_frame(b: &[u8]) -> Option<(usize, &'static str)> {
    let ch = chunks(b)?;
    for (id, body) in &ch {
        match id {
            b"clm " => {
                let text = String::from_utf8_lossy(body);
                let n: String = text
                    .trim_start()
                    .trim_start_matches("<!>")
                    .chars()
                    .take_while(|c| c.is_ascii_digit())
                    .collect();
                if let Ok(n) = n.parse::<usize>() {
                    if n >= 16 {
                        return Some((n, "clm"));
                    }
                }
            }
            b"uhWT" => return Some((2048, "uhwt")),
            b"srge" => {
                let n = u32le(body, 4)? as usize;
                if n >= 16 {
                    return Some((n, "srge"));
                }
            }
            _ => {}
        }
    }
    for (id, body) in &ch {
        match id {
            b"smpl" => {
                // 36 bytes of header, then loops of 24 bytes: id, type, start, end, …
                let loops = u32le(body, 28).unwrap_or(0);
                if loops == 0 {
                    return Some((2048, "smpl"));
                }
                let (start, end) = (u32le(body, 36 + 8)?, u32le(body, 36 + 12)?);
                if end > start {
                    return Some(((end - start + 1) as usize, "smpl"));
                }
            }
            b"cue " => {
                let n = u32le(body, 0).unwrap_or(0) as usize;
                if n >= 2 {
                    // Points of 24 bytes; the sample offset is the last field.
                    let mut offs: Vec<u32> = (0..n)
                        .filter_map(|i| u32le(body, 4 + 24 * i + 20))
                        .collect();
                    offs.sort_unstable();
                    let gap = offs.windows(2).map(|w| w[1] - w[0]).find(|g| *g > 0)?;
                    return Some((gap as usize, "cue"));
                }
            }
            _ => {}
        }
    }
    None
}

/// Look at a file chosen for oscillator import: a WAV is used as it is; a Surge `.wt` is
/// converted to a WAV in `convert_dir`.
pub fn inspect(path: &Path, convert_dir: &Path) -> Result<Found, ImportError> {
    let bytes = std::fs::read(path).map_err(|_| ImportError::Unreadable)?;
    if bytes.get(0..4) == Some(b"vawt") {
        return convert_wt(path, &bytes, convert_dir);
    }
    let ch = chunks(&bytes).ok_or(ImportError::NotWav)?;
    let len = sample_count(&ch).ok_or(ImportError::NotWav)?;
    if len == 0 {
        return Err(ImportError::Empty);
    }
    let undeclared = if len % 2048 == 0 {
        (2048, "multiple")
    } else if len <= 4096 {
        (len, "single")
    } else {
        (2048, "assumed")
    };
    let (frame, source) = declared_frame(&bytes).unwrap_or(undeclared);
    Ok(Found {
        path: path.to_path_buf(),
        frame,
        frames: (len / frame).max(1),
        source,
    })
}

/// Surge's `.wt` (and Bitwig's, the same container): `vawt`, samples per frame (u32),
/// frames (u16), flags (u16), then the frames — float32, or int16 at full range (flag
/// 0x08) or with a peak of 2¹⁴.
fn convert_wt(path: &Path, b: &[u8], dir: &Path) -> Result<Found, ImportError> {
    let size = u32le(b, 4).ok_or(ImportError::BadWt)? as usize;
    let count = u16le(b, 8).ok_or(ImportError::BadWt)? as usize;
    let flags = u16le(b, 10).ok_or(ImportError::BadWt)?;
    if !(2..=4096).contains(&size) || count == 0 || count > 512 || flags & 0x01 != 0 {
        return Err(ImportError::BadWt);
    }
    let n = size * count;
    let samples: Vec<f32> = if flags & 0x04 != 0 {
        let scale = if flags & 0x08 != 0 {
            1.0 / 32_768.0
        } else {
            1.0 / 16_384.0
        };
        (0..n)
            .map(|i| u16le(b, 12 + 2 * i).map(|v| v as i16 as f32 * scale))
            .collect::<Option<_>>()
            .ok_or(ImportError::BadWt)?
    } else {
        (0..n)
            .map(|i| u32le(b, 12 + 4 * i).map(f32::from_bits))
            .collect::<Option<_>>()
            .ok_or(ImportError::BadWt)?
    };
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    std::fs::create_dir_all(dir).map_err(|_| ImportError::Unreadable)?;
    let out = dir.join(format!("{stem}.wav"));
    std::fs::write(&out, table_wav(&samples, size)).map_err(|_| ImportError::Unreadable)?;
    Ok(Found {
        path: out,
        frame: size,
        frames: count,
        source: "wt",
    })
}

/// A mono 32-bit float WAV of a table, with a `clm ` chunk declaring its frame size (so
/// Serum, Vital, Surge and Unda itself read it as a table).
pub fn table_wav(samples: &[f32], frame: usize) -> Vec<u8> {
    let clm = format!("<!>{frame} 10000000 wavetable Cantus");
    let mut clm = clm.into_bytes();
    if clm.len() % 2 == 1 {
        clm.push(0);
    }
    let data_len = samples.len() * 4;
    let mut b = Vec::with_capacity(44 + clm.len() + 8 + data_len);
    let riff_len = 4 + (8 + 16) + (8 + clm.len()) + (8 + data_len);
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(riff_len as u32).to_le_bytes());
    b.extend_from_slice(b"WAVE");
    b.extend_from_slice(b"fmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&3u16.to_le_bytes()); // IEEE float
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&48_000u32.to_le_bytes());
    b.extend_from_slice(&(48_000u32 * 4).to_le_bytes());
    b.extend_from_slice(&4u16.to_le_bytes());
    b.extend_from_slice(&32u16.to_le_bytes());
    b.extend_from_slice(b"clm ");
    b.extend_from_slice(&(clm.len() as u32).to_le_bytes());
    b.extend_from_slice(&clm);
    b.extend_from_slice(b"data");
    b.extend_from_slice(&(data_len as u32).to_le_bytes());
    for s in samples {
        b.extend_from_slice(&s.to_le_bytes());
    }
    b
}

//! The ZIP container a `.dawproject` is, as far as the format needs it: entries stored or
//! deflated, read through the central directory (so a writer that streams with data
//! descriptors, as Java's does, reads the same), CRC-checked. No ZIP64, no encryption,
//! no multi-disk archives — a project is a few files, and the report says when one is
//! refused.

use std::io::{Read, Write};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ZipError {
    #[error("not a ZIP archive")]
    NotZip,
    #[error("the archive is cut short")]
    Truncated,
    #[error("{0}: compression method {1} is not supported")]
    Method(String, u16),
    #[error("{0}: the data does not match its checksum")]
    Crc(String),
    #[error("{0}: the data could not be inflated")]
    Inflate(String),
    #[error("ZIP64 archives are not supported")]
    Zip64,
}

/// CRC-32 (IEEE 802.3, reflected), the one ZIP uses.
pub fn crc32(data: &[u8]) -> u32 {
    static TABLE: std::sync::OnceLock<[u32; 256]> = std::sync::OnceLock::new();
    let table = TABLE.get_or_init(|| {
        let mut t = [0u32; 256];
        for (i, e) in t.iter_mut().enumerate() {
            let mut c = i as u32;
            for _ in 0..8 {
                c = if c & 1 != 0 {
                    0xEDB8_8320 ^ (c >> 1)
                } else {
                    c >> 1
                };
            }
            *e = c;
        }
        t
    });
    let mut c = 0xFFFF_FFFFu32;
    for b in data {
        c = table[((c ^ *b as u32) & 0xFF) as usize] ^ (c >> 8);
    }
    !c
}

struct Central {
    name: String,
    method: u16,
    crc: u32,
    compressed: u32,
    size: u32,
    offset: u32,
}

/// Builds an archive in memory.
#[derive(Default)]
pub struct ZipWriter {
    out: Vec<u8>,
    central: Vec<Central>,
}

/// 1980-01-01 00:00 in MS-DOS form: a fixed date, so the same project always makes the
/// same bytes.
const DOS_DATE: u16 = (1 << 5) | 1;
const DOS_TIME: u16 = 0;

impl ZipWriter {
    pub fn new() -> Self {
        ZipWriter::default()
    }

    /// Add a file; `deflate` compresses it (text), otherwise it is stored (audio that is
    /// already as small as it gets).
    pub fn add(&mut self, name: &str, data: &[u8], deflate: bool) {
        let crc = crc32(data);
        let (method, body) = if deflate {
            let mut enc =
                flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
            // Writing to a Vec cannot fail.
            let _ = enc.write_all(data);
            (8u16, enc.finish().unwrap_or_default())
        } else {
            (0u16, data.to_vec())
        };
        let offset = self.out.len() as u32;
        let o = &mut self.out;
        o.extend_from_slice(&0x0403_4B50u32.to_le_bytes());
        o.extend_from_slice(&20u16.to_le_bytes()); // version needed: 2.0
        o.extend_from_slice(&(1u16 << 11).to_le_bytes()); // names are UTF-8
        o.extend_from_slice(&method.to_le_bytes());
        o.extend_from_slice(&DOS_TIME.to_le_bytes());
        o.extend_from_slice(&DOS_DATE.to_le_bytes());
        o.extend_from_slice(&crc.to_le_bytes());
        o.extend_from_slice(&(body.len() as u32).to_le_bytes());
        o.extend_from_slice(&(data.len() as u32).to_le_bytes());
        o.extend_from_slice(&(name.len() as u16).to_le_bytes());
        o.extend_from_slice(&0u16.to_le_bytes());
        o.extend_from_slice(name.as_bytes());
        o.extend_from_slice(&body);
        self.central.push(Central {
            name: name.to_string(),
            method,
            crc,
            compressed: body.len() as u32,
            size: data.len() as u32,
            offset,
        });
    }

    pub fn finish(mut self) -> Vec<u8> {
        let start = self.out.len() as u32;
        for c in &self.central {
            let o = &mut self.out;
            o.extend_from_slice(&0x0201_4B50u32.to_le_bytes());
            o.extend_from_slice(&20u16.to_le_bytes()); // made by
            o.extend_from_slice(&20u16.to_le_bytes()); // needed
            o.extend_from_slice(&(1u16 << 11).to_le_bytes());
            o.extend_from_slice(&c.method.to_le_bytes());
            o.extend_from_slice(&DOS_TIME.to_le_bytes());
            o.extend_from_slice(&DOS_DATE.to_le_bytes());
            o.extend_from_slice(&c.crc.to_le_bytes());
            o.extend_from_slice(&c.compressed.to_le_bytes());
            o.extend_from_slice(&c.size.to_le_bytes());
            o.extend_from_slice(&(c.name.len() as u16).to_le_bytes());
            o.extend_from_slice(&[0; 8]); // extra, comment, disk, internal attributes
            o.extend_from_slice(&0u32.to_le_bytes()); // external attributes
            o.extend_from_slice(&c.offset.to_le_bytes());
            o.extend_from_slice(c.name.as_bytes());
        }
        let size = self.out.len() as u32 - start;
        let n = self.central.len() as u16;
        let o = &mut self.out;
        o.extend_from_slice(&0x0605_4B50u32.to_le_bytes());
        o.extend_from_slice(&[0; 4]); // this disk, the directory's disk
        o.extend_from_slice(&n.to_le_bytes());
        o.extend_from_slice(&n.to_le_bytes());
        o.extend_from_slice(&size.to_le_bytes());
        o.extend_from_slice(&start.to_le_bytes());
        o.extend_from_slice(&0u16.to_le_bytes()); // comment
        self.out
    }
}

fn u16_at(b: &[u8], at: usize) -> Result<u16, ZipError> {
    b.get(at..at + 2)
        .map(|s| u16::from_le_bytes([s[0], s[1]]))
        .ok_or(ZipError::Truncated)
}

fn u32_at(b: &[u8], at: usize) -> Result<u32, ZipError> {
    b.get(at..at + 4)
        .map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
        .ok_or(ZipError::Truncated)
}

/// Every file in an archive, by name, in the order the directory lists them.
pub fn read(bytes: &[u8]) -> Result<Vec<(String, Vec<u8>)>, ZipError> {
    if bytes.len() < 22 || u32_at(bytes, 0)? != 0x0403_4B50 {
        return Err(ZipError::NotZip);
    }
    // The end-of-directory record, searched from the end past a comment of up to 64 KiB.
    let min = bytes.len().saturating_sub(22 + 0xFFFF);
    let eocd = (min..=bytes.len() - 22)
        .rev()
        .find(|&i| u32_at(bytes, i) == Ok(0x0605_4B50))
        .ok_or(ZipError::NotZip)?;
    let count = u16_at(bytes, eocd + 10)? as usize;
    let dir = u32_at(bytes, eocd + 16)?;
    if dir == 0xFFFF_FFFF || count == 0xFFFF {
        return Err(ZipError::Zip64);
    }
    let mut at = dir as usize;
    let mut files = Vec::with_capacity(count);
    for _ in 0..count {
        if u32_at(bytes, at)? != 0x0201_4B50 {
            return Err(ZipError::Truncated);
        }
        let method = u16_at(bytes, at + 10)?;
        let crc = u32_at(bytes, at + 16)?;
        let compressed = u32_at(bytes, at + 20)? as usize;
        let size = u32_at(bytes, at + 24)? as usize;
        let name_len = u16_at(bytes, at + 28)? as usize;
        let extra_len = u16_at(bytes, at + 30)? as usize;
        let comment_len = u16_at(bytes, at + 32)? as usize;
        let local = u32_at(bytes, at + 42)? as usize;
        let name_bytes = bytes
            .get(at + 46..at + 46 + name_len)
            .ok_or(ZipError::Truncated)?;
        let name = String::from_utf8_lossy(name_bytes).into_owned();
        at += 46 + name_len + extra_len + comment_len;
        if name.ends_with('/') {
            continue; // a directory
        }
        // The local header's own name and extra lengths say where the data starts.
        if u32_at(bytes, local)? != 0x0403_4B50 {
            return Err(ZipError::Truncated);
        }
        let data_at =
            local + 30 + u16_at(bytes, local + 26)? as usize + u16_at(bytes, local + 28)? as usize;
        let raw = bytes
            .get(data_at..data_at + compressed)
            .ok_or(ZipError::Truncated)?;
        let data = match method {
            0 => raw.to_vec(),
            8 => {
                let mut out = Vec::with_capacity(size);
                flate2::read::DeflateDecoder::new(raw)
                    .read_to_end(&mut out)
                    .map_err(|_| ZipError::Inflate(name.clone()))?;
                out
            }
            m => return Err(ZipError::Method(name, m)),
        };
        if crc32(&data) != crc || data.len() != size {
            return Err(ZipError::Crc(name));
        }
        files.push((name, data));
    }
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc_matches_the_check_value() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        assert_eq!(crc32(b""), 0);
    }

    #[test]
    fn what_is_written_reads_back_stored_or_deflated() {
        let mut w = ZipWriter::new();
        let text = "<Project version=\"1.0\"/>".repeat(50);
        w.add("project.xml", text.as_bytes(), true);
        w.add("audio/é.bin", &[0, 1, 2, 3, 255], false);
        let bytes = w.finish();
        let files = read(&bytes).unwrap();
        assert_eq!(files.len(), 2);
        assert_eq!(files[0], ("project.xml".to_string(), text.into_bytes()));
        assert_eq!(files[1].0, "audio/é.bin");
        assert_eq!(files[1].1, vec![0, 1, 2, 3, 255]);
    }

    #[test]
    fn a_damaged_archive_is_refused_not_misread() {
        let mut w = ZipWriter::new();
        w.add("a.txt", b"hello hello hello", false);
        let mut bytes = w.finish();
        assert_eq!(read(b"not a zip at all, not at all"), Err(ZipError::NotZip));
        // Flip a byte of the stored data: the checksum catches it.
        let at = bytes.windows(5).position(|w| w == b"hello").unwrap();
        bytes[at] = b'j';
        assert_eq!(read(&bytes), Err(ZipError::Crc("a.txt".into())));
    }
}

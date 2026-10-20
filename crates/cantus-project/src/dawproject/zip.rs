//! The ZIP container a `.dawproject` is, as far as the format needs it: entries stored or
//! deflated, read through the central directory (so a writer that streams with data
//! descriptors, as Java's does, reads the same), CRC-checked; ZIP64 records where a size,
//! an offset or the directory passes 4 GiB (a project's audio can). No encryption, no
//! multi-disk archives — and the report says when one is refused.

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
    compressed: u64,
    size: u64,
    offset: u64,
}

/// Writes an archive to any byte sink, one entry at a time — never the whole archive in
/// memory — with ZIP64 records for an entry, an offset or a directory past 4 GiB (round
/// 29's review: a project's carried audio can be that large; the 32-bit fields would have
/// wrapped and corrupted it).
pub struct ZipStream<W: Write> {
    out: W,
    at: u64,
    central: Vec<Central>,
    /// The largest value a 32-bit field holds here (`u32::MAX`; lower in tests, so ZIP64
    /// is exercised without writing gigabytes).
    limit: u64,
}

/// Builds an archive in memory (small archives, tests).
pub struct ZipWriter(ZipStream<Vec<u8>>);

impl Default for ZipWriter {
    fn default() -> Self {
        ZipWriter(ZipStream::new(Vec::new()))
    }
}

/// 1980-01-01 00:00 in MS-DOS form: a fixed date, so the same project always makes the
/// same bytes.
const DOS_DATE: u16 = (1 << 5) | 1;
const DOS_TIME: u16 = 0;
/// The 32-bit fields' "see the ZIP64 record" value.
const FULL32: u32 = 0xFFFF_FFFF;

impl ZipWriter {
    pub fn new() -> Self {
        ZipWriter::default()
    }

    /// Add a file; `deflate` compresses it (text), otherwise it is stored (audio that is
    /// already as small as it gets).
    pub fn add(&mut self, name: &str, data: &[u8], deflate: bool) {
        // Writing to a Vec cannot fail.
        let _ = self.0.add(name, data, deflate);
    }

    pub fn finish(self) -> Vec<u8> {
        self.0.finish().unwrap_or_default()
    }
}

impl<W: Write> ZipStream<W> {
    pub fn new(out: W) -> Self {
        ZipStream {
            out,
            at: 0,
            central: Vec::new(),
            limit: FULL32 as u64,
        }
    }

    /// The same writer with 32-bit fields treated as full beyond `limit` (tests).
    #[doc(hidden)]
    pub fn with_limit(out: W, limit: u64) -> Self {
        ZipStream {
            limit,
            ..ZipStream::new(out)
        }
    }

    fn put(&mut self, b: &[u8]) -> std::io::Result<()> {
        self.out.write_all(b)?;
        self.at += b.len() as u64;
        Ok(())
    }

    /// Add a file (see [`ZipWriter::add`]).
    pub fn add(&mut self, name: &str, data: &[u8], deflate: bool) -> std::io::Result<()> {
        let crc = crc32(data);
        let compressed;
        let (method, body): (u16, std::borrow::Cow<[u8]>) = if deflate {
            let mut enc =
                flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
            enc.write_all(data)?;
            compressed = enc.finish()?;
            (8, std::borrow::Cow::Owned(compressed))
        } else {
            (0, std::borrow::Cow::Borrowed(data))
        };
        let offset = self.at;
        let (size, packed) = (data.len() as u64, body.len() as u64);
        let wide = size >= self.limit || packed >= self.limit;
        let mut h = Vec::with_capacity(64 + name.len());
        h.extend_from_slice(&0x0403_4B50u32.to_le_bytes());
        h.extend_from_slice(&(if wide { 45u16 } else { 20 }).to_le_bytes());
        h.extend_from_slice(&(1u16 << 11).to_le_bytes()); // names are UTF-8
        h.extend_from_slice(&method.to_le_bytes());
        h.extend_from_slice(&DOS_TIME.to_le_bytes());
        h.extend_from_slice(&DOS_DATE.to_le_bytes());
        h.extend_from_slice(&crc.to_le_bytes());
        if wide {
            h.extend_from_slice(&FULL32.to_le_bytes());
            h.extend_from_slice(&FULL32.to_le_bytes());
        } else {
            h.extend_from_slice(&(packed as u32).to_le_bytes());
            h.extend_from_slice(&(size as u32).to_le_bytes());
        }
        h.extend_from_slice(&(name.len() as u16).to_le_bytes());
        h.extend_from_slice(&(if wide { 20u16 } else { 0 }).to_le_bytes());
        h.extend_from_slice(name.as_bytes());
        if wide {
            h.extend_from_slice(&1u16.to_le_bytes());
            h.extend_from_slice(&16u16.to_le_bytes());
            h.extend_from_slice(&size.to_le_bytes());
            h.extend_from_slice(&packed.to_le_bytes());
        }
        self.put(&h)?;
        self.put(&body)?;
        self.central.push(Central {
            name: name.to_string(),
            method,
            crc,
            compressed: packed,
            size,
            offset,
        });
        Ok(())
    }

    /// Write the directory and the end records; the sink, done.
    pub fn finish(mut self) -> std::io::Result<W> {
        let start = self.at;
        let entries = std::mem::take(&mut self.central);
        for c in &entries {
            // The ZIP64 extra holds, in this order, whichever field is full.
            let mut extra = Vec::new();
            let field = |v: u64, extra: &mut Vec<u8>| -> u32 {
                if v >= self.limit {
                    extra.extend_from_slice(&v.to_le_bytes());
                    FULL32
                } else {
                    v as u32
                }
            };
            let size = field(c.size, &mut extra);
            let packed = field(c.compressed, &mut extra);
            let offset = field(c.offset, &mut extra);
            let wide = !extra.is_empty();
            let mut h = Vec::with_capacity(64 + c.name.len());
            h.extend_from_slice(&0x0201_4B50u32.to_le_bytes());
            let v = if wide { 45u16 } else { 20 };
            h.extend_from_slice(&v.to_le_bytes()); // made by
            h.extend_from_slice(&v.to_le_bytes()); // needed
            h.extend_from_slice(&(1u16 << 11).to_le_bytes());
            h.extend_from_slice(&c.method.to_le_bytes());
            h.extend_from_slice(&DOS_TIME.to_le_bytes());
            h.extend_from_slice(&DOS_DATE.to_le_bytes());
            h.extend_from_slice(&c.crc.to_le_bytes());
            h.extend_from_slice(&packed.to_le_bytes());
            h.extend_from_slice(&size.to_le_bytes());
            h.extend_from_slice(&(c.name.len() as u16).to_le_bytes());
            let extra_len = if wide { 4 + extra.len() } else { 0 };
            h.extend_from_slice(&(extra_len as u16).to_le_bytes());
            h.extend_from_slice(&[0; 6]); // comment, disk, internal attributes
            h.extend_from_slice(&0u32.to_le_bytes()); // external attributes
            h.extend_from_slice(&offset.to_le_bytes());
            h.extend_from_slice(c.name.as_bytes());
            if wide {
                h.extend_from_slice(&1u16.to_le_bytes());
                h.extend_from_slice(&(extra.len() as u16).to_le_bytes());
                h.extend_from_slice(&extra);
            }
            self.put(&h)?;
        }
        let size = self.at - start;
        let n = entries.len() as u64;
        let wide = start >= self.limit || size >= self.limit || n >= 0xFFFF;
        let mut e = Vec::with_capacity(98);
        if wide {
            let record = self.at;
            e.extend_from_slice(&0x0606_4B50u32.to_le_bytes());
            e.extend_from_slice(&44u64.to_le_bytes());
            e.extend_from_slice(&45u16.to_le_bytes());
            e.extend_from_slice(&45u16.to_le_bytes());
            e.extend_from_slice(&[0; 8]); // this disk, the directory's disk
            e.extend_from_slice(&n.to_le_bytes());
            e.extend_from_slice(&n.to_le_bytes());
            e.extend_from_slice(&size.to_le_bytes());
            e.extend_from_slice(&start.to_le_bytes());
            e.extend_from_slice(&0x0706_4B50u32.to_le_bytes());
            e.extend_from_slice(&0u32.to_le_bytes());
            e.extend_from_slice(&record.to_le_bytes());
            e.extend_from_slice(&1u32.to_le_bytes());
        }
        let n16 = if wide { 0xFFFF } else { n as u16 };
        e.extend_from_slice(&0x0605_4B50u32.to_le_bytes());
        e.extend_from_slice(&[0; 4]); // this disk, the directory's disk
        e.extend_from_slice(&n16.to_le_bytes());
        e.extend_from_slice(&n16.to_le_bytes());
        let (s32, o32) = if wide {
            (FULL32, FULL32)
        } else {
            (size as u32, start as u32)
        };
        e.extend_from_slice(&s32.to_le_bytes());
        e.extend_from_slice(&o32.to_le_bytes());
        e.extend_from_slice(&0u16.to_le_bytes()); // comment
        self.put(&e)?;
        self.out.flush()?;
        Ok(self.out)
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

fn u64_at(b: &[u8], at: usize) -> Result<u64, ZipError> {
    b.get(at..at + 8)
        .map(|s| {
            let mut a = [0u8; 8];
            a.copy_from_slice(s);
            u64::from_le_bytes(a)
        })
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
    let mut count = u16_at(bytes, eocd + 10)? as usize;
    let mut dir = u32_at(bytes, eocd + 16)? as u64;
    if dir == FULL32 as u64 || count == 0xFFFF {
        // The ZIP64 end record, through its locator just before the end record.
        let loc = eocd.checked_sub(20).ok_or(ZipError::Truncated)?;
        if u32_at(bytes, loc)? != 0x0706_4B50 {
            return Err(ZipError::Zip64);
        }
        let record = u64_at(bytes, loc + 8)? as usize;
        if u32_at(bytes, record)? != 0x0606_4B50 {
            return Err(ZipError::Zip64);
        }
        count = u64_at(bytes, record + 32)? as usize;
        dir = u64_at(bytes, record + 48)?;
    }
    let mut at = dir as usize;
    let mut files = Vec::with_capacity(count);
    for _ in 0..count {
        if u32_at(bytes, at)? != 0x0201_4B50 {
            return Err(ZipError::Truncated);
        }
        let method = u16_at(bytes, at + 10)?;
        let crc = u32_at(bytes, at + 16)?;
        let mut compressed = u32_at(bytes, at + 20)? as u64;
        let mut size = u32_at(bytes, at + 24)? as u64;
        let name_len = u16_at(bytes, at + 28)? as usize;
        let extra_len = u16_at(bytes, at + 30)? as usize;
        let comment_len = u16_at(bytes, at + 32)? as usize;
        let mut local = u32_at(bytes, at + 42)? as u64;
        let name_bytes = bytes
            .get(at + 46..at + 46 + name_len)
            .ok_or(ZipError::Truncated)?;
        let name = String::from_utf8_lossy(name_bytes).into_owned();
        // A ZIP64 extra holds, in order, the fields that read as full.
        let mut e = at + 46 + name_len;
        let end = e + extra_len;
        while e + 4 <= end {
            let (id, len) = (u16_at(bytes, e)?, u16_at(bytes, e + 2)? as usize);
            if id == 1 {
                let mut v = e + 4;
                for f in [&mut size, &mut compressed, &mut local] {
                    if *f == FULL32 as u64 {
                        *f = u64_at(bytes, v)?;
                        v += 8;
                    }
                }
            }
            e += 4 + len;
        }
        let (compressed, size, local) = (compressed as usize, size as usize, local as usize);
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

    /// Round 29's review: ZIP64 records where a size, an offset or the directory is past
    /// what 32 bits hold — written and read back (the threshold lowered, so it runs on
    /// bytes, not gigabytes).
    #[test]
    fn zip64_records_are_written_and_read_back() {
        let mut z = ZipStream::with_limit(Vec::new(), 100);
        let big: Vec<u8> = (0..1_000u32).map(|i| (i * 7) as u8).collect();
        z.add("project.xml", b"<Project/>", true).unwrap();
        z.add("audio/a.wav", &big, false).unwrap();
        z.add("audio/b.wav", &big[..50], false).unwrap();
        let bytes = z.finish().unwrap();
        // The end records say ZIP64.
        assert!(bytes.windows(4).any(|w| w == 0x0606_4B50u32.to_le_bytes()));
        let files = read(&bytes).unwrap();
        assert_eq!(files.len(), 3);
        assert_eq!(files[1], ("audio/a.wav".to_string(), big.clone()));
        assert_eq!(files[2].1, big[..50].to_vec());
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

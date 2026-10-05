//! Peak files on disk: `<cache>/<key>.peaks`, little-endian, the pairs as `i16`.
//!
//! The key is FNV-1a over the file's path, size and modification time — stable across
//! builds and platforms (std's hasher is not), and changed by any edit to the file.

use super::super::{Level, SourcePeaks, LODS};
use std::io::Write;
use std::path::Path;

const MAGIC: &[u8; 4] = b"CPK1";

fn fnv(h: &mut u64, bytes: &[u8]) {
    for b in bytes {
        *h ^= *b as u64;
        *h = h.wrapping_mul(0x0100_0000_01b3);
    }
}

/// The cache file's name for `path` as it is now, or `None` when it cannot be read.
pub fn key(path: &Path) -> Option<String> {
    let meta = std::fs::metadata(path).ok()?;
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    fnv(&mut h, path.to_string_lossy().as_bytes());
    fnv(&mut h, &meta.len().to_le_bytes());
    fnv(&mut h, &mtime.to_le_bytes());
    Some(format!("{h:016x}.peaks"))
}

fn q(v: f32) -> i16 {
    (v.clamp(-1.0, 1.0) * 32767.0).round() as i16
}

fn dq(v: i16) -> f32 {
    v as f32 / 32767.0
}

/// Write `p` to `file` (through a temporary next to it, so a reader never sees half).
pub fn write(file: &Path, p: &SourcePeaks) -> std::io::Result<()> {
    let mut out = Vec::new();
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&p.rate.to_le_bytes());
    out.push(p.channels);
    out.extend_from_slice(&p.frames.to_le_bytes());
    out.extend_from_slice(&(p.levels.len() as u32).to_le_bytes());
    for l in &p.levels {
        out.extend_from_slice(&(l.frames_per_peak as u32).to_le_bytes());
        out.extend_from_slice(&(l.mono.len() as u32).to_le_bytes());
        for list in l.channels.iter().chain(std::iter::once(&l.mono)) {
            for &(lo, hi) in list {
                out.extend_from_slice(&q(lo).to_le_bytes());
                out.extend_from_slice(&q(hi).to_le_bytes());
            }
        }
    }
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = file.with_extension("peaks.tmp");
    {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(&out)?;
    }
    std::fs::rename(&tmp, file)
}

struct Cursor<'a> {
    b: &'a [u8],
    at: usize,
}

impl Cursor<'_> {
    fn take(&mut self, n: usize) -> Option<&[u8]> {
        let s = self.b.get(self.at..self.at + n)?;
        self.at += n;
        Some(s)
    }
    fn u32(&mut self) -> Option<u32> {
        Some(u32::from_le_bytes(self.take(4)?.try_into().ok()?))
    }
    fn u64(&mut self) -> Option<u64> {
        Some(u64::from_le_bytes(self.take(8)?.try_into().ok()?))
    }
    fn i16(&mut self) -> Option<i16> {
        Some(i16::from_le_bytes(self.take(2)?.try_into().ok()?))
    }
}

/// Read a peak file; `None` when it is absent, from another version, or damaged.
pub fn read(file: &Path) -> Option<SourcePeaks> {
    let bytes = std::fs::read(file).ok()?;
    let mut c = Cursor { b: &bytes, at: 0 };
    if c.take(4)? != MAGIC {
        return None;
    }
    let rate = c.u32()?;
    let channels = *c.take(1)?.first()?;
    let frames = c.u64()?;
    let n = c.u32()? as usize;
    if n != LODS.len() || !(1..=2).contains(&channels) {
        return None;
    }
    let mut levels = Vec::with_capacity(n);
    for _ in 0..n {
        let fpp = c.u32()? as usize;
        let count = c.u32()? as usize;
        if count > bytes.len() {
            return None;
        }
        let mut lists = Vec::with_capacity(channels as usize + 1);
        for _ in 0..=channels {
            let mut list = Vec::with_capacity(count);
            for _ in 0..count {
                list.push((dq(c.i16()?), dq(c.i16()?)));
            }
            lists.push(list);
        }
        let mono = lists.pop()?;
        levels.push(Level {
            frames_per_peak: fpp,
            channels: lists,
            mono,
        });
    }
    Some(SourcePeaks {
        rate,
        channels,
        frames,
        levels,
    })
}

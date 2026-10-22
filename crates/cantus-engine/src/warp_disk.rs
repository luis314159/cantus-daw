//! Warp analyses on disk: `<cache>/<key>.onsets`, little-endian, magic `CWA1`. The key is
//! FNV-1a (stable across builds and platforms) over the file's **name**, size and
//! modification time, the rate and [`super::ANALYSER`] — as the pitch cache does
//! (ADR 0054): an edited file or a better analyser is analysed again, a reopened project
//! finds its files at once, and a file moved into the project's folder on its first save
//! (a rename keeps its time) is still found.

use super::Analysis;
use cantus_dsp::beat::Grid;
use std::io::Write;
use std::path::{Path, PathBuf};

const MAGIC: &[u8; 4] = b"CWA1";

fn fnv(h: &mut u64, bytes: &[u8]) {
    for b in bytes {
        *h ^= *b as u64;
        *h = h.wrapping_mul(0x0100_0000_01b3);
    }
}

/// The cache file for `path` at `rate` as the file is now, or `None` when it cannot be read.
pub fn file_for(dir: &Path, path: &Path, rate: u32) -> Option<PathBuf> {
    let meta = std::fs::metadata(path).ok()?;
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy())
        .unwrap_or_default();
    fnv(&mut h, name.as_bytes());
    fnv(&mut h, &meta.len().to_le_bytes());
    fnv(&mut h, &mtime.to_le_bytes());
    fnv(&mut h, &rate.to_le_bytes());
    fnv(&mut h, &super::ANALYSER.to_le_bytes());
    Some(dir.join(format!("{h:016x}.onsets")))
}

/// Write an analysis (through a temporary file, so a reader never sees half).
pub fn write(file: &Path, a: &Analysis) -> std::io::Result<()> {
    let g = &a.grid;
    let mut out = Vec::with_capacity(64 + a.onsets.len() * 12 + g.beats.len() * 8);
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&a.rate.to_le_bytes());
    out.extend_from_slice(&a.frames.to_le_bytes());
    out.extend_from_slice(&a.acid_bpm.unwrap_or(f64::NAN).to_le_bytes());
    out.extend_from_slice(&g.bpm.to_le_bytes());
    out.extend_from_slice(&g.confidence.to_le_bytes());
    out.extend_from_slice(&(g.downbeat as u32).to_le_bytes());
    out.extend_from_slice(&g.bars.unwrap_or(0).to_le_bytes());
    out.extend_from_slice(&(g.beats.len() as u32).to_le_bytes());
    for b in &g.beats {
        out.extend_from_slice(&b.to_le_bytes());
    }
    out.extend_from_slice(&(a.onsets.len() as u32).to_le_bytes());
    for (o, s) in a.onsets.iter().zip(&a.strengths) {
        out.extend_from_slice(&o.to_le_bytes());
        out.extend_from_slice(&s.to_le_bytes());
    }
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = file.with_extension("onsets.tmp");
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
    fn take<const N: usize>(&mut self) -> Option<[u8; N]> {
        let s = self.b.get(self.at..self.at + N)?;
        self.at += N;
        s.try_into().ok()
    }
    fn u32(&mut self) -> Option<u32> {
        self.take::<4>().map(u32::from_le_bytes)
    }
    fn u64(&mut self) -> Option<u64> {
        self.take::<8>().map(u64::from_le_bytes)
    }
    fn f32(&mut self) -> Option<f32> {
        self.take::<4>().map(f32::from_le_bytes)
    }
    fn f64(&mut self) -> Option<f64> {
        self.take::<8>().map(f64::from_le_bytes)
    }
}

/// Read an analysis back; `None` for a file that is not one.
pub fn read(file: &Path) -> Option<Analysis> {
    let b = std::fs::read(file).ok()?;
    let mut c = Cursor { b: &b, at: 0 };
    if c.take::<4>()? != *MAGIC {
        return None;
    }
    let rate = c.u32()?;
    let frames = c.u64()?;
    let acid = c.f64()?;
    let bpm = c.f64()?;
    let confidence = c.f32()?;
    let downbeat = c.u32()? as usize;
    let bars = c.u32()?;
    let n = c.u32()? as usize;
    if n > b.len() / 8 + 1 {
        return None;
    }
    let mut beats = Vec::with_capacity(n);
    for _ in 0..n {
        beats.push(c.f64()?);
    }
    let m = c.u32()? as usize;
    if m > b.len() / 12 + 1 {
        return None;
    }
    let (mut onsets, mut strengths) = (Vec::with_capacity(m), Vec::with_capacity(m));
    for _ in 0..m {
        onsets.push(c.f64()?);
        strengths.push(c.f32()?);
    }
    (c.at == b.len()).then_some(Analysis {
        rate,
        frames,
        onsets,
        strengths,
        grid: Grid {
            bpm,
            confidence,
            beats,
            downbeat,
            bars: (bars > 0).then_some(bars),
        },
        acid_bpm: (!acid.is_nan()).then_some(acid),
    })
}

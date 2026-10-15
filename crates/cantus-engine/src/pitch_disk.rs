//! Analyses on disk: `<cache>/<key>.pitch`, little-endian. The key is FNV-1a (stable
//! across builds and platforms) over the file's **name**, size and modification time, the
//! rate and the analyser's version — so an edited file or a better analyser is analysed
//! again, a reopened project finds its takes at once, and a take that moved into the
//! project's folder on its first save (a rename keeps its time) is still found.
//!
//! Only the tracker's result is stored; the notes and the key are derived from it again on
//! reading (deterministic, milliseconds).

use cantus_dsp::pitch::{Analysis, Epoch, PitchFrame};
use std::io::Write;
use std::path::{Path, PathBuf};

const MAGIC: &[u8; 4] = b"CPT1";

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
    let name = path.file_name().map(|n| n.to_string_lossy()).unwrap_or_default();
    fnv(&mut h, name.as_bytes());
    fnv(&mut h, &meta.len().to_le_bytes());
    fnv(&mut h, &mtime.to_le_bytes());
    fnv(&mut h, &rate.to_le_bytes());
    fnv(&mut h, &super::ANALYSER.to_le_bytes());
    Some(dir.join(format!("{h:016x}.pitch")))
}

/// Write an analysis of a take `frames` long (through a temporary file, so a reader never
/// sees half).
pub fn write(file: &Path, a: &Analysis, frames: u64) -> std::io::Result<()> {
    let mut out = Vec::with_capacity(16 + a.frames.len() * 37 + a.epochs.len() * 12);
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&a.sample_rate.to_le_bytes());
    out.extend_from_slice(&(a.hop as u32).to_le_bytes());
    out.extend_from_slice(&frames.to_le_bytes());
    out.extend_from_slice(&(a.frames.len() as u32).to_le_bytes());
    for f in &a.frames {
        out.extend_from_slice(&f.centre.to_le_bytes());
        for v in [
            f.f0,
            f.period,
            f.aperiodicity,
            f.level_db,
            f.sibilance,
            f.voiced_prob,
        ] {
            out.extend_from_slice(&v.to_le_bytes());
        }
        out.push(f.voiced as u8);
    }
    out.extend_from_slice(&(a.epochs.len() as u32).to_le_bytes());
    for e in &a.epochs {
        out.extend_from_slice(&e.at.to_le_bytes());
        out.extend_from_slice(&e.period.to_le_bytes());
    }
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = file.with_extension("pitch.tmp");
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
    fn f32(&mut self) -> Option<f32> {
        self.take::<4>().map(f32::from_le_bytes)
    }
    fn f64(&mut self) -> Option<f64> {
        self.take::<8>().map(f64::from_le_bytes)
    }
    fn u64(&mut self) -> Option<u64> {
        self.take::<8>().map(u64::from_le_bytes)
    }
}

/// Read an analysis back, with the take's length; `None` for a file that is not one.
pub fn read(file: &Path) -> Option<(Analysis, u64)> {
    let b = std::fs::read(file).ok()?;
    let mut c = Cursor { b: &b, at: 0 };
    if c.take::<4>()? != *MAGIC {
        return None;
    }
    let sample_rate = c.f32()?;
    let hop = c.u32()? as usize;
    let frames = c.u64()?;
    let n = c.u32()? as usize;
    if n > b.len() / 37 + 1 {
        return None;
    }
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        let centre = c.f64()?;
        let (f0, period, aperiodicity, level_db, sibilance, voiced_prob) =
            (c.f32()?, c.f32()?, c.f32()?, c.f32()?, c.f32()?, c.f32()?);
        let voiced = c.take::<1>()?[0] != 0;
        out.push(PitchFrame {
            centre,
            f0,
            period,
            voiced,
            aperiodicity,
            level_db,
            sibilance,
            voiced_prob,
        });
    }
    let m = c.u32()? as usize;
    if m > b.len() / 12 + 1 {
        return None;
    }
    let mut epochs = Vec::with_capacity(m);
    for _ in 0..m {
        epochs.push(Epoch {
            at: c.f64()?,
            period: c.f32()?,
        });
    }
    (c.at == b.len()).then_some((
        Analysis {
            sample_rate,
            hop,
            frames: out,
            epochs,
        },
        frames,
    ))
}

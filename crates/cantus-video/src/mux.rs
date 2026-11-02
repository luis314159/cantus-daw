//! What the MP4 and MOV writers share: the media data written as it comes, the index put in
//! front of it at the end ("fast start") without copying the file, and the sample tables.
//!
//! The file starts with a reserved region (zeros), then an 8-byte `wide` box and the `mdat`
//! header, then the samples as they arrive. At the end the header (`ftyp` + `moov`) is
//! written into the reserved region and a `free` box fills what is left of it; an `mdat`
//! past 4 GB takes the `wide` box's 8 bytes for its 64-bit size (as QuickTime and FFmpeg
//! do). Either way the samples start at `reserved + 16`, so their offsets are known while
//! they are written. The region is sized from the expected sample counts with room to
//! spare; if the index still outgrows it, the samples are moved up in place (from the end,
//! a block at a time) — slower, never wrong.

use crate::bmff;
use std::io::{Read, Seek, SeekFrom, Write};

/// One track's sample tables, as samples arrive.
#[derive(Debug, Default, Clone)]
pub struct TrackLog {
    /// Each sample's size (empty for a track of [`TrackLog::uniform`] samples).
    pub sizes: Vec<u32>,
    /// Every sample this size (PCM: a sample is one frame of every channel).
    pub uniform: Option<u32>,
    /// Samples written.
    pub count: u64,
    /// 1-based numbers of the sync samples (key frames).
    pub sync: Vec<u32>,
    /// Each chunk's offset from the start of the samples, and its sample count.
    pub chunks: Vec<(u64, u32)>,
}

impl TrackLog {
    pub fn samples(&self) -> u32 {
        self.count as u32
    }

    /// `stsc`, `stsz` and the chunk offsets (from the samples' `base` in the file).
    pub fn tables(&self, base: u64, wide: bool) -> Vec<u8> {
        let counts: Vec<u32> = self.chunks.iter().map(|c| c.1).collect();
        let offsets: Vec<u64> = self.chunks.iter().map(|c| base + c.0).collect();
        let uniform = match (self.uniform, self.sizes.first()) {
            (Some(u), _) => Some((u, self.samples())),
            (None, Some(&s)) if self.sizes.iter().all(|&x| x == s) => Some((s, self.samples())),
            _ => None,
        };
        [
            bmff::sample_to_chunk(&counts),
            bmff::sample_sizes(&self.sizes, uniform),
            bmff::chunk_offsets(&offsets, wide),
        ]
        .concat()
    }

    /// `stss`, or nothing when every sample is a sync sample.
    pub fn sync_table(&self) -> Vec<u8> {
        if self.sync.len() as u64 == self.count {
            return Vec::new();
        }
        let mut v = vec![self.sync.len() as u32];
        v.extend_from_slice(&self.sync);
        bmff::full(b"stss", 0, 0, &bmff::u32s(&v))
    }
}

/// Bytes of index a sample can need at most (`stsz` 4, a chunk of its own: `co64` 8 and
/// `stsc` 12, `stss` 4), and what a header needs besides its tables.
const PER_SAMPLE: u64 = 28;
const FIXED: u64 = 8_192;
/// The `wide` box and the `mdat` header between the region and the samples.
const GAP: u64 = 16;
/// Free boxes are at least their header.
const FREE_MIN: u64 = 8;

/// The media data of a file being written, with the region for its header in front.
pub struct Spool<W: Read + Write + Seek> {
    out: W,
    reserved: u64,
    written: u64,
    pub tracks: Vec<TrackLog>,
    last: Option<usize>,
}

impl<W: Read + Write + Seek> Spool<W> {
    /// Start a file with room for the index of about `samples` samples (all tracks).
    pub fn new(mut out: W, tracks: usize, samples: u64) -> std::io::Result<Spool<W>> {
        let reserved = FIXED + PER_SAMPLE * (samples + samples / 8 + 64);
        out.seek(SeekFrom::Start(0))?;
        write_zeros(&mut out, reserved + GAP)?;
        Ok(Spool {
            out,
            reserved,
            written: 0,
            tracks: vec![TrackLog::default(); tracks],
            last: None,
        })
    }

    /// Where the samples start in the file.
    pub fn base(&self) -> u64 {
        self.reserved + GAP
    }

    /// Bytes of samples written so far.
    pub fn written(&self) -> u64 {
        self.written
    }

    /// Append one sample of `track`; consecutive samples of a track share a chunk.
    pub fn sample(&mut self, track: usize, bytes: &[u8], sync: bool) -> std::io::Result<()> {
        self.out.write_all(bytes)?;
        let t = &mut self.tracks[track];
        t.sizes.push(bytes.len() as u32);
        t.count += 1;
        if sync {
            t.sync.push(t.count as u32);
        }
        self.chunk(track, 1);
        self.written += bytes.len() as u64;
        Ok(())
    }

    /// Append `bytes` holding samples of `size` bytes each (PCM) to `track`.
    pub fn uniform(&mut self, track: usize, bytes: &[u8], size: u32) -> std::io::Result<()> {
        if bytes.is_empty() {
            return Ok(());
        }
        self.out.write_all(bytes)?;
        let n = (bytes.len() / size as usize) as u32;
        let t = &mut self.tracks[track];
        t.uniform = Some(size);
        t.count += n as u64;
        self.chunk(track, n);
        self.written += bytes.len() as u64;
        Ok(())
    }

    /// Count `n` samples into the current chunk of `track`, or start one.
    fn chunk(&mut self, track: usize, n: u32) {
        let t = &mut self.tracks[track];
        match t.chunks.last_mut() {
            Some(c) if self.last == Some(track) => c.1 += n,
            _ => t.chunks.push((self.written, n)),
        }
        self.last = Some(track);
    }

    /// Write the header `header(base, wide)` in front (`wide`: offsets need 64 bits) and
    /// give the output back.
    pub fn finish(
        mut self,
        header: impl Fn(&[TrackLog], u64, bool) -> Vec<u8>,
    ) -> std::io::Result<W> {
        loop {
            let base = self.base();
            let wide = base + self.written > u32::MAX as u64;
            let head = header(&self.tracks, base, wide);
            let len = head.len() as u64;
            if len == self.reserved || len + FREE_MIN <= self.reserved {
                self.out.seek(SeekFrom::Start(0))?;
                self.out.write_all(&head)?;
                if len < self.reserved {
                    let free = self.reserved - len;
                    self.out.write_all(&(free as u32).to_be_bytes())?;
                    self.out.write_all(b"free")?;
                }
                self.out.seek(SeekFrom::Start(self.reserved))?;
                let mdat = self.written + 8;
                if mdat <= u32::MAX as u64 {
                    self.out.write_all(&8u32.to_be_bytes())?;
                    self.out.write_all(b"wide")?;
                    self.out.write_all(&(mdat as u32).to_be_bytes())?;
                    self.out.write_all(b"mdat")?;
                } else {
                    self.out.write_all(&1u32.to_be_bytes())?;
                    self.out.write_all(b"mdat")?;
                    self.out.write_all(&(self.written + 16).to_be_bytes())?;
                }
                self.out.flush()?;
                return Ok(self.out);
            }
            // The index outgrew its region: move the samples up and try again.
            let grow = len + FREE_MIN + 4_096 - self.reserved.min(len);
            let (base, written) = (self.base(), self.written);
            shift_up(&mut self.out, base, written, grow)?;
            self.reserved += grow;
        }
    }
}

fn write_zeros(out: &mut impl Write, mut n: u64) -> std::io::Result<()> {
    let block = [0u8; 8_192];
    while n > 0 {
        let k = n.min(block.len() as u64) as usize;
        out.write_all(&block[..k])?;
        n -= k as u64;
    }
    Ok(())
}

/// Move `len` bytes at `from` up by `by`, in place, from the end.
fn shift_up<W: Read + Write + Seek>(
    out: &mut W,
    from: u64,
    len: u64,
    by: u64,
) -> std::io::Result<()> {
    let mut buf = vec![0u8; 1 << 20];
    let mut end = from + len;
    while end > from {
        let n = (end - from).min(buf.len() as u64) as usize;
        let start = end - n as u64;
        out.seek(SeekFrom::Start(start))?;
        out.read_exact(&mut buf[..n])?;
        out.seek(SeekFrom::Start(start + by))?;
        out.write_all(&buf[..n])?;
        end = start;
    }
    out.seek(SeekFrom::Start(from + len + by))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn header_of(size: usize) -> impl Fn(&[TrackLog], u64, bool) -> Vec<u8> {
        move |tracks, base, _| {
            let mut h = bmff::bx(b"ftyp", b"isom");
            // Say where the first chunk is, so the test can follow it.
            h.extend(bmff::bx(
                b"base",
                &(base + tracks[0].chunks[0].0).to_be_bytes(),
            ));
            if size > h.len() + 8 {
                h.extend(bmff::bx(b"skip", &vec![0; size - h.len() - 8]));
            }
            h
        }
    }

    fn check(file: &[u8], payload: &[u8]) {
        let tops = bmff::parse(file);
        let kinds: Vec<&[u8; 4]> = tops.iter().map(|b| &b.kind).collect();
        assert_eq!(&kinds[..2], &[b"ftyp", b"base"]);
        let mdat = bmff::top(file, b"mdat").unwrap();
        assert_eq!(mdat.body, payload);
        let at = bmff::top(file, b"base").unwrap().u64_at(0) as usize;
        assert_eq!(&file[at..at + 3], &payload[..3]);
    }

    #[test]
    fn the_index_goes_in_front_of_the_samples() {
        let mut s = Spool::new(Cursor::new(Vec::new()), 2, 4).unwrap();
        s.sample(0, b"abc", true).unwrap();
        s.sample(0, b"de", false).unwrap();
        s.sample(1, b"f", true).unwrap();
        s.sample(0, b"gh", true).unwrap();
        assert_eq!(s.tracks[0].chunks, vec![(0, 2), (6, 1)]);
        assert_eq!(s.tracks[1].chunks, vec![(5, 1)]);
        assert_eq!(s.tracks[0].sync, vec![1, 3]);
        let file = s.finish(header_of(0)).unwrap().into_inner();
        check(&file, b"abcdefgh");
        assert!(bmff::top(&file, b"free").is_some());
        assert!(bmff::top(&file, b"wide").is_some());
    }

    #[test]
    fn an_index_larger_than_its_room_moves_the_samples_up() {
        let mut s = Spool::new(Cursor::new(Vec::new()), 1, 1).unwrap();
        let payload: Vec<u8> = (0..3_000_000u32).map(|i| (i % 251) as u8).collect();
        s.sample(0, &payload, true).unwrap();
        let big = s.base() as usize + 50_000;
        let file = s.finish(header_of(big)).unwrap().into_inner();
        check(&file, &payload);
    }
}

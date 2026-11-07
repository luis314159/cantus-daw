//! A WAV file's thumbnail read in pieces (round 37, finish; ADR 0103 §30, §38): the
//! format chunk, then the `data` chunk sampled column by column through a reader that
//! seeks — never the whole file in memory. PCM 8/16/24/32-bit, IEEE float 32/64-bit, and
//! `WAVE_FORMAT_EXTENSIBLE` (the tag 0xFFFE that hound and most recorders write past 16
//! bits) by its sub-format GUID's tag, as `cantus-content`'s reader does. // r37:finish

use std::io::{Read, Seek, SeekFrom};

/// The thumbnail's columns: each one the peak of its stretch of the file.
pub const COLUMNS: usize = 48;

/// The most bytes read for one column: a short file is read whole, a long one in
/// [`PIECES`] pieces spread over each column's stretch.
const COLUMN_BYTES: u64 = 32 << 10;
const PIECES: u64 = 8;

/// What a row shows of its file.
#[derive(Clone, Debug, PartialEq)]
pub struct Thumb {
    /// Each column's peak, 0…1.
    pub peaks: [f32; COLUMNS],
    /// The length in seconds.
    pub seconds: f32,
    /// Its sample rate and channels (the detail card's format line). // r37:finish
    pub rate: u32,
    pub channels: u16,
}

/// A sample format the thumbnail reads: the tag (1 PCM, 3 float) and the bits.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
struct Format {
    tag: u16,
    channels: u16,
    rate: u32,
    bits: u16,
}

impl Format {
    fn readable(self) -> bool {
        matches!(
            (self.tag, self.bits),
            (1, 8) | (1, 16) | (1, 24) | (1, 32) | (3, 32) | (3, 64)
        ) && self.channels > 0
            && self.rate > 0
    }

    fn bytes(self) -> usize {
        usize::from(self.bits / 8)
    }

    /// One sample's magnitude, 0…1.
    fn sample(self, s: &[u8]) -> f32 {
        let v = match (self.tag, self.bits) {
            (1, 8) => (s[0] as f32 - 128.0) / 128.0,
            (1, 16) => i16::from_le_bytes([s[0], s[1]]) as f32 / 32_768.0,
            (1, 24) => (i32::from_le_bytes([0, s[0], s[1], s[2]]) >> 8) as f32 / 8_388_608.0,
            (1, 32) => i32::from_le_bytes([s[0], s[1], s[2], s[3]]) as f32 / 2_147_483_648.0,
            (3, 32) => f32::from_le_bytes([s[0], s[1], s[2], s[3]]),
            (3, 64) => f64::from_le_bytes([s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7]]) as f32,
            _ => 0.0,
        };
        if v.is_finite() {
            v.abs().min(1.0)
        } else {
            0.0
        }
    }
}

fn u16_of(b: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([b[at], b[at + 1]])
}

fn u32_of(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

/// The thumbnail of the WAV file `r` holds (`len` bytes long).
pub fn read<R: Read + Seek>(r: &mut R, len: u64) -> Option<Thumb> {
    let mut head = [0u8; 12];
    r.read_exact(&mut head).ok()?;
    if &head[0..4] != b"RIFF" || &head[8..12] != b"WAVE" {
        return None;
    }
    let mut at = 12u64;
    let mut fmt: Option<Format> = None;
    while at + 8 <= len {
        r.seek(SeekFrom::Start(at)).ok()?;
        let mut ch = [0u8; 8];
        r.read_exact(&mut ch).ok()?;
        let size = u64::from(u32_of(&ch, 4));
        let body = at + 8;
        if &ch[0..4] == b"fmt " {
            if !(16..=1024).contains(&size) {
                return None;
            }
            let mut f = vec![0u8; size as usize];
            r.read_exact(&mut f).ok()?;
            let mut tag = u16_of(&f, 0);
            if tag == 0xFFFE {
                // WAVE_FORMAT_EXTENSIBLE: the tag is the sub-format GUID's first two bytes.
                if f.len() < 26 {
                    return None;
                }
                tag = u16_of(&f, 24);
            }
            fmt = Some(Format {
                tag,
                channels: u16_of(&f, 2),
                rate: u32_of(&f, 4),
                bits: u16_of(&f, 14),
            });
        } else if &ch[0..4] == b"data" {
            let f = fmt.filter(|f| f.readable())?;
            let size = size.min(len.saturating_sub(body));
            return peaks(r, f, body, size);
        }
        at = body + size + (size & 1);
    }
    None
}

/// The peaks of the `size` bytes of samples at `start`.
fn peaks<R: Read + Seek>(r: &mut R, f: Format, start: u64, size: u64) -> Option<Thumb> {
    let sample = f.bytes();
    let frame = (sample * usize::from(f.channels)) as u64;
    let frames = size / frame;
    if frames == 0 {
        return None;
    }
    let mut out = [0f32; COLUMNS];
    let mut buf = Vec::new();
    for (c, peak) in out.iter_mut().enumerate() {
        let a = frames * c as u64 / COLUMNS as u64;
        let z = (frames * (c as u64 + 1) / COLUMNS as u64).max(a + 1);
        let span = (z - a) * frame;
        // The pieces of this column read: all of it when short, else PIECES spread over it.
        let (n, piece) = if span <= COLUMN_BYTES {
            (1, span)
        } else {
            let piece = (COLUMN_BYTES / PIECES) / frame * frame;
            (PIECES, piece.max(frame))
        };
        for i in 0..n {
            let off = a * frame + (span - piece) * i / (n - 1).max(1) / frame * frame;
            buf.resize(piece as usize, 0);
            r.seek(SeekFrom::Start(start + off)).ok()?;
            let got = read_some(r, &mut buf)?;
            for s in buf[..got - got % sample].chunks_exact(sample) {
                *peak = peak.max(f.sample(s));
            }
        }
    }
    Some(Thumb {
        peaks: out,
        seconds: frames as f32 / f.rate as f32,
        rate: f.rate,
        channels: f.channels,
    })
}

/// Read as much of `buf` as the file has.
fn read_some<R: Read>(r: &mut R, buf: &mut [u8]) -> Option<usize> {
    let mut got = 0;
    while got < buf.len() {
        match r.read(&mut buf[got..]) {
            Ok(0) => break,
            Ok(n) => got += n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(_) => return None,
        }
    }
    Some(got)
}

/// The thumbnail of a WAV file's bytes (tests, and a file already in memory).
pub fn parse_wav(b: &[u8]) -> Option<Thumb> {
    read(&mut std::io::Cursor::new(b), b.len() as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A second of a ramp to near full scale, written by hound (an outside writer) in
    /// `spec`; hound writes WAVE_FORMAT_EXTENSIBLE past 16 bits.
    fn hound_ramp(spec: hound::WavSpec) -> Vec<u8> {
        let mut c = std::io::Cursor::new(Vec::new());
        {
            let mut w = hound::WavWriter::new(&mut c, spec).unwrap();
            let n = spec.sample_rate as usize;
            for i in 0..n {
                let v = i as f32 / n as f32 * 0.9;
                for _ in 0..spec.channels {
                    match spec.sample_format {
                        hound::SampleFormat::Float => w.write_sample(v).unwrap(),
                        hound::SampleFormat::Int => {
                            let full = (1i64 << (spec.bits_per_sample - 1)) as f32;
                            w.write_sample((v * full) as i32).unwrap()
                        }
                    }
                }
            }
            w.finalize().unwrap();
        }
        c.into_inner()
    }

    #[test]
    fn hounds_files_read_true_in_every_format() {
        for (bits, format) in [
            (16, hound::SampleFormat::Int),
            (24, hound::SampleFormat::Int),
            (32, hound::SampleFormat::Int),
            (32, hound::SampleFormat::Float),
        ] {
            let spec = hound::WavSpec {
                channels: 2,
                sample_rate: 8_000,
                bits_per_sample: bits,
                sample_format: format,
            };
            let bytes = hound_ramp(spec);
            if bits > 16 {
                assert_eq!(u16_of(&bytes, 20), 0xFFFE, "hound writes extensible");
            }
            let t = parse_wav(&bytes).unwrap_or_else(|| panic!("{bits} {format:?}"));
            assert!(
                (t.seconds - 1.0).abs() < 1e-3,
                "{bits} {format:?}: {}",
                t.seconds
            );
            // A ramp: the first column near silence, the last near 0.9, rising between.
            assert!(t.peaks[0] < 0.05, "{bits} {format:?}: {:?}", t.peaks);
            assert!(
                (t.peaks[COLUMNS - 1] - 0.9).abs() < 0.02,
                "{bits} {format:?}: {:?}",
                t.peaks
            );
            assert!(
                t.peaks.windows(2).all(|w| w[1] >= w[0]),
                "{bits} {format:?}: {:?}",
                t.peaks
            );
        }
    }

    /// 64-bit float, extensible, built from the published layout (hound writes no f64):
    /// the fmt chunk's 40 bytes with KSDATAFORMAT_SUBTYPE_IEEE_FLOAT.
    #[test]
    fn a_64_bit_float_file_reads_true() {
        let rate = 1000u32;
        let n = 1000usize;
        let mut b = Vec::new();
        b.extend_from_slice(b"RIFF");
        b.extend_from_slice(&(4 + 48 + 8 + 8 * n as u32).to_le_bytes());
        b.extend_from_slice(b"WAVEfmt ");
        b.extend_from_slice(&40u32.to_le_bytes());
        b.extend_from_slice(&0xFFFEu16.to_le_bytes());
        b.extend_from_slice(&1u16.to_le_bytes());
        b.extend_from_slice(&rate.to_le_bytes());
        b.extend_from_slice(&(rate * 8).to_le_bytes());
        b.extend_from_slice(&8u16.to_le_bytes());
        b.extend_from_slice(&64u16.to_le_bytes());
        b.extend_from_slice(&22u16.to_le_bytes());
        b.extend_from_slice(&64u16.to_le_bytes());
        b.extend_from_slice(&4u32.to_le_bytes());
        // {00000003-0000-0010-8000-00aa00389b71}
        b.extend_from_slice(&[
            3, 0, 0, 0, 0, 0, 0x10, 0, 0x80, 0, 0, 0xaa, 0, 0x38, 0x9b, 0x71,
        ]);
        b.extend_from_slice(b"data");
        b.extend_from_slice(&(8 * n as u32).to_le_bytes());
        for i in 0..n {
            b.extend_from_slice(&(i as f64 / n as f64 * 0.8).to_le_bytes());
        }
        let t = parse_wav(&b).expect("a thumbnail");
        assert!((t.seconds - 1.0).abs() < 1e-3);
        assert!((t.peaks[COLUMNS - 1] - 0.8).abs() < 0.02, "{:?}", t.peaks);
        assert!(t.peaks[0] < 0.05, "{:?}", t.peaks);
    }

    /// A long file is read in pieces: what is read for it stays under the columns' bound,
    /// whatever its length.
    #[test]
    fn a_long_file_is_read_in_pieces() {
        struct Counting<R> {
            inner: R,
            read: u64,
        }
        impl<R: Read> Read for Counting<R> {
            fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
                let n = self.inner.read(buf)?;
                self.read += n as u64;
                Ok(n)
            }
        }
        impl<R: Seek> Seek for Counting<R> {
            fn seek(&mut self, p: SeekFrom) -> std::io::Result<u64> {
                self.inner.seek(p)
            }
        }
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: 48_000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut c = std::io::Cursor::new(Vec::new());
        {
            let mut w = hound::WavWriter::new(&mut c, spec).unwrap();
            for i in 0..48_000 * 30 * 2 {
                w.write_sample(((i % 200) as i16 - 100) * 100).unwrap();
            }
            w.finalize().unwrap();
        }
        let bytes = c.into_inner();
        let len = bytes.len() as u64;
        let mut r = Counting {
            inner: std::io::Cursor::new(bytes),
            read: 0,
        };
        let t = read(&mut r, len).expect("a thumbnail");
        assert!((t.seconds - 30.0).abs() < 1e-2);
        assert!(
            r.read <= COLUMNS as u64 * COLUMN_BYTES + 4096,
            "{} of {len}",
            r.read
        );
        assert!(r.read < len / 3, "{} of {len}", r.read);
        assert!(
            t.peaks.iter().all(|p| (*p - 0.3).abs() < 0.01),
            "{:?}",
            t.peaks
        );
    }
}

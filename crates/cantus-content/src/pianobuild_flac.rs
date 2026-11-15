//! The piano build's FLAC step (round 40 phase 3, ADR 0125; the coordinator's decision):
//! a pack that arrives as integer PCM WAV (bitKlavier: 3.8 GB an image) is kept as FLAC —
//! lossless, about half the bytes — and every file is **decoded again and compared with the
//! WAV sample for sample** before the WAV goes. A file that does not compare (or a WAV that
//! is not 16- or 24-bit integer PCM) is kept as it came.

use std::path::Path;

/// Integer PCM read from a WAV: interleaved samples, channels, bits, rate.
#[derive(Clone, Debug, PartialEq)]
pub struct Pcm {
    pub samples: Vec<i32>,
    pub channels: usize,
    pub bits: usize,
    pub rate: usize,
}

fn u16_at(b: &[u8], i: usize) -> Option<u16> {
    Some(u16::from_le_bytes(b.get(i..i + 2)?.try_into().ok()?))
}

fn u32_at(b: &[u8], i: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(i..i + 4)?.try_into().ok()?))
}

/// A WAV's integer samples, or `None` when it is not 16/24-bit integer PCM (or
/// `WAVE_FORMAT_EXTENSIBLE` of it).
pub fn read_pcm(b: &[u8]) -> Option<Pcm> {
    if b.get(0..4)? != b"RIFF" || b.get(8..12)? != b"WAVE" {
        return None;
    }
    let mut i = 12;
    let (mut fmt, mut data) = (None, None);
    while i + 8 <= b.len() {
        let id = &b[i..i + 4];
        let len = u32_at(b, i + 4)? as usize;
        let body = i + 8;
        let end = body.checked_add(len)?.min(b.len());
        match id {
            b"fmt " => fmt = Some((body, end)),
            b"data" => data = Some((body, end)),
            _ => {}
        }
        i = body + len + (len & 1);
    }
    let (f, _) = fmt?;
    let tag = u16_at(b, f)?;
    let channels = usize::from(u16_at(b, f + 2)?);
    let rate = u32_at(b, f + 4)? as usize;
    let bits = usize::from(u16_at(b, f + 14)?);
    let pcm = tag == 1 || (tag == 0xFFFE && u16_at(b, f + 24)? == 1);
    if !pcm || !(bits == 16 || bits == 24) || channels == 0 || channels > 8 {
        return None;
    }
    let (d0, d1) = data?;
    let width = bits / 8;
    let n = (d1 - d0) / width / channels * channels;
    let mut samples = Vec::with_capacity(n);
    for k in 0..n {
        let at = d0 + k * width;
        let v = if bits == 16 {
            i32::from(i16::from_le_bytes([b[at], b[at + 1]]))
        } else {
            i32::from_le_bytes([0, b[at], b[at + 1], b[at + 2]]) >> 8
        };
        samples.push(v);
    }
    Some(Pcm {
        samples,
        channels,
        bits,
        rate,
    })
}

/// `pcm` as a FLAC stream.
pub fn encode(pcm: &Pcm) -> Result<Vec<u8>, String> {
    use flacenc::component::BitRepr;
    use flacenc::error::Verify;
    let config = flacenc::config::Encoder::default()
        .into_verified()
        .map_err(|e| format!("{e:?}"))?;
    let src =
        flacenc::source::MemSource::from_samples(&pcm.samples, pcm.channels, pcm.bits, pcm.rate);
    let stream = flacenc::encode_with_fixed_block_size(&config, src, config.block_size)
        .map_err(|e| format!("{e:?}"))?;
    let mut sink = flacenc::bitsink::ByteSink::new();
    stream.write(&mut sink).map_err(|e| format!("{e:?}"))?;
    Ok(sink.as_slice().to_vec())
}

/// Whether a FLAC stream decodes to exactly `pcm`.
pub fn same(flac: &[u8], pcm: &Pcm) -> bool {
    let Ok(mut r) = claxon::FlacReader::new(std::io::Cursor::new(flac)) else {
        return false;
    };
    let info = r.streaminfo();
    if info.channels as usize != pcm.channels
        || info.bits_per_sample as usize != pcm.bits
        || info.sample_rate as usize != pcm.rate
    {
        return false;
    }
    let mut n = 0;
    for s in r.samples() {
        match s {
            Ok(v) if pcm.samples.get(n) == Some(&v) => n += 1,
            _ => return false,
        }
    }
    n == pcm.samples.len()
}

/// What happened to one file.
#[derive(Clone, Debug, PartialEq)]
pub enum Done {
    /// Written as `.flac`, checked, the WAV removed.
    Flac,
    /// Kept as it came, and why.
    Kept(String),
}

/// Turn `dir/name` (a `.wav`) into `dir/<stem>.flac`, checked bit for bit, and remove the
/// WAV. Returns the file name the instrument now plays.
pub fn transcode(dir: &Path, name: &str) -> (String, Done) {
    let path = dir.join(name);
    let Some(stem) = name
        .strip_suffix(".wav")
        .or_else(|| name.strip_suffix(".WAV"))
    else {
        return (name.to_string(), Done::Kept("not a WAV".into()));
    };
    let bytes = match std::fs::read(&path) {
        Ok(b) => b,
        Err(e) => return (name.to_string(), Done::Kept(e.to_string())),
    };
    let Some(pcm) = read_pcm(&bytes) else {
        return (
            name.to_string(),
            Done::Kept("not 16- or 24-bit integer PCM".into()),
        );
    };
    let flac = match encode(&pcm) {
        Ok(f) => f,
        Err(e) => return (name.to_string(), Done::Kept(e)),
    };
    if !same(&flac, &pcm) {
        return (
            name.to_string(),
            Done::Kept("the FLAC did not decode to the WAV's samples".into()),
        );
    }
    let out = format!("{stem}.flac");
    if let Err(e) = std::fs::write(dir.join(&out), &flac) {
        return (name.to_string(), Done::Kept(e.to_string()));
    }
    let _ = std::fs::remove_file(&path);
    (out, Done::Flac)
}

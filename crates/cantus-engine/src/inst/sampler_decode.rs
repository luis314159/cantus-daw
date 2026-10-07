//! Decoding the audio formats sample libraries use beside WAV: AIFF/AIFC (parsed here),
//! FLAC (`claxon`, Apache-2.0) and Ogg Vorbis (`lewton`, MIT OR Apache-2.0). Called by
//! `SampleCache` off the audio thread; like its WAV path, it keeps at most two channels
//! and yields interleaved `f32`.

use cantus_core::SampleData;
use cantus_dsp::math;
use std::path::Path;

/// Decode a file by its extension; `None` when the extension is not one of these (the
/// caller falls back to WAV).
pub fn decode(path: &Path) -> Option<Result<SampleData, String>> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)?;
    Some(match ext.as_str() {
        "aif" | "aiff" | "aifc" => std::fs::read(path)
            .map_err(|e| e.to_string())
            .and_then(|b| decode_aiff(&b)),
        "flac" => decode_flac(path),
        "ogg" | "oga" => std::fs::File::open(path)
            .map_err(|e| e.to_string())
            .and_then(decode_ogg),
        _ => return None,
    })
}

fn finish(rate: u32, in_channels: usize, samples: Vec<f32>) -> Result<SampleData, String> {
    let in_channels = in_channels.max(1);
    let out_channels = in_channels.min(2);
    let frames: Vec<f32> = if in_channels == out_channels {
        samples
    } else {
        samples
            .chunks_exact(in_channels)
            .flat_map(|f| f[..out_channels].to_vec())
            .collect()
    };
    if frames.is_empty() {
        return Err("the file holds no audio".to_string());
    }
    Ok(SampleData {
        rate,
        channels: out_channels as u8,
        frames: frames.into(),
    })
}

/// An IEEE 754 80-bit extended float (AIFF's sample rate).
fn extended(b: &[u8]) -> f64 {
    let exp = (((b[0] as i32) & 0x7F) << 8 | b[1] as i32) - 16_383;
    let mant = u64::from_be_bytes([b[2], b[3], b[4], b[5], b[6], b[7], b[8], b[9]]);
    let v = mant as f64 * math::powi(2f64, exp - 63);
    if b[0] & 0x80 != 0 {
        -v
    } else {
        v
    }
}

/// AIFF and AIFC: big-endian PCM, `sowt` little-endian PCM, `fl32`/`fl64` floats.
pub fn decode_aiff(b: &[u8]) -> Result<SampleData, String> {
    if b.len() < 12 || &b[0..4] != b"FORM" || !(&b[8..12] == b"AIFF" || &b[8..12] == b"AIFC") {
        return Err("not an AIFF file".into());
    }
    let (mut channels, mut bits, mut rate, mut codec) = (0usize, 0usize, 0.0f64, *b"NONE");
    let mut data: Option<&[u8]> = None;
    let mut at = 12;
    while at + 8 <= b.len() {
        let id = &b[at..at + 4];
        let size = u32::from_be_bytes([b[at + 4], b[at + 5], b[at + 6], b[at + 7]]) as usize;
        let body = &b[at + 8..(at + 8 + size).min(b.len())];
        match id {
            b"COMM" if body.len() >= 18 => {
                channels = u16::from_be_bytes([body[0], body[1]]) as usize;
                bits = u16::from_be_bytes([body[6], body[7]]) as usize;
                rate = extended(&body[8..18]);
                if body.len() >= 22 {
                    codec.copy_from_slice(&body[18..22]);
                }
            }
            b"SSND" if body.len() >= 8 => {
                let offset = u32::from_be_bytes([body[0], body[1], body[2], body[3]]) as usize;
                data = body.get(8 + offset..);
            }
            _ => {}
        }
        at += 8 + size + (size & 1);
    }
    let data = data.ok_or("the AIFF file has no sound data")?;
    if channels == 0 || rate <= 0.0 {
        return Err("the AIFF file has no format chunk".into());
    }
    let samples: Vec<f32> = match &codec {
        b"fl32" | b"FL32" => data
            .chunks_exact(4)
            .map(|c| f32::from_be_bytes([c[0], c[1], c[2], c[3]]))
            .collect(),
        b"fl64" | b"FL64" => data
            .chunks_exact(8)
            .map(|c| f64::from_be_bytes([c[0], c[1], c[2], c[3], c[4], c[5], c[6], c[7]]) as f32)
            .collect(),
        b"NONE" | b"twos" | b"sowt" => {
            let little = &codec == b"sowt";
            let width = bits.div_ceil(8);
            if !(1..=4).contains(&width) {
                return Err(format!("{bits}-bit AIFF is not supported"));
            }
            let scale = 1.0 / (1u64 << (width * 8 - 1)) as f32;
            data.chunks_exact(width)
                .map(|c| {
                    let mut v: i32 = 0;
                    for k in 0..width {
                        let byte = if little { c[width - 1 - k] } else { c[k] };
                        v = (v << 8) | byte as i32;
                    }
                    let shift = 32 - width * 8;
                    ((v << shift) >> shift) as f32 * scale
                })
                .collect()
        }
        other => {
            return Err(format!(
                "AIFC compression {} is not supported",
                String::from_utf8_lossy(other)
            ))
        }
    };
    finish(rate.round() as u32, channels, samples)
}

fn decode_flac(path: &Path) -> Result<SampleData, String> {
    let mut reader = claxon::FlacReader::open(path).map_err(|e| e.to_string())?;
    let info = reader.streaminfo();
    let scale = 1.0 / (1u64 << (info.bits_per_sample - 1)) as f32;
    let mut samples =
        Vec::with_capacity(info.samples.unwrap_or(0) as usize * info.channels as usize);
    for s in reader.samples() {
        samples.push(s.map_err(|e| e.to_string())? as f32 * scale);
    }
    finish(info.sample_rate, info.channels as usize, samples)
}

/// Ogg Vorbis from any reader (a file, or an SF3 sample held in memory).
pub fn decode_ogg<R: std::io::Read + std::io::Seek>(r: R) -> Result<SampleData, String> {
    let mut ogg = lewton::inside_ogg::OggStreamReader::new(r).map_err(|e| e.to_string())?;
    let rate = ogg.ident_hdr.audio_sample_rate;
    let channels = ogg.ident_hdr.audio_channels as usize;
    let mut samples = Vec::new();
    while let Some(packet) = ogg.read_dec_packet_itl().map_err(|e| e.to_string())? {
        samples.extend(packet.iter().map(|s| *s as f32 / 32_768.0));
    }
    finish(rate, channels, samples)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn aiff(bits: u16, codec: Option<&[u8; 4]>, frames: &[i32]) -> Vec<u8> {
        let mut comm = Vec::new();
        comm.extend(1u16.to_be_bytes());
        comm.extend((frames.len() as u32).to_be_bytes());
        comm.extend(bits.to_be_bytes());
        // 44100 as an 80-bit extended float.
        comm.extend([0x40, 0x0E, 0xAC, 0x44, 0, 0, 0, 0, 0, 0]);
        if let Some(c) = codec {
            comm.extend(c);
            comm.push(0);
            comm.push(0);
        }
        let width = (bits / 8) as usize;
        let mut ssnd = vec![0u8; 8];
        for f in frames {
            let be = f.to_be_bytes();
            let bytes = &be[4 - width..];
            if codec == Some(b"sowt") {
                ssnd.extend(bytes.iter().rev());
            } else {
                ssnd.extend(bytes);
            }
        }
        let mut body = Vec::new();
        body.extend(if codec.is_some() { b"AIFC" } else { b"AIFF" });
        for (id, chunk) in [(b"COMM", &comm), (b"SSND", &ssnd)] {
            body.extend(id);
            body.extend((chunk.len() as u32).to_be_bytes());
            body.extend(chunk.iter());
            if chunk.len() % 2 == 1 {
                body.push(0);
            }
        }
        let mut out = b"FORM".to_vec();
        out.extend((body.len() as u32).to_be_bytes());
        out.extend(body);
        out
    }

    #[test]
    fn aiff_reads_big_and_little_endian_pcm() {
        for codec in [None, Some(b"sowt")] {
            let d = decode_aiff(&aiff(16, codec, &[0, 16_384, -32_768])).unwrap();
            assert_eq!((d.rate, d.channels), (44_100, 1));
            assert_eq!(&*d.frames, &[0.0, 0.5, -1.0], "{codec:?}");
        }
        let d = decode_aiff(&aiff(24, None, &[4_194_304])).unwrap();
        assert_eq!(&*d.frames, &[0.5]);
        assert!(decode_aiff(b"RIFF....WAVE").is_err());
    }
}

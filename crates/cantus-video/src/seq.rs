//! PNG frames and the PNG sequence + WAV (ADR 0088 §3): the universal fallback every
//! editor imports — a folder of numbered PNG files (straight alpha when transparent) and the
//! song as a 16-bit WAV beside them, frame `n` at `n / fps` seconds.

use crate::yuv;
use std::io::Write;
use std::path::{Path, PathBuf};

/// A frame as a PNG file: from tiny-skia's premultiplied RGBA, straight RGBA with `alpha`,
/// else RGB. `scratch` is reused between frames.
pub fn encode_png(
    premultiplied: &[u8],
    width: u32,
    height: u32,
    alpha: bool,
    scratch: &mut Vec<u8>,
) -> Vec<u8> {
    scratch.clear();
    let colour = if alpha {
        scratch.extend_from_slice(premultiplied);
        yuv::demultiply(scratch);
        png::ColorType::Rgba
    } else {
        yuv::rgba_to_rgb(premultiplied, scratch);
        png::ColorType::Rgb
    };
    let mut out = Vec::with_capacity(scratch.len() / 4);
    {
        let mut e = png::Encoder::new(&mut out, width, height);
        e.set_color(colour);
        e.set_depth(png::BitDepth::Eight);
        // Fast deflate with the adaptive filter: 4–5× the speed of the default for ~10 %
        // more bytes (measured in the probe), which a 30-second clip at 1080×1920 needs.
        e.set_compression(png::Compression::Fast);
        let mut w = e.write_header().expect("a PNG header into memory");
        w.write_image_data(scratch)
            .expect("PNG data of the header's size into memory");
    }
    out
}

/// The file name of frame `n` in a sequence called `stem` of `total` frames:
/// `stem_00042.png` (at least five digits, more for a longer sequence).
pub fn frame_name(stem: &str, n: u32, total: u32) -> String {
    let digits = total.saturating_sub(1).max(1).ilog10() as usize + 1;
    format!("{stem}_{n:0width$}.png", width = digits.max(5))
}

/// The WAV beside a sequence: `stem.wav`.
pub fn wav_name(stem: &str) -> String {
    format!("{stem}.wav")
}

/// A 16-bit PCM WAV file's bytes, interleaved `channels` at `rate`.
pub fn wav_bytes(samples: &[i16], rate: u32, channels: u16) -> Vec<u8> {
    let data = samples.len() as u32 * 2;
    let mut v = Vec::with_capacity(44 + data as usize);
    v.extend_from_slice(b"RIFF");
    v.extend_from_slice(&(36 + data).to_le_bytes());
    v.extend_from_slice(b"WAVEfmt ");
    v.extend_from_slice(&16u32.to_le_bytes());
    v.extend_from_slice(&1u16.to_le_bytes());
    v.extend_from_slice(&channels.to_le_bytes());
    v.extend_from_slice(&rate.to_le_bytes());
    v.extend_from_slice(&(rate * channels as u32 * 2).to_le_bytes());
    v.extend_from_slice(&(channels * 2).to_le_bytes());
    v.extend_from_slice(&16u16.to_le_bytes());
    v.extend_from_slice(b"data");
    v.extend_from_slice(&data.to_le_bytes());
    for s in samples {
        v.extend_from_slice(&s.to_le_bytes());
    }
    v
}

/// A sequence being written into `dir` (a folder made new for it).
pub struct SeqWriter {
    dir: PathBuf,
    stem: String,
    total: u32,
    written: u32,
    bytes: u64,
}

impl SeqWriter {
    pub fn new(dir: &Path, stem: &str, total: u32) -> std::io::Result<SeqWriter> {
        std::fs::create_dir(dir)?;
        Ok(SeqWriter {
            dir: dir.to_path_buf(),
            stem: stem.to_string(),
            total,
            written: 0,
            bytes: 0,
        })
    }

    pub fn frames(&self) -> u32 {
        self.written
    }

    pub fn bytes(&self) -> u64 {
        self.bytes
    }

    /// The next frame's PNG.
    pub fn frame(&mut self, png: &[u8]) -> std::io::Result<()> {
        let path = self
            .dir
            .join(frame_name(&self.stem, self.written, self.total));
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)?;
        f.write_all(png)?;
        self.written += 1;
        self.bytes += png.len() as u64;
        Ok(())
    }

    /// The song beside the frames; the folder, done.
    pub fn finish(self, audio: &[i16], rate: u32) -> std::io::Result<PathBuf> {
        let wav = wav_bytes(audio, rate, 2);
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(self.dir.join(wav_name(&self.stem)))?;
        f.write_all(&wav)?;
        f.sync_all()?;
        Ok(self.dir)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_names_sort_and_widen_for_long_sequences() {
        assert_eq!(frame_name("clip", 7, 900), "clip_00007.png");
        assert_eq!(frame_name("clip", 123_456, 200_000), "clip_123456.png");
        assert_eq!(frame_name("clip", 0, 1), "clip_00000.png");
    }

    #[test]
    fn a_png_frame_decodes_to_the_straight_colours() {
        // One opaque red pixel and one half-transparent white (premultiplied 128).
        let px = [255, 0, 0, 255, 128, 128, 128, 128];
        let mut scratch = Vec::new();
        let png = encode_png(&px, 2, 1, true, &mut scratch);
        let mut d = png::Decoder::new(std::io::Cursor::new(png))
            .read_info()
            .unwrap();
        let mut buf = vec![0; d.output_buffer_size().unwrap()];
        d.next_frame(&mut buf).unwrap();
        assert_eq!(buf, [255, 0, 0, 255, 255, 255, 255, 128]);
        let rgb = encode_png(&px, 2, 1, false, &mut scratch);
        let mut d = png::Decoder::new(std::io::Cursor::new(rgb))
            .read_info()
            .unwrap();
        let mut buf = vec![0; d.output_buffer_size().unwrap()];
        d.next_frame(&mut buf).unwrap();
        assert_eq!(buf, [255, 0, 0, 128, 128, 128]);
    }

    #[test]
    fn a_wav_header_is_right() {
        let w = wav_bytes(&[1, -1, 2, -2], 48_000, 2);
        assert_eq!(&w[..4], b"RIFF");
        assert_eq!(u32::from_le_bytes(w[4..8].try_into().unwrap()), 36 + 8);
        assert_eq!(u32::from_le_bytes(w[24..28].try_into().unwrap()), 48_000);
        assert_eq!(&w[44..], &[1, 0, 255, 255, 2, 0, 254, 255]);
    }
}

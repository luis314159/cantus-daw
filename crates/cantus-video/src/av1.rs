//! AV1 by rav1e (ADR 0088 §2): royalty-free, pure Rust (BSD-2-Clause with the Alliance
//! for Open Media's patent licence), nothing downloaded. Slow next to H.264 — the dialog
//! says so and shows its estimate — so it runs with tiles and threads at its fastest
//! sensible speed.

use crate::yuv::Nv12;
use crate::VideoError;
use rav1e::prelude::*;

/// One AV1 temporal unit (temporal delimiters taken out, as WebM's mapping wants) and the
/// frame it shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Av1Packet {
    pub frame: u64,
    pub key: bool,
    pub data: Vec<u8>,
}

/// rav1e's quantizer for a quality (its default is 100).
pub fn quantizer(q: crate::Quality) -> usize {
    match q {
        crate::Quality::Good => 130,
        crate::Quality::High => 100,
        crate::Quality::Best => 70,
    }
}

/// Whether rav1e was built with its assembly (`av1-asm`: 2.8× faster, the same bytes —
/// measured; releases build it with nasm).
pub const ASSEMBLY: bool = cfg!(feature = "av1-asm");

/// rav1e's speed preset (0 slowest … 10 fastest).
pub const SPEED: u8 = 10;

/// Tiles for a picture: about one a 256×256 area, at most 16 (each runs on its own).
pub fn tiles(width: u32, height: u32) -> usize {
    ((width as usize * height as usize) / (256 * 256)).clamp(1, 16)
}

/// The AV1 encoder.
pub struct Av1 {
    ctx: Context<u8>,
    width: usize,
    height: usize,
}

impl Av1 {
    /// An encoder at rav1e's `quantizer` (0 best … 255; [`quantizer`] for a quality).
    pub fn new(width: u32, height: u32, fps: u32, quantizer: usize) -> Result<Av1, VideoError> {
        let enc = EncoderConfig {
            width: width as usize,
            height: height as usize,
            time_base: Rational::new(1, fps as u64),
            bit_depth: 8,
            chroma_sampling: ChromaSampling::Cs420,
            pixel_range: PixelRange::Limited,
            color_description: Some(ColorDescription {
                color_primaries: ColorPrimaries::BT709,
                transfer_characteristics: TransferCharacteristics::BT709,
                matrix_coefficients: MatrixCoefficients::BT709,
            }),
            // Constant quantizer: rav1e 0.8's rate control (a bitrate) made streams neither
            // libaom nor dav1d could decode past some frames (measured; ADR 0088 amendment).
            bitrate: 0,
            quantizer,
            min_key_frame_interval: fps as u64,
            max_key_frame_interval: fps as u64 * 2,
            speed_settings: SpeedSettings::from_preset(SPEED),
            tiles: tiles(width, height),
            ..Default::default()
        };
        let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
        let ctx = Config::new()
            .with_encoder_config(enc)
            .with_threads(threads)
            .new_context()
            .map_err(|e| VideoError::encoder("AV1", format!("{e:?}")))?;
        Ok(Av1 {
            ctx,
            width: width as usize,
            height: height as usize,
        })
    }

    /// The `av1C` record (WebM's CodecPrivate).
    pub fn private(&self) -> Vec<u8> {
        self.ctx.container_sequence_header()
    }

    pub fn encode(&mut self, f: &Nv12, out: &mut Vec<Av1Packet>) -> Result<(), VideoError> {
        let mut frame = self.ctx.new_frame();
        let cw = self.width / 2;
        let (mut u, mut v) = (Vec::with_capacity(cw * self.height / 2), Vec::new());
        v.reserve(cw * self.height / 2);
        for uv in f.uv().chunks_exact(2) {
            u.push(uv[0]);
            v.push(uv[1]);
        }
        frame.planes[0].copy_from_raw_u8(f.y(), self.width, 1);
        frame.planes[1].copy_from_raw_u8(&u, cw, 1);
        frame.planes[2].copy_from_raw_u8(&v, cw, 1);
        self.ctx
            .send_frame(frame)
            .map_err(|e| VideoError::encoder("AV1", format!("{e:?}")))?;
        self.drain(out).map(|_| ())
    }

    pub fn finish(&mut self, out: &mut Vec<Av1Packet>) -> Result<(), VideoError> {
        self.ctx.flush();
        while !self.drain(out)? {}
        Ok(())
    }

    /// Take what is ready; `true` once everything is out.
    fn drain(&mut self, out: &mut Vec<Av1Packet>) -> Result<bool, VideoError> {
        loop {
            match self.ctx.receive_packet() {
                Ok(p) => {
                    let data = strip_temporal_delimiters(&p.data);
                    out.push(Av1Packet {
                        frame: p.input_frameno,
                        key: p.frame_type == FrameType::KEY,
                        data,
                    })
                }
                Err(EncoderStatus::Encoded) => {}
                Err(EncoderStatus::NeedMoreData) => return Ok(false),
                Err(EncoderStatus::LimitReached) => return Ok(true),
                Err(e) => return Err(VideoError::encoder("AV1", format!("{e:?}"))),
            }
        }
    }
}

/// The OBUs of `d` without its temporal delimiters (type 2).
pub fn strip_temporal_delimiters(d: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(d.len());
    let mut i = 0;
    while i < d.len() {
        let h = d[i];
        let kind = (h >> 3) & 0xf;
        let ext = ((h >> 2) & 1) as usize;
        let mut j = i + 1 + ext;
        let size = if (h >> 1) & 1 == 1 {
            let mut v = 0usize;
            let mut k = 0;
            while j < d.len() {
                let b = d[j];
                v |= ((b & 0x7f) as usize) << (7 * k);
                j += 1;
                k += 1;
                if b & 0x80 == 0 || k == 8 {
                    break;
                }
            }
            v
        } else {
            d.len() - j
        };
        let end = (j + size).min(d.len());
        if kind != 2 {
            out.extend_from_slice(&d[i..end]);
        }
        i = end;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn temporal_delimiters_are_taken_out() {
        // A temporal delimiter (type 2, size 0), then a frame OBU (type 6) of 3 bytes.
        let tu = [0x12, 0x00, 0x32, 0x03, 9, 8, 7];
        assert_eq!(strip_temporal_delimiters(&tu), vec![0x32, 0x03, 9, 8, 7]);
    }

    #[test]
    fn tiles_grow_with_the_picture() {
        assert_eq!(tiles(320, 240), 1);
        assert_eq!(tiles(1080, 1920), 16);
        assert_eq!(tiles(1080, 1080), 16);
    }

    #[test]
    fn one_packet_a_frame_in_order() {
        let (w, h) = (160u32, 96u32);
        let mut enc = Av1::new(w, h, 60, 100).unwrap();
        let mut out = Vec::new();
        for n in 0..150u32 {
            let mut f = Nv12 {
                width: w,
                height: h,
                data: vec![128; (w * h * 3 / 2) as usize],
            };
            let bright = if n % 30 == 0 { 235 } else { 16 + (n % 7) as u8 };
            for (i, y) in f.data[..(w * h) as usize].iter_mut().enumerate() {
                *y = if (i as u32 % w) < (n * 3 % w) {
                    bright
                } else {
                    60
                };
            }
            enc.encode(&f, &mut out).unwrap();
        }
        enc.finish(&mut out).unwrap();
        let frames: Vec<u64> = out.iter().map(|p| p.frame).collect();
        assert_eq!(frames, (0..150).collect::<Vec<u64>>());
    }
}

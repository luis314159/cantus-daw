//! macOS's own H.264 and AAC (ADR 0088 §1): VideoToolbox and AudioToolbox, in-box and
//! licensed with the system, into our MP4 writer — the same path as Media Foundation's on
//! Windows. The C interfaces live in `vt.rs` and `at.rs`, which depend on nothing but
//! `std` so they compile for Apple targets on any machine; this file is the glue.
//! What only a real Mac confirms is listed in `docs/mac-session.md`.

pub mod at;
pub mod vt;

use crate::audio::{aac_lc_config, AudioCodec, AudioEncoder};
use crate::h264::H264Encoder;
use crate::yuv::Nv12;
use crate::VideoError;

/// VideoToolbox's H.264 encoder.
pub struct VtH264 {
    session: vt::VtSession,
}

impl VtH264 {
    pub fn new(width: u32, height: u32, fps: u32, bitrate: u32) -> Result<VtH264, VideoError> {
        let session = vt::VtSession::new(width, height, fps, bitrate)
            .map_err(|e| VideoError::encoder("H.264", e))?;
        Ok(VtH264 { session })
    }
}

impl H264Encoder for VtH264 {
    fn name(&self) -> String {
        "VideoToolbox H.264".into()
    }

    fn encode(&mut self, frame: &Nv12, out: &mut Vec<Vec<u8>>) -> Result<(), VideoError> {
        self.session
            .encode(&frame.data, out)
            .map_err(|e| VideoError::encoder("H.264", e))
    }

    fn finish(&mut self, out: &mut Vec<Vec<u8>>) -> Result<(), VideoError> {
        self.session
            .finish(out)
            .map_err(|e| VideoError::encoder("H.264", e))
    }
}

/// Bits a second of AAC.
pub const BITRATE: u32 = 192_000;

/// AudioToolbox's AAC-LC encoder.
pub struct AtAac {
    conv: at::AtConverter,
    rate: u32,
    channels: u16,
}

impl AtAac {
    pub fn new(rate: u32, channels: u16) -> Result<AtAac, VideoError> {
        let conv = at::AtConverter::new(rate, channels, BITRATE)
            .map_err(|e| VideoError::encoder("AAC", e))?;
        Ok(AtAac {
            conv,
            rate,
            channels,
        })
    }
}

impl AudioEncoder for AtAac {
    fn name(&self) -> String {
        "AudioToolbox AAC-LC".into()
    }
    fn codec(&self) -> AudioCodec {
        AudioCodec::Aac {
            config: aac_lc_config(self.rate, self.channels)
                .map(|c| c.to_vec())
                .unwrap_or_default(),
        }
    }
    fn rate(&self) -> u32 {
        self.rate
    }
    fn channels(&self) -> u16 {
        self.channels
    }
    fn frame_len(&self) -> u32 {
        1_024
    }
    fn delay(&self) -> u32 {
        self.conv.priming
    }
    fn bitrate(&self) -> u32 {
        BITRATE
    }
    fn encode(&mut self, pcm: &[i16], out: &mut Vec<Vec<u8>>) -> Result<(), VideoError> {
        self.conv
            .encode(pcm, out)
            .map_err(|e| VideoError::encoder("AAC", e))
    }
    fn finish(&mut self, out: &mut Vec<Vec<u8>>) -> Result<(), VideoError> {
        self.conv
            .finish(out)
            .map_err(|e| VideoError::encoder("AAC", e))
    }
}

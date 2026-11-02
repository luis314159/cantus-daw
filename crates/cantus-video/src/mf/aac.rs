//! The Microsoft AAC encoder transform: AAC-LC, raw packets of 1 024 samples.

use super::{create, fail, sample, Mft};
use crate::audio::{aac_lc_config, AudioCodec, AudioEncoder};
use crate::VideoError;
use windows::core::GUID;
use windows::Win32::Media::MediaFoundation::*;

const CODEC: &str = "AAC";

/// CLSID_AACMFTEncoder (mfidl.h; not in the `windows` crate's metadata).
pub(crate) const CLSID_AAC_ENCODER: GUID = GUID::from_u128(0x93af0c51_2275_45d2_a35b_f2ba21caed00);

/// Silence given before the song: our own priming, which the MP4's edit list skips.
///
/// Measured (the round's probe, and `tests/sync.rs` decoding back with FFmpeg ignoring the
/// edit list): this encoder adds **no** delay of its own — decoded sample `d` is input
/// sample `d` — so its first packet, which a decoder cannot overlap with any before it,
/// loses the song's first ~10 ms (the probe's "click at t=0 lost"). Leading with one packet
/// of silence makes the song's first sample the first sample of a whole packet; the edit
/// list starts there, and a player that ignores edit lists is one packet (21 ms) late — as
/// with every other AAC file's priming.
pub const LEAD: u32 = 1_024;

/// Silence given after the song, so its last packet is overlapped by another and decodes
/// whole; the edit list ends at the song's last sample.
pub const TAIL: u32 = 2_048;

/// Bits a second: 192 kbit/s, the encoder's most (it offers 96, 128, 160 and 192).
pub const BITRATE: u32 = 192_000;

/// Media Foundation's AAC encoder.
pub struct MfAac {
    mft: Mft,
    rate: u32,
    channels: u16,
    config: Vec<u8>,
    /// Samples given so far (per channel).
    given: u64,
}

fn audio_type(subtype: &GUID, rate: u32, channels: u16) -> windows::core::Result<IMFMediaType> {
    // SAFETY: attributes set on a new media type.
    unsafe {
        let t = super::mf()?.media_type()?;
        t.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio)?;
        t.SetGUID(&MF_MT_SUBTYPE, subtype)?;
        t.SetUINT32(&MF_MT_AUDIO_BITS_PER_SAMPLE, 16)?;
        t.SetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND, rate)?;
        t.SetUINT32(&MF_MT_AUDIO_NUM_CHANNELS, channels as u32)?;
        Ok(t)
    }
}

impl MfAac {
    /// An encoder for `channels` (1 or 2) at `rate` (44 100 or 48 000).
    pub fn new(rate: u32, channels: u16) -> Result<MfAac, VideoError> {
        let t = create(&CLSID_AAC_ENCODER, CODEC)?;
        // SAFETY: types set on the new transform: the input first, then the output (the
        // AAC encoder's documented order).
        unsafe {
            let input = audio_type(&MFAudioFormat_PCM, rate, channels)?;
            input.SetUINT32(&MF_MT_AUDIO_BLOCK_ALIGNMENT, 2 * channels as u32)?;
            input.SetUINT32(
                &MF_MT_AUDIO_AVG_BYTES_PER_SECOND,
                rate * 2 * channels as u32,
            )?;
            t.SetInputType(0, &input, 0).map_err(|e| fail(CODEC, e))?;
            let out = audio_type(&MFAudioFormat_AAC, rate, channels)?;
            out.SetUINT32(&MF_MT_AUDIO_AVG_BYTES_PER_SECOND, BITRATE / 8)?;
            out.SetUINT32(&MF_MT_AAC_PAYLOAD_TYPE, 0)?; // raw access units
            out.SetUINT32(&MF_MT_AAC_AUDIO_PROFILE_LEVEL_INDICATION, 0x29)?; // AAC profile L2
            t.SetOutputType(0, &out, 0).map_err(|e| fail(CODEC, e))?;
        }
        let config = Self::config_of(&t)
            .or_else(|| aac_lc_config(rate, channels).map(|c| c.to_vec()))
            .ok_or_else(|| VideoError::Unsupported(format!("AAC at {rate} Hz")))?;
        let mft = Mft::new(t, CODEC, 8_192)?;
        Ok(MfAac {
            mft,
            rate,
            channels,
            config,
            given: 0,
        })
    }

    /// The AudioSpecificConfig the encoder states: `MF_MT_USER_DATA` is the tail of a
    /// HEAACWAVEINFO (12 bytes) followed by it.
    fn config_of(t: &IMFTransform) -> Option<Vec<u8>> {
        // SAFETY: the blob is read into a buffer of its own stated size.
        unsafe {
            let ty = t.GetOutputCurrentType(0).ok()?;
            let n = ty.GetBlobSize(&MF_MT_USER_DATA).ok()? as usize;
            let mut b = vec![0u8; n];
            ty.GetBlob(&MF_MT_USER_DATA, &mut b, None).ok()?;
            (n > 12).then(|| b[12..].to_vec())
        }
    }
}

impl MfAac {
    /// Give the encoder `pcm` as it is, timed by the samples given before it.
    fn feed(&mut self, pcm: &[i16], out: &mut Vec<Vec<u8>>) -> Result<(), VideoError> {
        let n = (pcm.len() / self.channels as usize) as u64;
        let at = |i: u64| (i * 10_000_000 / self.rate as u64) as i64;
        let bytes: Vec<u8> = pcm.iter().flat_map(|s| s.to_le_bytes()).collect();
        let (t0, t1) = (at(self.given), at(self.given + n));
        let s = sample(&bytes, t0, t1 - t0).map_err(|e| fail(CODEC, e))?;
        self.mft.push(&s, out)?;
        self.given += n;
        Ok(())
    }
}

impl AudioEncoder for MfAac {
    fn name(&self) -> String {
        "Media Foundation AAC-LC".into()
    }

    fn codec(&self) -> AudioCodec {
        AudioCodec::Aac {
            config: self.config.clone(),
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
        LEAD
    }

    fn bitrate(&self) -> u32 {
        BITRATE
    }

    fn encode(&mut self, pcm: &[i16], out: &mut Vec<Vec<u8>>) -> Result<(), VideoError> {
        if pcm.is_empty() {
            return Ok(());
        }
        if self.given == 0 {
            self.feed(&vec![0; LEAD as usize * self.channels as usize], out)?;
        }
        self.feed(pcm, out)
    }

    fn finish(&mut self, out: &mut Vec<Vec<u8>>) -> Result<(), VideoError> {
        if self.given == 0 {
            self.feed(&vec![0; LEAD as usize * self.channels as usize], out)?;
        }
        self.feed(&vec![0; TAIL as usize * self.channels as usize], out)?;
        self.mft.drain(out)
    }
}

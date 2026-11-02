//! The audio of a video: 16-bit PCM in, and an encoder for the MP4 (AAC-LC from the
//! system; MP3 where no AAC exists — ADR 0088 §1).
//!
//! An encoder's **delay** (its priming: the samples it puts out before the first input
//! sample — AAC's 1 024 and more, LAME's 1 105) is trimmed by the MP4's audio edit list,
//! so the first decoded sample is the song's first sample and a click stays on its flash.

use crate::VideoError;

/// The audio stream as an MP4 describes it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AudioCodec {
    /// AAC with its AudioSpecificConfig.
    Aac { config: Vec<u8> },
    /// MPEG-1 Layer III.
    Mp3,
}

impl AudioCodec {
    /// The MPEG-4 objectTypeIndication.
    pub fn object_type(&self) -> u8 {
        match self {
            AudioCodec::Aac { .. } => 0x40,
            AudioCodec::Mp3 => 0x6b,
        }
    }
}

/// An audio encoder: interleaved 16-bit PCM in, packets out.
pub trait AudioEncoder {
    fn name(&self) -> String;
    fn codec(&self) -> AudioCodec;
    fn rate(&self) -> u32;
    fn channels(&self) -> u16;
    /// Samples (per channel) each packet decodes to.
    fn frame_len(&self) -> u32;
    /// Samples of priming before the first input sample.
    fn delay(&self) -> u32;
    /// The mean bitrate, bits a second.
    fn bitrate(&self) -> u32;
    /// Encode interleaved samples; packets ready are pushed to `out`.
    fn encode(&mut self, pcm: &[i16], out: &mut Vec<Vec<u8>>) -> Result<(), VideoError>;
    /// No more samples: the packets still inside are pushed to `out`.
    fn finish(&mut self, out: &mut Vec<Vec<u8>>) -> Result<(), VideoError>;
}

/// The MPEG-4 sampling-frequency index of `rate` (ISO 14496-3 Table 1.18).
pub fn rate_index(rate: u32) -> Option<u8> {
    const RATES: [u32; 13] = [
        96_000, 88_200, 64_000, 48_000, 44_100, 32_000, 24_000, 22_050, 16_000, 12_000, 11_025,
        8_000, 7_350,
    ];
    RATES.iter().position(|&r| r == rate).map(|i| i as u8)
}

/// The AudioSpecificConfig of AAC-LC at `rate` with `channels` (2 bytes).
pub fn aac_lc_config(rate: u32, channels: u16) -> Option<[u8; 2]> {
    let idx = rate_index(rate)?;
    let object = 2u16; // AAC LC
    let bits = object << 11 | (idx as u16) << 7 | (channels.min(7)) << 3;
    Some(bits.to_be_bytes())
}

/// Interleaved floats to 16-bit, rounded to nearest and clipped at full scale.
pub fn to_i16(samples: &[f32], out: &mut Vec<i16>) {
    out.clear();
    out.extend(samples.iter().map(|&x| {
        let v = (x.clamp(-1.0, 1.0) * 32_767.0).round();
        v as i16
    }));
}

/// The audio of one export: interleaved floats at `rate`, `channels` of them (1 or 2).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Pcm {
    pub rate: u32,
    pub channels: u16,
    pub samples: Vec<f32>,
}

impl Pcm {
    /// Silence of `frames` samples.
    pub fn silence(rate: u32, channels: u16, frames: usize) -> Pcm {
        Pcm {
            rate,
            channels,
            samples: vec![0.0; frames * channels as usize],
        }
    }

    pub fn frames(&self) -> usize {
        self.samples.len() / self.channels.max(1) as usize
    }

    /// Samples `range` as interleaved stereo floats: mono doubled, past the end silent (a
    /// song shorter than its video ends in silence).
    pub fn stereo(&self, range: std::ops::Range<u64>, out: &mut Vec<f32>) {
        out.clear();
        let ch = self.channels.max(1) as usize;
        for i in range {
            let i = i as usize;
            let (l, r) = if i < self.frames() {
                let l = self.samples[i * ch];
                (l, if ch > 1 { self.samples[i * ch + 1] } else { l })
            } else {
                (0.0, 0.0)
            };
            out.push(l);
            out.push(r);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aac_lc_configs() {
        assert_eq!(aac_lc_config(48_000, 2), Some([0x11, 0x90]));
        assert_eq!(aac_lc_config(44_100, 2), Some([0x12, 0x10]));
        assert_eq!(aac_lc_config(44_100, 1), Some([0x12, 0x08]));
        assert_eq!(aac_lc_config(47_000, 2), None);
    }

    #[test]
    fn floats_become_rounded_clipped_integers() {
        let mut o = Vec::new();
        to_i16(&[0.0, 1.0, -1.0, 2.0, 0.5, -0.000_01], &mut o);
        assert_eq!(o, vec![0, 32_767, -32_767, 32_767, 16_384, 0]);
    }

    #[test]
    fn mono_is_doubled_and_the_end_is_silent() {
        let p = Pcm {
            rate: 48_000,
            channels: 1,
            samples: vec![0.1, 0.2],
        };
        let mut o = Vec::new();
        p.stereo(1..4, &mut o);
        assert_eq!(o, vec![0.2, 0.2, 0.0, 0.0, 0.0, 0.0]);
    }
}

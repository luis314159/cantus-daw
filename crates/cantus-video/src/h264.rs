//! H.264: what an encoder gives (Annex B access units) and what an MP4 keeps of it.
//!
//! Every H.264 encoder the export drives — Media Foundation's, later Cisco's OpenH264 and
//! VideoToolbox — is a [`H264Encoder`]: frames in, in order; access units out, in order,
//! one for each frame (no B-frames, so decode order is display order and the MP4 needs no
//! `ctts` and no video edit list).

use crate::yuv::Nv12;
use crate::VideoError;

/// An H.264 encoder: NV12 frames in, Annex B access units out.
pub trait H264Encoder {
    /// Which encoder it is, for the report ("Media Foundation H.264 (software)").
    fn name(&self) -> String;
    /// Encode the next frame; any access units ready are pushed to `out`.
    fn encode(&mut self, frame: &Nv12, out: &mut Vec<Vec<u8>>) -> Result<(), VideoError>;
    /// No more frames: the access units still inside are pushed to `out`.
    fn finish(&mut self, out: &mut Vec<Vec<u8>>) -> Result<(), VideoError>;
}

/// NAL unit types the writer looks at.
pub const NAL_IDR: u8 = 5;
pub const NAL_SPS: u8 = 7;
pub const NAL_PPS: u8 = 8;
pub const NAL_AUD: u8 = 9;

/// The NAL units of an Annex B stream (start codes of three or four bytes; trailing zeros
/// of a unit dropped).
pub fn nal_units(annexb: &[u8]) -> Vec<&[u8]> {
    let mut out = Vec::new();
    let mut i = 0;
    let mut start: Option<usize> = None;
    while i + 3 <= annexb.len() {
        if annexb[i] == 0 && annexb[i + 1] == 0 && annexb[i + 2] == 1 {
            if let Some(s) = start {
                let mut e = i;
                while e > s && annexb[e - 1] == 0 {
                    e -= 1;
                }
                if e > s {
                    out.push(&annexb[s..e]);
                }
            }
            i += 3;
            start = Some(i);
        } else {
            i += 1;
        }
    }
    if let Some(s) = start {
        // A unit ends in its stop bit, never in a zero byte: zeros after it are padding.
        let mut e = annexb.len();
        while e > s && annexb[e - 1] == 0 {
            e -= 1;
        }
        if e > s {
            out.push(&annexb[s..e]);
        }
    }
    out
}

/// One access unit as an MP4 sample: its NAL units with 4-byte lengths, parameter sets and
/// delimiters left out (they go in `avcC`).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Sample {
    pub data: Vec<u8>,
    pub idr: bool,
    pub sps: Option<Vec<u8>>,
    pub pps: Option<Vec<u8>>,
}

/// An Annex B access unit as an MP4 sample.
pub fn to_sample(annexb: &[u8]) -> Sample {
    let mut s = Sample::default();
    for n in nal_units(annexb) {
        match n[0] & 0x1f {
            NAL_SPS => s.sps = Some(n.to_vec()),
            NAL_PPS => s.pps = Some(n.to_vec()),
            NAL_AUD => {}
            t => {
                s.idr |= t == NAL_IDR;
                s.data.extend_from_slice(&(n.len() as u32).to_be_bytes());
                s.data.extend_from_slice(n);
            }
        }
    }
    s
}

/// The `avcC` box body (AVCDecoderConfigurationRecord) for one SPS and one PPS; High
/// profiles carry the 4:2:0 8-bit extension.
pub fn avcc(sps: &[u8], pps: &[u8]) -> Vec<u8> {
    let profile = sps.get(1).copied().unwrap_or(66);
    let mut v = vec![
        1,
        profile,
        sps.get(2).copied().unwrap_or(0),
        sps.get(3).copied().unwrap_or(0),
        0xff, // 4-byte NAL lengths
        0xe1, // one SPS
    ];
    v.extend_from_slice(&(sps.len() as u16).to_be_bytes());
    v.extend_from_slice(sps);
    v.push(1);
    v.extend_from_slice(&(pps.len() as u16).to_be_bytes());
    v.extend_from_slice(pps);
    if matches!(profile, 100 | 110 | 122 | 144) {
        // chroma_format_idc 1 (4:2:0), 8-bit luma and chroma, no SPS extensions.
        v.extend_from_slice(&[0xfc | 1, 0xf8, 0xf8, 0]);
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn annex_b_splits_into_units_and_becomes_a_sample() {
        let au = [
            0, 0, 0, 1, 0x09, 0xf0, // AUD
            0, 0, 0, 1, 0x67, 100, 0, 40, 0xaa, // SPS
            0, 0, 1, 0x68, 0xbb, // PPS
            0, 0, 1, 0x65, 1, 2, 3, 0, 0, // IDR slice with trailing zeros
        ];
        let units = nal_units(&au);
        assert_eq!(units.len(), 4);
        let s = to_sample(&au);
        assert!(s.idr);
        assert_eq!(s.sps.as_deref(), Some(&[0x67, 100, 0, 40, 0xaa][..]));
        assert_eq!(s.pps.as_deref(), Some(&[0x68, 0xbb][..]));
        assert_eq!(s.data, vec![0, 0, 0, 4, 0x65, 1, 2, 3]);
        let c = avcc(s.sps.as_ref().unwrap(), s.pps.as_ref().unwrap());
        assert_eq!(&c[..6], &[1, 100, 0, 40, 0xff, 0xe1]);
        assert_eq!(&c[c.len() - 4..], &[0xfd, 0xf8, 0xf8, 0]);
    }

    #[test]
    fn a_p_frame_is_not_a_sync_sample() {
        let s = to_sample(&[0, 0, 1, 0x41, 9, 9]);
        assert!(!s.idr);
        assert_eq!(s.data, vec![0, 0, 0, 3, 0x41, 9, 9]);
    }
}

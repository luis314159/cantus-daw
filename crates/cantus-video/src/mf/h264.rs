//! The Microsoft H.264 encoder transform (software, synchronous).

use super::{create, fail, sample, Mft};
use crate::h264::H264Encoder;
use crate::yuv::Nv12;
use crate::VideoError;
use windows::core::{Interface, GUID};
use windows::Win32::Media::MediaFoundation::*;
use windows::Win32::System::Variant::VARIANT;

const CODEC: &str = "H.264";

/// Media Foundation's H.264 encoder: NV12 in, Annex B out, one access unit a frame.
pub struct MfH264 {
    mft: Mft,
    width: u32,
    height: u32,
    fps: u32,
    frame: u64,
}

fn pack(a: u32, b: u32) -> u64 {
    (a as u64) << 32 | b as u64
}

/// A video type of `subtype` at the export's size and rate, BT.709 limited range.
fn video_type(subtype: &GUID, w: u32, h: u32, fps: u32) -> windows::core::Result<IMFMediaType> {
    // SAFETY: attributes set on a new media type.
    unsafe {
        let t = super::mf()?.media_type()?;
        t.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
        t.SetGUID(&MF_MT_SUBTYPE, subtype)?;
        t.SetUINT64(&MF_MT_FRAME_SIZE, pack(w, h))?;
        t.SetUINT64(&MF_MT_FRAME_RATE, pack(fps, 1))?;
        t.SetUINT64(&MF_MT_PIXEL_ASPECT_RATIO, pack(1, 1))?;
        t.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)?;
        t.SetUINT32(&MF_MT_VIDEO_PRIMARIES, MFVideoPrimaries_BT709.0 as u32)?;
        t.SetUINT32(&MF_MT_TRANSFER_FUNCTION, MFVideoTransFunc_709.0 as u32)?;
        t.SetUINT32(&MF_MT_YUV_MATRIX, MFVideoTransferMatrix_BT709.0 as u32)?;
        t.SetUINT32(&MF_MT_VIDEO_NOMINAL_RANGE, MFNominalRange_16_235.0 as u32)?;
        Ok(t)
    }
}

impl MfH264 {
    /// An encoder for `width` × `height` (even) at `fps`, aiming at `bitrate` bits a second.
    pub fn new(width: u32, height: u32, fps: u32, bitrate: u32) -> Result<MfH264, VideoError> {
        let t = create(&CLSID_MSH264EncoderMFT, CODEC)?;
        let api: ICodecAPI = t.cast().map_err(|e| fail(CODEC, e))?;
        let set = |key: &GUID, value: u32| {
            // SAFETY: a codec property set to a 32-bit unsigned value.
            unsafe { api.SetValue(key, &VARIANT::from(value)) }
        };
        // No B-frames: decode order is display order, frame n is time n (ADR 0088 §6).
        set(&CODECAPI_AVEncMPVDefaultBPictureCount, 0).map_err(|e| fail(CODEC, e))?;
        // A key frame every two seconds (what platforms ask for).
        set(&CODECAPI_AVEncMPVGOPSize, fps * 2).map_err(|e| fail(CODEC, e))?;
        // Variable bitrate around the mean; quality over speed. Older encoders without a
        // property keep their default rather than fail.
        // Its worker threads: up to 8 (measured at 1080×1920: as fast as its default of
        // one a processor thread here, with 200 MB less memory — ADR 0088's amendment).
        let threads = std::thread::available_parallelism()
            .map_or(4, |n| n.get())
            .min(8);
        let props = [
            (
                &CODECAPI_AVEncCommonRateControlMode,
                eAVEncCommonRateControlMode_UnconstrainedVBR.0 as u32,
            ),
            (&CODECAPI_AVEncCommonMeanBitRate, bitrate),
            (&CODECAPI_AVEncCommonQualityVsSpeed, 100),
            (&CODECAPI_AVEncNumWorkerThreads, threads as u32),
        ];
        for (key, value) in props {
            if let Err(e) = set(key, value) {
                log::warn!("H.264 encoder property {key:?} refused: {}", e.message());
            }
        }
        // SAFETY: the output type first, then the input (the H.264 encoder's order).
        unsafe {
            let out = video_type(&MFVideoFormat_H264, width, height, fps)?;
            out.SetUINT32(&MF_MT_AVG_BITRATE, bitrate)?;
            out.SetUINT32(&MF_MT_MPEG2_PROFILE, eAVEncH264VProfile_High.0 as u32)?;
            t.SetOutputType(0, &out, 0).map_err(|e| fail(CODEC, e))?;
            let input = video_type(&MFVideoFormat_NV12, width, height, fps)?;
            t.SetInputType(0, &input, 0).map_err(|e| fail(CODEC, e))?;
        }
        let mft = Mft::new(t, CODEC, width * height * 2)?;
        Ok(MfH264 {
            mft,
            width,
            height,
            fps,
            frame: 0,
        })
    }

    fn time(&self, n: u64) -> i64 {
        (n * 10_000_000 / self.fps as u64) as i64
    }
}

impl H264Encoder for MfH264 {
    fn name(&self) -> String {
        "Media Foundation H.264 (software)".into()
    }

    fn encode(&mut self, frame: &Nv12, out: &mut Vec<Vec<u8>>) -> Result<(), VideoError> {
        if (frame.width, frame.height) != (self.width, self.height) {
            return Err(VideoError::encoder(CODEC, "a frame of another size"));
        }
        let (t0, t1) = (self.time(self.frame), self.time(self.frame + 1));
        let s = sample(&frame.data, t0, t1 - t0).map_err(|e| fail(CODEC, e))?;
        self.mft.push(&s, out)?;
        self.frame += 1;
        Ok(())
    }

    fn finish(&mut self, out: &mut Vec<Vec<u8>>) -> Result<(), VideoError> {
        self.mft.drain(out)
    }
}

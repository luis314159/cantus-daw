//! VideoToolbox's H.264 encoder (macOS), through its C interface — self-contained (only
//! `std`), so it can be compiled for Apple targets on any machine:
//! `rustc --edition 2021 --crate-type rlib --target aarch64-apple-darwin src/apple/vt.rs`.
//!
//! A compression session takes NV12 (`420v`, video range) pixel buffers and gives AVCC
//! samples through a callback; they come out here as Annex B access units, the parameter
//! sets put in front of each key frame — what [`crate::mp4`] takes from every encoder.
//! High profile, no frame reordering (no B-frames), a key frame every two seconds,
//! BT.709 said in the stream, not real time (quality over speed).

#![allow(non_upper_case_globals, non_snake_case, clippy::missing_safety_doc)]

use std::ffi::c_void;
use std::sync::{Arc, Mutex};

pub type OSStatus = i32;
pub type CFTypeRef = *const c_void;
pub type CFStringRef = *const c_void;
pub type CFDictionaryRef = *const c_void;
pub type CFArrayRef = *const c_void;
pub type CVPixelBufferRef = *mut c_void;
pub type CMSampleBufferRef = *mut c_void;
pub type VTCompressionSessionRef = *mut c_void;

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CMTime {
    pub value: i64,
    pub timescale: i32,
    pub flags: u32,
    pub epoch: i64,
}

type OutputCallback = extern "C" fn(*mut c_void, *mut c_void, OSStatus, u32, CMSampleBufferRef);

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    pub fn CFRelease(cf: CFTypeRef);
    fn CFNumberCreate(allocator: *const c_void, kind: isize, value: *const c_void) -> CFTypeRef;
    fn CFArrayGetCount(a: CFArrayRef) -> isize;
    fn CFArrayGetValueAtIndex(a: CFArrayRef, i: isize) -> *const c_void;
    fn CFDictionaryGetValue(d: CFDictionaryRef, key: *const c_void) -> *const c_void;
    static kCFBooleanFalse: CFTypeRef;
}

#[link(name = "CoreMedia", kind = "framework")]
extern "C" {
    fn CMTimeMake(value: i64, timescale: i32) -> CMTime;
    static kCMTimeInvalid: CMTime;
    fn CMSampleBufferGetDataBuffer(s: CMSampleBufferRef) -> *mut c_void;
    fn CMBlockBufferGetDataLength(b: *mut c_void) -> usize;
    fn CMBlockBufferCopyDataBytes(
        b: *mut c_void,
        at: usize,
        len: usize,
        to: *mut c_void,
    ) -> OSStatus;
    fn CMSampleBufferGetFormatDescription(s: CMSampleBufferRef) -> *const c_void;
    fn CMVideoFormatDescriptionGetH264ParameterSetAtIndex(
        desc: *const c_void,
        index: usize,
        set: *mut *const u8,
        size: *mut usize,
        count: *mut usize,
        nal_length: *mut i32,
    ) -> OSStatus;
    fn CMSampleBufferGetSampleAttachmentsArray(s: CMSampleBufferRef, create: u8) -> CFArrayRef;
    static kCMSampleAttachmentKey_NotSync: CFStringRef;
}

#[link(name = "CoreVideo", kind = "framework")]
extern "C" {
    fn CVPixelBufferCreate(
        allocator: *const c_void,
        width: usize,
        height: usize,
        format: u32,
        attributes: CFDictionaryRef,
        out: *mut CVPixelBufferRef,
    ) -> i32;
    fn CVPixelBufferLockBaseAddress(pb: CVPixelBufferRef, flags: u64) -> i32;
    fn CVPixelBufferUnlockBaseAddress(pb: CVPixelBufferRef, flags: u64) -> i32;
    fn CVPixelBufferGetBaseAddressOfPlane(pb: CVPixelBufferRef, plane: usize) -> *mut u8;
    fn CVPixelBufferGetBytesPerRowOfPlane(pb: CVPixelBufferRef, plane: usize) -> usize;
    static kCVImageBufferColorPrimaries_ITU_R_709_2: CFStringRef;
    static kCVImageBufferTransferFunction_ITU_R_709_2: CFStringRef;
    static kCVImageBufferYCbCrMatrix_ITU_R_709_2: CFStringRef;
}

#[link(name = "VideoToolbox", kind = "framework")]
extern "C" {
    fn VTCompressionSessionCreate(
        allocator: *const c_void,
        width: i32,
        height: i32,
        codec: u32,
        encoder_spec: CFDictionaryRef,
        source_attrs: CFDictionaryRef,
        data_allocator: *const c_void,
        callback: Option<OutputCallback>,
        refcon: *mut c_void,
        out: *mut VTCompressionSessionRef,
    ) -> OSStatus;
    fn VTSessionSetProperty(
        s: VTCompressionSessionRef,
        key: CFStringRef,
        value: CFTypeRef,
    ) -> OSStatus;
    fn VTCompressionSessionPrepareToEncodeFrames(s: VTCompressionSessionRef) -> OSStatus;
    fn VTCompressionSessionEncodeFrame(
        s: VTCompressionSessionRef,
        image: CVPixelBufferRef,
        pts: CMTime,
        duration: CMTime,
        props: CFDictionaryRef,
        refcon: *mut c_void,
        info: *mut u32,
    ) -> OSStatus;
    fn VTCompressionSessionCompleteFrames(s: VTCompressionSessionRef, until: CMTime) -> OSStatus;
    fn VTCompressionSessionInvalidate(s: VTCompressionSessionRef);
    static kVTCompressionPropertyKey_RealTime: CFStringRef;
    static kVTCompressionPropertyKey_ProfileLevel: CFStringRef;
    static kVTProfileLevel_H264_High_AutoLevel: CFStringRef;
    static kVTCompressionPropertyKey_AllowFrameReordering: CFStringRef;
    static kVTCompressionPropertyKey_AverageBitRate: CFStringRef;
    static kVTCompressionPropertyKey_MaxKeyFrameInterval: CFStringRef;
    static kVTCompressionPropertyKey_ExpectedFrameRate: CFStringRef;
    static kVTCompressionPropertyKey_ColorPrimaries: CFStringRef;
    static kVTCompressionPropertyKey_TransferFunction: CFStringRef;
    static kVTCompressionPropertyKey_YCbCrMatrix: CFStringRef;
}

/// 'avc1'.
const CODEC_H264: u32 = u32::from_be_bytes(*b"avc1");
/// '420v': 4:2:0 bi-planar, video range — NV12 as [`crate::yuv`] writes it.
const NV12_VIDEO_RANGE: u32 = u32::from_be_bytes(*b"420v");
const CF_NUMBER_SINT32: isize = 3;

/// What the callback collects, shared with the session's threads.
#[derive(Default)]
struct Collected {
    units: Vec<Vec<u8>>,
    error: Option<OSStatus>,
}

/// A VideoToolbox H.264 session.
pub struct VtSession {
    session: VTCompressionSessionRef,
    out: Arc<Mutex<Collected>>,
    /// The callback's handle on `out` (one strong count, released on drop).
    refcon: *const Mutex<Collected>,
    width: usize,
    height: usize,
    fps: i32,
    frame: i64,
}

fn check(what: &str, s: OSStatus) -> Result<(), String> {
    if s == 0 {
        Ok(())
    } else {
        Err(format!("{what}: OSStatus {s}"))
    }
}

/// One encoded sample from the session, as Annex B.
extern "C" fn on_output(
    refcon: *mut c_void,
    _frame: *mut c_void,
    status: OSStatus,
    _flags: u32,
    sample: CMSampleBufferRef,
) {
    // SAFETY: `refcon` is the Mutex the session was made with, alive until it is invalidated.
    let out = unsafe { &*(refcon as *const Mutex<Collected>) };
    let mut out = out.lock().unwrap_or_else(|e| e.into_inner());
    if status != 0 || sample.is_null() {
        out.error = Some(if status != 0 { status } else { -1 });
        return;
    }
    // SAFETY: the sample buffer is valid for the duration of the callback.
    match unsafe { annex_b(sample) } {
        Ok(au) => out.units.push(au),
        Err(s) => out.error = Some(s),
    }
}

/// The sample's AVCC data as Annex B, with SPS and PPS in front of a key frame.
unsafe fn annex_b(sample: CMSampleBufferRef) -> Result<Vec<u8>, OSStatus> {
    let block = CMSampleBufferGetDataBuffer(sample);
    let len = CMBlockBufferGetDataLength(block);
    let mut avcc = vec![0u8; len];
    let s = CMBlockBufferCopyDataBytes(block, 0, len, avcc.as_mut_ptr() as *mut c_void);
    if s != 0 {
        return Err(s);
    }
    let mut key = true;
    let attachments = CMSampleBufferGetSampleAttachmentsArray(sample, 0);
    if !attachments.is_null() && CFArrayGetCount(attachments) > 0 {
        let d = CFArrayGetValueAtIndex(attachments, 0);
        let not_sync = CFDictionaryGetValue(d, kCMSampleAttachmentKey_NotSync);
        key = not_sync.is_null() || not_sync == kCFBooleanFalse;
    }
    let mut out = Vec::with_capacity(len + 64);
    let desc = CMSampleBufferGetFormatDescription(sample);
    let mut nal_len: i32 = 4;
    if key {
        for i in 0..2 {
            let (mut p, mut n, mut count) = (std::ptr::null(), 0usize, 0usize);
            let s = CMVideoFormatDescriptionGetH264ParameterSetAtIndex(
                desc,
                i,
                &mut p,
                &mut n,
                &mut count,
                &mut nal_len,
            );
            if s != 0 {
                return Err(s);
            }
            out.extend_from_slice(&[0, 0, 0, 1]);
            out.extend_from_slice(std::slice::from_raw_parts(p, n));
        }
    }
    let width = nal_len.clamp(1, 4) as usize;
    let mut at = 0;
    while at + width <= avcc.len() {
        let mut n = 0usize;
        for b in &avcc[at..at + width] {
            n = n << 8 | *b as usize;
        }
        at += width;
        let end = (at + n).min(avcc.len());
        out.extend_from_slice(&[0, 0, 0, 1]);
        out.extend_from_slice(&avcc[at..end]);
        at = end;
    }
    Ok(out)
}

unsafe fn number(v: i32) -> CFTypeRef {
    CFNumberCreate(
        std::ptr::null(),
        CF_NUMBER_SINT32,
        &v as *const i32 as *const c_void,
    )
}

unsafe fn set_number(s: VTCompressionSessionRef, key: CFStringRef, v: i32) -> OSStatus {
    let n = number(v);
    let r = VTSessionSetProperty(s, key, n);
    CFRelease(n);
    r
}

impl VtSession {
    pub fn new(width: u32, height: u32, fps: u32, bitrate: u32) -> Result<VtSession, String> {
        let out = Arc::new(Mutex::new(Collected::default()));
        let refcon = Arc::into_raw(out.clone());
        let mut session: VTCompressionSessionRef = std::ptr::null_mut();
        // SAFETY: documented VideoToolbox calls; the session holds `refcon` until invalidated.
        unsafe {
            let s = VTCompressionSessionCreate(
                std::ptr::null(),
                width as i32,
                height as i32,
                CODEC_H264,
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                Some(on_output),
                refcon as *mut c_void,
                &mut session,
            );
            if s != 0 || session.is_null() {
                drop(Arc::from_raw(refcon));
                return Err(format!("VTCompressionSessionCreate: OSStatus {s}"));
            }
            let props: [(CFStringRef, CFTypeRef); 6] = [
                (kVTCompressionPropertyKey_RealTime, kCFBooleanFalse),
                (
                    kVTCompressionPropertyKey_ProfileLevel,
                    kVTProfileLevel_H264_High_AutoLevel,
                ),
                (
                    kVTCompressionPropertyKey_AllowFrameReordering,
                    kCFBooleanFalse,
                ),
                (
                    kVTCompressionPropertyKey_ColorPrimaries,
                    kCVImageBufferColorPrimaries_ITU_R_709_2,
                ),
                (
                    kVTCompressionPropertyKey_TransferFunction,
                    kCVImageBufferTransferFunction_ITU_R_709_2,
                ),
                (
                    kVTCompressionPropertyKey_YCbCrMatrix,
                    kCVImageBufferYCbCrMatrix_ITU_R_709_2,
                ),
            ];
            let mut result = Ok(());
            for (k, v) in props {
                result = result.and(check(
                    "VTSessionSetProperty",
                    VTSessionSetProperty(session, k, v),
                ));
            }
            result = result
                .and(check(
                    "bitrate",
                    set_number(
                        session,
                        kVTCompressionPropertyKey_AverageBitRate,
                        bitrate as i32,
                    ),
                ))
                .and(check(
                    "key frames",
                    set_number(
                        session,
                        kVTCompressionPropertyKey_MaxKeyFrameInterval,
                        fps as i32 * 2,
                    ),
                ))
                .and(check(
                    "frame rate",
                    set_number(
                        session,
                        kVTCompressionPropertyKey_ExpectedFrameRate,
                        fps as i32,
                    ),
                ))
                .and(check(
                    "prepare",
                    VTCompressionSessionPrepareToEncodeFrames(session),
                ));
            let me = VtSession {
                session,
                out,
                refcon,
                width: width as usize,
                height: height as usize,
                fps: fps as i32,
                frame: 0,
            };
            result.map(|_| me)
        }
    }

    fn take(&self, out: &mut Vec<Vec<u8>>) -> Result<(), String> {
        let mut c = self.out.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(e) = c.error {
            return Err(format!("the encoder: OSStatus {e}"));
        }
        out.append(&mut c.units);
        Ok(())
    }

    /// Encode an NV12 frame (`width × height` of Y, then interleaved U, V).
    pub fn encode(&mut self, nv12: &[u8], out: &mut Vec<Vec<u8>>) -> Result<(), String> {
        let (w, h) = (self.width, self.height);
        if nv12.len() != w * h * 3 / 2 {
            return Err("a frame of another size".into());
        }
        // SAFETY: a new pixel buffer, written while locked within its planes' rows.
        unsafe {
            let mut pb: CVPixelBufferRef = std::ptr::null_mut();
            let s = CVPixelBufferCreate(
                std::ptr::null(),
                w,
                h,
                NV12_VIDEO_RANGE,
                std::ptr::null(),
                &mut pb,
            );
            if s != 0 || pb.is_null() {
                return Err(format!("CVPixelBufferCreate: {s}"));
            }
            CVPixelBufferLockBaseAddress(pb, 0);
            for (plane, rows, src) in [(0usize, h, &nv12[..w * h]), (1, h / 2, &nv12[w * h..])] {
                let base = CVPixelBufferGetBaseAddressOfPlane(pb, plane);
                let stride = CVPixelBufferGetBytesPerRowOfPlane(pb, plane);
                for r in 0..rows {
                    std::ptr::copy_nonoverlapping(src[r * w..].as_ptr(), base.add(r * stride), w);
                }
            }
            CVPixelBufferUnlockBaseAddress(pb, 0);
            let pts = CMTimeMake(self.frame, self.fps);
            let dur = CMTimeMake(1, self.fps);
            let s = VTCompressionSessionEncodeFrame(
                self.session,
                pb,
                pts,
                dur,
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            );
            CFRelease(pb as CFTypeRef);
            check("VTCompressionSessionEncodeFrame", s)?;
        }
        self.frame += 1;
        self.take(out)
    }

    /// Every frame out.
    pub fn finish(&mut self, out: &mut Vec<Vec<u8>>) -> Result<(), String> {
        // SAFETY: completes every pending frame; the callback has run for each on return.
        unsafe {
            check(
                "complete",
                VTCompressionSessionCompleteFrames(self.session, kCMTimeInvalid),
            )?
        };
        self.take(out)
    }
}

impl Drop for VtSession {
    fn drop(&mut self) {
        // SAFETY: the session stops calling back once invalidated; then its handle goes.
        unsafe {
            VTCompressionSessionInvalidate(self.session);
            CFRelease(self.session as CFTypeRef);
            drop(Arc::from_raw(self.refcon));
        }
    }
}

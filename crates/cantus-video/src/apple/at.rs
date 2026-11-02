//! AudioToolbox's AAC-LC encoder (macOS), through its C interface — self-contained, so it
//! compiles for Apple targets anywhere:
//! `rustc --edition 2021 --crate-type rlib --target aarch64-apple-darwin src/apple/at.rs`.
//!
//! An `AudioConverter` from 16-bit PCM to AAC-LC: PCM is handed to its input callback as
//! it arrives, one packet (1 024 samples) is asked for at a time, and the callback answers
//! "nothing yet" when it has given all it holds — until the end, when it answers "no more"
//! and the converter gives its last packets. Its priming (`kAudioConverterPrimeInfo`,
//! usually 2 112 samples) is the MP4 edit list's start.

#![allow(non_upper_case_globals, non_snake_case, clippy::missing_safety_doc)]

use std::ffi::c_void;

type OSStatus = i32;
type AudioConverterRef = *mut c_void;

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Asbd {
    sample_rate: f64,
    format: u32,
    flags: u32,
    bytes_per_packet: u32,
    frames_per_packet: u32,
    bytes_per_frame: u32,
    channels: u32,
    bits: u32,
    reserved: u32,
}

#[repr(C)]
struct AudioBuffer {
    channels: u32,
    size: u32,
    data: *mut c_void,
}

#[repr(C)]
struct AudioBufferList {
    count: u32,
    buffers: [AudioBuffer; 1],
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct PacketDescription {
    start: i64,
    variable_frames: u32,
    size: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct PrimeInfo {
    leading: u32,
    trailing: u32,
}

type InputProc = extern "C" fn(
    AudioConverterRef,
    *mut u32,
    *mut AudioBufferList,
    *mut *mut PacketDescription,
    *mut c_void,
) -> OSStatus;

#[link(name = "AudioToolbox", kind = "framework")]
extern "C" {
    fn AudioConverterNew(
        from: *const Asbd,
        to: *const Asbd,
        out: *mut AudioConverterRef,
    ) -> OSStatus;
    fn AudioConverterDispose(c: AudioConverterRef) -> OSStatus;
    fn AudioConverterSetProperty(
        c: AudioConverterRef,
        id: u32,
        size: u32,
        data: *const c_void,
    ) -> OSStatus;
    fn AudioConverterGetProperty(
        c: AudioConverterRef,
        id: u32,
        size: *mut u32,
        data: *mut c_void,
    ) -> OSStatus;
    fn AudioConverterFillComplexBuffer(
        c: AudioConverterRef,
        input: InputProc,
        user: *mut c_void,
        packets: *mut u32,
        out: *mut AudioBufferList,
        descriptions: *mut PacketDescription,
    ) -> OSStatus;
}

const LPCM: u32 = u32::from_be_bytes(*b"lpcm");
const AAC: u32 = u32::from_be_bytes(*b"aac ");
const SIGNED_PACKED: u32 = 4 | 8;
const AAC_LC: u32 = 2;
const ENCODE_BIT_RATE: u32 = u32::from_be_bytes(*b"brat");
const PRIME_INFO: u32 = u32::from_be_bytes(*b"prim");
const MAX_PACKET: u32 = u32::from_be_bytes(*b"xops");
/// Our input callback's "nothing more just now" (any non-zero status the converter passes
/// back without treating as an error of its own).
const WAIT: OSStatus = 0x7761_6974; // 'wait'

/// What the input callback reads.
struct Feed {
    pending: Vec<i16>,
    /// What was handed to the converter last: kept alive until the next call.
    given: Vec<i16>,
    channels: usize,
    end: bool,
}

extern "C" fn on_input(
    _c: AudioConverterRef,
    packets: *mut u32,
    data: *mut AudioBufferList,
    _desc: *mut *mut PacketDescription,
    user: *mut c_void,
) -> OSStatus {
    // SAFETY: `user` is the `Feed` given to FillComplexBuffer, alive for its call; the
    // buffer list has one buffer, as the input format is interleaved.
    unsafe {
        let feed = &mut *(user as *mut Feed);
        if feed.pending.is_empty() {
            *packets = 0;
            (*data).buffers[0].size = 0;
            (*data).buffers[0].data = std::ptr::null_mut();
            return if feed.end { 0 } else { WAIT };
        }
        feed.given = std::mem::take(&mut feed.pending);
        let frames = feed.given.len() / feed.channels;
        *packets = frames as u32;
        (*data).buffers[0].channels = feed.channels as u32;
        (*data).buffers[0].size = (feed.given.len() * 2) as u32;
        (*data).buffers[0].data = feed.given.as_mut_ptr() as *mut c_void;
        0
    }
}

/// An AudioToolbox AAC-LC converter.
pub struct AtConverter {
    conv: AudioConverterRef,
    feed: Box<Feed>,
    out_buf: Vec<u8>,
    /// Samples of priming before the first input sample.
    pub priming: u32,
}

fn check(what: &str, s: OSStatus) -> Result<(), String> {
    if s == 0 {
        Ok(())
    } else {
        Err(format!("{what}: OSStatus {s}"))
    }
}

impl AtConverter {
    pub fn new(rate: u32, channels: u16, bitrate: u32) -> Result<AtConverter, String> {
        let ch = channels as u32;
        let from = Asbd {
            sample_rate: rate as f64,
            format: LPCM,
            flags: SIGNED_PACKED,
            bytes_per_packet: 2 * ch,
            frames_per_packet: 1,
            bytes_per_frame: 2 * ch,
            channels: ch,
            bits: 16,
            reserved: 0,
        };
        let to = Asbd {
            sample_rate: rate as f64,
            format: AAC,
            flags: AAC_LC,
            frames_per_packet: 1024,
            channels: ch,
            ..Asbd::default()
        };
        let mut conv: AudioConverterRef = std::ptr::null_mut();
        // SAFETY: documented AudioToolbox calls with stack values of the stated sizes.
        unsafe {
            check(
                "AudioConverterNew",
                AudioConverterNew(&from, &to, &mut conv),
            )?;
            let mut me = AtConverter {
                conv,
                feed: Box::new(Feed {
                    pending: Vec::new(),
                    given: Vec::new(),
                    channels: channels as usize,
                    end: false,
                }),
                out_buf: Vec::new(),
                priming: 0,
            };
            check(
                "bitrate",
                AudioConverterSetProperty(
                    conv,
                    ENCODE_BIT_RATE,
                    4,
                    &bitrate as *const u32 as *const c_void,
                ),
            )?;
            let mut prime = PrimeInfo::default();
            let mut size = std::mem::size_of::<PrimeInfo>() as u32;
            check(
                "prime info",
                AudioConverterGetProperty(
                    conv,
                    PRIME_INFO,
                    &mut size,
                    &mut prime as *mut PrimeInfo as *mut c_void,
                ),
            )?;
            me.priming = prime.leading;
            let mut max = 0u32;
            let mut size = 4u32;
            check(
                "packet size",
                AudioConverterGetProperty(
                    conv,
                    MAX_PACKET,
                    &mut size,
                    &mut max as *mut u32 as *mut c_void,
                ),
            )?;
            me.out_buf = vec![0u8; (max as usize).max(1536)];
            Ok(me)
        }
    }

    /// Ask for packets until the converter waits for more input (or, at the end, is done).
    fn pull(&mut self, out: &mut Vec<Vec<u8>>) -> Result<(), String> {
        loop {
            let mut packets = 1u32;
            let mut desc = PacketDescription::default();
            let mut list = AudioBufferList {
                count: 1,
                buffers: [AudioBuffer {
                    channels: self.feed.channels as u32,
                    size: self.out_buf.len() as u32,
                    data: self.out_buf.as_mut_ptr() as *mut c_void,
                }],
            };
            let feed: *mut Feed = &mut *self.feed;
            // SAFETY: one output buffer of its stated size and one packet description; the
            // feed lives in its box for the call.
            let s = unsafe {
                AudioConverterFillComplexBuffer(
                    self.conv,
                    on_input,
                    feed as *mut c_void,
                    &mut packets,
                    &mut list,
                    &mut desc,
                )
            };
            if packets > 0 {
                let n = (desc.size as usize).min(self.out_buf.len());
                let at = desc.start.max(0) as usize;
                out.push(self.out_buf[at..at + n.min(self.out_buf.len() - at)].to_vec());
            }
            if s == WAIT || (s == 0 && packets == 0) {
                return Ok(());
            }
            check("AudioConverterFillComplexBuffer", s)?;
        }
    }

    /// Encode interleaved 16-bit samples.
    pub fn encode(&mut self, pcm: &[i16], out: &mut Vec<Vec<u8>>) -> Result<(), String> {
        self.feed.pending.extend_from_slice(pcm);
        self.pull(out)
    }

    /// The end: the last packets.
    pub fn finish(&mut self, out: &mut Vec<Vec<u8>>) -> Result<(), String> {
        self.feed.end = true;
        self.pull(out)
    }
}

impl Drop for AtConverter {
    fn drop(&mut self) {
        // SAFETY: the converter was made by AudioConverterNew and is not used after this.
        unsafe {
            AudioConverterDispose(self.conv);
        }
    }
}

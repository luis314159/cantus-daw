//! Media Foundation's H.264 and AAC encoders, driven directly (ADR 0088 §1, Windows).
//!
//! The encoder transforms (MFTs) are licensed with Windows: Cantus distributes no encoder
//! and owes nothing. They are driven one sample at a time on the export thread — **not**
//! through Media Foundation's sink writer, which (measured by the round's probe) delays the
//! video a frame with B-frames, writes its index at the end, drops the AAC encoder's
//! priming and ignores the bitrate. Our [`crate::mp4`] writer takes what comes out.
//!
//! - [`MfH264`]: the Microsoft H.264 encoder (software, synchronous), High profile, no
//!   B-frames, a key frame every two seconds, BT.709 limited range said in the stream.
//! - [`MfAac`]: the Microsoft AAC encoder, AAC-LC at 192 kbit/s (its most), raw packets.
//!
//! **Windows N** (no Media Feature Pack) has neither: creating one answers "class not
//! registered", which is [`crate::VideoError::NoEncoder`] — the dialog says how to add the
//! free pack.
//!
//! COM and Media Foundation are started for the thread by [`Session`] and stopped when it
//! goes; every object here lives and dies on that one thread.

pub mod aac;
pub mod h264;
pub mod sys;

pub use aac::MfAac;
pub use h264::MfH264;

use crate::VideoError;
use std::mem::ManuallyDrop;
use windows::core::{GUID, HRESULT};
use windows::Win32::Media::MediaFoundation::*;
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED,
};

/// "Class not registered": the encoder is not on this system (Windows N).
const REGDB_E_CLASSNOTREG: HRESULT = HRESULT(0x8004_0154_u32 as i32);
/// COM already started on this thread in another mode: usable, not ours to stop.
const RPC_E_CHANGED_MODE: HRESULT = HRESULT(0x8001_0106_u32 as i32);

/// COM and Media Foundation started on this thread; stopped on drop.
pub struct Session {
    com: bool,
    /// Started for this thread: never sent to another.
    _thread: std::marker::PhantomData<*const ()>,
}

impl Session {
    pub fn start() -> Result<Session, VideoError> {
        // A system without Media Foundation says so here, before anything is started.
        let api = sys::api()?;
        // SAFETY: plain Win32 start-up calls on the current thread; balanced in `drop`.
        unsafe {
            let hr = CoInitializeEx(None, COINIT_MULTITHREADED);
            let com = hr.is_ok();
            if hr.is_err() && hr != RPC_E_CHANGED_MODE {
                return Err(VideoError::encoder("Media Foundation", hr.message()));
            }
            if let Err(e) = api.startup(MF_VERSION, MFSTARTUP_LITE) {
                if com {
                    CoUninitialize();
                }
                return Err(VideoError::encoder("Media Foundation", e.message()));
            }
            Ok(Session {
                com,
                _thread: std::marker::PhantomData,
            })
        }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        // SAFETY: balances `start` on the same thread (the session is not Send).
        unsafe {
            if let Ok(api) = sys::api() {
                let _ = api.shutdown();
            }
            if self.com {
                CoUninitialize();
            }
        }
    }
}

/// Whether this system has Media Foundation's H.264 and AAC encoders (asked once).
pub fn available() -> (bool, bool) {
    static AVAILABLE: std::sync::OnceLock<(bool, bool)> = std::sync::OnceLock::new();
    *AVAILABLE.get_or_init(|| {
        let Ok(_session) = Session::start() else {
            return (false, false);
        };
        // Each transform is released here, while COM is still up: in a tail expression its
        // temporary would outlive the session and be released after CoUninitialize (an
        // access violation, found by the dialog's test).
        let video = create(&CLSID_MSH264EncoderMFT, "H.264").is_ok();
        let audio = create(&aac::CLSID_AAC_ENCODER, "AAC").is_ok();
        (video, audio)
    })
}

/// An encoder transform being driven: its output stream's needs, and the codec's name.
struct Mft {
    t: IMFTransform,
    codec: &'static str,
    /// The transform gives its own output samples.
    provides: bool,
    /// Bytes an output sample needs, when we give it.
    out_size: u32,
}

fn fail(codec: &'static str, e: windows::core::Error) -> VideoError {
    VideoError::encoder(codec, format!("{} ({:#010x})", e.message(), e.code().0))
}

/// Create the transform `clsid`; "not registered" means the system has no such encoder.
fn create(clsid: &GUID, codec: &'static str) -> Result<IMFTransform, VideoError> {
    // SAFETY: an in-process COM object created on a thread where COM is started.
    unsafe { CoCreateInstance(clsid, None, CLSCTX_INPROC_SERVER) }.map_err(|e| {
        if e.code() == REGDB_E_CLASSNOTREG {
            VideoError::NoEncoder(codec)
        } else {
            fail(codec, e)
        }
    })
}

/// Media Foundation's functions (present: a [`Session`] could not start without them).
fn mf() -> windows::core::Result<&'static sys::Api> {
    sys::api().map_err(|_| windows::core::Error::from(windows::Win32::Foundation::E_NOTIMPL))
}

/// A media sample holding `bytes`, at `time` for `duration` (100 ns units).
fn sample(bytes: &[u8], time: i64, duration: i64) -> windows::core::Result<IMFSample> {
    // SAFETY: the buffer is locked for exactly its own length while copied into.
    unsafe {
        let buf = mf()?.memory_buffer(bytes.len().max(1) as u32)?;
        let mut p = std::ptr::null_mut();
        buf.Lock(&mut p, None, None)?;
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), p, bytes.len());
        buf.Unlock()?;
        buf.SetCurrentLength(bytes.len() as u32)?;
        let s = mf()?.sample()?;
        s.AddBuffer(&buf)?;
        s.SetSampleTime(time)?;
        s.SetSampleDuration(duration)?;
        Ok(s)
    }
}

/// The bytes of a sample, all its buffers as one.
fn bytes_of(s: &IMFSample) -> windows::core::Result<Vec<u8>> {
    // SAFETY: the buffer is read for its current length while locked.
    unsafe {
        let buf = s.ConvertToContiguousBuffer()?;
        let mut p = std::ptr::null_mut();
        let mut len = 0u32;
        buf.Lock(&mut p, None, Some(&mut len))?;
        let v = std::slice::from_raw_parts(p, len as usize).to_vec();
        buf.Unlock()?;
        Ok(v)
    }
}

impl Mft {
    fn new(t: IMFTransform, codec: &'static str, fallback_size: u32) -> Result<Mft, VideoError> {
        // SAFETY: a query of stream 0 of a transform whose types are set.
        let info = unsafe { t.GetOutputStreamInfo(0) }.map_err(|e| fail(codec, e))?;
        let provides = info.dwFlags
            & (MFT_OUTPUT_STREAM_PROVIDES_SAMPLES.0 | MFT_OUTPUT_STREAM_CAN_PROVIDE_SAMPLES.0)
                as u32
            != 0;
        let m = Mft {
            t,
            codec,
            provides,
            out_size: info.cbSize.max(fallback_size),
        };
        // SAFETY: streaming messages to a transform with its types set.
        unsafe {
            m.t.ProcessMessage(MFT_MESSAGE_NOTIFY_BEGIN_STREAMING, 0)
                .and_then(|_| m.t.ProcessMessage(MFT_MESSAGE_NOTIFY_START_OF_STREAM, 0))
                .map_err(|e| fail(codec, e))?;
        }
        Ok(m)
    }

    /// Give the transform `input`, taking out what it has ready whenever it is full.
    fn push(&self, input: &IMFSample, out: &mut Vec<Vec<u8>>) -> Result<(), VideoError> {
        for _ in 0..2 {
            // SAFETY: a sample made by `sample`, given to stream 0.
            match unsafe { self.t.ProcessInput(0, input, 0) } {
                Ok(()) => return self.pull(out),
                Err(e) if e.code() == MF_E_NOTACCEPTING => self.pull(out)?,
                Err(e) => return Err(fail(self.codec, e)),
            }
        }
        Err(VideoError::encoder(self.codec, "it takes no more input"))
    }

    /// An empty output sample of the size the transform asked for.
    fn out_sample(&self) -> windows::core::Result<IMFSample> {
        // SAFETY: a new sample with one new buffer.
        unsafe {
            let s = mf()?.sample()?;
            s.AddBuffer(&mf()?.memory_buffer(self.out_size)?)?;
            Ok(s)
        }
    }

    /// Take every output sample ready.
    fn pull(&self, out: &mut Vec<Vec<u8>>) -> Result<(), VideoError> {
        loop {
            let given = if self.provides {
                None
            } else {
                Some(self.out_sample().map_err(|e| fail(self.codec, e))?)
            };
            let mut buffers = [MFT_OUTPUT_DATA_BUFFER {
                dwStreamID: 0,
                pSample: ManuallyDrop::new(given),
                dwStatus: 0,
                pEvents: ManuallyDrop::new(None),
            }];
            let mut status = 0u32;
            // SAFETY: one output buffer for stream 0; what it holds after is taken below.
            let r = unsafe { self.t.ProcessOutput(0, &mut buffers, &mut status) };
            // SAFETY: each field is taken exactly once, so the references are released.
            let (s, _events) = unsafe {
                (
                    ManuallyDrop::take(&mut buffers[0].pSample),
                    ManuallyDrop::take(&mut buffers[0].pEvents),
                )
            };
            match r {
                Ok(()) => {
                    if let Some(s) = s {
                        out.push(bytes_of(&s).map_err(|e| fail(self.codec, e))?);
                    }
                }
                Err(e) if e.code() == MF_E_TRANSFORM_NEED_MORE_INPUT => return Ok(()),
                Err(e) if e.code() == MF_E_TRANSFORM_STREAM_CHANGE => {
                    // SAFETY: the transform offers its new output type; it is taken as is.
                    unsafe {
                        let t = self.t.GetOutputAvailableType(0, 0)?;
                        self.t.SetOutputType(0, &t, 0)?;
                    }
                }
                Err(e) => return Err(fail(self.codec, e)),
            }
        }
    }

    /// The end of the stream: everything still inside comes out.
    fn drain(&self, out: &mut Vec<Vec<u8>>) -> Result<(), VideoError> {
        // SAFETY: end-of-stream messages to a streaming transform.
        unsafe {
            self.t
                .ProcessMessage(MFT_MESSAGE_NOTIFY_END_OF_STREAM, 0)
                .and_then(|_| self.t.ProcessMessage(MFT_MESSAGE_COMMAND_DRAIN, 0))
                .map_err(|e| fail(self.codec, e))?;
        }
        self.pull(out)
    }
}

impl From<windows::core::Error> for VideoError {
    fn from(e: windows::core::Error) -> VideoError {
        VideoError::encoder("Media Foundation", e.message())
    }
}

/// Whether the process `pid` is running (the export's sweep of parts an ended Cantus left).
/// Access refused means it runs (another user's, or protected).
pub fn process_alive(pid: u32) -> bool {
    use windows::Win32::Foundation::{CloseHandle, ERROR_ACCESS_DENIED, STILL_ACTIVE};
    use windows::Win32::System::Threading::{
        GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    // SAFETY: a handle opened for a query, closed before returning.
    unsafe {
        match OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) {
            Ok(h) => {
                let mut code = 0u32;
                let running =
                    GetExitCodeProcess(h, &mut code).is_ok() && code == STILL_ACTIVE.0 as u32;
                let _ = CloseHandle(h);
                running
            }
            Err(e) => e.code() == ERROR_ACCESS_DENIED.to_hresult(),
        }
    }
}

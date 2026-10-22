//! Linux: the test editor on the host's `IRunLoop` (ADR 0065), as the SDK's editors do — a
//! timer, and a descriptor (one end of a socket pair with a byte waiting) whose handler
//! reads it. The counts are what `cantus_test_run_loop` reports, so a test sees the host
//! serve both from its frame loop; both are unregistered when the view is removed.

use std::io::{Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::net::UnixStream;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::Mutex;
use vst3::Steinberg::Linux::{
    FileDescriptor, IEventHandler, IEventHandlerTrait, IRunLoop, IRunLoopTrait, ITimerHandler,
    ITimerHandlerTrait,
};
use vst3::Steinberg::{kResultOk, IPlugFrame};
use vst3::{Class, ComPtr, ComWrapper};

static REGISTERED: AtomicI32 = AtomicI32::new(0);
static TICKS: AtomicI32 = AtomicI32::new(0);
static EVENTS: AtomicI32 = AtomicI32::new(0);

pub fn counts(ticks: *mut i32, events: *mut i32) -> i32 {
    // SAFETY: the caller passes valid out-pointers or null.
    unsafe {
        if !ticks.is_null() {
            *ticks = TICKS.load(Ordering::SeqCst);
        }
        if !events.is_null() {
            *events = EVENTS.load(Ordering::SeqCst);
        }
    }
    REGISTERED.load(Ordering::SeqCst)
}

struct Timer;

impl Class for Timer {
    type Interfaces = (ITimerHandler,);
}

impl ITimerHandlerTrait for Timer {
    unsafe fn onTimer(&self) {
        TICKS.fetch_add(1, Ordering::SeqCst);
    }
}

struct Event {
    stream: Mutex<UnixStream>,
}

impl Class for Event {
    type Interfaces = (IEventHandler,);
}

impl IEventHandlerTrait for Event {
    unsafe fn onFDIsSet(&self, _fd: FileDescriptor) {
        let mut byte = [0u8; 16];
        let mut s = self.stream.lock().unwrap_or_else(|e| e.into_inner());
        if s.read(&mut byte).is_ok_and(|n| n > 0) {
            EVENTS.fetch_add(1, Ordering::SeqCst);
        }
    }
}

/// The registrations of one attached view; dropping it unregisters them.
pub struct Registered {
    run_loop: ComPtr<IRunLoop>,
    timer: ComWrapper<Timer>,
    event: ComWrapper<Event>,
    _writer: UnixStream,
}

// SAFETY: the run loop and handlers are used on the host's main thread only.
unsafe impl Send for Registered {}

impl Registered {
    /// Ask the frame for the host's run loop and register a timer and a descriptor on it.
    pub fn on(frame: &ComPtr<IPlugFrame>) -> Option<Registered> {
        let run_loop = frame.cast::<IRunLoop>()?;
        let (reader, mut writer) = UnixStream::pair().ok()?;
        reader.set_nonblocking(true).ok()?;
        writer.write_all(&[1]).ok()?;
        let fd = reader.as_raw_fd();
        let timer = ComWrapper::new(Timer);
        let event = ComWrapper::new(Event {
            stream: Mutex::new(reader),
        });
        let t = timer.as_com_ref::<ITimerHandler>()?;
        let e = event.as_com_ref::<IEventHandler>()?;
        // SAFETY: the host's run loop, on the main thread, with live handlers.
        unsafe {
            if run_loop.registerTimer(t.as_ptr(), 20) != kResultOk
                || run_loop.registerEventHandler(e.as_ptr(), fd) != kResultOk
            {
                return None;
            }
        }
        REGISTERED.fetch_add(1, Ordering::SeqCst);
        Some(Registered {
            run_loop,
            timer,
            event,
            _writer: writer,
        })
    }
}

impl Drop for Registered {
    fn drop(&mut self) {
        // SAFETY: as in `on`.
        unsafe {
            if let Some(t) = self.timer.as_com_ref::<ITimerHandler>() {
                self.run_loop.unregisterTimer(t.as_ptr());
            }
            if let Some(e) = self.event.as_com_ref::<IEventHandler>() {
                self.run_loop.unregisterEventHandler(e.as_ptr());
            }
        }
        REGISTERED.fetch_sub(1, Ordering::SeqCst);
    }
}

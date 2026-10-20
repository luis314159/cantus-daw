//! The hidden **Fault** parameter (round 29, ADR 0063): failures on purpose, so the host's
//! isolation is tested against a plug-in that really crashes or hangs where third-party
//! ones do. A value set **while processing** (an event in `process`) arms a fault; the same
//! value arriving any other way (a flush, a state, a reload) arms nothing, so a plug-in the
//! host reloads after the crash comes back healthy. The twin of this file lives in every
//! test plug-in (the synth, the two VST3 ones).
//!
//! | Value | What happens |
//! |---|---|
//! | 1 | aborts in this `process` |
//! | 2 | never returns from this `process` (a hang) |
//! | 3 | aborts in the next `save_state` |
//! | 4 | aborts in the next `load_state` |
//! | 5 | aborts in the next `activate` (a restart) |
//! | 6 | aborts when its editor is next created |
//! | 7 | never returns from the next `save_state` |
//! | 8 | never returns from the next `load_state` |
//! | 9 | never returns when its editor is next created |
//! | 10 | the next `save_state` runs a nested message loop for 3 s, as a modal dialog does |
//! | 11 | this `process` takes 50 ms, once (late, not hung) |

use std::sync::atomic::{AtomicU32, Ordering};

pub const ABORT_PROCESS: u32 = 1;
pub const HANG_PROCESS: u32 = 2;
pub const ABORT_SAVE: u32 = 3;
pub const ABORT_LOAD: u32 = 4;
pub const ABORT_ACTIVATE: u32 = 5;
pub const ABORT_EDITOR: u32 = 6;
pub const HANG_SAVE: u32 = 7;
pub const HANG_LOAD: u32 = 8;
pub const HANG_EDITOR: u32 = 9;
pub const PUMP_SAVE: u32 = 10;
pub const SLOW_ONCE: u32 = 11;
/// The parameter's range: 0 (none) … this.
pub const LAST: f64 = 11.0;

/// The fault armed in this instance, 0 for none.
#[derive(Debug, Default)]
pub struct Fault(AtomicU32);

impl Fault {
    /// A value set while processing: arm it, and fire the ones that belong to `process`.
    pub fn arm(&self, value: f64) {
        let v = value.round().clamp(0.0, LAST) as u32;
        self.0.store(v, Ordering::Release);
        match v {
            ABORT_PROCESS => std::process::abort(),
            HANG_PROCESS => hang(),
            SLOW_ONCE => {
                self.0.store(0, Ordering::Release);
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            _ => {}
        }
    }

    /// At a site — `ABORT_SAVE`, `ABORT_LOAD`, `ABORT_EDITOR` or `ABORT_ACTIVATE` — fires
    /// what is armed for it: an abort, a hang, or (saving) a nested message loop.
    pub fn fire(&self, site: u32) {
        let armed = self.0.load(Ordering::Acquire);
        let hang_at = match site {
            ABORT_SAVE => HANG_SAVE,
            ABORT_LOAD => HANG_LOAD,
            ABORT_EDITOR => HANG_EDITOR,
            _ => u32::MAX,
        };
        if armed == site {
            std::process::abort();
        }
        if armed == hang_at {
            hang();
        }
        if site == ABORT_SAVE && armed == PUMP_SAVE {
            self.0.store(0, Ordering::Release);
            pump(std::time::Duration::from_secs(3));
        }
    }
}

fn hang() -> ! {
    loop {
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}

/// What a modal dialog does to its thread: a message loop of its own, for `long`.
#[cfg(windows)]
fn pump(long: std::time::Duration) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        DispatchMessageW, PeekMessageW, TranslateMessage, MSG, PM_REMOVE,
    };
    let start = std::time::Instant::now();
    while start.elapsed() < long {
        // SAFETY: a zeroed MSG filled by PeekMessageW, dispatched on this thread.
        unsafe {
            let mut msg: MSG = std::mem::zeroed();
            while PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

#[cfg(not(windows))]
fn pump(long: std::time::Duration) {
    std::thread::sleep(long);
}

/// The process this instance lives in: what the read-only *Process* parameter reports, so
/// a test sees an isolated instance really ran in another process.
pub fn process_id() -> f64 {
    f64::from(std::process::id())
}

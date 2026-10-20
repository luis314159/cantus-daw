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

use std::sync::atomic::{AtomicU32, Ordering};

pub const ABORT_PROCESS: u32 = 1;
pub const HANG_PROCESS: u32 = 2;
pub const ABORT_SAVE: u32 = 3;
pub const ABORT_LOAD: u32 = 4;
pub const ABORT_ACTIVATE: u32 = 5;
pub const ABORT_EDITOR: u32 = 6;
/// The parameter's range: 0 (none) … this.
pub const LAST: f64 = 6.0;

/// The fault armed in this instance, 0 for none.
#[derive(Debug, Default)]
pub struct Fault(AtomicU32);

impl Fault {
    /// A value set while processing: arm it, and fire the ones that belong to `process`.
    pub fn arm(&self, value: f64) {
        let v = value.round().clamp(0.0, LAST) as u32;
        self.0.store(v, Ordering::Release);
        self.fire(ABORT_PROCESS);
        self.fire(HANG_PROCESS);
    }

    /// Fires `kind` if it is the one armed: aborts the process, or hangs for ever.
    pub fn fire(&self, kind: u32) {
        if kind == 0 || self.0.load(Ordering::Acquire) != kind {
            return;
        }
        if kind == HANG_PROCESS {
            loop {
                std::thread::sleep(std::time::Duration::from_secs(1));
            }
        }
        std::process::abort();
    }
}

/// The process this instance lives in: what the read-only *Process* parameter reports, so
/// a test sees an isolated instance really ran in another process.
pub fn process_id() -> f64 {
    f64::from(std::process::id())
}

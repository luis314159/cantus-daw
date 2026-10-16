//! The hidden **Fault** parameter of both VST3 test plug-ins (round 29, ADR 0063): failures
//! on purpose, for the host's isolation tests — the twin of `clap-gain/src/fault.rs`. A
//! value that arrives **in `process`** (a parameter change of the block) arms a fault; the
//! same value arriving through the controller or a state arms nothing, so a reloaded
//! instance comes back healthy. The armed fault is the module's (one per process): the
//! controller that opens the editor is another object than the processor that armed it.
//!
//! | Value | What happens |
//! |---|---|
//! | 1 | aborts in this `process` |
//! | 2 | never returns from this `process` (a hang) |
//! | 3 | aborts in the next `getState` |
//! | 4 | aborts in the next `setState` |
//! | 5 | aborts in the next `setActive(true)` |
//! | 6 | aborts when an editor is next created (the gain; the pulse has none) |

use std::sync::atomic::{AtomicU32, Ordering};

pub const ABORT_PROCESS: u32 = 1;
pub const HANG_PROCESS: u32 = 2;
pub const ABORT_SAVE: u32 = 3;
pub const ABORT_LOAD: u32 = 4;
pub const ABORT_ACTIVATE: u32 = 5;
pub const ABORT_EDITOR: u32 = 6;
/// The steps of the parameter: 0 (none) … 6.
pub const STEPS: i32 = 6;
/// The Process parameter's plain range: 0 … this.
pub const PROCESS_RANGE: f64 = 4.0e9;

static ARMED: AtomicU32 = AtomicU32::new(0);

/// A normalised value that arrived in `process`: arm it, and fire what belongs there.
pub fn arm(norm: f64) {
    let v = (norm.clamp(0.0, 1.0) * f64::from(STEPS)).round() as u32;
    ARMED.store(v, Ordering::SeqCst);
    fire(ABORT_PROCESS);
    fire(HANG_PROCESS);
}

/// Fires `kind` if it is armed.
pub fn fire(kind: u32) {
    if ARMED.load(Ordering::SeqCst) != kind {
        return;
    }
    if kind == HANG_PROCESS {
        loop {
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
    }
    std::process::abort();
}

/// The Process parameter's normalised value: this process's id over its range.
pub fn process_norm() -> f64 {
    f64::from(std::process::id()) / PROCESS_RANGE
}

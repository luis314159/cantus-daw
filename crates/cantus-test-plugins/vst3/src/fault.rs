//! The hidden **Fault** parameter of both VST3 test plug-ins (round 29, ADR 0063): failures
//! on purpose, for the host's isolation tests — the twin of `clap-gain/src/fault.rs`, with
//! the same values. A value that arrives **in `process`** (a parameter change of the
//! block) arms a fault; the same value arriving through the controller or a state arms
//! nothing, so a reloaded instance comes back healthy. The armed fault is the module's (one
//! per process): the controller that opens the editor is another object than the processor
//! that armed it.
//!
//! | Value | What happens |
//! |---|---|
//! | 1 | aborts in this `process` |
//! | 2 | never returns from this `process` (a hang) |
//! | 3 | aborts in the next `getState` |
//! | 4 | aborts in the next `setState` |
//! | 5 | aborts in the next `setActive(true)` |
//! | 6 | aborts when an editor is next created (the gain; the pulse has none) |
//! | 7 | never returns from the next `getState` |
//! | 8 | never returns from the next `setState` |
//! | 9 | never returns when an editor is next created |
//! | 10 | the next `getState` blocks its thread for 3 s (no message loop here) |
//! | 11 | this `process` takes 50 ms, once |

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
/// The steps of the parameter: 0 (none) … 11.
pub const STEPS: i32 = 11;
/// The Process parameter's plain range: 0 … this.
pub const PROCESS_RANGE: f64 = 4.0e9;

static ARMED: AtomicU32 = AtomicU32::new(0);

/// A normalised value that arrived in `process`: arm it, and fire what belongs there.
pub fn arm(norm: f64) {
    let v = (norm.clamp(0.0, 1.0) * f64::from(STEPS)).round() as u32;
    ARMED.store(v, Ordering::SeqCst);
    match v {
        ABORT_PROCESS => std::process::abort(),
        HANG_PROCESS => hang(),
        SLOW_ONCE => {
            ARMED.store(0, Ordering::SeqCst);
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        _ => {}
    }
}

/// At a site (`ABORT_SAVE`, `ABORT_LOAD`, `ABORT_EDITOR`, `ABORT_ACTIVATE`): fires what is
/// armed for it.
pub fn fire(site: u32) {
    let armed = ARMED.load(Ordering::SeqCst);
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
        ARMED.store(0, Ordering::SeqCst);
        std::thread::sleep(std::time::Duration::from_secs(3));
    }
}

fn hang() -> ! {
    loop {
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}

/// The Process parameter's normalised value: this process's id over its range.
pub fn process_norm() -> f64 {
    f64::from(std::process::id()) / PROCESS_RANGE
}

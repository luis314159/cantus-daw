//! The floating-point environment the engine renders in: the same on every thread, and —
//! as far as the C runtime allows — on every machine.
//!
//! Two switches:
//!
//! - **Denormals** (per thread): [`DenormalGuard`] inside `Processor::process` and on every
//!   pool thread, [`reassert`] after every call into a hosted plug-in (ADR 0031 §5.2, R9).
//! - **The CRT's FMA3 code path** (per process, Windows x64 only): Microsoft's universal CRT
//!   picks FMA3 or non-FMA3 implementations of `exp`, `sin`, `pow`, `log`… by CPU at start-up
//!   and documents that results differ "between computers that do or don't support FMA3";
//!   Rust's `f32::sin` and friends call it. [`disable_crt_fma3`] makes every Windows machine
//!   take the non-FMA3 path. **Interim** (R9 (2)): it goes when the *math* builder's
//!   deterministic module replaces std's transcendentals in the DSP.
//!
//! The FMA3 switch is process-wide and changes results from the next call on, on every
//! thread: it belongs at start-up, before any audio — [`crate::init_process`] does it.

pub use crate::denormal::{flushing, reassert, DenormalGuard};

/// What [`disable_crt_fma3`] did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CrtFma3 {
    /// The CRT takes its non-FMA3 path from now on, whatever the CPU.
    Disabled,
    /// There is no such switch here (not Windows x64): nothing to do.
    NotApplicable,
    /// The CRT reported the FMA3 path still on after the call.
    StillEnabled,
}

impl std::fmt::Display for CrtFma3 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            CrtFma3::Disabled => "CRT FMA3 path disabled (one code path on every Windows machine)",
            CrtFma3::NotApplicable => "no CRT FMA3 switch on this platform",
            CrtFma3::StillEnabled => "the CRT refused to leave its FMA3 path",
        })
    }
}

/// Makes the C runtime's transcendental functions take their non-FMA3 implementation.
pub fn disable_crt_fma3() -> CrtFma3 {
    imp::disable()
}

/// Whether the CRT uses its FMA3 implementations now; `None` where there is no such switch.
pub fn crt_fma3_enabled() -> Option<bool> {
    imp::enabled()
}

/// Turns the FMA3 path back on (where the CPU has FMA3). For the tests that measure what the
/// switch changes; the application never calls it.
#[doc(hidden)]
pub fn enable_crt_fma3_for_tests() -> Option<bool> {
    imp::enable()
}

#[cfg(all(windows, target_arch = "x86_64", not(miri)))]
mod imp {
    use super::CrtFma3;

    extern "C" {
        // <math.h>, x64 only: "Returns a nonzero value if the FMA3 implementations … are
        // enabled after the call; otherwise zero." Exported by the universal CRT
        // (`api-ms-win-crt-math`), which every MSVC Rust binary links.
        fn _set_FMA3_enable(flag: i32) -> i32;
        fn _get_FMA3_enable() -> i32;
    }

    pub fn disable() -> CrtFma3 {
        // SAFETY: both functions only read or write the CRT's own dispatch flag; they take
        // and return plain integers.
        let now = unsafe { _set_FMA3_enable(0) };
        if now == 0 {
            CrtFma3::Disabled
        } else {
            CrtFma3::StillEnabled
        }
    }

    pub fn enabled() -> Option<bool> {
        // SAFETY: reads the CRT's dispatch flag.
        Some(unsafe { _get_FMA3_enable() } != 0)
    }

    pub fn enable() -> Option<bool> {
        // SAFETY: as in `disable`.
        Some(unsafe { _set_FMA3_enable(1) } != 0)
    }
}

#[cfg(not(all(windows, target_arch = "x86_64", not(miri))))]
mod imp {
    use super::CrtFma3;

    pub fn disable() -> CrtFma3 {
        CrtFma3::NotApplicable
    }

    pub fn enabled() -> Option<bool> {
        None
    }

    pub fn enable() -> Option<bool> {
        None
    }
}

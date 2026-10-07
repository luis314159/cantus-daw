//! The seam to `cantus-rt` (ADR 0031 §2.7, §5.2, amendment R1), built by the *rt* builder
//! in parallel with this one.
//!
//! **Until `cantus-rt` merges** this file holds a copy of the denormal guard of
//! `cantus-plugins/src/ftz.rs` (byte for byte the code that crate ships, which `cantus-rt`
//! moves into `denormal.rs`), and `promote_callback` does nothing. That is the one place
//! the engine holds `unsafe`, which is why `lib.rs` says `deny(unsafe_code)` instead of
//! `forbid` for now.
//!
//! **At the merge**, this file's body becomes
//!
//! ```ignore
//! pub use cantus_rt::DenormalGuard;
//! pub fn promote_callback() -> bool { cantus_rt::promote_callback() }
//! ```
//!
//! (adapted to the names `cantus-rt` exports), `cantus-rt` is added to the engine's
//! dependencies, and `lib.rs` goes back to `forbid(unsafe_code)`. Nothing else in the
//! engine changes.

/// The driver API a stream runs on — `cantus_rt::AudioHost`'s shape, so the merge only
/// swaps this file's body.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum AudioHost {
    Asio,
    Wasapi,
    CoreAudio,
    Jack,
    Alsa,
    Other,
}

impl AudioHost {
    /// From a driver API's name as cpal gives it (`HostId::name()`), case-insensitive.
    pub fn from_driver_name(name: &str) -> AudioHost {
        match name.to_ascii_lowercase().as_str() {
            "asio" => AudioHost::Asio,
            "wasapi" => AudioHost::Wasapi,
            "coreaudio" => AudioHost::CoreAudio,
            "jack" => AudioHost::Jack,
            "alsa" => AudioHost::Alsa,
            _ => AudioHost::Other,
        }
    }
}

/// Raise the audio callback's own thread — MMCSS "Pro Audio" at high priority on WASAPI,
/// nothing on ASIO (whose driver owns the thread) — once, on the first block after a
/// stream starts (`cantus_rt::promote_callback(host, period)`). Returns whether a system
/// call was made (`EngineStats::wakes`). Until `cantus-rt` lands: nothing, `false`.
#[inline]
pub(super) fn promote_callback(host: AudioHost, period: std::time::Duration) -> bool {
    let _ = (host, period);
    false
}

/// Sets FTZ and DAZ (x86-64: MXCSR bits 15 and 6; AArch64: FPCR's FZ, bit 24) on creation
/// and restores the caller's register on drop.
#[derive(Debug)]
pub(super) struct DenormalGuard {
    #[allow(dead_code)]
    saved: u64,
}

impl DenormalGuard {
    #[cfg(target_arch = "x86_64")]
    #[inline]
    pub(super) fn new() -> DenormalGuard {
        let mut csr: u32 = 0;
        // SAFETY: stmxcsr/ldmxcsr only read and write the SSE control register; the value
        // written is the current one with two mode bits set.
        unsafe {
            std::arch::asm!("stmxcsr [{}]", in(reg) &mut csr, options(nostack, preserves_flags));
            let flushed = csr | 0x8040;
            std::arch::asm!("ldmxcsr [{}]", in(reg) &flushed, options(nostack, preserves_flags));
        }
        DenormalGuard { saved: csr as u64 }
    }

    #[cfg(target_arch = "aarch64")]
    #[inline]
    pub(super) fn new() -> DenormalGuard {
        let fpcr: u64;
        // SAFETY: reads and writes the floating-point control register only.
        unsafe {
            std::arch::asm!("mrs {}, fpcr", out(reg) fpcr, options(nomem, nostack, preserves_flags));
            std::arch::asm!("msr fpcr, {}", in(reg) fpcr | (1 << 24), options(nomem, nostack, preserves_flags));
        }
        DenormalGuard { saved: fpcr }
    }

    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    #[inline]
    pub(super) fn new() -> DenormalGuard {
        DenormalGuard { saved: 0 }
    }
}

impl Drop for DenormalGuard {
    #[inline]
    fn drop(&mut self) {
        #[cfg(target_arch = "x86_64")]
        {
            let csr = self.saved as u32;
            // SAFETY: restores the register to the value read in `new`.
            unsafe {
                std::arch::asm!("ldmxcsr [{}]", in(reg) &csr, options(nostack, preserves_flags));
            }
        }
        #[cfg(target_arch = "aarch64")]
        {
            // SAFETY: restores the register to the value read in `new`.
            unsafe {
                std::arch::asm!("msr fpcr, {}", in(reg) self.saved, options(nomem, nostack, preserves_flags));
            }
        }
    }
}

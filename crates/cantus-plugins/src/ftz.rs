//! Denormals flushed to zero for the length of a block, the host's setting restored after.
//!
//! The DSPs already flush their own state in software so that silence ends in exact
//! zeros; the hardware flags are the belt to those braces, for the one filter state or
//! gain product that would otherwise crawl through subnormal arithmetic at a hundred
//! times the cost. On x86-64 that is MXCSR's FTZ (bit 15) and DAZ (bit 6); on AArch64,
//! FPCR's FZ (bit 24).

/// Sets the flags on creation and restores the previous ones on drop.
#[derive(Debug)]
pub struct DenormalGuard {
    #[allow(dead_code)]
    saved: u64,
}

impl DenormalGuard {
    #[cfg(target_arch = "x86_64")]
    #[inline]
    pub fn new() -> DenormalGuard {
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
    pub fn new() -> DenormalGuard {
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
    pub fn new() -> DenormalGuard {
        DenormalGuard { saved: 0 }
    }
}

impl Default for DenormalGuard {
    fn default() -> Self {
        Self::new()
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

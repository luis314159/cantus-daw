//! Denormals flushed to zero for the length of a block, the caller's setting restored after.
//!
//! The DSPs already flush their own state in software so that silence ends in exact zeros;
//! the hardware flags are the belt to those braces, for the one filter state or gain product
//! that would otherwise crawl through subnormal arithmetic at a hundred times the cost. On
//! x86-64 that is MXCSR's FTZ (bit 15) and DAZ (bit 6); on AArch64, FPCR's FZ (bit 24) —
//! which flushes inputs and outputs alike: there is no separate DAZ bit.
//!
//! Moved here from `cantus-plugins/src/ftz.rs` (round 21, ADR 0031 §2.7/§5.2), which now
//! re-exports it. **What it rests on** (ADR 0031 R9 (5)): changing the floating-point
//! control bits outside an `asm!` block is outside Rust's model (RFC 3514), so the guarantee
//! that the flags change no sample is empirical — Cantus's DSP never needs a subnormal, and
//! the null corpus is rendered with the guard on and off to prove it. JUCE's
//! `ScopedNoDenormals` and every DAW do the same; the guard saves and restores so a host's
//! or a plug-in's own setting survives it.

// Each architecture uses one of these; the tests check both.
#[allow(dead_code)]
const X86_FTZ_DAZ: u64 = 0x8040;
#[allow(dead_code)]
const ARM_FZ: u64 = 1 << 24;

/// Sets the flags on creation and restores the previous ones on drop.
///
/// Not `Send`: the control register is per thread, so the guard restores the thread that
/// made it.
#[derive(Debug)]
pub struct DenormalGuard {
    #[allow(dead_code)]
    saved: u64,
    _not_send: std::marker::PhantomData<*const ()>,
}

impl DenormalGuard {
    /// Flushes denormals on this thread until the guard drops.
    #[inline]
    pub fn new() -> DenormalGuard {
        let saved = read();
        write(saved | FLUSH);
        DEPTH.with(|d| d.set(d.get() + 1));
        DenormalGuard {
            saved,
            _not_send: std::marker::PhantomData,
        }
    }

    /// Sets the flags again — after a hosted plug-in's call, which may have changed the
    /// register and not put it back (ADR 0031 R9). What is restored on drop is still the
    /// value saved by [`DenormalGuard::new`].
    #[inline]
    pub fn reassert(&self) {
        reassert_raw();
    }
}

thread_local! {
    // How many guards are alive on this thread: `reassert` acts only inside one, so a call
    // from code that may also run unguarded (a hosted plug-in's wrapper) never leaves the
    // flags set behind a caller's back. A const-initialised `Cell` needs no destructor and
    // no allocation: a plain thread-local slot.
    static DEPTH: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

/// Re-asserts the flags if a [`DenormalGuard`] is alive on this thread, and does nothing
/// otherwise. For call sites that do not hold the guard — the hosted plug-in wrappers in
/// `cantus-host`, after every audio-thread call into the plug-in. Costs a thread-local read
/// and a register read; the register is written only when a plug-in changed it.
#[inline]
pub fn reassert() {
    if DEPTH.with(|d| d.get()) > 0 {
        reassert_raw();
    }
}

#[inline]
fn reassert_raw() {
    let csr = read();
    if csr & FLUSH != FLUSH {
        write(csr | FLUSH);
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
        DEPTH.with(|d| d.set(d.get().saturating_sub(1)));
        write(self.saved);
    }
}

/// True when this thread flushes denormals now (x86-64: FTZ and DAZ both set; AArch64: FZ).
/// Always false where the crate cannot set them.
#[inline]
pub fn flushing() -> bool {
    FLUSH != 0 && read() & FLUSH == FLUSH
}

/// The raw control register (MXCSR or FPCR), for tests and the probe.
#[inline]
pub fn control_register() -> u64 {
    read()
}

#[cfg(all(target_arch = "x86_64", not(miri)))]
const FLUSH: u64 = X86_FTZ_DAZ;
#[cfg(all(target_arch = "aarch64", not(miri)))]
const FLUSH: u64 = ARM_FZ;
#[cfg(any(miri, not(any(target_arch = "x86_64", target_arch = "aarch64"))))]
const FLUSH: u64 = 0;

#[cfg(all(target_arch = "x86_64", not(miri)))]
#[inline]
fn read() -> u64 {
    let mut csr: u32 = 0;
    // SAFETY: `stmxcsr` stores the SSE control register into the live local `csr`; it changes
    // no register and no other memory.
    unsafe {
        std::arch::asm!("stmxcsr [{}]", in(reg) &mut csr, options(nostack, preserves_flags));
    }
    csr as u64
}

#[cfg(all(target_arch = "x86_64", not(miri)))]
#[inline]
fn write(value: u64) {
    let csr = value as u32;
    // SAFETY: `ldmxcsr` loads the SSE control register from the live local `csr`. Every value
    // written here is one read from the register, with at most the FTZ and DAZ mode bits
    // added, so no reserved bit is ever set (which would fault).
    unsafe {
        std::arch::asm!("ldmxcsr [{}]", in(reg) &csr, options(nostack, preserves_flags));
    }
}

#[cfg(all(target_arch = "aarch64", not(miri)))]
#[inline]
fn read() -> u64 {
    let fpcr: u64;
    // SAFETY: reads the floating-point control register; no memory is touched.
    unsafe {
        std::arch::asm!("mrs {}, fpcr", out(reg) fpcr, options(nomem, nostack, preserves_flags));
    }
    fpcr
}

#[cfg(all(target_arch = "aarch64", not(miri)))]
#[inline]
fn write(value: u64) {
    // SAFETY: writes the floating-point control register with a value read from it, with at
    // most the FZ mode bit added; no memory is touched.
    unsafe {
        std::arch::asm!("msr fpcr, {}", in(reg) value, options(nomem, nostack, preserves_flags));
    }
}

// Miri cannot run inline assembly, and other architectures have no flag we set: the guard is
// then a value with no effect, which keeps every caller's code the same.
#[cfg(any(miri, not(any(target_arch = "x86_64", target_arch = "aarch64"))))]
#[inline]
fn read() -> u64 {
    0
}

#[cfg(any(miri, not(any(target_arch = "x86_64", target_arch = "aarch64"))))]
#[inline]
fn write(_value: u64) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reassert_acts_only_inside_a_guard() {
        let before = control_register();
        // Outside a guard: nothing changes.
        reassert();
        assert_eq!(control_register(), before);
        let g = DenormalGuard::new();
        // A "plug-in" that clears the flags and does not put them back.
        write(read() & !FLUSH);
        reassert();
        assert_eq!(flushing(), FLUSH != 0);
        drop(g);
        assert_eq!(control_register(), before);
    }

    // The constants are the architectures' documented bits; this keeps them honest on every
    // target (and under Miri, where FLUSH is zero).
    #[test]
    fn the_bits_are_the_documented_ones() {
        assert_eq!(X86_FTZ_DAZ, (1 << 15) | (1 << 6));
        assert_eq!(ARM_FZ, 0x0100_0000);
    }
}

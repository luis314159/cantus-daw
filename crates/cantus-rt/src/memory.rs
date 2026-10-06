//! Memory the audio thread never faults on (ADR 0031 R11).
//!
//! The OS maps a fresh allocation's pages on **first touch**: the first block that writes a
//! page of a slab built on the UI thread takes a soft page fault, and the kernel zeroes the
//! page, *on the audio thread*. So every slab, ring and pool is [`prefault`]ed on the thread
//! that allocates it, and [`lock`]ed where the OS allows (`VirtualLock` after raising the
//! working-set minimum; `mlock`), best-effort and reported; each pool thread calls
//! [`prefault_stack`] when it starts. Large pages are not used: they need a privilege and are
//! hard to get once the system has run a while.

use crate::thread::Refusal;
use std::fmt;

/// The OS page size, bytes (4096 where the OS is not asked).
pub fn page_size() -> usize {
    static SIZE: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *SIZE.get_or_init(|| imp::page_size().filter(|&s| s.is_power_of_two()).unwrap_or(4096))
}

/// Writes one element per page of `buf` with the value already there, so every page is
/// mapped now, by the calling thread. Returns how many writes it made. Contents unchanged.
pub fn prefault<T: Copy>(buf: &mut [T]) -> usize {
    let size = std::mem::size_of::<T>();
    if size == 0 || buf.is_empty() {
        return 0;
    }
    // At most a page apart, so no page is skipped; the last element covers the tail.
    let stride = (page_size() / size).max(1);
    let mut n = 0;
    let last = buf.len() - 1;
    for i in (0..buf.len()).step_by(stride).chain(std::iter::once(last)) {
        let p: *mut T = &mut buf[i];
        // SAFETY: `p` comes from a live `&mut` into `buf`; a volatile read and write of the
        // same value cannot be elided by the compiler, so the page is really touched.
        unsafe { std::ptr::write_volatile(p, std::ptr::read_volatile(p)) };
        n += 1;
    }
    n
}

/// Touches 64 KiB of the calling thread's stack, top-down, so a deep call chain on a pool
/// thread finds its stack pages mapped. Call once when the thread starts.
#[inline(never)]
pub fn prefault_stack() {
    let mut probe = [0u8; 64 * 1024];
    let page = page_size().min(probe.len());
    for i in (0..probe.len()).rev().step_by(page) {
        // SAFETY: an in-bounds element of a live local array.
        unsafe { std::ptr::write_volatile(&mut probe[i], 1) };
    }
    std::hint::black_box(&probe);
}

/// The result of [`lock`].
#[derive(Debug)]
pub enum Lock {
    /// The pages stay in RAM until the region drops.
    Locked(LockedRegion),
    /// The OS said no (a small `RLIMIT_MEMLOCK`, a working-set quota): the memory still
    /// works, it may just be paged out under pressure.
    Refused(Refusal),
    NotApplicable,
}

impl Lock {
    pub fn is_locked(&self) -> bool {
        matches!(self, Lock::Locked(_))
    }
}

impl fmt::Display for Lock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Lock::Locked(r) => write!(f, "locked {} KiB", r.len / 1024),
            Lock::Refused(r) => write!(f, "not locked: {r}"),
            Lock::NotApplicable => f.write_str("no page locking on this target"),
        }
    }
}

/// Locked pages, unlocked on drop. Drop it before the memory is freed (unlocking freed
/// memory is a failed, harmless call — but it could unlock a later allocation's pages).
#[derive(Debug)]
pub struct LockedRegion {
    addr: usize,
    len: usize,
}

impl Drop for LockedRegion {
    fn drop(&mut self) {
        imp::unlock(self.addr, self.len);
    }
}

/// Locks `buf`'s pages in physical memory, best-effort (see [`Lock`]). Prefault first: a
/// locked page is resident, and locking is what keeps it so.
pub fn lock<T>(buf: &[T]) -> Lock {
    let len = std::mem::size_of_val(buf);
    if len == 0 {
        return Lock::NotApplicable;
    }
    imp::lock(buf.as_ptr() as usize, len)
}

#[cfg(all(windows, not(miri)))]
mod imp {
    use super::Lock;
    use crate::thread::Refusal;
    use windows_sys::Win32::Foundation::GetLastError;
    use windows_sys::Win32::System::Memory::{
        SetProcessWorkingSetSizeEx, VirtualLock, VirtualUnlock, QUOTA_LIMITS_HARDWS_MAX_DISABLE,
        QUOTA_LIMITS_HARDWS_MIN_DISABLE,
    };
    use windows_sys::Win32::System::SystemInformation::{GetSystemInfo, SYSTEM_INFO};
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, GetProcessWorkingSetSize};

    pub fn page_size() -> Option<usize> {
        // SAFETY: an all-zero SYSTEM_INFO is valid to overwrite; the call fills it.
        let mut info: SYSTEM_INFO = unsafe { std::mem::zeroed() };
        // SAFETY: `info` is a live out-pointer.
        unsafe { GetSystemInfo(&mut info) };
        Some(info.dwPageSize as usize)
    }

    fn refused(step: &'static str) -> Lock {
        // SAFETY: reads this thread's last-error value.
        Lock::Refused(Refusal { step, code: unsafe { GetLastError() } as i32 })
    }

    pub fn lock(addr: usize, len: usize) -> Lock {
        // VirtualLock can lock only what fits the working-set minimum (minus a few pages the
        // process needs): raise it by the region first, as soft limits.
        let (mut min, mut max) = (0usize, 0usize);
        // SAFETY: a pseudo-handle for this process; live out-pointers.
        if unsafe { GetProcessWorkingSetSize(GetCurrentProcess(), &mut min, &mut max) } == 0 {
            return refused("GetProcessWorkingSetSize");
        }
        let slack = 64 * super::page_size();
        let (new_min, new_max) = (min + len + slack, max.max(min + len + slack));
        let soft = QUOTA_LIMITS_HARDWS_MIN_DISABLE | QUOTA_LIMITS_HARDWS_MAX_DISABLE;
        // SAFETY: a pseudo-handle for this process; plain sizes.
        if unsafe { SetProcessWorkingSetSizeEx(GetCurrentProcess(), new_min, new_max, soft) } == 0 {
            return refused("SetProcessWorkingSetSizeEx");
        }
        // SAFETY: `addr..addr+len` is the live slice the caller passed to `super::lock`; the
        // call only changes the pages' residency, never their contents.
        if unsafe { VirtualLock(addr as *const _, len) } == 0 {
            return refused("VirtualLock");
        }
        Lock::Locked(super::LockedRegion { addr, len })
    }

    pub fn unlock(addr: usize, len: usize) {
        // SAFETY: undoes the lock made on this range; a range no longer locked or mapped
        // makes the call fail without effect.
        unsafe { VirtualUnlock(addr as *const _, len) };
    }
}

#[cfg(all(unix, not(miri)))]
mod imp {
    use super::Lock;
    use crate::thread::Refusal;

    pub fn page_size() -> Option<usize> {
        // SAFETY: sysconf has no preconditions.
        let s = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
        (s > 0).then_some(s as usize)
    }

    pub fn lock(addr: usize, len: usize) -> Lock {
        // SAFETY: the range is the live slice the caller passed; mlock changes residency only.
        if unsafe { libc::mlock(addr as *const libc::c_void, len) } == 0 {
            Lock::Locked(super::LockedRegion { addr, len })
        } else {
            let code = std::io::Error::last_os_error().raw_os_error().unwrap_or(0);
            Lock::Refused(Refusal { step: "mlock (RLIMIT_MEMLOCK)", code })
        }
    }

    pub fn unlock(addr: usize, len: usize) {
        // SAFETY: undoes the lock on this range; an unmapped range fails without effect.
        unsafe { libc::munlock(addr as *const libc::c_void, len) };
    }
}

#[cfg(any(miri, not(any(windows, unix))))]
mod imp {
    use super::Lock;

    pub fn page_size() -> Option<usize> {
        None
    }

    pub fn lock(_addr: usize, _len: usize) -> Lock {
        Lock::NotApplicable
    }

    pub fn unlock(_addr: usize, _len: usize) {}
}

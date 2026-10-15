//! The process's memory in use — the top bar's RAM readout (ADR 0052). Read off the audio
//! thread, at a low rate (the interface reads it about once a second).
//!
//! - **Windows**: the working set, `K32GetProcessMemoryInfo` on the current process.
//! - **Linux**: the resident set, `VmRSS` in `/proc/self/status` (plain file reading, no
//!   `unsafe`).
//! - **Elsewhere** (macOS among them): `None` — the readout shows a dash rather than a
//!   number it cannot stand behind.

/// The bytes of physical memory the process holds now, or `None` when the system cannot
/// say.
pub fn working_set_bytes() -> Option<u64> {
    os_working_set()
}

#[cfg(windows)]
fn os_working_set() -> Option<u64> {
    use windows_sys::Win32::System::ProcessStatus::{
        K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS,
    };
    use windows_sys::Win32::System::Threading::GetCurrentProcess;
    let mut c: PROCESS_MEMORY_COUNTERS = PROCESS_MEMORY_COUNTERS {
        cb: 0,
        PageFaultCount: 0,
        PeakWorkingSetSize: 0,
        WorkingSetSize: 0,
        QuotaPeakPagedPoolUsage: 0,
        QuotaPagedPoolUsage: 0,
        QuotaPeakNonPagedPoolUsage: 0,
        QuotaNonPagedPoolUsage: 0,
        PagefileUsage: 0,
        PeakPagefileUsage: 0,
    };
    let size = std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
    c.cb = size;
    // SAFETY: `GetCurrentProcess` returns a pseudo-handle that needs no closing; `c` is a
    // live, writable struct of the size passed, read only after the call reported success.
    let ok = unsafe { K32GetProcessMemoryInfo(GetCurrentProcess(), &mut c, size) };
    (ok != 0).then_some(c.WorkingSetSize as u64)
}

#[cfg(target_os = "linux")]
fn os_working_set() -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    parse_vm_rss(&status)
}

/// `VmRSS:   123456 kB` → bytes.
#[cfg(any(target_os = "linux", test))]
fn parse_vm_rss(status: &str) -> Option<u64> {
    let line = status.lines().find(|l| l.starts_with("VmRSS:"))?;
    let mut words = line["VmRSS:".len()..].split_whitespace();
    let n: u64 = words.next()?.parse().ok()?;
    let unit = match words.next() {
        Some("kB") | Some("KB") | Some("kb") => 1024,
        Some("mB") | Some("MB") => 1024 * 1024,
        Some("B") | None => 1,
        Some(_) => return None,
    };
    Some(n.saturating_mul(unit))
}

#[cfg(not(any(windows, target_os = "linux")))]
fn os_working_set() -> Option<u64> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_status_files_resident_set_is_read_in_bytes() {
        let s = "Name:\tcantus\nVmPeak:\t  900 kB\nVmRSS:\t   123456 kB\nThreads:\t9\n";
        assert_eq!(parse_vm_rss(s), Some(123_456 * 1024));
        assert_eq!(parse_vm_rss("Name:\tx\n"), None);
        assert_eq!(parse_vm_rss("VmRSS:\tlots kB\n"), None);
    }

    /// On the systems that can say, the reading is the process's: more than a megabyte,
    /// and it grows by about what is allocated and touched.
    #[test]
    fn the_working_set_follows_what_the_process_touches() {
        if cfg!(not(any(windows, target_os = "linux"))) {
            assert_eq!(working_set_bytes(), None);
            return;
        }
        let before = working_set_bytes().expect("a reading");
        assert!(before > 1 << 20, "{before}");
        let mut big = vec![0u8; 64 << 20];
        for i in (0..big.len()).step_by(4096) {
            big[i] = 1;
        }
        let after = working_set_bytes().expect("a reading");
        std::hint::black_box(&big);
        assert!(
            after >= before + (48 << 20),
            "64 MB touched: {before} → {after}"
        );
    }
}

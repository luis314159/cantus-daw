//! How many files the process may hold open (third review #1): on Unix the soft
//! `RLIMIT_NOFILE` is raised to the hard limit at start-up (macOS: at most `OPEN_MAX`, 10 240,
//! as `setrlimit` there accepts) — a desktop session often starts at 256 (macOS) or 1 024
//! (Linux) — and its value sizes the stream cache's handle budget. Windows has no such limit
//! for file handles. Off the audio thread.

/// The macOS ceiling `setrlimit` accepts for `RLIMIT_NOFILE` (`OPEN_MAX`).
#[cfg(target_os = "macos")]
const MAC_OPEN_MAX: u64 = 10_240;

/// What Windows is taken to allow (its handles have no per-process limit worth naming).
#[cfg(not(unix))]
const WINDOWS_FILES: u64 = 16_384;

/// Raise the soft limit on open files to the hard one (once is enough; idempotent). Returns
/// the soft limit in force afterwards.
pub fn raise_limit() -> u64 {
    #[cfg(unix)]
    {
        let Some((soft, hard)) = get() else {
            return 1_024;
        };
        #[cfg(target_os = "macos")]
        let hard = hard.min(MAC_OPEN_MAX);
        if hard > soft && set(hard, None) {
            return hard;
        }
        soft
    }
    #[cfg(not(unix))]
    {
        WINDOWS_FILES
    }
}

/// The soft limit on open files now.
pub fn limit() -> u64 {
    #[cfg(unix)]
    {
        get().map_or(1_024, |(soft, _)| soft)
    }
    #[cfg(not(unix))]
    {
        WINDOWS_FILES
    }
}

/// Lower (or raise, within the hard limit) the soft limit — tests of what a low limit does.
/// Returns whether it took.
pub fn set_limit(soft: u64) -> bool {
    #[cfg(unix)]
    {
        set(soft, get().map(|(_, h)| h))
    }
    #[cfg(not(unix))]
    {
        let _ = soft;
        false
    }
}

#[cfg(unix)]
fn get() -> Option<(u64, u64)> {
    let mut r = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    // SAFETY: `r` is a live, writable rlimit; the call only fills it.
    let ok = unsafe { libc::getrlimit(libc::RLIMIT_NOFILE, &mut r) } == 0;
    #[allow(clippy::unnecessary_cast)]
    ok.then_some((r.rlim_cur as u64, r.rlim_max as u64))
}

#[cfg(unix)]
fn set(soft: u64, hard: Option<u64>) -> bool {
    let Some(hard) = hard.or_else(|| get().map(|(_, h)| h)) else {
        return false;
    };
    let r = libc::rlimit {
        rlim_cur: soft.min(hard) as libc::rlim_t,
        rlim_max: hard as libc::rlim_t,
    };
    // SAFETY: `r` is a live rlimit read by the call; nothing else is touched.
    unsafe { libc::setrlimit(libc::RLIMIT_NOFILE, &r) == 0 }
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_limit_is_raised_to_what_the_system_allows() {
        let raised = super::raise_limit();
        assert!(raised >= 256, "{raised}");
        assert_eq!(super::limit(), raised);
        #[cfg(unix)]
        {
            assert!(super::set_limit(200));
            assert_eq!(super::limit(), 200);
            assert_eq!(super::raise_limit(), raised, "raised again");
        }
    }
}

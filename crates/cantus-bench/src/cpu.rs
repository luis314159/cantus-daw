//! The calling thread's CPU time, to tell *slow* from *not scheduled*.
//!
//! On a machine shared with other work (other builds, the CI runner's neighbours) a long
//! run's wall time includes the time the thread waited for a core. The thread's own CPU
//! time does not: over a run of seconds, `cpu / wall` is the share of a core the run
//! actually got, and `cpu` per block is a figure the neighbours move far less. The OS
//! counts it in scheduler ticks (15.6 ms on Windows), so it is only meaningful over runs
//! of a second or more — never for one block.

#[cfg(windows)]
mod imp {
    #[repr(C)]
    #[derive(Default)]
    struct FileTime {
        low: u32,
        high: u32,
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn GetCurrentThread() -> isize;
        fn GetThreadTimes(
            thread: isize,
            creation: *mut FileTime,
            exit: *mut FileTime,
            kernel: *mut FileTime,
            user: *mut FileTime,
        ) -> i32;
    }

    pub fn thread_seconds() -> Option<f64> {
        let (mut c, mut e, mut k, mut u) = Default::default();
        // SAFETY: GetCurrentThread returns a pseudo-handle valid for this call; the four
        // out-pointers are to live, properly laid out FILETIMEs on this stack frame.
        let ok = unsafe { GetThreadTimes(GetCurrentThread(), &mut c, &mut e, &mut k, &mut u) };
        let t = |f: &FileTime| ((f.high as u64) << 32 | f.low as u64) as f64 * 100e-9;
        (ok != 0).then(|| t(&k) + t(&u))
    }
}

#[cfg(unix)]
mod imp {
    #[repr(C)]
    struct TimeSpec {
        sec: i64,
        nsec: i64,
    }

    #[cfg(target_os = "macos")]
    const CLOCK_THREAD_CPUTIME_ID: i32 = 16;
    #[cfg(not(target_os = "macos"))]
    const CLOCK_THREAD_CPUTIME_ID: i32 = 3;

    extern "C" {
        fn clock_gettime(clock: i32, ts: *mut TimeSpec) -> i32;
    }

    pub fn thread_seconds() -> Option<f64> {
        let mut ts = TimeSpec { sec: 0, nsec: 0 };
        // SAFETY: `ts` is a live timespec (64-bit fields on every 64-bit target we build).
        let ok = unsafe { clock_gettime(CLOCK_THREAD_CPUTIME_ID, &mut ts) };
        (ok == 0).then(|| ts.sec as f64 + ts.nsec as f64 * 1e-9)
    }
}

#[cfg(not(any(windows, unix)))]
mod imp {
    pub fn thread_seconds() -> Option<f64> {
        None
    }
}

/// CPU seconds this thread has run (user + kernel), when the OS says.
pub fn thread_seconds() -> Option<f64> {
    imp::thread_seconds()
}

/// Run `f`, returning its result, its wall seconds and the thread's CPU seconds in it.
pub fn timed<R>(f: impl FnOnce() -> R) -> (R, f64, Option<f64>) {
    let c0 = thread_seconds();
    let t = std::time::Instant::now();
    let r = f();
    let wall = t.elapsed().as_secs_f64();
    let cpu = match (c0, thread_seconds()) {
        (Some(a), Some(b)) => Some(b - a),
        _ => None,
    };
    (r, wall, cpu)
}

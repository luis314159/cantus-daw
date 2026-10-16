//! Lock-step exchanges between two processes over shared memory (ADR 0063).
//!
//! A **caller** hands a job to a **worker** in another process and waits for it to finish;
//! the job's data lives beside the [`Exchange`] in a [`crate::shm::SharedMemory`] region,
//! and this module only moves the two sequence numbers and the wake-ups:
//!
//! 1. The caller writes the job's data, then [`Caller::submit`]: `job += 1` (sequentially
//!    consistent, so the data is visible before the number), and — only when the worker
//!    has said it is parked — rings the worker's [`Bell`].
//! 2. The worker, spinning or woken, sees the new number ([`Worker::next`]), reads the
//!    data, does the work, writes the results, and [`Worker::complete`]s: `done = job`,
//!    and — only when the caller has said it is waiting in the kernel — rings the caller.
//! 3. The caller spins on `done` for a bounded time, then sleeps on its own bell until a
//!    **deadline** ([`Caller::wait`]): done, late, or closed (the worker's process is gone,
//!    which whoever watches it marks with [`Exchange::close`] and a ring).
//!
//! Each side announces that it is about to sleep (`caller_waiting`, `worker_parked`)
//! *before* it checks the other's number for the last time, and each side publishes its
//! number *before* it reads the other's announcement — Dekker's pattern, so under the
//! sequentially consistent order of these four operations a wake-up is never lost. A bell
//! is an eventcount: the sleeper reads its word, announces, re-checks, and sleeps only if
//! the word has not moved, so a ring between the re-check and the sleep is never missed
//! either. `tests/ipc_model.rs` explores every interleaving of both sides, step by step,
//! and finds no state where one sleeps while the other has already done what it waits for.
//!
//! A job not finished by the deadline is **late**: the caller goes on without it and does
//! not submit another until [`Caller::finished`] says the late one is done. Nothing here
//! allocates; the only system calls are a ring (`SetEvent`, a futex wake) when the other
//! side sleeps, and a sleep on the caller's own bell once its spin budget is spent.
//!
//! Nothing here knows about audio (ADR 0031 §2.7).

use std::io;
use std::sync::atomic::{AtomicU32, Ordering::*};
use std::time::{Duration, Instant};

/// The words of one exchange, in shared memory: `#[repr(C)]`, all atomics, so both
/// processes agree on the layout. What the caller writes and the worker reads sits on one
/// cache line, the other way on another.
#[repr(C, align(128))]
#[derive(Debug, Default)]
pub struct Exchange {
    /// The last job the caller published.
    job: AtomicU32,
    /// 1 while the worker is about to sleep, or sleeps, on its bell.
    worker_parked: AtomicU32,
    /// The worker's bell (an eventcount word).
    worker_bell: AtomicU32,
    _pad0: [u32; 29],
    /// The last job the worker finished.
    done: AtomicU32,
    /// 1 while the caller is about to sleep, or sleeps, on its bell.
    caller_waiting: AtomicU32,
    /// The caller's bell.
    caller_bell: AtomicU32,
    /// Non-zero once the worker side is gone: nothing will ever complete.
    closed: AtomicU32,
    _pad1: [u32; 28],
}

impl Exchange {
    /// Marks the worker side gone and wakes a caller that waits. Called by whoever watches
    /// the worker's process; then ring the caller's bell.
    pub fn close(&self) {
        self.closed.store(1, SeqCst);
    }

    pub fn is_closed(&self) -> bool {
        self.closed.load(Acquire) != 0
    }

    /// The last job published, and the last finished.
    pub fn numbers(&self) -> (u32, u32) {
        (self.job.load(Acquire), self.done.load(Acquire))
    }
}

/// How a wait ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// The job finished; its results may be read.
    Done,
    /// The deadline passed first: the job is still running, and its buffers belong to the
    /// worker until [`Caller::finished`] says otherwise.
    Late,
    /// The worker side is gone.
    Closed,
}

/// The caller's half: its own bell to sleep on, the worker's to ring.
#[derive(Debug)]
pub struct Caller {
    x: *const Exchange,
    mine: Bell,
    theirs: Bell,
}

// SAFETY: the exchange is atomics in a mapping its owner keeps alive for this value's
// life (the `new` contract); bells are OS handles usable from any thread.
unsafe impl Send for Caller {}

/// The worker's half.
#[derive(Debug)]
pub struct Worker {
    x: *const Exchange,
    mine: Bell,
    theirs: Bell,
}

// SAFETY: as for `Caller`.
unsafe impl Send for Worker {}

impl Caller {
    /// Makes the exchange's bells under `name` (the region's own name serves) for the
    /// exchange at `x`.
    ///
    /// # Safety
    /// `x` points to an `Exchange` in shared memory that stays mapped for this value's life.
    pub unsafe fn create(name: &str, x: *const Exchange) -> io::Result<Caller> {
        // SAFETY: the words are inside `*x`, alive as long as the caller promises.
        unsafe {
            Ok(Caller {
                mine: Bell::create(&format!("{name}-c"), &(*x).caller_bell)?,
                theirs: Bell::create(&format!("{name}-w"), &(*x).worker_bell)?,
                x,
            })
        }
    }

    fn x(&self) -> &Exchange {
        // SAFETY: the `create` contract keeps the exchange mapped while `self` lives.
        unsafe { &*self.x }
    }

    /// Publishes the next job (its data already written) and wakes the worker if it
    /// sleeps. Returns the job's number.
    pub fn submit(&self) -> u32 {
        let x = self.x();
        let n = x.job.load(Relaxed).wrapping_add(1);
        x.job.store(n, SeqCst);
        if x.worker_parked.load(SeqCst) != 0 {
            self.theirs.ring();
        }
        n
    }

    /// Whether job `n` has finished (a late job, polled before the next one).
    pub fn finished(&self, n: u32) -> bool {
        self.x().done.load(Acquire) == n
    }

    /// Waits for job `n`: spins for `spin`, then sleeps on the caller's bell until
    /// `deadline` (`None`: for as long as it takes — offline work — while the worker
    /// lives).
    pub fn wait(&self, n: u32, spin: Duration, deadline: Option<Instant>) -> Outcome {
        let x = self.x();
        let start = Instant::now();
        let mut k = 0u32;
        loop {
            if x.done.load(Acquire) == n {
                return Outcome::Done;
            }
            if x.closed.load(Acquire) != 0 {
                return Outcome::Closed;
            }
            k = k.wrapping_add(1);
            if k % 32 == 0 && start.elapsed() >= spin {
                break;
            }
            std::hint::spin_loop();
        }
        loop {
            let epoch = self.mine.epoch();
            x.caller_waiting.store(1, SeqCst);
            if x.done.load(SeqCst) == n {
                x.caller_waiting.store(0, SeqCst);
                return Outcome::Done;
            }
            if x.closed.load(SeqCst) != 0 {
                x.caller_waiting.store(0, SeqCst);
                return Outcome::Closed;
            }
            let left = match deadline {
                Some(d) => {
                    let left = d.saturating_duration_since(Instant::now());
                    if left.is_zero() {
                        x.caller_waiting.store(0, SeqCst);
                        return Outcome::Late;
                    }
                    Some(left)
                }
                None => None,
            };
            self.mine.wait(epoch, left);
            x.caller_waiting.store(0, SeqCst);
        }
    }

    /// A handle another thread keeps to mark the worker side gone and wake this caller
    /// (whoever watches the worker's process). Opened by `name`, as made here.
    pub fn closer(&self, name: &str) -> io::Result<Closer> {
        // SAFETY: the same word as `self.mine`, in the same mapping; the closer's owner
        // keeps the mapping alive as long as the closer (the `Closer` contract).
        let bell = unsafe { Bell::open(&format!("{name}-c"), &self.x().caller_bell)? };
        Ok(Closer { x: self.x, bell })
    }
}

/// Marks an exchange's worker side gone and wakes its caller ([`Caller::closer`]). Its
/// owner keeps the exchange's mapping alive while it holds one.
#[derive(Debug)]
pub struct Closer {
    x: *const Exchange,
    bell: Bell,
}

// SAFETY: atomics in a mapping kept alive by the owner, and an OS handle.
unsafe impl Send for Closer {}
// SAFETY: `close` touches only atomics and the bell.
unsafe impl Sync for Closer {}

impl Closer {
    pub fn close(&self) {
        // SAFETY: the `Closer` contract keeps the exchange mapped.
        unsafe { &*self.x }.close();
        self.bell.ring();
    }
}

/// What a worker found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Next {
    /// A job with this number: its data may be read.
    Job(u32),
    /// Nothing came before the park timeout.
    Idle,
}

impl Worker {
    /// Opens the bells the caller made under `name`, for the exchange at `x`.
    ///
    /// # Safety
    /// As [`Caller::create`].
    pub unsafe fn open(name: &str, x: *const Exchange) -> io::Result<Worker> {
        // SAFETY: as in `Caller::create`.
        unsafe {
            Ok(Worker {
                mine: Bell::open(&format!("{name}-w"), &(*x).worker_bell)?,
                theirs: Bell::open(&format!("{name}-c"), &(*x).caller_bell)?,
                x,
            })
        }
    }

    fn x(&self) -> &Exchange {
        // SAFETY: the `open` contract keeps the exchange mapped while `self` lives.
        unsafe { &*self.x }
    }

    /// Waits for a job after `last`: spins for `spin` (the next one usually comes soon —
    /// the rest of a block's segments), then parks on the worker's bell for at most
    /// `park` (`None`: until rung).
    pub fn next(&self, last: u32, spin: Duration, park: Option<Duration>) -> Next {
        let x = self.x();
        let start = Instant::now();
        let mut k = 0u32;
        loop {
            let n = x.job.load(Acquire);
            if n != last {
                return Next::Job(n);
            }
            k = k.wrapping_add(1);
            if k % 32 == 0 && start.elapsed() >= spin {
                break;
            }
            std::hint::spin_loop();
        }
        let epoch = self.mine.epoch();
        x.worker_parked.store(1, SeqCst);
        let n = x.job.load(SeqCst);
        if n != last {
            x.worker_parked.store(0, SeqCst);
            return Next::Job(n);
        }
        self.mine.wait(epoch, park);
        x.worker_parked.store(0, SeqCst);
        let n = x.job.load(SeqCst);
        if n != last {
            Next::Job(n)
        } else {
            Next::Idle
        }
    }

    /// Job `n` is finished (its results written): publish it, and wake the caller if it
    /// sleeps.
    pub fn complete(&self, n: u32) {
        let x = self.x();
        x.done.store(n, SeqCst);
        if x.caller_waiting.load(SeqCst) != 0 {
            self.theirs.ring();
        }
    }
}

/// A wake-up across processes: an eventcount over a 32-bit word in shared memory. The
/// sleeper reads [`Bell::epoch`], re-checks its condition, and [`Bell::wait`]s only while
/// the word still holds that epoch; [`Bell::ring`] moves the word and wakes it.
///
/// Windows: a named auto-reset event beside the word (a stale ring makes one wait return
/// early, which the loops above absorb). Linux: a futex on the word itself (shared, not
/// process-private). Other Unix: a short sleep loop on the word.
#[derive(Debug)]
pub struct Bell {
    word: *const AtomicU32,
    os: bell::Os,
}

// SAFETY: the word is an atomic in a mapping kept alive by the bell's owner; the OS
// object is a handle usable from any thread.
unsafe impl Send for Bell {}
// SAFETY: as above; every method takes `&self` and touches only atomics and the handle.
unsafe impl Sync for Bell {}

impl Bell {
    /// # Safety
    /// `word` stays valid (mapped) for the bell's life.
    pub unsafe fn create(name: &str, word: *const AtomicU32) -> io::Result<Bell> {
        Ok(Bell {
            word,
            os: bell::Os::create(name)?,
        })
    }

    /// # Safety
    /// As [`Bell::create`].
    pub unsafe fn open(name: &str, word: *const AtomicU32) -> io::Result<Bell> {
        Ok(Bell {
            word,
            os: bell::Os::open(name)?,
        })
    }

    fn word(&self) -> &AtomicU32 {
        // SAFETY: the constructors' contract.
        unsafe { &*self.word }
    }

    pub fn epoch(&self) -> u32 {
        self.word().load(SeqCst)
    }

    pub fn ring(&self) {
        self.word().fetch_add(1, SeqCst);
        self.os.wake(self.word());
    }

    /// Sleeps while the word holds `epoch`, for at most `timeout` (`None`: until rung).
    /// Returns `false` on a timeout. May return early (spuriously): re-check and loop.
    pub fn wait(&self, epoch: u32, timeout: Option<Duration>) -> bool {
        if self.word().load(SeqCst) != epoch {
            return true;
        }
        self.os.wait(self.word(), epoch, timeout)
    }
}

#[cfg(windows)]
mod bell {
    use std::io;
    use std::sync::atomic::AtomicU32;
    use std::time::Duration;
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0};
    use windows_sys::Win32::System::Threading::{
        CreateEventW, OpenEventW, SetEvent, WaitForSingleObject, EVENT_MODIFY_STATE, INFINITE,
        SYNCHRONIZATION_SYNCHRONIZE,
    };

    #[derive(Debug)]
    pub struct Os(HANDLE);

    fn wide(name: &str) -> Vec<u16> {
        format!("Local\\{name}")
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect()
    }

    impl Os {
        pub fn create(name: &str) -> io::Result<Os> {
            let w = wide(name);
            // SAFETY: an auto-reset event, not signalled, default security, a
            // NUL-terminated name that outlives the call.
            let h = unsafe { CreateEventW(std::ptr::null(), 0, 0, w.as_ptr()) };
            if h.is_null() {
                return Err(io::Error::last_os_error());
            }
            Ok(Os(h))
        }

        pub fn open(name: &str) -> io::Result<Os> {
            let w = wide(name);
            // SAFETY: a NUL-terminated name that outlives the call.
            let h = unsafe {
                OpenEventW(EVENT_MODIFY_STATE | SYNCHRONIZATION_SYNCHRONIZE, 0, w.as_ptr())
            };
            if h.is_null() {
                return Err(io::Error::last_os_error());
            }
            Ok(Os(h))
        }

        pub fn wake(&self, _word: &AtomicU32) {
            // SAFETY: our own event handle; setting it never blocks.
            unsafe { SetEvent(self.0) };
        }

        pub fn wait(&self, _word: &AtomicU32, _epoch: u32, timeout: Option<Duration>) -> bool {
            // Whole milliseconds, rounded up, so a short deadline is never a zero wait.
            let ms = timeout.map_or(INFINITE, |t| {
                let ms = t.as_micros().div_ceil(1000);
                ms.min(u128::from(INFINITE - 1)) as u32
            });
            // SAFETY: our own event handle.
            unsafe { WaitForSingleObject(self.0, ms) == WAIT_OBJECT_0 }
        }
    }

    impl Drop for Os {
        fn drop(&mut self) {
            // SAFETY: our own handle, closed once.
            unsafe { CloseHandle(self.0) };
        }
    }
}

#[cfg(target_os = "linux")]
mod bell {
    use std::io;
    use std::sync::atomic::AtomicU32;
    use std::time::Duration;

    /// The futex is the word itself: nothing to make or open.
    #[derive(Debug)]
    pub struct Os;

    impl Os {
        pub fn create(_name: &str) -> io::Result<Os> {
            Ok(Os)
        }

        pub fn open(_name: &str) -> io::Result<Os> {
            Ok(Os)
        }

        pub fn wake(&self, word: &AtomicU32) {
            // SAFETY: FUTEX_WAKE on an aligned 32-bit word in a shared mapping; no pointer
            // is dereferenced by the kernel beyond it.
            unsafe {
                libc::syscall(
                    libc::SYS_futex,
                    word.as_ptr(),
                    libc::FUTEX_WAKE,
                    i32::MAX,
                    std::ptr::null::<libc::timespec>(),
                    std::ptr::null::<u32>(),
                    0,
                );
            }
        }

        pub fn wait(&self, word: &AtomicU32, epoch: u32, timeout: Option<Duration>) -> bool {
            let ts = timeout.map(|t| libc::timespec {
                tv_sec: t.as_secs().min(i64::MAX as u64) as libc::time_t,
                tv_nsec: t.subsec_nanos() as libc::c_long,
            });
            let tp = ts
                .as_ref()
                .map_or(std::ptr::null(), |t| t as *const libc::timespec);
            // SAFETY: FUTEX_WAIT compares the word with `epoch` and sleeps atomically;
            // `tp` is null or points to a live relative timespec.
            let r = unsafe {
                libc::syscall(
                    libc::SYS_futex,
                    word.as_ptr(),
                    libc::FUTEX_WAIT,
                    epoch,
                    tp,
                    std::ptr::null::<u32>(),
                    0,
                )
            };
            !(r != 0 && io::Error::last_os_error().raw_os_error() == Some(libc::ETIMEDOUT))
        }
    }
}

#[cfg(all(unix, not(target_os = "linux")))]
mod bell {
    use std::io;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::time::{Duration, Instant};

    /// No cross-process futex here: a 50 µs sleep loop on the word.
    #[derive(Debug)]
    pub struct Os;

    impl Os {
        pub fn create(_name: &str) -> io::Result<Os> {
            Ok(Os)
        }

        pub fn open(_name: &str) -> io::Result<Os> {
            Ok(Os)
        }

        pub fn wake(&self, _word: &AtomicU32) {}

        pub fn wait(&self, word: &AtomicU32, epoch: u32, timeout: Option<Duration>) -> bool {
            let start = Instant::now();
            while word.load(Ordering::SeqCst) == epoch {
                if timeout.is_some_and(|t| start.elapsed() >= t) {
                    return false;
                }
                std::thread::sleep(Duration::from_micros(50));
            }
            true
        }
    }
}

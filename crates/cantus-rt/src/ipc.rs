//! Lock-step exchanges between two processes over shared memory (ADR 0063).
//!
//! A **caller** hands a job to a **group of workers** in another process and waits for it
//! to finish. Each job's data lives beside its [`Exchange`] in a
//! [`crate::shm::SharedMemory`] region; the workers of one process share a [`Lobby`]; this
//! module only moves the sequence numbers and the wake-ups:
//!
//! 1. The caller writes the job's data, then [`Caller::submit`]: `job += 1` (sequentially
//!    consistent, so the data is visible before the number), and — only when no worker of
//!    the lobby is spinning — rings the lobby's [`Bell`].
//! 2. A worker, spinning or woken, scans its exchanges and [`Exchange::take`]s a pending
//!    job (a compare-and-swap: one worker per job), reads the data, does the work, writes
//!    the results, and completes ([`Completer::complete`]): `done = job`, and — only when
//!    the caller has said it sleeps — rings the caller's bell.
//! 3. The caller spins on `done` for a bounded time, then sleeps on its own bell until a
//!    **deadline** ([`Caller::wait`]): done, late, or closed (the workers' process is gone,
//!    which whoever watches it marks with [`Closer::close`]).
//!
//! **Why a lobby.** The workers of a process serve all its exchanges: the one that just
//! finished a job keeps scanning for a short while ([`Lobby`] counts it as *spinning*), so
//! the next job of the same block — another plug-in in the same process, an instrument's
//! next note segment — is taken a cache-line hand-off later instead of waking a parked
//! thread. Only the first job after a quiet spell pays a wake-up; a lobby's spinning costs
//! one short spin per burst, not one per exchange (ADR 0031 R15's spinner budget).
//!
//! **No lost wake-up.** Each side announces before it checks the other's number for the
//! last time, and publishes its number before it reads the other's announcement — Dekker's
//! pattern under the sequentially consistent order of those operations: a caller that
//! sees no spinner rings, and a worker that stops spinning scans once more after saying so.
//! A worker that takes a job while no other spins, and finds another job waiting, rings
//! too, so jobs that arrive together run together. A bell is an eventcount: the sleeper
//! reads its word, announces, re-checks, and sleeps only while the word has not moved.
//! `tests/ipc_model.rs` explores every interleaving of two callers and two workers.
//!
//! A job not finished by the deadline is **late**: the caller goes on without it and does
//! not submit another until [`Caller::finished`] says the late one is done. Nothing here
//! allocates; the only system calls are a ring when the other side sleeps, and a sleep on
//! the caller's own bell once its spin budget is spent.
//!
//! Nothing here knows about audio (ADR 0031 §2.7).

use std::io;
use std::sync::atomic::{AtomicU32, Ordering::*};
use std::time::{Duration, Instant};

/// The words of one exchange, in shared memory: `#[repr(C)]`, all atomics, so both
/// processes agree on the layout. What the caller writes sits on one cache line, what the
/// workers write on another.
#[repr(C, align(128))]
#[derive(Debug, Default)]
pub struct Exchange {
    /// The last job the caller published.
    job: AtomicU32,
    _pad0: [u32; 31],
    /// The last job a worker took.
    taken: AtomicU32,
    /// The last job a worker finished.
    done: AtomicU32,
    /// 1 while the caller is about to sleep, or sleeps, on its bell.
    caller_waiting: AtomicU32,
    /// The caller's bell (an eventcount word).
    caller_bell: AtomicU32,
    /// Non-zero once the workers' side is gone: nothing will ever complete.
    closed: AtomicU32,
    _pad1: [u32; 27],
}

impl Exchange {
    /// Marks the workers' side gone. Whoever watches their process calls it, through a
    /// [`Closer`], which also wakes the caller.
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

    /// Whether a worker took a job and has not finished it (a job runs now).
    pub fn running(&self) -> bool {
        self.taken.load(SeqCst) != self.done.load(SeqCst)
    }

    /// Whether a job waits to be taken (a worker's scan).
    pub fn pending(&self) -> bool {
        self.job.load(SeqCst) != self.taken.load(SeqCst)
    }

    /// Takes the pending job, if there is one and no other worker took it first: its
    /// number. The job's data may be read until it is completed.
    pub fn take(&self) -> Option<u32> {
        let t = self.taken.load(SeqCst);
        let j = self.job.load(SeqCst);
        if j == t {
            return None;
        }
        self.taken
            .compare_exchange(t, j, SeqCst, SeqCst)
            .ok()
            .map(|_| j)
    }
}

/// The words a process's workers share, in a shared memory region of their own.
#[repr(C, align(128))]
#[derive(Debug, Default)]
pub struct Lobby {
    /// Workers scanning for jobs now (neither working nor parked).
    spinning: AtomicU32,
    /// The lobby's bell (an eventcount word).
    bell: AtomicU32,
    _pad: [u32; 30],
}

impl Lobby {
    /// Workers scanning now.
    pub fn spinning(&self) -> u32 {
        self.spinning.load(SeqCst)
    }

    /// A worker starts scanning (it woke, or finished a job).
    pub fn arrive(&self) {
        self.spinning.fetch_add(1, SeqCst);
    }

    /// A worker stops scanning (it took a job, or is about to park). After it, the worker
    /// must scan once more before it sleeps, and ring when it took a job while others wait
    /// ([`LobbyBell::ring_if_alone`]).
    pub fn leave(&self) {
        self.spinning.fetch_sub(1, SeqCst);
    }
}

/// How a wait ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// The job finished; its results may be read.
    Done,
    /// The deadline passed first: the job is still running, and its buffers belong to the
    /// workers until [`Caller::finished`] says otherwise.
    Late,
    /// The workers' side is gone.
    Closed,
}

/// The caller's half: its own bell to sleep on, the lobby's to ring.
#[derive(Debug)]
pub struct Caller {
    x: *const Exchange,
    lobby: *const Lobby,
    mine: Bell,
    theirs: Bell,
    /// What ends a sleep at its deadline to the microsecond, not the system timer's tick.
    timer: bell::Timer,
}

// SAFETY: the exchange and the lobby are atomics in mappings their owner keeps alive for
// this value's life (the `create` contract); bells are OS handles usable from any thread.
unsafe impl Send for Caller {}

impl Caller {
    /// Makes (or opens) the caller's bell under `name` for the exchange at `x`, and opens
    /// the lobby's bell made under `lobby_name` ([`LobbyBell::create`]).
    ///
    /// # Safety
    /// `x` and `lobby` point to live structures in shared memory that stay mapped for this
    /// value's life.
    pub unsafe fn create(
        name: &str,
        x: *const Exchange,
        lobby_name: &str,
        lobby: *const Lobby,
    ) -> io::Result<Caller> {
        // SAFETY: the words are inside `*x` and `*lobby`, alive as the caller promises.
        unsafe {
            Ok(Caller {
                mine: Bell::create(&format!("{name}-c"), &(*x).caller_bell)?,
                theirs: Bell::open(&format!("{lobby_name}-l"), &(*lobby).bell)?,
                timer: bell::Timer::make(),
                x,
                lobby,
            })
        }
    }

    fn x(&self) -> &Exchange {
        // SAFETY: the `create` contract keeps the exchange mapped while `self` lives.
        unsafe { &*self.x }
    }

    fn lobby(&self) -> &Lobby {
        // SAFETY: as above, for the lobby.
        unsafe { &*self.lobby }
    }

    /// Publishes the next job (its data already written) and wakes a worker if none is
    /// scanning. Returns the job's number.
    pub fn submit(&self) -> u32 {
        let x = self.x();
        let n = x.job.load(Relaxed).wrapping_add(1);
        x.job.store(n, SeqCst);
        if self.lobby().spinning.load(SeqCst) == 0 {
            self.theirs.ring();
        }
        n
    }

    /// Whether job `n` has finished (a late job, polled before the next one).
    pub fn finished(&self, n: u32) -> bool {
        self.x().done.load(Acquire) == n
    }

    /// Waits for job `n`: spins for `spin`, then sleeps on the caller's bell until
    /// `deadline` (`None`: for as long as it takes — offline work — while the workers
    /// live).
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
            if k.is_multiple_of(32) && start.elapsed() >= spin {
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
            self.mine.wait_with(epoch, left, &self.timer);
            x.caller_waiting.store(0, SeqCst);
        }
    }

    /// A handle another thread keeps to mark the workers' side gone and wake this caller
    /// (whoever watches their process). Opened by `name`, as made here.
    pub fn closer(&self, name: &str) -> io::Result<Closer> {
        // SAFETY: the same word as `self.mine`, in the same mapping; the closer's owner
        // keeps the mapping alive as long as the closer (the `Closer` contract).
        let bell = unsafe { Bell::open(&format!("{name}-c"), &self.x().caller_bell)? };
        Ok(Closer { x: self.x, bell })
    }
}

/// Marks an exchange's workers gone and wakes its caller ([`Caller::closer`]). Its owner
/// keeps the exchange's mapping alive while it holds one.
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

/// The workers' handle on one exchange: what completes its jobs and wakes its caller.
#[derive(Debug)]
pub struct Completer {
    x: *const Exchange,
    caller: Bell,
}

// SAFETY: as for `Closer`.
unsafe impl Send for Completer {}
// SAFETY: as for `Closer`; `complete` is called by the one worker that took the job.
unsafe impl Sync for Completer {}

impl Completer {
    /// Opens the caller's bell made under `name` for the exchange at `x`.
    ///
    /// # Safety
    /// `x` points to an `Exchange` in shared memory that stays mapped for this value's
    /// life.
    pub unsafe fn open(name: &str, x: *const Exchange) -> io::Result<Completer> {
        // SAFETY: the word is inside `*x`, alive as the caller promises.
        unsafe {
            Ok(Completer {
                caller: Bell::open(&format!("{name}-c"), &(*x).caller_bell)?,
                x,
            })
        }
    }

    pub fn exchange(&self) -> &Exchange {
        // SAFETY: the `open` contract.
        unsafe { &*self.x }
    }

    /// Job `n` (taken by this worker) is finished, its results written: publish it, and
    /// wake the caller if it sleeps.
    pub fn complete(&self, n: u32) {
        let x = self.exchange();
        x.done.store(n, SeqCst);
        if x.caller_waiting.load(SeqCst) != 0 {
            self.caller.ring();
        }
    }
}

/// The lobby's bell, made by the side that makes the lobby (Cantus) and opened by the
/// workers.
#[derive(Debug)]
pub struct LobbyBell {
    lobby: *const Lobby,
    bell: Bell,
}

// SAFETY: as for `Closer`.
unsafe impl Send for LobbyBell {}
// SAFETY: every method takes `&self` and touches atomics and the OS handle.
unsafe impl Sync for LobbyBell {}

impl LobbyBell {
    /// Makes the bell of the lobby at `lobby`, under `name`.
    ///
    /// # Safety
    /// `lobby` stays mapped for this value's life.
    pub unsafe fn create(name: &str, lobby: *const Lobby) -> io::Result<LobbyBell> {
        // SAFETY: the word is inside `*lobby`.
        unsafe {
            Ok(LobbyBell {
                bell: Bell::create(&format!("{name}-l"), &(*lobby).bell)?,
                lobby,
            })
        }
    }

    /// Opens it (a worker).
    ///
    /// # Safety
    /// As [`Self::create`].
    pub unsafe fn open(name: &str, lobby: *const Lobby) -> io::Result<LobbyBell> {
        // SAFETY: as above.
        unsafe {
            Ok(LobbyBell {
                bell: Bell::open(&format!("{name}-l"), &(*lobby).bell)?,
                lobby,
            })
        }
    }

    pub fn lobby(&self) -> &Lobby {
        // SAFETY: the constructors' contract.
        unsafe { &*self.lobby }
    }

    pub fn bell(&self) -> &Bell {
        &self.bell
    }

    /// A worker that took a job wakes another when none scans and more work waits
    /// (`more`), so jobs that arrive together run together.
    pub fn ring_if_alone(&self, more: bool) {
        if more && self.lobby().spinning() == 0 {
            self.bell.ring();
        }
    }
}

/// A worker's life (ADR 0063): scan and take a job, run it, and keep scanning for `spin`
/// after the last one; then say so, scan once more, and park on the lobby's bell (for at
/// most `park`, to look at `stop`). `take` scans the worker's exchanges and takes one
/// pending job (which one, and its number); `pending` says whether any other job waits;
/// `run` does a taken job's work and completes it ([`Completer::complete`]).
pub fn serve(
    bell: &LobbyBell,
    spin: Duration,
    park: Option<Duration>,
    stop: &std::sync::atomic::AtomicBool,
    mut take: impl FnMut() -> Option<(usize, u32)>,
    mut pending: impl FnMut() -> bool,
    mut run: impl FnMut(usize, u32),
) {
    let lobby = bell.lobby();
    lobby.arrive();
    let mut last = Instant::now();
    let mut k = 0u32;
    while !stop.load(Relaxed) {
        if let Some((i, n)) = take() {
            lobby.leave();
            bell.ring_if_alone(pending());
            run(i, n);
            lobby.arrive();
            last = Instant::now();
            continue;
        }
        k = k.wrapping_add(1);
        if !k.is_multiple_of(16) || last.elapsed() < spin {
            std::hint::spin_loop();
            continue;
        }
        // Park: the epoch, then stop counting as a spinner, then one last look.
        let epoch = bell.bell().epoch();
        lobby.leave();
        if let Some((i, n)) = take() {
            bell.ring_if_alone(pending());
            run(i, n);
            lobby.arrive();
            last = Instant::now();
            continue;
        }
        bell.bell().wait(epoch, park);
        lobby.arrive();
        last = Instant::now();
    }
    lobby.leave();
}

/// A wake-up across processes: an eventcount over a 32-bit word in shared memory. The
/// sleeper reads [`Bell::epoch`], re-checks its condition, and [`Bell::wait`]s only while
/// the word still holds that epoch; [`Bell::ring`] moves the word and wakes it.
///
/// Windows: a named auto-reset event beside the word (a stale ring makes one wait return
/// early, which the loops absorb). Linux: a futex on the word itself (shared, not
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

    /// As [`Bell::wait`], ending a timed sleep with `timer` — to the microsecond where the
    /// system's own wait would round up to its timer tick (15.6 ms on Windows by default).
    fn wait_with(&self, epoch: u32, timeout: Option<Duration>, timer: &bell::Timer) -> bool {
        if self.word().load(SeqCst) != epoch {
            return true;
        }
        self.os.wait_with(self.word(), epoch, timeout, timer)
    }
}
#[cfg(windows)]
mod bell {
    use std::io;
    use std::sync::atomic::AtomicU32;
    use std::time::Duration;
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0};
    use windows_sys::Win32::System::Threading::{
        CreateEventW, CreateWaitableTimerExW, OpenEventW, SetEvent, SetWaitableTimer,
        WaitForMultipleObjects, WaitForSingleObject, EVENT_MODIFY_STATE, INFINITE,
        SYNCHRONIZATION_SYNCHRONIZE,
    };

    /// `CREATE_WAITABLE_TIMER_HIGH_RESOLUTION` (Windows 10 1803 and later).
    const HIGH_RESOLUTION: u32 = 0x2;
    /// `TIMER_ALL_ACCESS`.
    const TIMER_ALL_ACCESS: u32 = 0x1F_0003;

    /// A high-resolution waitable timer, made once per caller (off the audio thread);
    /// `None` where the system has none, and the wait rounds to the tick as before.
    #[derive(Debug)]
    pub struct Timer(Option<HANDLE>);

    impl Timer {
        pub fn make() -> Timer {
            // SAFETY: an unnamed timer with default security; no pointers kept.
            let h = unsafe {
                CreateWaitableTimerExW(
                    std::ptr::null(),
                    std::ptr::null(),
                    HIGH_RESOLUTION,
                    TIMER_ALL_ACCESS,
                )
            };
            Timer((!h.is_null()).then_some(h))
        }
    }

    impl Drop for Timer {
        fn drop(&mut self) {
            if let Some(h) = self.0 {
                // SAFETY: our own handle, closed once.
                unsafe { CloseHandle(h) };
            }
        }
    }

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
                OpenEventW(
                    EVENT_MODIFY_STATE | SYNCHRONIZATION_SYNCHRONIZE,
                    0,
                    w.as_ptr(),
                )
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

        pub fn wait_with(
            &self,
            word: &AtomicU32,
            epoch: u32,
            timeout: Option<Duration>,
            timer: &Timer,
        ) -> bool {
            let (Some(t), Some(th)) = (timeout, timer.0) else {
                return self.wait(word, epoch, timeout);
            };
            // Relative due time, in 100-ns units (negative), at least one unit.
            let due = -((t.as_nanos() / 100).clamp(1, i64::MAX as u128) as i64);
            // SAFETY: our own timer handle; `due` outlives the call; no completion routine.
            let armed = unsafe { SetWaitableTimer(th, &due, 0, None, std::ptr::null(), 0) };
            if armed == 0 {
                return self.wait(word, epoch, timeout);
            }
            let both = [self.0, th];
            // SAFETY: two live handles of ours; arming the timer reset its state, so it
            // signals only at this deadline.
            unsafe { WaitForMultipleObjects(2, both.as_ptr(), 0, INFINITE) == WAIT_OBJECT_0 }
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

    /// A futex's timeout is already exact: nothing more.
    #[derive(Debug)]
    pub struct Timer;

    impl Timer {
        pub fn make() -> Timer {
            Timer
        }
    }

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

        pub fn wait_with(
            &self,
            word: &AtomicU32,
            epoch: u32,
            timeout: Option<Duration>,
            _timer: &Timer,
        ) -> bool {
            self.wait(word, epoch, timeout)
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

    /// The sleep loop's timeout is its own: nothing more.
    #[derive(Debug)]
    pub struct Timer;

    impl Timer {
        pub fn make() -> Timer {
            Timer
        }
    }

    impl Os {
        pub fn create(_name: &str) -> io::Result<Os> {
            Ok(Os)
        }

        pub fn open(_name: &str) -> io::Result<Os> {
            Ok(Os)
        }

        pub fn wake(&self, _word: &AtomicU32) {}

        pub fn wait_with(
            &self,
            word: &AtomicU32,
            epoch: u32,
            timeout: Option<Duration>,
            _timer: &Timer,
        ) -> bool {
            self.wait(word, epoch, timeout)
        }

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

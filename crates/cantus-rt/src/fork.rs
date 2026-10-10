//! Fork–join inside a task (ADR 0038): a thread that is running a task publishes a few
//! jobs — closures that borrow its stack — the pool's idle threads take them, the
//! publisher takes them too, and returns once every job has returned.
//!
//! The one piece of this that safe Rust cannot say is "a reference to my stack, lent to
//! another thread, for less time than my frame lives". [`Forks::run`] keeps that promise
//! itself: it does not return — not even by unwinding — until every job it published has
//! finished and no helper is still looking at its slot.
//!
//! **Protocol.** A slot is `FREE`, `SETUP` (one forker filling it), `OPEN` (jobs claimable)
//! or `CLOSING`. The forker takes a free slot by a compare-and-swap, writes the jobs'
//! address and count, and opens it (`Release`). A helper registers in the slot's `active`
//! count *before* it reads the state (both `SeqCst`), and claims job indices with one
//! `fetch_add` each; every index below the count is claimed by exactly one thread, which
//! is what makes its `&mut` exclusive, and it runs its jobs while still registered. The
//! forker claims too until none is left, then closes the slot and waits until `active` is
//! zero — which is every job a helper took having returned (it waits only for jobs already
//! running, never for a thread to wake). A helper that registers after that check sees the
//! slot closed (the `SeqCst` order of the two stores and two loads), so it never reads the
//! forker's jobs after they are gone.
//!
//! **Panics.** A helper that unwinds leaves `active` (a guard); a forker that unwinds first
//! claims every job nobody took — without running it — then waits as above. Nothing here
//! allocates, locks or makes a system call.
//!
//! **Testing hook** (the `fuzz` feature): [`Forks::set_hook`] installs a callback the
//! protocol calls at each [`ForkSite`] — the windows its safety rests on — so the engine's
//! schedule fuzz can stall a thread inside them. Without the feature the sites compile to
//! nothing.

use std::cell::Cell;
use std::sync::atomic::{AtomicPtr, AtomicU64, AtomicU8, AtomicUsize, Ordering::*};

use crate::pad::CachePadded;

/// A job as the forker lends it: run once, on any thread.
pub type Job<'a> = Option<&'a mut (dyn FnMut() + Send + 'a)>;
type Erased = Job<'static>;

const FREE: u8 = 0;
const SETUP: u8 = 1;
const OPEN: u8 = 2;
const CLOSING: u8 = 3;

/// A point inside the fork protocol where a test may stall the thread that reached it
/// ([`Forks::set_hook`], `fuzz` feature only).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ForkSite {
    /// The forker opened its slot, before it claims its first job (helpers may take all).
    Opened,
    /// A helper registered in a slot's `active`, before it re-reads the slot's state (the
    /// Dekker pair against the forker's `CLOSING` store and `active` load).
    Registered,
    /// A thread claimed a job index, before it runs that job.
    Claimed,
    /// The forker marked its slot closing, before it waits for `active` to reach zero.
    Closing,
}

/// The callback [`Forks::set_hook`] installs.
#[cfg(feature = "fuzz")]
pub type ForkHook = std::sync::Arc<dyn Fn(ForkSite) + Send + Sync>;

/// Where a test's hook lives: set once, read with one load per site.
#[cfg(feature = "fuzz")]
#[derive(Default)]
struct HookCell(std::sync::OnceLock<ForkHook>);

#[cfg(feature = "fuzz")]
impl std::fmt::Debug for HookCell {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(if self.0.get().is_some() {
            "hook"
        } else {
            "no hook"
        })
    }
}

/// How many forks may be open at once (one per task running a fork); a forker that finds
/// none free runs its jobs alone.
pub const SLOTS: usize = 4;

#[derive(Debug)]
struct Slot {
    state: AtomicU8,
    jobs: AtomicPtr<Erased>,
    len: AtomicUsize,
    next: AtomicUsize,
    active: AtomicUsize,
}

impl Slot {
    const fn new() -> Self {
        Slot {
            state: AtomicU8::new(FREE),
            jobs: AtomicPtr::new(std::ptr::null_mut()),
            len: AtomicUsize::new(0),
            next: AtomicUsize::new(0),
            active: AtomicUsize::new(0),
        }
    }

    /// Claim and run jobs until none is left. Only while registered in `active` with the
    /// slot open, or as its forker.
    fn claim(&self, jobs: *mut Erased, len: usize, forks: &Forks) -> usize {
        let mut ran = 0;
        loop {
            let i = self.next.fetch_add(1, AcqRel);
            if i >= len {
                return ran;
            }
            forks.site(ForkSite::Claimed);
            // SAFETY: `i < len` was claimed by this thread alone (one `fetch_add` per
            // index), so this is the only access to element `i`; the forker keeps the
            // slice alive until every index is claimed and `active == 0`, and this thread
            // is either the forker or registered in `active` and saw the slot open.
            let job = unsafe { (*jobs.add(i)).take() };
            if let Some(f) = job {
                f();
            }
            ran += 1;
        }
    }
}

/// Leaves a slot's `active` count when dropped.
struct Active<'a>(&'a AtomicUsize);

impl Drop for Active<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, SeqCst);
    }
}

/// The forks a pool shares: open slots, and who is around to help.
#[derive(Debug)]
pub struct Forks {
    slots: [CachePadded<Slot>; SLOTS],
    /// Threads inside the pool's block right now (they look for jobs when idle).
    present: CachePadded<AtomicUsize>,
    /// Jobs run by helpers (not by their forker), and forks published, since built.
    helped: AtomicU64,
    forks: AtomicU64,
    #[cfg(feature = "fuzz")]
    hook: HookCell,
}

impl Default for Forks {
    fn default() -> Self {
        Self::new()
    }
}

impl Forks {
    pub fn new() -> Self {
        Forks {
            slots: std::array::from_fn(|_| CachePadded::new(Slot::new())),
            present: CachePadded::new(AtomicUsize::new(0)),
            helped: AtomicU64::new(0),
            forks: AtomicU64::new(0),
            #[cfg(feature = "fuzz")]
            hook: HookCell::default(),
        }
    }

    /// Install a test's hook, called at every [`ForkSite`] by the thread that reached it
    /// (`fuzz` feature). Once per `Forks`: `false` if one was already installed.
    #[cfg(feature = "fuzz")]
    pub fn set_hook(&self, hook: ForkHook) -> bool {
        self.hook.0.set(hook).is_ok()
    }

    /// A [`ForkSite`] reached: the test's hook, with the `fuzz` feature; nothing otherwise.
    #[inline(always)]
    fn site(&self, site: ForkSite) {
        #[cfg(feature = "fuzz")]
        if let Some(h) = self.hook.0.get() {
            h(site);
        }
        #[cfg(not(feature = "fuzz"))]
        let _ = site;
    }

    /// A thread of the pool entered (`true`) or left (`false`) the part of a block where
    /// it looks for work.
    #[inline]
    pub fn presence(&self, inside: bool) {
        if inside {
            self.present.fetch_add(1, SeqCst);
        } else {
            self.present.fetch_sub(1, SeqCst);
        }
    }

    /// Threads that could take part in a fork now (a forker counts itself among them).
    #[inline]
    pub fn present(&self) -> usize {
        self.present.load(Relaxed)
    }

    /// (jobs run by helpers, forks published) since built.
    pub fn counts(&self) -> (u64, u64) {
        (self.helped.load(Relaxed), self.forks.load(Relaxed))
    }

    /// Whether any fork is open now (a hint, one load per slot): an idle thread asks this
    /// before it reads a clock to time its help.
    #[inline]
    pub fn any_open(&self) -> bool {
        self.slots.iter().any(|s| s.state.load(Relaxed) == OPEN)
    }

    /// Take jobs from any open fork; `true` if this thread ran at least one. For an idle
    /// thread of the pool.
    pub fn help(&self) -> bool {
        let mut ran = 0;
        for s in &self.slots {
            if s.state.load(Relaxed) != OPEN {
                continue;
            }
            s.active.fetch_add(1, SeqCst);
            let _active = Active(&s.active);
            self.site(ForkSite::Registered);
            if s.state.load(SeqCst) == OPEN {
                let (jobs, len) = (s.jobs.load(Acquire), s.len.load(Acquire));
                ran += s.claim(jobs, len, self);
            }
        }
        if ran > 0 {
            self.helped.fetch_add(ran as u64, Relaxed);
        }
        ran > 0
    }

    /// Run every `Some` job once, on this thread and on any helper, and return when all
    /// have returned. With one job, no free slot or nobody present, it runs them here, in
    /// order.
    pub fn run(&self, jobs: &mut [Job<'_>]) {
        let len = jobs.len();
        let free = (len > 1 && self.present() > 1)
            .then(|| {
                self.slots.iter().find(|s| {
                    s.state
                        .compare_exchange(FREE, SETUP, Acquire, Relaxed)
                        .is_ok()
                })
            })
            .flatten();
        let Some(slot) = free else {
            for j in jobs.iter_mut() {
                if let Some(f) = j.take() {
                    f();
                }
            }
            return;
        };
        // The lifetime is erased here and restored by `Close` below, which does not let
        // this frame end while anyone can reach the jobs.
        let ptr = jobs.as_mut_ptr().cast::<Erased>();
        slot.jobs.store(ptr, Relaxed);
        slot.len.store(len, Relaxed);
        slot.next.store(0, Relaxed);
        let close = Close {
            slot,
            forks: self,
            len,
            claimed_all: Cell::new(false),
        };
        slot.state.store(OPEN, SeqCst);
        self.forks.fetch_add(1, Relaxed);
        self.site(ForkSite::Opened);
        slot.claim(ptr, len, self);
        close.claimed_all.set(true);
        drop(close);
    }
}

/// Ends a fork: closes the slot, waits for the helpers (and the jobs they took) to leave,
/// frees it. On a panic it first claims whatever nobody took.
struct Close<'a> {
    slot: &'a Slot,
    forks: &'a Forks,
    len: usize,
    claimed_all: Cell<bool>,
}

impl Drop for Close<'_> {
    fn drop(&mut self) {
        let s = self.slot;
        if !self.claimed_all.get() {
            while s.next.fetch_add(1, AcqRel) < self.len {}
        }
        s.state.store(CLOSING, SeqCst);
        self.forks.site(ForkSite::Closing);
        while s.active.load(SeqCst) != 0 {
            std::hint::spin_loop();
        }
        s.jobs.store(std::ptr::null_mut(), Relaxed);
        s.state.store(FREE, Release);
    }
}

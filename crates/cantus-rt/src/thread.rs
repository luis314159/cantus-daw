//! Thread priority and placement (ADR 0031 §2.6, amended by R1, R3, R4, R5 — and by what
//! round 21 measured: **a real-time class that throttles must never hold a spinning thread**).
//!
//! - [`promote_callback`]: the audio callback's thread, on the first block after a stream
//!   starts. On **WASAPI** the thread is cpal's, at normal priority (cpal promotes only under
//!   its `realtime` feature, and then asks MMCSS for "Audio"): it joins MMCSS **"Pro Audio"**
//!   at **HIGH**. On **ASIO**, CoreAudio and JACK the driver owns the thread and has already
//!   made it real-time: nothing is done. One system call per stream start, never per block
//!   (ADR 0005 rule 3 as amended by ADR 0031 §2.4); later calls on the same thread return the
//!   first answer from a thread-local, with no system call.
//! - [`promote_worker`]: a pool thread, by role **and by how it waits** ([`Waiting`]). Every
//!   OS's real-time class polices how much of each period a thread may use: MMCSS demotes a
//!   thread past `100 − SystemResponsiveness` % (80 % by default) to priority 1–7 for the
//!   rest of the period — behind every compiler on a busy machine, which round 21 measured as
//!   blocks of 0.1–1 s (rt) and 2.5–17.6 s (sched); macOS demotes a time-constraint thread
//!   that keeps overrunning its computation; rtkit sets `RLIMIT_RTTIME`, and a real-time
//!   thread that runs that long without blocking is killed. So a worker that **spins** gets
//!   the highest priority no OS throttles (Windows `THREAD_PRIORITY_TIME_CRITICAL`; Linux
//!   `SCHED_FIFO` only when `RLIMIT_RTTIME` is unlimited; nothing on macOS), and only a worker
//!   whose busy share stays under [`realtime_share`] — spin a bounded time, then park — gets
//!   the real-time class (MMCSS "Pro Audio", the time constraint and the workgroup, rtkit).
//!   Reverted when the returned guard drops.
//! - [`set_current_thread_cpus`]: soft placement — Windows CPU sets (which the power manager
//!   respects), Linux affinity; macOS has none.
//! - [`opt_out_of_power_throttling`]: the process leaves EcoQoS (R1), which Steinberg tells
//!   Cubase users to do by hand after prefetch threads dropped out on E-cores.
//!
//! The callback is in the real-time class because it blocks every period in the driver: its
//! busy share is the engine's load, which must stay under [`realtime_share`] anyway — past it
//! the device drops out before MMCSS would throttle.

use std::cell::Cell;
use std::fmt;
use std::time::Duration;

#[cfg(target_os = "linux")]
#[path = "thread/linux.rs"]
mod os;
#[cfg(target_os = "macos")]
#[path = "thread/macos.rs"]
mod os;
#[cfg(all(windows, not(miri)))]
#[path = "thread/windows.rs"]
mod os;
#[cfg(not(any(target_os = "linux", target_os = "macos", all(windows, not(miri)))))]
#[path = "thread/other.rs"]
mod os;

#[cfg(target_os = "macos")]
pub use os::Workgroup;

/// The driver API the callback runs under.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AudioHost {
    Asio,
    Wasapi,
    CoreAudio,
    Jack,
    /// PipeWire's native API (Linux, ADR 0065).
    PipeWire,
    Alsa,
    /// Anything else whose callback thread is ours (cpal's) — promoted like WASAPI/ALSA.
    Other,
}

impl AudioHost {
    /// From a driver API's name as cpal gives it (`HostId::name()`: "ASIO", "WASAPI",
    /// "CoreAudio", "JACK", "PipeWire", "ALSA"), case-insensitive; anything else is
    /// [`AudioHost::Other`].
    pub fn from_driver_name(name: &str) -> AudioHost {
        match name.to_ascii_lowercase().as_str() {
            "asio" => AudioHost::Asio,
            "wasapi" => AudioHost::Wasapi,
            "coreaudio" => AudioHost::CoreAudio,
            "jack" => AudioHost::Jack,
            "pipewire" => AudioHost::PipeWire,
            "alsa" => AudioHost::Alsa,
            _ => AudioHost::Other,
        }
    }

    /// True when the driver creates the callback thread and gives it real-time priority
    /// itself, so promoting it again would only fight the driver. JACK's and PipeWire's
    /// threads are made real-time by their client libraries (through rtkit when the
    /// process may not ask itself); on Linux a thread they left at normal priority is
    /// promoted by [`promote_callback`] after all.
    pub fn driver_owns_priority(self) -> bool {
        matches!(
            self,
            AudioHost::Asio | AudioHost::CoreAudio | AudioHost::Jack | AudioHost::PipeWire
        )
    }
}

/// A pool thread's role, which decides its priority (ADR 0031 §2.6, R3).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum WorkerRole {
    /// Renders the current block beside the callback.
    Live,
    /// Renders ahead of the device (anticipation): a deadline too, a longer one.
    Ahead,
    /// Freeze and export: throughput, no deadline.
    Background,
}

/// How a pool thread waits between tasks — which decides the promotion that is safe for it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Waiting {
    /// Spins (or may spin) through most of a period — the pool's spin-for-1.5-periods
    /// discipline at steady state. Never given a throttled real-time class.
    Spins,
    /// Busy — working plus spinning — at most `busy_share` of each period, then blocks (parks)
    /// until woken. Given the real-time class when `busy_share` < [`realtime_share`].
    Bounded { busy_share: f32 },
}

impl Waiting {
    /// True when the real-time class is safe for a thread that waits this way: bounded, and
    /// at least [`SHARE_MARGIN`] under [`realtime_share`] (round 21 measured the MMCSS
    /// threshold at exactly 80 %: a 61 % busy share ran clean, 81 % was throttled).
    pub fn realtime_safe(self) -> bool {
        matches!(self, Waiting::Bounded { busy_share } if busy_share <= realtime_share() - SHARE_MARGIN)
    }
}

/// How far under [`realtime_share`] a bounded thread's busy share must stay.
pub const SHARE_MARGIN: f32 = 0.15;

/// The share of each period a thread may be busy in this OS's real-time class before it is
/// throttled: Windows `1 − SystemResponsiveness/100` (0.8 by default, read from the registry
/// once); Linux 0.95 (`sched_rt_runtime_us`); macOS 1.0 (the computation is set from the
/// declared share). Pool threads must stay *under* it, with margin.
pub fn realtime_share() -> f32 {
    static SHARE: std::sync::OnceLock<f32> = std::sync::OnceLock::new();
    *SHARE.get_or_init(os::realtime_share)
}

/// What a promotion achieved.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Promotion {
    /// The thread now runs with the named real-time treatment.
    Granted(Grant),
    /// Nothing was asked: the driver owns this thread and its priority (ASIO, CoreAudio,
    /// JACK), or the role runs at normal priority on this OS by design.
    LeftAsIs(&'static str),
    /// The OS refused; the thread keeps its previous priority.
    Refused(Refusal),
}

/// The real-time treatment granted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Grant {
    /// Windows MMCSS: the task name, the priority inside it, the task index MMCSS assigned.
    Mmcss {
        task: &'static str,
        priority: MmcssPriority,
        task_index: u32,
    },
    /// macOS `THREAD_TIME_CONSTRAINT_POLICY`, and whether the thread joined a workgroup.
    TimeConstraint {
        period_ns: u64,
        computation_ns: u64,
        joined_workgroup: bool,
    },
    /// Windows `SetThreadPriority` without MMCSS (15 = `THREAD_PRIORITY_TIME_CRITICAL`, 2 =
    /// `HIGHEST`): for threads that spin, which MMCSS would throttle.
    ThreadPriority { level: i32 },
    /// Linux real-time scheduling: the policy and priority, and whether rtkit granted it.
    RealTime {
        fifo: bool,
        priority: i32,
        via_rtkit: bool,
    },
}

/// MMCSS priority inside a task (`AVRT_PRIORITY`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MmcssPriority {
    Low,
    Normal,
    High,
}

/// Why a request failed: the OS call and its error code (`GetLastError`, `errno`, or the
/// Mach `kern_return_t`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Refusal {
    pub step: &'static str,
    pub code: i32,
}

impl Promotion {
    /// True when the thread runs at real-time priority now — granted here, or by its driver.
    pub fn is_realtime(&self) -> bool {
        match self {
            Promotion::Granted(_) => true,
            Promotion::LeftAsIs(why) => why.starts_with("driver"),
            Promotion::Refused(_) => false,
        }
    }
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.code == 0 {
            return write!(f, "{}", self.step);
        }
        let os = std::io::Error::from_raw_os_error(self.code);
        write!(f, "{} failed: {} (code {})", self.step, os, self.code)
    }
}

impl fmt::Display for Promotion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Promotion::Granted(Grant::Mmcss {
                task,
                priority,
                task_index,
            }) => {
                write!(
                    f,
                    "MMCSS \"{task}\" at {priority:?} (task index {task_index})"
                )
            }
            Promotion::Granted(Grant::TimeConstraint {
                period_ns,
                computation_ns,
                joined_workgroup,
            }) => write!(
                f,
                "time constraint, period {:.2} ms, computation {:.2} ms{}",
                *period_ns as f64 / 1e6,
                *computation_ns as f64 / 1e6,
                if *joined_workgroup {
                    ", in the device's workgroup"
                } else {
                    ""
                }
            ),
            Promotion::Granted(Grant::ThreadPriority { level }) => {
                write!(f, "thread priority {level} (no MMCSS: the thread spins)")
            }
            Promotion::Granted(Grant::RealTime {
                fifo,
                priority,
                via_rtkit,
            }) => write!(
                f,
                "SCHED_{} priority {priority}{}",
                if *fifo { "FIFO" } else { "RR" },
                if *via_rtkit { " via rtkit" } else { "" }
            ),
            Promotion::LeftAsIs(why) => f.write_str(why),
            Promotion::Refused(r) => write!(f, "refused: {r}"),
        }
    }
}

thread_local! {
    // The first answer on this thread. Const-initialised, no destructor: no allocation and
    // no registration, so reading it on the audio thread is a plain thread-local load.
    static CALLBACK: Cell<Option<Promotion>> = const { Cell::new(None) };
}

/// Promotes the calling thread — the audio callback's — for `host`, the first time it is
/// called on this thread; later calls return the first answer without a system call.
/// `period` is the device period (Linux uses it to set `RLIMIT_RTTIME` for rtkit).
///
/// No revert: the registration ends with the thread, which the driver's stream owns.
pub fn promote_callback(host: AudioHost, period: Duration) -> Promotion {
    if let Some(p) = CALLBACK.with(Cell::get) {
        return p;
    }
    #[cfg(target_os = "linux")]
    let linux_driver_left_it =
        matches!(host, AudioHost::Jack | AudioHost::PipeWire) && os::policy_now() == Some(false);
    #[cfg(not(target_os = "linux"))]
    let linux_driver_left_it = false;
    let p = if host.driver_owns_priority() && !linux_driver_left_it {
        driver_owned()
    } else {
        os::promote(
            WorkerRole::Live,
            period,
            Some(0.5),
            &mut os::Revert::default(),
        )
    };
    CALLBACK.with(|c| c.set(Some(p)));
    // For the device page: never waits (a frame reading it now loses this one answer).
    if let Ok(mut seen) = SEEN_CALLBACK.try_lock() {
        *seen = Some(p);
    }
    p
}

/// What the audio threads were given, for the device page (ADR 0065): the callback's
/// promotion (the latest stream's) and each live pool worker's.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PriorityReport {
    pub callback: Option<Promotion>,
    pub workers: Vec<Promotion>,
}

impl PriorityReport {
    /// How many workers run at real-time priority.
    pub fn realtime_workers(&self) -> usize {
        self.workers.iter().filter(|p| p.is_realtime()).count()
    }
}

/// The most workers the report keeps (more than any machine's pool).
const SEEN_MAX: usize = 64;

static SEEN_CALLBACK: std::sync::Mutex<Option<Promotion>> = std::sync::Mutex::new(None);
#[allow(clippy::type_complexity)]
static SEEN_WORKERS: std::sync::Mutex<[Option<(std::thread::ThreadId, Promotion)>; SEEN_MAX]> =
    std::sync::Mutex::new([None; SEEN_MAX]);

fn seen_workers(
) -> std::sync::MutexGuard<'static, [Option<(std::thread::ThreadId, Promotion)>; SEEN_MAX]> {
    SEEN_WORKERS.lock().unwrap_or_else(|e| e.into_inner())
}

/// The answer a live worker thread got, while its promotion lives.
pub fn worker_promotion_of(thread: std::thread::ThreadId) -> Option<Promotion> {
    seen_workers()
        .iter()
        .flatten()
        .find(|(id, _)| *id == thread)
        .map(|(_, p)| *p)
}

/// What the audio threads have now (see [`PriorityReport`]).
pub fn priority_report() -> PriorityReport {
    let callback = *SEEN_CALLBACK.lock().unwrap_or_else(|e| e.into_inner());
    let workers = seen_workers().iter().flatten().map(|(_, p)| *p).collect();
    PriorityReport { callback, workers }
}

/// What a driver-owned callback thread is left with. On Linux the policy the driver's
/// library gave it is named (JACK's and PipeWire's threads run `SCHED_FIFO`, or `SCHED_RR`
/// when rtkit granted it).
fn driver_owned() -> Promotion {
    #[cfg(target_os = "linux")]
    if let Some(name) = os::realtime_policy_name() {
        return Promotion::LeftAsIs(name);
    }
    Promotion::LeftAsIs("driver-owned thread: real-time already, left alone")
}

/// The answer [`promote_callback`] gave on this thread, if it was called.
pub fn callback_promotion() -> Option<Promotion> {
    CALLBACK.with(Cell::get)
}

/// A pool thread's promotion; reverted when dropped. Not `Send`: it must drop on the thread
/// it promoted.
#[derive(Debug)]
pub struct WorkerPromotion {
    pub promotion: Promotion,
    revert: os::Revert,
    _not_send: std::marker::PhantomData<*const ()>,
}

impl Drop for WorkerPromotion {
    fn drop(&mut self) {
        os::revert(&mut self.revert);
        let me = std::thread::current().id();
        for slot in seen_workers().iter_mut() {
            if slot.is_some_and(|(id, _)| id == me) {
                *slot = None;
            }
        }
    }
}

impl WorkerPromotion {
    /// Keep this thread's answer for [`priority_report`] while the promotion lives. Off the
    /// audio path: a worker calls it once, before it serves.
    fn noted(self) -> Self {
        let me = std::thread::current().id();
        let mut seen = seen_workers();
        if let Some(slot) = seen.iter_mut().find(|s| s.is_none()) {
            *slot = Some((me, self.promotion));
        }
        drop(seen);
        self
    }
}

/// Promotes the calling pool thread for `role`, safely for how it `waits` (module docs):
/// the real-time class only when `waiting.realtime_safe()`, else the highest priority no OS
/// throttles. `period` is the device period (macOS' constraint, Linux's `RLIMIT_RTTIME`).
pub fn promote_worker(role: WorkerRole, period: Duration, waiting: Waiting) -> WorkerPromotion {
    let mut revert = os::Revert::default();
    let promotion = os::promote(role, period, share(waiting), &mut revert);
    WorkerPromotion {
        promotion,
        revert,
        _not_send: std::marker::PhantomData,
    }
    .noted()
}

/// macOS: the time-constraint policy **then** `os_workgroup_join` (in that order, or the join
/// fails with `EINVAL`). A refusal from step `"os_workgroup_join"` with `EINVAL` means the
/// workgroup was cancelled (the device changed): fetch the new one and call again.
#[cfg(target_os = "macos")]
pub fn promote_worker_in(
    wg: &Workgroup,
    role: WorkerRole,
    period: Duration,
    waiting: Waiting,
) -> WorkerPromotion {
    let mut revert = os::Revert::default();
    let promotion = os::promote_in(wg, role, period, share(waiting), &mut revert);
    WorkerPromotion {
        promotion,
        revert,
        _not_send: std::marker::PhantomData,
    }
}

/// The busy share to promote with, `None` when the thread must not enter the real-time class.
fn share(waiting: Waiting) -> Option<f32> {
    match waiting {
        Waiting::Bounded { busy_share } if waiting.realtime_safe() => Some(busy_share.max(0.05)),
        _ => None,
    }
}

/// Places the calling thread on these logical processors (ids from [`crate::topology`]), as
/// a preference the OS power manager respects (Windows CPU sets) or an affinity (Linux).
pub fn set_current_thread_cpus(ids: &[u32]) -> Result<(), Refusal> {
    os::set_cpus(ids)
}

/// The process-wide power-throttling state [`opt_out_of_power_throttling`] left.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PowerThrottling {
    /// Opted out of EcoQoS; `timer_resolution` says whether Windows will also keep honouring
    /// the process's timer resolution when it is minimised or occluded.
    OptedOut {
        timer_resolution: bool,
    },
    NotApplicable,
    Refused(Refusal),
}

impl fmt::Display for PowerThrottling {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PowerThrottling::OptedOut { timer_resolution } => write!(
                f,
                "EcoQoS opted out{}",
                if *timer_resolution {
                    ", timer resolution kept"
                } else {
                    ""
                }
            ),
            PowerThrottling::NotApplicable => f.write_str("no power throttling on this OS"),
            PowerThrottling::Refused(r) => write!(f, "EcoQoS opt-out refused: {r}"),
        }
    }
}

/// Takes the whole process out of EcoQoS: `SetProcessInformation(ProcessPowerThrottling)`
/// with `EXECUTION_SPEED` controlled and off (HighQoS), and the timer resolution kept.
pub fn opt_out_of_power_throttling() -> PowerThrottling {
    os::opt_out_of_power_throttling()
}

/// The calling thread's OS priority as the OS reports it (Windows `GetThreadPriority`; Linux
/// and macOS the `sched_param` priority), for the probe and the tests.
pub fn current_priority() -> Option<i32> {
    os::priority()
}

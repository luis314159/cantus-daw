//! Thread priority and placement (ADR 0031 §2.6, amended by R1, R3, R4, R5).
//!
//! - [`promote_callback`]: the audio callback's thread, on the first block after a stream
//!   starts. On **WASAPI** the thread is cpal's, at normal priority (cpal promotes only under
//!   its `realtime` feature, and then asks MMCSS for "Audio"): it joins MMCSS **"Pro Audio"**
//!   at **HIGH**. On **ASIO**, CoreAudio and JACK the driver owns the thread and has already
//!   made it real-time: nothing is done. One system call per stream start, never per block
//!   (ADR 0005 rule 3 as amended by ADR 0031 §2.4); later calls on the same thread return the
//!   first answer from a thread-local, with no system call.
//! - [`promote_worker`]: a pool thread, by role — live workers "Pro Audio" HIGH, ahead workers
//!   "Pro Audio" LOW, background threads "Audio" NORMAL on Windows; the time-constraint policy
//!   (and, with a device workgroup, `os_workgroup_join` after it) on macOS; rtkit's SCHED_RR or
//!   SCHED_FIFO on Linux. Reverted when the returned guard drops.
//! - [`set_current_thread_cpus`]: soft placement — Windows CPU sets (which the power manager
//!   respects), Linux affinity; macOS has none.
//! - [`opt_out_of_power_throttling`]: the process leaves EcoQoS (R1), which Steinberg tells
//!   Cubase users to do by hand after prefetch threads dropped out on E-cores.

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
    Alsa,
    /// Anything else whose callback thread is ours (cpal's) — promoted like WASAPI/ALSA.
    Other,
}

impl AudioHost {
    /// True when the driver creates the callback thread and gives it real-time priority
    /// itself, so promoting it again would only fight the driver.
    pub fn driver_owns_priority(self) -> bool {
        matches!(self, AudioHost::Asio | AudioHost::CoreAudio | AudioHost::Jack)
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
    Mmcss { task: &'static str, priority: MmcssPriority, task_index: u32 },
    /// macOS `THREAD_TIME_CONSTRAINT_POLICY`, and whether the thread joined a workgroup.
    TimeConstraint { period_ns: u64, computation_ns: u64, joined_workgroup: bool },
    /// Linux real-time scheduling: the policy and priority, and whether rtkit granted it.
    RealTime { fifo: bool, priority: i32, via_rtkit: bool },
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
            Promotion::Granted(Grant::Mmcss { task, priority, task_index }) => {
                write!(f, "MMCSS \"{task}\" at {priority:?} (task index {task_index})")
            }
            Promotion::Granted(Grant::TimeConstraint { period_ns, computation_ns, joined_workgroup }) => write!(
                f,
                "time constraint, period {:.2} ms, computation {:.2} ms{}",
                *period_ns as f64 / 1e6,
                *computation_ns as f64 / 1e6,
                if *joined_workgroup { ", in the device's workgroup" } else { "" }
            ),
            Promotion::Granted(Grant::RealTime { fifo, priority, via_rtkit }) => write!(
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
    let p = if host.driver_owns_priority() {
        Promotion::LeftAsIs("driver-owned thread: real-time already, left alone")
    } else {
        os::promote(WorkerRole::Live, period, &mut os::Revert::default())
    };
    CALLBACK.with(|c| c.set(Some(p)));
    p
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
    }
}

/// Promotes the calling pool thread for `role`; `period` is the device period (macOS'
/// constraint, Linux's `RLIMIT_RTTIME`).
pub fn promote_worker(role: WorkerRole, period: Duration) -> WorkerPromotion {
    let mut revert = os::Revert::default();
    let promotion = os::promote(role, period, &mut revert);
    WorkerPromotion {
        promotion,
        revert,
        _not_send: std::marker::PhantomData,
    }
}

/// macOS: the time-constraint policy **then** `os_workgroup_join` (in that order, or the join
/// fails with `EINVAL`). A refusal from step `"os_workgroup_join"` with `EINVAL` means the
/// workgroup was cancelled (the device changed): fetch the new one and call again.
#[cfg(target_os = "macos")]
pub fn promote_worker_in(wg: &Workgroup, role: WorkerRole, period: Duration) -> WorkerPromotion {
    let mut revert = os::Revert::default();
    let promotion = os::promote_in(wg, role, period, &mut revert);
    WorkerPromotion {
        promotion,
        revert,
        _not_send: std::marker::PhantomData,
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
    OptedOut { timer_resolution: bool },
    NotApplicable,
    Refused(Refusal),
}

impl fmt::Display for PowerThrottling {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PowerThrottling::OptedOut { timer_resolution } => write!(
                f,
                "EcoQoS opted out{}",
                if *timer_resolution { ", timer resolution kept" } else { "" }
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

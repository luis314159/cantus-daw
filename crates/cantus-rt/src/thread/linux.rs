//! Linux: `SCHED_FIFO` directly when `RLIMIT_RTPRIO` allows it (or the process may), else
//! rtkit over D-Bus — which grants `SCHED_RR` at its capped priority, with `RLIMIT_RTTIME` set
//! — through Mozilla's `audio_thread_priority` (ADR 0031 §2.6, R4). Real-time threads are
//! also frequency-boosted by the kernel (`uclamp_min = 1024`).

use super::{Grant, PowerThrottling, Promotion, Refusal, WorkerRole};
use std::time::Duration;

/// What to undo on drop.
#[derive(Default)]
pub enum Revert {
    #[default]
    Nothing,
    Direct {
        policy: i32,
        priority: i32,
    },
    #[cfg(feature = "rtkit")]
    Rtkit(audio_thread_priority::RtPriorityHandle),
}

impl std::fmt::Debug for Revert {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Revert::Nothing => f.write_str("Nothing"),
            Revert::Direct { policy, priority } => write!(f, "Direct({policy}, {priority})"),
            #[cfg(feature = "rtkit")]
            Revert::Rtkit(_) => f.write_str("Rtkit"),
        }
    }
}

fn errno() -> i32 {
    std::io::Error::last_os_error().raw_os_error().unwrap_or(0)
}

/// The calling thread's policy and priority.
fn current() -> Option<(i32, i32)> {
    let mut policy = 0;
    // SAFETY: an all-zero sched_param is a valid value to be overwritten.
    let mut param: libc::sched_param = unsafe { std::mem::zeroed() };
    // SAFETY: pthread_self() is this thread; both out-pointers are live locals.
    let r = unsafe { libc::pthread_getschedparam(libc::pthread_self(), &mut policy, &mut param) };
    (r == 0).then_some((policy, param.sched_priority))
}

fn set(policy: i32, priority: i32) -> i32 {
    // SAFETY: as above.
    let mut param: libc::sched_param = unsafe { std::mem::zeroed() };
    param.sched_priority = priority;
    // SAFETY: pthread_self() is this thread; `param` is live for the call.
    unsafe { libc::pthread_setschedparam(libc::pthread_self(), policy, &param) }
}

/// A limit on CPU time without blocking, per `RLIMIT_RTTIME`; `None` when unlimited.
fn rttime_limit() -> Option<u64> {
    // SAFETY: an all-zero rlimit is a valid value to be overwritten.
    let mut lim: libc::rlimit = unsafe { std::mem::zeroed() };
    // SAFETY: `lim` is a live out-pointer.
    let ok = unsafe { libc::getrlimit(libc::RLIMIT_RTTIME, &mut lim) } == 0;
    (ok && lim.rlim_cur != libc::RLIM_INFINITY).then_some(lim.rlim_cur as u64)
}

/// `sched_rt_runtime_us / sched_rt_period_us` (0.95 by default; −1 means unthrottled).
pub fn realtime_share() -> f32 {
    let read = |f: &str| {
        std::fs::read_to_string(format!("/proc/sys/kernel/{f}"))
            .ok()
            .and_then(|t| t.trim().parse::<i64>().ok())
    };
    match (read("sched_rt_runtime_us"), read("sched_rt_period_us")) {
        (Some(-1), _) => 1.0,
        (Some(r), Some(p)) if r > 0 && p > 0 => (r as f32 / p as f32).min(1.0),
        _ => 0.95,
    }
}

pub fn promote(
    role: WorkerRole,
    period: Duration,
    share: Option<f32>,
    revert: &mut Revert,
) -> Promotion {
    // A spinning thread in a real-time class is killed (SIGKILL after SIGXCPU) once it runs
    // `RLIMIT_RTTIME` without blocking — which rtkit requires and Mozilla's crate sets for the
    // whole process. Such a thread gets SCHED_FIFO only directly and only while unlimited.
    let spins = share.is_none();
    if spins && role != WorkerRole::Background && rttime_limit().is_some() {
        return Promotion::LeftAsIs(
            "a spinning thread stays at normal priority: RLIMIT_RTTIME would kill it",
        );
    }
    let wanted = match role {
        WorkerRole::Live => 50,
        WorkerRole::Ahead => 40,
        WorkerRole::Background => {
            return Promotion::LeftAsIs("background threads run at normal priority")
        }
    };
    let Some((old_policy, old_priority)) = current() else {
        return Promotion::Refused(Refusal {
            step: "pthread_getschedparam",
            code: errno(),
        });
    };
    // SAFETY: an all-zero rlimit is a valid value to be overwritten.
    let mut lim: libc::rlimit = unsafe { std::mem::zeroed() };
    // SAFETY: `lim` is a live out-pointer.
    let allowed = if unsafe { libc::getrlimit(libc::RLIMIT_RTPRIO, &mut lim) } == 0 {
        lim.rlim_cur.min(99) as i32
    } else {
        0
    };
    // SAFETY: geteuid has no preconditions.
    let root = unsafe { libc::geteuid() } == 0;
    let priority = if root { wanted } else { wanted.min(allowed) };
    let direct = if priority > 0 {
        set(libc::SCHED_FIFO | libc::SCHED_RESET_ON_FORK, priority)
    } else {
        libc::EPERM
    };
    if direct == 0 {
        *revert = Revert::Direct {
            policy: old_policy,
            priority: old_priority,
        };
        return Promotion::Granted(Grant::RealTime {
            fifo: true,
            priority,
            via_rtkit: false,
        });
    }
    if spins {
        return Promotion::Refused(Refusal {
            step: "pthread_setschedparam(SCHED_FIFO) (rtkit is not used for spinning threads)",
            code: direct,
        });
    }
    rtkit(period, revert, direct)
}

#[cfg(feature = "rtkit")]
fn rtkit(period: Duration, revert: &mut Revert, _direct: i32) -> Promotion {
    // The crate takes a period as frames at a rate: microseconds at 1 MHz say it exactly.
    let us = period.as_micros().clamp(1, u32::MAX as u128) as u32;
    match audio_thread_priority::promote_current_thread_to_real_time(us, 1_000_000) {
        Ok(handle) => {
            *revert = Revert::Rtkit(handle);
            let (policy, priority) = current().unwrap_or((libc::SCHED_RR, 0));
            let fifo = policy & !libc::SCHED_RESET_ON_FORK == libc::SCHED_FIFO;
            Promotion::Granted(Grant::RealTime {
                fifo,
                priority,
                via_rtkit: true,
            })
        }
        Err(_) => Promotion::Refused(Refusal {
            step: "rtkit (D-Bus org.freedesktop.RealtimeKit1) after SCHED_FIFO was not allowed",
            code: 0,
        }),
    }
}

#[cfg(not(feature = "rtkit"))]
fn rtkit(_period: Duration, _revert: &mut Revert, direct: i32) -> Promotion {
    Promotion::Refused(Refusal {
        step: "pthread_setschedparam(SCHED_FIFO) (built without rtkit)",
        code: direct,
    })
}

pub fn revert(r: &mut Revert) {
    match std::mem::take(r) {
        Revert::Nothing => {}
        Revert::Direct { policy, priority } => {
            set(policy, priority);
        }
        #[cfg(feature = "rtkit")]
        Revert::Rtkit(handle) => {
            let _ = audio_thread_priority::demote_current_thread_from_real_time(handle);
        }
    }
}

pub fn set_cpus(ids: &[u32]) -> Result<(), Refusal> {
    // SAFETY: an all-zero cpu_set_t is the empty set.
    let mut set: libc::cpu_set_t = unsafe { std::mem::zeroed() };
    // An empty selection means "anywhere", as on Windows.
    let all: Vec<u32> = (0..libc::CPU_SETSIZE as u32).collect();
    let ids = if ids.is_empty() { &all[..] } else { ids };
    for &id in ids
        .iter()
        .filter(|&&id| (id as usize) < libc::CPU_SETSIZE as usize)
    {
        // SAFETY: `id` is below CPU_SETSIZE, so the bit is inside the live `set`.
        unsafe { libc::CPU_SET(id as usize, &mut set) };
    }
    // SAFETY: pid 0 is the calling thread; `set` is a live cpu_set_t of the size passed.
    let r = unsafe { libc::sched_setaffinity(0, std::mem::size_of::<libc::cpu_set_t>(), &set) };
    if r == 0 {
        Ok(())
    } else {
        Err(Refusal {
            step: "sched_setaffinity",
            code: errno(),
        })
    }
}

pub fn opt_out_of_power_throttling() -> PowerThrottling {
    PowerThrottling::NotApplicable
}

pub fn priority() -> Option<i32> {
    current().map(|(_, p)| p)
}

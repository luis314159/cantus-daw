//! macOS: `THREAD_TIME_CONSTRAINT_POLICY`, then the device's audio workgroup (WWDC20 10224,
//! *Adding parallel real-time threads to audio workgroups*). The policy must be set **before**
//! `os_workgroup_join`, or the join fails with `EINVAL` (R4).
//!
//! The workgroup functions exist from macOS 11; they are looked up with `dlsym` so the binary
//! still starts on older systems, where a join reports a refusal instead.

use super::{Grant, PowerThrottling, Promotion, Refusal, WorkerRole};
use std::ffi::{c_int, c_void};
use std::sync::OnceLock;
use std::time::Duration;

/// `mach_timebase_info_data_t` (`<mach/mach_time.h>`); libc deprecates its own copy.
#[repr(C)]
struct Timebase {
    numer: u32,
    denom: u32,
}

extern "C" {
    fn mach_timebase_info(info: *mut Timebase) -> c_int;
}

/// `os_workgroup_join_token_s`: a 4-byte signature and 36 opaque bytes on 64-bit systems
/// (`<os/workgroup_base.h>`); 64 bytes here leave room to spare.
#[repr(C, align(8))]
struct Token([u8; 64]);

type JoinFn = unsafe extern "C" fn(*mut c_void, *mut Token) -> c_int;
type LeaveFn = unsafe extern "C" fn(*mut c_void, *mut Token);
type MptFn = unsafe extern "C" fn(*mut c_void, *mut c_void) -> c_int;
type ObjFn = unsafe extern "C" fn(*mut c_void) -> *mut c_void;

struct Syms {
    join: JoinFn,
    leave: LeaveFn,
    max_parallel: MptFn,
    retain: ObjFn,
    release: ObjFn,
}

fn syms() -> Option<&'static Syms> {
    static SYMS: OnceLock<Option<Syms>> = OnceLock::new();
    SYMS.get_or_init(|| {
        let f = |name: &[u8]| {
            // SAFETY: `name` is a NUL-terminated literal; RTLD_DEFAULT searches the loaded images.
            let p = unsafe { libc::dlsym(libc::RTLD_DEFAULT, name.as_ptr().cast()) };
            (!p.is_null()).then_some(p)
        };
        let (j, l, m, r, x) = (
            f(b"os_workgroup_join\0")?,
            f(b"os_workgroup_leave\0")?,
            f(b"os_workgroup_max_parallel_threads\0")?,
            f(b"os_retain\0")?,
            f(b"os_release\0")?,
        );
        // SAFETY: each pointer is the named libSystem function, whose C signature the type
        // spells out (`<os/workgroup_object.h>`, `<os/object.h>`).
        unsafe {
            Some(Syms {
                join: std::mem::transmute::<*mut c_void, JoinFn>(j),
                leave: std::mem::transmute::<*mut c_void, LeaveFn>(l),
                max_parallel: std::mem::transmute::<*mut c_void, MptFn>(m),
                retain: std::mem::transmute::<*mut c_void, ObjFn>(r),
                release: std::mem::transmute::<*mut c_void, ObjFn>(x),
            })
        }
    })
    .as_ref()
}

/// A device's audio workgroup (`kAudioDevicePropertyIOThreadOSWorkgroup`), retained.
#[derive(Debug)]
pub struct Workgroup {
    raw: *mut c_void,
}

// SAFETY: an os_workgroup is a reference-counted OS object made to be joined from many
// threads at once; the handle is only passed to its thread-safe functions.
unsafe impl Send for Workgroup {}
// SAFETY: as above.
unsafe impl Sync for Workgroup {}

impl Workgroup {
    /// Wraps and retains an `os_workgroup_t`. `None` if the workgroup API is missing (macOS
    /// before 11) or `raw` is null.
    ///
    /// # Safety
    /// `raw` must be a valid `os_workgroup_t` (as CoreAudio returns it) for this call.
    pub unsafe fn from_raw(raw: *mut c_void) -> Option<Workgroup> {
        let s = syms()?;
        if raw.is_null() {
            return None;
        }
        // SAFETY: the caller guarantees `raw` is a live workgroup; retaining keeps it alive
        // for as long as this value, which releases it on drop.
        unsafe { (s.retain)(raw) };
        Some(Workgroup { raw })
    }

    /// `os_workgroup_max_parallel_threads`: how many threads Apple's performance controller
    /// plans for; the pool uses that minus one (the callback) as its live workers (R4).
    pub fn max_parallel_threads(&self) -> Option<usize> {
        let s = syms()?;
        // SAFETY: `raw` is retained by `self`; a null attribute asks for the default.
        let n = unsafe { (s.max_parallel)(self.raw, std::ptr::null_mut()) };
        (n > 0).then_some(n as usize)
    }
}

impl Drop for Workgroup {
    fn drop(&mut self) {
        if let Some(s) = syms() {
            // SAFETY: balances the retain in `from_raw`.
            unsafe { (s.release)(self.raw) };
        }
    }
}

/// What to undo: the policy, and a workgroup membership (retained group and its token).
#[derive(Debug, Default)]
pub struct Revert {
    constrained: bool,
    joined: Option<(usize, Box<Token>)>,
}

impl std::fmt::Debug for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Token")
    }
}

fn this_thread() -> libc::thread_t {
    // SAFETY: the Mach port of the calling thread; not owned (no deallocation needed).
    unsafe { libc::pthread_mach_thread_np(libc::pthread_self()) }
}

fn constrain(period: Duration) -> Result<(u64, u64), Refusal> {
    let mut tb = Timebase { numer: 0, denom: 0 };
    // SAFETY: `tb` is a live out-pointer.
    unsafe { mach_timebase_info(&mut tb) };
    let period_ns = period.as_nanos().clamp(100_000, 100_000_000) as u64;
    // Mozilla's choice (rt_mach.rs): computation half the period, capped at 50 ms.
    let computation_ns = (period_ns / 2).min(50_000_000);
    let abs = |ns: u64| (ns * tb.denom.max(1) as u64 / tb.numer.max(1) as u64) as u32;
    let mut policy = libc::thread_time_constraint_policy {
        period: abs(period_ns),
        computation: abs(computation_ns),
        constraint: abs(period_ns),
        preemptible: 1,
    };
    // SAFETY: this thread's port; `policy` is a live struct of exactly the count passed.
    let kr = unsafe {
        libc::thread_policy_set(
            this_thread(),
            libc::THREAD_TIME_CONSTRAINT_POLICY as libc::thread_policy_flavor_t,
            (&mut policy as *mut libc::thread_time_constraint_policy).cast(),
            libc::THREAD_TIME_CONSTRAINT_POLICY_COUNT,
        )
    };
    if kr != 0 {
        return Err(Refusal {
            step: "thread_policy_set(TIME_CONSTRAINT)",
            code: kr,
        });
    }
    Ok((period_ns, computation_ns))
}

pub fn promote(role: WorkerRole, period: Duration, revert: &mut Revert) -> Promotion {
    if role == WorkerRole::Background {
        return Promotion::LeftAsIs("background threads run at normal priority");
    }
    match constrain(period) {
        Ok((period_ns, computation_ns)) => {
            revert.constrained = true;
            Promotion::Granted(Grant::TimeConstraint {
                period_ns,
                computation_ns,
                joined_workgroup: false,
            })
        }
        Err(r) => Promotion::Refused(r),
    }
}

pub fn promote_in(
    wg: &Workgroup,
    role: WorkerRole,
    period: Duration,
    revert: &mut Revert,
) -> Promotion {
    let p = promote(role, period, revert);
    let Promotion::Granted(Grant::TimeConstraint {
        period_ns,
        computation_ns,
        ..
    }) = p
    else {
        return p;
    };
    let Some(s) = syms() else {
        return Promotion::Refused(Refusal {
            step: "os_workgroup_join (needs macOS 11)",
            code: 0,
        });
    };
    let mut token = Box::new(Token([0; 64]));
    // SAFETY: `wg.raw` is retained by `wg`; `token` is a live, writable, large-enough buffer
    // that stays at this address (boxed) until `os_workgroup_leave`.
    let r = unsafe { (s.join)(wg.raw, &mut *token) };
    if r != 0 {
        // EINVAL: the workgroup was cancelled (device change) — fetch the new one and retry.
        return Promotion::Refused(Refusal {
            step: "os_workgroup_join",
            code: r,
        });
    }
    // SAFETY: an extra retain for the membership, released after the leave in `revert`.
    unsafe { (s.retain)(wg.raw) };
    revert.joined = Some((wg.raw as usize, token));
    Promotion::Granted(Grant::TimeConstraint {
        period_ns,
        computation_ns,
        joined_workgroup: true,
    })
}

pub fn revert(r: &mut Revert) {
    if let (Some((raw, mut token)), Some(s)) = (r.joined.take(), syms()) {
        // SAFETY: leaves the group this thread joined with this token, then drops the
        // membership's retain.
        unsafe {
            (s.leave)(raw as *mut c_void, &mut *token);
            (s.release)(raw as *mut c_void);
        }
    }
    if std::mem::take(&mut r.constrained) {
        let mut none: libc::integer_t = 0;
        // SAFETY: the standard policy takes no data (count 0); this thread's port.
        unsafe {
            libc::thread_policy_set(
                this_thread(),
                libc::THREAD_STANDARD_POLICY as libc::thread_policy_flavor_t,
                &mut none,
                libc::THREAD_STANDARD_POLICY_COUNT as libc::mach_msg_type_number_t,
            )
        };
    }
}

pub fn set_cpus(_ids: &[u32]) -> Result<(), Refusal> {
    Err(Refusal {
        step: "macOS has no thread affinity (the workgroup places threads)",
        code: 0,
    })
}

pub fn opt_out_of_power_throttling() -> PowerThrottling {
    PowerThrottling::NotApplicable
}

pub fn priority() -> Option<i32> {
    let mut policy = 0;
    // SAFETY: an all-zero sched_param is valid to overwrite.
    let mut param: libc::sched_param = unsafe { std::mem::zeroed() };
    // SAFETY: this thread; live out-pointers.
    let r = unsafe { libc::pthread_getschedparam(libc::pthread_self(), &mut policy, &mut param) };
    (r == 0).then_some(param.sched_priority)
}

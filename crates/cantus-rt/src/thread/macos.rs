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

fn constrain(period: Duration, share: f32) -> Result<(u64, u64), Refusal> {
    let mut tb = Timebase { numer: 0, denom: 0 };
    // SAFETY: `tb` is a live out-pointer.
    unsafe { mach_timebase_info(&mut tb) };
    let period_ns = period.as_nanos().clamp(100_000, 100_000_000) as u64;
    // The declared busy share of the period (the callback: a half, Mozilla's `rt_mach.rs`
    // choice), capped at 50 ms; the kernel demotes a thread that keeps overrunning it.
    let computation_ns =
        ((period_ns as f64 * share.clamp(0.05, 1.0) as f64) as u64).min(50_000_000);
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

/// The computation is set from the declared busy share, so any share under 1 is honest.
pub fn realtime_share() -> f32 {
    1.0
}

/// A pool worker: the time constraint, and the audio device's workgroup when one is
/// registered ([`use_device`]) and the thread is a live worker — Apple's performance
/// controller then plans the workers with the device's own I/O thread, on performance cores
/// at the clock the deadline needs (WWDC20 10224). A cancelled workgroup (the device
/// changed) is fetched again once, by the device's name.
pub fn promote(
    role: WorkerRole,
    period: Duration,
    share: Option<f32>,
    revert: &mut Revert,
) -> Promotion {
    let Some((name, wg)) = (role == WorkerRole::Live && share.is_some())
        .then(device)
        .flatten()
    else {
        return constrain_only(role, period, share, revert);
    };
    let p = promote_in(&wg, role, period, share, revert);
    match p {
        Promotion::Refused(Refusal {
            step: "os_workgroup_join",
            code: libc::EINVAL,
        }) => {
            revert_constraint(revert);
            match name.as_deref().and_then(lookup) {
                Some(fresh) => {
                    let fresh = std::sync::Arc::new(fresh);
                    remember(name, Some(fresh.clone()));
                    promote_in(&fresh, role, period, share, revert)
                }
                None => constrain_only(role, period, share, revert),
            }
        }
        p => p,
    }
}

fn constrain_only(
    role: WorkerRole,
    period: Duration,
    share: Option<f32>,
    revert: &mut Revert,
) -> Promotion {
    if role == WorkerRole::Background {
        return Promotion::LeftAsIs("background threads run at normal priority");
    }
    let Some(share) = share else {
        return Promotion::LeftAsIs(
            "a spinning thread gets no time constraint: macOS demotes threads that overrun it",
        );
    };
    match constrain(period, share) {
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
    share: Option<f32>,
    revert: &mut Revert,
) -> Promotion {
    let p = constrain_only(role, period, share, revert);
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
    revert_constraint(r);
}

fn revert_constraint(r: &mut Revert) {
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

// ---- the audio device's workgroup ----------------------------------------------------

/// `AudioObjectPropertyAddress` (`<CoreAudio/AudioHardwareBase.h>`).
#[repr(C)]
struct Address {
    selector: u32,
    scope: u32,
    element: u32,
}

#[link(name = "CoreAudio", kind = "framework")]
extern "C" {
    fn AudioObjectGetPropertyDataSize(
        id: u32,
        address: *const Address,
        qualifier_size: u32,
        qualifier: *const c_void,
        size: *mut u32,
    ) -> i32;
    fn AudioObjectGetPropertyData(
        id: u32,
        address: *const Address,
        qualifier_size: u32,
        qualifier: *const c_void,
        size: *mut u32,
        data: *mut c_void,
    ) -> i32;
}

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFStringGetCString(s: *const c_void, buf: *mut libc::c_char, size: isize, enc: u32) -> u8;
    fn CFRelease(cf: *const c_void);
}

const fn four(code: &[u8; 4]) -> u32 {
    u32::from_be_bytes(*code)
}

/// `kAudioObjectSystemObject`.
const SYSTEM_OBJECT: u32 = 1;
/// `kAudioHardwarePropertyDevices`.
const DEVICES: u32 = four(b"dev#");
/// `kAudioHardwarePropertyDefaultOutputDevice`.
const DEFAULT_OUTPUT: u32 = four(b"dOut");
/// `kAudioObjectPropertyName` (= `kAudioDevicePropertyDeviceNameCFString`, cpal's name).
const NAME: u32 = four(b"lnam");
/// `kAudioDevicePropertyIOThreadOSWorkgroup` (macOS 11).
const IO_WORKGROUP: u32 = four(b"oswg");
/// `kAudioObjectPropertyScopeGlobal`.
const GLOBAL: u32 = four(b"glob");
/// `kCFStringEncodingUTF8`.
const UTF8: u32 = 0x0800_0100;

fn address(selector: u32) -> Address {
    Address {
        selector,
        scope: GLOBAL,
        element: 0,
    }
}

/// One fixed-size property of an audio object.
///
/// # Safety
/// `T` must be the property's C type.
unsafe fn property<T: Copy>(id: u32, selector: u32, zero: T) -> Option<T> {
    let mut value = zero;
    let mut size = std::mem::size_of::<T>() as u32;
    // SAFETY: `value` is a live, writable `T` of `size` bytes; the caller names its type.
    let r = unsafe {
        AudioObjectGetPropertyData(
            id,
            &address(selector),
            0,
            std::ptr::null(),
            &mut size,
            (&mut value as *mut T).cast(),
        )
    };
    (r == 0 && size as usize == std::mem::size_of::<T>()).then_some(value)
}

/// Every audio device's id (aggregate devices included: they are devices to CoreAudio).
fn devices() -> Vec<u32> {
    let mut size = 0u32;
    // SAFETY: asks the system object for the size of its device list.
    let r = unsafe {
        AudioObjectGetPropertyDataSize(
            SYSTEM_OBJECT,
            &address(DEVICES),
            0,
            std::ptr::null(),
            &mut size,
        )
    };
    if r != 0 || size == 0 {
        return Vec::new();
    }
    let mut ids = vec![0u32; size as usize / 4];
    // SAFETY: `ids` holds `size` writable bytes.
    let r = unsafe {
        AudioObjectGetPropertyData(
            SYSTEM_OBJECT,
            &address(DEVICES),
            0,
            std::ptr::null(),
            &mut size,
            ids.as_mut_ptr().cast(),
        )
    };
    if r != 0 {
        return Vec::new();
    }
    ids.truncate(size as usize / 4);
    ids
}

/// A device's name, as cpal reports it.
fn device_name(id: u32) -> Option<String> {
    // SAFETY: the name property is a `CFStringRef`, returned retained (released below).
    let s = unsafe { property::<*const c_void>(id, NAME, std::ptr::null()) }?;
    if s.is_null() {
        return None;
    }
    let mut buf = [0 as libc::c_char; 512];
    // SAFETY: `s` is a live CFString; `buf` holds 512 bytes; the string is released once.
    let ok = unsafe {
        let ok = CFStringGetCString(s, buf.as_mut_ptr(), buf.len() as isize, UTF8);
        CFRelease(s);
        ok
    };
    // SAFETY: `CFStringGetCString` wrote a NUL-terminated string when it succeeded.
    (ok != 0).then(|| {
        unsafe { std::ffi::CStr::from_ptr(buf.as_ptr()) }
            .to_string_lossy()
            .into()
    })
}

/// The workgroup of the device called `name` (`""`: the default output device), or `None`
/// where there is no such device or no workgroup API (macOS before 11).
fn lookup(name: &str) -> Option<Workgroup> {
    syms()?;
    let id = if name.is_empty() {
        // SAFETY: the default output device is an `AudioObjectID` (u32).
        unsafe { property::<u32>(SYSTEM_OBJECT, DEFAULT_OUTPUT, 0) }?
    } else {
        devices()
            .into_iter()
            .find(|id| device_name(*id).as_deref() == Some(name))?
    };
    // SAFETY: the property is an `os_workgroup_t` the caller must release (Apple's docs).
    let raw = unsafe { property::<*mut c_void>(id, IO_WORKGROUP, std::ptr::null_mut()) }?;
    // SAFETY: a live workgroup; `from_raw` retains it, and the property's own reference is
    // released here, so the value holds exactly one.
    let wg = unsafe { Workgroup::from_raw(raw) };
    if let Some(s) = syms() {
        // SAFETY: balances the property's +1 (from_raw took its own).
        unsafe { (s.release)(raw) };
    }
    wg
}

type Device = (Option<String>, std::sync::Arc<Workgroup>);

static DEVICE: std::sync::Mutex<Option<Device>> = std::sync::Mutex::new(None);

fn remember(name: Option<String>, wg: Option<std::sync::Arc<Workgroup>>) {
    let mut d = DEVICE.lock().unwrap_or_else(|e| e.into_inner());
    *d = wg.map(|wg| (name, wg));
}

/// The registered device's workgroup (a pool worker's start, never the audio thread's
/// blocks: the lock is taken once per worker start).
fn device() -> Option<Device> {
    DEVICE.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// The device the pool's workers will join: its workgroup looked up now (by the name cpal
/// gives it; `None`: the default output device). `false` when it has none to join (no such
/// device, or macOS before 11): the workers then get the time constraint alone.
pub fn use_device(name: Option<&str>) -> bool {
    let wg = lookup(name.unwrap_or("")).map(std::sync::Arc::new);
    let found = wg.is_some();
    remember(name.map(str::to_owned), wg);
    found
}

/// Forget the device (its stream stopped).
pub fn forget_device() {
    remember(None, None);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn four_char_codes_are_big_endian() {
        assert_eq!(DEVICES, 0x6465_7623);
        assert_eq!(IO_WORKGROUP, 0x6f73_7767);
        assert_eq!(GLOBAL, 0x676c_6f62);
    }

    /// The runner has no sound card, but CoreAudio always lists a device (a null or
    /// virtual one on a headless Mac); where one has a name, the lookup finds it by that
    /// name, and an unknown name finds nothing.
    #[test]
    fn devices_are_found_by_name() {
        assert!(lookup("no such device, surely").is_none());
        assert!(!use_device(Some("no such device, surely")));
        assert!(device().is_none());
        for id in devices() {
            if let Some(name) = device_name(id) {
                // A device's workgroup exists only while its I/O runs on some systems:
                // the lookup may answer None, but must not crash or hang.
                let _ = lookup(&name);
            }
        }
        forget_device();
    }
}

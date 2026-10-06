//! Windows: MMCSS (`avrt.dll`), CPU sets, power throttling.

use super::{Grant, MmcssPriority, PowerThrottling, Promotion, Refusal, WorkerRole};
use std::time::Duration;
use windows_sys::Win32::Foundation::{GetLastError, HANDLE};
use windows_sys::Win32::System::Threading::{
    AvRevertMmThreadCharacteristics, AvSetMmThreadCharacteristicsW, AvSetMmThreadPriority,
    GetCurrentProcess, GetCurrentThread, GetThreadPriority, ProcessPowerThrottling,
    SetProcessInformation, SetThreadSelectedCpuSets, AVRT_PRIORITY, AVRT_PRIORITY_HIGH,
    AVRT_PRIORITY_LOW, AVRT_PRIORITY_NORMAL, PROCESS_POWER_THROTTLING_CURRENT_VERSION,
    PROCESS_POWER_THROTTLING_EXECUTION_SPEED, PROCESS_POWER_THROTTLING_IGNORE_TIMER_RESOLUTION,
    PROCESS_POWER_THROTTLING_STATE,
};

/// The MMCSS registration to revert (the handle as an integer; 0 = none).
#[derive(Debug, Default)]
pub struct Revert {
    handle: usize,
}

/// An ASCII string as a NUL-terminated UTF-16 array, at compile time.
const fn wide<const N: usize>(s: &[u8]) -> [u16; N] {
    let mut out = [0u16; N];
    let mut i = 0;
    while i < s.len() {
        out[i] = s[i] as u16;
        i += 1;
    }
    out
}

static PRO_AUDIO: [u16; 10] = wide(b"Pro Audio");
static AUDIO: [u16; 6] = wide(b"Audio");

fn last_error() -> i32 {
    // SAFETY: reads this thread's last-error value.
    unsafe { GetLastError() as i32 }
}

pub fn promote(role: WorkerRole, _period: Duration, revert: &mut Revert) -> Promotion {
    let (task, name, priority, avrt): (&'static str, &[u16], _, AVRT_PRIORITY) = match role {
        WorkerRole::Live => (
            "Pro Audio",
            &PRO_AUDIO,
            MmcssPriority::High,
            AVRT_PRIORITY_HIGH,
        ),
        WorkerRole::Ahead => (
            "Pro Audio",
            &PRO_AUDIO,
            MmcssPriority::Low,
            AVRT_PRIORITY_LOW,
        ),
        WorkerRole::Background => ("Audio", &AUDIO, MmcssPriority::Normal, AVRT_PRIORITY_NORMAL),
    };
    let mut task_index = 0u32;
    // SAFETY: `name` is a static NUL-terminated UTF-16 string; `task_index` is a live u32 the
    // call writes (0 on entry asks MMCSS for a new task index).
    let h: HANDLE = unsafe { AvSetMmThreadCharacteristicsW(name.as_ptr(), &mut task_index) };
    if h.is_null() {
        return Promotion::Refused(Refusal {
            step: "AvSetMmThreadCharacteristicsW",
            code: last_error(),
        });
    }
    // SAFETY: `h` is the registration just returned for this thread.
    if unsafe { AvSetMmThreadPriority(h, avrt) } == 0 {
        let code = last_error();
        // SAFETY: as above; undoes the registration so the thread is exactly as it was.
        unsafe { AvRevertMmThreadCharacteristics(h) };
        return Promotion::Refused(Refusal {
            step: "AvSetMmThreadPriority",
            code,
        });
    }
    revert.handle = h as usize;
    Promotion::Granted(Grant::Mmcss {
        task,
        priority,
        task_index,
    })
}

pub fn revert(r: &mut Revert) {
    if r.handle != 0 {
        // SAFETY: the handle is the registration `promote` made on this thread (the guard
        // holding it is not `Send`), reverted once.
        unsafe { AvRevertMmThreadCharacteristics(r.handle as HANDLE) };
        r.handle = 0;
    }
}

pub fn set_cpus(ids: &[u32]) -> Result<(), Refusal> {
    // SAFETY: a pseudo-handle for this thread; `ids` is a live slice of `ids.len()` CPU set
    // ids (an empty slice clears the selection).
    let ok =
        unsafe { SetThreadSelectedCpuSets(GetCurrentThread(), ids.as_ptr(), ids.len() as u32) };
    if ok == 0 {
        Err(Refusal {
            step: "SetThreadSelectedCpuSets",
            code: last_error(),
        })
    } else {
        Ok(())
    }
}

pub fn opt_out_of_power_throttling() -> PowerThrottling {
    let set = |control: u32| {
        let state = PROCESS_POWER_THROTTLING_STATE {
            Version: PROCESS_POWER_THROTTLING_CURRENT_VERSION,
            ControlMask: control,
            // Controlled and off: HighQoS, never EcoQoS; and the timer resolution honoured.
            StateMask: 0,
        };
        // SAFETY: a pseudo-handle for this process; `state` is a live struct of the size
        // passed, read during the call only.
        let ok = unsafe {
            SetProcessInformation(
                GetCurrentProcess(),
                ProcessPowerThrottling,
                (&state as *const PROCESS_POWER_THROTTLING_STATE).cast(),
                std::mem::size_of::<PROCESS_POWER_THROTTLING_STATE>() as u32,
            )
        };
        if ok == 0 {
            Err(last_error())
        } else {
            Ok(())
        }
    };
    let speed = PROCESS_POWER_THROTTLING_EXECUTION_SPEED;
    match set(speed | PROCESS_POWER_THROTTLING_IGNORE_TIMER_RESOLUTION) {
        Ok(()) => PowerThrottling::OptedOut {
            timer_resolution: true,
        },
        // Windows 10 before 21H2 knows only EXECUTION_SPEED.
        Err(_) => match set(speed) {
            Ok(()) => PowerThrottling::OptedOut {
                timer_resolution: false,
            },
            Err(code) => PowerThrottling::Refused(Refusal {
                step: "SetProcessInformation",
                code,
            }),
        },
    }
}

pub fn priority() -> Option<i32> {
    // SAFETY: a pseudo-handle for this thread.
    let p = unsafe { GetThreadPriority(GetCurrentThread()) };
    // THREAD_PRIORITY_ERROR_RETURN
    (p != 0x7FFF_FFFF).then_some(p)
}

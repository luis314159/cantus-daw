//! What the helper needs from its operating system: quiet crashes, the protocol's pipes
//! taken off the standard handles, COM for VST3 editors, the parent-death signal, a
//! main-thread wait that also wakes for the plug-ins' windows — and, on Windows, a
//! **pulse**: a hidden message-only window whose timer and wake-ups run inside any message
//! loop the main thread is in, a plug-in's modal one included (a file dialog, a window
//! being dragged), so the helper shows life and serves Cantus's calls from there too
//! (review #0, #48).

use std::fs::File;
use std::time::Duration;

/// The protocol's two ends: what Cantus writes, what it reads.
pub struct Pipes {
    pub input: File,
    pub output: File,
}

#[cfg(windows)]
mod imp {
    use super::blame;
    use super::*;
    use std::os::windows::io::FromRawHandle;
    use std::sync::atomic::{AtomicIsize, Ordering};
    use std::sync::OnceLock;
    use windows_sys::Win32::Foundation::{
        GENERIC_READ, GENERIC_WRITE, HANDLE, INVALID_HANDLE_VALUE,
    };
    use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows_sys::Win32::Storage::FileSystem::{
        CreateFileW, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
    };
    use windows_sys::Win32::System::Console::{
        GetStdHandle, SetStdHandle, STD_ERROR_HANDLE, STD_INPUT_HANDLE, STD_OUTPUT_HANDLE,
    };
    use windows_sys::Win32::System::Diagnostics::Debug::{
        SetErrorMode, SEM_FAILCRITICALERRORS, SEM_NOGPFAULTERRORBOX, SEM_NOOPENFILEERRORBOX,
    };
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::System::Ole::OleInitialize;
    use windows_sys::Win32::System::StationsAndDesktops::SetUserObjectInformationW;
    use windows_sys::Win32::System::Threading::{
        CreateEventW, GetCurrentProcess, SetEvent, INFINITE,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::UOI_TIMERPROC_EXCEPTION_SUPPRESSION;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DispatchMessageW, GetAncestor, MsgWaitForMultipleObjects,
        PeekMessageW, PostMessageW, RegisterClassW, SetTimer, TranslateMessage, GA_ROOTOWNER,
        HWND_MESSAGE, MSG, PM_REMOVE, QS_ALLINPUT, WM_APP, WM_TIMER, WNDCLASSW,
    };

    /// The pulse window (0 until made).
    static PULSE: AtomicIsize = AtomicIsize::new(0);
    /// What a pulse runs (set once, by `start_pulse`).
    static ON_PULSE: OnceLock<fn()> = OnceLock::new();
    /// How often the pulse's timer fires.
    const PULSE_MS: u32 = 100;
    const WM_PULSE: u32 = WM_APP + 1;

    unsafe extern "system" fn pulse_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        if msg == WM_TIMER || msg == WM_PULSE {
            if let Some(f) = ON_PULSE.get() {
                f();
            }
            return 0;
        }
        // SAFETY: the default handling of a message for our own window.
        unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
    }

    /// Makes the pulse window on this (the main) thread: `on_pulse` runs every 100 ms and
    /// whenever a request arrives, from whichever message loop the thread is in.
    pub fn start_pulse(on_pulse: fn()) {
        let _ = ON_PULSE.set(on_pulse);
        let class: Vec<u16> = "CantusSandboxPulse\0".encode_utf16().collect();
        // SAFETY: a window class with our own procedure, registered once; a message-only
        // window of it, made and kept on this thread for the process's life; its timer.
        unsafe {
            let module = GetModuleHandleW(std::ptr::null());
            let wc = WNDCLASSW {
                lpfnWndProc: Some(pulse_proc),
                hInstance: module,
                lpszClassName: class.as_ptr(),
                ..std::mem::zeroed()
            };
            RegisterClassW(&wc);
            let hwnd = CreateWindowExW(
                0,
                class.as_ptr(),
                class.as_ptr(),
                0,
                0,
                0,
                0,
                0,
                HWND_MESSAGE,
                std::ptr::null_mut(),
                module,
                std::ptr::null(),
            );
            if hwnd.is_null() {
                return;
            }
            SetTimer(hwnd, 1, PULSE_MS, None);
            PULSE.store(hwnd as isize, Ordering::Release);
        }
    }

    pub fn quiet_crashes() {
        // SAFETY: sets this process's error mode; no pointers involved.
        unsafe {
            SetErrorMode(SEM_FAILCRITICALERRORS | SEM_NOGPFAULTERRORBOX | SEM_NOOPENFILEERRORBOX);
        }
        // A crash in a plug-in window's timer callback ends the process like any other,
        // instead of being swallowed by the system and leaving the plug-in broken (and
        // its crash unseen; review #1, #39).
        let off: i32 = 0;
        // SAFETY: this process's handle (a pseudo-handle) and a BOOL that outlives the
        // call, with its size.
        unsafe {
            SetUserObjectInformationW(
                GetCurrentProcess(),
                UOI_TIMERPROC_EXCEPTION_SUPPRESSION as i32,
                &off as *const i32 as *const std::ffi::c_void,
                size_of::<i32>() as u32,
            );
        }
    }

    pub fn die_with_parent() {
        // Cantus's job object (kill on close) does it on Windows.
    }

    pub fn take_pipes() -> Pipes {
        let nul: Vec<u16> = "NUL".encode_utf16().chain(std::iter::once(0)).collect();
        // SAFETY: the standard handles are this process's own, inherited from Cantus as
        // its pipes; each is wrapped in a `File` once and replaced in the process's table
        // by a handle to NUL, so nothing else writes into the protocol.
        unsafe {
            let input = GetStdHandle(STD_INPUT_HANDLE);
            let output = GetStdHandle(STD_OUTPUT_HANDLE);
            let null = CreateFileW(
                nul.as_ptr(),
                GENERIC_READ | GENERIC_WRITE,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                std::ptr::null(),
                OPEN_EXISTING,
                0,
                std::ptr::null_mut(),
            );
            if null != INVALID_HANDLE_VALUE {
                SetStdHandle(STD_INPUT_HANDLE, null);
                SetStdHandle(STD_OUTPUT_HANDLE, null);
                SetStdHandle(STD_ERROR_HANDLE, null);
            }
            Pipes {
                input: File::from_raw_handle(input as _),
                output: File::from_raw_handle(output as _),
            }
        }
    }

    pub fn init_com() {
        // SAFETY: once, on the main thread, before any plug-in is loaded: VST3 editors and
        // drag and drop need an apartment (in Cantus, winit makes one).
        unsafe {
            OleInitialize(std::ptr::null_mut());
        }
    }

    /// Wakes the main thread's wait from another thread (a request arrived).
    #[derive(Clone, Copy)]
    pub struct Wake(usize);

    impl Wake {
        pub fn new() -> Wake {
            // SAFETY: an unnamed auto-reset event, never closed (it lives as long as the
            // process).
            Wake(unsafe { CreateEventW(std::ptr::null(), 0, 0, std::ptr::null()) } as usize)
        }

        pub fn ring(self) {
            // SAFETY: the event made in `new`, alive for the process's life; the pulse
            // window, when made, lives as long (posting never blocks).
            unsafe {
                SetEvent(self.0 as HANDLE);
                let pulse = PULSE.load(Ordering::Acquire);
                if pulse != 0 {
                    PostMessageW(pulse as HWND, WM_PULSE, 0, 0);
                }
            }
        }

        /// Waits until a request arrives, a window message is queued, or `timeout`; then
        /// dispatches every queued window message — but the pulse's own, which the main
        /// loop does itself right after (the pulse serves only from loops nested in it).
        pub fn wait(self, timeout: Duration) {
            let ms = timeout.as_millis().min(u128::from(INFINITE - 1)) as u32;
            let h = self.0 as HANDLE;
            let pulse = PULSE.load(Ordering::Acquire);
            // SAFETY: one live event handle; MSG is plain data filled by PeekMessageW.
            unsafe {
                MsgWaitForMultipleObjects(1, &h, 0, ms, QS_ALLINPUT);
                let mut msg: MSG = std::mem::zeroed();
                while PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                    if pulse != 0 && msg.hwnd as isize == pulse {
                        continue;
                    }
                    // A plug-in window's message runs as a call into its instance: a crash
                    // or a modal loop in it is that instance's (review #1, #44).
                    let owner = if msg.hwnd.is_null() {
                        0
                    } else {
                        blame::window_owner(GetAncestor(msg.hwnd, GA_ROOTOWNER) as isize)
                    };
                    let _calling = (owner != 0).then(|| blame::calling(owner));
                    TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }
        }
    }
}

#[cfg(unix)]
mod imp {
    use super::*;
    use std::os::unix::io::FromRawFd;
    use std::sync::{Condvar, Mutex};

    pub fn quiet_crashes() {}

    pub fn die_with_parent() {
        #[cfg(target_os = "linux")]
        // SAFETY: asks the kernel to send SIGKILL to this process when its parent dies.
        unsafe {
            libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL);
        }
    }

    pub fn take_pipes() -> Pipes {
        // SAFETY: duplicates this process's own standard descriptors (the pipes from
        // Cantus) for the protocol, then points 0, 1 and 2 at /dev/null so nothing else
        // writes into the protocol.
        unsafe {
            let input = libc::dup(0);
            let output = libc::dup(1);
            let null = libc::open(c"/dev/null".as_ptr(), libc::O_RDWR);
            if null >= 0 {
                libc::dup2(null, 0);
                libc::dup2(null, 1);
                libc::dup2(null, 2);
                libc::close(null);
            }
            Pipes {
                input: File::from_raw_fd(input),
                output: File::from_raw_fd(output),
            }
        }
    }

    pub fn init_com() {}

    /// No plug-in windows here, so no nested message loops: the main loop is the pulse.
    pub fn start_pulse(_on_pulse: fn()) {}

    /// Wakes the main thread's wait from another thread (a request arrived).
    #[derive(Clone, Copy)]
    pub struct Wake(&'static (Mutex<bool>, Condvar));

    impl Wake {
        pub fn new() -> Wake {
            Wake(Box::leak(Box::new((Mutex::new(false), Condvar::new()))))
        }

        pub fn ring(self) {
            let (m, c) = self.0;
            *m.lock().unwrap_or_else(|e| e.into_inner()) = true;
            c.notify_one();
        }

        pub fn wait(self, timeout: Duration) {
            let (m, c) = self.0;
            let mut rung = m.lock().unwrap_or_else(|e| e.into_inner());
            if !*rung {
                rung = c
                    .wait_timeout(rung, timeout)
                    .map(|(g, _)| g)
                    .unwrap_or_else(|e| e.into_inner().0);
            }
            *rung = false;
        }
    }
}

pub use imp::*;

/// Which plug-in a crash is laid on (review #49, #1, #39): each thread says which instance
/// it is calling into — the main thread in its calls, its ticks and the plug-in windows'
/// messages it dispatches, each processing thread in its jobs — and a crash handler (the
/// unhandled-exception filter and the C runtime's abort signal on Windows; the fatal signals
/// on Unix, on a stack of their own) writes the faulting thread's into the lobby
/// ([`cantus_host::bridge::sandbox::CRASH_AT`]) as the process dies — or *outside*
/// ([`cantus_host::bridge::sandbox::OUTSIDE`]) for a thread calling into none, so the
/// crash is laid on nobody rather than on whoever was processing. A Windows fail-fast abort
/// runs no handler; Cantus then judges by what was running, and blames nobody when several
/// were.
pub mod blame {
    use cantus_host::bridge::sandbox::OUTSIDE;
    use std::cell::{Cell, RefCell};
    use std::sync::atomic::{AtomicPtr, AtomicU32, Ordering};

    thread_local! {
        static CURRENT: Cell<u32> = const { Cell::new(0) };
        /// The plug-in windows (their root handles) and their instances (main thread).
        static WINDOWS: RefCell<Vec<(isize, u32)>> = const { RefCell::new(Vec::new()) };
    }

    static CRASH_AT: AtomicPtr<AtomicU32> = AtomicPtr::new(std::ptr::null_mut());
    static MAIN_AT: AtomicPtr<AtomicU32> = AtomicPtr::new(std::ptr::null_mut());

    /// The instance this thread calls into now (0: none).
    pub fn current() -> u32 {
        CURRENT.try_with(|c| c.get()).unwrap_or(0)
    }

    /// This thread calls into `instance` now (0: none) — a processing thread's job.
    pub fn set_current(instance: u32) {
        let _ = CURRENT.try_with(|c| c.set(instance));
    }

    /// The main thread calls into `instance` while the value lives: the lobby's word and
    /// the thread's own say so; the previous ones come back after (calls nest — a window's
    /// message inside a call, a call served from a plug-in's modal loop).
    pub struct Calling {
        prev: u32,
    }

    pub fn calling(instance: u32) -> Calling {
        let prev = current();
        mark(instance);
        Calling { prev }
    }

    impl Drop for Calling {
        fn drop(&mut self) {
            mark(self.prev);
        }
    }

    fn mark(instance: u32) {
        set_current(instance);
        let word = MAIN_AT.load(Ordering::Acquire);
        if !word.is_null() {
            // SAFETY: the lobby's word, mapped for the life of the process (`install`).
            unsafe { &*word }.store(instance, Ordering::Release);
        }
    }

    /// A plug-in window (its root handle) belongs to `instance`.
    pub fn own_window(hwnd: isize, instance: u32) {
        WINDOWS.with(|w| {
            let mut w = w.borrow_mut();
            w.retain(|(h, _)| *h != hwnd);
            w.push((hwnd, instance));
        });
    }

    pub fn forget_window(hwnd: isize) {
        WINDOWS.with(|w| w.borrow_mut().retain(|(h, _)| *h != hwnd));
    }

    /// The instance whose window this root handle is (0: none of ours).
    #[cfg_attr(not(windows), allow(dead_code))]
    pub fn window_owner(root: isize) -> u32 {
        WINDOWS
            .try_with(|w| {
                w.borrow()
                    .iter()
                    .find(|(h, _)| *h == root)
                    .map_or(0, |(_, n)| *n)
            })
            .unwrap_or(0)
    }

    /// From a crash handler, on the faulting thread: its instance (or *outside*) into the
    /// lobby. Only atomics and a thread-local made without initialisation are touched.
    fn record() {
        let word = CRASH_AT.load(Ordering::Acquire);
        if word.is_null() {
            return;
        }
        let n = CURRENT.try_with(|c| c.get()).unwrap_or(0);
        let n = if n == 0 { OUTSIDE } else { n };
        // SAFETY: `word` points into the lobby, mapped for the life of the process.
        let w = unsafe { &*word };
        // The first fault's word stands (a second one in the dying process says nothing).
        let _ = w.compare_exchange(0, n, Ordering::AcqRel, Ordering::Relaxed);
    }

    /// Installs the crash handlers, writing to `crash_at`; `main_at` is the lobby's word
    /// naming the instance the main thread is calling into.
    pub fn install(crash_at: &'static AtomicU32, main_at: &'static AtomicU32) {
        CRASH_AT.store(
            crash_at as *const AtomicU32 as *mut AtomicU32,
            Ordering::Release,
        );
        MAIN_AT.store(
            main_at as *const AtomicU32 as *mut AtomicU32,
            Ordering::Release,
        );
        imp::install();
        alt_stack();
    }

    /// Puts our handler back in front (a plug-in may have installed its own crash reporter
    /// over it while it loaded): ours records, then hands on to it.
    pub fn reinstall() {
        imp::install();
    }

    /// A stack of its own for this thread's fatal-signal handler, so a stack overflow is
    /// recorded too (Unix; once per thread; nothing on Windows).
    pub fn alt_stack() {
        imp::alt_stack();
    }

    #[cfg(windows)]
    mod imp {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use windows_sys::Win32::System::Diagnostics::Debug::{
            SetUnhandledExceptionFilter, EXCEPTION_POINTERS, LPTOP_LEVEL_EXCEPTION_FILTER,
        };

        /// A filter that was in place before ours (a plug-in's crash reporter): handed on.
        static CHAINED: AtomicUsize = AtomicUsize::new(0);

        unsafe extern "system" fn filter(info: *const EXCEPTION_POINTERS) -> i32 {
            super::record();
            let chained = CHAINED.load(Ordering::Acquire);
            if chained != 0 {
                // SAFETY: a filter function the process installed, called as the system
                // would have.
                let f: unsafe extern "system" fn(*const EXCEPTION_POINTERS) -> i32 =
                    unsafe { std::mem::transmute(chained) };
                return unsafe { f(info) };
            }
            0 // EXCEPTION_CONTINUE_SEARCH: the process ends as it would have
        }

        unsafe extern "C" {
            /// The C runtime's `signal` (UCRT): its abort raises SIGABRT before it ends the
            /// process, which the unhandled-exception filter never sees.
            fn signal(sig: i32, handler: usize) -> usize;
        }
        const SIGABRT: i32 = 22;

        extern "C" fn on_abort(_sig: i32) {
            super::record();
        }

        pub fn install() {
            // SAFETY: a filter for this process's unhandled exceptions, which only stores
            // an atomic and hands on; the previous one, kept to hand on to.
            let prev: LPTOP_LEVEL_EXCEPTION_FILTER =
                unsafe { SetUnhandledExceptionFilter(Some(filter)) };
            if let Some(p) = prev {
                if p as usize != filter as *const () as usize {
                    CHAINED.store(p as usize, Ordering::Release);
                }
            }
            // SAFETY: a handler for the C runtime's abort signal, which only stores an
            // atomic; returning lets the abort end the process as it would have.
            unsafe {
                signal(SIGABRT, on_abort as extern "C" fn(i32) as usize);
            }
        }

        pub fn alt_stack() {}
    }

    #[cfg(unix)]
    mod imp {
        extern "C" fn on_signal(_sig: libc::c_int) {
            super::record();
            // The handler was reset (SA_RESETHAND): returning re-raises the fault (or, for
            // SIGABRT, `abort` raises it again), and the process ends as it would have.
        }

        pub fn install() {
            for sig in [
                libc::SIGSEGV,
                libc::SIGBUS,
                libc::SIGILL,
                libc::SIGFPE,
                libc::SIGABRT,
            ] {
                // SAFETY: a zeroed sigaction with our handler, reset to the default after
                // it runs once, on the thread's own signal stack when it has one; the
                // handler only stores an atomic.
                unsafe {
                    let mut sa: libc::sigaction = std::mem::zeroed();
                    sa.sa_sigaction = on_signal as extern "C" fn(libc::c_int) as usize;
                    sa.sa_flags = libc::SA_RESETHAND | libc::SA_NODEFER | libc::SA_ONSTACK;
                    libc::sigemptyset(&mut sa.sa_mask);
                    libc::sigaction(sig, &sa, std::ptr::null_mut());
                }
            }
        }

        pub fn alt_stack() {
            const SIZE: usize = 64 * 1024;
            let stack: &'static mut [u8] = Box::leak(vec![0u8; SIZE].into_boxed_slice());
            let ss = libc::stack_t {
                ss_sp: stack.as_mut_ptr().cast(),
                ss_flags: 0,
                ss_size: SIZE,
            };
            // SAFETY: a stack this thread keeps for the life of the process (leaked).
            unsafe {
                libc::sigaltstack(&ss, std::ptr::null_mut());
            }
        }
    }
}

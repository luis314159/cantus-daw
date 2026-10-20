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
        CreateEventW, GetCurrentProcess, GetCurrentThreadId, SetEvent, INFINITE,
    };
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::IsWindowEnabled;
    use windows_sys::Win32::UI::WindowsAndMessaging::UOI_TIMERPROC_EXCEPTION_SUPPRESSION;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        BringWindowToTop, CreateWindowExW, DefWindowProcW, DispatchMessageW, EnumThreadWindows,
        GetWindow, IsWindowVisible, GW_OWNER, MsgWaitForMultipleObjects, PeekMessageW, PostMessageW, RegisterClassW,
        SetForegroundWindow, SetTimer, TranslateMessage, HWND_MESSAGE, MSG, PM_REMOVE, QS_ALLINPUT,
        WM_APP, WM_TIMER, WNDCLASSW,
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
                    // A plug-in window's message (or its timer's) runs as a call into its
                    // instance: a crash or a modal loop in it is that instance's (review
                    // #1, #44; isolation-4 #55).
                    let owner = blame::message_owner(&msg);
                    let _calling = (owner != 0).then(|| blame::calling(owner));
                    TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }
        }
    }

    /// The calling thread's id (the main thread's, for [`surface`] from another thread).
    pub fn current_thread() -> u32 {
        // SAFETY: a plain query.
        unsafe { GetCurrentThreadId() }
    }

    /// Brings the main thread's (`thread`'s) waiting window to the front — a plug-in's
    /// dialog: the topmost visible, enabled window whose owner it disabled (a modal one),
    /// else the topmost visible, enabled one (review isolation-4 #54; isolation-5 #5, #17,
    /// #26, #35, #47, #57, #68: never the bottom-most). Cantus let this process take the
    /// foreground first. Safe from any thread.
    pub fn surface(thread: u32) {
        struct Found {
            modal: HWND,
            any: HWND,
        }
        unsafe extern "system" fn each(hwnd: HWND, found: LPARAM) -> i32 {
            // SAFETY: queries of a window of that thread; `found` is our own slot; the
            // windows come topmost first, so the first of each kind is kept.
            unsafe {
                let f = &mut *(found as *mut Found);
                if IsWindowVisible(hwnd) == 0 || IsWindowEnabled(hwnd) == 0 {
                    return 1;
                }
                if f.any.is_null() {
                    f.any = hwnd;
                }
                let owner = GetWindow(hwnd, GW_OWNER);
                if f.modal.is_null() && !owner.is_null() && IsWindowEnabled(owner) == 0 {
                    f.modal = hwnd;
                }
            }
            1
        }
        let mut found = Found {
            modal: std::ptr::null_mut(),
            any: std::ptr::null_mut(),
        };
        // SAFETY: walks that thread's top-level windows with our callback.
        unsafe {
            EnumThreadWindows(thread, Some(each), &mut found as *mut Found as LPARAM);
            let w = if found.modal.is_null() {
                found.any
            } else {
                found.modal
            };
            if !w.is_null() {
                BringWindowToTop(w);
                SetForegroundWindow(w);
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

    /// No plug-in windows here.
    pub fn surface(_thread: u32) {}

    pub fn current_thread() -> u32 {
        0
    }

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

/// Which plug-in a crash is laid on (review #49, #1, #39; isolation-4 #55; isolation-5 #7,
/// #32, #55): each thread says which instance it is calling into — the main thread in its
/// calls, its ticks and the window messages its own loop dispatches (the instance whose
/// window it is for, or whose library alone the window's class or the timer's procedure
/// comes from), each processing thread in its jobs — and a crash handler writes, as the
/// process dies, the faulting thread's instance; for a thread calling into none, the one
/// instance whose library the faulting code is in; else *outside*
/// ([`cantus_host::bridge::sandbox::OUTSIDE`]) ([`cantus_host::bridge::sandbox::CRASH_AT`]).
/// A library several instances share names none of them. Inside a plug-in's own modal loop
/// the main thread dispatches messages of any window (a neighbour's timer, say): a crash
/// there is laid on the one instance whose library the faulting code is in, else on
/// nobody — never on the plug-in whose loop it is for being there (isolation-5). A Windows
/// fail-fast abort runs no handler; Cantus then judges by what was running, and blames
/// nobody when several were.
pub mod blame {
    use cantus_host::bridge::sandbox::OUTSIDE;
    use std::cell::{Cell, RefCell};
    use std::sync::atomic::{AtomicBool, AtomicPtr, AtomicU32, AtomicUsize, Ordering};

    thread_local! {
        /// The instance this thread calls into now (0: none).
        static CURRENT: Cell<u32> = const { Cell::new(0) };
        /// The plug-in windows (their root handles) and their instances (main thread).
        static WINDOWS: RefCell<Vec<(isize, u32)>> = const { RefCell::new(Vec::new()) };
    }

    thread_local! {
        /// This is the helper's main thread (set at `install`).
        static MAIN: Cell<bool> = const { Cell::new(false) };
    }

    /// The main thread is in a loop nested in a plug-in's code (its dialog).
    static NESTED: AtomicBool = AtomicBool::new(false);

    /// The main thread entered (`true`) or left a plug-in's own loop; what it was before.
    pub fn nest(on: bool) -> bool {
        NESTED.swap(on, Ordering::AcqRel)
    }

    static CRASH_AT: AtomicPtr<AtomicU32> = AtomicPtr::new(std::ptr::null_mut());
    static MAIN_AT: AtomicPtr<AtomicU32> = AtomicPtr::new(std::ptr::null_mut());

    /// The plug-ins' libraries: the address range of each, and an instance loaded from it
    /// (0: a free entry). Written by the main thread at loads and unloads, read by a crash
    /// handler with atomics only.
    struct Module {
        start: AtomicUsize,
        end: AtomicUsize,
        instance: AtomicU32,
    }

    const MODULES: usize = 256;
    #[allow(clippy::declare_interior_mutable_const)]
    const FREE: Module = Module {
        start: AtomicUsize::new(0),
        end: AtomicUsize::new(0),
        instance: AtomicU32::new(0),
    };
    static TABLE: [Module; MODULES] = [FREE; MODULES];

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

    /// The one instance loaded from the library `addr` is in — 0 for none of the plug-ins'
    /// and for a library several instances share (nothing says whose; isolation-5 #7, #32,
    /// #55). Atomics only.
    pub(crate) fn by_module(addr: usize) -> u32 {
        if addr == 0 {
            return 0;
        }
        let mut found = 0;
        for m in &TABLE {
            let n = m.instance.load(Ordering::Acquire);
            if n == 0 {
                continue;
            }
            if m.start.load(Ordering::Acquire) <= addr && addr < m.end.load(Ordering::Acquire) {
                if found != 0 && found != n {
                    return 0;
                }
                found = n;
            }
        }
        found
    }

    /// Records `[start, end)` as a library of `instance`'s (main thread).
    fn remember_module(start: usize, end: usize, instance: u32) {
        let known = TABLE.iter().any(|m| {
            m.instance.load(Ordering::Acquire) == instance
                && m.start.load(Ordering::Acquire) == start
        });
        if known || start >= end {
            return;
        }
        if let Some(m) = TABLE
            .iter()
            .find(|m| m.instance.load(Ordering::Acquire) == 0)
        {
            m.start.store(start, Ordering::Release);
            m.end.store(end, Ordering::Release);
            m.instance.store(instance, Ordering::Release);
        }
    }

    /// The libraries loaded now, by start address (main thread).
    pub fn modules_now() -> Vec<usize> {
        let mut v = Vec::new();
        imp::modules(&mut |start, _, _: &str| v.push(start));
        v
    }

    /// After `instance` was loaded from `path`: the libraries that are its — the plug-in's
    /// own, and those that came with it from its folder (not loaded `before`) — are
    /// remembered for the crash handler and for attributing its windows' and timers'
    /// messages (main thread; review isolation-4 #48, #55).
    pub fn learn_modules(instance: u32, path: &std::path::Path, before: &[usize]) {
        let own = path.to_string_lossy().to_lowercase();
        let folder = path
            .parent()
            .map(|p| p.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        imp::modules(&mut |start, end, file: &str| {
            let file = file.to_lowercase();
            let with_it = !before.contains(&start)
                && !folder.is_empty()
                && std::path::Path::new(&file)
                    .parent()
                    .is_some_and(|p| p.to_string_lossy() == folder);
            if file.starts_with(&own) || with_it {
                remember_module(start, end, instance);
            }
        });
    }

    /// `instance` left: its libraries are no longer laid anything on.
    pub fn forget_instance(instance: u32) {
        for m in &TABLE {
            if m.instance.load(Ordering::Acquire) == instance {
                m.instance.store(0, Ordering::Release);
            }
        }
    }

    /// From a crash handler, on the faulting thread: the instance it calls into — but for
    /// the main thread inside a plug-in's own loop — else the one instance whose library
    /// `addr` (the faulting code) is in, else *outside*, into the lobby; the first fault's
    /// word stands. The instance written when this call wrote it. Only atomics and
    /// thread-locals made without initialisation are touched.
    fn record(addr: usize) -> Option<u32> {
        let word = CRASH_AT.load(Ordering::Acquire);
        if word.is_null() {
            return None;
        }
        let main = MAIN.try_with(|m| m.get()).unwrap_or(false);
        let nested = main && NESTED.load(Ordering::Acquire);
        let n = match CURRENT.try_with(|c| c.get()).unwrap_or(0) {
            n if n != 0 && !nested => n,
            _ => match by_module(addr) {
                0 => OUTSIDE,
                n => n,
            },
        };
        // SAFETY: `word` points into the lobby, mapped for the life of the process.
        let w = unsafe { &*word };
        w.compare_exchange(0, n, Ordering::AcqRel, Ordering::Relaxed)
            .ok()
            .map(|_| n)
    }

    /// The fault was handled after all (a plug-in's own filter resumed it): its word is
    /// taken back, so a later end is not laid on it (review isolation-4 #47, #61).
    #[cfg_attr(not(windows), allow(dead_code))]
    fn unrecord(n: u32) {
        let word = CRASH_AT.load(Ordering::Acquire);
        if !word.is_null() {
            // SAFETY: as in `record`.
            let _ = unsafe { &*word }.compare_exchange(n, 0, Ordering::AcqRel, Ordering::Relaxed);
        }
    }

    /// Installs the crash handlers, writing to `crash_at`; `main_at` is the lobby's word
    /// naming the instance the main thread is calling into.
    pub fn install(crash_at: &'static AtomicU32, main_at: &'static AtomicU32) {
        let _ = MAIN.try_with(|m| m.set(true));
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

    /// Puts our handler back in front where a plug-in put its own over it while it loaded,
    /// opened its editor or restarted (Windows: the unhandled-exception filter, ours
    /// recording, then handing on to it; the C runtime's abort signal is ours only from the
    /// start, so a plug-in's reporter keeps its own — isolation-5 #73). On Unix a plug-in's
    /// own fatal-signal handler is left in place — its runtime may handle its own faults —
    /// and ours goes back only where the default was restored (review isolation-4 #17,
    /// #52).
    pub fn reinstall() {
        imp::reinstall();
    }

    /// A stack of its own for this thread's fatal-signal handler, so a stack overflow is
    /// recorded too (Unix; once per thread; nothing on Windows).
    pub fn alt_stack() {
        imp::alt_stack();
    }

    #[cfg(windows)]
    mod imp {
        use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
        use windows_sys::Win32::Foundation::{HMODULE, HWND};
        use windows_sys::Win32::System::Diagnostics::Debug::{
            SetUnhandledExceptionFilter, EXCEPTION_POINTERS, LPTOP_LEVEL_EXCEPTION_FILTER,
        };
        use windows_sys::Win32::System::LibraryLoader::{
            GetModuleFileNameW, GetModuleHandleExW, GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS,
            GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
        };
        use windows_sys::Win32::System::ProcessStatus::{
            K32EnumProcessModules, K32GetModuleInformation, MODULEINFO,
        };
        use windows_sys::Win32::System::Threading::GetCurrentProcess;
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            GetAncestor, GetClassLongPtrW, GA_ROOTOWNER, GCLP_HMODULE, MSG, WM_TIMER,
        };

        /// A filter that was in place before ours (a plug-in's crash reporter): handed on.
        static CHAINED: AtomicUsize = AtomicUsize::new(0);
        /// Ours runs now: entered again (a plug-in's filter handing back to ours), it
        /// hands on to nothing (review isolation-4 #7, #16, #28).
        static INSIDE: AtomicBool = AtomicBool::new(false);
        const EXCEPTION_CONTINUE_EXECUTION: i32 = -1;

        /// Whether `addr` is in a library still loaded (a filter of one that was unloaded
        /// is never called).
        pub(super) fn loaded(addr: usize) -> bool {
            let mut h: HMODULE = std::ptr::null_mut();
            // SAFETY: a lookup of the module an address is in, not changing its count.
            unsafe {
                GetModuleHandleExW(
                    GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS
                        | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
                    addr as *const u16,
                    &mut h,
                ) != 0
            }
        }

        unsafe extern "system" fn filter(info: *const EXCEPTION_POINTERS) -> i32 {
            if INSIDE.swap(true, Ordering::AcqRel) {
                return 0; // EXCEPTION_CONTINUE_SEARCH: the crash goes on as it would have
            }
            // SAFETY: the system hands a valid record of the exception being dispatched.
            let addr = unsafe {
                info.as_ref()
                    .and_then(|i| i.ExceptionRecord.as_ref())
                    .map_or(0, |r| r.ExceptionAddress as usize)
            };
            let wrote = super::record(addr);
            let chained = CHAINED.load(Ordering::Acquire);
            let mut verdict = 0;
            if chained != 0 && loaded(chained) {
                // SAFETY: a filter function the process installed, in a library still
                // loaded, called as the system would have.
                let f: unsafe extern "system" fn(*const EXCEPTION_POINTERS) -> i32 =
                    unsafe { std::mem::transmute(chained) };
                verdict = unsafe { f(info) };
            }
            if verdict == EXCEPTION_CONTINUE_EXECUTION {
                if let Some(n) = wrote {
                    super::unrecord(n);
                }
            }
            INSIDE.store(false, Ordering::Release);
            verdict
        }

        unsafe extern "C" {
            /// The C runtime's `signal` (UCRT): its abort raises SIGABRT before it ends the
            /// process, which the unhandled-exception filter never sees.
            fn signal(sig: i32, handler: usize) -> usize;
        }
        const SIGABRT: i32 = 22;

        extern "C" fn on_abort(_sig: i32) {
            super::record(0);
        }

        pub fn install() {
            filter_on_top();
            // SAFETY: a handler for the C runtime's abort signal, which only stores an
            // atomic; returning lets the abort end the process as it would have.
            unsafe {
                signal(SIGABRT, on_abort as extern "C" fn(i32) as usize);
            }
        }

        fn filter_on_top() {
            // SAFETY: a filter for this process's unhandled exceptions, which stores an
            // atomic and hands on; the previous one, kept to hand on to — or forgotten when
            // there is none (review isolation-4 #61).
            let prev: LPTOP_LEVEL_EXCEPTION_FILTER =
                unsafe { SetUnhandledExceptionFilter(Some(filter)) };
            match prev {
                Some(p) if p as usize != filter as *const () as usize => {
                    CHAINED.store(p as usize, Ordering::Release);
                }
                // Ours on top already: what it hands on to stands (and is never called
                // once its library is gone).
                Some(_) => {}
                None => CHAINED.store(0, Ordering::Release),
            }
        }

        pub fn reinstall() {
            filter_on_top();
        }

        pub fn alt_stack() {}

        /// The library a window's class came from (its module handle is its base).
        fn class_module(hwnd: HWND) -> usize {
            // SAFETY: a query of a window's class; 0 for a window that is gone.
            unsafe { GetClassLongPtrW(hwnd, GCLP_HMODULE) }
        }

        /// The instance a retrieved message belongs to: its window's (by its root owner,
        /// or the library its class came from), or — a timer with no window — its
        /// procedure's library's.
        pub(crate) fn owner(msg: &MSG) -> u32 {
            if !msg.hwnd.is_null() {
                // SAFETY: a query of a window handle (any, even one gone).
                let root = unsafe { GetAncestor(msg.hwnd, GA_ROOTOWNER) };
                let n = super::window_owner(root as isize);
                if n != 0 {
                    return n;
                }
                let n = super::by_module(class_module(msg.hwnd));
                if n != 0 {
                    return n;
                }
            }
            if msg.message == WM_TIMER && msg.lParam != 0 {
                return super::by_module(msg.lParam as usize);
            }
            0
        }

        /// Each library loaded now: its address range and file.
        pub fn modules(f: &mut dyn FnMut(usize, usize, &str)) {
            let mut handles: Vec<HMODULE> = vec![std::ptr::null_mut(); 1024];
            let mut needed = 0u32;
            // SAFETY: this process's pseudo-handle; a buffer with its size in bytes.
            let ok = unsafe {
                K32EnumProcessModules(
                    GetCurrentProcess(),
                    handles.as_mut_ptr(),
                    (handles.len() * size_of::<HMODULE>()) as u32,
                    &mut needed,
                )
            };
            if ok == 0 {
                return;
            }
            let n = (needed as usize / size_of::<HMODULE>()).min(handles.len());
            let mut name = vec![0u16; 1024];
            for h in &handles[..n] {
                let mut info: MODULEINFO = MODULEINFO {
                    lpBaseOfDll: std::ptr::null_mut(),
                    SizeOfImage: 0,
                    EntryPoint: std::ptr::null_mut(),
                };
                // SAFETY: a module of this process and a structure of the asked size.
                let ok = unsafe {
                    K32GetModuleInformation(
                        GetCurrentProcess(),
                        *h,
                        &mut info,
                        size_of::<MODULEINFO>() as u32,
                    )
                };
                if ok == 0 {
                    continue;
                }
                // SAFETY: a buffer with its length in characters.
                let len = unsafe { GetModuleFileNameW(*h, name.as_mut_ptr(), name.len() as u32) };
                let file = String::from_utf16_lossy(&name[..len as usize]);
                let start = info.lpBaseOfDll as usize;
                f(start, start + info.SizeOfImage as usize, &file);
            }
        }
    }

    #[cfg(windows)]
    pub(crate) use imp::owner as message_owner;

    #[cfg(all(test, windows))]
    mod tests {
        /// A filter is called only while its library is loaded: one in this program is, an
        /// address in no library is not (review isolation-4 #7, #16, #19, #28, #61).
        #[test]
        fn a_filter_in_no_library_is_never_called() {
            let here = a_filter_in_no_library_is_never_called as *const () as usize;
            assert!(super::imp::loaded(here));
            assert!(!super::imp::loaded(0x10));
        }
    }

    #[cfg(unix)]
    mod imp {
        const SIGNALS: [libc::c_int; 5] = [
            libc::SIGSEGV,
            libc::SIGBUS,
            libc::SIGILL,
            libc::SIGFPE,
            libc::SIGABRT,
        ];

        /// The faulting code's address, from the signal's context (0 where unknown).
        fn pc(ctx: *mut libc::c_void) -> usize {
            if ctx.is_null() {
                return 0;
            }
            #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
            {
                // SAFETY: the kernel hands a `ucontext_t` with a SA_SIGINFO handler.
                let uc = unsafe { &*(ctx as *const libc::ucontext_t) };
                uc.uc_mcontext.gregs[libc::REG_RIP as usize] as usize
            }
            #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
            {
                // SAFETY: as above.
                let uc = unsafe { &*(ctx as *const libc::ucontext_t) };
                uc.uc_mcontext.pc as usize
            }
            #[cfg(not(all(
                target_os = "linux",
                any(target_arch = "x86_64", target_arch = "aarch64")
            )))]
            {
                0
            }
        }

        extern "C" fn on_signal(
            _sig: libc::c_int,
            _info: *mut libc::siginfo_t,
            ctx: *mut libc::c_void,
        ) {
            super::record(pc(ctx));
            // The handler was reset (SA_RESETHAND): returning re-raises the fault (or, for
            // SIGABRT, `abort` raises it again), and the process ends as it would have.
        }

        fn ours() -> usize {
            on_signal as extern "C" fn(libc::c_int, *mut libc::siginfo_t, *mut libc::c_void)
                as usize
        }

        fn set(sig: libc::c_int) {
            // SAFETY: a zeroed sigaction with our handler, reset to the default after it
            // runs once, on the thread's own signal stack when it has one; the handler only
            // stores an atomic.
            unsafe {
                let mut sa: libc::sigaction = std::mem::zeroed();
                sa.sa_sigaction = ours();
                sa.sa_flags =
                    libc::SA_SIGINFO | libc::SA_RESETHAND | libc::SA_NODEFER | libc::SA_ONSTACK;
                libc::sigemptyset(&mut sa.sa_mask);
                libc::sigaction(sig, &sa, std::ptr::null_mut());
            }
        }

        pub fn install() {
            for sig in SIGNALS {
                set(sig);
            }
        }

        /// Ours back only where the default was restored: a plug-in's own handler stays.
        pub fn reinstall() {
            for sig in SIGNALS {
                // SAFETY: reads the signal's current disposition into a zeroed structure.
                let current = unsafe {
                    let mut old: libc::sigaction = std::mem::zeroed();
                    libc::sigaction(sig, std::ptr::null(), &mut old);
                    old.sa_sigaction
                };
                if current == libc::SIG_DFL {
                    set(sig);
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

        /// Each shared object loaded now: its address range and file (Linux; none
        /// elsewhere — macOS has no `dl_iterate_phdr`, isolation-5 #9).
        #[cfg(not(target_os = "linux"))]
        pub fn modules(_f: &mut dyn FnMut(usize, usize, &str)) {}

        /// Each shared object loaded now: its address range and file.
        #[cfg(target_os = "linux")]
        pub fn modules(f: &mut dyn FnMut(usize, usize, &str)) {
            unsafe extern "C" fn each(
                info: *mut libc::dl_phdr_info,
                _size: libc::size_t,
                data: *mut libc::c_void,
            ) -> libc::c_int {
                // SAFETY: the loader hands a valid record; `data` is our closure.
                unsafe {
                    let info = &*info;
                    let f = &mut *(data as *mut &mut dyn FnMut(usize, usize, &str));
                    let name = if info.dlpi_name.is_null() {
                        String::new()
                    } else {
                        std::ffi::CStr::from_ptr(info.dlpi_name)
                            .to_string_lossy()
                            .into_owned()
                    };
                    let base = info.dlpi_addr as usize;
                    let (mut lo, mut hi) = (usize::MAX, 0usize);
                    for i in 0..info.dlpi_phnum as usize {
                        let ph = &*info.dlpi_phdr.add(i);
                        if ph.p_type == libc::PT_LOAD {
                            let start = base + ph.p_vaddr as usize;
                            lo = lo.min(start);
                            hi = hi.max(start + ph.p_memsz as usize);
                        }
                    }
                    if lo < hi {
                        f(lo, hi, &name);
                    }
                }
                0
            }
            let mut f = f;
            // SAFETY: walks the loaded objects with our callback, on this thread.
            unsafe {
                libc::dl_iterate_phdr(
                    Some(each),
                    &mut f as *mut &mut dyn FnMut(usize, usize, &str) as *mut libc::c_void,
                );
            }
        }
    }
}

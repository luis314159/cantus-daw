//! The hidden **Fault** parameter (round 29, ADR 0063): failures on purpose, so the host's
//! isolation is tested against a plug-in that really crashes or hangs where third-party
//! ones do. A value set **while processing** (an event in `process`) arms a fault; the same
//! value arriving any other way (a flush, a state, a reload) arms nothing, so a plug-in the
//! host reloads after the crash comes back healthy. The twin of this file lives in every
//! test plug-in (the synth, the two VST3 ones).
//!
//! | Value | What happens |
//! |---|---|
//! | 1 | aborts in this `process` |
//! | 2 | never returns from this `process` (a hang) |
//! | 3 | aborts in the next `save_state` |
//! | 4 | aborts in the next `load_state` |
//! | 5 | aborts in the next `activate` (a restart) |
//! | 6 | aborts when its editor is next created |
//! | 7 | never returns from the next `save_state` |
//! | 8 | never returns from the next `load_state` |
//! | 9 | never returns when its editor is next created |
//! | 10 | the next `save_state` runs a nested message loop for 3 s, as a modal dialog does |
//! | 11 | this `process` takes 50 ms, once (late, not hung) |
//! | 12 | its editor's window runs a nested message loop for 3 s, as a file dialog does |
//! | 13 | the next `save_state` fails (once) |
//! | 14 | an access violation in this `process` (a crash a handler sees, unlike an abort) |
//! | 15 | its editor's window runs a nested message loop for 10 s |
//! | 16 | an access violation in its editor's window (its timer), on the main thread |
//! | 17 | the next `save_state` takes 3 s, without pumping messages (slow, not hung) |
//! | 18 | this `process` takes 1 s, once (late for long, not hung) |
//! | 19 | an access violation when the instance is destroyed (its destructor, an unload) |
//! | 20 | an access violation in a thread timer of the main thread (no window: as a framework's) |
//! | 21 | a thread timer of the main thread that never returns |
//! | 22 | it asks the host to rescan its parameters, and the second list read takes 2 s |
//! | 23 | this `process` raises an exception its own crash filter handles (it goes on) |
//! | 24 | a hidden window of its own (a message-only one, as JUCE's) never returns from a message |
//! | 25 | its state's tag becomes `heard`, and it tells the host nothing (a state moved quietly) |
//!
//! And, read from the environment (the helper inherits Cantus's, so a test sets them before
//! the helper that must see them starts): `CANTUS_TEST_GAIN_DESTROY_MARKER=<folder>` writes
//! a file there when an instance is destroyed, 300 ms on; `CANTUS_TEST_GAIN_PUMP_LOAD_MS=<ms>` makes
//! every `load_state` run a nested message loop that long;
//! `CANTUS_TEST_GAIN_PUMP_ACTIVATE_ONCE=<file>` makes the first activation run one for
//! 1.5 s; `CANTUS_TEST_GAIN_SLOW_ACTIVATE_ONCE=<file>` makes it take 1.5 s without one;
//! `CANTUS_TEST_GAIN_SLOW_RESTART_MS=<ms>` makes every activation after the first (a restart)
//! take that long; `CANTUS_TEST_GAIN_CHAIN_FILTER`
//! installs, at load, a crash filter that hands on to the one before it (Windows), as a
//! plug-in's crash reporter does.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

pub const ABORT_PROCESS: u32 = 1;
pub const HANG_PROCESS: u32 = 2;
pub const ABORT_SAVE: u32 = 3;
pub const ABORT_LOAD: u32 = 4;
pub const ABORT_ACTIVATE: u32 = 5;
pub const ABORT_EDITOR: u32 = 6;
pub const HANG_SAVE: u32 = 7;
pub const HANG_LOAD: u32 = 8;
pub const HANG_EDITOR: u32 = 9;
pub const PUMP_SAVE: u32 = 10;
pub const SLOW_ONCE: u32 = 11;
pub const PUMP_WINDOW: u32 = 12;
pub const FAIL_SAVE_ONCE: u32 = 13;
pub const SEGV_PROCESS: u32 = 14;
pub const PUMP_WINDOW_LONG: u32 = 15;
pub const SEGV_EDITOR: u32 = 16;
pub const SLOW_SAVE: u32 = 17;
pub const SLOW_SECOND: u32 = 18;
pub const SEGV_DESTROY: u32 = 19;
pub const SEGV_THREAD_TIMER: u32 = 20;
pub const HANG_THREAD_TIMER: u32 = 21;
pub const RESCAN_SLOW: u32 = 22;
pub const RAISE_HANDLED: u32 = 23;
pub const HANG_HIDDEN_WINDOW: u32 = 24;
pub const TAG_QUIETLY: u32 = 25;
/// The parameter's range: 0 (none) … this.
pub const LAST: f64 = 25.0;

/// The exception `RAISE_HANDLED` raises, which the plug-in's own crash filter
/// (`CANTUS_TEST_GAIN_CHAIN_FILTER`) resumes, as a licence check's trick does.
#[cfg_attr(not(windows), allow(dead_code))]
pub const HANDLED_CODE: u32 = 0xE000_0CA7;

/// A rescan to ask for on the main thread (`RESCAN_SLOW`).
static RESCAN: AtomicBool = AtomicBool::new(false);
/// Reads of the parameter list until the slow one (`RESCAN_SLOW`): the second after the
/// rescan — the host's own first read refreshes what it mirrors; the next is the one asked
/// for (an isolated plug-in's: Cantus's).
static SLOW_PARAMS: AtomicU32 = AtomicU32::new(0);

/// What a thread timer of the main thread does when it fires (`SEGV_THREAD_TIMER`,
/// `HANG_THREAD_TIMER`); 0 for none.
static THREAD_TIMER: AtomicU32 = AtomicU32::new(0);

/// The fault armed in this instance, 0 for none; a nested loop its editor's window runs at
/// its next timer, in seconds (`PUMP_WINDOW`, `PUMP_WINDOW_LONG`; 0 for none); an access
/// violation its window makes at its next timer (`SEGV_EDITOR`). Each instance its own.
#[derive(Debug, Default)]
pub struct Fault(AtomicU32, AtomicU32, AtomicBool);

impl Fault {
    /// A value set while processing: arm it, and fire the ones that belong to `process`.
    pub fn arm(&self, value: f64) {
        let v = value.round().clamp(0.0, LAST) as u32;
        self.0.store(v, Ordering::Release);
        match v {
            ABORT_PROCESS => std::process::abort(),
            HANG_PROCESS => hang(),
            SLOW_ONCE => {
                self.0.store(0, Ordering::Release);
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            PUMP_WINDOW | PUMP_WINDOW_LONG => {
                self.0.store(0, Ordering::Release);
                let secs = if v == PUMP_WINDOW { 3 } else { 10 };
                self.1.store(secs, Ordering::Release);
            }
            SEGV_EDITOR => {
                self.0.store(0, Ordering::Release);
                self.2.store(true, Ordering::Release);
            }
            SEGV_PROCESS => {
                // SAFETY: none — a write to an address no process maps, on purpose.
                unsafe { std::ptr::write_volatile(std::ptr::without_provenance_mut::<u8>(16), 1) }
            }
            SLOW_SECOND => {
                self.0.store(0, Ordering::Release);
                std::thread::sleep(std::time::Duration::from_secs(1));
            }
            SEGV_THREAD_TIMER | HANG_THREAD_TIMER | HANG_HIDDEN_WINDOW => {
                self.0.store(0, Ordering::Release);
                THREAD_TIMER.store(v, Ordering::Release);
            }
            RESCAN_SLOW => {
                self.0.store(0, Ordering::Release);
                RESCAN.store(true, Ordering::Release);
            }
            RAISE_HANDLED => {
                self.0.store(0, Ordering::Release);
                raise_handled();
            }
            _ => {}
        }
    }

    /// Whether the main thread should be asked for a callback (a thread timer to start).
    pub fn wants_main_thread(value: f64) -> bool {
        matches!(
            value.round() as u32,
            SEGV_THREAD_TIMER | HANG_THREAD_TIMER | RESCAN_SLOW | HANG_HIDDEN_WINDOW
        )
    }

    /// The instance is being destroyed: `SEGV_DESTROY` crashes here; the destroy marker
    /// (environment) is written.
    pub fn on_destroy(&self) {
        if let Some(dir) = std::env::var_os("CANTUS_TEST_GAIN_DESTROY_MARKER") {
            // A destructor that takes its time (preferences written, a licence released).
            std::thread::sleep(std::time::Duration::from_millis(300));
            let at = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos());
            let _ = std::fs::write(
                std::path::Path::new(&dir).join(format!("{}-{at}.gone", std::process::id())),
                b"gone",
            );
        }
        if self.0.load(Ordering::Acquire) == SEGV_DESTROY {
            // SAFETY: none — a write to an address no process maps, on purpose.
            unsafe { std::ptr::write_volatile(std::ptr::without_provenance_mut::<u8>(16), 1) }
        }
    }

    /// Whether the editor's window should run its nested loop now (once), and how long.
    #[cfg_attr(not(windows), allow(dead_code))]
    pub fn take_window_pump(&self) -> u32 {
        self.1.swap(0, Ordering::AcqRel)
    }

    /// Whether the editor's window should crash now (once).
    #[cfg_attr(not(windows), allow(dead_code))]
    pub fn take_window_segv(&self) -> bool {
        self.2.swap(false, Ordering::AcqRel)
    }

    /// Whether the save asked for now fails (`FAIL_SAVE_ONCE`, then disarmed).
    pub fn refuse_save(&self) -> bool {
        self.0
            .compare_exchange(FAIL_SAVE_ONCE, 0, Ordering::AcqRel, Ordering::Relaxed)
            .is_ok()
    }

    /// At a site — `ABORT_SAVE`, `ABORT_LOAD`, `ABORT_EDITOR` or `ABORT_ACTIVATE` — fires
    /// what is armed for it: an abort, a hang, or (saving) a nested message loop.
    pub fn fire(&self, site: u32) {
        let armed = self.0.load(Ordering::Acquire);
        let hang_at = match site {
            ABORT_SAVE => HANG_SAVE,
            ABORT_LOAD => HANG_LOAD,
            ABORT_EDITOR => HANG_EDITOR,
            _ => u32::MAX,
        };
        if armed == site {
            std::process::abort();
        }
        if armed == hang_at {
            hang();
        }
        if site == ABORT_SAVE && armed == SLOW_SAVE {
            self.0.store(0, Ordering::Release);
            std::thread::sleep(std::time::Duration::from_secs(3));
        }
        if site == ABORT_SAVE && armed == PUMP_SAVE {
            self.0.store(0, Ordering::Release);
            pump(std::time::Duration::from_secs(3));
        }
    }
}

/// On the main thread: whether the host should rescan the parameters now (and the next
/// list is slow).
pub fn take_rescan() -> bool {
    let r = RESCAN.swap(false, Ordering::AcqRel);
    if r {
        SLOW_PARAMS.store(2, Ordering::Release);
    }
    r
}

/// The parameter list is being read: slow once after a rescan asked for.
pub fn params_read() {
    let left = SLOW_PARAMS.load(Ordering::Acquire);
    if left > 0 && SLOW_PARAMS.fetch_sub(1, Ordering::AcqRel) == 1 {
        std::thread::sleep(std::time::Duration::from_secs(2));
    }
}

/// On the main thread (the host's callback): starts the thread timer armed, if any.
pub fn on_main_thread() {
    let what = THREAD_TIMER.swap(0, Ordering::AcqRel);
    if what != 0 {
        start_thread_timer(what);
    }
}

#[cfg(windows)]
static TIMER_DOES: AtomicU32 = AtomicU32::new(0);

#[cfg(windows)]
unsafe extern "system" fn thread_timer(
    _hwnd: windows_sys::Win32::Foundation::HWND,
    _msg: u32,
    id: usize,
    _time: u32,
) {
    // SAFETY: our own timer, killed once.
    unsafe { windows_sys::Win32::UI::WindowsAndMessaging::KillTimer(std::ptr::null_mut(), id) };
    match TIMER_DOES.swap(0, Ordering::AcqRel) {
        SEGV_THREAD_TIMER => {
            // SAFETY: none — a write to an address no process maps, on purpose.
            unsafe { std::ptr::write_volatile(std::ptr::without_provenance_mut::<u8>(16), 1) }
        }
        HANG_THREAD_TIMER => hang(),
        PUMP_ON_TIMER => pump(std::time::Duration::from_millis(3_000)),
        _ => {}
    }
}

/// What a thread timer started at activation does: a 3 s nested loop, as a nag a
/// framework's timer opens — outside any call, from code several instances share
/// (`CANTUS_TEST_GAIN_PUMP_ON_TIMER_ONCE`).
#[cfg(windows)]
const PUMP_ON_TIMER: u32 = 1_000;

/// `CANTUS_TEST_GAIN_PUMP_ON_TIMER_ONCE=<file>`: the first activation (while the file is
/// not there) starts a thread timer that, half a second on, runs a 3 s nested loop.
pub fn pump_on_timer_once() {
    if let Some(file) = std::env::var_os("CANTUS_TEST_GAIN_PUMP_ON_TIMER_ONCE") {
        let file = std::path::PathBuf::from(file);
        if !file.exists() {
            let _ = std::fs::write(&file, b"timer");
            start_pump_timer();
        }
    }
}

#[cfg(windows)]
fn start_pump_timer() {
    TIMER_DOES.store(PUMP_ON_TIMER, Ordering::Release);
    // SAFETY: a thread timer (no window) on this, the main, thread, with our procedure.
    unsafe {
        windows_sys::Win32::UI::WindowsAndMessaging::SetTimer(
            std::ptr::null_mut(),
            0,
            500,
            Some(thread_timer),
        );
    }
}

#[cfg(not(windows))]
fn start_pump_timer() {}

/// A hidden window of the plug-in's own — its class registered from its library, as a
/// framework's message window is — sent a message that never returns (`HANG_HIDDEN_WINDOW`).
#[cfg(windows)]
fn hang_in_hidden_window() {
    use windows_sys::Win32::System::LibraryLoader::{
        GetModuleHandleExW, GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS,
        GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, PostMessageW, RegisterClassW, HWND_MESSAGE, WM_APP,
        WNDCLASSW,
    };
    unsafe extern "system" fn proc_(
        hwnd: windows_sys::Win32::Foundation::HWND,
        msg: u32,
        w: windows_sys::Win32::Foundation::WPARAM,
        l: windows_sys::Win32::Foundation::LPARAM,
    ) -> windows_sys::Win32::Foundation::LRESULT {
        if msg == WM_APP + 7 {
            hang();
        }
        // SAFETY: the default handling of a message for our own window.
        unsafe { DefWindowProcW(hwnd, msg, w, l) }
    }
    let class: Vec<u16> = "CantusTestHidden\0".encode_utf16().collect();
    // SAFETY: this library's own handle, from an address in it; a class of ours registered
    // once (a second time fails harmlessly); a message-only window of it; a posted message.
    unsafe {
        let mut module = std::ptr::null_mut();
        GetModuleHandleExW(
            GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
            proc_ as *const () as *const u16,
            &mut module,
        );
        let wc = WNDCLASSW {
            lpfnWndProc: Some(proc_),
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
        PostMessageW(hwnd, WM_APP + 7, 0, 0);
    }
}

#[cfg(windows)]
fn start_thread_timer(what: u32) {
    if what == HANG_HIDDEN_WINDOW {
        hang_in_hidden_window();
        return;
    }
    TIMER_DOES.store(what, Ordering::Release);
    // SAFETY: a thread timer (no window) on this, the main, thread, with our procedure.
    unsafe {
        windows_sys::Win32::UI::WindowsAndMessaging::SetTimer(
            std::ptr::null_mut(),
            0,
            10,
            Some(thread_timer),
        );
    }
}

#[cfg(not(windows))]
fn start_thread_timer(_what: u32) {}

/// `CANTUS_TEST_GAIN_PUMP_ACTIVATE_ONCE=<file>`: the first activation (while the file is
/// not there) runs a nested message loop for 1.5 s, as a licence dialog does, and makes
/// the file.
pub fn pump_activate_once() {
    if let Some(file) = std::env::var_os("CANTUS_TEST_GAIN_PUMP_ACTIVATE_ONCE") {
        let file = std::path::PathBuf::from(file);
        if !file.exists() {
            let _ = std::fs::write(&file, b"pumped");
            pump(std::time::Duration::from_millis(1_500));
        }
    }
}

/// `CANTUS_TEST_GAIN_SLOW_ACTIVATE_ONCE=<file>`: the first activation (while the file is
/// not there) takes 1.5 s — or `CANTUS_TEST_GAIN_SLOW_MS` — without pumping: a slow load,
/// not a dialog; it makes the file.
pub fn slow_activate_once() {
    if let Some(file) = std::env::var_os("CANTUS_TEST_GAIN_SLOW_ACTIVATE_ONCE") {
        let file = std::path::PathBuf::from(file);
        if !file.exists() {
            let _ = std::fs::write(&file, b"slow");
            let ms = std::env::var("CANTUS_TEST_GAIN_SLOW_MS")
                .ok()
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(1_500);
            std::thread::sleep(std::time::Duration::from_millis(ms));
        }
    }
}

/// `CANTUS_TEST_GAIN_SLOW_RESTART_MS=<ms>`: every activation after an instance's first (a
/// restart: a new latency, say) takes that long, without pumping — a plug-in slow to
/// restart, or a helper whose main thread a busy machine runs late (round 31: the macOS
/// runner's crash test compared the song while the restart was still under way).
pub fn slow_restart() {
    if let Some(ms) = std::env::var("CANTUS_TEST_GAIN_SLOW_RESTART_MS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
    {
        std::thread::sleep(std::time::Duration::from_millis(ms));
    }
}

/// `CANTUS_TEST_GAIN_PUMP_LOAD_MS`: how long a `load_state` runs a nested message loop.
pub fn pump_load() {
    if let Some(ms) = std::env::var("CANTUS_TEST_GAIN_PUMP_LOAD_MS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
    {
        pump(std::time::Duration::from_millis(ms));
    }
}

/// `CANTUS_TEST_GAIN_CHAIN_FILTER`: a crash filter installed at load that hands every
/// crash on to the one before it — ours, for a plug-in loaded in Cantus's helper.
#[cfg(windows)]
pub fn chain_filter() {
    use std::sync::atomic::AtomicUsize;
    use windows_sys::Win32::System::Diagnostics::Debug::{
        SetUnhandledExceptionFilter, EXCEPTION_POINTERS,
    };
    static PREV: AtomicUsize = AtomicUsize::new(0);
    static ONCE: std::sync::Once = std::sync::Once::new();
    unsafe extern "system" fn mine(info: *const EXCEPTION_POINTERS) -> i32 {
        // Its own exception: handled, and the code goes on.
        // SAFETY: the record the system hands.
        let code = unsafe {
            info.as_ref()
                .and_then(|i| i.ExceptionRecord.as_ref())
                .map_or(0, |r| r.ExceptionCode as u32)
        };
        if code == HANDLED_CODE {
            return -1; // EXCEPTION_CONTINUE_EXECUTION
        }
        let prev = PREV.load(Ordering::Acquire);
        if prev != 0 {
            // SAFETY: the filter that was in place before ours, called as the system would.
            let f: unsafe extern "system" fn(*const EXCEPTION_POINTERS) -> i32 =
                unsafe { std::mem::transmute(prev) };
            return unsafe { f(info) };
        }
        0
    }
    if std::env::var_os("CANTUS_TEST_GAIN_CHAIN_FILTER").is_none() {
        return;
    }
    ONCE.call_once(|| {
        // SAFETY: a process-wide filter of our own; the previous one kept to hand on to.
        let prev = unsafe { SetUnhandledExceptionFilter(Some(mine)) };
        PREV.store(prev.map_or(0, |p| p as usize), Ordering::Release);
    });
}

#[cfg(not(windows))]
pub fn chain_filter() {}

/// Raises the exception the plug-in's own filter resumes (`RAISE_HANDLED`).
#[cfg(windows)]
fn raise_handled() {
    // SAFETY: a continuable exception of ours, with no arguments.
    unsafe {
        windows_sys::Win32::System::Diagnostics::Debug::RaiseException(
            HANDLED_CODE,
            0,
            0,
            std::ptr::null(),
        );
    }
}

#[cfg(not(windows))]
fn raise_handled() {}

fn hang() -> ! {
    loop {
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}

/// What a modal dialog does to its thread: a message loop of its own, for `long`.
#[cfg(windows)]
pub fn pump(long: std::time::Duration) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        DispatchMessageW, PeekMessageW, TranslateMessage, MSG, PM_REMOVE,
    };
    let start = std::time::Instant::now();
    while start.elapsed() < long {
        // SAFETY: a zeroed MSG filled by PeekMessageW, dispatched on this thread.
        unsafe {
            let mut msg: MSG = std::mem::zeroed();
            while PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

#[cfg(not(windows))]
pub fn pump(long: std::time::Duration) {
    std::thread::sleep(long);
}

/// The process this instance lives in: what the read-only *Process* parameter reports, so
/// a test sees an isolated instance really ran in another process.
pub fn process_id() -> f64 {
    f64::from(std::process::id())
}

//! What the helper needs from its operating system: quiet crashes, the protocol's pipes
//! taken off the standard handles, COM for VST3 editors, the parent-death signal, and a
//! main-thread wait that also wakes for the plug-ins' windows.

use std::fs::File;
use std::time::Duration;

/// The protocol's two ends: what Cantus writes, what it reads.
pub struct Pipes {
    pub input: File,
    pub output: File,
}

#[cfg(windows)]
mod imp {
    use super::*;
    use std::os::windows::io::FromRawHandle;
    use windows_sys::Win32::Foundation::{GENERIC_READ, GENERIC_WRITE, HANDLE, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::Storage::FileSystem::{
        CreateFileW, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
    };
    use windows_sys::Win32::System::Console::{
        GetStdHandle, SetStdHandle, STD_ERROR_HANDLE, STD_INPUT_HANDLE, STD_OUTPUT_HANDLE,
    };
    use windows_sys::Win32::System::Diagnostics::Debug::{
        SetErrorMode, SEM_FAILCRITICALERRORS, SEM_NOGPFAULTERRORBOX, SEM_NOOPENFILEERRORBOX,
    };
    use windows_sys::Win32::System::Ole::OleInitialize;
    use windows_sys::Win32::System::Threading::{CreateEventW, SetEvent, INFINITE};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        DispatchMessageW, MsgWaitForMultipleObjects, PeekMessageW, TranslateMessage, MSG,
        PM_REMOVE, QS_ALLINPUT,
    };

    pub fn quiet_crashes() {
        // SAFETY: sets this process's error mode; no pointers involved.
        unsafe {
            SetErrorMode(SEM_FAILCRITICALERRORS | SEM_NOGPFAULTERRORBOX | SEM_NOOPENFILEERRORBOX);
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
            // SAFETY: the event made in `new`, alive for the process's life.
            unsafe { SetEvent(self.0 as HANDLE) };
        }

        /// Waits until a request arrives, a window message is queued, or `timeout`; then
        /// dispatches every queued window message.
        pub fn wait(self, timeout: Duration) {
            let ms = timeout.as_millis().min(u128::from(INFINITE - 1)) as u32;
            let h = self.0 as HANDLE;
            // SAFETY: one live event handle; MSG is plain data filled by PeekMessageW.
            unsafe {
                MsgWaitForMultipleObjects(1, &h, 0, ms, QS_ALLINPUT);
                let mut msg: MSG = std::mem::zeroed();
                while PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
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

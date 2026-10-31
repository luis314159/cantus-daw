//! What the helper and its client need from the operating system, as the plug-in sandbox
//! does it (ADR 0063 §3): in the helper, crashes without the system's dialog, the
//! protocol's pipes taken off the standard handles (a runtime that prints cannot break a
//! line), and an end when Cantus ends; in Cantus, no console window for the helper and,
//! on Windows, a job object that ends it however Cantus ends.

#![allow(unsafe_code)]

use std::fs::File;

/// The protocol's two ends, in the helper: what Cantus writes, what it reads.
pub struct Pipes {
    pub input: File,
    pub output: File,
}

#[cfg(windows)]
mod imp {
    use super::*;
    use std::os::windows::io::{AsRawHandle, FromRawHandle};
    use std::process::Child;
    use windows_sys::Win32::Foundation::{
        CloseHandle, GENERIC_READ, GENERIC_WRITE, HANDLE, INVALID_HANDLE_VALUE,
    };
    use windows_sys::Win32::Storage::FileSystem::{
        CreateFileW, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
    };
    use windows_sys::Win32::System::Console::{
        GetStdHandle, SetStdHandle, STD_ERROR_HANDLE, STD_INPUT_HANDLE, STD_OUTPUT_HANDLE,
    };
    use windows_sys::Win32::System::Diagnostics::Debug::{
        SetErrorMode, SEM_FAILCRITICALERRORS, SEM_NOGPFAULTERRORBOX, SEM_NOOPENFILEERRORBOX,
    };
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
        SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
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

    /// A job object with *kill on job close*: when Cantus ends, however it ends, the system
    /// closes the handle and ends the helpers in it.
    pub struct Job(HANDLE);

    // SAFETY: a job handle is a kernel object usable from any thread.
    unsafe impl Send for Job {}
    // SAFETY: as above; assigning a process is thread-safe in the kernel.
    unsafe impl Sync for Job {}

    impl Job {
        pub fn new() -> Option<Job> {
            // SAFETY: an anonymous job object with default security.
            let h = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
            if h.is_null() {
                return None;
            }
            // SAFETY: a zeroed limit structure is valid; one flag is set in it and its size
            // handed over, as the call asks.
            let ok = unsafe {
                let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
                info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
                SetInformationJobObject(
                    h,
                    JobObjectExtendedLimitInformation,
                    &info as *const _ as *const std::ffi::c_void,
                    size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                )
            };
            if ok == 0 {
                // SAFETY: our own handle, closed once.
                unsafe { CloseHandle(h) };
                return None;
            }
            Some(Job(h))
        }

        pub fn assign(&self, child: &Child) -> bool {
            // SAFETY: the child's process handle is live while `child` is; the job is ours.
            unsafe { AssignProcessToJobObject(self.0, child.as_raw_handle() as HANDLE) != 0 }
        }
    }

    impl Drop for Job {
        fn drop(&mut self) {
            // SAFETY: our own handle, closed once; closing it ends the helpers.
            unsafe { CloseHandle(self.0) };
        }
    }

    pub fn no_window(cmd: &mut std::process::Command) {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
}

#[cfg(unix)]
mod imp {
    use super::*;
    use std::os::unix::io::FromRawFd;

    pub fn quiet_crashes() {}

    pub fn die_with_parent() {
        #[cfg(target_os = "linux")]
        // SAFETY: asks the kernel to send SIGKILL to this process when its parent dies.
        unsafe {
            libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL);
        }
        #[cfg(target_vendor = "apple")]
        watch_parent();
    }

    /// macOS has no parent-death signal: a thread waits in a kqueue for the parent's
    /// `NOTE_EXIT` and ends this process at once (as the plug-in sandbox, ADR 0066). A
    /// parent already gone ends it too.
    #[cfg(target_vendor = "apple")]
    fn watch_parent() {
        // SAFETY: plain system calls on this process's own ids and a kqueue it owns; the
        // event list is a live, zeroed array of one.
        unsafe {
            let parent = libc::getppid();
            let kq = libc::kqueue();
            if kq < 0 {
                return;
            }
            let mut change: libc::kevent = std::mem::zeroed();
            change.ident = parent as usize;
            change.filter = libc::EVFILT_PROC;
            change.flags = libc::EV_ADD | libc::EV_ONESHOT;
            change.fflags = libc::NOTE_EXIT;
            let r = libc::kevent(kq, &change, 1, std::ptr::null_mut(), 0, std::ptr::null());
            if r < 0 || parent == 1 || libc::getppid() != parent {
                libc::_exit(1);
            }
            let _ = std::thread::Builder::new()
                .name("parent-watch".into())
                .spawn(move || loop {
                    let mut ev: libc::kevent = std::mem::zeroed();
                    let n = libc::kevent(kq, std::ptr::null(), 0, &mut ev, 1, std::ptr::null());
                    if n > 0 || (n < 0 && *libc::__error() != libc::EINTR) {
                        libc::_exit(1);
                    }
                });
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

    /// Nothing: the kernel's parent-death signal (Linux) and the kqueue watch (macOS) end
    /// the helper with Cantus.
    pub struct Job;

    impl Job {
        pub fn new() -> Option<Job> {
            Some(Job)
        }

        pub fn assign(&self, _child: &std::process::Child) -> bool {
            true
        }
    }

    pub fn no_window(_cmd: &mut std::process::Command) {}
}

pub use imp::*;

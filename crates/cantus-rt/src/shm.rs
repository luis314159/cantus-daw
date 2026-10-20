//! Named shared memory between two processes (ADR 0063, ADR 0031 §6).
//!
//! One process [`creates`](SharedMemory::create) a region under a name, zero-filled, and
//! hands the name to another, which [`opens`](SharedMemory::open) the same pages. The
//! region is fixed-size: made once, before any real-time thread touches it, and never
//! grown. What lives in it, and who may touch which part when, is the caller's protocol
//! (see [`crate::ipc`]); this module only maps the pages.
//!
//! - **Windows**: a pagefile-backed `CreateFileMappingW` named `Local\<name>` (this session
//!   only), mapped with `MapViewOfFile`. The pages live while either side holds a handle.
//! - **Unix**: `shm_open("/<name>")`, `ftruncate`, `mmap(MAP_SHARED)`. The creator unlinks
//!   the name when it drops its side, so a crash of the *other* process leaks nothing, and
//!   the opener may map it only while the creator is alive.
//!
//! Nothing here knows about audio (ADR 0031 §2.7).

use std::io;
use std::ptr::NonNull;

/// A mapped region of named shared memory. `Send + Sync`: the pages are plain memory; what
/// each thread may do with them is the protocol's business, through raw pointers.
#[derive(Debug)]
pub struct SharedMemory {
    ptr: NonNull<u8>,
    len: usize,
    os: imp::Handle,
}

// SAFETY: the region is a plain block of bytes owned by this value until it drops; the
// handle is an OS object usable from any thread. Access to the bytes goes through raw
// pointers whose synchronisation is the caller's protocol.
unsafe impl Send for SharedMemory {}
// SAFETY: as above; `&SharedMemory` only hands out the raw pointer and the length.
unsafe impl Sync for SharedMemory {}

impl SharedMemory {
    /// Makes a new region of `len` bytes (rounded up to a page by the OS), zero-filled,
    /// under `name` (letters, digits, `-`, `_`, `.`; at most 200 bytes). Fails if a region
    /// of that name exists.
    pub fn create(name: &str, len: usize) -> io::Result<SharedMemory> {
        check(name, len)?;
        let (ptr, os) = imp::create(name, len)?;
        Ok(SharedMemory { ptr, len, os })
    }

    /// Maps the region another process made under `name`, which must be at least `len`
    /// bytes.
    pub fn open(name: &str, len: usize) -> io::Result<SharedMemory> {
        check(name, len)?;
        let (ptr, os) = imp::open(name, len)?;
        Ok(SharedMemory { ptr, len, os })
    }

    /// The first byte. Aligned to the OS page (≥ 4 096 bytes).
    pub fn as_ptr(&self) -> *mut u8 {
        self.ptr.as_ptr()
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

impl Drop for SharedMemory {
    fn drop(&mut self) {
        imp::close(self.ptr, self.len, &mut self.os);
    }
}

fn check(name: &str, len: usize) -> io::Result<()> {
    let ok = !name.is_empty()
        && name.len() <= 200
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'));
    if !ok || len == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "shared memory: a plain name and a size above zero",
        ));
    }
    Ok(())
}

/// A name no other region of this machine uses now: `prefix`, this process's id, a
/// counter and the clock.
pub fn unique_name(prefix: &str) -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos() as u64 & 0xFFFF_FFFF);
    format!("{prefix}-{}-{n}-{t:x}", std::process::id())
}

#[cfg(windows)]
mod imp {
    use std::io;
    use std::ptr::NonNull;
    use windows_sys::Win32::Foundation::{
        CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, HANDLE, INVALID_HANDLE_VALUE,
    };
    use windows_sys::Win32::System::Memory::{
        CreateFileMappingW, MapViewOfFile, OpenFileMappingW, UnmapViewOfFile, FILE_MAP_ALL_ACCESS,
        MEMORY_MAPPED_VIEW_ADDRESS, PAGE_READWRITE,
    };

    #[derive(Debug)]
    pub struct Handle(HANDLE);

    fn wide(name: &str) -> Vec<u16> {
        format!("Local\\{name}")
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect()
    }

    fn map(h: HANDLE, len: usize) -> io::Result<(NonNull<u8>, Handle)> {
        // SAFETY: `h` is a live mapping handle; mapping `len` bytes of it read-write.
        let view = unsafe { MapViewOfFile(h, FILE_MAP_ALL_ACCESS, 0, 0, len) };
        match NonNull::new(view.Value.cast::<u8>()) {
            Some(p) => Ok((p, Handle(h))),
            None => {
                let e = io::Error::last_os_error();
                // SAFETY: our own handle, closed once.
                unsafe { CloseHandle(h) };
                Err(e)
            }
        }
    }

    pub fn create(name: &str, len: usize) -> io::Result<(NonNull<u8>, Handle)> {
        let w = wide(name);
        let size = len as u64;
        // SAFETY: a pagefile-backed mapping (`INVALID_HANDLE_VALUE`), default security, a
        // NUL-terminated name that outlives the call.
        let h = unsafe {
            CreateFileMappingW(
                INVALID_HANDLE_VALUE,
                std::ptr::null(),
                PAGE_READWRITE,
                (size >> 32) as u32,
                size as u32,
                w.as_ptr(),
            )
        };
        if h.is_null() {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: reads this thread's last error, set by the call above.
        if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
            // SAFETY: our own handle, closed once.
            unsafe { CloseHandle(h) };
            return Err(io::Error::from(io::ErrorKind::AlreadyExists));
        }
        map(h, len)
    }

    pub fn open(name: &str, len: usize) -> io::Result<(NonNull<u8>, Handle)> {
        let w = wide(name);
        // SAFETY: a NUL-terminated name that outlives the call.
        let h = unsafe { OpenFileMappingW(FILE_MAP_ALL_ACCESS, 0, w.as_ptr()) };
        if h.is_null() {
            return Err(io::Error::last_os_error());
        }
        map(h, len)
    }

    pub fn close(ptr: NonNull<u8>, _len: usize, os: &mut Handle) {
        // SAFETY: the view and the handle this value made, released once, here.
        unsafe {
            UnmapViewOfFile(MEMORY_MAPPED_VIEW_ADDRESS {
                Value: ptr.as_ptr().cast(),
            });
            CloseHandle(os.0);
        }
    }
}

#[cfg(unix)]
mod imp {
    use std::ffi::CString;
    use std::io;
    use std::ptr::NonNull;

    #[derive(Debug)]
    pub struct Handle {
        /// The name to unlink on drop; `None` for the side that opened it.
        unlink: Option<CString>,
    }

    fn c_name(name: &str) -> io::Result<CString> {
        CString::new(format!("/{name}")).map_err(|_| io::ErrorKind::InvalidInput.into())
    }

    fn map(fd: libc::c_int, len: usize) -> io::Result<NonNull<u8>> {
        // SAFETY: a shared read-write mapping of a descriptor we own, `len` bytes from 0.
        let p = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                len,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_SHARED,
                fd,
                0,
            )
        };
        let e = io::Error::last_os_error();
        // SAFETY: the descriptor is ours; the mapping keeps the pages without it.
        unsafe { libc::close(fd) };
        if p == libc::MAP_FAILED {
            return Err(e);
        }
        NonNull::new(p.cast::<u8>()).ok_or(e)
    }

    pub fn create(name: &str, len: usize) -> io::Result<(NonNull<u8>, Handle)> {
        let c = c_name(name)?;
        // SAFETY: a NUL-terminated name; flags and mode are plain integers.
        let fd = unsafe {
            libc::shm_open(
                c.as_ptr(),
                libc::O_CREAT | libc::O_EXCL | libc::O_RDWR,
                0o600 as libc::mode_t,
            )
        };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: our descriptor; a fresh object is zero-filled by `ftruncate`.
        if unsafe { libc::ftruncate(fd, len as libc::off_t) } != 0 {
            let e = io::Error::last_os_error();
            // SAFETY: our descriptor and name, released once.
            unsafe {
                libc::close(fd);
                libc::shm_unlink(c.as_ptr());
            }
            return Err(e);
        }
        match map(fd, len) {
            Ok(p) => Ok((p, Handle { unlink: Some(c) })),
            Err(e) => {
                // SAFETY: the name we made, removed once.
                unsafe { libc::shm_unlink(c.as_ptr()) };
                Err(e)
            }
        }
    }

    pub fn open(name: &str, len: usize) -> io::Result<(NonNull<u8>, Handle)> {
        let c = c_name(name)?;
        // SAFETY: a NUL-terminated name.
        let fd = unsafe { libc::shm_open(c.as_ptr(), libc::O_RDWR, 0) };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        map(fd, len).map(|p| (p, Handle { unlink: None }))
    }

    pub fn close(ptr: NonNull<u8>, len: usize, os: &mut Handle) {
        // SAFETY: the mapping this value made, unmapped once; the name removed once.
        unsafe {
            libc::munmap(ptr.as_ptr().cast(), len);
            if let Some(c) = os.unlink.take() {
                libc::shm_unlink(c.as_ptr());
            }
        }
    }
}

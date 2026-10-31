//! A move that never replaces: the system's own atomic no-replace rename (round 34, fix
//! review 4 #3). The exporter's "never replace" places a finished part with a hard link;
//! where the file system has none (FAT32, exFAT, some shares) this is the next step, before
//! a copy that a crash could leave half written at the real path.

use std::io;
use std::path::Path;

/// Move `from` to `to` in one step, failing with [`io::ErrorKind::AlreadyExists`] if anything
/// is at `to`, which is then left as it is. [`io::ErrorKind::Unsupported`] where the system or
/// the file system has no such move. Windows: `MoveFileExW` without
/// `MOVEFILE_REPLACE_EXISTING` (FAT and exFAT included); Linux: `renameat2` with
/// `RENAME_NOREPLACE`; macOS: `renamex_np` with `RENAME_EXCL`.
pub fn rename_no_replace(from: &Path, to: &Path) -> io::Result<()> {
    imp::rename_no_replace(from, to)
}

#[cfg(windows)]
mod imp {
    use std::io;
    use std::os::windows::ffi::OsStrExt;
    use std::path::Path;
    use windows_sys::Win32::Storage::FileSystem::{MoveFileExW, MOVEFILE_WRITE_THROUGH};

    fn wide(p: &Path) -> Vec<u16> {
        p.as_os_str().encode_wide().chain(Some(0)).collect()
    }

    pub fn rename_no_replace(from: &Path, to: &Path) -> io::Result<()> {
        let (f, t) = (wide(from), wide(to));
        // SAFETY: both are NUL-terminated wide strings that outlive the call; without
        // MOVEFILE_REPLACE_EXISTING an existing `to` fails the move (ERROR_ALREADY_EXISTS).
        let ok = unsafe { MoveFileExW(f.as_ptr(), t.as_ptr(), MOVEFILE_WRITE_THROUGH) };
        if ok != 0 {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
mod imp {
    use std::ffi::CString;
    use std::io;
    use std::os::unix::ffi::OsStrExt;
    use std::path::Path;

    fn c(p: &Path) -> io::Result<CString> {
        CString::new(p.as_os_str().as_bytes())
            .map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))
    }

    pub fn rename_no_replace(from: &Path, to: &Path) -> io::Result<()> {
        let (f, t) = (c(from)?, c(to)?);
        // SAFETY: both are NUL-terminated C strings that outlive the call.
        #[cfg(target_os = "linux")]
        let r = unsafe {
            libc::renameat2(
                libc::AT_FDCWD,
                f.as_ptr(),
                libc::AT_FDCWD,
                t.as_ptr(),
                libc::RENAME_NOREPLACE,
            )
        };
        // SAFETY: as above.
        #[cfg(target_os = "macos")]
        let r = unsafe { libc::renamex_np(f.as_ptr(), t.as_ptr(), libc::RENAME_EXCL) };
        if r == 0 {
            return Ok(());
        }
        let e = io::Error::last_os_error();
        match e.raw_os_error() {
            // The file system (or an old kernel) has no no-replace rename.
            Some(libc::EINVAL | libc::ENOSYS | libc::ENOTSUP) => {
                Err(io::ErrorKind::Unsupported.into())
            }
            _ => Err(e),
        }
    }
}

#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
mod imp {
    use std::io;
    use std::path::Path;

    pub fn rename_no_replace(_: &Path, _: &Path) -> io::Result<()> {
        Err(io::ErrorKind::Unsupported.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moves_and_never_replaces() {
        let d = std::env::temp_dir().join(format!("cantus-rt-place-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let (a, b) = (d.join("a.part"), d.join("a.wav"));
        std::fs::write(&a, b"first").unwrap();
        rename_no_replace(&a, &b).unwrap();
        assert!(!a.exists());
        assert_eq!(std::fs::read(&b).unwrap(), b"first");
        std::fs::write(&a, b"second").unwrap();
        let e = rename_no_replace(&a, &b).unwrap_err();
        assert_eq!(e.kind(), io::ErrorKind::AlreadyExists, "{e}");
        assert_eq!(std::fs::read(&b).unwrap(), b"first", "never replaced");
        assert_eq!(std::fs::read(&a).unwrap(), b"second", "the part stays");
        let _ = std::fs::remove_dir_all(&d);
    }
}

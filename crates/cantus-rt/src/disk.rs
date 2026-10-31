//! The volume a folder is on: its size and the space free on it (ADR 0050: the sample
//! cache's automatic limit and the floor of free space it leaves). Off the audio thread.

use std::path::Path;

/// A volume's size and the bytes free on it for this user, bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Space {
    pub total: u64,
    pub free: u64,
}

/// The volume `path` is on (the nearest folder of it that exists), or `None` when the
/// system cannot say.
pub fn space(path: &Path) -> Option<Space> {
    let mut p = path;
    while !p.exists() {
        p = p.parent()?;
    }
    os_space(p)
}

#[cfg(windows)]
fn os_space(path: &Path) -> Option<Space> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
    let wide: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let (mut free, mut total, mut all_free) = (0u64, 0u64, 0u64);
    // SAFETY: `wide` is a NUL-terminated UTF-16 string that outlives the call; the three
    // out-pointers are to live locals.
    let ok = unsafe { GetDiskFreeSpaceExW(wide.as_ptr(), &mut free, &mut total, &mut all_free) };
    (ok != 0).then_some(Space { total, free })
}

#[cfg(unix)]
// The fields' widths differ by system: a cast needed on one is plain on another.
#[allow(clippy::unnecessary_cast)]
fn os_space(path: &Path) -> Option<Space> {
    use std::os::unix::ffi::OsStrExt;
    let c = std::ffi::CString::new(path.as_os_str().as_bytes()).ok()?;
    let mut s = std::mem::MaybeUninit::<libc::statvfs>::zeroed();
    // SAFETY: `c` is a NUL-terminated path that outlives the call; `s` is writable memory
    // of the right type, read only after the call reported success.
    let r = unsafe { libc::statvfs(c.as_ptr(), s.as_mut_ptr()) };
    if r != 0 {
        return None;
    }
    // SAFETY: statvfs returned 0, so it filled `s`.
    let s = unsafe { s.assume_init() };
    let unit = s.f_frsize as u64;
    Some(Space {
        total: (s.f_blocks as u64).saturating_mul(unit),
        free: (s.f_bavail as u64).saturating_mul(unit),
    })
}

#[cfg(not(any(windows, unix)))]
fn os_space(_: &Path) -> Option<Space> {
    None
}

// r34:stems — which volume a folder is on, so two folders are known to share one.
/// The volume `path` is on (the nearest folder of it that exists), as an identity two
/// folders on one volume share: its mount point on Windows (`GetVolumePathNameW`), its
/// device on Unix (`st_dev`). `None` when the system cannot say.
pub fn volume(path: &Path) -> Option<String> {
    let mut p = path;
    while !p.exists() {
        p = p.parent()?;
    }
    os_volume(p)
}

#[cfg(windows)]
fn os_volume(path: &Path) -> Option<String> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::GetVolumePathNameW;
    let wide: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let mut buf = vec![0u16; 1024];
    // SAFETY: `wide` is a NUL-terminated UTF-16 string and `buf` a writable buffer of the
    // length passed, both outliving the call.
    let ok = unsafe { GetVolumePathNameW(wide.as_ptr(), buf.as_mut_ptr(), buf.len() as u32) };
    if ok == 0 {
        return None;
    }
    let n = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    Some(String::from_utf16_lossy(&buf[..n]).to_lowercase())
}

#[cfg(unix)]
fn os_volume(path: &Path) -> Option<String> {
    use std::os::unix::fs::MetadataExt;
    std::fs::metadata(path)
        .ok()
        .map(|m| format!("dev {}", m.dev()))
}

#[cfg(not(any(windows, unix)))]
fn os_volume(_: &Path) -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_temporary_folders_volume_has_a_size_and_free_space() {
        let s = space(&std::env::temp_dir().join("not/there/yet")).expect("a volume");
        assert!(s.total > 0 && s.free <= s.total, "{s:?}");
    }

    // r34:stems
    #[test]
    fn folders_on_one_volume_share_its_identity() {
        let dir = std::env::temp_dir();
        let a = volume(&dir).expect("a volume");
        assert_eq!(volume(&dir.join("not/there/yet")), Some(a.clone()));
        let here = volume(Path::new(env!("CARGO_MANIFEST_DIR"))).expect("a volume");
        assert!(!here.is_empty());
    }
}

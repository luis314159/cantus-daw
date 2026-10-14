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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_temporary_folders_volume_has_a_size_and_free_space() {
        let s = space(&std::env::temp_dir().join("not/there/yet")).expect("a volume");
        assert!(s.total > 0 && s.free <= s.total, "{s:?}");
    }
}

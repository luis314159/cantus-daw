//! Secrets and files kept to their owner (round 34, ADR 0081: the MCP door's token, file
//! tools), the only OS code for it (ADR 0031 §2.7):
//!
//! - [`secure_dir`]: a folder for a secret — this user's (anyone else's is refused), made
//!   private (Unix 0700; Windows a protected DACL, *OWNER RIGHTS* alone, inherited by what
//!   it holds), its parent unable to swap it;
//! - [`create_secret`]: a new file owner-only *from its creation* (Unix `O_CREAT|O_EXCL`
//!   0600 without following a link; Windows `CreateFileW` with its security descriptor),
//!   so nothing can open it in between (review 4 #4);
//! - [`owned_by_me`], [`owner_only`];
//! - [`open_no_follow`] and [`real_path`]: a file opened without following a link at its
//!   last step, and where the opened handle really is (review 4 #6, #8).

use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};

/// How a file is opened by [`open_no_follow`].
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Open {
    Read,
    /// A new file; refused if anything (a file, a link) is there.
    CreateNew,
    /// For writing whole, made if missing — **not** truncated: the caller checks
    /// [`real_path`] first, then `set_len(0)`.
    Write,
}

/// Open `path` without following a link at its last step.
pub fn open_no_follow(path: &Path, how: Open) -> io::Result<File> {
    let mut o = std::fs::OpenOptions::new();
    match how {
        Open::Read => o.read(true),
        Open::CreateNew => o.write(true).create_new(true),
        Open::Write => o.write(true).create(true),
    };
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        o.custom_flags(libc::O_NOFOLLOW);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        // FILE_FLAG_OPEN_REPARSE_POINT: a link is opened as itself, never followed.
        o.custom_flags(0x0020_0000);
    }
    let f = o.open(path)?;
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        // FILE_ATTRIBUTE_REPARSE_POINT: it was a link (opened as itself): refused.
        if f.metadata()?.file_attributes() & 0x400 != 0 {
            return Err(io::Error::new(io::ErrorKind::PermissionDenied, "a link"));
        }
    }
    Ok(f)
}

/// Where an opened file really is, from its handle (not from a path that may have
/// changed since).
pub fn real_path(file: &File) -> io::Result<PathBuf> {
    #[cfg(target_os = "linux")]
    {
        use std::os::fd::AsRawFd;
        std::fs::read_link(format!("/proc/self/fd/{}", file.as_raw_fd()))
    }
    #[cfg(target_os = "macos")]
    {
        use std::os::fd::AsRawFd;
        let mut buf = vec![0u8; libc::PATH_MAX as usize];
        // SAFETY: F_GETPATH writes at most MAXPATHLEN (= PATH_MAX) bytes into `buf`, which
        // is that long; the descriptor is the open file's.
        let r = unsafe { libc::fcntl(file.as_raw_fd(), libc::F_GETPATH, buf.as_mut_ptr()) };
        if r == -1 {
            return Err(io::Error::last_os_error());
        }
        let end = buf.iter().position(|b| *b == 0).unwrap_or(buf.len());
        use std::os::unix::ffi::OsStrExt;
        Ok(PathBuf::from(std::ffi::OsStr::from_bytes(&buf[..end])))
    }
    #[cfg(windows)]
    {
        windows::final_path(file)
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
    {
        let _ = file;
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "no real path here",
        ))
    }
}

/// A folder to keep a secret in: made if missing, refused unless this user owns it, then
/// made private — on Unix mode 0700 (a 0775 folder from a umask of 002 is tightened, not
/// refused: review 4 #1), its parent not writable by others (unless sticky) nor by a group
/// that is not this user's; on Windows a protected owner-only DACL.
pub fn secure_dir(dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    if !owned_by_me(dir).map_err(|e| e.to_string())? {
        return Err(format!("{} is not this user's", dir.display()));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        let m = std::fs::metadata(dir).map_err(|e| e.to_string())?;
        if m.mode() & 0o077 != 0 {
            std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))
                .map_err(|e| e.to_string())?;
        }
        if let Some(parent) = dir.parent().filter(|p| !p.as_os_str().is_empty()) {
            let p = std::fs::metadata(parent).map_err(|e| e.to_string())?;
            // SAFETY: geteuid has no preconditions and cannot fail.
            let me = unsafe { libc::geteuid() };
            let owner_ok = p.uid() == me || p.uid() == 0;
            let sticky = p.mode() & 0o1000 != 0;
            let world = p.mode() & 0o002 != 0 && !sticky;
            // A group may write in it only when that group is this user's alone (review 5
            // #13: a shared `users` group with a umask of 002 let others swap the folder).
            let group = p.mode() & 0o020 != 0 && !sticky && !private_group(p.gid(), me);
            if !owner_ok || world || group {
                return Err(format!(
                    "{} could be replaced by another user",
                    dir.display()
                ));
            }
        }
        Ok(())
    }
    #[cfg(windows)]
    {
        // This user by SID, not OWNER RIGHTS: a file an elevated run made is owned by
        // Administrators, and the user must still read it (review 5 #2).
        windows::set_dacl(dir, &windows::user_dacl(true).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())
    }
    #[cfg(not(any(unix, windows)))]
    {
        Ok(())
    }
}

/// Whether group `gid` is user `uid`'s alone: the user's own primary group, named as the
/// user, with no other member (the "user private group" most distributions make) — a
/// shared group (`users`, `staff`) is not.
#[cfg(unix)]
pub fn private_group(gid: u32, uid: u32) -> bool {
    use std::ffi::CStr;
    let mut pw_buf = vec![0u8; 16 << 10];
    // SAFETY: a zeroed passwd is a valid out-parameter; getpwuid_r fills it from `pw_buf`.
    let mut pw: libc::passwd = unsafe { std::mem::zeroed() };
    let mut pw_found: *mut libc::passwd = std::ptr::null_mut();
    // SAFETY: the buffers outlive the call and their lengths are given.
    let r = unsafe {
        libc::getpwuid_r(
            uid,
            &mut pw,
            pw_buf.as_mut_ptr().cast(),
            pw_buf.len(),
            &mut pw_found,
        )
    };
    if r != 0 || pw_found.is_null() || pw.pw_gid != gid {
        return false;
    }
    // SAFETY: getpwuid_r succeeded: pw_name is a NUL-terminated string in `pw_buf`.
    let user = unsafe { CStr::from_ptr(pw.pw_name) }.to_bytes().to_vec();
    let mut gr_buf = vec![0u8; 64 << 10];
    // SAFETY: as above, for the group.
    let mut gr: libc::group = unsafe { std::mem::zeroed() };
    let mut gr_found: *mut libc::group = std::ptr::null_mut();
    // SAFETY: the buffers outlive the call and their lengths are given.
    let r = unsafe {
        libc::getgrgid_r(
            gid,
            &mut gr,
            gr_buf.as_mut_ptr().cast(),
            gr_buf.len(),
            &mut gr_found,
        )
    };
    if r != 0 || gr_found.is_null() {
        return false;
    }
    // SAFETY: getgrgid_r succeeded: gr_name and the NULL-terminated gr_mem list point into
    // `gr_buf`.
    unsafe {
        if CStr::from_ptr(gr.gr_name).to_bytes() != user.as_slice() {
            return false;
        }
        let mut m = gr.gr_mem;
        while !m.is_null() && !(*m).is_null() {
            if CStr::from_ptr(*m).to_bytes() != user.as_slice() {
                return false;
            }
            m = m.add(1);
        }
    }
    true
}

/// Create `path` new, readable and writable by this user only from the start. Fails if
/// anything is there (a file, a link).
pub fn create_secret(path: &Path) -> io::Result<File> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        std::fs::OpenOptions::new()
            .write(true)
            .read(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(path)
    }
    #[cfg(windows)]
    {
        windows::create_secret(path)
    }
    #[cfg(not(any(unix, windows)))]
    {
        std::fs::OpenOptions::new()
            .write(true)
            .read(true)
            .create_new(true)
            .open(path)
    }
}

/// Whether this user owns `path`.
pub fn owned_by_me(path: &Path) -> io::Result<bool> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let m = std::fs::symlink_metadata(path)?;
        // SAFETY: geteuid has no preconditions and cannot fail.
        Ok(m.uid() == unsafe { libc::geteuid() })
    }
    #[cfg(windows)]
    {
        windows::owned_by_me(path)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = path;
        Ok(true)
    }
}

/// Make `path` (an existing file) readable and writable by its owner only.
pub fn owner_only(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
    }
    #[cfg(windows)]
    {
        windows::set_dacl(path, &windows::user_dacl(false)?)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = path;
        Ok(())
    }
}

/// What a file system without owners answers: no secret is kept there.
pub const NO_OWNER: &str = "this drive keeps no owner for its files (FAT32, exFAT or a network share), so external agents cannot keep a secret here; put Cantus's settings on an NTFS drive";

/// BUILTIN\Administrators (S-1-5-32-544), as SID bytes.
#[cfg(windows)]
const ADMINISTRATORS: [u8; 16] = [1, 2, 0, 0, 0, 0, 0, 5, 32, 0, 0, 0, 0x20, 2, 0, 0];

/// Whether an owner (`None`: the file system keeps none) is this user (`me`) — refused with
/// [`NO_OWNER`] rather than compared (review 5 #1: a null owner went into `EqualSid`).
/// Administrators counts as this user when the user is one of them (`admin`): what an
/// elevated run of an administrator makes is owned by the group (review 5 #2).
#[cfg(windows)]
fn judge_owner(owner: Option<&[u8]>, me: &[u8], admin: impl FnOnce() -> bool) -> io::Result<bool> {
    match owner {
        None => Err(io::Error::new(io::ErrorKind::Unsupported, NO_OWNER)),
        Some(o) if o == me => Ok(true),
        Some(o) => Ok(o == ADMINISTRATORS && admin()),
    }
}

/// The DACL of `path` in SDDL (`D:P(A;;FA;;;OW)`), for tests.
#[cfg(windows)]
pub fn dacl_sddl(path: &Path) -> io::Result<String> {
    windows::dacl_sddl(path)
}

#[cfg(windows)]
mod windows {
    use std::fs::File;
    use std::io;
    use std::os::windows::ffi::{OsStrExt, OsStringExt};
    use std::os::windows::io::{AsRawHandle, FromRawHandle};
    use std::path::{Path, PathBuf};
    use windows_sys::Win32::Foundation::{
        CloseHandle, LocalFree, GENERIC_READ, GENERIC_WRITE, HANDLE, HLOCAL, INVALID_HANDLE_VALUE,
    };
    use windows_sys::Win32::Security::Authorization::{
        ConvertSecurityDescriptorToStringSecurityDescriptorW, ConvertSidToStringSidW,
        ConvertStringSecurityDescriptorToSecurityDescriptorW, GetNamedSecurityInfoW,
        GetSecurityInfo, SDDL_REVISION_1, SE_FILE_OBJECT,
    };
    use windows_sys::Win32::Security::{
        GetLengthSid, GetSecurityDescriptorOwner, GetTokenInformation, SetFileSecurityW,
        TokenGroups, TokenUser, DACL_SECURITY_INFORMATION, OWNER_SECURITY_INFORMATION,
        PROTECTED_DACL_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR, PSID, SECURITY_ATTRIBUTES,
        SID_AND_ATTRIBUTES, TOKEN_GROUPS, TOKEN_QUERY, TOKEN_USER,
    };
    use windows_sys::Win32::Storage::FileSystem::{
        CreateFileW, GetFinalPathNameByHandleW, CREATE_NEW, FILE_ATTRIBUTE_NORMAL,
        FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, FILE_NAME_NORMALIZED,
        FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING, READ_CONTROL,
    };
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    fn wide(s: &std::ffi::OsStr) -> Vec<u16> {
        s.encode_wide().chain(std::iter::once(0)).collect()
    }

    /// A security descriptor made from SDDL, freed when dropped.
    pub(super) struct Sd(pub(super) PSECURITY_DESCRIPTOR);

    impl Sd {
        pub(super) fn from_sddl(sddl: &str) -> io::Result<Sd> {
            let text = wide(std::ffi::OsStr::new(sddl));
            let mut sd: PSECURITY_DESCRIPTOR = std::ptr::null_mut();
            // SAFETY: `text` is NUL-terminated and outlives the call; `sd` receives a
            // descriptor the system allocates, freed in Drop.
            let ok = unsafe {
                ConvertStringSecurityDescriptorToSecurityDescriptorW(
                    text.as_ptr(),
                    SDDL_REVISION_1,
                    &mut sd,
                    std::ptr::null_mut(),
                )
            };
            if ok == 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(Sd(sd))
        }
    }

    impl Drop for Sd {
        fn drop(&mut self) {
            // SAFETY: allocated by the system for us (LocalAlloc), freed once.
            unsafe {
                LocalFree(self.0 as HLOCAL);
            }
        }
    }

    pub fn set_dacl(path: &Path, sddl: &str) -> io::Result<()> {
        let sd = Sd::from_sddl(sddl)?;
        let file = wide(path.as_os_str());
        // SAFETY: `file` is a NUL-terminated wide path; `sd` is a valid descriptor.
        let ok = unsafe {
            SetFileSecurityW(
                file.as_ptr(),
                DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
                sd.0,
            )
        };
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    /// This process's user SID, as bytes (copied out of the token's buffer).
    pub(super) fn my_sid() -> io::Result<Vec<u8>> {
        let mut token: HANDLE = std::ptr::null_mut();
        // SAFETY: GetCurrentProcess returns a pseudo-handle; `token` receives a handle we
        // close below.
        if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
            return Err(io::Error::last_os_error());
        }
        let mut len = 0u32;
        // SAFETY: a size query (null buffer, 0 length) on our own token.
        unsafe { GetTokenInformation(token, TokenUser, std::ptr::null_mut(), 0, &mut len) };
        let mut buf = vec![0u8; len as usize];
        // SAFETY: `buf` is `len` bytes, as the query said; the token is open.
        let ok = unsafe {
            GetTokenInformation(token, TokenUser, buf.as_mut_ptr().cast(), len, &mut len)
        };
        // SAFETY: the token handle is ours, closed once.
        unsafe { CloseHandle(token) };
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: GetTokenInformation(TokenUser) filled `buf` with a TOKEN_USER whose SID
        // points inside `buf`; read unaligned, its SID is copied whole below.
        let user: TOKEN_USER = unsafe { std::ptr::read_unaligned(buf.as_ptr().cast()) };
        let sid = user.User.Sid as *const u8;
        // A SID is 8 bytes plus 4 per sub-authority (its second byte counts them).
        // SAFETY: `sid` points at a valid SID inside `buf`.
        let n = unsafe { *sid.add(1) } as usize;
        let size = 8 + 4 * n;
        // SAFETY: the SID's `size` bytes lie inside `buf`.
        Ok(unsafe { std::slice::from_raw_parts(sid, size) }.to_vec())
    }

    /// The owner a security descriptor names, copied out — `None` when it names none, as
    /// FAT32, exFAT and some network shares answer (review 5 #1).
    pub(super) fn owner_in(sd: PSECURITY_DESCRIPTOR) -> io::Result<Option<Vec<u8>>> {
        let mut owner: PSID = std::ptr::null_mut();
        let mut defaulted = 0;
        // SAFETY: `sd` is a valid descriptor; `owner` receives a pointer into it (or null).
        if unsafe { GetSecurityDescriptorOwner(sd, &mut owner, &mut defaulted) } == 0 {
            return Err(io::Error::last_os_error());
        }
        if owner.is_null() {
            return Ok(None);
        }
        // SAFETY: `owner` is a valid SID inside `sd`; GetLengthSid gives its size.
        let len = unsafe { GetLengthSid(owner) } as usize;
        // SAFETY: the SID's `len` bytes lie inside `sd`, which outlives this copy.
        Ok(Some(
            unsafe { std::slice::from_raw_parts(owner as *const u8, len) }.to_vec(),
        ))
    }

    /// The owner of `path` itself — a link is opened as itself, never followed (review 5
    /// #10) — or `None` when its file system keeps none.
    fn owner_of(path: &Path) -> io::Result<Option<Vec<u8>>> {
        let file = wide(path.as_os_str());
        // SAFETY: `file` is a NUL-terminated wide path; the handle is closed below.
        let h = unsafe {
            CreateFileW(
                file.as_ptr(),
                READ_CONTROL,
                FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                std::ptr::null(),
                OPEN_EXISTING,
                FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
                std::ptr::null_mut(),
            )
        };
        if h == INVALID_HANDLE_VALUE {
            return Err(io::Error::last_os_error());
        }
        let mut owner: PSID = std::ptr::null_mut();
        let mut sd: PSECURITY_DESCRIPTOR = std::ptr::null_mut();
        // SAFETY: `h` is open with READ_CONTROL; `sd` is system-allocated, freed below.
        let err = unsafe {
            GetSecurityInfo(
                h,
                SE_FILE_OBJECT,
                OWNER_SECURITY_INFORMATION,
                &mut owner,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &mut sd,
            )
        };
        // SAFETY: the handle is ours, closed once.
        unsafe { CloseHandle(h) };
        if err != 0 {
            return Err(io::Error::from_raw_os_error(err as i32));
        }
        let found = owner_in(sd);
        // SAFETY: `sd` was allocated by the system for us, freed once — before anything
        // else can return.
        unsafe { LocalFree(sd as HLOCAL) };
        found
    }

    /// A SID in its string form (`S-1-5-21-…`).
    pub(super) fn sid_string(sid: &[u8]) -> io::Result<String> {
        let mut copy = sid.to_vec();
        let mut text: *mut u16 = std::ptr::null_mut();
        // SAFETY: `copy` holds a valid SID; `text` receives a system-allocated string,
        // freed below.
        if unsafe { ConvertSidToStringSidW(copy.as_mut_ptr().cast(), &mut text) } == 0 {
            return Err(io::Error::last_os_error());
        }
        let mut n = 0;
        // SAFETY: `text` is NUL-terminated; counted up to it.
        while unsafe { *text.add(n) } != 0 {
            n += 1;
        }
        // SAFETY: `text` holds `n` wide chars.
        let out = String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(text, n) });
        // SAFETY: allocated by the system for us, freed once.
        unsafe { LocalFree(text as HLOCAL) };
        Ok(out)
    }

    /// A protected DACL giving this user — by SID — full control (`folder`: inherited by
    /// what it holds), nothing else.
    pub(super) fn user_dacl(folder: bool) -> io::Result<String> {
        let me = sid_string(&my_sid()?)?;
        let inherit = if folder { "OICI" } else { "" };
        Ok(format!("D:P(A;{inherit};FA;;;{me})"))
    }

    /// Whether this process's token holds `group` — enabled or for deny only (a filtered
    /// administrator's token holds Administrators that way).
    pub(super) fn in_group(group: &[u8]) -> io::Result<bool> {
        let mut token: HANDLE = std::ptr::null_mut();
        // SAFETY: a pseudo-handle for this process; `token` is closed below.
        if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
            return Err(io::Error::last_os_error());
        }
        let mut len = 0u32;
        // SAFETY: a size query on our own token.
        unsafe { GetTokenInformation(token, TokenGroups, std::ptr::null_mut(), 0, &mut len) };
        // u64s: the TOKEN_GROUPS header and its pointers aligned.
        let mut buf = vec![0u64; (len as usize).div_ceil(8)];
        // SAFETY: `buf` holds at least `len` bytes; the token is open.
        let ok = unsafe {
            GetTokenInformation(token, TokenGroups, buf.as_mut_ptr().cast(), len, &mut len)
        };
        // SAFETY: the token handle is ours, closed once.
        unsafe { CloseHandle(token) };
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
        let groups = buf.as_ptr() as *const TOKEN_GROUPS;
        // SAFETY: the system filled `buf` with a TOKEN_GROUPS of GroupCount entries, each
        // SID pointing inside `buf`.
        let found = unsafe {
            let n = (*groups).GroupCount as usize;
            let first = std::ptr::addr_of!((*groups).Groups) as *const SID_AND_ATTRIBUTES;
            (0..n).any(|i| {
                let sid = (*first.add(i)).Sid;
                let len = GetLengthSid(sid) as usize;
                std::slice::from_raw_parts(sid as *const u8, len) == group
            })
        };
        Ok(found)
    }

    pub fn owned_by_me(path: &Path) -> io::Result<bool> {
        let owner = owner_of(path)?;
        super::judge_owner(owner.as_deref(), &my_sid()?, || {
            in_group(&super::ADMINISTRATORS).unwrap_or(false)
        })
    }

    pub fn create_secret(path: &Path) -> io::Result<File> {
        let sd = Sd::from_sddl(&user_dacl(false)?)?;
        let sa = SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: sd.0,
            bInheritHandle: 0,
        };
        let file = wide(path.as_os_str());
        // SAFETY: `file` is a NUL-terminated wide path; `sa` and its descriptor outlive the
        // call; CREATE_NEW fails if anything is there.
        let h = unsafe {
            CreateFileW(
                file.as_ptr(),
                GENERIC_READ | GENERIC_WRITE,
                0,
                &sa,
                CREATE_NEW,
                // A dangling link there is not followed to make its target (review 5 #10).
                FILE_ATTRIBUTE_NORMAL | FILE_FLAG_OPEN_REPARSE_POINT,
                std::ptr::null_mut(),
            )
        };
        if h == INVALID_HANDLE_VALUE {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: `h` is a valid file handle we own; the File takes it over.
        Ok(unsafe { File::from_raw_handle(h) })
    }

    pub fn final_path(f: &File) -> io::Result<PathBuf> {
        let h = f.as_raw_handle() as HANDLE;
        let mut buf = vec![0u16; 1024];
        loop {
            // SAFETY: `h` is the open file's handle; `buf` is `buf.len()` wide chars.
            let n = unsafe {
                GetFinalPathNameByHandleW(
                    h,
                    buf.as_mut_ptr(),
                    buf.len() as u32,
                    FILE_NAME_NORMALIZED,
                )
            } as usize;
            if n == 0 {
                return Err(io::Error::last_os_error());
            }
            if n < buf.len() {
                buf.truncate(n);
                return Ok(PathBuf::from(std::ffi::OsString::from_wide(&buf)));
            }
            buf.resize(n + 1, 0);
        }
    }

    pub fn dacl_sddl(path: &Path) -> io::Result<String> {
        let file = wide(path.as_os_str());
        let mut sd: PSECURITY_DESCRIPTOR = std::ptr::null_mut();
        // SAFETY: `file` is a NUL-terminated wide path; `sd` is system-allocated, freed
        // below.
        let err = unsafe {
            GetNamedSecurityInfoW(
                file.as_ptr(),
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &mut sd,
            )
        };
        if err != 0 {
            return Err(io::Error::from_raw_os_error(err as i32));
        }
        let mut text: *mut u16 = std::ptr::null_mut();
        let mut len = 0u32;
        // SAFETY: `sd` is valid; `text` receives a system-allocated wide string.
        let ok = unsafe {
            ConvertSecurityDescriptorToStringSecurityDescriptorW(
                sd,
                SDDL_REVISION_1,
                DACL_SECURITY_INFORMATION,
                &mut text,
                &mut len,
            )
        };
        let out = if ok == 0 {
            Err(io::Error::last_os_error())
        } else {
            // SAFETY: `text` holds `len` wide chars (the NUL among them).
            let s = unsafe { std::slice::from_raw_parts(text, len as usize) };
            let s = s.split(|c| *c == 0).next().unwrap_or(s);
            Ok(String::from_utf16_lossy(s))
        };
        // SAFETY: both were allocated by the system for us, each freed once.
        unsafe {
            if !text.is_null() {
                LocalFree(text as HLOCAL);
            }
            LocalFree(sd as HLOCAL);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("cantus-rt-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    /// Review 4 #2, #4: a secret is the owner's alone from its creation, read back from
    /// its DACL (not from a localised tool's output).
    #[test]
    fn a_secret_file_is_the_owners_alone_from_its_creation() {
        let dir = temp("secret");
        secure_dir(&dir).unwrap();
        let f = dir.join("secret");
        let mut file = create_secret(&f).unwrap();
        std::io::Write::write_all(&mut file, b"x").unwrap();
        drop(file);
        assert_eq!(std::fs::read_to_string(&f).unwrap(), "x");
        assert!(owned_by_me(&f).unwrap());
        assert!(
            create_secret(&f).is_err(),
            "never over something already there"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&f).unwrap().permissions().mode();
            assert_eq!(mode & 0o077, 0);
            let dmode = std::fs::metadata(&dir).unwrap().permissions().mode();
            assert_eq!(dmode & 0o077, 0);
        }
        #[cfg(windows)]
        {
            // The user by SID (review 5 #2), read back from the system.
            let me = windows::sid_string(&windows::my_sid().unwrap()).unwrap();
            assert!(me.starts_with("S-1-5-"), "{me}");
            assert_eq!(dacl_sddl(&f).unwrap(), format!("D:P(A;;FA;;;{me})"));
            assert_eq!(dacl_sddl(&dir).unwrap(), format!("D:P(A;OICI;FA;;;{me})"));
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Review 4 #1: a folder made under a umask of 002 (0775) is tightened, not refused.
    #[cfg(unix)]
    #[test]
    fn a_group_writable_folder_of_ours_is_tightened() {
        use std::os::unix::fs::PermissionsExt;
        let dir = temp("umask");
        // Made by secure_dir itself under a umask of 002 (Debian's and Fedora's default
        // for a user with a group of their own): 0775 from mkdir, tightened to 0700.
        // SAFETY: umask has no preconditions; the old mask is put back below.
        let old = unsafe { libc::umask(0o002) };
        let made = secure_dir(&dir);
        // SAFETY: as above.
        unsafe { libc::umask(old) };
        assert_eq!(made, Ok(()));
        let mode = std::fs::metadata(&dir).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o700);
        // And one already there at 0775 (from an earlier version, or by hand).
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o775)).unwrap();
        assert_eq!(secure_dir(&dir), Ok(()));
        let mode = std::fs::metadata(&dir).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o700);
        // A secret made in it is the owner's alone whatever the umask.
        let f = dir.join("secret");
        // SAFETY: as above.
        let old = unsafe { libc::umask(0o002) };
        let file = create_secret(&f);
        // SAFETY: as above.
        unsafe { libc::umask(old) };
        drop(file.unwrap());
        let mode = std::fs::metadata(&f).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Review 4 #1: a parent others may write in (not sticky) lets the folder be swapped:
    /// refused. One sticky, like /tmp, is not; nor one only our own group may write in.
    #[cfg(unix)]
    #[test]
    fn a_folder_another_user_could_swap_is_refused() {
        use std::os::unix::fs::PermissionsExt;
        let parent = temp("swap");
        std::fs::create_dir_all(&parent).unwrap();
        let dir = parent.join("cantus");
        std::fs::set_permissions(&parent, std::fs::Permissions::from_mode(0o777)).unwrap();
        assert!(secure_dir(&dir).is_err(), "world-writable, not sticky");
        std::fs::set_permissions(&parent, std::fs::Permissions::from_mode(0o1777)).unwrap();
        assert_eq!(secure_dir(&dir), Ok(()), "sticky, as /tmp");
        std::fs::set_permissions(&parent, std::fs::Permissions::from_mode(0o775)).unwrap();
        // Group-writable: fine only when the group is this user's alone (review 5 #13).
        use std::os::unix::fs::MetadataExt;
        let gid = std::fs::metadata(&parent).unwrap().gid();
        // SAFETY: geteuid has no preconditions.
        let me = unsafe { libc::geteuid() };
        if private_group(gid, me) {
            assert_eq!(secure_dir(&dir), Ok(()), "group-writable by our own group");
        } else {
            assert!(
                secure_dir(&dir).is_err(),
                "group-writable by a shared group"
            );
        }
        let _ = std::fs::remove_dir_all(&parent);
    }

    /// Review 5 #1: a descriptor with no owner (FAT32, exFAT, some shares) is refused in
    /// words, never handed to a SID comparison; one with an owner gives it.
    #[cfg(windows)]
    #[test]
    fn a_drive_that_keeps_no_owner_is_refused_not_crashed_on() {
        let none = windows::Sd::from_sddl("D:(A;;FA;;;WD)").unwrap();
        assert_eq!(windows::owner_in(none.0).unwrap(), None);
        let err = judge_owner(None, &[1, 2, 3], || true).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::Unsupported);
        assert!(err.to_string().contains("keeps no owner"));
        let admins = windows::Sd::from_sddl("O:BAD:(A;;FA;;;WD)").unwrap();
        let owner = windows::owner_in(admins.0).unwrap().expect("an owner");
        // S-1-5-32-544: revision 1, one... two sub-authorities, NT authority 5, 32, 544.
        assert_eq!(owner, [1, 2, 0, 0, 0, 0, 0, 5, 32, 0, 0, 0, 0x20, 2, 0, 0]);
        assert_eq!(owner, ADMINISTRATORS);
        assert!(judge_owner(Some(&owner), &owner, || false).unwrap());
        // Review 5 #2: Administrators as owner is this user's only when the user is one.
        assert!(judge_owner(Some(&ADMINISTRATORS), &[1, 1], || true).unwrap());
        assert!(!judge_owner(Some(&ADMINISTRATORS), &[1, 1], || false).unwrap());
        assert!(!judge_owner(Some(&[1, 9]), &[1, 1], || true).unwrap());
    }

    /// Review 5 #10: on Windows a dangling link where a secret goes is not followed to
    /// create its target (where links can be made).
    #[cfg(windows)]
    #[test]
    fn a_secret_is_never_made_through_a_link() {
        let dir = temp("secret-link");
        std::fs::create_dir_all(&dir).unwrap();
        let target = dir.join("elsewhere.txt");
        if std::os::windows::fs::symlink_file(&target, dir.join("secret")).is_ok() {
            assert!(create_secret(&dir.join("secret")).is_err());
            assert!(!target.exists(), "the link's target was not made");
            assert!(
                owned_by_me(&dir.join("secret")).is_ok(),
                "the link itself is read"
            );
        } else {
            eprintln!("links cannot be made by this user here: not run");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Review 5 #13: a shared group is not a user's own — root's group never is a normal
    /// user's, nor a group the user is not in.
    #[cfg(unix)]
    #[test]
    fn only_a_users_own_group_is_private() {
        // SAFETY: geteuid has no preconditions.
        let me = unsafe { libc::geteuid() };
        if me != 0 {
            assert!(!private_group(0, me), "root's group");
        }
        assert!(!private_group(u32::MAX - 7, me), "no such group");
        // Review 6 #9: the user's primary group, judged against an independent reading —
        // `id` and `getent group` — not against the code under test.
        let out = |args: &[&str]| {
            let o = std::process::Command::new(args[0])
                .args(&args[1..])
                .output()
                .ok()?;
            o.status
                .success()
                .then(|| String::from_utf8_lossy(&o.stdout).trim().to_string())
        };
        let (Some(user), Some(group), Some(gid)) =
            (out(&["id", "-un"]), out(&["id", "-gn"]), out(&["id", "-g"]))
        else {
            eprintln!("`id` is not here: the independent reading is not run");
            return;
        };
        let gid: u32 = gid.parse().unwrap();
        // getent group <name> → name:x:gid:member,member
        let members: Vec<String> = out(&["getent", "group", &group])
            .and_then(|l| l.rsplit(':').next().map(str::to_string))
            .map(|m| {
                m.split(',')
                    .filter(|x| !x.is_empty())
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default();
        let expected = group == user && members.iter().all(|m| *m == user);
        assert_eq!(
            private_group(gid, me),
            expected,
            "user {user}, group {group} ({gid}), members {members:?}"
        );
    }

    /// Review 6 #9: the token's group check agrees with Windows' own account of it —
    /// `whoami /groups` lists Administrators (S-1-5-32-544), enabled or for deny only,
    /// exactly when `in_group` finds it.
    #[cfg(windows)]
    #[test]
    fn the_administrators_check_agrees_with_whoami() {
        let Ok(o) = std::process::Command::new("whoami")
            .args(["/groups", "/fo", "csv", "/nh"])
            .output()
        else {
            eprintln!("whoami is not here: not run");
            return;
        };
        let listed = String::from_utf8_lossy(&o.stdout).contains("S-1-5-32-544");
        assert_eq!(windows::in_group(&ADMINISTRATORS).unwrap(), listed);
    }

    /// Review 4 #6, #8: a link is never followed, and an opened file says where it is.
    #[test]
    fn a_file_opened_says_where_it_really_is() {
        let dir = temp("real");
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("a.txt");
        let file = open_no_follow(&f, Open::CreateNew).unwrap();
        let real = real_path(&file).unwrap();
        let want = std::fs::canonicalize(&f).unwrap();
        assert_eq!(real, want);
        assert!(open_no_follow(&f, Open::CreateNew).is_err());
        assert!(open_no_follow(&f, Open::Write).is_ok());
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&f, dir.join("link")).unwrap();
            assert!(open_no_follow(&dir.join("link"), Open::Read).is_err());
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}

//! Cantus's taskbar identity on Windows (its AppUserModelID).
//!
//! In a release `cantus.exe` is the launcher and the window belongs to the program it
//! starts (`cantus-x86-64-v3.exe`). Windows groups taskbar buttons, and resolves a pin, by
//! AppUserModelID, which by default comes from each executable's path — so the Start-menu
//! shortcut (to the launcher) and the window (of the program) would be two buttons, and
//! pinning the window would pin the program past the launcher. The program therefore sets
//! [`APP_ID`] for its process before it opens a window ([`set_app_id`]), and the installer
//! gives the shortcuts the same ID (`xtask/installer/appid.nsh`): one button, and a pin
//! that starts the launcher.

/// `Company.Product`, as Windows' guidelines shape it; the installer's `APP_ID` is this
/// (`xtask/src/nsis.rs` writes it into `config.nsh`).
pub const APP_ID: &str = "Cantus.Cantus";

/// Set [`APP_ID`] as this process's AppUserModelID. Call it first thing in `main`, before
/// any window exists. Off Windows it does nothing. `Err` holds the HRESULT.
pub fn set_app_id() -> Result<(), i32> {
    imp::set(APP_ID)
}

/// This process's explicit AppUserModelID, if one was set.
pub fn current_app_id() -> Option<String> {
    imp::get()
}

#[cfg(windows)]
#[allow(unsafe_code)]
mod imp {
    use windows_sys::Win32::System::Com::CoTaskMemFree;
    use windows_sys::Win32::UI::Shell::{
        GetCurrentProcessExplicitAppUserModelID, SetCurrentProcessExplicitAppUserModelID,
    };

    pub fn set(id: &str) -> Result<(), i32> {
        let wide: Vec<u16> = id.encode_utf16().chain([0]).collect();
        // SAFETY: `wide` is a NUL-terminated UTF-16 string that outlives the call, which
        // copies it.
        let hr = unsafe { SetCurrentProcessExplicitAppUserModelID(wide.as_ptr()) };
        if hr >= 0 {
            Ok(())
        } else {
            Err(hr)
        }
    }

    pub fn get() -> Option<String> {
        let mut p: windows_sys::core::PWSTR = std::ptr::null_mut();
        // SAFETY: on success `p` is a NUL-terminated string allocated with CoTaskMemAlloc,
        // read up to its NUL and then freed once; on failure it is left null.
        unsafe {
            if GetCurrentProcessExplicitAppUserModelID(&mut p) < 0 || p.is_null() {
                return None;
            }
            let mut len = 0;
            while *p.add(len) != 0 {
                len += 1;
            }
            let s = String::from_utf16_lossy(std::slice::from_raw_parts(p, len));
            CoTaskMemFree(p as *const core::ffi::c_void);
            Some(s)
        }
    }
}

#[cfg(not(windows))]
mod imp {
    pub fn set(_id: &str) -> Result<(), i32> {
        Ok(())
    }

    pub fn get() -> Option<String> {
        None
    }
}

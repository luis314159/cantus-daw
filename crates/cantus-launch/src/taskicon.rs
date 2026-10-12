//! The taskbar's and the title bar's icon of a running window, changed live (ADR 0046):
//! the icon's colour is the user's to choose in Appearance.
//!
//! Windows keeps two icons per window — `ICON_BIG`, which the taskbar button and Alt+Tab
//! show, and `ICON_SMALL`, the title bar's. `winit` only sets the small one, so a colour
//! changed through it would never reach the taskbar; this sets both with `WM_SETICON`, each
//! drawn at the exact size the window's monitor wants (the caller renders them, so the
//! small sizes stay on the pixel grid). The icons are PNG images, which
//! `CreateIconFromResourceEx` reads directly (Windows Vista and later).
//!
//! What it does not change: an icon Windows takes from a *file* — the `.exe`'s own, the
//! Start menu's and a pinned shortcut's while Cantus is closed, the installer's — keeps the
//! colours it was built with.

/// The icon sizes, in pixels, the window `hwnd` wants now: (taskbar, title bar) — 32 and
/// 16 at 100 % scale, 48 and 24 at 150 %, and so on. Off Windows: (64, 32).
pub fn icon_sizes(hwnd: isize) -> (u32, u32) {
    imp::sizes(hwnd)
}

/// Give the window `hwnd` these icons, PNG-encoded at the sizes [`icon_sizes`] named.
/// The icons this replaced (if this function set them) are released. Returns whether
/// Windows took both. Off Windows it does nothing and returns `false`.
pub fn set_window_icons(
    hwnd: isize,
    big_png: &[u8],
    big: u32,
    small_png: &[u8],
    small: u32,
) -> bool {
    imp::set(hwnd, big_png, big, small_png, small)
}

#[cfg(windows)]
#[allow(unsafe_code)]
mod imp {
    use std::sync::Mutex;
    use windows_sys::Win32::UI::HiDpi::{GetDpiForWindow, GetSystemMetricsForDpi};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CreateIconFromResourceEx, DestroyIcon, SendMessageW, HICON, ICON_BIG, ICON_SMALL,
        LR_DEFAULTCOLOR, SM_CXICON, SM_CXSMICON, WM_SETICON,
    };

    /// The icons this module set last (as integers, so the static is `Send`), released
    /// once Windows holds their replacements.
    static OURS: Mutex<Option<(isize, isize)>> = Mutex::new(None);

    pub fn sizes(hwnd: isize) -> (u32, u32) {
        // SAFETY: plain queries; a stale or null window handle makes GetDpiForWindow return
        // 0, which is then replaced by the 96-dpi default.
        let dpi = match unsafe { GetDpiForWindow(hwnd as _) } {
            0 => 96,
            d => d,
        };
        let (big, small) = unsafe {
            (
                GetSystemMetricsForDpi(SM_CXICON, dpi),
                GetSystemMetricsForDpi(SM_CXSMICON, dpi),
            )
        };
        (big.clamp(16, 256) as u32, small.clamp(16, 256) as u32)
    }

    fn icon(png: &[u8], px: u32) -> HICON {
        // SAFETY: `png` is a complete PNG image that outlives the call, which copies it;
        // 0x00030000 is the icon format version every Windows since 2000 reads.
        unsafe {
            CreateIconFromResourceEx(
                png.as_ptr(),
                png.len() as u32,
                1,
                0x0003_0000,
                px as i32,
                px as i32,
                LR_DEFAULTCOLOR,
            )
        }
    }

    pub fn set(hwnd: isize, big_png: &[u8], big: u32, small_png: &[u8], small: u32) -> bool {
        if hwnd == 0 {
            return false;
        }
        let (b, s) = (icon(big_png, big), icon(small_png, small));
        if b.is_null() || s.is_null() {
            // SAFETY: each handle is either null or an icon created above and owned here.
            unsafe {
                if !b.is_null() {
                    DestroyIcon(b);
                }
                if !s.is_null() {
                    DestroyIcon(s);
                }
            }
            return false;
        }
        // SAFETY: WM_SETICON takes an icon handle by value; the window keeps using it, so
        // it is only released once replaced. The big icon goes first: Windows derives a
        // missing small icon from it, and the small one then overrides that.
        unsafe {
            SendMessageW(hwnd as _, WM_SETICON, ICON_BIG as usize, b as isize);
            SendMessageW(hwnd as _, WM_SETICON, ICON_SMALL as usize, s as isize);
        }
        let mut ours = OURS.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((ob, os)) = ours.replace((b as isize, s as isize)) {
            // SAFETY: icons this module created and the window no longer uses.
            unsafe {
                DestroyIcon(ob as HICON);
                DestroyIcon(os as HICON);
            }
        }
        true
    }
}

#[cfg(not(windows))]
mod imp {
    pub fn sizes(_hwnd: isize) -> (u32, u32) {
        (64, 32)
    }

    pub fn set(_hwnd: isize, _big: &[u8], _b: u32, _small: &[u8], _s: u32) -> bool {
        false
    }
}

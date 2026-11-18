//! A plug-in's own editor in a native window of its own (ADR 0019).
//!
//! On Windows: a top-level window, owned by Cantus's main window so it stays above it, the
//! plug-in's view embedded as its child (CLAP `win32`, VST3 `HWND`). The window follows the
//! editor's size — the size the editor gives when it opens, and any resize it asks for —
//! and, when the editor can be resized, offers the user's resizing to it. winit's message
//! loop runs on the same thread and dispatches this window's messages with its own.
//!
//! A plug-in that can only float gets no window from us: it opens its own.
//!
//! On Linux: a top-level X11 window (`window/x11.rs`, ADR 0065), transient for Cantus's,
//! the plug-in's view embedded as its child (CLAP `x11`, VST3 `X11EmbedWindowID`).
//! On macOS: an `NSWindow` whose content view holds the plug-in's `NSView` ([`cocoa`]).
//! Elsewhere there is no embedding yet and opening fails with `Unsupported`.

use crate::backend::{EditorSize, HostError, MainThread};
use raw_window_handle::RawWindowHandle;

#[cfg(windows)]
pub use win32::PluginWindow;

thread_local! {
    /// Where the next editor window opens (desktop pixels), when the host chose a place.
    static NEXT_PLACE: std::cell::Cell<Option<(i32, i32)>> = const { std::cell::Cell::new(None) };
}

/// Several screens (ADR 0148 §10): open the next editor window with its top-left corner at
/// `at` (desktop pixels: the screen of the Cantus window that asked for it), or where the
/// system puts it. Taken by the next window made on this thread.
pub fn place_next(at: Option<(i32, i32)>) {
    NEXT_PLACE.with(|c| c.set(at));
}

/// The place asked for the next window, without taking it (tests, diagnostics).
pub fn next_place() -> Option<(i32, i32)> {
    NEXT_PLACE.with(|c| c.get())
}

/// The place asked for the window being made now, once.
#[allow(dead_code)]
pub(crate) fn take_place() -> Option<(i32, i32)> {
    NEXT_PLACE.with(|c| c.take())
}

#[cfg(target_os = "linux")]
mod x11;
#[cfg(target_os = "linux")]
pub use x11::PluginWindow;

#[cfg(target_os = "macos")]
mod cocoa;
#[cfg(target_os = "macos")]
pub use cocoa::PluginWindow;

#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
pub use other::PluginWindow;

#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
mod other {
    use super::*;

    /// No embedding on this system yet.
    pub struct PluginWindow;

    impl PluginWindow {
        pub fn open(
            _title: &str,
            _owner: Option<RawWindowHandle>,
            _main: &mut dyn MainThread,
        ) -> Result<PluginWindow, HostError> {
            Err(HostError::Unsupported("plug-in windows on this system"))
        }
        /// No window of ours stands for a floating one here.
        pub fn floating(_title: &str) -> Option<PluginWindow> {
            None
        }
        pub fn fit(&mut self, _size: EditorSize) {}
        /// Nothing to bring forward: there is no window here (opening fails above).
        pub fn focus(&self) {}
        pub fn is_minimised(&self) -> bool {
            false
        }
        pub fn is_floating(&self) -> bool {
            false
        }
        pub fn is_frontmost(&self) -> bool {
            false
        }
        pub fn closed(&self) -> bool {
            true
        }
        pub fn take_resize(&mut self) -> Option<EditorSize> {
            None
        }
        pub fn handle(&self) -> Option<isize> {
            None
        }
        pub fn owner(&self) -> isize {
            0
        }
        pub fn set_owner(&mut self, _owner: Option<RawWindowHandle>) {}
    }
}

#[cfg(windows)]
mod win32 {
    use super::*;
    use std::cell::Cell;
    use std::num::NonZeroIsize;
    use std::sync::Once;
    use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
    use windows_sys::Win32::Graphics::Gdi::{
        GetStockObject, MonitorFromPoint, BLACK_BRUSH, MONITOR_DEFAULTTONULL,
    };
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        AdjustWindowRectEx, CreateWindowExW, DefWindowProcW, DestroyWindow, GetAncestor, GetWindow,
        GetWindowLongPtrW, GetWindowRect, IsIconic, IsWindowVisible, LoadCursorW, RegisterClassW,
        SetForegroundWindow, SetWindowLongPtrW, SetWindowPos, ShowWindow, WindowFromPoint,
        CW_USEDEFAULT, GA_ROOT, GWLP_HWNDPARENT, GWLP_USERDATA, GWL_EXSTYLE, GWL_STYLE, GW_OWNER,
        HWND_TOP, IDC_ARROW, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
        SWP_NOZORDER, SW_RESTORE, SW_SHOW, WM_CLOSE, WM_SIZE, WNDCLASSW, WS_CLIPCHILDREN,
        WS_EX_TOPMOST, WS_MAXIMIZEBOX, WS_OVERLAPPEDWINDOW, WS_THICKFRAME,
    };

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    const CLASS: &str = "CantusPluginWindow";

    /// What the window procedure reports back to the frame loop.
    #[derive(Default)]
    struct Shared {
        closed: Cell<bool>,
        resize: Cell<Option<(u32, u32)>>,
        /// Set while the host sizes the window itself, so that size is not offered back.
        fitting: Cell<bool>,
    }

    unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
        let shared = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const Shared;
        if let Some(s) = shared.as_ref() {
            match msg {
                WM_CLOSE => {
                    // Closed from the frame loop, after the editor is gone from it.
                    s.closed.set(true);
                    return 0;
                }
                WM_SIZE if !s.fitting.get() => {
                    let (width, height) = ((l as u32) & 0xFFFF, ((l as u32) >> 16) & 0xFFFF);
                    if width > 0 && height > 0 {
                        s.resize.set(Some((width, height)));
                    }
                }
                _ => {}
            }
        }
        DefWindowProcW(hwnd, msg, w, l)
    }

    pub struct PluginWindow {
        hwnd: HWND,
        shared: Box<Shared>,
        style: u32,
        /// The plug-in floats a window of its own; ours stays hidden and is never shown.
        floating: bool,
        /// The window that owns it (Cantus's main window, or a window of its own — ADR
        /// 0148 §10): Windows keeps it above its owner and destroys it with its owner, so a
        /// Cantus window that closes gives its editors to another owner first.
        owner: isize,
    }

    impl PluginWindow {
        /// Ours for a plug-in whose editor floats a window of its own that opened later than
        /// asked (an isolated plug-in whose helper was busy; review isolation-4 #39): hidden,
        /// never shown — it stands for theirs, which closing ours closes.
        /// A hidden window owned by `owner`, with no editor (the owner tests).
        #[cfg(test)]
        pub(crate) fn floating_owned(
            title: &str,
            owner: Option<RawWindowHandle>,
        ) -> Option<PluginWindow> {
            Self::create(title, owner).ok()
        }

        pub fn floating(title: &str) -> Option<PluginWindow> {
            let mut w = Self::create(title, None).ok()?;
            w.floating = true;
            Some(w)
        }

        /// Create the window, embed the editor in it, size it to the editor, show it.
        pub fn open(
            title: &str,
            owner: Option<RawWindowHandle>,
            main: &mut dyn MainThread,
        ) -> Result<PluginWindow, HostError> {
            let mut window = Self::create(title, owner)?;
            let hwnd = window.hwnd;
            let style = window.style;
            // SAFETY: plain Win32 calls on our own window, on the UI thread.
            unsafe {
                let parent = RawWindowHandle::Win32(raw_window_handle::Win32WindowHandle::new(
                    NonZeroIsize::new(hwnd as isize).expect("checked non-null"),
                ));
                let size = main.open_editor(parent)?;
                if main.editor_is_floating() {
                    // The plug-in has a window of its own; ours stays hidden.
                    window.floating = true;
                    return Ok(window);
                }
                if !main.editor_can_resize() {
                    window.style = style & !(WS_THICKFRAME | WS_MAXIMIZEBOX);
                    SetWindowLongPtrW(hwnd, GWL_STYLE, window.style as isize);
                    SetWindowPos(
                        hwnd,
                        std::ptr::null_mut(),
                        0,
                        0,
                        0,
                        0,
                        SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
                    );
                }
                window.fit(size);
                ShowWindow(hwnd, SW_SHOW);
                Ok(window)
            }
        }

        /// The window, hidden, with no editor in it yet.
        fn create(title: &str, owner: Option<RawWindowHandle>) -> Result<PluginWindow, HostError> {
            static REGISTER: Once = Once::new();
            let class = wide(CLASS);
            // SAFETY: plain Win32 calls on the UI thread with valid, NUL-terminated strings.
            unsafe {
                let instance = GetModuleHandleW(std::ptr::null());
                REGISTER.call_once(|| {
                    let wc = WNDCLASSW {
                        style: 0,
                        lpfnWndProc: Some(wndproc),
                        cbClsExtra: 0,
                        cbWndExtra: 0,
                        hInstance: instance,
                        hIcon: std::ptr::null_mut(),
                        hCursor: LoadCursorW(std::ptr::null_mut(), IDC_ARROW),
                        hbrBackground: GetStockObject(BLACK_BRUSH),
                        lpszMenuName: std::ptr::null(),
                        lpszClassName: class.as_ptr(),
                    };
                    RegisterClassW(&wc);
                });
                let owner_hwnd: HWND = owner_of(owner);
                let style = WS_OVERLAPPEDWINDOW | WS_CLIPCHILDREN;
                let name = wide(title);
                let (x, y) = super::take_place().unwrap_or((CW_USEDEFAULT, CW_USEDEFAULT));
                let hwnd = CreateWindowExW(
                    0,
                    class.as_ptr(),
                    name.as_ptr(),
                    style,
                    x,
                    y,
                    640,
                    480,
                    owner_hwnd,
                    std::ptr::null_mut(),
                    instance,
                    std::ptr::null(),
                );
                if hwnd.is_null() {
                    return Err(HostError::Editor("could not create a window".into()));
                }
                let shared = Box::new(Shared::default());
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, &*shared as *const Shared as isize);
                Ok(PluginWindow {
                    hwnd,
                    shared,
                    style,
                    floating: false,
                    owner: owner_hwnd as isize,
                })
            }
        }

        /// Size the window so its client area is `size`.
        pub fn fit(&mut self, size: EditorSize) {
            if size.width == 0 || size.height == 0 {
                return;
            }
            let mut rect = RECT {
                left: 0,
                top: 0,
                right: size.width as i32,
                bottom: size.height as i32,
            };
            self.shared.fitting.set(true);
            // SAFETY: our own window, on the UI thread.
            unsafe {
                AdjustWindowRectEx(&mut rect, self.style, 0, 0);
                SetWindowPos(
                    self.hwnd,
                    std::ptr::null_mut(),
                    0,
                    0,
                    rect.right - rect.left,
                    rect.bottom - rect.top,
                    SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
                );
            }
            self.shared.fitting.set(false);
        }

        /// Bring the window to the front (round 27, ADR 0051): back from the taskbar if it
        /// was minimised — and only then, since `SW_RESTORE` on a maximised window would
        /// un-maximise it — shown, on top of the other windows, and given the keyboard.
        ///
        /// The foreground lock: Windows lets a process take the foreground only while it
        /// has it, and the click that asked for the editor was on Cantus's main window, so
        /// it does. When it is refused anyway (another program came in front meanwhile),
        /// the window still goes on top of its siblings, and Windows flashes its taskbar
        /// button instead of stealing the focus — its own answer, which we keep. A window
        /// kept hidden for a plug-in that floats its own is left alone.
        pub fn focus(&self) {
            if self.floating {
                return;
            }
            // SAFETY: our own window, on the UI thread.
            unsafe {
                if IsIconic(self.hwnd) != 0 {
                    ShowWindow(self.hwnd, SW_RESTORE);
                } else if IsWindowVisible(self.hwnd) == 0 {
                    ShowWindow(self.hwnd, SW_SHOW);
                }
                SetWindowPos(self.hwnd, HWND_TOP, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE);
                SetForegroundWindow(self.hwnd);
            }
        }

        /// Whether the window is minimised to the taskbar.
        pub fn is_minimised(&self) -> bool {
            // SAFETY: a query of our own window.
            !self.floating && unsafe { IsIconic(self.hwnd) != 0 }
        }

        /// Whether the window is in front as the user sees it (round 27, ADR 0051 §3):
        /// shown, not minimised, and what is on top at its middle is this window (or the
        /// plug-in's view inside it) — no other window over it. A plug-in that floats a
        /// window of its own counts as in front: ours is hidden and theirs unknown, so F8
        /// closes it as before.
        /// The plug-in floats a window of its own (ours stays hidden): Cantus cannot bring
        /// it forward, only close it.
        pub fn is_floating(&self) -> bool {
            self.floating
        }

        pub fn is_frontmost(&self) -> bool {
            if self.floating {
                return true;
            }
            // SAFETY: queries of our own window and of the screen, on the UI thread.
            unsafe {
                if IsIconic(self.hwnd) != 0 || IsWindowVisible(self.hwnd) == 0 {
                    return false;
                }
                let mut r = RECT {
                    left: 0,
                    top: 0,
                    right: 0,
                    bottom: 0,
                };
                if GetWindowRect(self.hwnd, &mut r) == 0 {
                    return false;
                }
                let middle = POINT {
                    x: (r.left + r.right) / 2,
                    y: (r.top + r.bottom) / 2,
                };
                // A middle on no screen (the window dragged mostly off it): nothing to
                // bring forward there.
                if MonitorFromPoint(middle, MONITOR_DEFAULTTONULL).is_null() {
                    return true;
                }
                let w = WindowFromPoint(middle);
                if w.is_null() {
                    return true;
                }
                let top = GetAncestor(w, GA_ROOT);
                // Ours, or a window it owns (directly or not): the plug-in's own popups,
                // preset browsers and dialogs, which always stay above their owner.
                let mut owner = top;
                while !owner.is_null() {
                    if owner == self.hwnd {
                        return true;
                    }
                    owner = GetWindow(owner, GW_OWNER);
                }
                // An always-on-top window cannot be gone above by `focus`: it does not
                // count as covering, or F8 could never close the editor.
                (GetWindowLongPtrW(top, GWL_EXSTYLE) as u32) & WS_EX_TOPMOST != 0
            }
        }

        /// The user pressed the window's close box.
        pub fn closed(&self) -> bool {
            self.shared.closed.get()
        }

        /// A size the user dragged the window to, once.
        pub fn take_resize(&mut self) -> Option<EditorSize> {
            self.shared
                .resize
                .take()
                .map(|(width, height)| EditorSize { width, height })
        }

        /// The native window (`HWND`), for tests and diagnostics.
        pub fn handle(&self) -> Option<isize> {
            Some(self.hwnd as isize)
        }

        /// The window that owns it (`0`: none).
        pub fn owner(&self) -> isize {
            self.owner
        }

        /// Give it to another owner (ADR 0148 §10): a Cantus window about to close hands
        /// its editors to the main window first, or Windows would destroy them with it.
        pub fn set_owner(&mut self, owner: Option<RawWindowHandle>) {
            let hwnd = owner_of(owner);
            // SAFETY: our own window; GWLP_HWNDPARENT on a top-level window sets its owner.
            unsafe {
                SetWindowLongPtrW(self.hwnd, GWLP_HWNDPARENT, hwnd as isize);
            }
            self.owner = hwnd as isize;
        }
    }

    fn owner_of(owner: Option<RawWindowHandle>) -> HWND {
        match owner {
            Some(RawWindowHandle::Win32(h)) => h.hwnd.get() as HWND,
            _ => std::ptr::null_mut(),
        }
    }

    impl Drop for PluginWindow {
        fn drop(&mut self) {
            // SAFETY: our own window; the procedure stops reading `shared` first.
            unsafe {
                SetWindowLongPtrW(self.hwnd, GWLP_USERDATA, 0);
                DestroyWindow(self.hwnd);
            }
        }
    }
}

/// Several screens (ADR 0148 §10): Windows destroys an owned window with its owner — the
/// bug a Cantus window closing would have caused its plug-in editors — so a window about to
/// go hands them to another owner first.
#[cfg(all(test, windows))]
mod owner_tests {
    use super::PluginWindow;
    use raw_window_handle::{RawWindowHandle, Win32WindowHandle};
    use std::num::NonZeroIsize;
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetWindow, IsWindow, GW_OWNER};

    fn raw(h: isize) -> RawWindowHandle {
        RawWindowHandle::Win32(Win32WindowHandle::new(NonZeroIsize::new(h).unwrap()))
    }

    #[test]
    fn an_editor_given_to_another_owner_outlives_the_first() {
        // Two stand-ins for Cantus's windows (hidden), an editor's window owned by the first.
        let main = PluginWindow::floating("main").expect("a window");
        let other = PluginWindow::floating("other").expect("a window");
        let other_h = other.handle().unwrap();
        let main_h = main.handle().unwrap();
        super::place_next(Some((120, 140)));
        let mut editor = PluginWindow::floating_owned("editor", Some(raw(other_h))).unwrap();
        let ed = editor.handle().unwrap();
        // SAFETY: plain queries on live windows of this thread.
        unsafe {
            assert_eq!(GetWindow(ed as _, GW_OWNER) as isize, other_h);
        }
        assert_eq!(editor.owner(), other_h);
        assert!(
            super::take_place().is_none(),
            "the place was taken by the window made"
        );
        // The window that opened it closes: its editor goes to the main window first.
        editor.set_owner(Some(raw(main_h)));
        assert_eq!(editor.owner(), main_h);
        drop(other);
        // SAFETY: as above.
        unsafe {
            assert_eq!(GetWindow(ed as _, GW_OWNER) as isize, main_h);
            assert_ne!(
                IsWindow(ed as _),
                0,
                "the editor's window survives its first owner"
            );
        }
        drop(editor);
        drop(main);
    }
}

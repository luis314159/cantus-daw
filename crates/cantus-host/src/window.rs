//! A plug-in's own editor in a native window of its own (ADR 0019).
//!
//! On Windows: a top-level window, owned by Cantus's main window so it stays above it, the
//! plug-in's view embedded as its child (CLAP `win32`, VST3 `HWND`). The window follows the
//! editor's size — the size the editor gives when it opens, and any resize it asks for —
//! and, when the editor can be resized, offers the user's resizing to it. winit's message
//! loop runs on the same thread and dispatches this window's messages with its own.
//!
//! A plug-in that can only float gets no window from us: it opens its own.
//! Elsewhere (macOS, Linux) there is no embedding yet and opening fails with
//! `Unsupported`.

use crate::backend::{EditorSize, HostError, MainThread};
use raw_window_handle::RawWindowHandle;

#[cfg(windows)]
pub use win32::PluginWindow;

#[cfg(not(windows))]
pub use other::PluginWindow;

#[cfg(not(windows))]
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
        CW_USEDEFAULT, GA_ROOT, GWLP_USERDATA, GWL_EXSTYLE, GWL_STYLE, GW_OWNER, HWND_TOP,
        IDC_ARROW, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER,
        SW_RESTORE, SW_SHOW, WM_CLOSE, WM_SIZE, WNDCLASSW, WS_CLIPCHILDREN, WS_EX_TOPMOST,
        WS_MAXIMIZEBOX, WS_OVERLAPPEDWINDOW, WS_THICKFRAME,
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
    }

    impl PluginWindow {
        /// Create the window, embed the editor in it, size it to the editor, show it.
        pub fn open(
            title: &str,
            owner: Option<RawWindowHandle>,
            main: &mut dyn MainThread,
        ) -> Result<PluginWindow, HostError> {
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
                let owner_hwnd: HWND = match owner {
                    Some(RawWindowHandle::Win32(h)) => h.hwnd.get() as HWND,
                    _ => std::ptr::null_mut(),
                };
                let style = WS_OVERLAPPEDWINDOW | WS_CLIPCHILDREN;
                let name = wide(title);
                let hwnd = CreateWindowExW(
                    0,
                    class.as_ptr(),
                    name.as_ptr(),
                    style,
                    CW_USEDEFAULT,
                    CW_USEDEFAULT,
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
                let mut window = PluginWindow {
                    hwnd,
                    shared,
                    style,
                    floating: false,
                };
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

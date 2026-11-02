//! A per-pixel-alpha window drawn from our own pixels (Windows; round 35, ADR 0091 §2): the
//! stage's companion window.
//!
//! The companion is drawn by `cantus-dance`'s renderer into a pixmap already; on Windows it
//! is shown as a **layered window** updated with `UpdateLayeredWindow(…, ULW_ALPHA)` from a
//! premultiplied BGRA DIB section — no OpenGL for that window, so its soft shadows, glows
//! and anti-aliased edges are true alpha on any GPU (a machine where one GPU draws and
//! another shows included), and the system leaves pixels of alpha 0 to the windows under it
//! (clicks there pass through, for free).
//!
//! The window lives on the thread that made it (the interface's: its messages are pumped
//! by the application's own event loop). What happens to it — moved, resized, minimised,
//! right-clicked, the pointer in and out, closed — is queued as [`WinEvent`]s, and a waker
//! given by the caller is called so its next frame reads them.

use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::{Arc, Mutex};
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, GetMonitorInfoW, MonitorFromRect,
    ScreenToClient, SelectObject, AC_SRC_ALPHA, AC_SRC_OVER, BITMAPINFO, BITMAPINFOHEADER, BI_RGB,
    BLENDFUNCTION, DIB_RGB_COLORS, HBITMAP, HDC, HGDIOBJ, MONITORINFO, MONITOR_DEFAULTTONEAREST,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::HiDpi::{GetDpiForMonitor, GetDpiForWindow, MDT_EFFECTIVE_DPI};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    TrackMouseEvent, TME_LEAVE, TME_NONCLIENT, TRACKMOUSEEVENT,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, GetWindowLongPtrW, GetWindowRect, IsIconic,
    LoadCursorW, RegisterClassExW, SetWindowLongPtrW, SetWindowPos, ShowWindow,
    UpdateLayeredWindow, GWLP_USERDATA, HTBOTTOMRIGHT, HTCAPTION, HTCLIENT, HWND_NOTOPMOST,
    HWND_TOPMOST, IDC_ARROW, SIZE_MINIMIZED, SIZE_RESTORED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
    SW_SHOWNOACTIVATE, ULW_ALPHA, WM_CLOSE, WM_DPICHANGED, WM_EXITSIZEMOVE, WM_MOUSEMOVE,
    WM_NCCALCSIZE, WM_NCHITTEST, WM_NCMOUSELEAVE, WM_NCMOUSEMOVE, WM_NCRBUTTONUP, WM_RBUTTONUP,
    WM_SIZE, WNDCLASSEXW, WS_EX_LAYERED, WS_EX_TOPMOST, WS_MINIMIZEBOX, WS_POPUP, WS_SYSMENU,
    WS_THICKFRAME,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    SetForegroundWindow, MINMAXINFO, WA_INACTIVE, WM_ACTIVATE, WM_ENTERSIZEMOVE, WM_GETMINMAXINFO,
    WM_KEYDOWN, WM_LBUTTONDOWN, WM_LBUTTONUP, WS_EX_TOOLWINDOW,
};

/// `WM_MOUSELEAVE` (in windows-sys's Controls module, a feature this crate does not need).
const WM_MOUSELEAVE: u32 = 0x02A3;

/// The window class every layered stage window shares (what OBS's window capture lists).
pub const CLASS: &str = "Cantus Stage";

/// What happened to the window since the last [`Layered::events`].
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum WinEvent {
    /// Moved or resized by the person (its outer rectangle, physical pixels).
    Placed { x: i32, y: i32, w: i32, h: i32 },
    /// Minimised (true) or shown again (false).
    Minimised(bool),
    /// The pointer came in or went out.
    Hover(bool),
    /// Right-clicked at this point of the screen (physical pixels).
    Menu { x: i32, y: i32 },
    /// Asked to close (Alt+F4, the taskbar).
    Close,
    /// Being resized by its corner: its outer size now (physical pixels). Until the
    /// person lets go (a [`WinEvent::Placed`] then), it is drawn at this size — drawn at
    /// any other, `UpdateLayeredWindow` would size it back under the pointer.
    Resizing { w: i32, h: i32 },
    /// A window of [`Layered::popup`]: the pointer moved to (`x`, `y`), its own pixels.
    Pointer { x: i32, y: i32 },
    /// A window of [`Layered::popup`]: the left button pressed or released there.
    Button { down: bool, x: i32, y: i32 },
    /// A window of [`Layered::popup`]: a key pressed (its virtual-key code).
    Key(u32),
    /// A window of [`Layered::popup`]: it became the active window, or stopped being it.
    Active(bool),
}

/// What the window procedure shares with its owner.
struct Shared {
    events: Mutex<Vec<WinEvent>>,
    waker: Mutex<Option<Box<dyn Fn() + Send + Sync>>>,
    movable: AtomicBool,
    resizable: AtomicBool,
    hover: AtomicBool,
    /// The resize grip's size at the bottom-right corner, physical pixels.
    grip: AtomicI32,
    /// A button in the window (client pixels: x, y, w, h) that a left click turns into a
    /// [`WinEvent::Menu`]; `w` 0 for none.
    button: [AtomicI32; 4],
    /// Between `WM_ENTERSIZEMOVE` and `WM_EXITSIZEMOVE`: the system is moving or sizing it.
    sizing: AtomicBool,
    /// Minimised, as last told (a `WM_SIZE` is queued only when this changes).
    minimised: AtomicBool,
    /// A popup's: its pointer, buttons, keys and activation are queued as events.
    client: AtomicBool,
    /// The least outer size the person can resize it to, physical pixels.
    min: [AtomicI32; 2],
}

impl Shared {
    fn on_button(&self, x: i32, y: i32) -> bool {
        let [bx, by, bw, bh] = std::array::from_fn(|i| self.button[i].load(Ordering::Relaxed));
        bw > 0 && x >= bx && y >= by && x < bx + bw && y < by + bh
    }

    /// The pointer is over the window: ask for its leaving, and say it came in once.
    fn track_hover(&self, hwnd: HWND, nonclient: bool) {
        if self.hover.swap(true, Ordering::Relaxed) {
            return;
        }
        let mut t = TRACKMOUSEEVENT {
            cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
            dwFlags: TME_LEAVE | if nonclient { TME_NONCLIENT } else { 0 },
            hwndTrack: hwnd,
            dwHoverTime: 0,
        };
        // SAFETY: a valid structure for our window.
        unsafe { TrackMouseEvent(&mut t) };
        self.push(WinEvent::Hover(true));
    }

    fn push(&self, e: WinEvent) {
        if let Ok(mut q) = self.events.lock() {
            q.push(e);
        }
        if let Ok(w) = self.waker.lock() {
            if let Some(w) = w.as_ref() {
                w();
            }
        }
    }
}

/// A layered window and the DIB it is drawn from.
pub struct Layered {
    hwnd: HWND,
    dc: HDC,
    dib: HBITMAP,
    old: HGDIOBJ,
    bits: *mut u8,
    size: (u32, u32),
    shared: Arc<Shared>,
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn lo(l: LPARAM) -> i32 {
    (l & 0xFFFF) as u16 as i16 as i32
}

fn hi(l: LPARAM) -> i32 {
    ((l >> 16) & 0xFFFF) as u16 as i16 as i32
}

fn shared_of(hwnd: HWND) -> Option<&'static Shared> {
    // SAFETY: the pointer was set from an `Arc<Shared>` the `Layered` keeps alive until it
    // destroys the window (which clears it first).
    let p = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *const Shared;
    (!p.is_null()).then(|| unsafe { &*p })
}

fn outer(hwnd: HWND) -> RECT {
    let mut r = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    // SAFETY: our window and a valid out-pointer.
    unsafe { GetWindowRect(hwnd, &mut r) };
    r
}

unsafe extern "system" fn proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    let Some(s) = shared_of(hwnd) else {
        // SAFETY: the default handling of a message to our window.
        return unsafe { DefWindowProcW(hwnd, msg, wp, lp) };
    };
    match msg {
        WM_NCHITTEST => {
            let r = outer(hwnd);
            let (x, y) = (lo(lp), hi(lp));
            let g = s.grip.load(Ordering::Relaxed);
            if s.on_button(x - r.left, y - r.top) {
                return HTCLIENT as LRESULT;
            }
            if s.resizable.load(Ordering::Relaxed) && x >= r.right - g && y >= r.bottom - g {
                return HTBOTTOMRIGHT as LRESULT;
            }
            if s.movable.load(Ordering::Relaxed) {
                return HTCAPTION as LRESULT;
            }
            HTCLIENT as LRESULT
        }
        WM_MOUSEMOVE if s.client.load(Ordering::Relaxed) => {
            s.track_hover(hwnd, false);
            s.push(WinEvent::Pointer {
                x: lo(lp),
                y: hi(lp),
            });
            0
        }
        WM_LBUTTONDOWN | WM_LBUTTONUP if s.client.load(Ordering::Relaxed) => {
            s.push(WinEvent::Button {
                down: msg == WM_LBUTTONDOWN,
                x: lo(lp),
                y: hi(lp),
            });
            0
        }
        WM_KEYDOWN if s.client.load(Ordering::Relaxed) => {
            s.push(WinEvent::Key(wp as u32));
            0
        }
        WM_ACTIVATE if s.client.load(Ordering::Relaxed) => {
            s.push(WinEvent::Active((wp & 0xFFFF) as u32 != WA_INACTIVE));
            0
        }
        WM_NCRBUTTONUP => {
            s.push(WinEvent::Menu {
                x: lo(lp),
                y: hi(lp),
            });
            0
        }
        WM_LBUTTONUP if s.on_button(lo(lp), hi(lp)) => {
            let mut p = POINT {
                x: lo(lp),
                y: hi(lp),
            };
            // SAFETY: client coordinates of our window to the screen's.
            unsafe { windows_sys::Win32::Graphics::Gdi::ClientToScreen(hwnd, &mut p) };
            s.push(WinEvent::Menu { x: p.x, y: p.y });
            0
        }
        WM_RBUTTONUP => {
            let mut p = POINT {
                x: lo(lp),
                y: hi(lp),
            };
            // SAFETY: client coordinates of our window to the screen's.
            unsafe { windows_sys::Win32::Graphics::Gdi::ClientToScreen(hwnd, &mut p) };
            s.push(WinEvent::Menu { x: p.x, y: p.y });
            0
        }
        WM_MOUSEMOVE | WM_NCMOUSEMOVE => {
            s.track_hover(hwnd, msg == WM_NCMOUSEMOVE);
            // SAFETY: the default handling continues (the system's caption drag).
            unsafe { DefWindowProcW(hwnd, msg, wp, lp) }
        }
        WM_MOUSELEAVE | WM_NCMOUSELEAVE => {
            if s.hover.swap(false, Ordering::Relaxed) {
                s.push(WinEvent::Hover(false));
            }
            0
        }
        // The whole window is its client (no frame is drawn: the bitmap is the window).
        WM_NCCALCSIZE if wp != 0 => 0,
        WM_GETMINMAXINFO => {
            // SAFETY: the system's structure for this message.
            let m = unsafe { &mut *(lp as *mut MINMAXINFO) };
            m.ptMinTrackSize.x = s.min[0].load(Ordering::Relaxed).max(1);
            m.ptMinTrackSize.y = s.min[1].load(Ordering::Relaxed).max(1);
            0
        }
        WM_ENTERSIZEMOVE => {
            s.sizing.store(true, Ordering::Relaxed);
            0
        }
        WM_EXITSIZEMOVE | WM_DPICHANGED => {
            if msg == WM_EXITSIZEMOVE {
                s.sizing.store(false, Ordering::Relaxed);
            }
            if msg == WM_DPICHANGED {
                // SAFETY: the system's suggested rectangle for the new DPI.
                let r = unsafe { &*(lp as *const RECT) };
                unsafe {
                    SetWindowPos(
                        hwnd,
                        std::ptr::null_mut(),
                        r.left,
                        r.top,
                        r.right - r.left,
                        r.bottom - r.top,
                        SWP_NOACTIVATE,
                    )
                };
            }
            let r = outer(hwnd);
            s.push(WinEvent::Placed {
                x: r.left,
                y: r.top,
                w: r.right - r.left,
                h: r.bottom - r.top,
            });
            0
        }
        WM_SIZE => {
            match wp as u32 {
                SIZE_MINIMIZED => {
                    if !s.minimised.swap(true, Ordering::Relaxed) {
                        s.push(WinEvent::Minimised(true));
                    }
                }
                SIZE_RESTORED => {
                    if s.minimised.swap(false, Ordering::Relaxed) {
                        s.push(WinEvent::Minimised(false));
                    } else if s.sizing.load(Ordering::Relaxed) {
                        // The client is the whole window (a popup without a frame).
                        s.push(WinEvent::Resizing {
                            w: lo(lp),
                            h: hi(lp),
                        });
                    }
                }
                _ => {}
            }
            0
        }
        WM_CLOSE => {
            s.push(WinEvent::Close);
            0
        }
        // SAFETY: everything else as any window.
        _ => unsafe { DefWindowProcW(hwnd, msg, wp, lp) },
    }
}

fn register() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let class = wide(CLASS);
        // SAFETY: a class description whose strings outlive the call (the class name is
        // copied by the system).
        unsafe {
            let wc = WNDCLASSEXW {
                cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                style: 0,
                lpfnWndProc: Some(proc),
                cbClsExtra: 0,
                cbWndExtra: 0,
                hInstance: GetModuleHandleW(std::ptr::null()),
                hIcon: std::ptr::null_mut(),
                hCursor: LoadCursorW(std::ptr::null_mut(), IDC_ARROW),
                hbrBackground: std::ptr::null_mut(),
                lpszMenuName: std::ptr::null(),
                lpszClassName: class.as_ptr(),
                hIconSm: std::ptr::null_mut(),
            };
            RegisterClassExW(&wc);
        }
    });
}

impl std::fmt::Debug for Layered {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Layered")
            .field("hwnd", &self.hwnd)
            .field("size", &self.size)
            .finish()
    }
}

impl Layered {
    /// Open a layered window titled `title` at (`x`, `y`), `w` × `h` physical pixels, on top
    /// of other windows or not. `None` if the system refuses.
    pub fn new(title: &str, x: i32, y: i32, w: i32, h: i32, topmost: bool) -> Option<Layered> {
        let ex = WS_EX_LAYERED | if topmost { WS_EX_TOPMOST } else { 0 };
        Self::create(title, (x, y, w, h), ex, false)
    }

    /// A popup (a menu): on top, not in the taskbar, never moved or resized by the person;
    /// its pointer, left button, keys and activation are queued as events for the caller
    /// to hand to what it draws. Call [`Layered::activate`] once it shows.
    pub fn popup(title: &str, x: i32, y: i32, w: i32, h: i32) -> Option<Layered> {
        let ex = WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TOOLWINDOW;
        let l = Self::create(title, (x, y, w, h), ex, true)?;
        l.set_interactive(false, false, 1);
        Some(l)
    }

    fn create(title: &str, r: (i32, i32, i32, i32), ex: u32, client: bool) -> Option<Layered> {
        let (x, y, w, h) = r;
        register();
        let shared = Arc::new(Shared {
            events: Mutex::new(Vec::new()),
            waker: Mutex::new(None),
            movable: AtomicBool::new(true),
            resizable: AtomicBool::new(true),
            hover: AtomicBool::new(false),
            grip: AtomicI32::new(24),
            button: std::array::from_fn(|_| AtomicI32::new(0)),
            sizing: AtomicBool::new(false),
            minimised: AtomicBool::new(false),
            client: AtomicBool::new(client),
            min: [AtomicI32::new(1), AtomicI32::new(1)],
        });
        let class = wide(CLASS);
        let name = wide(title);
        // Made per-monitor aware (v2) whatever the thread's default: its coordinates are
        // physical pixels, its scale its monitor's, and the system never stretches its
        // pixels (a window it thinks unaware is drawn blurred, and placed scaled twice).
        // Created at once where it goes: nothing it hears while being made is queued (its
        // shared state is attached after), so no stray placement can be read back.
        // SAFETY: a registered class, valid strings; the window belongs to this thread.
        let hwnd = per_monitor(|| unsafe {
            CreateWindowExW(
                ex,
                class.as_ptr(),
                name.as_ptr(),
                // A sizing frame, or the system never starts sizing it from its corner (no
                // frame is drawn: `WM_NCCALCSIZE` makes the whole window its client).
                WS_POPUP | WS_SYSMENU | WS_MINIMIZEBOX | if client { 0 } else { WS_THICKFRAME },
                x,
                y,
                w.max(1),
                h.max(1),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                GetModuleHandleW(std::ptr::null()),
                std::ptr::null(),
            )
        });
        if hwnd.is_null() {
            return None;
        }
        // SAFETY: the pointer stays valid while `Layered` lives (it clears it on drop).
        unsafe {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, Arc::as_ptr(&shared) as isize);
            // Its frame measured again now that the procedure answers: no frame at all.
            SetWindowPos(
                hwnd,
                std::ptr::null_mut(),
                0,
                0,
                0,
                0,
                SWP_NOMOVE
                    | SWP_NOSIZE
                    | SWP_NOACTIVATE
                    | windows_sys::Win32::UI::WindowsAndMessaging::SWP_NOZORDER
                    | windows_sys::Win32::UI::WindowsAndMessaging::SWP_FRAMECHANGED,
            );
        }
        // SAFETY: no device context is needed for a memory DC on the screen's format.
        let dc = unsafe { CreateCompatibleDC(std::ptr::null_mut()) };
        let mut l = Layered {
            hwnd,
            dc,
            dib: std::ptr::null_mut(),
            old: std::ptr::null_mut(),
            bits: std::ptr::null_mut(),
            size: (0, 0),
            shared,
        };
        l.allocate(w.max(1) as u32, h.max(1) as u32);
        // SAFETY: our window.
        unsafe { ShowWindow(hwnd, SW_SHOWNOACTIVATE) };
        Some(l)
    }

    fn allocate(&mut self, w: u32, h: u32) {
        if self.size == (w, h) && !self.bits.is_null() {
            return;
        }
        // SAFETY: the old bitmap (if any) is put back out of the DC before it is deleted;
        // the new one is a top-down 32-bit DIB whose bits the system allocates.
        unsafe {
            if !self.dib.is_null() {
                SelectObject(self.dc, self.old);
                DeleteObject(self.dib);
            }
            let mut info: BITMAPINFO = std::mem::zeroed();
            info.bmiHeader = BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w as i32,
                biHeight: -(h as i32),
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB,
                biSizeImage: 0,
                biXPelsPerMeter: 0,
                biYPelsPerMeter: 0,
                biClrUsed: 0,
                biClrImportant: 0,
            };
            let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
            self.dib = CreateDIBSection(
                self.dc,
                &info,
                DIB_RGB_COLORS,
                &mut bits,
                std::ptr::null_mut(),
                0,
            );
            self.bits = bits as *mut u8;
            self.old = SelectObject(self.dc, self.dib as HGDIOBJ);
        }
        self.size = (w, h);
    }

    /// Show `rgba` (premultiplied, `w` × `h`, tiny-skia's order) as the window's pixels; the
    /// window takes that size, its top-left corner where it is.
    pub fn present(&mut self, rgba: &[u8], w: u32, h: u32) -> bool {
        if rgba.len() < (w * h * 4) as usize || w == 0 || h == 0 {
            return false;
        }
        self.allocate(w, h);
        if self.bits.is_null() {
            return false;
        }
        // SAFETY: the DIB holds exactly w × h × 4 bytes (allocated for this size above).
        let dst = unsafe { std::slice::from_raw_parts_mut(self.bits, (w * h * 4) as usize) };
        for (d, s) in dst.chunks_exact_mut(4).zip(rgba.chunks_exact(4)) {
            d[0] = s[2];
            d[1] = s[1];
            d[2] = s[0];
            d[3] = s[3];
        }
        let size = SIZE {
            cx: w as i32,
            cy: h as i32,
        };
        let src = POINT { x: 0, y: 0 };
        let blend = BLENDFUNCTION {
            BlendOp: AC_SRC_OVER as u8,
            BlendFlags: 0,
            SourceConstantAlpha: 255,
            AlphaFormat: AC_SRC_ALPHA as u8,
        };
        // SAFETY: our window, our DC holding the DIB, valid structures.
        unsafe {
            UpdateLayeredWindow(
                self.hwnd,
                std::ptr::null_mut(),
                std::ptr::null(),
                &size,
                self.dc,
                &src,
                0,
                &blend,
                ULW_ALPHA,
            ) != 0
        }
    }

    /// What happened to the window since the last call.
    pub fn events(&self) -> Vec<WinEvent> {
        self.shared
            .events
            .lock()
            .map(|mut q| std::mem::take(&mut *q))
            .unwrap_or_default()
    }

    /// Called (on this thread) whenever an event is queued: the caller's next frame.
    pub fn set_waker(&self, f: Box<dyn Fn() + Send + Sync>) {
        if let Ok(mut w) = self.shared.waker.lock() {
            *w = Some(f);
        }
    }

    /// Whether the person may move it (by any opaque pixel) and resize it (by its corner).
    pub fn set_interactive(&self, movable: bool, resizable: bool, grip_px: i32) {
        self.shared.movable.store(movable, Ordering::Relaxed);
        self.shared.resizable.store(resizable, Ordering::Relaxed);
        self.shared.grip.store(grip_px.max(1), Ordering::Relaxed);
    }

    /// The least size the person can resize it to by its corner, physical pixels.
    pub fn set_min_size(&self, w: i32, h: i32) {
        self.shared.min[0].store(w.max(1), Ordering::Relaxed);
        self.shared.min[1].store(h.max(1), Ordering::Relaxed);
    }

    /// A button (client pixels) a left click on turns into a menu event; `None`: none.
    pub fn set_button(&self, rect: Option<(i32, i32, i32, i32)>) {
        let (x, y, w, h) = rect.unwrap_or((0, 0, 0, 0));
        for (a, v) in self.shared.button.iter().zip([x, y, w, h]) {
            a.store(v, Ordering::Relaxed);
        }
    }

    pub fn set_topmost(&self, on: bool) {
        let after = if on { HWND_TOPMOST } else { HWND_NOTOPMOST };
        // SAFETY: our window; position and size kept.
        unsafe {
            SetWindowPos(
                self.hwnd,
                after,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            )
        };
    }

    /// Put its top-left corner at (`x`, `y`), physical pixels.
    pub fn move_to(&self, x: i32, y: i32) {
        // SAFETY: our window; its size and order kept.
        per_monitor(|| unsafe {
            SetWindowPos(
                self.hwnd,
                std::ptr::null_mut(),
                x,
                y,
                0,
                0,
                SWP_NOSIZE
                    | windows_sys::Win32::UI::WindowsAndMessaging::SWP_NOZORDER
                    | SWP_NOACTIVATE,
            )
        });
    }

    /// Its outer rectangle, physical pixels: x, y, width, height.
    pub fn rect(&self) -> (i32, i32, i32, i32) {
        // Read per-monitor aware: a thread of another awareness is told scaled numbers.
        let r = per_monitor(|| outer(self.hwnd));
        (r.left, r.top, r.right - r.left, r.bottom - r.top)
    }

    /// Physical pixels per logical pixel of the monitor it is on (1.0 at 96 DPI).
    pub fn scale(&self) -> f32 {
        // SAFETY: our window.
        let dpi = unsafe { GetDpiForWindow(self.hwnd) };
        if dpi == 0 {
            1.0
        } else {
            dpi as f32 / 96.0
        }
    }

    /// Being moved or resized by the person right now (the system's sizing loop).
    pub fn is_sizing(&self) -> bool {
        self.shared.sizing.load(Ordering::Relaxed)
    }

    /// Make it the active window (a popup, to have the keyboard and to hear when another
    /// window is chosen). The system allows it while the person's last input was ours.
    pub fn activate(&self) -> bool {
        // SAFETY: our window.
        unsafe { SetForegroundWindow(self.hwnd) != 0 }
    }

    pub fn is_minimised(&self) -> bool {
        // SAFETY: our window.
        unsafe { IsIconic(self.hwnd) != 0 }
    }

    pub fn hovered(&self) -> bool {
        self.shared.hover.load(Ordering::Relaxed)
    }

    /// Where the pointer is, in the window's own pixels (`None` when outside it).
    pub fn pointer(&self) -> Option<(i32, i32)> {
        let mut p = POINT { x: 0, y: 0 };
        // SAFETY: valid out-pointers; our window.
        per_monitor(|| unsafe {
            windows_sys::Win32::UI::WindowsAndMessaging::GetCursorPos(&mut p);
            ScreenToClient(self.hwnd, &mut p);
        });
        let (w, h) = self.size;
        (p.x >= 0 && p.y >= 0 && (p.x as u32) < w && (p.y as u32) < h).then_some((p.x, p.y))
    }
}

impl Drop for Layered {
    fn drop(&mut self) {
        // SAFETY: the window procedure's pointer is cleared before the window and the GDI
        // objects it used go.
        unsafe {
            SetWindowLongPtrW(self.hwnd, GWLP_USERDATA, 0);
            DestroyWindow(self.hwnd);
            if !self.dib.is_null() {
                SelectObject(self.dc, self.old);
                DeleteObject(self.dib);
            }
            DeleteDC(self.dc);
        }
    }
}

/// `f` run with this thread per-monitor aware (v2), its previous awareness put back: the
/// coordinates it reads and writes are the screen's physical pixels.
fn per_monitor<T>(f: impl FnOnce() -> T) -> T {
    // SAFETY: a documented context constant; the thread's previous one is put back.
    let previous = unsafe {
        windows_sys::Win32::UI::HiDpi::SetThreadDpiAwarenessContext(
            windows_sys::Win32::UI::HiDpi::DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
        )
    };
    let out = f();
    if !previous.is_null() {
        // SAFETY: the context this thread had before.
        unsafe { windows_sys::Win32::UI::HiDpi::SetThreadDpiAwarenessContext(previous) };
    }
    out
}

type Monitor = (MONITORINFO, windows_sys::Win32::Graphics::Gdi::HMONITOR);

fn monitor_info(r: (i32, i32, i32, i32)) -> Option<Monitor> {
    let rect = RECT {
        left: r.0,
        top: r.1,
        right: r.0 + r.2.max(1),
        bottom: r.1 + r.3.max(1),
    };
    per_monitor(|| {
        // SAFETY: a valid rectangle; the nearest monitor is always one.
        let mon = unsafe { MonitorFromRect(&rect, MONITOR_DEFAULTTONEAREST) };
        if mon.is_null() {
            return None;
        }
        // SAFETY: a plain structure, its size set as the call requires.
        let mut info: MONITORINFO = unsafe { std::mem::zeroed() };
        info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
        // SAFETY: a monitor handle and a valid out-pointer.
        (unsafe { GetMonitorInfoW(mon, &mut info) } != 0).then_some((info, mon))
    })
}

/// The work area (the monitor less the taskbar) of the monitor `r` is most on — the nearest
/// one when it is on none — in physical pixels: x, y, width, height.
pub fn work_area_at(r: (i32, i32, i32, i32)) -> Option<(i32, i32, i32, i32)> {
    monitor_info(r).map(|(i, _)| {
        let a = i.rcWork;
        (a.left, a.top, a.right - a.left, a.bottom - a.top)
    })
}

/// Physical pixels per logical pixel of the monitor `r` is most on (1.0 at 96 DPI).
pub fn scale_at(r: (i32, i32, i32, i32)) -> f32 {
    let Some((_, mon)) = monitor_info(r) else {
        return 1.0;
    };
    let (mut dx, mut dy) = (0u32, 0u32);
    // SAFETY: a monitor handle and valid out-pointers. The DPI read is the monitor's own
    // whatever this thread's awareness (per-monitor v2 in `Layered::new`).
    let ok = unsafe { GetDpiForMonitor(mon, MDT_EFFECTIVE_DPI, &mut dx, &mut dy) } == 0;
    if ok && dx > 0 {
        dx as f32 / 96.0
    } else {
        1.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows_sys::Win32::UI::WindowsAndMessaging::{SendMessageW, SIZE_RESTORED};

    fn send(l: &Layered, msg: u32, wp: usize, lp: isize) -> isize {
        // SAFETY: our window, on this thread; the messages carry no pointers.
        unsafe { SendMessageW(l.hwnd, msg, wp, lp) }
    }

    fn lparam(x: i32, y: i32) -> isize {
        ((y as u16 as isize) << 16) | x as u16 as isize
    }

    fn resize(l: &Layered, w: i32, h: i32) {
        // SAFETY: our window; its place kept. Physical pixels, as the application's thread
        // (made per-monitor aware by winit) speaks.
        per_monitor(|| unsafe {
            SetWindowPos(
                l.hwnd,
                std::ptr::null_mut(),
                0,
                0,
                w,
                h,
                SWP_NOMOVE | SWP_NOACTIVATE,
            )
        });
    }

    fn opaque(w: u32, h: u32) -> Vec<u8> {
        vec![255; (w * h * 4) as usize]
    }

    /// The snap-back the review found (round 35): sized by its corner, a layered window
    /// presented at its old size is sized back to it. Presented at the size the sizing
    /// loop gives it, it keeps it — and that size is what it is placed at when let go.
    #[test]
    fn being_resized_it_is_drawn_at_the_system_s_size_and_kept_there() {
        let mut l = Layered::new("Cantus test", 120, 140, 300, 200, false).expect("a window");
        assert!(l.present(&opaque(300, 200), 300, 200));
        l.events();
        send(&l, WM_ENTERSIZEMOVE, 0, 0);
        assert!(l.is_sizing());
        resize(&l, 380, 260);
        let e = l.events();
        assert!(
            e.contains(&WinEvent::Resizing { w: 380, h: 260 }),
            "the sizing loop's new size is told: {e:?} {:?}",
            l.rect()
        );
        let (_, _, w, h) = l.rect();
        assert!(l.present(&opaque(w as u32, h as u32), w as u32, h as u32));
        assert_eq!((l.rect().2, l.rect().3), (380, 260), "kept");
        // What the companion used to do: present at the stored size — sized back.
        assert!(l.present(&opaque(300, 200), 300, 200));
        assert_eq!((l.rect().2, l.rect().3), (300, 200), "the snap-back");
        resize(&l, 400, 280);
        send(&l, WM_EXITSIZEMOVE, 0, 0);
        assert!(!l.is_sizing());
        assert!(l.events().contains(&WinEvent::Placed {
            x: 120,
            y: 140,
            w: 400,
            h: 280
        }));
    }

    #[test]
    fn a_plain_resize_is_no_restore_and_a_minimise_is_told_once() {
        let l = Layered::new("Cantus test", 100, 100, 200, 200, false).expect("a window");
        l.events();
        // WM_SIZE with SIZE_RESTORED comes with every size change: not a restore.
        send(&l, WM_SIZE, SIZE_RESTORED as usize, lparam(220, 240));
        assert!(l.events().is_empty());
        send(&l, WM_SIZE, SIZE_MINIMIZED as usize, 0);
        send(&l, WM_SIZE, SIZE_MINIMIZED as usize, 0);
        send(&l, WM_SIZE, SIZE_RESTORED as usize, lparam(200, 200));
        assert_eq!(
            l.events(),
            vec![WinEvent::Minimised(true), WinEvent::Minimised(false)]
        );
    }

    #[test]
    fn a_popup_hears_its_pointer_buttons_keys_and_activation_the_companion_does_not() {
        let p = Layered::popup("Cantus test menu", 100, 100, 200, 300).expect("a popup");
        p.events();
        send(&p, WM_MOUSEMOVE, 0, lparam(10, 20));
        send(&p, WM_LBUTTONDOWN, 1, lparam(10, 20));
        send(&p, WM_LBUTTONUP, 0, lparam(11, 20));
        send(&p, WM_KEYDOWN, 0x1B, 0);
        send(&p, WM_ACTIVATE, WA_INACTIVE as usize, 0);
        let e = p.events();
        for want in [
            WinEvent::Pointer { x: 10, y: 20 },
            WinEvent::Button {
                down: true,
                x: 10,
                y: 20,
            },
            WinEvent::Button {
                down: false,
                x: 11,
                y: 20,
            },
            WinEvent::Key(0x1B),
            WinEvent::Active(false),
        ] {
            assert!(e.contains(&want), "{want:?} in {e:?}");
        }
        let c = Layered::new("Cantus test", 100, 100, 200, 200, false).expect("a window");
        c.events();
        send(&c, WM_MOUSEMOVE, 0, lparam(10, 20));
        send(&c, WM_KEYDOWN, 0x1B, 0);
        assert!(c
            .events()
            .iter()
            .all(|e| !matches!(e, WinEvent::Pointer { .. } | WinEvent::Key(_))));
    }

    /// The companion has a sizing frame (or the system never sizes it from its corner) but
    /// none is drawn: the whole window is its client.
    #[test]
    fn the_companion_is_sizable_and_all_client() {
        use windows_sys::Win32::UI::WindowsAndMessaging::{GetClientRect, GWL_STYLE};
        let l = Layered::new("Cantus test", 100, 100, 300, 200, false).expect("a window");
        // SAFETY: our window.
        let style = unsafe { GetWindowLongPtrW(l.hwnd, GWL_STYLE) } as u32;
        assert_ne!(style & WS_THICKFRAME, 0);
        let mut c = RECT {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        // SAFETY: our window and a valid out-pointer.
        per_monitor(|| unsafe { GetClientRect(l.hwnd, &mut c) });
        let (_, _, w, h) = l.rect();
        assert_eq!((c.right - c.left, c.bottom - c.top), (w, h));
    }

    #[test]
    fn the_least_size_is_what_the_system_is_told() {
        let l = Layered::new("Cantus test", 100, 100, 400, 400, false).expect("a window");
        l.set_min_size(300, 375);
        // SAFETY: a plain structure.
        let mut m: MINMAXINFO = unsafe { std::mem::zeroed() };
        send(&l, WM_GETMINMAXINFO, 0, &mut m as *mut MINMAXINFO as isize);
        assert_eq!((m.ptMinTrackSize.x, m.ptMinTrackSize.y), (300, 375));
    }

    #[test]
    fn a_rectangle_anywhere_has_a_monitor_and_is_fitted_onto_it() {
        let far = (100_000, -80_000, 400, 450);
        let area = work_area_at(far).expect("the nearest monitor");
        assert!(area.2 > 0 && area.3 > 0);
        let (x, y, w, h) = crate::window::fit_in(far, area, 64);
        assert!(x >= area.0 && y >= area.1 && x + w <= area.0 + area.2 && y + h <= area.1 + area.3);
        let s = scale_at(far);
        assert!((0.5..=5.0).contains(&s), "{s}");
    }
}

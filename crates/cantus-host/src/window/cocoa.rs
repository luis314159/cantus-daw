//! macOS: a plug-in's editor in an `NSWindow` of ours (ADR 0019, ADR 0066). The window's
//! content view is the parent handed to the plug-in (CLAP `cocoa`, VST3 `NSView`); the
//! plug-in adds its own `NSView` inside it. The window follows the editor's size (in points,
//! as both formats give it on macOS) and offers the user's resizing to a resizable editor.
//!
//! Above Cantus: a window with an owner floats (`NSFloatingWindowLevel`) and hides while
//! Cantus is not the active application, as the plug-in windows of Logic and Live do.
//! In the plug-in sandbox there is no owner: the helper's windows are ordinary ones of a
//! background application (`platform.rs` makes it one).
//!
//! AppKit is main-thread-only: every call here runs on the main thread (Cantus's UI
//! thread, the helper's main loop), which `MainThreadMarker` checks.

use super::{EditorSize, HostError, MainThread, RawWindowHandle};
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject};
use objc2::{define_class, msg_send, DefinedClass, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSApplication, NSBackingStoreType, NSFloatingWindowLevel, NSView, NSWindow, NSWindowStyleMask,
};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};
use raw_window_handle::AppKitWindowHandle;
use std::cell::Cell;
use std::ptr::NonNull;

define_class!(
    // SAFETY: a plain NSObject subclass whose one instance variable is a `Cell`; no Drop.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "CantusPluginWindowDelegate"]
    #[ivars = Cell<bool>]
    struct Delegate;

    impl Delegate {
        /// The user closed the window (its close button, ⌘W): the only "closed" there is.
        /// Hiding — Cantus deactivated, ⌘H, minimising — sends no such message.
        #[unsafe(method(windowWillClose:))]
        fn window_will_close(&self, _note: *mut AnyObject) {
            self.ivars().set(true);
        }
    }
);

impl Delegate {
    fn new(mtm: MainThreadMarker) -> Retained<Delegate> {
        let this = Delegate::alloc(mtm).set_ivars(Cell::new(false));
        // SAFETY: NSObject's designated initialiser.
        unsafe { msg_send![super(this), init] }
    }
}

pub struct PluginWindow {
    window: Retained<NSWindow>,
    /// Says when the user closed the window (the window holds it weakly; this keeps it).
    delegate: Retained<Delegate>,
    /// The size last given to the editor (the content view's, in points).
    size: EditorSize,
    /// The plug-in floats a window of its own; ours stays hidden and is never shown.
    floating: bool,
}

fn main_thread() -> Result<MainThreadMarker, HostError> {
    MainThreadMarker::new().ok_or_else(|| HostError::Editor("not on the main thread".into()))
}

fn style(resizable: bool) -> NSWindowStyleMask {
    let mut s =
        NSWindowStyleMask::Titled | NSWindowStyleMask::Closable | NSWindowStyleMask::Miniaturizable;
    if resizable {
        s |= NSWindowStyleMask::Resizable;
    }
    s
}

impl PluginWindow {
    /// Ours for a plug-in whose editor floats a window of its own that opened later than
    /// asked: hidden, never shown — it stands for theirs, which closing ours closes.
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
        let view = window
            .window
            .contentView()
            .ok_or_else(|| HostError::Editor("the window has no content view".into()))?;
        let ptr = NonNull::from(&*view).cast::<std::ffi::c_void>();
        let parent = RawWindowHandle::AppKit(AppKitWindowHandle::new(ptr));
        let size = main.open_editor(parent)?;
        if main.editor_is_floating() {
            window.floating = true;
            return Ok(window);
        }
        window.window.setStyleMask(style(main.editor_can_resize()));
        window.fit(size);
        window.window.center();
        window.window.makeKeyAndOrderFront(None);
        Ok(window)
    }

    /// The window, hidden, with an empty content view.
    fn create(title: &str, owner: Option<RawWindowHandle>) -> Result<PluginWindow, HostError> {
        let mtm = main_thread()?;
        let rect = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(640.0, 480.0));
        // SAFETY: a plain window made on the main thread; it is not released on close (we
        // keep the only strong reference and close it in `drop`).
        let window = unsafe {
            let w = NSWindow::initWithContentRect_styleMask_backing_defer(
                mtm.alloc::<NSWindow>(),
                rect,
                style(true),
                NSBackingStoreType::Buffered,
                false,
            );
            w.setReleasedWhenClosed(false);
            w
        };
        window.setTitle(&NSString::from_str(title));
        // A plain view as the parent: the plug-in's view goes inside it.
        let content = NSView::initWithFrame(mtm.alloc::<NSView>(), rect);
        window.setContentView(Some(&content));
        if matches!(owner, Some(RawWindowHandle::AppKit(_))) {
            // Above Cantus's windows, and out of the way while another application is used.
            window.setLevel(NSFloatingWindowLevel);
            window.setHidesOnDeactivate(true);
        }
        let delegate = Delegate::new(mtm);
        // SAFETY: an object answering `windowWillClose:`, kept alive by the value it is
        // stored in and taken off the window in `drop` before the window closes.
        unsafe {
            let _: () = msg_send![&*window, setDelegate: &*delegate];
        }
        Ok(PluginWindow {
            window,
            delegate,
            size: EditorSize {
                width: 640,
                height: 480,
            },
            floating: false,
        })
    }

    /// Size the window so its content view is `size` (points).
    pub fn fit(&mut self, size: EditorSize) {
        if size.width == 0 || size.height == 0 {
            return;
        }
        self.window
            .setContentSize(NSSize::new(f64::from(size.width), f64::from(size.height)));
        self.size = size;
    }

    pub fn focus(&self) {
        if self.floating {
            return;
        }
        self.window.makeKeyAndOrderFront(None);
        if let Some(mtm) = MainThreadMarker::new() {
            #[allow(deprecated)]
            NSApplication::sharedApplication(mtm).activateIgnoringOtherApps(true);
        }
    }

    pub fn is_minimised(&self) -> bool {
        self.window.isMiniaturized()
    }

    pub fn is_floating(&self) -> bool {
        self.floating
    }

    pub fn is_frontmost(&self) -> bool {
        self.window.isKeyWindow()
    }

    /// The user closed it — told by the window's delegate. A window that is merely not
    /// visible is not closed: one with an owner hides while Cantus is not the active
    /// application (`setHidesOnDeactivate`), and ⌘H and minimising hide it too.
    pub fn closed(&self) -> bool {
        !self.floating && self.delegate.ivars().get()
    }

    /// A window with no editor in it (the tests of the window itself), shown.
    #[doc(hidden)]
    pub fn empty(title: &str, owner: Option<RawWindowHandle>) -> Result<PluginWindow, HostError> {
        let w = Self::create(title, owner)?;
        w.window.makeKeyAndOrderFront(None);
        Ok(w)
    }

    /// Hide it as macOS does when Cantus is deactivated (the tests).
    #[doc(hidden)]
    pub fn hide(&self) {
        self.window.orderOut(None);
    }

    /// Close it as its close button does (the tests).
    #[doc(hidden)]
    pub fn close_as_the_user(&self) {
        self.window.performClose(None);
    }

    /// A size the user dragged the window to, once (points).
    pub fn take_resize(&mut self) -> Option<EditorSize> {
        if self.floating {
            return None;
        }
        let frame = self.window.contentView()?.frame();
        let now = EditorSize {
            width: frame.size.width.round().max(1.0) as u32,
            height: frame.size.height.round().max(1.0) as u32,
        };
        (now != self.size).then(|| {
            self.size = now;
            now
        })
    }

    /// The `NSWindow`'s address, as a number (what Windows gives as its `HWND`).
    pub fn handle(&self) -> Option<isize> {
        Some(Retained::as_ptr(&self.window) as isize)
    }
}

impl Drop for PluginWindow {
    fn drop(&mut self) {
        // SAFETY: nil takes the delegate off before the window is closed here.
        unsafe {
            let _: () = msg_send![&*self.window, setDelegate: std::ptr::null_mut::<AnyObject>()];
        }
        self.window.orderOut(None);
        self.window.setContentView(None);
        self.window.close();
    }
}

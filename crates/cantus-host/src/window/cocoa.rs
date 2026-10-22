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
use objc2::MainThreadMarker;
use objc2_app_kit::{
    NSApplication, NSBackingStoreType, NSFloatingWindowLevel, NSView, NSWindow, NSWindowStyleMask,
};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};
use raw_window_handle::AppKitWindowHandle;
use std::ptr::NonNull;

pub struct PluginWindow {
    window: Retained<NSWindow>,
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
        Ok(PluginWindow {
            window,
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

    /// The user closed it (the close button orders it out; a minimised window is not
    /// closed, though it is not visible either).
    pub fn closed(&self) -> bool {
        !self.floating && !self.window.isVisible() && !self.window.isMiniaturized()
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
        self.window.orderOut(None);
        self.window.setContentView(None);
        self.window.close();
    }
}

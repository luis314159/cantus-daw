//! The plug-in window on macOS (ADR 0066): hidden is not closed. A window with an owner
//! hides whenever Cantus is not the active application (and on ⌘H); the session must not
//! take that for the user closing it and destroy the editor. Only the close button (or ⌘W)
//! closes it.
//!
//! AppKit runs on the main thread, and the test harness runs tests on others, so this test
//! has no harness (`harness = false`): `main` is the test, on the main thread.

fn main() {
    #[cfg(target_os = "macos")]
    mac::hidden_is_not_closed();
    println!("cocoa_window: ok");
}

#[cfg(target_os = "macos")]
mod mac {
    use cantus_host::window::PluginWindow;
    use objc2::MainThreadMarker;
    use objc2_app_kit::NSApplication;
    use raw_window_handle::{AppKitWindowHandle, RawWindowHandle};
    use std::ptr::NonNull;

    pub fn hidden_is_not_closed() {
        let mtm = MainThreadMarker::new().expect("main runs on the main thread");
        let _app = NSApplication::sharedApplication(mtm);
        // An owner, as Cantus always passes one (its window's view); only its kind is read.
        let owner = RawWindowHandle::AppKit(AppKitWindowHandle::new(NonNull::dangling()));
        let w = PluginWindow::empty("Cantus test", Some(owner)).expect("a window");
        assert!(!w.closed(), "just opened");
        w.hide();
        assert!(!w.closed(), "hidden (Cantus deactivated, ⌘H) is not closed");
        w.focus();
        w.close_as_the_user();
        assert!(w.closed(), "the close button closes it");
        drop(w);
    }
}

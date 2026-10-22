//! Files macOS asks Cantus to open (round 30, ADR 0066): a `.cantus` double-clicked in
//! Finder, dropped on the Dock icon, or chosen in *Open With*. macOS sends them as an
//! Apple Event (`kAEOpenDocuments`), not as arguments; winit owns the application's
//! delegate and does not pass the event on, so this installs a handler of its own — in
//! `NSApplicationWillFinishLaunchingNotification`, the moment Apple documents for it (after
//! AppKit's default handlers, before the launch's own event is processed).
//!
//! The paths go into a queue every system has ([`push`], [`take`]): the interface takes a
//! project from it at start (`cantus_ui::run`) and each frame after that, through the same
//! *Save changes?* question as *Open*.

use std::path::PathBuf;
use std::sync::Mutex;

/// What to call when paths arrive (the interface asks for a frame).
type Wake = Box<dyn Fn() + Send + Sync>;

static QUEUE: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());
static WAKE: Mutex<Option<Wake>> = Mutex::new(None);

/// Paths asked to be opened (the Apple Event's handler; anything else that delivers files).
pub fn push(paths: impl IntoIterator<Item = PathBuf>) {
    let mut q = QUEUE.lock().unwrap_or_else(|e| e.into_inner());
    let before = q.len();
    q.extend(paths);
    let grew = q.len() > before;
    drop(q);
    if grew {
        if let Some(wake) = WAKE.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
            wake();
        }
    }
}

/// The paths asked for since the last call, oldest first.
pub fn take() -> Vec<PathBuf> {
    std::mem::take(&mut *QUEUE.lock().unwrap_or_else(|e| e.into_inner()))
}

/// Call `wake` whenever paths arrive (replacing the one before).
pub fn set_wake(wake: Wake) {
    *WAKE.lock().unwrap_or_else(|e| e.into_inner()) = Some(wake);
}

/// Listen for the open-documents event. Call once, on the main thread, before the event
/// loop runs. Nothing off macOS.
pub fn install() {
    #[cfg(target_os = "macos")]
    imp::install();
}

#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
mod imp {
    use objc2::rc::Retained;
    use objc2::runtime::{AnyClass, AnyObject, NSObject};
    use objc2::{define_class, msg_send, sel, AnyThread};
    use objc2_foundation::NSString;
    use std::path::PathBuf;

    /// `kCoreEventClass` ('aevt') and `kAEOpenDocuments` ('odoc').
    const CORE_EVENT_CLASS: u32 = u32::from_be_bytes(*b"aevt");
    const OPEN_DOCUMENTS: u32 = u32::from_be_bytes(*b"odoc");
    /// `keyDirectObject` ('----'): the event's list of files.
    const DIRECT_OBJECT: u32 = u32::from_be_bytes(*b"----");

    define_class!(
        // SAFETY: a plain NSObject subclass with no instance variables and no Drop.
        #[unsafe(super(NSObject))]
        #[name = "CantusOpenDocuments"]
        struct Handler;

        impl Handler {
            /// AppKit's default handlers are in place: take the open-documents event over.
            #[unsafe(method(willFinishLaunching:))]
            fn will_finish_launching(&self, _note: *mut AnyObject) {
                let Some(manager) = AnyClass::get(c"NSAppleEventManager") else {
                    return;
                };
                // SAFETY: the shared manager; `self` lives for the whole process (leaked in
                // `install`) and answers the selector below with the right signature.
                unsafe {
                    let m: *mut AnyObject = msg_send![manager, sharedAppleEventManager];
                    if m.is_null() {
                        return;
                    }
                    let _: () = msg_send![
                        m,
                        setEventHandler: self,
                        andSelector: sel!(openDocuments:withReply:),
                        forEventClass: CORE_EVENT_CLASS,
                        andEventID: OPEN_DOCUMENTS
                    ];
                }
            }

            /// The files of an open-documents event, queued for the interface.
            #[unsafe(method(openDocuments:withReply:))]
            fn open_documents(&self, event: *mut AnyObject, _reply: *mut AnyObject) {
                // SAFETY: AppKit hands a live `NSAppleEventDescriptor`; every object read
                // here is autoreleased and used only within this call.
                let paths = unsafe { files(event) };
                super::push(paths);
            }
        }
    );

    /// The path of a descriptor that is a file (`fileURL`, macOS 10.11+).
    unsafe fn path_of(d: *mut AnyObject) -> Option<PathBuf> {
        if d.is_null() {
            return None;
        }
        // SAFETY: `d` is a live descriptor; `fileURL` and `path` return autoreleased
        // objects or nil.
        unsafe {
            let url: *mut AnyObject = msg_send![d, fileURL];
            if url.is_null() {
                return None;
            }
            let path: *mut NSString = msg_send![url, path];
            path.as_ref().map(|p| PathBuf::from(p.to_string()))
        }
    }

    unsafe fn files(event: *mut AnyObject) -> Vec<PathBuf> {
        if event.is_null() {
            return Vec::new();
        }
        // SAFETY: as in `open_documents`; list indices are 1-based.
        unsafe {
            let list: *mut AnyObject = msg_send![event, paramDescriptorForKeyword: DIRECT_OBJECT];
            if list.is_null() {
                return Vec::new();
            }
            let n: isize = msg_send![list, numberOfItems];
            if n <= 0 {
                // One file, not in a list.
                return path_of(list).into_iter().collect();
            }
            (1..=n)
                .filter_map(|i| {
                    let d: *mut AnyObject = msg_send![list, descriptorAtIndex: i];
                    path_of(d)
                })
                .collect()
        }
    }

    pub fn install() {
        let Some(center_class) = AnyClass::get(c"NSNotificationCenter") else {
            return;
        };
        // SAFETY: NSObject's designated initialiser.
        let handler: Retained<Handler> = unsafe { msg_send![Handler::alloc(), init] };
        let name = NSString::from_str("NSApplicationWillFinishLaunchingNotification");
        // SAFETY: the default centre; the observer is never removed and is leaked below, so
        // it outlives every notification it can receive.
        unsafe {
            let center: *mut AnyObject = msg_send![center_class, defaultCenter];
            if center.is_null() {
                return;
            }
            let _: () = msg_send![
                center,
                addObserver: &*handler,
                selector: sel!(willFinishLaunching:),
                name: &*name,
                object: std::ptr::null_mut::<AnyObject>()
            ];
        }
        std::mem::forget(handler);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    /// What arrives is taken once, in order, and wakes the interface.
    #[test]
    fn opened_paths_queue_in_order_and_wake_the_interface() {
        let woken = Arc::new(AtomicUsize::new(0));
        let w = woken.clone();
        set_wake(Box::new(move || {
            w.fetch_add(1, Ordering::SeqCst);
        }));
        let _ = take();
        push([
            PathBuf::from("/a/One.cantus"),
            PathBuf::from("/b/Two.cantus"),
        ]);
        push(Vec::new());
        push([PathBuf::from("/c/Three.cantus")]);
        assert_eq!(
            woken.load(Ordering::SeqCst),
            2,
            "an empty push wakes nothing"
        );
        assert_eq!(
            take(),
            ["/a/One.cantus", "/b/Two.cantus", "/c/Three.cantus"].map(PathBuf::from)
        );
        assert!(take().is_empty(), "taken once");
        install();
    }
}

//! The helper's main thread on macOS (ADR 0066): an AppKit application of its own — a
//! background one (`Accessory`: no Dock icon, no menu bar of its own) whose windows are
//! the plug-ins' editors — and a main-loop wait that dispatches AppKit's events while it
//! waits, so an editor draws, takes the mouse and resizes between Cantus's calls. A ring
//! from another thread posts an application-defined event, which ends the wait at once.

use objc2::rc::{autoreleasepool, Retained};
use objc2::runtime::AnyObject;
use objc2::{msg_send, MainThreadMarker};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSEvent, NSEventMask, NSEventModifierFlags,
    NSEventType,
};
use objc2_foundation::{
    NSActivityOptions, NSDate, NSDefaultRunLoopMode, NSPoint, NSProcessInfo, NSString,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// The application object, made on the main thread before any plug-in loads — and kept
/// out of App Nap for the process's life (round 31).
pub fn init_app() {
    if let Some(mtm) = MainThreadMarker::new() {
        let app = NSApplication::sharedApplication(mtm);
        app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
        app.finishLaunching();
    }
    no_app_nap();
}

/// A helper is an application with no window on screen most of the time that plays no
/// sound of its own (its audio goes to Cantus through shared memory): just what macOS puts
/// in App Nap — its timers coalesced and late, its threads at a background priority, while
/// it processes Cantus's audio. A latency-critical, user-initiated activity for the
/// process's whole life keeps it awake, as Apple's guide asks of real-time audio work; the
/// system may still sleep when idle.
fn no_app_nap() {
    let reason = NSString::from_str("Cantus's plug-in sandbox processes audio in real time");
    let activity = NSProcessInfo::processInfo().beginActivityWithOptions_reason(
        NSActivityOptions::UserInitiatedAllowingIdleSystemSleep
            | NSActivityOptions::LatencyCritical,
        &reason,
    );
    // Held until the process ends: never ended.
    std::mem::forget(activity);
}

struct Shared {
    rung: AtomicBool,
    /// `NSApp`, as a number (it lives for the whole process); 0 off the main thread.
    app: usize,
}

/// Wakes the main thread's wait from another thread (a request arrived).
#[derive(Clone, Copy)]
pub struct Wake(&'static Shared);

impl Wake {
    pub fn new() -> Wake {
        let app = MainThreadMarker::new().map_or(0, |mtm| {
            Retained::as_ptr(&NSApplication::sharedApplication(mtm)) as usize
        });
        Wake(Box::leak(Box::new(Shared {
            rung: AtomicBool::new(false),
            app,
        })))
    }

    pub fn ring(self) {
        self.0.rung.store(true, Ordering::SeqCst);
        if self.0.app == 0 {
            return;
        }
        autoreleasepool(|_| {
            let event = NSEvent::otherEventWithType_location_modifierFlags_timestamp_windowNumber_context_subtype_data1_data2(
                NSEventType::ApplicationDefined,
                NSPoint::new(0.0, 0.0),
                NSEventModifierFlags::empty(),
                0.0,
                0,
                None,
                0,
                0,
                0,
            );
            if let Some(event) = event {
                // SAFETY: `postEvent:atStart:` may be called from any thread (AppKit's
                // documentation); `app` is NSApp, alive for the whole process.
                unsafe {
                    let app = self.0.app as *mut AnyObject;
                    let _: () = msg_send![app, postEvent: &*event, atStart: false];
                }
            }
        });
    }

    /// Until rung or `timeout`, dispatching AppKit's events meanwhile.
    pub fn wait(self, timeout: Duration) {
        let Some(mtm) = MainThreadMarker::new() else {
            std::thread::sleep(timeout.min(Duration::from_millis(1)));
            return;
        };
        let app = NSApplication::sharedApplication(mtm);
        autoreleasepool(|_| {
            let until = NSDate::dateWithTimeIntervalSinceNow(timeout.as_secs_f64());
            loop {
                if self.0.rung.swap(false, Ordering::SeqCst) {
                    return;
                }
                // SAFETY: the default mode is a constant string AppKit exports.
                let mode = unsafe { NSDefaultRunLoopMode };
                let Some(event) = app.nextEventMatchingMask_untilDate_inMode_dequeue(
                    NSEventMask::Any,
                    Some(&until),
                    mode,
                    true,
                ) else {
                    return;
                };
                if event.r#type() != NSEventType::ApplicationDefined {
                    app.sendEvent(&event);
                }
            }
        });
    }
}

//! A tick on the UI thread, so a VST3 instance can tell its host that the latency changed.
//!
//! CLAP lets the audio thread ask for a restart; VST3 wants
//! `IComponentHandler::restartComponent(kLatencyChanged)` from the UI thread, and gives a
//! plug-in without an open editor no callback there. So every instance registers with a
//! thread timer on the thread that initialised it (the host's UI thread, which runs a
//! message loop), 20 times a second, and compares the latency the effect reports with the
//! latency the host last read. On Windows that is `SetTimer`; the other systems' run loops
//! (`IRunLoop` on Linux, `CFRunLoopTimer` on macOS) are for when those builds exist — until
//! then a change there is picked up when the host next activates the plug-in.

use crate::instance::Shared;
use ::vst3::ComPtr;
use ::vst3::Steinberg::Vst::{IComponentHandler, IComponentHandlerTrait, RestartFlags_};
use std::cell::RefCell;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

/// One instance's side of the tick.
pub struct Notifier {
    shared: Arc<Shared>,
    /// The host's handler: UI thread only.
    handler: RefCell<Option<ComPtr<IComponentHandler>>>,
    /// The latency the host last read (`getLatencySamples`).
    reported: AtomicU32,
}

// SAFETY: `handler` is only touched on the UI thread (`setComponentHandler`, `terminate`
// and the tick all run there); the rest is atomic or immutable.
unsafe impl Send for Notifier {}
unsafe impl Sync for Notifier {}

impl Notifier {
    pub fn new(shared: Arc<Shared>) -> Arc<Notifier> {
        Arc::new(Notifier {
            shared,
            handler: RefCell::new(None),
            reported: AtomicU32::new(0),
        })
    }

    pub fn set_handler(&self, handler: Option<ComPtr<IComponentHandler>>) {
        *self.handler.borrow_mut() = handler;
    }

    /// The host read the latency: this is what it knows now.
    pub fn mark_reported(&self) {
        self.reported
            .store(self.shared.latency(), Ordering::Release);
    }

    /// An edit from the plug-in's own editor, to the host (UI thread): the standard's
    /// `beginEdit`, `performEdit` (normalised) and `endEdit`.
    // Called from the editor view, which exists on Windows with the `editor` feature.
    #[cfg_attr(not(all(windows, feature = "editor")), allow(dead_code))]
    pub fn edit(&self, edit: crate::instance::Edit) {
        use crate::instance::Edit;
        let table = self.shared.table;
        let Some(h) = self.handler.borrow().clone() else {
            return;
        };
        // SAFETY: a live handler the host gave us, called on the UI thread.
        unsafe {
            match edit {
                Edit::Begin(i) => {
                    if let Some(p) = table.params.get(i as usize) {
                        h.beginEdit(p.id);
                    }
                }
                Edit::Value(i, v) => {
                    if let Some(p) = table.params.get(i as usize) {
                        h.performEdit(p.id, p.to_normalized(v));
                    }
                }
                Edit::End(i) => {
                    if let Some(p) = table.params.get(i as usize) {
                        h.endEdit(p.id);
                    }
                }
            }
        }
    }

    /// UI thread: tell the host if the latency moved since it last read it.
    #[cfg_attr(not(windows), allow(dead_code))]
    pub fn tick(&self) {
        let now = self.shared.latency();
        if self.reported.swap(now, Ordering::AcqRel) == now {
            return;
        }
        if let Some(h) = self.handler.borrow().as_ref() {
            // SAFETY: a live handler the host gave us, called on the UI thread.
            // The cast is a no-op on Windows and needed elsewhere (the enum's base type
            // differs by platform in the bindings).
            #[allow(clippy::unnecessary_cast)]
            let flags = RestartFlags_::kLatencyChanged as i32;
            unsafe {
                h.restartComponent(flags);
            }
        }
    }
}

thread_local! {
    static LIVE: RefCell<Vec<Arc<Notifier>>> = const { RefCell::new(Vec::new()) };
    static TIMER: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Start ticking an instance (from `initialize`, on the UI thread).
pub fn register(n: &Arc<Notifier>) {
    LIVE.with(|l| l.borrow_mut().push(n.clone()));
    os::start();
}

/// Stop ticking an instance (from `terminate`).
pub fn unregister(n: &Arc<Notifier>) {
    let empty = LIVE.with(|l| {
        let mut l = l.borrow_mut();
        l.retain(|x| !Arc::ptr_eq(x, n));
        l.is_empty()
    });
    if empty {
        os::stop();
    }
}

/// Tick every instance on this thread (the Windows timer's callback; the other systems'
/// run loops are not wired yet).
#[cfg_attr(not(windows), allow(dead_code))]
fn tick_all() {
    // A copy: a host may call back into an instance (and so into `unregister`) from
    // `restartComponent`.
    let live: Vec<Arc<Notifier>> = LIVE.with(|l| l.borrow().clone());
    for n in &live {
        n.tick();
    }
}

#[cfg(windows)]
mod os {
    use super::TIMER;
    use windows_sys::Win32::Foundation::HWND;
    use windows_sys::Win32::UI::WindowsAndMessaging::{KillTimer, SetTimer};

    unsafe extern "system" fn on_timer(_hwnd: HWND, _msg: u32, _id: usize, _time: u32) {
        super::tick_all();
    }

    pub fn start() {
        if TIMER.with(|t| t.get()) == 0 {
            // SAFETY: a thread timer (no window) with a plain callback.
            let id = unsafe { SetTimer(std::ptr::null_mut(), 0, 50, Some(on_timer)) };
            TIMER.with(|t| t.set(id));
        }
    }

    pub fn stop() {
        let id = TIMER.with(|t| t.replace(0));
        if id != 0 {
            // SAFETY: the timer this thread made.
            unsafe {
                KillTimer(std::ptr::null_mut(), id);
            }
        }
    }
}

#[cfg(not(windows))]
mod os {
    pub fn start() {
        let _ = super::TIMER.with(|t| t.get());
    }

    pub fn stop() {}
}

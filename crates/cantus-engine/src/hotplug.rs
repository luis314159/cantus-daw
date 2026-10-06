//! Noticing interfaces being plugged in and pulled out (round 20, audio setup), off the
//! UI thread.
//!
//! Windows has no cheap "device added" callback that cpal exposes, and enumerating
//! WASAPI's endpoints costs tens of milliseconds — too much for a frame. A watcher thread
//! lists the devices every couple of seconds and sends the list to the UI **only when it
//! changed**; the UI drains it with [`DeviceWatch::poll`] once a frame, which never
//! blocks.
//!
//! The watcher always lists the system's own driver (WASAPI on Windows): an interface's
//! endpoints appear and vanish there when its cable does, even while Cantus plays through
//! its ASIO driver — and listing ASIO drivers means loading them, which must never happen
//! while the engine holds one (`devices.rs`).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::Duration;

/// The devices present at one moment, by name.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DeviceList {
    pub outputs: Vec<String>,
    pub inputs: Vec<String>,
    pub default_output: Option<String>,
    pub default_input: Option<String>,
}

impl DeviceList {
    /// What the system's driver lists now. Slow (tens of ms): call it off the UI thread.
    pub fn scan() -> DeviceList {
        use cpal::traits::{DeviceTrait, HostTrait};
        let host = crate::devices::host(None);
        let name = |d: &cpal::Device| d.description().ok().map(|x| x.name().to_string());
        let mut outputs: Vec<String> = host
            .output_devices()
            .map(|it| it.filter_map(|d| name(&d)).collect())
            .unwrap_or_default();
        let mut inputs: Vec<String> = host
            .input_devices()
            .map(|it| it.filter_map(|d| name(&d)).collect())
            .unwrap_or_default();
        outputs.sort();
        inputs.sort();
        DeviceList {
            outputs,
            inputs,
            default_output: host.default_output_device().and_then(|d| name(&d)),
            default_input: host.default_input_device().and_then(|d| name(&d)),
        }
    }

    /// Names in `self` that `before` did not have, outputs then inputs.
    pub fn added_since(&self, before: &DeviceList) -> Vec<String> {
        let mut v: Vec<String> = Vec::new();
        for n in self.outputs.iter().chain(&self.inputs) {
            let old = before.outputs.contains(n) || before.inputs.contains(n);
            if !old && !v.contains(n) {
                v.push(n.clone());
            }
        }
        v
    }

    /// Names `before` had that `self` has not.
    pub fn removed_since(&self, before: &DeviceList) -> Vec<String> {
        before.added_since(self)
    }

    /// Whether a device of this name is present, as an output or an input.
    pub fn has(&self, name: &str) -> bool {
        self.outputs.iter().chain(&self.inputs).any(|n| n == name)
    }
}

/// A thread that lists the devices every `interval` and reports changes.
pub struct DeviceWatch {
    rx: mpsc::Receiver<DeviceList>,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl DeviceWatch {
    /// Watch the machine's devices.
    pub fn spawn(interval: Duration) -> DeviceWatch {
        Self::with_source(interval, DeviceList::scan)
    }

    /// Watch whatever `source` lists (tests hand in a scripted list). The first list is
    /// sent at once; later ones only when they differ from the last sent.
    pub fn with_source(
        interval: Duration,
        mut source: impl FnMut() -> DeviceList + Send + 'static,
    ) -> DeviceWatch {
        let (tx, rx) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let thread = std::thread::Builder::new()
            .name("cantus-device-watch".into())
            .spawn(move || {
                let mut last: Option<DeviceList> = None;
                let tick = Duration::from_millis(25).min(interval);
                while !flag.load(Ordering::Acquire) {
                    let now = source();
                    if last.as_ref() != Some(&now) {
                        if tx.send(now.clone()).is_err() {
                            return;
                        }
                        last = Some(now);
                    }
                    // Sleep in short steps, so dropping the watch ends the thread soon.
                    let mut slept = Duration::ZERO;
                    while slept < interval && !flag.load(Ordering::Acquire) {
                        std::thread::sleep(tick);
                        slept += tick;
                    }
                }
            })
            .ok();
        DeviceWatch { rx, stop, thread }
    }

    /// The newest list since the last poll, if the devices changed. Never blocks.
    pub fn poll(&self) -> Option<DeviceList> {
        let mut newest = None;
        while let Ok(l) = self.rx.try_recv() {
            newest = Some(l);
        }
        newest
    }
}

impl Drop for DeviceWatch {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        // Not joined: a scan in progress can take a moment, and the UI must not wait for
        // it. The thread sees the flag and ends on its own.
        drop(self.thread.take());
    }
}

impl std::fmt::Debug for DeviceWatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DeviceWatch").finish_non_exhaustive()
    }
}

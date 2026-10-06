//! Plug-and-play (round 20): interfaces noticed as they are connected and removed.
//!
//! The device watcher (`cantus_engine::hotplug`) lists the endpoints on its own thread
//! and sends a list only when it changed. Here a change becomes what a musician cares
//! about, by interface rather than by endpoint:
//!
//! - **the interface in use disappeared** → it is *lost*: the session stays open and
//!   editable (the engine's handle keeps accepting messages), a card says so and offers
//!   the computer's own audio meanwhile;
//! - **the lost interface came back** → it is opened again with its remembered settings,
//!   without asking (that is what anyone wants), and a card says so;
//! - **another interface appeared** → a card offers to switch to it.

use super::{interfaces, Notice};
use crate::app::CantusApp;
use cantus_engine::hotplug::{DeviceList, DeviceWatch};
use std::time::Duration;

/// How often the watcher lists the devices.
pub const INTERVAL: Duration = Duration::from_millis(1500);

#[derive(Default)]
pub struct Plug {
    pub watch: Option<DeviceWatch>,
    /// The last list seen.
    pub last: Option<DeviceList>,
    /// The interface that disappeared while in use, until it returns or another is chosen.
    pub lost: Option<String>,
    /// The watcher was started (it is not started again after being dropped by a test).
    started: bool,
}

impl Plug {
    /// Start watching the machine's devices, once.
    pub fn start(&mut self) {
        if !self.started {
            self.started = true;
            self.watch = Some(DeviceWatch::spawn(INTERVAL));
        }
    }

    /// Watch a scripted source instead (tests).
    pub fn watch_with(&mut self, w: DeviceWatch) {
        self.started = true;
        self.watch = Some(w);
    }
}

/// The interfaces (by key) a device list holds.
pub fn keys(list: &DeviceList) -> Vec<String> {
    let mut v: Vec<String> = list
        .outputs
        .iter()
        .chain(&list.inputs)
        .map(|e| interfaces::adapter_of(e))
        .collect();
    v.sort();
    v.dedup();
    v
}

/// Poll the watcher and act on a change.
pub fn poll(app: &mut CantusApp) {
    let Some(list) = app.audio_setup.plug.watch.as_ref().and_then(|w| w.poll()) else {
        return;
    };
    devices_changed(app, list);
}

/// The devices changed to `list`: lose, recover, or offer, as the module says.
pub fn devices_changed(app: &mut CantusApp, list: DeviceList) {
    let before = app.audio_setup.plug.last.replace(list.clone());
    let now_keys = keys(&list);
    // The first list only sets the baseline — unless the interface remembered last is
    // missing from it, which is the same as losing it.
    let before_keys = before
        .as_ref()
        .map(keys)
        .unwrap_or_else(|| now_keys.clone());
    let current = super::current_key(app);
    // Rescan so the popover and the setup show what is there now.
    app.audio_setup.request_scan();

    if let Some(lost) = app.audio_setup.plug.lost.clone() {
        if now_keys.contains(&lost) {
            super::reconnect(app, &lost);
            return;
        }
    }
    if let Some(cur) = current.as_deref() {
        let wasapi_key = app
            .audio_setup
            .scan
            .as_ref()
            .and_then(|s| s.interface(cur))
            .is_none_or(|i| !i.outputs.is_empty() || !i.inputs.is_empty());
        // An ASIO-only driver has no endpoint to watch; only interfaces with endpoints
        // can be seen leaving.
        if wasapi_key && before_keys.iter().any(|k| k == cur) && !now_keys.iter().any(|k| k == cur)
        {
            app.audio_setup.plug.lost = Some(cur.to_string());
            app.audio_setup.notice(Notice::Lost(cur.to_string()));
            app.set_error(crate::i18n::tf("devices.notice.lost", &[("name", &cur)]));
            return;
        }
    }
    for k in now_keys.iter().filter(|k| !before_keys.contains(k)) {
        if Some(k.as_str()) != current.as_deref()
            && interfaces::kind_of(k) != interfaces::Kind::Virtual
        {
            app.audio_setup.notice(Notice::Connected(k.clone()));
        }
    }
    // A card about an interface that left again is no longer true.
    app.audio_setup.notices.retain(|(n, _)| match n {
        Notice::Connected(k) => now_keys.contains(k),
        _ => true,
    });
}

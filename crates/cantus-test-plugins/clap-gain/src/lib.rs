//! A CLAP plug-in made only to test Cantus's host (ADR 0019). Built from source in this
//! repository by the host's tests; never shipped.
//!
//! **Cantus Test Gain** (`org.cantus.test.gain`):
//! - a stereo effect whose output is its input, delayed by its latency and scaled by
//!   **Gain** (dB; at 0 dB the factor is exactly 1.0, so the output is bit-exact);
//! - **Latency** in samples, which takes effect at the next activation: changing it asks
//!   the host to restart the plug-in, and `latency.get` then reports the new value;
//! - **Key**: when on, the sidechain input (a second, non-main stereo port) is added to the
//!   output, so a test can hear that the key arrived;
//! - **Gesture** (hidden): setting it to *n* makes the plug-in perform a gesture on Gain
//!   over its next five blocks — begin, −n, −n−1, −n−2 dB, end — as its own editor would;
//! - a state that carries Gain, Latency, Key and Gesture and a free *tag* that is not a
//!   parameter, so a test can tell the state travelled and not only the values; the tag
//!   `dirty` makes every load of that state mark it dirty and ask for the values to be
//!   read again, as JUCE's VST3 wrapper does, and `rewrite` does the same and keeps the
//!   state in bytes of its own (its tag becomes `rewritten`), saved in 800 ms; `count:<n>`
//!   loads as `count:<n+1>` and says nothing (its state moves with no word to the host);
//! - **Fault** (hidden, round 29): a failure on purpose — an abort or a hang in `process`,
//!   an abort in `save_state`, `load_state`, `activate` or the editor (`fault.rs`) — and
//!   **Process** (hidden, read-only): the id of the process the instance runs in, so the
//!   host's isolation tests see it really lived elsewhere.
//!
//! Its behaviour at load follows its **file name**: a copy whose name contains `crash`
//! aborts inside `clap_entry.init`, one whose name contains `hang` never returns from it.
//! That is how the scanner's crash and hang handling is tested with one build.

#![allow(clippy::missing_safety_doc)]

mod ext;
mod fault;
mod gui;
mod plugin;
#[cfg(target_os = "linux")]
mod x11gui;

use clap_sys::entry::clap_plugin_entry;
use clap_sys::factory::plugin_factory::{clap_plugin_factory, CLAP_PLUGIN_FACTORY_ID};
use clap_sys::host::clap_host;
use clap_sys::plugin::{clap_plugin, clap_plugin_descriptor};
use clap_sys::version::CLAP_VERSION;
use std::ffi::{c_char, c_void, CStr};

pub const PLUGIN_ID: &CStr = c"org.cantus.test.gain";

struct SyncPtrs([*const c_char; 4]);
unsafe impl Sync for SyncPtrs {}

static FEATURES: SyncPtrs = SyncPtrs([
    c"audio-effect".as_ptr(),
    c"utility".as_ptr(),
    c"stereo".as_ptr(),
    std::ptr::null(),
]);

static DESCRIPTOR: clap_plugin_descriptor = clap_plugin_descriptor {
    clap_version: CLAP_VERSION,
    id: PLUGIN_ID.as_ptr(),
    name: c"Cantus Test Gain".as_ptr(),
    vendor: c"Cantus Tests".as_ptr(),
    url: c"".as_ptr(),
    manual_url: c"".as_ptr(),
    support_url: c"".as_ptr(),
    version: c"1.0.0".as_ptr(),
    description: c"Gain, latency, state and a sidechain, for testing a host.".as_ptr(),
    features: FEATURES.0.as_ptr(),
};

unsafe extern "C" fn entry_init(path: *const c_char) -> bool {
    let name = if path.is_null() {
        String::new()
    } else {
        CStr::from_ptr(path).to_string_lossy().to_lowercase()
    };
    let file = name.rsplit(['/', '\\']).next().unwrap_or("").to_string();
    if file.contains("crash") {
        std::process::abort();
    }
    if file.contains("hang") {
        loop {
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
    }
    true
}

unsafe extern "C" fn entry_deinit() {}

unsafe extern "C" fn entry_get_factory(id: *const c_char) -> *const c_void {
    if !id.is_null() && CStr::from_ptr(id) == CLAP_PLUGIN_FACTORY_ID {
        (&FACTORY as *const clap_plugin_factory).cast()
    } else {
        std::ptr::null()
    }
}

static FACTORY: clap_plugin_factory = clap_plugin_factory {
    get_plugin_count: Some(factory_count),
    get_plugin_descriptor: Some(factory_descriptor),
    create_plugin: Some(factory_create),
};

unsafe extern "C" fn factory_count(_f: *const clap_plugin_factory) -> u32 {
    1
}

unsafe extern "C" fn factory_descriptor(
    _f: *const clap_plugin_factory,
    index: u32,
) -> *const clap_plugin_descriptor {
    if index == 0 {
        &DESCRIPTOR
    } else {
        std::ptr::null()
    }
}

unsafe extern "C" fn factory_create(
    _f: *const clap_plugin_factory,
    host: *const clap_host,
    id: *const c_char,
) -> *const clap_plugin {
    if id.is_null() || CStr::from_ptr(id) != PLUGIN_ID {
        return std::ptr::null();
    }
    plugin::create(host, &DESCRIPTOR)
}

#[no_mangle]
#[used]
#[allow(non_upper_case_globals)]
pub static clap_entry: clap_plugin_entry = clap_plugin_entry {
    clap_version: CLAP_VERSION,
    init: Some(entry_init),
    deinit: Some(entry_deinit),
    get_factory: Some(entry_get_factory),
};

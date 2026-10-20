//! A CLAP instrument made only to test Cantus's host (ADR 0028). Built from source in this
//! repository by the host's tests; never shipped.
//!
//! **Cantus Test Synth** (`org.cantus.test.synth`):
//! - one stereo output and one note input that takes CLAP note events and MIDI 1.0;
//! - a sine per note at `velocity × 0.25 × Gain`, starting at phase 0 on the note's sample
//!   and released linearly over 480 samples, computed sample by sample in `f64` so the
//!   output is the same at every block size;
//! - **Gain** (dB; exactly ×1.0 at 0 dB) and **Latency** in samples (the whole output is
//!   delayed by it; a change takes a restart, as `clap-gain`'s does);
//! - the **mod wheel** (CC 1) scales the output by `1 − ½·cc` (×0.5 at 127), **pitch bend**
//!   (MIDI, or a channel-wide tuning expression) moves every note by up to ±2 semitones,
//!   and a per-note tuning expression moves one note;
//! - read-only **Tempo seen** and **Playing seen**: what the last transport said, so a test
//!   can tell the song's position arrived;
//! - **Gesture** (hidden): setting it to *n* makes the plug-in perform a gesture on Gain,
//!   as its own editor would (begin, −n, −n−1, −n−2 dB, end);
//! - a state carrying the parameters before Fault and a free *tag*;
//! - **Fault** (hidden, round 29): a failure on purpose (`fault.rs`), and **Process**
//!   (hidden, read-only): the id of the process the instance runs in.

#![allow(clippy::missing_safety_doc)]

mod ext;
mod fault;
mod gui;
mod plugin;

use clap_sys::entry::clap_plugin_entry;
use clap_sys::factory::plugin_factory::{clap_plugin_factory, CLAP_PLUGIN_FACTORY_ID};
use clap_sys::host::clap_host;
use clap_sys::plugin::{clap_plugin, clap_plugin_descriptor};
use clap_sys::version::CLAP_VERSION;
use std::ffi::{c_char, c_void, CStr};

pub const PLUGIN_ID: &CStr = c"org.cantus.test.synth";

struct SyncPtrs([*const c_char; 4]);
unsafe impl Sync for SyncPtrs {}

static FEATURES: SyncPtrs = SyncPtrs([
    c"instrument".as_ptr(),
    c"synthesizer".as_ptr(),
    c"stereo".as_ptr(),
    std::ptr::null(),
]);

static DESCRIPTOR: clap_plugin_descriptor = clap_plugin_descriptor {
    clap_version: CLAP_VERSION,
    id: PLUGIN_ID.as_ptr(),
    name: c"Cantus Test Synth".as_ptr(),
    vendor: c"Cantus Tests".as_ptr(),
    url: c"".as_ptr(),
    manual_url: c"".as_ptr(),
    support_url: c"".as_ptr(),
    version: c"1.0.0".as_ptr(),
    description: c"Sines per note, controllers, transport, latency and state, for testing a host."
        .as_ptr(),
    features: FEATURES.0.as_ptr(),
};

unsafe extern "C" fn entry_init(_path: *const c_char) -> bool {
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

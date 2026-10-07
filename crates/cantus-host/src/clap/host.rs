//! The host side of the CLAP ABI: the `clap_host` a plug-in is created with, and the host
//! extensions it may ask for.
//!
//! A plug-in calls these from any thread. None of them does anything but set a flag (or
//! store a number) that [`super::main::ClapMain::idle`] reads on the main thread, so every
//! one is lock-free and allocation-free, which the audio thread needs (ADR 0005) and the
//! CLAP threading rules allow.

use clap_sys::ext::audio_ports::{clap_host_audio_ports, CLAP_EXT_AUDIO_PORTS};
use clap_sys::ext::gui::{clap_host_gui, CLAP_EXT_GUI};
use clap_sys::ext::latency::{clap_host_latency, CLAP_EXT_LATENCY};
use clap_sys::ext::log::{clap_host_log, clap_log_severity, CLAP_EXT_LOG};
use clap_sys::ext::note_ports::{
    clap_host_note_ports, clap_note_dialect, CLAP_EXT_NOTE_PORTS, CLAP_NOTE_DIALECT_CLAP,
    CLAP_NOTE_DIALECT_MIDI,
};
use clap_sys::ext::params::{
    clap_host_params, clap_param_clear_flags, clap_param_rescan_flags, CLAP_EXT_PARAMS,
    CLAP_PARAM_RESCAN_ALL, CLAP_PARAM_RESCAN_INFO, CLAP_PARAM_RESCAN_TEXT,
    CLAP_PARAM_RESCAN_VALUES,
};
use clap_sys::ext::state::{clap_host_state, CLAP_EXT_STATE};
use clap_sys::ext::thread_check::{clap_host_thread_check, CLAP_EXT_THREAD_CHECK};
use clap_sys::host::clap_host;
use clap_sys::id::clap_id;
use clap_sys::version::CLAP_VERSION;
use std::cell::Cell;
use std::ffi::{c_char, c_void, CStr};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

/// What a plug-in asked for since the last `idle`. Bits of [`HostData::flags`].
pub(crate) mod flag {
    pub const CALLBACK: u32 = 1 << 0;
    pub const RESTART: u32 = 1 << 1;
    pub const PROCESS: u32 = 1 << 2;
    pub const PARAM_VALUES: u32 = 1 << 3;
    pub const PARAM_INFO: u32 = 1 << 4;
    pub const STATE_DIRTY: u32 = 1 << 5;
    pub const LATENCY: u32 = 1 << 6;
    pub const GUI_RESIZE: u32 = 1 << 7;
    pub const GUI_CLOSED: u32 = 1 << 8;
    pub const REQUEST_FLUSH: u32 = 1 << 9;
    pub const PORTS: u32 = 1 << 10;
}

thread_local! {
    /// What this thread is to the plug-ins: bit 0 the main thread (the one that loads
    /// them), bit 1 inside an audio-thread call. Plain thread-locals with a constant
    /// initialiser: reading them on the audio thread allocates nothing.
    static ROLE: Cell<u8> = const { Cell::new(0) };
}

const ROLE_MAIN: u8 = 1;
const ROLE_AUDIO: u8 = 2;

/// Mark the calling thread as the main thread. Called by whatever loads plug-ins.
pub(crate) fn mark_main_thread() {
    ROLE.with(|r| r.set(r.get() | ROLE_MAIN));
}

pub(crate) fn on_main_thread() -> bool {
    ROLE.with(|r| r.get() & (ROLE_MAIN | ROLE_AUDIO) == ROLE_MAIN)
}

fn on_audio_thread() -> bool {
    ROLE.with(|r| r.get() & ROLE_AUDIO != 0 || r.get() & ROLE_MAIN == 0)
}

/// While alive, the calling thread is an audio thread to the plug-in (a call into the
/// audio half, or a flush the main thread makes while nothing is processing).
pub(crate) struct AudioScope(u8);

impl AudioScope {
    pub(crate) fn enter() -> Self {
        AudioScope(ROLE.with(|r| {
            let old = r.get();
            r.set(old | ROLE_AUDIO);
            old
        }))
    }
}

impl Drop for AudioScope {
    fn drop(&mut self) {
        let old = self.0;
        ROLE.with(|r| r.set(old));
    }
}

/// The `clap_host` handed to one plug-in instance, and what it has asked for.
///
/// Boxed and never moved, because the plug-in keeps the pointer: `clap.host_data` points
/// back at this struct.
#[repr(C)]
pub(crate) struct HostData {
    pub clap: clap_host,
    pub flags: AtomicU32,
    /// The editor's requested size, `width << 32 | height`.
    pub gui_size: AtomicU64,
}

const NAME: &CStr = c"Cantus";
const VENDOR: &CStr = c"Cantus";
const URL: &CStr = c"https://github.com/luis314159/cantus";
/// The workspace's version (`0.1.0-beta.1`), as plug-ins are told it.
const VERSION: &CStr =
    match CStr::from_bytes_with_nul(concat!(env!("CARGO_PKG_VERSION"), "\0").as_bytes()) {
        Ok(v) => v,
        Err(_) => panic!("the version has no NUL"),
    };

impl HostData {
    pub(crate) fn new() -> Box<HostData> {
        let mut data = Box::new(HostData {
            clap: clap_host {
                clap_version: CLAP_VERSION,
                host_data: std::ptr::null_mut(),
                name: NAME.as_ptr(),
                vendor: VENDOR.as_ptr(),
                url: URL.as_ptr(),
                version: VERSION.as_ptr(),
                get_extension: Some(get_extension),
                request_restart: Some(request_restart),
                request_process: Some(request_process),
                request_callback: Some(request_callback),
            },
            flags: AtomicU32::new(0),
            gui_size: AtomicU64::new(0),
        });
        let ptr: *mut HostData = &mut *data;
        data.clap.host_data = ptr.cast();
        data
    }

    pub(crate) fn raw(&self) -> *const clap_host {
        &self.clap
    }

    pub(crate) fn raise(&self, bits: u32) {
        self.flags.fetch_or(bits, Ordering::AcqRel);
    }

    /// Clear some bits and say whether any of them was set.
    pub(crate) fn take_bits(&self, bits: u32) -> bool {
        self.flags.fetch_and(!bits, Ordering::AcqRel) & bits != 0
    }
}

/// The host data behind a `clap_host` pointer. `None` for a null or foreign pointer.
unsafe fn data<'a>(host: *const clap_host) -> Option<&'a HostData> {
    if host.is_null() {
        return None;
    }
    let ptr = (*host).host_data as *const HostData;
    ptr.as_ref()
}

unsafe extern "C" fn request_restart(host: *const clap_host) {
    if let Some(d) = data(host) {
        d.raise(flag::RESTART);
    }
}

unsafe extern "C" fn request_process(host: *const clap_host) {
    // The engine calls every insert on every block while a device is open, so there is
    // nothing to wake; the flag is kept for the record.
    if let Some(d) = data(host) {
        d.raise(flag::PROCESS);
    }
}

unsafe extern "C" fn request_callback(host: *const clap_host) {
    if let Some(d) = data(host) {
        d.raise(flag::CALLBACK);
    }
}

unsafe extern "C" fn get_extension(_host: *const clap_host, id: *const c_char) -> *const c_void {
    if id.is_null() {
        return std::ptr::null();
    }
    let id = CStr::from_ptr(id);
    let ext: *const c_void = if id == CLAP_EXT_THREAD_CHECK {
        (&THREAD_CHECK as *const clap_host_thread_check).cast()
    } else if id == CLAP_EXT_PARAMS {
        (&PARAMS as *const clap_host_params).cast()
    } else if id == CLAP_EXT_STATE {
        (&STATE as *const clap_host_state).cast()
    } else if id == CLAP_EXT_LATENCY {
        (&LATENCY as *const clap_host_latency).cast()
    } else if id == CLAP_EXT_AUDIO_PORTS {
        (&AUDIO_PORTS as *const clap_host_audio_ports).cast()
    } else if id == CLAP_EXT_NOTE_PORTS {
        (&NOTE_PORTS as *const clap_host_note_ports).cast()
    } else if id == CLAP_EXT_GUI {
        (&GUI as *const clap_host_gui).cast()
    } else if id == CLAP_EXT_LOG {
        (&LOG as *const clap_host_log).cast()
    } else {
        std::ptr::null()
    };
    ext
}

static THREAD_CHECK: clap_host_thread_check = clap_host_thread_check {
    is_main_thread: Some(is_main_thread),
    is_audio_thread: Some(is_audio_thread),
};

unsafe extern "C" fn is_main_thread(_host: *const clap_host) -> bool {
    on_main_thread()
}

unsafe extern "C" fn is_audio_thread(_host: *const clap_host) -> bool {
    on_audio_thread()
}

static PARAMS: clap_host_params = clap_host_params {
    rescan: Some(params_rescan),
    clear: Some(params_clear),
    request_flush: Some(params_request_flush),
};

unsafe extern "C" fn params_rescan(host: *const clap_host, flags: clap_param_rescan_flags) {
    let Some(d) = data(host) else { return };
    if flags & (CLAP_PARAM_RESCAN_ALL | CLAP_PARAM_RESCAN_INFO) != 0 {
        d.raise(flag::PARAM_INFO | flag::PARAM_VALUES);
    } else if flags & (CLAP_PARAM_RESCAN_VALUES | CLAP_PARAM_RESCAN_TEXT) != 0 {
        d.raise(flag::PARAM_VALUES);
    }
}

unsafe extern "C" fn params_clear(
    _host: *const clap_host,
    _id: clap_id,
    _flags: clap_param_clear_flags,
) {
    // Cantus has no automation or modulation yet, so there is nothing to clear.
}

unsafe extern "C" fn params_request_flush(host: *const clap_host) {
    if let Some(d) = data(host) {
        d.raise(flag::REQUEST_FLUSH);
    }
}

static STATE: clap_host_state = clap_host_state {
    mark_dirty: Some(state_mark_dirty),
};

unsafe extern "C" fn state_mark_dirty(host: *const clap_host) {
    if let Some(d) = data(host) {
        d.raise(flag::STATE_DIRTY);
    }
}

static LATENCY: clap_host_latency = clap_host_latency {
    changed: Some(latency_changed),
};

unsafe extern "C" fn latency_changed(host: *const clap_host) {
    if let Some(d) = data(host) {
        d.raise(flag::LATENCY);
    }
}

static AUDIO_PORTS: clap_host_audio_ports = clap_host_audio_ports {
    is_rescan_flag_supported: Some(ports_rescan_supported),
    rescan: Some(ports_rescan),
};

unsafe extern "C" fn ports_rescan_supported(_host: *const clap_host, _flag: u32) -> bool {
    // Any change to the ports is handled the same way: a restart, which reads them again.
    true
}

unsafe extern "C" fn ports_rescan(host: *const clap_host, _flags: u32) {
    if let Some(d) = data(host) {
        d.raise(flag::PORTS | flag::RESTART);
    }
}

static NOTE_PORTS: clap_host_note_ports = clap_host_note_ports {
    supported_dialects: Some(note_dialects),
    rescan: Some(note_ports_rescan),
};

unsafe extern "C" fn note_dialects(_host: *const clap_host) -> clap_note_dialect {
    CLAP_NOTE_DIALECT_CLAP | CLAP_NOTE_DIALECT_MIDI
}

unsafe extern "C" fn note_ports_rescan(host: *const clap_host, _flags: u32) {
    if let Some(d) = data(host) {
        d.raise(flag::PORTS | flag::RESTART);
    }
}

static GUI: clap_host_gui = clap_host_gui {
    resize_hints_changed: Some(gui_hints_changed),
    request_resize: Some(gui_request_resize),
    request_show: Some(gui_request_show),
    request_hide: Some(gui_request_hide),
    closed: Some(gui_closed),
};

unsafe extern "C" fn gui_hints_changed(_host: *const clap_host) {}

unsafe extern "C" fn gui_request_resize(host: *const clap_host, width: u32, height: u32) -> bool {
    let Some(d) = data(host) else { return false };
    d.gui_size.store(
        (u64::from(width) << 32) | u64::from(height),
        Ordering::Release,
    );
    d.raise(flag::GUI_RESIZE);
    true
}

unsafe extern "C" fn gui_request_show(_host: *const clap_host) -> bool {
    true
}

unsafe extern "C" fn gui_request_hide(_host: *const clap_host) -> bool {
    true
}

unsafe extern "C" fn gui_closed(host: *const clap_host, _was_destroyed: bool) {
    if let Some(d) = data(host) {
        d.raise(flag::GUI_CLOSED);
    }
}

static LOG: clap_host_log = clap_host_log {
    log: Some(host_log),
};

unsafe extern "C" fn host_log(
    _host: *const clap_host,
    severity: clap_log_severity,
    msg: *const c_char,
) {
    // Logging allocates, and a plug-in may log from its audio thread: only the main
    // thread's messages are kept.
    if msg.is_null() || !on_main_thread() {
        return;
    }
    let text = CStr::from_ptr(msg).to_string_lossy();
    match severity {
        0 | 1 => log::debug!("plug-in: {text}"),
        2 => log::info!("plug-in: {text}"),
        _ => log::warn!("plug-in: {text}"),
    }
}

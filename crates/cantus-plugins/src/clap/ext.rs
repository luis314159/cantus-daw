//! The CLAP extensions an instance offers: parameters, state, latency, audio ports and
//! tail (and the GUI, `gui.rs`; the note port of a kind that hears notes, `notes.rs`).

use super::{read_events, Instance};
use crate::params::{Param, Role};
use clap_sys::events::clap_output_events;
use clap_sys::ext::audio_ports::*;
use clap_sys::ext::latency::{clap_plugin_latency, CLAP_EXT_LATENCY};
use clap_sys::ext::params::*;
use clap_sys::ext::state::{clap_plugin_state, CLAP_EXT_STATE};
use clap_sys::ext::tail::{clap_plugin_tail, CLAP_EXT_TAIL};
use clap_sys::id::{clap_id, CLAP_INVALID_ID};
use clap_sys::plugin::clap_plugin;
use clap_sys::stream::{clap_istream, clap_ostream};
use std::ffi::{c_char, c_void, CStr};
use std::ptr;

pub(crate) use super::gui::{drain_edits, gui_destroy_for, gui_on_main_thread, Gui};

pub(crate) unsafe extern "C" fn get_extension(
    plugin: *const clap_plugin,
    id: *const c_char,
) -> *const c_void {
    if id.is_null() {
        return ptr::null();
    }
    let id = CStr::from_ptr(id);
    if id == CLAP_EXT_PARAMS {
        &PARAMS as *const clap_plugin_params as *const c_void
    } else if id == CLAP_EXT_STATE {
        &STATE as *const clap_plugin_state as *const c_void
    } else if id == CLAP_EXT_LATENCY {
        &LATENCY as *const clap_plugin_latency as *const c_void
    } else if id == CLAP_EXT_AUDIO_PORTS {
        &AUDIO_PORTS as *const clap_plugin_audio_ports as *const c_void
    } else if id == CLAP_EXT_TAIL {
        &TAIL as *const clap_plugin_tail as *const c_void
    } else if id == clap_sys::ext::note_ports::CLAP_EXT_NOTE_PORTS
        && super::notes::hears_notes(plugin)
    {
        &super::notes::NOTE_PORTS as *const clap_sys::ext::note_ports::clap_plugin_note_ports
            as *const c_void
    } else {
        super::gui::extension(plugin, id)
    }
}

/// Copy `s` into a C string buffer, cut at a character boundary, always terminated.
pub(crate) fn copy_str(dst: &mut [c_char], s: &str) {
    if dst.is_empty() {
        return;
    }
    let mut end = s.len().min(dst.len() - 1);
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    for (d, b) in dst.iter_mut().zip(&s.as_bytes()[..end]) {
        *d = *b as c_char;
    }
    dst[end] = 0;
}

// ---- Parameters ----------------------------------------------------------------------

static PARAMS: clap_plugin_params = clap_plugin_params {
    count: Some(params_count),
    get_info: Some(params_get_info),
    get_value: Some(params_get_value),
    value_to_text: Some(params_value_to_text),
    text_to_value: Some(params_text_to_value),
    flush: Some(params_flush),
};

fn param(inst: &Instance, id: clap_id) -> Option<&Param> {
    inst.shared.table.by_id(id)
}

/// The flags a parameter carries.
pub fn flags(p: &Param) -> clap_param_info_flags {
    let mut f = CLAP_PARAM_IS_AUTOMATABLE;
    if p.stepped {
        f |= CLAP_PARAM_IS_STEPPED;
        if !p.choices.is_empty() {
            f |= CLAP_PARAM_IS_ENUM;
        }
    } else {
        f |= CLAP_PARAM_IS_MODULATABLE;
    }
    if p.role == Role::Bypass {
        f |= CLAP_PARAM_IS_BYPASS;
    }
    f
}

unsafe extern "C" fn params_count(plugin: *const clap_plugin) -> u32 {
    Instance::from(plugin).map_or(0, |i| i.shared.table.params.len() as u32)
}

unsafe extern "C" fn params_get_info(
    plugin: *const clap_plugin,
    index: u32,
    info: *mut clap_param_info,
) -> bool {
    let Some(inst) = Instance::from(plugin) else {
        return false;
    };
    let Some(p) = inst.shared.table.params.get(index as usize) else {
        return false;
    };
    if info.is_null() {
        return false;
    }
    let (min, max, default) = p.clap_range();
    let info = &mut *info;
    info.id = p.id;
    info.flags = flags(p);
    // The index + 1, handed back with every event: the audio thread needs no search.
    info.cookie = (p.index + 1) as *mut c_void;
    copy_str(&mut info.name, &p.name);
    copy_str(&mut info.module, &p.group);
    info.min_value = min;
    info.max_value = max;
    info.default_value = default;
    true
}

unsafe extern "C" fn params_get_value(
    plugin: *const clap_plugin,
    id: clap_id,
    out: *mut f64,
) -> bool {
    let Some(inst) = Instance::from(plugin) else {
        return false;
    };
    let Some(p) = param(inst, id) else {
        return false;
    };
    if out.is_null() {
        return false;
    }
    *out = p.to_clap(inst.shared.value(p.index));
    true
}

unsafe extern "C" fn params_value_to_text(
    plugin: *const clap_plugin,
    id: clap_id,
    value: f64,
    buf: *mut c_char,
    capacity: u32,
) -> bool {
    let Some(p) = Instance::from(plugin).and_then(|i| param(i, id)) else {
        return false;
    };
    if buf.is_null() || capacity == 0 {
        return false;
    }
    let dst = std::slice::from_raw_parts_mut(buf, capacity as usize);
    copy_str(dst, &p.text(p.from_clap(value)));
    true
}

unsafe extern "C" fn params_text_to_value(
    plugin: *const clap_plugin,
    id: clap_id,
    text: *const c_char,
    out: *mut f64,
) -> bool {
    let Some(p) = Instance::from(plugin).and_then(|i| param(i, id)) else {
        return false;
    };
    if text.is_null() || out.is_null() {
        return false;
    }
    let Ok(text) = CStr::from_ptr(text).to_str() else {
        return false;
    };
    match p.parse(text) {
        Some(v) => {
            *out = p.to_clap(v);
            true
        }
        None => false,
    }
}

unsafe extern "C" fn params_flush(
    plugin: *const clap_plugin,
    in_: *const clap_input_events,
    out: *const clap_output_events,
) {
    let Some(inst) = Instance::from(plugin) else {
        return;
    };
    let events = &mut *inst.events.get();
    match inst.audio().as_mut() {
        // Active: the audio thread's call, between blocks. The buffer was reserved in
        // `activate`, so nothing here allocates.
        Some(proc_) => {
            drain_edits(inst, Some(&mut *proc_), out);
            read_events(&inst.shared, in_, events);
            proc_.flush(events);
        }
        // Inactive: the main thread's.
        None => {
            if events.capacity() == 0 {
                events.reserve(super::EVENT_CAPACITY);
            }
            drain_edits(inst, None, out);
            read_events(&inst.shared, in_, events);
            for ev in events.iter().filter(|e| !e.modulation) {
                inst.shared.set_value(ev.index as usize, ev.value as f64);
            }
        }
    }
}

use clap_sys::events::clap_input_events;

// ---- State ---------------------------------------------------------------------------

static STATE: clap_plugin_state = clap_plugin_state {
    save: Some(state_save),
    load: Some(state_load),
};

unsafe extern "C" fn state_save(plugin: *const clap_plugin, stream: *const clap_ostream) -> bool {
    let Some(inst) = Instance::from(plugin) else {
        return false;
    };
    if stream.is_null() {
        return false;
    }
    let Some(write) = (*stream).write else {
        return false;
    };
    let bytes = inst.shared.save_state();
    let mut at = 0;
    while at < bytes.len() {
        let n = write(
            stream,
            bytes[at..].as_ptr() as *const c_void,
            (bytes.len() - at) as u64,
        );
        if n <= 0 {
            return false;
        }
        at += n as usize;
    }
    true
}

unsafe extern "C" fn state_load(plugin: *const clap_plugin, stream: *const clap_istream) -> bool {
    let Some(inst) = Instance::from(plugin) else {
        return false;
    };
    if stream.is_null() {
        return false;
    }
    let Some(read) = (*stream).read else {
        return false;
    };
    let mut bytes = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        let n = read(
            stream,
            chunk.as_mut_ptr() as *mut c_void,
            chunk.len() as u64,
        );
        if n < 0 {
            return false;
        }
        if n == 0 {
            break;
        }
        bytes.extend_from_slice(&chunk[..n as usize]);
    }
    if inst.shared.load_state(&bytes).is_err() {
        return false;
    }
    // Every value may have moved: the host re-reads them (and clap-validator holds a
    // plug-in to saying so).
    let host_params = inst.host_extension::<clap_host_params>(CLAP_EXT_PARAMS);
    if !host_params.is_null() {
        if let Some(rescan) = (*host_params).rescan {
            rescan(inst.host, CLAP_PARAM_RESCAN_VALUES);
        }
    }
    true
}

// ---- Latency and tail ----------------------------------------------------------------

static LATENCY: clap_plugin_latency = clap_plugin_latency {
    get: Some(latency_get),
};

unsafe extern "C" fn latency_get(plugin: *const clap_plugin) -> u32 {
    Instance::from(plugin).map_or(0, |i| i.shared.latency())
}

static TAIL: clap_plugin_tail = clap_plugin_tail {
    get: Some(tail_get),
};

unsafe extern "C" fn tail_get(plugin: *const clap_plugin) -> u32 {
    Instance::from(plugin).map_or(0, |i| i.shared.tail_samples())
}

// ---- Audio ports ---------------------------------------------------------------------

/// The ports' ids: the main pair shares 0, the sidechain input is 1.
const MAIN: clap_id = 0;
const SIDECHAIN: clap_id = 1;

static AUDIO_PORTS: clap_plugin_audio_ports = clap_plugin_audio_ports {
    count: Some(ports_count),
    get: Some(ports_get),
};

unsafe extern "C" fn ports_count(_plugin: *const clap_plugin, is_input: bool) -> u32 {
    if is_input {
        2
    } else {
        1
    }
}

unsafe extern "C" fn ports_get(
    _plugin: *const clap_plugin,
    index: u32,
    is_input: bool,
    info: *mut clap_audio_port_info,
) -> bool {
    if info.is_null() {
        return false;
    }
    let info = &mut *info;
    let (id, name_key, main) = match (is_input, index) {
        (true, 0) => (MAIN, "export.bus.input", true),
        (true, 1) => (SIDECHAIN, "export.bus.sidechain", false),
        (false, 0) => (MAIN, "export.bus.output", true),
        _ => return false,
    };
    info.id = id;
    copy_str(&mut info.name, &crate::text::t(name_key));
    info.flags = if main { CLAP_AUDIO_PORT_IS_MAIN } else { 0 };
    info.channel_count = 2;
    info.port_type = CLAP_PORT_STEREO.as_ptr();
    info.in_place_pair = if main { MAIN } else { CLAP_INVALID_ID };
    true
}

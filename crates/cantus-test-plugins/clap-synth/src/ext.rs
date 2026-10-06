//! The extensions: params, state, latency, audio-ports, note-ports, gui.

use crate::plugin::{me, name_bytes, PARAMS, PARAM_COUNT, P_LATENCY};
use clap_sys::events::{
    clap_event_param_value, clap_input_events, clap_output_events, CLAP_CORE_EVENT_SPACE_ID,
    CLAP_EVENT_PARAM_VALUE,
};
use clap_sys::ext::audio_ports::{
    clap_audio_port_info, clap_plugin_audio_ports, CLAP_AUDIO_PORT_IS_MAIN, CLAP_EXT_AUDIO_PORTS,
    CLAP_PORT_STEREO,
};
use clap_sys::ext::latency::{clap_plugin_latency, CLAP_EXT_LATENCY};
use clap_sys::ext::note_ports::{
    clap_note_port_info, clap_plugin_note_ports, CLAP_EXT_NOTE_PORTS, CLAP_NOTE_DIALECT_CLAP,
    CLAP_NOTE_DIALECT_MIDI,
};
use clap_sys::ext::params::{
    clap_param_info, clap_plugin_params, CLAP_EXT_PARAMS, CLAP_PARAM_IS_AUTOMATABLE,
    CLAP_PARAM_IS_HIDDEN, CLAP_PARAM_IS_READONLY, CLAP_PARAM_IS_STEPPED,
};
use clap_sys::ext::state::{clap_plugin_state, CLAP_EXT_STATE};
use clap_sys::id::{clap_id, CLAP_INVALID_ID};
use clap_sys::plugin::clap_plugin;
use clap_sys::stream::{clap_istream, clap_ostream};
use std::ffi::{c_char, c_void, CStr};
use std::sync::atomic::Ordering;

pub unsafe extern "C" fn get_extension(
    _plugin: *const clap_plugin,
    id: *const c_char,
) -> *const c_void {
    if id.is_null() {
        return std::ptr::null();
    }
    let id = CStr::from_ptr(id);
    if id == CLAP_EXT_PARAMS {
        (&PARAMS_EXT as *const clap_plugin_params).cast()
    } else if id == CLAP_EXT_STATE {
        (&STATE_EXT as *const clap_plugin_state).cast()
    } else if id == CLAP_EXT_LATENCY {
        (&LATENCY_EXT as *const clap_plugin_latency).cast()
    } else if id == clap_sys::ext::gui::CLAP_EXT_GUI {
        (&crate::gui::GUI_EXT as *const clap_sys::ext::gui::clap_plugin_gui).cast()
    } else if id == CLAP_EXT_AUDIO_PORTS {
        (&PORTS_EXT as *const clap_plugin_audio_ports).cast()
    } else if id == CLAP_EXT_NOTE_PORTS {
        (&NOTE_PORTS_EXT as *const clap_plugin_note_ports).cast()
    } else {
        std::ptr::null()
    }
}

// ---- params -----------------------------------------------------------------------------

static PARAMS_EXT: clap_plugin_params = clap_plugin_params {
    count: Some(params_count),
    get_info: Some(params_info),
    get_value: Some(params_value),
    value_to_text: Some(params_to_text),
    text_to_value: Some(params_from_text),
    flush: Some(params_flush),
};

unsafe extern "C" fn params_count(_p: *const clap_plugin) -> u32 {
    PARAM_COUNT as u32
}

unsafe extern "C" fn params_info(
    _p: *const clap_plugin,
    index: u32,
    out: *mut clap_param_info,
) -> bool {
    let Some(&(id, name, module, min, max, default, stepped, hidden, read_only)) =
        PARAMS.get(index as usize)
    else {
        return false;
    };
    let info = &mut *out;
    info.id = id;
    info.flags = 0;
    if read_only {
        info.flags |= CLAP_PARAM_IS_READONLY;
    } else if !hidden {
        info.flags |= CLAP_PARAM_IS_AUTOMATABLE;
    }
    if stepped {
        info.flags |= CLAP_PARAM_IS_STEPPED;
    }
    if hidden {
        info.flags |= CLAP_PARAM_IS_HIDDEN;
    }
    info.cookie = std::ptr::null_mut();
    name_bytes(&mut info.name, name);
    name_bytes(&mut info.module, module);
    info.min_value = min;
    info.max_value = max;
    info.default_value = default;
    true
}

unsafe extern "C" fn params_value(p: *const clap_plugin, id: clap_id, out: *mut f64) -> bool {
    if id as usize >= PARAM_COUNT {
        return false;
    }
    *out = me(p).get(id);
    true
}

unsafe extern "C" fn params_to_text(
    _p: *const clap_plugin,
    id: clap_id,
    value: f64,
    out: *mut c_char,
    capacity: u32,
) -> bool {
    let text = match id {
        0 => format!("{value:.1} dB"),
        1 => format!("{} smp", value.round()),
        2 => format!("{value:.2} BPM"),
        3 => (if value >= 0.5 { "Playing" } else { "Stopped" }).to_string(),
        4 => format!("{}", value.round()),
        _ => return false,
    };
    if capacity == 0 {
        return false;
    }
    let dst = std::slice::from_raw_parts_mut(out, capacity as usize);
    name_bytes(dst, &text);
    true
}

unsafe extern "C" fn params_from_text(
    _p: *const clap_plugin,
    _id: clap_id,
    text: *const c_char,
    out: *mut f64,
) -> bool {
    let s = CStr::from_ptr(text).to_string_lossy();
    let number: String = s
        .trim()
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.' || *c == '-')
        .collect();
    match number.parse::<f64>() {
        Ok(v) => {
            *out = v;
            true
        }
        Err(_) => false,
    }
}

unsafe extern "C" fn params_flush(
    p: *const clap_plugin,
    input: *const clap_input_events,
    _out: *const clap_output_events,
) {
    let me = me(p);
    let events = &*input;
    let count = events.size.map_or(0, |f| f(events));
    for i in 0..count {
        let Some(h) = events.get.map(|f| f(events, i)).filter(|h| !h.is_null()) else {
            continue;
        };
        if (*h).space_id == CLAP_CORE_EVENT_SPACE_ID && (*h).type_ == CLAP_EVENT_PARAM_VALUE {
            me.apply(&*(h as *const clap_event_param_value));
        }
    }
}

// ---- state ------------------------------------------------------------------------------

const MAGIC: &[u8; 4] = b"CTS1";

static STATE_EXT: clap_plugin_state = clap_plugin_state {
    save: Some(state_save),
    load: Some(state_load),
};

unsafe extern "C" fn state_save(p: *const clap_plugin, stream: *const clap_ostream) -> bool {
    let me = me(p);
    let mut bytes = MAGIC.to_vec();
    for row in PARAMS {
        bytes.extend_from_slice(&me.get(row.0).to_le_bytes());
    }
    let tag = me.tag.lock().map(|t| t.clone()).unwrap_or_default();
    bytes.extend_from_slice(&(tag.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&tag);
    let s = &*stream;
    let Some(write) = s.write else { return false };
    let mut at = 0;
    while at < bytes.len() {
        let n = write(
            stream,
            bytes[at..].as_ptr().cast(),
            (bytes.len() - at) as u64,
        );
        if n <= 0 {
            return false;
        }
        at += n as usize;
    }
    true
}

unsafe extern "C" fn state_load(p: *const clap_plugin, stream: *const clap_istream) -> bool {
    let me = me(p);
    let s = &*stream;
    let Some(read) = s.read else { return false };
    let mut bytes = Vec::new();
    let mut buf = [0u8; 256];
    loop {
        let n = read(stream, buf.as_mut_ptr().cast(), buf.len() as u64);
        if n < 0 {
            return false;
        }
        if n == 0 {
            break;
        }
        bytes.extend_from_slice(&buf[..n as usize]);
    }
    if bytes.len() < 4 + 8 * PARAM_COUNT + 4 || &bytes[..4] != MAGIC {
        return false;
    }
    let mut at = 4;
    for row in PARAMS {
        let v = f64::from_le_bytes(bytes[at..at + 8].try_into().unwrap_or([0; 8]));
        if !row.8 {
            me.set(row.0, v);
        }
        at += 8;
    }
    let len = u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap_or([0; 4])) as usize;
    at += 4;
    let Some(tag) = bytes.get(at..at + len) else {
        return false;
    };
    if let Ok(mut t) = me.tag.lock() {
        *t = tag.to_vec();
    }
    if me.get(P_LATENCY) as u32 != me.active_latency.load(Ordering::Acquire) {
        if let Some(f) = (*me.host).request_restart {
            f(me.host);
        }
    }
    true
}

// ---- latency ----------------------------------------------------------------------------

static LATENCY_EXT: clap_plugin_latency = clap_plugin_latency {
    get: Some(latency_get),
};

unsafe extern "C" fn latency_get(p: *const clap_plugin) -> u32 {
    me(p).active_latency.load(Ordering::Acquire)
}

// ---- audio ports: no input, one stereo output ---------------------------------------------

static PORTS_EXT: clap_plugin_audio_ports = clap_plugin_audio_ports {
    count: Some(ports_count),
    get: Some(ports_get),
};

unsafe extern "C" fn ports_count(_p: *const clap_plugin, is_input: bool) -> u32 {
    u32::from(!is_input)
}

unsafe extern "C" fn ports_get(
    _p: *const clap_plugin,
    index: u32,
    is_input: bool,
    out: *mut clap_audio_port_info,
) -> bool {
    if is_input || index != 0 {
        return false;
    }
    let info = &mut *out;
    info.id = 0;
    name_bytes(&mut info.name, "Main");
    info.flags = CLAP_AUDIO_PORT_IS_MAIN;
    info.in_place_pair = CLAP_INVALID_ID;
    info.channel_count = 2;
    info.port_type = CLAP_PORT_STEREO.as_ptr();
    true
}

// ---- note ports: one input, CLAP and MIDI ---------------------------------------------------

static NOTE_PORTS_EXT: clap_plugin_note_ports = clap_plugin_note_ports {
    count: Some(note_ports_count),
    get: Some(note_ports_get),
};

unsafe extern "C" fn note_ports_count(_p: *const clap_plugin, is_input: bool) -> u32 {
    u32::from(is_input)
}

unsafe extern "C" fn note_ports_get(
    _p: *const clap_plugin,
    index: u32,
    is_input: bool,
    out: *mut clap_note_port_info,
) -> bool {
    if !is_input || index != 0 {
        return false;
    }
    let info = &mut *out;
    info.id = 0;
    info.supported_dialects = CLAP_NOTE_DIALECT_CLAP | CLAP_NOTE_DIALECT_MIDI;
    info.preferred_dialect = CLAP_NOTE_DIALECT_CLAP;
    name_bytes(&mut info.name, "Notes");
    true
}

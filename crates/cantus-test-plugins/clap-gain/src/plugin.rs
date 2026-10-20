//! The plug-in instance: parameters in atomics (the main thread reads them while the audio
//! thread writes), a delay line sized at activation, and `process`.

use crate::ext;
use clap_sys::events::{
    clap_event_header, clap_event_param_gesture, clap_event_param_value, CLAP_CORE_EVENT_SPACE_ID,
    CLAP_EVENT_PARAM_GESTURE_BEGIN, CLAP_EVENT_PARAM_GESTURE_END, CLAP_EVENT_PARAM_VALUE,
};
use clap_sys::host::clap_host;
use clap_sys::plugin::{clap_plugin, clap_plugin_descriptor};
use clap_sys::process::{
    clap_process, clap_process_status, CLAP_PROCESS_CONTINUE, CLAP_PROCESS_ERROR,
};
use std::ffi::{c_char, c_void};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::Mutex;

pub const P_GAIN: u32 = 0;
pub const P_LATENCY: u32 = 1;
pub const P_KEY: u32 = 2;
pub const P_GESTURE: u32 = 3;
/// Hidden: a failure on purpose (`crate::fault`, round 29).
pub const P_FAULT: u32 = 4;
/// Hidden and read-only: the id of the process this instance runs in (round 29).
pub const P_PROCESS: u32 = 5;
pub const PARAM_COUNT: usize = 6;
/// The parameters the state carries (the ones before Fault and Process).
pub const STATE_PARAMS: usize = 4;

/// One parameter: (id, name, module, min, max, default, stepped, hidden).
pub type ParamRow = (u32, &'static str, &'static str, f64, f64, f64, bool, bool);

pub const PARAMS: [ParamRow; PARAM_COUNT] = [
    (P_GAIN, "Gain", "Main", -60.0, 12.0, 0.0, false, false),
    (
        P_LATENCY, "Latency", "Timing", 0.0, 20_000.0, 0.0, true, false,
    ),
    (P_KEY, "Key", "Sidechain", 0.0, 1.0, 0.0, true, false),
    (P_GESTURE, "Gesture", "Test", 0.0, 100.0, 0.0, true, true),
    (
        P_FAULT,
        "Fault",
        "Test",
        0.0,
        crate::fault::LAST,
        0.0,
        true,
        true,
    ),
    (P_PROCESS, "Process", "Test", 0.0, 4.0e9, 0.0, true, true),
];

pub struct Plugin {
    pub clap: clap_plugin,
    pub host: *const clap_host,
    pub values: [AtomicU64; PARAM_COUNT],
    /// Latency in force since the last activation.
    pub active_latency: AtomicU32,
    /// A tag carried in the state that is not a parameter.
    pub tag: Mutex<Vec<u8>>,
    pub processing: AtomicBool,
    /// A failure armed by the Fault parameter (round 29).
    pub fault: crate::fault::Fault,
    /// The editor: created (`gui.create`), and its window once it has a parent.
    pub editor_created: bool,
    #[cfg_attr(not(windows), allow(dead_code))]
    pub editor: isize,
    // Audio-thread state, sized at activation.
    delay_l: Vec<f32>,
    delay_r: Vec<f32>,
    pos: usize,
    /// Blocks left in a gesture being performed, and its base value.
    gesture_step: u32,
    gesture_base: f64,
}

impl Plugin {
    pub fn get(&self, id: u32) -> f64 {
        if id == P_PROCESS {
            return crate::fault::process_id();
        }
        f64::from_bits(self.values[id as usize].load(Ordering::Acquire))
    }

    pub fn set(&self, id: u32, v: f64) {
        if id == P_PROCESS {
            return; // read-only
        }
        if let Some((_, _, _, min, max, _, stepped, _)) = PARAMS.iter().find(|p| p.0 == id) {
            let v = v.clamp(*min, *max);
            let v = if *stepped { v.round() } else { v };
            self.values[id as usize].store(v.to_bits(), Ordering::Release);
        }
    }

    /// Apply a parameter event from the host; a latency change that differs from the one
    /// in force asks for a restart. `processing`: it came in `process`, where a Fault value
    /// arms its failure (a flush or a state never does).
    pub unsafe fn apply(&mut self, e: &clap_event_param_value, processing: bool) {
        self.set(e.param_id, e.value);
        if processing && e.param_id == P_FAULT {
            self.fault.arm(e.value);
        }
        if e.param_id == P_LATENCY
            && self.get(P_LATENCY) as u32 != self.active_latency.load(Ordering::Acquire)
        {
            if let Some(f) = (*self.host).request_restart {
                f(self.host);
            }
        }
        if e.param_id == P_GESTURE && e.value >= 1.0 {
            self.gesture_step = 5;
            self.gesture_base = -e.value.round();
        }
    }
}

pub unsafe fn create(
    host: *const clap_host,
    desc: &'static clap_plugin_descriptor,
) -> *const clap_plugin {
    let p = Box::new(Plugin {
        clap: clap_plugin {
            desc,
            plugin_data: std::ptr::null_mut(),
            init: Some(init),
            destroy: Some(destroy),
            activate: Some(activate),
            deactivate: Some(deactivate),
            start_processing: Some(start_processing),
            stop_processing: Some(stop_processing),
            reset: Some(reset),
            process: Some(process),
            get_extension: Some(ext::get_extension),
            on_main_thread: Some(on_main_thread),
        },
        host,
        values: std::array::from_fn(|i| AtomicU64::new(PARAMS[i].5.to_bits())),
        active_latency: AtomicU32::new(0),
        tag: Mutex::new(Vec::new()),
        processing: AtomicBool::new(false),
        fault: crate::fault::Fault::default(),
        editor_created: false,
        editor: 0,
        delay_l: Vec::new(),
        delay_r: Vec::new(),
        pos: 0,
        gesture_step: 0,
        gesture_base: 0.0,
    });
    let raw = Box::into_raw(p);
    (*raw).clap.plugin_data = raw.cast::<c_void>();
    &(*raw).clap
}

pub unsafe fn me<'a>(plugin: *const clap_plugin) -> &'a mut Plugin {
    &mut *((*plugin).plugin_data as *mut Plugin)
}

unsafe extern "C" fn init(_plugin: *const clap_plugin) -> bool {
    true
}

unsafe extern "C" fn destroy(plugin: *const clap_plugin) {
    drop(Box::from_raw((*plugin).plugin_data as *mut Plugin));
}

unsafe extern "C" fn activate(plugin: *const clap_plugin, _sr: f64, _min: u32, _max: u32) -> bool {
    let p = me(plugin);
    p.fault.fire(crate::fault::ABORT_ACTIVATE);
    // For the isolation tests: a plug-in that crashes every time it is loaded (the helper
    // inherits Cantus's environment).
    if std::env::var_os("CANTUS_TEST_GAIN_ABORT_ON_ACTIVATE").is_some() {
        std::process::abort();
    }
    let latency = p.get(P_LATENCY) as usize;
    p.active_latency.store(latency as u32, Ordering::Release);
    p.delay_l = vec![0.0; latency + 1];
    p.delay_r = vec![0.0; latency + 1];
    p.pos = 0;
    true
}

unsafe extern "C" fn deactivate(_plugin: *const clap_plugin) {}

unsafe extern "C" fn start_processing(plugin: *const clap_plugin) -> bool {
    me(plugin).processing.store(true, Ordering::Release);
    true
}

unsafe extern "C" fn stop_processing(plugin: *const clap_plugin) {
    me(plugin).processing.store(false, Ordering::Release);
}

unsafe extern "C" fn reset(plugin: *const clap_plugin) {
    let p = me(plugin);
    p.delay_l.fill(0.0);
    p.delay_r.fill(0.0);
}

unsafe extern "C" fn on_main_thread(_plugin: *const clap_plugin) {}

fn header<T>(type_: u16) -> clap_event_header {
    clap_event_header {
        size: size_of::<T>() as u32,
        time: 0,
        space_id: CLAP_CORE_EVENT_SPACE_ID,
        type_,
        flags: 0,
    }
}

/// One step of a performed gesture, into the output list.
unsafe fn gesture_out(p: &mut Plugin, process: &clap_process) {
    if p.gesture_step == 0 || process.out_events.is_null() {
        return;
    }
    let out = &*process.out_events;
    let Some(push) = out.try_push else { return };
    let step = p.gesture_step;
    p.gesture_step -= 1;
    match step {
        5 => {
            let e = clap_event_param_gesture {
                header: header::<clap_event_param_gesture>(CLAP_EVENT_PARAM_GESTURE_BEGIN),
                param_id: P_GAIN,
            };
            push(out, &e.header);
        }
        2..=4 => {
            let v = p.gesture_base - f64::from(4 - step);
            p.set(P_GAIN, v);
            let e = clap_event_param_value {
                header: header::<clap_event_param_value>(CLAP_EVENT_PARAM_VALUE),
                param_id: P_GAIN,
                cookie: std::ptr::null_mut(),
                note_id: -1,
                port_index: -1,
                channel: -1,
                key: -1,
                value: p.get(P_GAIN),
            };
            push(out, &e.header);
        }
        _ => {
            let e = clap_event_param_gesture {
                header: header::<clap_event_param_gesture>(CLAP_EVENT_PARAM_GESTURE_END),
                param_id: P_GAIN,
            };
            push(out, &e.header);
        }
    }
}

unsafe extern "C" fn process(
    plugin: *const clap_plugin,
    process: *const clap_process,
) -> clap_process_status {
    let p = me(plugin);
    let pr = &*process;
    let n = pr.frames_count as usize;
    if pr.audio_inputs_count < 1 || pr.audio_outputs_count < 1 {
        return CLAP_PROCESS_ERROR;
    }
    let input = &*pr.audio_inputs;
    let output = &*pr.audio_outputs;
    if input.channel_count < 2 || output.channel_count < 2 {
        return CLAP_PROCESS_ERROR;
    }
    let in_l = std::slice::from_raw_parts(*input.data32, n);
    let in_r = std::slice::from_raw_parts(*input.data32.add(1), n);
    let out_l = std::slice::from_raw_parts_mut(*output.data32, n);
    let out_r = std::slice::from_raw_parts_mut(*output.data32.add(1), n);
    let key = if pr.audio_inputs_count >= 2 {
        let k = &*pr.audio_inputs.add(1);
        (k.channel_count >= 2).then(|| {
            (
                std::slice::from_raw_parts(*k.data32, n),
                std::slice::from_raw_parts(*k.data32.add(1), n),
            )
        })
    } else {
        None
    };
    // Events, sample-accurately: process up to each event's time, then apply it.
    let events = &*pr.in_events;
    let count = events.size.map_or(0, |f| f(events));
    let mut done = 0usize;
    for i in 0..=count {
        let until = if i < count {
            let Some(h) = events.get.map(|f| f(events, i)).filter(|h| !h.is_null()) else {
                continue;
            };
            (*h).time as usize
        } else {
            n
        }
        .min(n);
        if until > done {
            run(
                p,
                &in_l[done..until],
                &in_r[done..until],
                key.map(|(l, r)| (&l[done..until], &r[done..until])),
                done,
                out_l,
                out_r,
            );
            done = until;
        }
        if i < count {
            let h = events.get.map(|f| f(events, i)).unwrap_or(std::ptr::null());
            if !h.is_null()
                && (*h).space_id == CLAP_CORE_EVENT_SPACE_ID
                && (*h).type_ == CLAP_EVENT_PARAM_VALUE
            {
                p.apply(&*(h as *const clap_event_param_value), true);
            }
        }
    }
    gesture_out(p, pr);
    CLAP_PROCESS_CONTINUE
}

/// Frames `[at, at + in_l.len())`: delay, gain, and the key when it is on.
fn run(
    p: &mut Plugin,
    in_l: &[f32],
    in_r: &[f32],
    key: Option<(&[f32], &[f32])>,
    at: usize,
    out_l: &mut [f32],
    out_r: &mut [f32],
) {
    let gain = 10f64.powf(p.get(P_GAIN) / 20.0) as f32;
    let key_on = p.get(P_KEY) >= 0.5;
    let len = p.delay_l.len().max(1);
    for i in 0..in_l.len() {
        let (dl, dr) = if p.delay_l.is_empty() {
            (in_l[i], in_r[i])
        } else {
            // A ring of latency + 1: write, then read the oldest.
            p.delay_l[p.pos] = in_l[i];
            p.delay_r[p.pos] = in_r[i];
            p.pos = (p.pos + 1) % len;
            (p.delay_l[p.pos], p.delay_r[p.pos])
        };
        let (mut l, mut r) = (dl * gain, dr * gain);
        if let (true, Some((kl, kr))) = (key_on, key) {
            l += kl[i];
            r += kr[i];
        }
        out_l[at + i] = l;
        out_r[at + i] = r;
    }
}

pub fn name_bytes(dst: &mut [c_char], s: &str) {
    for (i, b) in s.bytes().take(dst.len() - 1).enumerate() {
        dst[i] = b as c_char;
    }
    dst[s.len().min(dst.len() - 1)] = 0;
}

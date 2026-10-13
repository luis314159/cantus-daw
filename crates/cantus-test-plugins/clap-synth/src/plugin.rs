//! The instrument: parameters in atomics (the main thread reads them while the audio thread
//! writes), voices and a delay line sized at activation, and `process`.

use crate::ext;
use clap_sys::events::{
    clap_event_header, clap_event_midi, clap_event_note, clap_event_note_expression,
    clap_event_param_gesture, clap_event_param_value, CLAP_CORE_EVENT_SPACE_ID, CLAP_EVENT_MIDI,
    CLAP_EVENT_NOTE_CHOKE, CLAP_EVENT_NOTE_EXPRESSION, CLAP_EVENT_NOTE_OFF, CLAP_EVENT_NOTE_ON,
    CLAP_EVENT_PARAM_GESTURE_BEGIN, CLAP_EVENT_PARAM_GESTURE_END, CLAP_EVENT_PARAM_VALUE,
    CLAP_NOTE_EXPRESSION_TUNING, CLAP_TRANSPORT_IS_PLAYING,
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
pub const P_TEMPO: u32 = 2;
pub const P_PLAYING: u32 = 3;
pub const P_GESTURE: u32 = 4;
pub const PARAM_COUNT: usize = 5;

/// One parameter: (id, name, module, min, max, default, stepped, hidden, read-only).
pub type ParamRow = (
    u32,
    &'static str,
    &'static str,
    f64,
    f64,
    f64,
    bool,
    bool,
    bool,
);

pub const PARAMS: [ParamRow; PARAM_COUNT] = [
    (
        P_GAIN, "Gain", "Main", -60.0, 12.0, 0.0, false, false, false,
    ),
    (
        P_LATENCY, "Latency", "Timing", 0.0, 20_000.0, 0.0, true, false, false,
    ),
    (
        P_TEMPO,
        "Tempo seen",
        "Host",
        0.0,
        1000.0,
        0.0,
        false,
        false,
        true,
    ),
    (
        P_PLAYING,
        "Playing seen",
        "Host",
        0.0,
        1.0,
        0.0,
        true,
        false,
        true,
    ),
    (
        P_GESTURE, "Gesture", "Test", 0.0, 100.0, 0.0, true, true, false,
    ),
];

pub const VOICES: usize = 32;
/// Samples a released note takes to fall to silence.
pub const RELEASE: u32 = 480;

#[derive(Copy, Clone, Debug, Default)]
struct Voice {
    active: bool,
    note_id: i32,
    channel: i16,
    key: i16,
    phase: f64,
    velocity: f64,
    /// Semitones from this note's own tuning expression.
    tuning: f64,
    releasing: bool,
    left: u32,
}

pub struct Plugin {
    pub clap: clap_plugin,
    pub host: *const clap_host,
    pub values: [AtomicU64; PARAM_COUNT],
    /// Latency in force since the last activation.
    pub active_latency: AtomicU32,
    /// A tag carried in the state that is not a parameter.
    pub tag: Mutex<Vec<u8>>,
    pub processing: AtomicBool,
    pub editor_created: bool,
    #[cfg_attr(not(windows), allow(dead_code))]
    pub editor: isize,
    // Audio-thread state, sized at activation.
    sample_rate: f64,
    voices: [Voice; VOICES],
    delay: Vec<f32>,
    pos: usize,
    mod_wheel: f64,
    bend: f64,
    gesture_step: u32,
    gesture_base: f64,
}

impl Plugin {
    pub fn get(&self, id: u32) -> f64 {
        f64::from_bits(self.values[id as usize].load(Ordering::Acquire))
    }

    pub fn set(&self, id: u32, v: f64) {
        if let Some(row) = PARAMS.iter().find(|p| p.0 == id) {
            let v = v.clamp(row.3, row.4);
            let v = if row.6 { v.round() } else { v };
            self.values[id as usize].store(v.to_bits(), Ordering::Release);
        }
    }

    /// A parameter event from the host; a latency change asks for a restart.
    pub unsafe fn apply(&mut self, e: &clap_event_param_value) {
        if PARAMS.get(e.param_id as usize).is_some_and(|p| p.8) {
            return; // read-only
        }
        self.set(e.param_id, e.value);
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

    fn note_on(&mut self, note_id: i32, channel: i16, key: i16, velocity: f64) {
        let slot = self
            .voices
            .iter()
            .position(|v| !v.active)
            .unwrap_or_else(|| {
                // Steal the quietest: the one furthest into its release, else the first.
                (0..VOICES)
                    .filter(|&i| self.voices[i].releasing)
                    .min_by_key(|&i| self.voices[i].left)
                    .unwrap_or(0)
            });
        self.voices[slot] = Voice {
            active: true,
            note_id,
            channel,
            key,
            phase: 0.0,
            velocity,
            tuning: 0.0,
            releasing: false,
            left: RELEASE,
        };
    }

    fn matches(v: &Voice, note_id: i32, channel: i16, key: i16) -> bool {
        v.active
            && if note_id >= 0 {
                v.note_id == note_id
            } else {
                (key < 0 || v.key == key) && (channel < 0 || v.channel == channel)
            }
    }

    fn note_off(&mut self, note_id: i32, channel: i16, key: i16, choke: bool) {
        for v in self.voices.iter_mut() {
            if Self::matches(v, note_id, channel, key) {
                if choke {
                    v.active = false;
                } else if !v.releasing {
                    v.releasing = true;
                    v.left = RELEASE;
                }
            }
        }
    }

    fn midi(&mut self, data: [u8; 3]) {
        let channel = i16::from(data[0] & 15);
        match data[0] & 0xF0 {
            0x90 if data[2] > 0 => {
                self.note_on(-1, channel, i16::from(data[1]), f64::from(data[2]) / 127.0)
            }
            0x80 | 0x90 => self.note_off(-1, channel, i16::from(data[1]), false),
            0xB0 if data[1] == 1 => self.mod_wheel = f64::from(data[2]) / 127.0,
            0xE0 => {
                let raw = (i32::from(data[2]) << 7 | i32::from(data[1])) - 8192;
                self.bend = f64::from(raw) / 8192.0 * 2.0;
            }
            _ => {}
        }
    }

    unsafe fn event(&mut self, h: *const clap_event_header) {
        if (*h).space_id != CLAP_CORE_EVENT_SPACE_ID {
            return;
        }
        match (*h).type_ {
            CLAP_EVENT_PARAM_VALUE => self.apply(&*(h as *const clap_event_param_value)),
            CLAP_EVENT_NOTE_ON => {
                let n = &*(h as *const clap_event_note);
                self.note_on(n.note_id, n.channel, n.key, n.velocity);
            }
            CLAP_EVENT_NOTE_OFF | CLAP_EVENT_NOTE_CHOKE => {
                let n = &*(h as *const clap_event_note);
                let choke = (*h).type_ == CLAP_EVENT_NOTE_CHOKE;
                self.note_off(n.note_id, n.channel, n.key, choke);
            }
            CLAP_EVENT_NOTE_EXPRESSION => {
                let x = &*(h as *const clap_event_note_expression);
                if x.expression_id == CLAP_NOTE_EXPRESSION_TUNING {
                    if x.note_id < 0 && x.key < 0 {
                        self.bend = x.value; // channel-wide
                    } else {
                        for v in self.voices.iter_mut() {
                            if Self::matches(v, x.note_id, x.channel, x.key) {
                                v.tuning = x.value;
                            }
                        }
                    }
                }
            }
            CLAP_EVENT_MIDI => {
                let m = &*(h as *const clap_event_midi);
                self.midi(m.data);
            }
            _ => {}
        }
    }

    /// Frames `[at, at + n)` of the output.
    fn run(&mut self, at: usize, n: usize, out_l: &mut [f32], out_r: &mut [f32]) {
        let gain = 10f64.powf(self.get(P_GAIN) / 20.0) * (1.0 - 0.5 * self.mod_wheel);
        let sr = self.sample_rate.max(1.0);
        let len = self.delay.len().max(1);
        for i in at..at + n {
            let mut sum = 0.0f64;
            for v in self.voices.iter_mut().filter(|v| v.active) {
                let env = if v.releasing {
                    f64::from(v.left) / f64::from(RELEASE)
                } else {
                    1.0
                };
                sum += (v.phase * std::f64::consts::TAU).sin() * v.velocity * 0.25 * env;
                let semis = f64::from(v.key) - 69.0 + v.tuning + self.bend;
                v.phase += 440.0 * 2f64.powf(semis / 12.0) / sr;
                v.phase -= v.phase.floor();
                if v.releasing {
                    v.left = v.left.saturating_sub(1);
                    if v.left == 0 {
                        v.active = false;
                    }
                }
            }
            let s = (sum * gain) as f32;
            let y = if self.delay.is_empty() {
                s
            } else {
                self.delay[self.pos] = s;
                self.pos = (self.pos + 1) % len;
                self.delay[self.pos]
            };
            out_l[i] = y;
            out_r[i] = y;
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
        editor_created: false,
        editor: 0,
        sample_rate: 48_000.0,
        voices: [Voice::default(); VOICES],
        delay: Vec::new(),
        pos: 0,
        mod_wheel: 0.0,
        bend: 0.0,
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

unsafe extern "C" fn activate(plugin: *const clap_plugin, sr: f64, _min: u32, _max: u32) -> bool {
    let p = me(plugin);
    let latency = p.get(P_LATENCY) as usize;
    p.active_latency.store(latency as u32, Ordering::Release);
    p.sample_rate = sr;
    p.delay = if latency == 0 {
        Vec::new()
    } else {
        vec![0.0; latency + 1]
    };
    p.pos = 0;
    p.voices = [Voice::default(); VOICES];
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
    p.voices = [Voice::default(); VOICES];
    p.delay.fill(0.0);
    p.bend = 0.0;
    p.mod_wheel = 0.0;
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
            p.set(P_GAIN, p.gesture_base - f64::from(4 - step));
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
    if pr.audio_outputs_count < 1 {
        return CLAP_PROCESS_ERROR;
    }
    let output = &*pr.audio_outputs;
    if output.channel_count < 2 {
        return CLAP_PROCESS_ERROR;
    }
    let out_l = std::slice::from_raw_parts_mut(*output.data32, n);
    let out_r = std::slice::from_raw_parts_mut(*output.data32.add(1), n);
    if !pr.transport.is_null() {
        let t = &*pr.transport;
        p.set(P_TEMPO, t.tempo);
        let playing = t.flags & CLAP_TRANSPORT_IS_PLAYING != 0;
        p.values[P_PLAYING as usize]
            .store(f64::from(u8::from(playing)).to_bits(), Ordering::Release);
    }
    // Events, sample-accurately: render up to each event's time, then apply it.
    let events = &*pr.in_events;
    let count = events.size.map_or(0, |f| f(events));
    let mut done = 0usize;
    for i in 0..=count {
        let h = if i < count {
            events.get.map_or(std::ptr::null(), |f| f(events, i))
        } else {
            std::ptr::null()
        };
        let until = if h.is_null() {
            n
        } else {
            ((*h).time as usize).min(n)
        };
        if until > done {
            p.run(done, until - done, out_l, out_r);
            done = until;
        }
        if !h.is_null() {
            p.event(h);
        }
    }
    if done < n {
        p.run(done, n - done, out_l, out_r);
    }
    gesture_out(p, pr);
    CLAP_PROCESS_CONTINUE
}

pub fn name_bytes(dst: &mut [c_char], s: &str) {
    for (i, b) in s.bytes().take(dst.len() - 1).enumerate() {
        dst[i] = b as c_char;
    }
    dst[s.len().min(dst.len() - 1)] = 0;
}

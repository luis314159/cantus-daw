//! A minimal CLAP host for the tests: it drives a plug-in through the library's own
//! `clap_entry`, exactly as a host that loaded the binary would — the factory, the
//! plug-in's function table, its extensions — with nothing but the C ABI.

#![allow(dead_code)]

use cantus_core::{EffectId, EffectKind, EffectSlot, EffectSpec};
use cantus_engine::EffectUnit;
use cantus_plugins::desc;
use clap_sys::audio_buffer::clap_audio_buffer;
use clap_sys::entry::clap_plugin_entry;
use clap_sys::events::*;
use clap_sys::ext::latency::{clap_host_latency, clap_plugin_latency, CLAP_EXT_LATENCY};
use clap_sys::ext::params::{
    clap_host_params, clap_param_info, clap_plugin_params, CLAP_EXT_PARAMS,
};
use clap_sys::ext::state::{clap_plugin_state, CLAP_EXT_STATE};
use clap_sys::factory::plugin_factory::{clap_plugin_factory, CLAP_PLUGIN_FACTORY_ID};
use clap_sys::host::clap_host;
use clap_sys::plugin::clap_plugin;
use clap_sys::process::*;
use clap_sys::stream::{clap_istream, clap_ostream};
use clap_sys::version::CLAP_VERSION;
use std::ffi::{c_char, c_void, CStr, CString};
use std::ptr;
use std::sync::atomic::{AtomicU32, Ordering};

pub const SR: f32 = 48_000.0;

/// The host's side: counts what the plug-in asked of it.
pub struct Host {
    pub clap: clap_host,
    pub restarts: AtomicU32,
    pub latency_changed: AtomicU32,
    pub rescans: AtomicU32,
    _name: CString,
}

static HOST_PARAMS: clap_host_params = clap_host_params {
    rescan: Some(host_params_rescan),
    clear: Some(host_params_clear),
    request_flush: Some(host_noop),
};

unsafe extern "C" fn host_params_rescan(host: *const clap_host, _flags: u32) {
    let h = &*((*host).host_data as *const Host);
    h.rescans.fetch_add(1, Ordering::SeqCst);
}

unsafe extern "C" fn host_params_clear(_host: *const clap_host, _id: u32, _flags: u32) {}

static HOST_LATENCY: clap_host_latency = clap_host_latency {
    changed: Some(host_latency_changed),
};

unsafe extern "C" fn host_latency_changed(host: *const clap_host) {
    let h = &*((*host).host_data as *const Host);
    h.latency_changed.fetch_add(1, Ordering::SeqCst);
}

unsafe extern "C" fn host_get_extension(_h: *const clap_host, id: *const c_char) -> *const c_void {
    if CStr::from_ptr(id) == CLAP_EXT_LATENCY {
        &HOST_LATENCY as *const clap_host_latency as *const c_void
    } else if CStr::from_ptr(id) == CLAP_EXT_PARAMS {
        &HOST_PARAMS as *const clap_host_params as *const c_void
    } else {
        ptr::null()
    }
}

unsafe extern "C" fn host_request_restart(host: *const clap_host) {
    let h = &*((*host).host_data as *const Host);
    h.restarts.fetch_add(1, Ordering::SeqCst);
}

unsafe extern "C" fn host_noop(_h: *const clap_host) {}

impl Host {
    pub fn new() -> Box<Host> {
        let name = CString::new("cantus-tests").unwrap();
        let mut h = Box::new(Host {
            clap: clap_host {
                clap_version: CLAP_VERSION,
                host_data: ptr::null_mut(),
                name: name.as_ptr(),
                vendor: name.as_ptr(),
                url: name.as_ptr(),
                version: name.as_ptr(),
                get_extension: Some(host_get_extension),
                request_restart: Some(host_request_restart),
                request_process: Some(host_noop),
                request_callback: Some(host_noop),
            },
            restarts: AtomicU32::new(0),
            latency_changed: AtomicU32::new(0),
            rescans: AtomicU32::new(0),
            _name: name,
        });
        let p = &mut *h as *mut Host as *mut c_void;
        h.clap.host_data = p;
        h
    }
}

/// The library's entry, as a host finds it.
pub fn entry() -> &'static clap_plugin_entry {
    &cantus_plugins::clap::clap_entry
}

pub fn factory() -> &'static clap_plugin_factory {
    factory_of(entry())
}

/// The factory behind any CLAP entry (this library's, or a built binary's).
pub fn factory_of(e: &'static clap_plugin_entry) -> &'static clap_plugin_factory {
    unsafe {
        assert!(e.init.unwrap()(c"test".as_ptr()));
        let f = e.get_factory.unwrap()(CLAP_PLUGIN_FACTORY_ID.as_ptr());
        assert!(!f.is_null());
        &*(f as *const clap_plugin_factory)
    }
}

/// One plug-in instance driven through its function table.
pub struct Plug {
    pub host: Box<Host>,
    pub plugin: *const clap_plugin,
    pub kind: EffectKind,
    params: *const clap_plugin_params,
    state: *const clap_plugin_state,
    latency: *const clap_plugin_latency,
    /// The host's transport sent with each block (round 28, ADR 0055); `None`: none.
    pub transport: std::cell::Cell<Option<clap_event_transport>>,
}

/// A parameter event for [`Plug::process`]: `(offset, id, value in CLAP's units)`.
pub type Ev = (u32, u32, f64);

impl Plug {
    pub fn new(kind: EffectKind) -> Plug {
        Plug::new_from(entry(), kind)
    }

    /// An instance from any CLAP entry: this library's, or an exported binary's.
    pub fn new_from(e: &'static clap_plugin_entry, kind: EffectKind) -> Plug {
        let d = desc::of(kind).unwrap();
        let host = Host::new();
        unsafe {
            let f = factory_of(e);
            let id = CString::new(d.clap_id).unwrap();
            let plugin = f.create_plugin.unwrap()(f, &host.clap, id.as_ptr());
            assert!(!plugin.is_null(), "{} is created", d.clap_id);
            assert!((*plugin).init.unwrap()(plugin));
            let ext = |id: &CStr| (*plugin).get_extension.unwrap()(plugin, id.as_ptr());
            Plug {
                params: ext(CLAP_EXT_PARAMS) as *const _,
                state: ext(CLAP_EXT_STATE) as *const _,
                latency: ext(CLAP_EXT_LATENCY) as *const _,
                transport: std::cell::Cell::new(None),
                host,
                plugin,
                kind,
            }
        }
    }

    pub fn activate(&self, max_frames: u32) {
        assert!(self.activate_at(SR as f64, max_frames));
    }

    /// Activate at a sample rate; whether the plug-in accepted it.
    pub fn activate_at(&self, sample_rate: f64, max_frames: u32) -> bool {
        unsafe {
            if !(*self.plugin).activate.unwrap()(self.plugin, sample_rate, 1, max_frames) {
                return false;
            }
            assert!((*self.plugin).start_processing.unwrap()(self.plugin));
            true
        }
    }

    /// fx/final — `clap.render`'s mode: offline (`true`) or real time; whether the plug-in
    /// offers the extension and took it.
    pub fn set_render(&self, offline: bool) -> bool {
        use clap_sys::ext::render::*;
        unsafe {
            let ext = (*self.plugin).get_extension.unwrap()(self.plugin, CLAP_EXT_RENDER.as_ptr())
                as *const clap_plugin_render;
            if ext.is_null() {
                return false;
            }
            let mode = if offline {
                CLAP_RENDER_OFFLINE
            } else {
                CLAP_RENDER_REALTIME
            };
            (*ext).set.unwrap()(self.plugin, mode)
        }
    }

    pub fn deactivate(&self) {
        unsafe {
            (*self.plugin).stop_processing.unwrap()(self.plugin);
            (*self.plugin).deactivate.unwrap()(self.plugin);
        }
    }

    pub fn param_count(&self) -> u32 {
        unsafe { (*self.params).count.unwrap()(self.plugin) }
    }

    pub fn param_info(&self, index: u32) -> clap_param_info {
        unsafe {
            let mut info: clap_param_info = std::mem::zeroed();
            assert!((*self.params).get_info.unwrap()(
                self.plugin,
                index,
                &mut info
            ));
            info
        }
    }

    pub fn value(&self, id: u32) -> f64 {
        unsafe {
            let mut v = 0.0;
            assert!((*self.params).get_value.unwrap()(self.plugin, id, &mut v));
            v
        }
    }

    pub fn text(&self, id: u32, value: f64) -> String {
        unsafe {
            let mut buf = [0 as c_char; 256];
            assert!((*self.params).value_to_text.unwrap()(
                self.plugin,
                id,
                value,
                buf.as_mut_ptr(),
                256
            ));
            CStr::from_ptr(buf.as_ptr()).to_string_lossy().into_owned()
        }
    }

    pub fn parse(&self, id: u32, text: &str) -> Option<f64> {
        unsafe {
            let t = CString::new(text).unwrap();
            let mut v = 0.0;
            (*self.params).text_to_value.unwrap()(self.plugin, id, t.as_ptr(), &mut v).then_some(v)
        }
    }

    pub fn latency(&self) -> u32 {
        unsafe { (*self.latency).get.unwrap()(self.plugin) }
    }

    /// Flush parameter values outside processing (inactive: the main thread's call).
    pub fn flush(&self, events: &[Ev]) {
        let list = EventList::new(events);
        unsafe {
            (*self.params).flush.unwrap()(self.plugin, &list.clap, &OUT_EVENTS);
        }
    }

    /// One block through `process`, separate input and output buffers (or in place).
    pub fn process(
        &self,
        l: &mut [f32],
        r: &mut [f32],
        key: Option<(&[f32], &[f32])>,
        events: &[Ev],
        in_place: bool,
    ) {
        let n = l.len();
        let mut in_l = l.to_vec();
        let mut in_r = r.to_vec();
        let mut in_ptrs: [*mut f32; 2] = if in_place {
            [l.as_mut_ptr(), r.as_mut_ptr()]
        } else {
            [in_l.as_mut_ptr(), in_r.as_mut_ptr()]
        };
        let mut out_ptrs: [*mut f32; 2] = [l.as_mut_ptr(), r.as_mut_ptr()];
        let mut key_l = key.map(|k| k.0.to_vec()).unwrap_or_else(|| vec![0.0; n]);
        let mut key_r = key.map(|k| k.1.to_vec()).unwrap_or_else(|| vec![0.0; n]);
        let key_ptrs = [key_l.as_mut_ptr(), key_r.as_mut_ptr()];
        let list = EventList::new(events);
        self.process_raw(n, in_ptrs, out_ptrs, Some(key_ptrs), &list);
        let _ = (&mut in_l, &mut in_r, &mut in_ptrs, &mut out_ptrs);
    }

    /// One block in place with transport events inside it (CLAP's `CLAP_EVENT_TRANSPORT`
    /// in the input list, each at its time), the block's own transport as `transport` says.
    pub fn process_moves(
        &self,
        l: &mut [f32],
        r: &mut [f32],
        moves: &[(u32, clap_event_transport)],
    ) {
        let list = EventList::with_moves(&[], moves);
        self.process_list(l, r, None, &list);
    }

    /// One block in place, the events already built: allocates nothing on this side.
    pub fn process_list(
        &self,
        l: &mut [f32],
        r: &mut [f32],
        key: Option<(&[f32], &[f32])>,
        list: &EventList,
    ) {
        let n = l.len();
        let ptrs = [l.as_mut_ptr(), r.as_mut_ptr()];
        let key = key.map(|(a, b)| [a.as_ptr() as *mut f32, b.as_ptr() as *mut f32]);
        self.process_raw(n, ptrs, ptrs, key, list);
    }

    fn process_raw(
        &self,
        n: usize,
        mut in_ptrs: [*mut f32; 2],
        mut out_ptrs: [*mut f32; 2],
        key: Option<[*mut f32; 2]>,
        list: &EventList,
    ) {
        let mut key_ptrs = key.unwrap_or([ptr::null_mut(); 2]);
        let transport = self.transport.get();
        let inputs = [
            clap_audio_buffer {
                data32: in_ptrs.as_mut_ptr(),
                data64: ptr::null_mut(),
                channel_count: 2,
                latency: 0,
                constant_mask: 0,
            },
            clap_audio_buffer {
                data32: key_ptrs.as_mut_ptr(),
                data64: ptr::null_mut(),
                channel_count: 2,
                latency: 0,
                constant_mask: 0,
            },
        ];
        let mut outputs = [clap_audio_buffer {
            data32: out_ptrs.as_mut_ptr(),
            data64: ptr::null_mut(),
            channel_count: 2,
            latency: 0,
            constant_mask: 0,
        }];
        let p = clap_process {
            steady_time: -1,
            frames_count: n as u32,
            transport: transport
                .as_ref()
                .map_or(ptr::null(), |t| t as *const clap_event_transport),
            audio_inputs: inputs.as_ptr(),
            audio_outputs: outputs.as_mut_ptr(),
            audio_inputs_count: if key.is_some() { 2 } else { 1 },
            audio_outputs_count: 1,
            in_events: &list.clap,
            out_events: &OUT_EVENTS,
        };
        let status = unsafe { (*self.plugin).process.unwrap()(self.plugin, &p) };
        assert_eq!(status, CLAP_PROCESS_CONTINUE);
    }

    /// The whole signal in blocks of the given sizes (cycled), events at absolute times.
    pub fn run(
        &self,
        l: &mut [f32],
        r: &mut [f32],
        key: Option<(&[f32], &[f32])>,
        events: &[Ev],
        blocks: &[usize],
    ) {
        let mut at = 0;
        let mut b = 0;
        while at < l.len() {
            let n = blocks[b % blocks.len()].min(l.len() - at);
            b += 1;
            let evs: Vec<Ev> = events
                .iter()
                .filter(|(t, _, _)| (*t as usize) >= at && (*t as usize) < at + n)
                .map(|(t, id, v)| (*t - at as u32, *id, *v))
                .collect();
            let k = key.map(|(a, c)| (&a[at..at + n], &c[at..at + n]));
            self.process(&mut l[at..at + n], &mut r[at..at + n], k, &evs, b % 2 == 0);
            at += n;
        }
    }

    pub fn save(&self) -> Vec<u8> {
        let mut out: Vec<u8> = Vec::new();
        let stream = clap_ostream {
            ctx: &mut out as *mut Vec<u8> as *mut c_void,
            write: Some(ostream_write),
        };
        unsafe { assert!((*self.state).save.unwrap()(self.plugin, &stream)) };
        out
    }

    pub fn load(&self, bytes: &[u8]) -> bool {
        let mut cursor = (bytes.to_vec(), 0usize);
        let stream = clap_istream {
            ctx: &mut cursor as *mut (Vec<u8>, usize) as *mut c_void,
            read: Some(istream_read),
        };
        unsafe { (*self.state).load.unwrap()(self.plugin, &stream) }
    }
}

impl Drop for Plug {
    fn drop(&mut self) {
        unsafe { (*self.plugin).destroy.unwrap()(self.plugin) };
    }
}

unsafe extern "C" fn ostream_write(s: *const clap_ostream, buf: *const c_void, n: u64) -> i64 {
    let out = &mut *((*s).ctx as *mut Vec<u8>);
    // Short writes, to exercise the plug-in's loop.
    let n = (n as usize).min(7);
    out.extend_from_slice(std::slice::from_raw_parts(buf as *const u8, n));
    n as i64
}

unsafe extern "C" fn istream_read(s: *const clap_istream, buf: *mut c_void, n: u64) -> i64 {
    let (bytes, at) = &mut *((*s).ctx as *mut (Vec<u8>, usize));
    let n = (n as usize).min(bytes.len() - *at).min(5);
    ptr::copy_nonoverlapping(bytes[*at..].as_ptr(), buf as *mut u8, n);
    *at += n;
    n as i64
}

static OUT_EVENTS: clap_output_events = clap_output_events {
    ctx: ptr::null_mut(),
    try_push: Some(out_push),
};

unsafe extern "C" fn out_push(_l: *const clap_output_events, _e: *const clap_event_header) -> bool {
    true
}

/// A host's input event list over parameter values (and, for the transport tests, the
/// transport changes a host sends inside a block).
pub struct EventList {
    pub clap: clap_input_events,
    // Boxed: `clap.ctx` points at the headers' list, and it at each event, none of which
    // may move with the list.
    #[allow(clippy::box_collection)]
    _heads: Box<Vec<*const clap_event_header>>,
    _events: Vec<clap_event_param_value>,
    _moves: Vec<clap_event_transport>,
}

impl EventList {
    pub fn new(events: &[Ev]) -> EventList {
        EventList::with_moves(events, &[])
    }

    /// Parameter values and transport events (`(time, transport)`), in time order.
    pub fn with_moves(events: &[Ev], moves: &[(u32, clap_event_transport)]) -> EventList {
        let v: Vec<clap_event_param_value> = events
            .iter()
            .map(|(t, id, value)| clap_event_param_value {
                header: clap_event_header {
                    size: std::mem::size_of::<clap_event_param_value>() as u32,
                    time: *t,
                    space_id: CLAP_CORE_EVENT_SPACE_ID,
                    type_: CLAP_EVENT_PARAM_VALUE,
                    flags: 0,
                },
                param_id: *id,
                cookie: ptr::null_mut(),
                note_id: -1,
                port_index: -1,
                channel: -1,
                key: -1,
                value: *value,
            })
            .collect();
        let m: Vec<clap_event_transport> = moves
            .iter()
            .map(|(t, e)| clap_event_transport {
                header: clap_event_header {
                    time: *t,
                    ..e.header
                },
                ..*e
            })
            .collect();
        let mut heads: Box<Vec<*const clap_event_header>> = Box::new(
            v.iter()
                .map(|e| &e.header as *const clap_event_header)
                .chain(m.iter().map(|e| &e.header as *const clap_event_header))
                .collect(),
        );
        heads.sort_by_key(|h| unsafe { (**h).time });
        let ctx = &mut *heads as *mut Vec<*const clap_event_header> as *mut c_void;
        EventList {
            clap: clap_input_events {
                ctx,
                size: Some(list_size),
                get: Some(list_get),
            },
            _heads: heads,
            _events: v,
            _moves: m,
        }
    }
}

unsafe extern "C" fn list_size(l: *const clap_input_events) -> u32 {
    (*((*l).ctx as *const Vec<*const clap_event_header>)).len() as u32
}

unsafe extern "C" fn list_get(l: *const clap_input_events, i: u32) -> *const clap_event_header {
    (&(*((*l).ctx as *const Vec<*const clap_event_header>)))[i as usize]
}

/// A deterministic test signal: noise bursts over a quiet floor, and sines — something
/// for every detector to act on, and silence for the gates to close on.
pub fn signal(n: usize, seed: u32) -> (Vec<f32>, Vec<f32>) {
    let mut s = seed.max(1);
    let mut noise = move || {
        s ^= s << 13;
        s ^= s >> 17;
        s ^= s << 5;
        s as f32 / u32::MAX as f32 * 2.0 - 1.0
    };
    let mut l = Vec::with_capacity(n);
    let mut r = Vec::with_capacity(n);
    for i in 0..n {
        let t = i as f32 / SR;
        let burst = if (i / 4_800) % 3 == 0 { 0.7 } else { 0.01 };
        let sine = cantus_dsp::math::sin(t * 440.0 * std::f32::consts::TAU) * 0.3
            + cantus_dsp::math::sin(t * 7_000.0 * std::f32::consts::TAU) * 0.1;
        l.push(noise() * burst + sine * burst);
        r.push(noise() * burst * 0.8 + sine * 0.5 * burst);
    }
    (l, r)
}

/// A non-default setting per plug-in, one that makes it work (keys and values from the
/// parameter tables).
pub fn settings(kind: EffectKind) -> Vec<(&'static str, f32)> {
    match kind {
        EffectKind::Eq => vec![
            ("b0.used", 1.0),
            ("b0.shape", 0.0),
            ("b0.freq", 250.0),
            ("b0.gain", 6.0),
            ("b1.used", 1.0),
            ("b1.freq", 4_000.0),
            ("b1.gain", -4.0),
            ("b1.q", 2.0),
            ("b2.used", 1.0),
            ("b2.freq", 8_000.0),
            ("b2.gain", 3.0),
            ("b2.dyn", 1.0),
            ("b2.thr", -30.0),
            ("out", -1.5),
        ],
        EffectKind::Compressor => vec![
            ("thr", -30.0),
            ("ratio", 4.0),
            ("att", 5.0),
            ("rel", 80.0),
            ("style", 2.0),
            ("makeup", 3.0),
        ],
        EffectKind::Limiter => vec![("gain", 12.0), ("ceiling", -1.0), ("style", 1.0)],
        EffectKind::Space => vec![("decay", 1.8), ("mix", 0.35), ("predelay", 12.0)],
        EffectKind::Multiband => vec![
            ("b0.used", 1.0),
            ("b0.thr", -30.0),
            ("b1.used", 1.0),
            ("b1.from", 400.0),
            ("b1.thr", -25.0),
            ("b1.ratio", 3.0),
            ("b2.used", 1.0),
            ("b2.from", 3_000.0),
            ("b2.thr", -35.0),
        ],
        EffectKind::DeEsser => vec![("thr", -40.0), ("range", 9.0)],
        EffectKind::Gate => vec![("thr", -30.0), ("range", 60.0), ("hold", 10.0)],
        EffectKind::Stereo => vec![
            ("width", 1.4),
            ("rot", 10.0),
            ("mono", 150.0),
            ("w_hi", 1.6),
            ("s_hi", 3.0),
            ("m_lo", -2.0),
            ("haas", 6.0),
            ("haas_mode", 2.0),
        ],
        EffectKind::Maximizer => vec![
            ("gain", 9.0),
            ("lo.thr", -30.0),
            ("lo.ratio", 3.0),
            ("all.thr", -20.0),
            ("all.ratio", 2.0),
            ("xmode", 1.0),
        ],
        EffectKind::Saturator => vec![
            ("model", 1.0),
            ("drive", 18.0),
            ("char", 0.6),
            ("bias", 0.2),
            ("os", 1.0),
            ("tone", 2.0),
            ("mix", 0.8),
            ("p2.on", 1.0),
            ("p2.x", 0.5),
            ("p2.y", 0.8),
        ],
        // Round 28: each builder writes its own settings line.
        EffectKind::Chorus => vec![
            ("mode", 7.0),
            ("mix", 0.5),
            ("voices", 3.0),
            ("rate", 1.3),
            ("tone", 6_000.0),
        ],
        EffectKind::Flanger => vec![
            ("mix", 0.5),
            ("fb", 0.5),
            ("tz", 1.0),
            ("rate", 0.7),
            ("invert", 1.0),
        ],
        EffectKind::Phaser => vec![
            ("stages", 6.0),
            ("mix", 0.5),
            ("fb", 0.5),
            ("rate", 0.8),
            ("spread", 90.0),
        ],
        EffectKind::Tremolo => vec![
            ("depth", 0.8),
            ("sync", 0.0),
            ("rate", 7.0),
            ("mode", 2.0),
            ("shape", 2.0),
            ("spread", 90.0),
        ],
        EffectKind::Vibrato => vec![
            ("depth", 60.0),
            ("rate", 4.0),
            ("spread", 90.0),
            ("mix", 0.8),
        ],
        EffectKind::RingMod => vec![
            ("freq", 220.0),
            ("wave", 1.0),
            ("lfo", 5.0),
            ("mix", 0.8),
            ("spread", 90.0),
        ],
        EffectKind::FreqShift => vec![
            ("shift", 140.0),
            ("side", 2.0),
            ("fb", 0.4),
            ("lfo", 30.0),
            ("mix", 0.8),
        ],
        EffectKind::VolumeShaper => vec![
            ("depth", 0.9),
            ("div", 11.0),
            ("smooth", 3.0),
            ("offset", 0.1),
        ],
        EffectKind::Clipper => vec![
            ("drive", 9.0),
            ("ceil", -1.0),
            ("knee", 2.0),
            ("os", 2.0),
            ("tp", 1.0),
        ],
        EffectKind::Transient => vec![
            ("attack", 0.7),
            ("sustain", -0.4),
            ("speed", 6.0),
            ("band", 1.0),
        ],
        EffectKind::TransientSplit => vec![
            ("mode", 1.0),
            ("t.gain", 5.0),
            ("b.comp", 0.5),
            ("b.tilt", -2.0),
        ],
        EffectKind::Analyzer => vec![("view", 2.0), ("fft", 1.0), ("chan", 4.0)],
        EffectKind::AutoTune => vec![
            ("speed", 0.0),
            ("key", 2.0),
            ("scale", 1.0),
            ("throat", 2.0),
            ("vdepth", 25.0),
            ("n4", 3.0),
        ],
        // r39:conv
        EffectKind::Aula => vec![
            ("ir", 3.0),
            ("damp", 0.3),
            ("predelay", 20.0),
            ("width", 1.2),
            ("mix", 0.6),
        ],
        // r40:delay — synced (the host gives no tempo here: 120 BPM), an eighth triplet.
        EffectKind::Delay => vec![
            ("sync", 1.0),
            ("note", 11.0),
            ("feedback", 0.55),
            ("tone_hz", 4_500.0),
            ("mix", 0.6),
        ],
        // fx:contract — the side round's five: each builder writes its own settings line.
        // fx:cribrum — both filters, every source moving, oversampled.
        EffectKind::AutoFilter => vec![
            ("mix", 0.9),
            ("os", 1.0),
            ("a.model", 1.0),
            ("a.slope", 1.0),
            ("a.res", 0.6),
            ("b.on", 1.0),
            ("b.mode", 5.0),
            ("lfo.cut", 2.0),
            ("lfo.stereo", 90.0),
            ("env.cut", 1.0),
            ("cur.vowel", 1.0),
            ("out", -3.0),
        ],
        EffectKind::Pedal => vec![
            ("s1", 1.0),
            ("s2", 4.0),
            ("od.drive", 0.7),
            ("ge.fuzz", 0.8),
            ("out", -6.0),
        ], // fx:fragor
        // fx:lapsus — the roll build under a trance gate, mixed, long joins.
        EffectKind::TimeShaper => vec![
            ("tslot", 16.0),
            ("vslot", 5.0),
            ("mix", 0.8),
            ("xfade", 4.0),
            ("smooth", 3.0),
            ("out", -3.0),
        ],
        // fx:vetus — every module on, the dither and the noise's hashes, Character up.
        EffectKind::LoFi => vec![
            ("char", 0.4),
            ("bits.on", 1.0),
            ("bits.depth", 7.0),
            ("bits.dither", 1.0),
            ("rate.on", 1.0),
            ("rate.hz", 9_000.0),
            ("rate.filter", 2.0),
            ("wob.on", 1.0),
            ("age.on", 1.0),
            ("noise.on", 1.0),
            ("hum", 0.3),
            ("tone.on", 1.0),
            ("tone.medium", 3.0),
            ("mix", 0.8),
            ("out", -3.0),
        ], // fx:vetus
        // fx:libra — every stage at work.
        EffectKind::Utility => vec![
            ("dc", 1.0),
            ("chan", 2.0),
            ("pol.r", 1.0),
            ("delay.l", 3.3),
            ("width", 0.6),
            ("pan", 0.25),
            ("out", -6.0),
        ], // fx:libra
        // fx3:contract — wave 3's four: each builder writes its own settings line.
        EffectKind::Board => vec![("in", 2.0), ("mix", 0.8), ("m1", 0.6), ("out", -6.0)], // fx3:tabula
        // fx3:saltus — the mono shift's two voices spread over the dry, formants moved.
        EffectKind::PitchShift => vec![
            ("st1", 7.0),
            ("ct1", 12.0),
            ("pan1", -0.5),
            ("st2", -12.0),
            ("lvl2", 0.7),
            ("pan2", 0.4),
            ("dry", 0.6),
            ("formant", 0.0),
            ("throat", -2.0),
            ("out", -6.0),
        ],
        // fx3:rota — fast, the amp driven, the microphones wide and back, more horn.
        EffectKind::Rotary => vec![
            ("speed", 2.0),
            ("drive", 0.4),
            ("spread", 150.0),
            ("dist", 90.0),
            ("balance", 2.0),
            ("mix", 1.0),
            ("out", -6.0),
        ], // fx3:rota
        // fx3:arca — the crunch amp driven into the vintage 4×12, a ribbon off-centre, 2×.
        EffectKind::Amp => vec![
            ("model", 2.0),
            ("gain", 0.7),
            ("treble", 0.6),
            ("cab", 3.0),
            ("mic", 1.0),
            ("micpos", 0.4),
            ("os", 0.0),
            ("out", -6.0),
        ], // fx3:arca
        // fxa:loquax — the robot drones under the test signal in 24 steep bands, its formants
        // up, spread, a band drawn down (the signal speaks with no notes and no key).
        EffectKind::Vocoder => vec![
            ("mix", 0.9),
            ("robot", 1.0),
            ("rgate", -80.0),
            ("bands", 24.0),
            ("order", 2.0),
            ("formant", 3.0),
            ("spread", 0.5),
            ("b5", -9.0),
            ("out", -3.0),
        ], // fxa:loquax
        // fxa:fractus — no host transport here, so the steps rest (the starting stutter is
        // painted all the same): the cloud beside the chain, pitched a fifth and spread, some
        // grains backwards, and the output moved. The lanes on the host's beats are in
        // `transport.rs` and `vst3.rs`; notes playing them in `notes.rs`.
        EffectKind::Glitch => vec![
            ("clevel", 0.5),
            ("cpitch", 7.0),
            ("cdens", 60.0),
            ("csize", 80.0),
            ("cspread", 0.7),
            ("crev", 0.3),
            ("mix", 0.9),
            ("out", -3.0),
        ], // fxa:fractus
        // fxa:caelum — both voices climbing (an octave, a fifth), a short bloom, ducking by
        // its own input, half wet, the output moved. Keyed ducking is in `clap.rs` and
        // `vst3.rs`; the pre-delay on the host's tempo in `transport.rs` and `vst3.rs`.
        EffectKind::Shimmer => vec![
            ("mix", 0.5),
            ("decay", 3.0),
            ("shim", 0.6),
            ("pblvl", 0.5),
            ("bloom", 300.0),
            ("duck", 6.0),
            ("out", -3.0),
        ], // fxa:caelum
        // fxb:vinculum — the valve variable-mu working hard, its colour up, half parallel,
        // the side chain's high-pass in, the output moved. Every model's own sound is in
        // `clap.rs` (`vinculum_s_models_match_the_dsp`); the external key in `clap.rs` and
        // `vst3.rs`.
        EffectKind::BusCompressor => vec![
            ("model", 2.0),
            ("thr", -30.0),
            ("time", 4.0),
            ("colour", 0.6),
            ("hpf", 120.0),
            ("makeup", 6.0),
            ("mix", 0.5),
            ("out", -3.0),
        ], // fxb:vinculum
        // fxb:lenis — working hard on the precise frame: Hard, most of each bump, each
        // channel on its own, the focus from 300 Hz with a band raised, part dry, the output
        // moved. The latency per quality, its change and the delta are in `clap.rs` and
        // `vst3.rs`.
        EffectKind::ResonanceSuppressor => vec![
            ("depth", 0.8),
            ("curve", 1.0),
            ("qual", 2.0),
            ("chan", 1.0),
            ("select", 0.3),
            ("att", 2.0),
            ("lo", 300.0),
            ("b3on", 1.0),
            ("b3hz", 3000.0),
            ("b3amt", 0.6),
            ("mix", 0.8),
            ("out", -2.0),
        ], // fxb:lenis
        // fxb:cedo — the sidechain trigger (no key here: the duck waits, the lookahead and
        // the linear-phase lows' split still delay it, the output moved). The key, the
        // tempo, the notes, the shape and the latency's change are in `clap.rs`, `vst3.rs`,
        // `notes.rs` and `transport.rs`.
        EffectKind::Ducker => vec![
            ("trig", 0.0),
            ("depth", 18.0),
            ("kfilt", 2.0),
            ("khz", 70.0),
            ("look", 2.0),
            ("band", 1.0),
            ("xover", 1.0),
            ("xhz", 150.0),
            ("out", -3.0),
        ], // fxb:cedo
        // fxc:purus — every module on: the hum followed and subtracted gently over twelve
        // harmonics, the de-click with crackle, the room estimated and taken down, the
        // output moved (its latency the three modules': 50 ms, 767 and 1 023 samples).
        // fxc:comparatio — the reference asked to be heard, matched short-term with an offset
        // and a long crossfade, looped: with no reference loaded (a test has no file) the mix
        // passes to the bit, in the plug-in as in the DSP.
        EffectKind::Reference => vec![
            ("listen", 1.0),
            ("match", 1.0),
            ("offset", -2.0),
            ("fade", 120.0),
            ("sync", 2.0),
            ("lstart", 1.0),
            ("lend", 3.0),
            ("res", 1.0),
        ], // fxc:comparatio
        EffectKind::Repair => vec![
            ("hon", 1.0),
            ("hmode", 1.0),
            ("hharm", 12.0),
            ("hgentle", 1.0),
            ("con", 1.0),
            ("crackle", 0.5),
            ("von", 1.0),
            ("vamount", 14.0),
            ("vdecay", 1.0),
            ("out", -1.0),
        ], // fxc:purus
        // r41:nitor — a character, the tonic sub on its own key (the plug-in has no song),
        // the tightening and two exciter bands.
        EffectKind::Enhancer => vec![
            ("amount", 0.7),
            ("char", 1.0),
            ("sub", 0.4),
            ("submode", 1.0),
            ("tight", 0.4),
            ("pres", 0.3),
            ("air", 0.2),
            ("key", 10.0),
            ("out", -3.0),
        ], // r41:nitor
        _ => vec![],
    }
}

/// fx3:tabula — a board's spec as the exported plug-in holds it: the guitar rig when the
/// state names no board (a fresh instance's), its macros applied; any other spec as it is.
pub fn board_spec(spec: &EffectSpec) -> EffectSpec {
    let mut s = spec.clone();
    if s.kind == EffectKind::Board {
        if s.board.is_none() {
            s.board = cantus_core::fx::tabula::guitar_rig().board;
        }
        cantus_core::fx::tabula::ensure_macro_keys(&mut s);
        cantus_core::fx::tabula::apply_macros(&mut s);
    }
    s
}

/// The plug-ins of the family.
pub fn kinds() -> Vec<EffectKind> {
    desc::ALL.iter().map(|d| d.kind).collect()
}

/// The spec a DAW slot would hold for these settings.
/// What a fresh plug-in holds once `values` are set: its kind's starting preset (round 31,
/// ADR 0071 — a fresh instance starts there) with `values` over it.
pub fn spec_with(kind: EffectKind, values: &[(&str, f32)]) -> EffectSpec {
    let mut spec = cantus_core::starting::starting_spec(kind);
    for (k, v) in values {
        spec.set(k, *v)
            .unwrap_or_else(|| panic!("{kind:?} has {k}"));
    }
    spec
}

/// The engine's own unit for a spec: what Cantus runs for this insert.
pub fn engine_unit(spec: &EffectSpec) -> EffectUnit {
    let slot = EffectSlot {
        id: EffectId(1),
        bypass: false,
        spec: spec.clone(),
        mix: 1.0,
        sidechain: None,
        midi_from: None,
    };
    EffectUnit::new(&slot, SR)
}

/// CLAP's value for a key's plain value (through the knob's position where CLAP sees it).
pub fn clap_value(kind: EffectKind, key: &str, plain: f32) -> (u32, f64) {
    let t = cantus_plugins::params::table(kind);
    let p = t.by_key(key).unwrap();
    (p.id, p.to_clap(plain))
}

/// The DSP called directly — `cantus-dsp`'s own types, set through the engine's mapping
/// of the parameters (`cantus_engine::{eq_bands, comp_settings, fx::*::settings}`), with
/// no `EffectUnit` in between.
pub enum Direct {
    Eq(Box<cantus_dsp::Aequalis>),
    Comp(Box<cantus_dsp::Comprimo>),
    Lim(Box<cantus_dsp::terminus::Terminus>),
    Space(Box<cantus_dsp::spatium::Spatium>),
    Mb(Box<cantus_dsp::multiplex::Multiplex>),
    Ds(Box<cantus_dsp::sibilus::Sibilus>),
    Gate(Box<cantus_dsp::porta::Porta>),
    Stereo(Box<cantus_dsp::latitudo::Latitudo>),
    Max(Box<cantus_dsp::culmen::Culmen>),
    Sat(Box<cantus_dsp::fervor::Fervor>),
    Tonus(Box<cantus_dsp::tonus::Tonus>),
    Tremor(Box<cantus_dsp::tremor::Tremor>),
    Fluctus(Box<cantus_dsp::fluctus::Fluctus>),
    Translatio(Box<cantus_dsp::translatio::Translatio>),
    Anulus(Box<cantus_dsp::anulus::Anulus>),
    Phasis(Box<cantus_dsp::phasis::Phasis>),
    Flexus(Box<cantus_dsp::flexus::Flexus>),
    Concentus(Box<cantus_dsp::concentus::Concentus>),
    Vibrans(Box<cantus_dsp::vibrans::Vibrans>),
    Tonsor(Box<cantus_dsp::tonsor::Tonsor>),
    Impetus(Box<cantus_dsp::impetus::Impetus>),
    Discrimen(Box<cantus_dsp::discrimen::Discrimen>),
    /// Speculum: a pure pass-through, no DSP on the signal (ADR 0056).
    Wire,
    /// r39:conv — Aula, with the source and shape its kernel was built from.
    Aula(Box<cantus_dsp::aula::Aula>, (u64, [u32; 9])),
    /// r40:delay — Delay, on the line its settings want.
    Delay(Box<cantus_dsp::StereoDelay>),
    /// fx:fragor — the engine's unit itself (Fragor's board, its circuits and oversampling
    /// live in the engine's `fx::fragor`).
    Stub(Box<dyn cantus_engine::fx::FxUnit>),
    /// fx:cribrum — Cribrum, the auto filter.
    Cribrum(Box<cantus_dsp::cribrum::Cribrum>),
    /// fx:vetus — Vetus, lo-fi.
    Vetus(Box<cantus_dsp::vetus::Vetus>),
    /// fx:libra — Libra, the utility.
    Libra(Box<cantus_dsp::libra::Libra>),
    /// fx3:saltus — Saltus, the pitch unit.
    Saltus(Box<cantus_dsp::saltus::Saltus>),
    /// fx3:rota — Rota, the rotary speaker.
    Rota(Box<cantus_dsp::rota::Rota>),
    /// fx3:arca — Arca, the amp and its cabinet.
    Arca(Box<cantus_dsp::arca::Arca>),
    /// fxa:loquax — Loquax, the vocoder (its notes, when any, through `Direct::note`).
    Loquax(Box<cantus_dsp::loquax::Loquax>),
    /// fxa:fractus — Fractus, the glitch sequencer: its DSP with the lanes written into
    /// its steps as the engine writes them (`fx::fractus::pattern_into`).
    /// fxc:purus — Purus, the real-time repair: its DSP (its output level is its own).
    Purus(Box<cantus_dsp::purus::Purus>),
    /// fxc:comparatio — Comparatio, the reference comparator: its DSP, no reference.
    Comparatio(Box<cantus_dsp::comparatio::Comparatio>),
    /// fxa:caelum — Caelum, the shimmer reverb: its DSP (the pre-delay synced at 120 BPM when
    /// asked, no host transport here) and its output level gliding after it, as the engine's
    /// unit glides it.
    Caelum(
        Box<cantus_dsp::caelum::Caelum>,
        cantus_dsp::modfx_line::Glide,
    ),
    Fractus(
        Box<cantus_dsp::fractus::Fractus>,
        Box<cantus_dsp::fractus::Pattern>,
    ),
    /// fxb:vinculum — Vinculum, the bus compressor: its DSP keyed by the sidechain when
    /// asked, and its output level gliding after it, as the engine's unit glides it.
    Vinculum(
        Box<cantus_dsp::vinculum::Vinculum>,
        cantus_dsp::modfx_line::Glide,
    ),
    /// fxb:lenis — Lenis, the resonance suppressor: its DSP, its output level its own.
    Lenis(Box<cantus_dsp::lenis::Lenis>),
    /// fxb:cedo — Cedo, the ducker: its DSP, its shape as the engine passes it (the
    /// classic pump when the spec draws none).
    Cedo(Box<cantus_dsp::cedo::Cedo>),
    /// r41:nitor — Nitor, the enhancer: its DSP, tuned to its own key (no song here).
    Nitor(Box<cantus_dsp::nitor::Nitor>),
}

/// r39:conv — Aula's kernel for `spec`, built as the engine builds it, in `cap`.
fn aula_kernel(
    spec: &EffectSpec,
    cap: Option<cantus_dsp::aula_kernel::Layout>,
) -> std::sync::Arc<cantus_dsp::aula_kernel::Kernel> {
    let (source, shape) = cantus_engine::fx::aula::want(spec);
    let cap = cap.unwrap_or_else(|| cantus_dsp::aula::capacity_for(0));
    let m = cantus_engine::fx::aula_store::build(
        cantus_engine::fx::aula::AulaNeed { source, shape, cap },
        SR,
    );
    m.kernel.expect("a kernel")
}

impl Direct {
    pub fn new(spec: &EffectSpec) -> Direct {
        let mut d = match spec.kind {
            EffectKind::Eq => Direct::Eq(Box::new(cantus_dsp::Aequalis::new(SR))),
            EffectKind::Compressor => Direct::Comp(Box::new(cantus_dsp::Comprimo::new(SR))),
            EffectKind::Limiter => Direct::Lim(Box::new(cantus_dsp::terminus::Terminus::new(SR))),
            EffectKind::Space => Direct::Space(Box::new(cantus_dsp::spatium::Spatium::new(SR))),
            EffectKind::Multiband => {
                Direct::Mb(Box::new(cantus_dsp::multiplex::Multiplex::new(SR)))
            }
            EffectKind::DeEsser => Direct::Ds(Box::new(cantus_dsp::sibilus::Sibilus::new(SR))),
            EffectKind::Gate => Direct::Gate(Box::new(cantus_dsp::porta::Porta::new(SR))),
            EffectKind::Stereo => Direct::Stereo(Box::new(cantus_dsp::latitudo::Latitudo::new(SR))),
            EffectKind::Maximizer => Direct::Max(Box::new(cantus_dsp::culmen::Culmen::new(SR))),
            EffectKind::Saturator => Direct::Sat(Box::new(cantus_dsp::fervor::Fervor::new(SR))),
            EffectKind::Chorus => {
                Direct::Concentus(Box::new(cantus_dsp::concentus::Concentus::new(SR)))
            }
            EffectKind::Flanger => Direct::Flexus(Box::new(cantus_dsp::flexus::Flexus::new(SR))),
            EffectKind::Phaser => Direct::Phasis(Box::new(cantus_dsp::phasis::Phasis::new(SR))),
            EffectKind::Tremolo => Direct::Tremor(Box::new(cantus_dsp::tremor::Tremor::new(SR))),
            EffectKind::Vibrato => Direct::Vibrans(Box::new(cantus_dsp::vibrans::Vibrans::new(SR))),
            EffectKind::RingMod => Direct::Anulus(Box::new(cantus_dsp::anulus::Anulus::new(SR))),
            EffectKind::FreqShift => {
                Direct::Translatio(Box::new(cantus_dsp::translatio::Translatio::new(SR)))
            }
            EffectKind::VolumeShaper => Direct::Fluctus(Box::new(
                cantus_dsp::fluctus::Fluctus::new(SR, cantus_engine::fx::fluctus::shape_at),
            )),
            EffectKind::Clipper => Direct::Tonsor(Box::new(cantus_dsp::tonsor::Tonsor::new(SR))),
            EffectKind::Transient => {
                Direct::Impetus(Box::new(cantus_dsp::impetus::Impetus::new(SR)))
            }
            EffectKind::TransientSplit => {
                Direct::Discrimen(Box::new(cantus_dsp::discrimen::Discrimen::new(SR)))
            }
            EffectKind::Analyzer => Direct::Wire,
            EffectKind::AutoTune => Direct::Tonus(Box::new(cantus_dsp::tonus::Tonus::new(SR))),
            EffectKind::Aula => Direct::Aula(
                Box::new(cantus_dsp::aula::Aula::with_kernel(
                    SR,
                    aula_kernel(spec, None),
                )),
                cantus_engine::fx::aula::want(spec),
            ), // r39:conv
            EffectKind::Delay => Direct::Delay(Box::new(cantus_dsp::StereoDelay::new(SR))),
            EffectKind::AutoFilter => Direct::Cribrum(Box::new(cantus_dsp::cribrum::Cribrum::new(
                SR,
                cantus_engine::fx::fluctus::shape_at,
            ))), // fx:cribrum
            EffectKind::Pedal => {
                Direct::Stub(cantus_engine::fx::make(EffectKind::Pedal, SR).unwrap())
            } // fx:contract
            // fx:lapsus — the engine's unit with its history, as `EffectUnit::new` gives it.
            EffectKind::TimeShaper => {
                let mut u = cantus_engine::fx::make(EffectKind::TimeShaper, SR).unwrap();
                u.configure(spec);
                if let Some(need) = u.memory_need() {
                    drop(u.supply(cantus_engine::EffectMemory::build(need, SR)));
                }
                Direct::Stub(u)
            }
            EffectKind::LoFi => {
                Direct::Vetus(Box::new(cantus_dsp::vetus::Vetus::new(SR))) // fx:vetus
            } // fx:contract
            EffectKind::Utility => {
                Direct::Libra(Box::new(cantus_dsp::libra::Libra::new(
                    SR,
                    cantus_core::balance_gains,
                ))) // fx:libra
            } // fx:contract
            // fx3:contract — wave 3's stubs: the engine's units (each builder names its DSP
            // here and drops its Stub arm; the board stays the engine's own unit).
            EffectKind::Board => {
                let s = board_spec(spec);
                let mut u = cantus_engine::fx::tabula::Unit::build(&s, SR);
                use cantus_engine::FxUnit;
                if let Some(need) = u.memory_need() {
                    drop(u.supply(cantus_engine::EffectMemory::build(need, SR)));
                }
                Direct::Stub(Box::new(u))
            } // fx3:tabula
            EffectKind::PitchShift => Direct::Saltus(Box::new(cantus_dsp::saltus::Saltus::new(SR))), // fx3:saltus
            EffectKind::Rotary => Direct::Rota(Box::new(cantus_dsp::rota::Rota::new(SR))), // fx3:rota
            EffectKind::Amp => Direct::Arca(Box::new(cantus_dsp::arca::Arca::new(SR))), // fx3:arca
            EffectKind::Vocoder => Direct::Loquax(Box::new(cantus_dsp::loquax::Loquax::new(SR))), // fxa:loquax
            EffectKind::Repair => Direct::Purus(Box::new(cantus_dsp::purus::Purus::new(SR))), // fxc:purus
            EffectKind::Reference => {
                Direct::Comparatio(Box::new(cantus_dsp::comparatio::Comparatio::new(SR)))
            } // fxc:comparatio
            EffectKind::Shimmer => Direct::Caelum(
                Box::new(cantus_dsp::caelum::Caelum::new(SR)),
                cantus_dsp::modfx_line::Glide::new(1.0, 10.0, SR, 1e-6),
            ), // fxa:caelum
            EffectKind::Glitch => Direct::Fractus(
                Box::new(cantus_dsp::fractus::Fractus::new(SR)),
                Box::new(cantus_dsp::fractus::Pattern::EMPTY),
            ), // fxa:fractus
            EffectKind::BusCompressor => Direct::Vinculum(
                Box::new(cantus_dsp::vinculum::Vinculum::new(SR)),
                cantus_dsp::modfx_line::Glide::new(1.0, 10.0, SR, 1e-6),
            ), // fxb:vinculum
            EffectKind::ResonanceSuppressor => {
                Direct::Lenis(Box::new(cantus_dsp::lenis::Lenis::new(SR)))
            } // fxb:lenis
            EffectKind::Ducker => Direct::Cedo(Box::new(cantus_dsp::cedo::Cedo::new(
                SR,
                cantus_engine::fx::fluctus::shape_at,
            ))), // fxb:cedo
            // r41:nitor — Nitor's DSP itself (the plug-in has no song: its own key).
            EffectKind::Enhancer => Direct::Nitor(Box::new(cantus_dsp::nitor::Nitor::new(SR))), // r41:nitor
            k => panic!("{k:?} is not exported"),
        };
        d.set(spec);
        d.reset();
        d
    }

    pub fn set(&mut self, spec: &EffectSpec) {
        use cantus_engine::fx;
        match self {
            Direct::Eq(e) => {
                let solo = spec.get("solo").round();
                e.set_options(cantus_dsp::aequalis::Options {
                    output_db: spec.get("out"),
                    mode: cantus_dsp::aequalis::Mode::from_index(spec.get("mode").round() as usize),
                    resolution: cantus_dsp::aequalis::Resolution::from_index(
                        spec.get("lp_res").round() as usize,
                    ),
                    auto_gain: spec.get("auto") >= 0.5,
                    solo: (solo >= 1.0).then(|| solo as usize - 1),
                });
                e.set(&cantus_engine::eq_bands(spec), spec.get("out"));
            }
            Direct::Comp(c) => c.set(&cantus_engine::comp_settings(spec)),
            Direct::Lim(x) => x.set(&fx::limiter::settings(spec)),
            Direct::Space(x) => x.set(&fx::space::settings(spec)),
            Direct::Mb(x) => x.set(&fx::multiband::settings(spec)),
            Direct::Ds(x) => x.set(&fx::deesser::settings(spec)),
            Direct::Gate(x) => x.set(&fx::gate::settings(spec)),
            Direct::Stereo(x) => x.set(&fx::stereo::settings(spec)),
            Direct::Max(x) => x.set(&fx::maximizer::settings(spec)),
            Direct::Sat(x) => x.set(&fx::saturator::settings(spec)),
            Direct::Tonus(x) => x.set(&fx::tonus::settings(spec)),
            Direct::Tremor(x) => x.set(&fx::tremor::settings(spec)),
            Direct::Fluctus(x) => x.set(&fx::fluctus::settings(spec)),
            Direct::Translatio(x) => x.set(&fx::translatio::settings(spec)),
            Direct::Anulus(x) => x.set(&fx::anulus::settings(spec)),
            Direct::Phasis(x) => x.set(&fx::phasis::settings(spec)),
            Direct::Flexus(x) => x.set(&fx::flexus::settings(spec)),
            Direct::Concentus(x) => x.set(&fx::concentus::settings(spec)),
            Direct::Vibrans(x) => x.set(&fx::vibrans::settings(spec)),
            Direct::Tonsor(x) => x.set(&fx::tonsor::settings(spec)),
            Direct::Impetus(x) => x.set(&fx::impetus::settings(spec)),
            Direct::Discrimen(x) => x.set(&fx::discrimen::settings(spec)),
            Direct::Stub(x) => x.configure(&board_spec(spec)), // fx:contract; fx3:tabula
            Direct::Cribrum(x) => x.set(&fx::cribrum::settings(spec)), // fx:cribrum
            Direct::Vetus(x) => x.set(&fx::vetus::settings(spec)), // fx:vetus
            Direct::Libra(x) => x.set(&fx::libra::settings(spec)), // fx:libra
            Direct::Saltus(x) => x.set(&fx::saltus::settings(spec)), // fx3:saltus
            Direct::Rota(x) => x.set(&fx::rota::settings(spec).0), // fx3:rota
            Direct::Loquax(x) => x.set(&fx::loquax::settings(spec)), // fxa:loquax
            // fxa:fractus — its values, then its lanes.
            // fxa:caelum — a synced pre-delay at 120 BPM (a host without a transport); the
            // output's target as the engine's unit sets it (0 dB not applied).
            Direct::Purus(x) => x.set(&fx::purus::settings(spec)), // fxc:purus
            Direct::Comparatio(x) => x.set(&fx::comparatio::settings(spec, SR, 0)), // fxc:comparatio
            Direct::Caelum(x, out) => {
                let mut s = fx::caelum::settings(spec);
                if let Some(beats) = fx::caelum::sync_beats(spec.get("psync")) {
                    s.predelay_ms = cantus_dsp::caelum::sync_ms(beats, 120.0);
                }
                x.set(&s);
                let db = spec.get("out");
                out.target = if db == 0.0 {
                    1.0
                } else {
                    cantus_dsp::math::db_to_gain(db.clamp(-24.0, 24.0))
                };
            }
            Direct::Fractus(x, pat) => {
                x.set(&fx::fractus::settings(spec));
                fx::fractus::pattern_into(spec.fractus.as_deref(), pat);
                x.set_pattern(pat);
            }
            // fx3:arca — the cabinet built as the engine builds it, supplied once.
            Direct::Arca(x) => {
                let (s, need) = fx::arca::settings(spec);
                if let (Some(n), false) = (need, x.has_cab()) {
                    if let Some(k) = fx::arca::build(n, SR).kernel {
                        let st = Box::new(cantus_dsp::aula_conv::State::new(k.cap()));
                        let _ = x.supply_cab(k, Some(st));
                    }
                }
                x.set(&s);
            }
            // fxb:vinculum — the output's target as the engine's unit sets it (0 dB not
            // applied).
            Direct::Vinculum(x, out) => {
                x.set(&fx::vinculum::settings(spec));
                let db = spec.get("out");
                out.target = if db == 0.0 {
                    1.0
                } else {
                    cantus_dsp::math::db_to_gain(db.clamp(-24.0, 24.0))
                };
            }
            Direct::Lenis(x) => x.set(&fx::lenis::settings(spec)), // fxb:lenis
            Direct::Nitor(x) => x.set(&fx::nitor::settings(spec, None)), // r41:nitor
            // fxb:cedo — the settings, then the shape's points.
            Direct::Cedo(x) => {
                x.set(&fx::cedo::settings(spec));
                let mut pts =
                    [cantus_dsp::fluctus::CurvePoint::default(); cantus_core::fx::cedo::MAX_POINTS];
                let n = fx::cedo::shape_points(spec, &mut pts);
                x.set_shape(&pts[..n]);
            }
            Direct::Wire => {}
            Direct::Delay(x) => {
                x.set(&cantus_engine::delay_settings(spec));
                if let Some(len) = x.need() {
                    drop(x.supply(Box::new(cantus_dsp::delay::Lines::new(len))));
                }
            }
            Direct::Aula(x, have) => {
                let want = fx::aula::want(spec);
                if *have != want {
                    let k = aula_kernel(spec, Some(x.convolver().cap()));
                    let state = (k.cap() != x.convolver().cap())
                        .then(|| Box::new(cantus_dsp::aula_conv::State::new(k.cap())));
                    let _ = x.supply(k, state);
                    *have = want;
                }
                x.set(&fx::aula::settings(spec));
            }
        }
    }

    pub fn reset(&mut self) {
        match self {
            Direct::Eq(x) => x.reset(),
            Direct::Comp(x) => x.reset(),
            Direct::Lim(x) => x.reset(),
            Direct::Space(x) => x.reset(),
            Direct::Mb(x) => x.reset(),
            Direct::Ds(x) => x.reset(),
            Direct::Gate(x) => x.reset(),
            Direct::Stereo(x) => x.reset(),
            Direct::Max(x) => x.reset(),
            Direct::Sat(x) => x.reset(),
            Direct::Tonus(x) => x.reset(),
            Direct::Tremor(x) => x.reset(),
            Direct::Fluctus(x) => x.reset(),
            Direct::Translatio(x) => x.reset(),
            Direct::Anulus(x) => x.reset(),
            Direct::Phasis(x) => x.reset(),
            Direct::Flexus(x) => x.reset(),
            Direct::Concentus(x) => x.reset(),
            Direct::Vibrans(x) => x.reset(),
            Direct::Tonsor(x) => x.reset(),
            Direct::Impetus(x) => x.reset(),
            Direct::Discrimen(x) => x.reset(),
            Direct::Stub(x) => x.reset(),       // fx:contract
            Direct::Cribrum(x) => x.reset(),    // fx:cribrum
            Direct::Vetus(x) => x.reset(),      // fx:vetus
            Direct::Libra(x) => x.reset(),      // fx:libra
            Direct::Saltus(x) => x.reset(),     // fx3:saltus
            Direct::Rota(x) => x.reset(),       // fx3:rota
            Direct::Arca(x) => x.reset(),       // fx3:arca
            Direct::Loquax(x) => x.reset(),     // fxa:loquax
            Direct::Fractus(x, _) => x.reset(), // fxa:fractus
            Direct::Purus(x) => x.reset(),      // fxc:purus
            Direct::Comparatio(x) => x.reset(), // fxc:comparatio
            Direct::Caelum(x, out) => {
                out.jump(out.target);
                x.reset();
            } // fxa:caelum
            Direct::Vinculum(x, out) => {
                out.jump(out.target);
                x.reset();
            } // fxb:vinculum
            Direct::Lenis(x) => x.restart(),    // fxb:lenis: as the engine's reset
            Direct::Cedo(x) => x.reset(),       // fxb:cedo
            Direct::Nitor(x) => x.reset(),      // r41:nitor
            Direct::Wire => {}
            Direct::Aula(x, _) => x.reset(),
            Direct::Delay(x) => x.reset(),
        }
    }

    pub fn latency(&self) -> usize {
        match self {
            Direct::Eq(x) => x.latency(),
            Direct::Comp(x) => x.latency(),
            Direct::Lim(x) => x.latency(),
            Direct::Space(x) => x.latency(),
            Direct::Mb(x) => x.latency(),
            Direct::Ds(x) => x.latency(),
            Direct::Gate(x) => x.latency(),
            Direct::Stereo(x) => x.latency(),
            Direct::Max(x) => x.latency(),
            Direct::Sat(x) => x.latency(),
            Direct::Tonus(x) => x.latency(),
            Direct::Tremor(x) => x.latency(),
            Direct::Fluctus(x) => x.latency(),
            Direct::Translatio(x) => x.latency(),
            Direct::Anulus(x) => x.latency(),
            Direct::Phasis(x) => x.latency(),
            Direct::Flexus(x) => x.latency(),
            Direct::Concentus(x) => x.latency(),
            Direct::Vibrans(x) => x.latency(),
            Direct::Tonsor(x) => x.latency(),
            Direct::Impetus(x) => x.latency(),
            Direct::Discrimen(x) => x.latency(),
            Direct::Stub(x) => x.latency(),        // fx:contract
            Direct::Cribrum(x) => x.latency(),     // fx:cribrum
            Direct::Vetus(x) => x.latency(),       // fx:vetus
            Direct::Libra(x) => x.latency(),       // fx:libra
            Direct::Saltus(x) => x.latency(),      // fx3:saltus
            Direct::Rota(_) => 0,                  // fx3:rota
            Direct::Arca(x) => x.latency(),        // fx3:arca
            Direct::Loquax(x) => x.latency(),      // fxa:loquax
            Direct::Fractus(x, _) => x.latency(),  // fxa:fractus
            Direct::Caelum(x, _) => x.latency(),   // fxa:caelum
            Direct::Vinculum(x, _) => x.latency(), // fxb:vinculum
            Direct::Lenis(x) => x.latency(),       // fxb:lenis
            Direct::Cedo(x) => x.latency(),        // fxb:cedo
            Direct::Nitor(x) => x.latency(),       // r41:nitor
            Direct::Purus(x) => x.latency(),       // fxc:purus
            Direct::Comparatio(x) => x.latency(),  // fxc:comparatio
            Direct::Wire => 0,
            Direct::Aula(x, _) => x.latency(),
            Direct::Delay(_) => 0,
        }
    }

    pub fn process(&mut self, l: &mut [f32], r: &mut [f32], key: Option<(&[f32], &[f32])>) {
        match self {
            Direct::Eq(x) => x.process_keyed(l, r, key),
            Direct::Comp(x) => x.process(l, r, key),
            Direct::Lim(x) => x.process(l, r, key),
            Direct::Space(x) => x.process(l, r, key),
            Direct::Mb(x) => x.process(l, r, key),
            Direct::Ds(x) => x.process(l, r, key),
            Direct::Gate(x) => x.process(l, r, key),
            Direct::Stereo(x) => x.process(l, r),
            Direct::Max(x) => x.process(l, r),
            Direct::Sat(x) => x.process(l, r),
            Direct::Tonus(x) => x.process(l, r),
            Direct::Tremor(x) => x.process(l, r),
            Direct::Fluctus(x) => x.process(l, r),
            Direct::Translatio(x) => x.process(l, r),
            Direct::Anulus(x) => x.process(l, r),
            Direct::Phasis(x) => x.process(l, r),
            Direct::Flexus(x) => x.process(l, r),
            Direct::Concentus(x) => x.process(l, r),
            Direct::Vibrans(x) => x.process(l, r),
            Direct::Tonsor(x) => x.process(l, r),
            Direct::Impetus(x) => x.process(l, r, key),
            Direct::Discrimen(x) => x.process(l, r),
            Direct::Stub(x) => x.process(l, r, key), // fx:contract
            Direct::Cribrum(x) => x.process(l, r, key), // fx:cribrum
            Direct::Vetus(x) => x.process(l, r),     // fx:vetus
            Direct::Libra(x) => x.process(l, r),     // fx:libra
            Direct::Saltus(x) => x.process(l, r),    // fx3:saltus
            Direct::Rota(x) => x.process(l, r),      // fx3:rota
            Direct::Arca(x) => x.process(l, r),      // fx3:arca
            Direct::Loquax(x) => x.process(l, r, key, &[]), // fxa:loquax
            Direct::Fractus(x, _) => x.process(l, r), // fxa:fractus
            Direct::Purus(x) => x.process(l, r),     // fxc:purus
            Direct::Comparatio(x) => x.process(l, r, None), // fxc:comparatio
            Direct::Caelum(x, out) => {
                x.process(l, r, key);
                if !(out.still() && out.value == 1.0) {
                    for (a, b) in l.iter_mut().zip(r.iter_mut()) {
                        let g = out.next();
                        *a *= g;
                        *b *= g;
                    }
                }
            } // fxa:caelum
            Direct::Vinculum(x, out) => {
                x.process_keyed(l, r, key);
                if !(out.still() && out.value == 1.0) {
                    for (a, b) in l.iter_mut().zip(r.iter_mut()) {
                        let g = out.next();
                        *a *= g;
                        *b *= g;
                    }
                }
            } // fxb:vinculum
            Direct::Lenis(x) => x.process(l, r),     // fxb:lenis
            Direct::Cedo(x) => x.process(l, r, key), // fxb:cedo
            Direct::Nitor(x) => x.process(l, r),     // r41:nitor
            Direct::Wire => {}
            Direct::Aula(x, _) => x.process(l, r),
            Direct::Delay(x) => x.process(l, r),
        }
    }
}

/// The first sample where two signals differ, and how many do.
pub fn diff(a: &[f32], b: &[f32]) -> Option<(usize, usize, f32, f32)> {
    assert_eq!(a.len(), b.len());
    let mut first = None;
    let mut count = 0;
    for (i, (x, y)) in a.iter().zip(b).enumerate() {
        if x.to_bits() != y.to_bits() {
            count += 1;
            first.get_or_insert((i, *x, *y));
        }
    }
    first.map(|(i, x, y)| (i, count, x, y))
}

#[track_caller]
pub fn assert_identical(what: &str, a: &[f32], b: &[f32]) {
    if let Some((i, count, x, y)) = diff(a, b) {
        panic!("{what}: {count} samples differ, the first at {i}: {x} against {y}");
    }
}

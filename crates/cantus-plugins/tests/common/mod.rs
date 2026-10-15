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
            transport: ptr::null(),
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

/// A host's input event list over parameter values.
pub struct EventList {
    pub clap: clap_input_events,
    // Boxed: `clap.ctx` points at the vector itself, which must not move with the list.
    #[allow(clippy::box_collection)]
    events: Box<Vec<clap_event_param_value>>,
}

impl EventList {
    pub fn new(events: &[Ev]) -> EventList {
        let mut v: Box<Vec<clap_event_param_value>> = Box::new(
            events
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
                .collect(),
        );
        let ctx = &mut *v as *mut Vec<clap_event_param_value> as *mut c_void;
        EventList {
            clap: clap_input_events {
                ctx,
                size: Some(list_size),
                get: Some(list_get),
            },
            events: v,
        }
    }
}

unsafe extern "C" fn list_size(l: *const clap_input_events) -> u32 {
    (*((*l).ctx as *const Vec<clap_event_param_value>)).len() as u32
}

unsafe extern "C" fn list_get(l: *const clap_input_events, i: u32) -> *const clap_event_header {
    let v = &*((*l).ctx as *const Vec<clap_event_param_value>);
    &v[i as usize].header
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
        EffectKind::Chorus => vec![("out", -6.0)],
        EffectKind::Flanger => vec![("out", -6.0)],
        EffectKind::Phaser => vec![("out", -6.0)],
        EffectKind::Tremolo => vec![("out", -6.0)],
        EffectKind::Vibrato => vec![("out", -6.0)],
        EffectKind::RingMod => vec![("out", -6.0)],
        EffectKind::FreqShift => vec![("out", -6.0)],
        EffectKind::VolumeShaper => vec![("out", -6.0)],
        EffectKind::Clipper => vec![("out", -6.0)],
        EffectKind::Transient => vec![("out", -6.0)],
        EffectKind::TransientSplit => vec![("out", -6.0)],
        EffectKind::Analyzer => vec![("out", -6.0)],
        EffectKind::AutoTune => vec![
            ("speed", 0.0),
            ("key", 2.0),
            ("scale", 1.0),
            ("throat", 2.0),
            ("vdepth", 25.0),
            ("n4", 3.0),
        ],
        _ => vec![],
    }
}

/// The plug-ins of the family.
pub fn kinds() -> Vec<EffectKind> {
    desc::ALL.iter().map(|d| d.kind).collect()
}

/// The spec a DAW slot would hold for these settings.
pub fn spec_with(kind: EffectKind, values: &[(&str, f32)]) -> EffectSpec {
    let mut spec = EffectSpec::new(kind);
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
    /// Round 28's stubs, until each builder names its DSP here: the engine's unit.
    Stub(Box<dyn cantus_engine::FxUnit>),
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
                Direct::Stub(cantus_engine::fx::make(EffectKind::Chorus, SR).unwrap())
            }
            EffectKind::Flanger => {
                Direct::Stub(cantus_engine::fx::make(EffectKind::Flanger, SR).unwrap())
            }
            EffectKind::Phaser => {
                Direct::Stub(cantus_engine::fx::make(EffectKind::Phaser, SR).unwrap())
            }
            EffectKind::Tremolo => {
                Direct::Stub(cantus_engine::fx::make(EffectKind::Tremolo, SR).unwrap())
            }
            EffectKind::Vibrato => {
                Direct::Stub(cantus_engine::fx::make(EffectKind::Vibrato, SR).unwrap())
            }
            EffectKind::RingMod => {
                Direct::Stub(cantus_engine::fx::make(EffectKind::RingMod, SR).unwrap())
            }
            EffectKind::FreqShift => {
                Direct::Stub(cantus_engine::fx::make(EffectKind::FreqShift, SR).unwrap())
            }
            EffectKind::VolumeShaper => {
                Direct::Stub(cantus_engine::fx::make(EffectKind::VolumeShaper, SR).unwrap())
            }
            EffectKind::Clipper => {
                Direct::Stub(cantus_engine::fx::make(EffectKind::Clipper, SR).unwrap())
            }
            EffectKind::Transient => {
                Direct::Stub(cantus_engine::fx::make(EffectKind::Transient, SR).unwrap())
            }
            EffectKind::TransientSplit => {
                Direct::Stub(cantus_engine::fx::make(EffectKind::TransientSplit, SR).unwrap())
            }
            EffectKind::Analyzer => {
                Direct::Stub(cantus_engine::fx::make(EffectKind::Analyzer, SR).unwrap())
            }
            EffectKind::AutoTune => Direct::Tonus(Box::new(cantus_dsp::tonus::Tonus::new(SR))),
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
            Direct::Stub(x) => x.configure(spec),
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
            Direct::Stub(x) => x.reset(),
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
            Direct::Stub(x) => x.latency(),
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
            Direct::Stub(x) => x.process(l, r, key),
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

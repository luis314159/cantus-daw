//! The CLAP entry point: `clap_entry`, a factory listing the plug-ins this binary
//! carries, and each instance's lifecycle and `process`.
//!
//! Written straight on the standard's C ABI (`clap-sys`), with nothing between the host's
//! call and [`Processor::process`]: the events are read once into a buffer made at
//! activation, the input is copied to the output when the host did not process in
//! place, and the engine's unit runs on the host's own buffers.
//!
//! Threads, as the standard assigns them: `init`, `activate`, `deactivate`, the
//! parameters' text and info, state, latency and the GUI on the main thread; `process`,
//! `reset`, `start/stop_processing` on the audio thread; never both on one instance at
//! once where the standard forbids it. The instance keeps the audio thread's half in an
//! `UnsafeCell` and touches it only where the standard says that thread owns it.

use crate::desc::{self, Descriptor, VENDOR};
use crate::ftz::DenormalGuard;
use crate::instance::{NoteIn, ParamEvent, Processor, Shared, NOTE_CAPACITY};
use clap_sys::entry::clap_plugin_entry;
use clap_sys::events::*;
use clap_sys::ext::latency::{clap_host_latency, CLAP_EXT_LATENCY};
use clap_sys::factory::plugin_factory::{clap_plugin_factory, CLAP_PLUGIN_FACTORY_ID};
use clap_sys::host::clap_host;
use clap_sys::plugin::{clap_plugin, clap_plugin_descriptor};
use clap_sys::process::*;
use clap_sys::version::{clap_version_is_compatible, CLAP_VERSION};
use std::cell::UnsafeCell;
use std::ffi::{c_char, c_void, CStr, CString};
use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicPtr, AtomicU32, Ordering};
use std::sync::{Arc, OnceLock};

mod ext;
mod gui;
pub mod notes;

/// How many parameter events one block can carry before the rest are taken at the
/// block's start. Far above what any host sends (a change per parameter per block).
const EVENT_CAPACITY: usize = 8192;

/// The descriptors, with the strings they point into kept alive for the life of the
/// library.
struct Descriptors {
    list: Vec<(&'static Descriptor, clap_plugin_descriptor)>,
    _strings: Vec<CString>,
    _features: Vec<Box<[*const c_char]>>,
}

// SAFETY: every pointer in the list points into `_strings` or `_features`, which are
// never changed after construction and live as long as the static that holds them.
unsafe impl Send for Descriptors {}
unsafe impl Sync for Descriptors {}

fn descriptors() -> &'static Descriptors {
    static D: OnceLock<Descriptors> = OnceLock::new();
    D.get_or_init(|| {
        let mut strings = Vec::new();
        let mut features = Vec::new();
        let mut list = Vec::new();
        let mut keep = |s: &str| {
            let c = CString::new(s).unwrap_or_default();
            let p = c.as_ptr();
            strings.push(c);
            p
        };
        for d in desc::compiled() {
            let feats: Vec<*const c_char> = d
                .clap_features
                .iter()
                .map(|f| keep(f))
                .chain(std::iter::once(ptr::null()))
                .collect();
            let feats = feats.into_boxed_slice();
            let cd = clap_plugin_descriptor {
                clap_version: CLAP_VERSION,
                id: keep(d.clap_id),
                name: keep(d.full_name),
                vendor: keep(VENDOR),
                url: keep(desc::URL),
                manual_url: keep(desc::MANUAL_URL),
                support_url: keep(desc::SUPPORT_URL),
                version: keep(desc::VERSION),
                description: keep(d.description),
                features: feats.as_ptr(),
            };
            features.push(feats);
            list.push((d, cd));
        }
        Descriptors {
            list,
            _strings: strings,
            _features: features,
        }
    })
}

/// The symbol every CLAP host looks for.
#[allow(non_upper_case_globals)]
#[no_mangle]
pub static clap_entry: clap_plugin_entry = clap_plugin_entry {
    clap_version: CLAP_VERSION,
    init: Some(entry_init),
    deinit: Some(entry_deinit),
    get_factory: Some(entry_get_factory),
};

unsafe extern "C" fn entry_init(_path: *const c_char) -> bool {
    true
}

unsafe extern "C" fn entry_deinit() {}

unsafe extern "C" fn entry_get_factory(id: *const c_char) -> *const c_void {
    if !id.is_null() && CStr::from_ptr(id) == CLAP_PLUGIN_FACTORY_ID {
        &FACTORY as *const clap_plugin_factory as *const c_void
    } else {
        ptr::null()
    }
}

static FACTORY: clap_plugin_factory = clap_plugin_factory {
    get_plugin_count: Some(factory_count),
    get_plugin_descriptor: Some(factory_descriptor),
    create_plugin: Some(factory_create),
};

unsafe extern "C" fn factory_count(_f: *const clap_plugin_factory) -> u32 {
    descriptors().list.len() as u32
}

unsafe extern "C" fn factory_descriptor(
    _f: *const clap_plugin_factory,
    index: u32,
) -> *const clap_plugin_descriptor {
    descriptors()
        .list
        .get(index as usize)
        .map_or(ptr::null(), |(_, d)| d as *const _)
}

unsafe extern "C" fn factory_create(
    _f: *const clap_plugin_factory,
    host: *const clap_host,
    id: *const c_char,
) -> *const clap_plugin {
    if host.is_null() || id.is_null() || !clap_version_is_compatible((*host).clap_version) {
        return ptr::null();
    }
    let id = CStr::from_ptr(id);
    let Some((d, cd)) = descriptors()
        .list
        .iter()
        .find(|(_, cd)| CStr::from_ptr(cd.id) == id)
    else {
        return ptr::null();
    };
    Instance::create(d, cd, host)
}

/// One plug-in instance. Boxed; the host holds `&plugin`, whose `plugin_data` points back
/// here.
pub(crate) struct Instance {
    plugin: clap_plugin,
    host: *const clap_host,
    pub(crate) shared: Arc<Shared>,
    /// The audio thread's half: built in `activate`, dropped in `deactivate`.
    audio: UnsafeCell<Option<Processor>>,
    /// The block's parameter events, read from the host's list.
    events: UnsafeCell<Vec<ParamEvent>>,
    /// The block's notes, for a kind that hears them (`notes.rs`).
    notes: UnsafeCell<Vec<NoteIn>>,
    /// A right channel for a host that hands a mono output.
    scratch: UnsafeCell<Vec<f32>>,
    host_latency: AtomicPtr<clap_host_latency>,
    /// The latency the host was last told.
    reported_latency: AtomicU32,
    restart_asked: AtomicBool,
    pub(crate) gui: UnsafeCell<Option<ext::Gui>>,
    /// What an open editor did, for the audio thread to apply and send (`gui.rs`).
    edits_rx: UnsafeCell<rtrb::Consumer<crate::instance::Edit>>,
    // Read by the editor (`gui.rs`), which exists on Windows with the `editor` feature.
    #[cfg_attr(not(all(windows, feature = "editor")), allow(dead_code))]
    edits_tx: gui::EditTx,
}

impl Instance {
    unsafe fn create(
        d: &'static Descriptor,
        cd: &clap_plugin_descriptor,
        host: *const clap_host,
    ) -> *const clap_plugin {
        let (edits_tx, edits_rx) = rtrb::RingBuffer::new(gui::EDITS);
        let instance = Box::new(Instance {
            plugin: clap_plugin {
                desc: cd,
                plugin_data: ptr::null_mut(),
                init: Some(plugin_init),
                destroy: Some(plugin_destroy),
                activate: Some(plugin_activate),
                deactivate: Some(plugin_deactivate),
                start_processing: Some(plugin_start),
                stop_processing: Some(plugin_stop),
                reset: Some(plugin_reset),
                process: Some(plugin_process),
                get_extension: Some(ext::get_extension),
                on_main_thread: Some(plugin_on_main_thread),
            },
            host,
            shared: Shared::new(d),
            audio: UnsafeCell::new(None),
            events: UnsafeCell::new(Vec::new()),
            notes: UnsafeCell::new(Vec::new()),
            scratch: UnsafeCell::new(Vec::new()),
            host_latency: AtomicPtr::new(ptr::null_mut()),
            reported_latency: AtomicU32::new(0),
            restart_asked: AtomicBool::new(false),
            gui: UnsafeCell::new(None),
            edits_rx: UnsafeCell::new(edits_rx),
            edits_tx: Arc::new(std::sync::Mutex::new(edits_tx)),
        });
        let raw = Box::into_raw(instance);
        (*raw).plugin.plugin_data = raw as *mut c_void;
        &(*raw).plugin
    }

    /// The instance behind a host's pointer.
    ///
    /// # Safety
    /// `plugin` must be a pointer this module returned from `create`, not yet destroyed.
    pub(crate) unsafe fn from<'a>(plugin: *const clap_plugin) -> Option<&'a Instance> {
        if plugin.is_null() || (*plugin).plugin_data.is_null() {
            return None;
        }
        Some(&*((*plugin).plugin_data as *const Instance))
    }

    pub(crate) fn host(&self) -> &clap_host {
        // SAFETY: the host outlives every instance it created (the standard).
        unsafe { &*self.host }
    }

    /// A host extension, or null.
    pub(crate) unsafe fn host_extension<T>(&self, id: &CStr) -> *const T {
        match self.host().get_extension {
            Some(f) => f(self.host, id.as_ptr()) as *const T,
            None => ptr::null(),
        }
    }

    /// The audio half. Only where the standard gives this thread the instance.
    #[allow(clippy::mut_from_ref)]
    pub(crate) unsafe fn audio(&self) -> &mut Option<Processor> {
        &mut *self.audio.get()
    }

    /// Tell the host the latency changed, if it did — main thread, during `activate`
    /// (the only time the standard lets it change).
    unsafe fn report_latency(&self) {
        let now = self.shared.latency();
        let host_latency = self.host_latency.load(Ordering::Acquire);
        if self.reported_latency.swap(now, Ordering::AcqRel) != now && !host_latency.is_null() {
            if let Some(changed) = (*host_latency).changed {
                changed(self.host);
            }
        }
    }
}

unsafe extern "C" fn plugin_init(plugin: *const clap_plugin) -> bool {
    let Some(inst) = Instance::from(plugin) else {
        return false;
    };
    let host_latency = inst.host_extension::<clap_host_latency>(CLAP_EXT_LATENCY);
    inst.host_latency
        .store(host_latency as *mut clap_host_latency, Ordering::Release);
    true
}

unsafe extern "C" fn plugin_destroy(plugin: *const clap_plugin) {
    if let Some(inst) = Instance::from(plugin) {
        ext::gui_destroy_for(inst);
        drop(Box::from_raw(inst as *const Instance as *mut Instance));
    }
}

unsafe extern "C" fn plugin_activate(
    plugin: *const clap_plugin,
    sample_rate: f64,
    _min_frames: u32,
    max_frames: u32,
) -> bool {
    let Some(inst) = Instance::from(plugin) else {
        return false;
    };
    if !(sample_rate.is_finite() && sample_rate > 0.0) {
        return false;
    }
    // The old one first: it hands back the analyser tap the new one takes.
    *inst.audio() = None;
    *inst.audio() = Some(Processor::new(inst.shared.clone(), sample_rate as f32));
    let events = &mut *inst.events.get();
    events.clear();
    events.reserve(EVENT_CAPACITY);
    let notes = &mut *inst.notes.get();
    notes.clear();
    notes.reserve(NOTE_CAPACITY);
    let scratch = &mut *inst.scratch.get();
    *scratch = vec![0.0; (max_frames as usize).max(1)];
    inst.restart_asked.store(false, Ordering::Release);
    inst.shared.take_latency_change();
    inst.report_latency();
    true
}

unsafe extern "C" fn plugin_deactivate(plugin: *const clap_plugin) {
    if let Some(inst) = Instance::from(plugin) {
        *inst.audio() = None;
    }
}

unsafe extern "C" fn plugin_start(_plugin: *const clap_plugin) -> bool {
    true
}

unsafe extern "C" fn plugin_stop(_plugin: *const clap_plugin) {}

unsafe extern "C" fn plugin_reset(plugin: *const clap_plugin) {
    if let Some(p) = Instance::from(plugin).and_then(|i| i.audio().as_mut()) {
        p.reset();
    }
}

unsafe extern "C" fn plugin_on_main_thread(plugin: *const clap_plugin) {
    if let Some(inst) = Instance::from(plugin) {
        ext::gui_on_main_thread(inst);
    }
}

/// Read the host's parameter events into `out` (sorted by time, as the host sends them),
/// in the table's terms. Allocation-free: `out` was reserved in `activate`, and events
/// past its capacity are taken at their block's start rather than dropped.
///
/// # Safety
/// `list` must be a valid host event list for this call.
pub(crate) unsafe fn read_events(
    shared: &Shared,
    list: *const clap_input_events,
    out: &mut Vec<ParamEvent>,
) {
    out.clear();
    if list.is_null() {
        return;
    }
    let (Some(size), Some(get)) = ((*list).size, (*list).get) else {
        return;
    };
    let table = shared.table;
    for i in 0..size(list) {
        let h = get(list, i);
        if h.is_null() || (*h).space_id != CLAP_CORE_EVENT_SPACE_ID {
            continue;
        }
        let (id, cookie, value, modulation) = match (*h).type_ {
            CLAP_EVENT_PARAM_VALUE => {
                let e = &*(h as *const clap_event_param_value);
                (e.param_id, e.cookie, e.value, false)
            }
            CLAP_EVENT_PARAM_MOD => {
                let e = &*(h as *const clap_event_param_mod);
                (e.param_id, e.cookie, e.amount, true)
            }
            _ => continue,
        };
        // The cookie is the parameter's index + 1 (`params::get_info`): no search.
        let index = match cookie as usize {
            0 => match table.index_of(id) {
                Some(i) => i,
                None => continue,
            },
            c if c <= table.params.len() && table.params[c - 1].id == id => c - 1,
            _ => match table.index_of(id) {
                Some(i) => i,
                None => continue,
            },
        };
        let p = &table.params[index];
        let value = if modulation {
            value as f32
        } else {
            p.from_clap(value)
        };
        let ev = ParamEvent {
            offset: (*h).time,
            index: index as u32,
            value,
            modulation,
        };
        if out.len() < out.capacity() {
            out.push(ev);
        } else if let Some(first) = out.first_mut() {
            // Full: keep the latest value, at the block's start.
            *first = ParamEvent { offset: 0, ..ev };
        }
    }
}

unsafe extern "C" fn plugin_process(
    plugin: *const clap_plugin,
    process: *const clap_process,
) -> clap_process_status {
    let Some(inst) = Instance::from(plugin) else {
        return CLAP_PROCESS_ERROR;
    };
    let Some(proc_) = inst.audio().as_mut() else {
        return CLAP_PROCESS_ERROR;
    };
    if process.is_null() {
        return CLAP_PROCESS_ERROR;
    }
    let _denormals = DenormalGuard::new();
    let p = &*process;
    let n = p.frames_count as usize;
    let events = &mut *inst.events.get();
    read_events(&inst.shared, p.in_events, events);
    let notes = &mut *inst.notes.get();
    if inst.shared.desc.kind.takes_notes() {
        notes::read(p.in_events, notes);
    }
    // What the editor did since the last block, first.
    gui::drain_edits(inst, Some(&mut *proc_), p.out_events);

    if n > 0 && p.audio_outputs_count > 0 && !p.audio_outputs.is_null() {
        let out = &*p.audio_outputs;
        let input =
            (p.audio_inputs_count > 0 && !p.audio_inputs.is_null()).then(|| &*p.audio_inputs);
        let side = (p.audio_inputs_count > 1 && !p.audio_inputs.is_null())
            .then(|| &*p.audio_inputs.add(1))
            .filter(|b| b.channel_count > 0 && !b.data32.is_null());
        if out.channel_count == 0 || out.data32.is_null() {
            return CLAP_PROCESS_ERROR;
        }
        let out_l = *out.data32;
        let scratch = &mut *inst.scratch.get();
        let out_r = if out.channel_count > 1 {
            *out.data32.add(1)
        } else if scratch.len() >= n {
            scratch.as_mut_ptr()
        } else {
            return CLAP_PROCESS_ERROR;
        };
        // The input into the output, unless the host processes in place.
        match input.filter(|b| b.channel_count > 0 && !b.data32.is_null()) {
            Some(b) => {
                let in_l = *b.data32 as *const f32;
                let in_r = if b.channel_count > 1 {
                    *b.data32.add(1) as *const f32
                } else {
                    in_l
                };
                if !std::ptr::eq(in_l, out_l) {
                    ptr::copy(in_l, out_l, n);
                }
                if !std::ptr::eq(in_r, out_r) {
                    ptr::copy(in_r, out_r, n);
                }
            }
            None => {
                ptr::write_bytes(out_l, 0, n);
                ptr::write_bytes(out_r, 0, n);
            }
        }
        let key = side.map(|b| {
            let l = *b.data32 as *const f32;
            let r = if b.channel_count > 1 {
                *b.data32.add(1) as *const f32
            } else {
                l
            };
            (
                std::slice::from_raw_parts(l, n),
                std::slice::from_raw_parts(r, n),
            )
        });
        let l = std::slice::from_raw_parts_mut(out_l, n);
        let r = std::slice::from_raw_parts_mut(out_r, n);
        proc_.process_with_notes(l, r, key, events, notes);
    } else {
        // No audio (a parameter-only call): the values still land.
        proc_.process_with_notes(&mut [], &mut [], None, events, notes);
    }

    if inst.shared.take_latency_change() && !inst.restart_asked.swap(true, Ordering::AcqRel) {
        // The latency may only change while inactive: ask the host to restart us, and
        // `activate` reports it.
        if let Some(restart) = inst.host().request_restart {
            restart(inst.host);
        }
    }
    CLAP_PROCESS_CONTINUE
}

//! One CLAP plug-in instance, shared by its main-thread and audio-thread halves.
//!
//! The rules it keeps (CLAP `plugin.h`): `init`, `activate`, `deactivate` and `destroy` on
//! the main thread; `start_processing`, `process` and `stop_processing` on the audio
//! thread; never `deactivate` while processing. The halves share an `Arc<Instance>`, so it
//! is destroyed when the second of them is dropped — on the main thread, or, if that drop
//! happens anywhere else, in the next main-thread call that collects the leftovers
//! ([`collect_garbage`]).

use super::host::{on_main_thread, HostData};
use super::library::Library;
use crate::backend::{AudioSetup, HostError};
use clap_sys::ext::audio_ports::{
    clap_audio_port_info, clap_plugin_audio_ports, CLAP_AUDIO_PORT_IS_MAIN, CLAP_EXT_AUDIO_PORTS,
};
use clap_sys::ext::gui::{clap_plugin_gui, CLAP_EXT_GUI};
use clap_sys::ext::latency::{clap_plugin_latency, CLAP_EXT_LATENCY};
use clap_sys::ext::note_ports::{
    clap_note_port_info, clap_plugin_note_ports, CLAP_EXT_NOTE_PORTS, CLAP_NOTE_DIALECT_CLAP,
    CLAP_NOTE_DIALECT_MIDI,
};
use clap_sys::ext::params::{clap_plugin_params, CLAP_EXT_PARAMS};
use clap_sys::ext::state::{clap_plugin_state, CLAP_EXT_STATE};
use clap_sys::plugin::clap_plugin;
use std::ffi::{CStr, CString};
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Arc, Mutex};

/// Who is calling into the plug-in's processing side right now.
pub(crate) mod owner {
    /// Nobody: the audio half has not started processing (or has stopped).
    pub const NONE: u8 = 0;
    /// The audio half, between `start_processing` and `stop_processing`.
    pub const AUDIO: u8 = 1;
    /// The main half, flushing parameters while nothing processes.
    pub const MAIN: u8 = 2;
}

/// The plug-in's extensions, looked up once after `init`. Null where it has none.
#[derive(Copy, Clone)]
pub(crate) struct Extensions {
    pub params: *const clap_plugin_params,
    pub state: *const clap_plugin_state,
    pub latency: *const clap_plugin_latency,
    pub audio_ports: *const clap_plugin_audio_ports,
    pub note_ports: *const clap_plugin_note_ports,
    pub gui: *const clap_plugin_gui,
    /// What the host's run loop calls back (Linux's editors, ADR 0065).
    #[cfg(unix)]
    pub timer_support: *const clap_sys::ext::timer_support::clap_plugin_timer_support,
    #[cfg(unix)]
    pub posix_fd_support: *const clap_sys::ext::posix_fd_support::clap_plugin_posix_fd_support,
}

/// One audio port as the plug-in describes it.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Port {
    pub channels: u32,
    pub main: bool,
}

/// The plug-in's audio ports, and which of them Cantus feeds.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Ports {
    pub inputs: Vec<Port>,
    pub outputs: Vec<Port>,
    /// The input that gets the strip's signal, the output that goes back to it.
    pub main_in: Option<usize>,
    pub main_out: Option<usize>,
    /// The first input that is not the main one: the sidechain.
    pub side_in: Option<usize>,
    pub note_inputs: u32,
    /// What the first note input speaks (ADR 0028): CLAP note events, MIDI 1.0, or both.
    /// A port that declares neither is sent CLAP events.
    pub note_dialects: NoteDialects,
}

/// The note dialects the first input note port accepts.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct NoteDialects {
    pub clap: bool,
    pub midi: bool,
}

impl NoteDialects {
    /// Notes go as MIDI only when the port cannot take CLAP note events.
    pub fn notes_as_midi(self) -> bool {
        self.midi && !self.clap
    }
}

/// What lives exactly as long as the plug-in: its pointer, its host, its library.
pub(crate) struct Raw {
    pub plugin: *const clap_plugin,
    pub host: Box<HostData>,
    pub lib: Arc<Library>,
    pub active: bool,
}

// A raw instance moves between threads only to be destroyed on the main thread.
unsafe impl Send for Raw {}

impl Raw {
    /// Deactivate if needed and destroy. Main thread only.
    fn destroy(mut self) {
        // SAFETY: called on the main thread, with nothing processing (both halves gone).
        unsafe {
            let p = &*self.plugin;
            if self.active {
                if let Some(f) = p.deactivate {
                    f(self.plugin);
                }
                self.active = false;
            }
            if let Some(f) = p.destroy {
                f(self.plugin);
            }
        }
        // `host` and `lib` drop here, after the plug-in is gone.
    }
}

static GRAVEYARD: Mutex<Vec<Raw>> = Mutex::new(Vec::new());

/// Destroy the instances whose last half was dropped off the main thread. Main thread.
pub fn collect_garbage() {
    if !on_main_thread() {
        return;
    }
    let dead: Vec<Raw> = std::mem::take(&mut *GRAVEYARD.lock().unwrap_or_else(|e| e.into_inner()));
    for raw in dead {
        raw.destroy();
    }
}

pub(crate) struct Instance {
    raw: Option<Raw>,
    pub ext: Extensions,
    pub setup: AudioSetup,
    /// [`owner`]: who may call the processing side now.
    pub owner: AtomicU8,
    /// The audio half exists (made and not yet dropped).
    pub audio_alive: AtomicBool,
    pub active: AtomicBool,
}

// The plug-in's functions are called only on the threads CLAP names for them; the halves
// enforce that (see the module documentation).
unsafe impl Send for Instance {}
unsafe impl Sync for Instance {}

impl Instance {
    /// Create and `init` the plug-in `id` from `lib`. Main thread.
    pub fn create(lib: Arc<Library>, id: &str, setup: AudioSetup) -> Result<Instance, HostError> {
        super::host::mark_main_thread();
        let host = HostData::new();
        let c_id = CString::new(id).map_err(|_| HostError::NotFound(id.into()))?;
        let factory = lib.factory();
        let create = factory
            .create_plugin
            .ok_or_else(|| HostError::Load("factory without create_plugin".into()))?;
        // SAFETY: the factory's documented contract, on the main thread.
        let plugin = unsafe { create(lib.factory, host.raw(), c_id.as_ptr()) };
        if plugin.is_null() {
            return Err(HostError::NotFound(id.into()));
        }
        let raw = Raw {
            plugin,
            host,
            lib,
            active: false,
        };
        // SAFETY: a freshly created plug-in must be `init`ed before anything else.
        let ok = unsafe { (*plugin).init.is_some_and(|f| f(plugin)) };
        if !ok {
            raw.destroy();
            return Err(HostError::Instantiate(id.into()));
        }
        // SAFETY: `get_extension` may be called on the main thread once `init` returned.
        let ext = unsafe {
            let get = |name: &CStr| -> *const std::ffi::c_void {
                match (*plugin).get_extension {
                    Some(f) => f(plugin, name.as_ptr()),
                    None => std::ptr::null(),
                }
            };
            Extensions {
                params: get(CLAP_EXT_PARAMS).cast(),
                state: get(CLAP_EXT_STATE).cast(),
                latency: get(CLAP_EXT_LATENCY).cast(),
                audio_ports: get(CLAP_EXT_AUDIO_PORTS).cast(),
                note_ports: get(CLAP_EXT_NOTE_PORTS).cast(),
                gui: get(CLAP_EXT_GUI).cast(),
                #[cfg(unix)]
                timer_support: get(clap_sys::ext::timer_support::CLAP_EXT_TIMER_SUPPORT).cast(),
                #[cfg(unix)]
                posix_fd_support: get(clap_sys::ext::posix_fd_support::CLAP_EXT_POSIX_FD_SUPPORT)
                    .cast(),
            }
        };
        Ok(Instance {
            raw: Some(raw),
            ext,
            setup,
            owner: AtomicU8::new(owner::NONE),
            audio_alive: AtomicBool::new(false),
            active: AtomicBool::new(false),
        })
    }

    pub fn plugin(&self) -> *const clap_plugin {
        self.raw.as_ref().map_or(std::ptr::null(), |r| r.plugin)
    }

    pub fn host(&self) -> &HostData {
        &self
            .raw
            .as_ref()
            .expect("an instance has its host until dropped")
            .host
    }

    pub fn library(&self) -> &Library {
        &self
            .raw
            .as_ref()
            .expect("an instance has its library until dropped")
            .lib
    }

    /// The plug-in's audio and note ports. Main thread, while inactive or active.
    pub fn ports(&self) -> Ports {
        let mut ports = Ports::default();
        let plugin = self.plugin();
        if let Some(ap) = unsafe { self.ext.audio_ports.as_ref() } {
            if let (Some(count), Some(get)) = (ap.count, ap.get) {
                for input in [true, false] {
                    // SAFETY: the audio-ports extension, on the main thread.
                    let n = unsafe { count(plugin, input) };
                    for i in 0..n.min(16) {
                        // SAFETY: zeroed is a valid bit pattern for this plain C struct.
                        let mut info: clap_audio_port_info = unsafe { std::mem::zeroed() };
                        if !unsafe { get(plugin, i, input, &mut info) } {
                            continue;
                        }
                        let port = Port {
                            channels: info.channel_count,
                            main: info.flags & CLAP_AUDIO_PORT_IS_MAIN != 0,
                        };
                        if input {
                            ports.inputs.push(port);
                        } else {
                            ports.outputs.push(port);
                        }
                    }
                }
            }
        }
        let pick_main = |list: &[Port]| {
            list.iter()
                .position(|p| p.main && p.channels > 0)
                .or_else(|| list.iter().position(|p| p.channels > 0))
        };
        ports.main_in = pick_main(&ports.inputs);
        ports.main_out = pick_main(&ports.outputs);
        ports.side_in = ports
            .inputs
            .iter()
            .enumerate()
            .position(|(i, p)| Some(i) != ports.main_in && p.channels > 0);
        if let Some(np) = unsafe { self.ext.note_ports.as_ref() } {
            if let (Some(count), Some(get)) = (np.count, np.get) {
                // SAFETY: the note-ports extension, on the main thread.
                let n = unsafe { count(plugin, true) };
                for i in 0..n {
                    let mut info: clap_note_port_info = unsafe { std::mem::zeroed() };
                    if unsafe { get(plugin, i, true, &mut info) } {
                        if ports.note_inputs == 0 {
                            ports.note_dialects = NoteDialects {
                                clap: info.supported_dialects & CLAP_NOTE_DIALECT_CLAP != 0
                                    || info.supported_dialects == 0,
                                midi: info.supported_dialects & CLAP_NOTE_DIALECT_MIDI != 0,
                            };
                        }
                        ports.note_inputs += 1;
                    }
                }
            }
        }
        ports
    }

    /// `activate`. Main thread, with no audio half processing.
    pub fn activate(&mut self) -> Result<(), HostError> {
        let plugin = self.plugin();
        let setup = self.setup;
        // SAFETY: main thread; the plug-in is inactive.
        let ok = unsafe {
            (*plugin).activate.is_some_and(|f| {
                f(
                    plugin,
                    setup.sample_rate,
                    1,
                    setup.max_block.clamp(1, u32::MAX as usize) as u32,
                )
            })
        };
        if !ok {
            return Err(HostError::Activate(
                self.library().path.display().to_string(),
            ));
        }
        if let Some(raw) = self.raw.as_mut() {
            raw.active = true;
        }
        self.active.store(true, Ordering::Release);
        Ok(())
    }

    /// `deactivate`. Main thread, once the audio half has stopped processing.
    pub fn deactivate(&mut self) {
        if let Some(raw) = self.raw.as_mut() {
            if raw.active {
                // SAFETY: main thread; nothing is processing (the caller checked).
                unsafe {
                    if let Some(f) = (*raw.plugin).deactivate {
                        f(raw.plugin);
                    }
                }
                raw.active = false;
            }
        }
        self.active.store(false, Ordering::Release);
    }

    /// Samples of latency the plug-in reports. Main thread, while active.
    pub fn latency(&self) -> usize {
        match unsafe { self.ext.latency.as_ref() }.and_then(|l| l.get) {
            // SAFETY: the latency extension, on the main thread.
            Some(get) => unsafe { get(self.plugin()) as usize },
            None => 0,
        }
    }
}

impl Drop for Instance {
    fn drop(&mut self) {
        let Some(raw) = self.raw.take() else { return };
        if on_main_thread() {
            raw.destroy();
        } else {
            // Not ours to destroy here: the main thread does it on its next call.
            GRAVEYARD
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .push(raw);
        }
    }
}

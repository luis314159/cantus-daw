//! CLAP, the first format Cantus hosts (ADR 0019).
//!
//! Bindings: `clap-sys` 0.5 (CLAP 1.2.2), the ABI translated from the official headers with
//! nothing on top. The threading, the event lists and the lifetime of an instance are this
//! module's, because they are where Cantus's rules (ADR 0005, ADR 0018) meet the format's,
//! and no binding layer can make those choices for us.
//!
//! - [`library`]: a `.clap` file, loaded once and shared, `init`/`deinit` balanced.
//! - [`host`]: the `clap_host` and the host extensions (thread-check, params, state,
//!   latency, audio-ports, note-ports, gui, log): flags the main thread reads in `idle`.
//! - [`instance`]: one plug-in, shared by the two halves, destroyed on the main thread.
//! - [`main`] and [`audio`]: the two halves of the contract (`crate::backend`).
//! - [`events`]: fixed-capacity in/out event lists.

mod audio;

/// The CLAP transport event for `t` (the tests check what a plug-in is told, ADR 0060).
#[cfg(test)]
pub(crate) fn transport_event_for_tests(
    t: &crate::backend::TransportInfo,
) -> clap_sys::events::clap_event_transport {
    audio::transport_event(t)
}
mod events;
pub(crate) mod host;
mod instance;
mod library;
mod main;

pub use audio::ClapAudio;
pub use instance::collect_garbage;
pub use main::ClapMain;

use crate::backend::{AudioSetup, Backend, Format, HostError, Loaded, PluginInfo, PluginKind};
use clap_sys::plugin::clap_plugin_descriptor;
use instance::Instance;
use library::Library;
use std::ffi::{c_char, CStr};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// The CLAP format.
#[derive(Debug, Default)]
pub struct ClapBackend;

fn text(ptr: *const c_char) -> String {
    if ptr.is_null() {
        return String::new();
    }
    // SAFETY: descriptor strings are NUL-terminated and live as long as the library.
    unsafe { CStr::from_ptr(ptr) }
        .to_string_lossy()
        .into_owned()
}

fn features(desc: &clap_plugin_descriptor) -> Vec<String> {
    let mut out = Vec::new();
    if desc.features.is_null() {
        return out;
    }
    for i in 0..256 {
        // SAFETY: a NULL-terminated array of C strings.
        let f = unsafe { *desc.features.add(i) };
        if f.is_null() {
            break;
        }
        out.push(text(f));
    }
    out
}

fn kind_of(features: &[String]) -> PluginKind {
    let has = |f: &str| features.iter().any(|x| x == f);
    if has("instrument") {
        PluginKind::Instrument
    } else if has("audio-effect") {
        PluginKind::Effect
    } else if has("note-effect") {
        PluginKind::NoteEffect
    } else {
        PluginKind::Other
    }
}

/// The CLAP folders on Windows, in the order the CLAP entry header gives them.
#[cfg(windows)]
fn standard_folders() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(common) = std::env::var_os("COMMONPROGRAMFILES") {
        out.push(PathBuf::from(common).join("CLAP"));
    }
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        out.push(
            PathBuf::from(local)
                .join("Programs")
                .join("Common")
                .join("CLAP"),
        );
    }
    out
}

#[cfg(target_os = "macos")]
fn standard_folders() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(home) = std::env::var_os("HOME") {
        out.push(PathBuf::from(home).join("Library/Audio/Plug-Ins/CLAP"));
    }
    out.push(PathBuf::from("/Library/Audio/Plug-Ins/CLAP"));
    out
}

#[cfg(all(unix, not(target_os = "macos")))]
fn standard_folders() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(home) = std::env::var_os("HOME") {
        out.push(PathBuf::from(home).join(".clap"));
    }
    out.push(PathBuf::from("/usr/lib/clap"));
    // Where a plug-in built from source installs by default (`make install`'s prefix).
    out.push(PathBuf::from("/usr/local/lib/clap"));
    out
}

impl ClapBackend {
    /// The descriptors in `lib`, with the ports and editor each plug-in declares (which
    /// takes an instance: created, `init`ed, asked, destroyed).
    fn describe(&self, lib: &Arc<Library>) -> Vec<PluginInfo> {
        let factory = lib.factory();
        let (Some(count), Some(get)) = (factory.get_plugin_count, factory.get_plugin_descriptor)
        else {
            return Vec::new();
        };
        // SAFETY: the factory's documented contract, on the main thread.
        let n = unsafe { count(lib.factory) };
        let mut out = Vec::new();
        for i in 0..n {
            // SAFETY: as above.
            let desc = unsafe { get(lib.factory, i) };
            let Some(desc) = (unsafe { desc.as_ref() }) else {
                continue;
            };
            let features = features(desc);
            let mut info = PluginInfo {
                format: Format::Clap,
                id: text(desc.id),
                name: text(desc.name),
                vendor: text(desc.vendor),
                version: text(desc.version),
                path: lib.path.clone(),
                kind: kind_of(&features),
                main_inputs: 0,
                main_outputs: 0,
                sidechain: false,
                editor: false,
                features,
            };
            let setup = AudioSetup {
                sample_rate: 48_000.0,
                max_block: 1024,
            };
            if let Ok(inst) = Instance::create(lib.clone(), &info.id, setup) {
                let ports = inst.ports();
                info.main_inputs = ports.main_in.map_or(0, |p| ports.inputs[p].channels);
                info.main_outputs = ports.main_out.map_or(0, |p| ports.outputs[p].channels);
                info.sidechain = ports.side_in.is_some();
                info.editor = editor_supported(&inst);
            }
            out.push(info);
        }
        out
    }
}

/// Whether the plug-in offers an editor Cantus can show: embedded in a Win32 window (on
/// Windows) or an X11 one (on Linux, ADR 0065), or floating.
fn editor_supported(inst: &Instance) -> bool {
    let Some(gui) = (unsafe { inst.ext.gui.as_ref() }) else {
        return false;
    };
    let Some(supported) = gui.is_api_supported else {
        return false;
    };
    // macOS: the `cocoa` API, into an NSWindow of ours (ADR 0066); Linux: `x11` (ADR 0065).
    let api = if cfg!(target_os = "macos") {
        clap_sys::ext::gui::CLAP_WINDOW_API_COCOA.as_ptr()
    } else if cfg!(windows) {
        clap_sys::ext::gui::CLAP_WINDOW_API_WIN32.as_ptr()
    } else {
        clap_sys::ext::gui::CLAP_WINDOW_API_X11.as_ptr()
    };
    // SAFETY: the gui extension, on the main thread.
    cfg!(any(windows, target_os = "linux", target_os = "macos"))
        && unsafe { supported(inst.plugin(), api, false) || supported(inst.plugin(), api, true) }
}

impl Backend for ClapBackend {
    fn format(&self) -> Format {
        Format::Clap
    }

    fn search_paths(&self) -> Vec<PathBuf> {
        let mut out = standard_folders();
        // `CLAP_PATH` comes after the standard folders, as the entry header lists them.
        if let Some(extra) = std::env::var_os("CLAP_PATH") {
            out.extend(std::env::split_paths(&extra));
        }
        out
    }

    fn is_candidate(&self, path: &Path) -> bool {
        let is_clap = path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("clap"));
        // A file on Windows and Linux, a bundle (a folder) on macOS.
        is_clap && (path.is_file() || (cfg!(target_os = "macos") && path.is_dir()))
    }

    fn scan(&self, path: &Path) -> Result<Vec<PluginInfo>, HostError> {
        host::mark_main_thread();
        let lib = Library::open(path)?;
        Ok(self.describe(&lib))
    }

    fn load(&self, info: &PluginInfo, setup: AudioSetup) -> Result<Loaded, HostError> {
        host::mark_main_thread();
        instance::collect_garbage();
        let lib = Library::open(&info.path)?;
        let mut inst = Instance::create(lib, &info.id, setup)?;
        let ports = inst.ports();
        inst.activate()?;
        let inst = Arc::new(inst);
        let audio = ClapAudio::new(inst.clone(), ports.clone());
        let main = ClapMain::new(inst, info.clone(), ports);
        Ok(Loaded {
            main: Box::new(main),
            audio: Box::new(audio),
        })
    }
}

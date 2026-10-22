//! The VST3 backend: Steinberg's format behind [`crate::Backend`].
//!
//! Bindings are the `vst3` crate (coupler-rs, MIT OR Apache-2.0), generated from the VST
//! 3.8 SDK interface headers, which Steinberg publishes under the MIT licence. The design
//! notes — why these bindings, the state layout, how the controller's edits reach the
//! processor and the host, what is left — are in `README.md` next to this file.
//!
//! - [`module`]: bundle → binary, `InitDll`/`ExitDll`, the factory, one load per process;
//! - [`moduleinfo`]: `moduleinfo.json`, read without loading the binary;
//! - [`scan`]: the factory's classes, buses and editor;
//! - `instance`: create, connect, set up, activate — and tear down;
//! - `main_thread` / `audio_thread`: the two halves of [`crate::Loaded`];
//! - `host_objects` / `process_objects`: the COM objects the host hands the plug-in;
//! - [`params`]: plain ⇄ normalized on the audio thread;
//! - [`vstpreset`]: the state blob, a `.vstpreset` in memory.

#![cfg(feature = "vst3")]
// The SDK's untyped enums are `int` on Windows and `unsigned int` elsewhere, so the casts
// between them and the `int32` fields are no-ops here and needed there.
#![allow(clippy::unnecessary_cast)]

mod audio_thread;
mod host_objects;
mod instance;
mod main_thread;
pub mod module;
pub mod moduleinfo;
pub mod params;
mod process_objects;
pub mod scan;
pub mod util;
pub mod vstpreset;

use std::path::{Path, PathBuf};

use crate::backend::{AudioSetup, Backend, Format, HostError, Loaded, PluginInfo};

/// The VST3 format.
#[derive(Clone, Debug, Default)]
pub struct Vst3Backend;

impl Vst3Backend {
    pub fn new() -> Self {
        Vst3Backend
    }

    /// What `moduleinfo.json` says, without loading anything; `None` when the bundle has
    /// no such file. Buses and editor are assumed (stereo, an editor) and made exact by
    /// [`Backend::load`] ([`crate::MainThread::info`]).
    pub fn describe_without_loading(&self, path: &Path) -> Option<Vec<PluginInfo>> {
        path.is_dir().then(|| moduleinfo::read(path)).flatten()
    }

    /// Load the binary and describe every class from its factory, instantiating each to
    /// read its buses and editor, whether or not the bundle has a `moduleinfo.json`.
    pub fn scan_loading(&self, path: &Path) -> Result<Vec<PluginInfo>, HostError> {
        let module = module::open(path)?;
        Ok(scan::scan_module(&module, path))
    }
}

impl Backend for Vst3Backend {
    fn format(&self) -> Format {
        Format::Vst3
    }

    /// The per-user folder first, then the system-wide one (the SDK's "Plug-in
    /// Locations").
    fn search_paths(&self) -> Vec<PathBuf> {
        let env = |k: &str| std::env::var_os(k).map(PathBuf::from);
        let mut out = Vec::new();
        if cfg!(target_os = "windows") {
            if let Some(local) = env("LOCALAPPDATA") {
                out.push(local.join("Programs").join("Common").join("VST3"));
            }
            if let Some(common) = env("COMMONPROGRAMFILES") {
                out.push(common.join("VST3"));
            }
        } else if cfg!(target_os = "macos") {
            if let Some(home) = env("HOME") {
                out.push(home.join("Library/Audio/Plug-Ins/VST3"));
            }
            out.push(PathBuf::from("/Library/Audio/Plug-Ins/VST3"));
        } else {
            if let Some(home) = env("HOME") {
                out.push(home.join(".vst3"));
            }
            out.push(PathBuf::from("/usr/lib/vst3"));
            out.push(PathBuf::from("/usr/local/lib/vst3"));
            // Not the SDK's, but what Flatpak's Linux audio extensions and other hosts
            // read (ADR 0065): more folders, `:`-separated.
            if let Some(extra) = std::env::var_os("VST3_PATH") {
                out.extend(std::env::split_paths(&extra).filter(|p| p.is_absolute()));
            }
        }
        out
    }

    /// A `.vst3` bundle folder, or a legacy single `.vst3` file — but not the binary
    /// inside a bundle (`X.vst3/Contents/<arch>/X.vst3`), so a walker that meets the
    /// bundle first need not know to stop there.
    fn is_candidate(&self, path: &Path) -> bool {
        let named = path
            .extension()
            .is_some_and(|e| e.to_string_lossy().eq_ignore_ascii_case("vst3"));
        if !named {
            return false;
        }
        if path.is_dir() {
            return true;
        }
        let inside_bundle = path
            .parent()
            .and_then(Path::parent)
            .and_then(Path::file_name)
            .is_some_and(|n| n.eq_ignore_ascii_case("Contents"));
        path.is_file() && !inside_bundle
    }

    /// `moduleinfo.json` when the bundle has one (nothing is loaded), else the factory.
    fn scan(&self, path: &Path) -> Result<Vec<PluginInfo>, HostError> {
        match self.describe_without_loading(path) {
            Some(infos) => Ok(infos),
            None => self.scan_loading(path),
        }
    }

    fn load(&self, info: &PluginInfo, setup: AudioSetup) -> Result<Loaded, HostError> {
        if info.format != Format::Vst3 {
            return Err(HostError::NotFound(info.id.clone()));
        }
        main_thread::load(info, setup)
    }
}

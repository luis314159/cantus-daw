//! The hosted instruments of an open project (ADR 0028): the twin of the insert side of
//! [`PluginHost`], keyed by track instead of effect. A child of `session`, so it shares its
//! helpers and the host's tables.
//!
//! Each frame, [`PluginHost::sync`] loads the plug-in of every track whose instrument is
//! [`InstrumentSpec::External`] and has none, loads the project's state into it, and
//! delivers its audio half to the engine as an instrument unit
//! ([`EngineHandle::add_instrument`]) before the snapshot that needs it — a track whose
//! plug-in is not on this machine gets nothing, and plays silence; it unloads what left
//! (keeping its state for an undo); it loads a state the project changed. [`PluginHost::idle`]
//! serves the plug-ins' requests, restarts them, and records what their windows did.

use super::{changes, settle_new, PluginHost};
use crate::backend::{AudioThread, HostError, MainThread, ParamInfo, ParamOut, PluginInfo};
use crate::inst_unit::HostedInstrument;
use crate::session::HostNotice;
use crate::window::PluginWindow;
use crate::{backend_for, from_project_format, to_project_format};
use cantus_core::{ExternalPlugin, InstrumentSpec, PluginState, Project, TrackId};
use cantus_engine::{hosted_key, EngineHandle, NativeUnit};
use std::collections::{BTreeMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// One loaded instrument plug-in on a track.
pub struct HostedInst {
    pub info: PluginInfo,
    pub params: Vec<ParamInfo>,
    pub(super) main: Box<dyn MainThread>,
    /// The audio half, until it is delivered to the engine.
    pub(super) audio: Option<Box<dyn AudioThread>>,
    /// [`hosted_key`] of the plug-in: what the engine matches the unit by.
    pub(super) key: u64,
    pub(super) synced_state: PluginState,
    pub(super) flushed: BTreeMap<u32, f64>,
    pub(super) generation: Arc<AtomicU64>,
    pub(super) restarting: bool,
    pub(super) gestures: Vec<u32>,
    pub(super) window: Option<PluginWindow>,
}

impl HostedInst {
    pub fn param_text(&self, id: u32, value: f64) -> Option<String> {
        self.main.param_text(id, value)
    }

    pub fn param_value(&self, id: u32) -> Option<f64> {
        self.main.param_value(id)
    }

    pub fn latency(&self) -> usize {
        self.main.latency()
    }

    pub fn has_editor(&self) -> bool {
        self.main.has_editor()
    }

    pub fn window_open(&self) -> bool {
        self.window.is_some()
    }

    /// Whether its window is open but minimised to the taskbar (round 27, ADR 0051).
    pub fn window_minimised(&self) -> bool {
        self.window.as_ref().is_some_and(PluginWindow::is_minimised)
    }

    /// Whether its window is open and in front, nothing over it (round 27, ADR 0051).
    pub fn window_frontmost(&self) -> bool {
        self.window.as_ref().is_some_and(PluginWindow::is_frontmost)
    }

    /// The plug-in window's native handle, when one is open.
    pub fn window_handle(&self) -> Option<isize> {
        self.window.as_ref().and_then(|w| w.handle())
    }

    fn new(info: PluginInfo, main: Box<dyn MainThread>, audio: Box<dyn AudioThread>) -> Self {
        let key = hosted_key(&ExternalPlugin::new(
            to_project_format(info.format),
            info.id.clone(),
            "",
        ));
        HostedInst {
            params: main.params(),
            info,
            main,
            audio: Some(audio),
            key,
            synced_state: PluginState::default(),
            flushed: BTreeMap::new(),
            generation: Arc::new(AtomicU64::new(0)),
            restarting: false,
            gestures: Vec::new(),
            window: None,
        }
    }
}

/// The unit the engine runs for a loaded instrument.
pub(super) fn make_unit(
    h: &HostedInst,
    track: TrackId,
    audio: Box<dyn AudioThread>,
    sr: f64,
) -> NativeUnit {
    let unit = HostedInstrument::new(
        audio,
        &h.params,
        |id| h.main.param_value(id),
        h.main.latency(),
        sr,
        h.generation.clone(),
    );
    NativeUnit::hosted(track, h.key, Box::new(unit))
}

pub(super) fn deliver(engine: &mut EngineHandle, unit: NativeUnit) {
    if engine.knows_instrument(unit.track, unit.kind) {
        engine.replace_instrument(unit);
    } else {
        engine.add_instrument(unit);
    }
}

/// The values a fresh instance holds, for the project.
fn values_of(main: &dyn MainThread, params: &[ParamInfo]) -> BTreeMap<u32, f64> {
    params
        .iter()
        .filter(|p| !p.read_only)
        .filter_map(|p| main.param_value(p.id).map(|v| (p.id, v)))
        .collect()
}

impl PluginHost {
    /// The instrument plug-in loaded for a track, if any.
    pub fn hosted_instrument(&self, track: TrackId) -> Option<&HostedInst> {
        self.instruments.get(&track)
    }

    /// Why a track's hosted instrument has no plug-in, if it has none.
    pub fn instrument_missing(&self, track: TrackId) -> Option<&str> {
        self.inst_missing.get(&track).map(|(_, why)| why.as_str())
    }

    /// The spec that puts `info` on `track`, ready for `Command::SetInstrument`. The
    /// plug-in is loaded once, here, to read its parameters and its state as it starts;
    /// that instance then serves the track. What the track had loaded before is parked, so
    /// an undo brings it back with its sound.
    pub fn make_instrument(
        &mut self,
        track: TrackId,
        info: &PluginInfo,
    ) -> Result<InstrumentSpec, HostError> {
        let backend = backend_for(info.format).ok_or(HostError::Unsupported("format"))?;
        let loaded = backend.load(info, self.setup)?;
        let mut main = loaded.main;
        self.learn(main.as_ref());
        let info = main.info().clone();
        let state = main.save_state().map(PluginState::new).unwrap_or_default();
        let mut h = HostedInst::new(info.clone(), main, loaded.audio);
        let values = values_of(h.main.as_ref(), &h.params);
        let mut plugin =
            ExternalPlugin::new(to_project_format(info.format), info.id.clone(), info.name);
        plugin.vendor = info.vendor.clone();
        plugin.state = state.clone();
        plugin.params = values.clone();
        h.synced_state = state;
        h.flushed = values;
        self.unload_instrument(track);
        self.inst_missing.remove(&track);
        self.instruments.insert(track, h);
        Ok(InstrumentSpec::External(plugin))
    }

    /// Load a track's plug-in the host has not seen, from the database by format and id,
    /// with the project's state (or the one it had when it was last unloaded) and values.
    fn load_instrument(
        &mut self,
        track: TrackId,
        plugin: &ExternalPlugin,
        key: u64,
    ) -> Result<HostedInst, String> {
        let format = from_project_format(plugin.format);
        let info = self
            .db
            .find(format, &plugin.id)
            .cloned()
            .ok_or_else(|| "not installed on this computer".to_string())?;
        let backend = backend_for(format).ok_or_else(|| "format not hosted".to_string())?;
        let loaded = backend.load(&info, self.setup).map_err(|e| e.to_string())?;
        let mut main = loaded.main;
        let state = self
            .inst_parked
            .remove(&(track, key))
            .unwrap_or_else(|| plugin.state.clone());
        if !state.is_empty() {
            if let Err(e) = main.load_state(state.bytes()) {
                log::warn!("{}: {e}", info.name);
            }
        }
        let mut out = ParamOut::new();
        main.flush(&changes(&plugin.params), &mut out);
        let audio = settle_new(&mut main, loaded.audio, &info, self.setup, &plugin.params)
            .ok_or_else(|| "it could not restart after its state was loaded".to_string())?;
        self.learn(main.as_ref());
        let mut h = HostedInst::new(main.info().clone(), main, audio);
        h.synced_state = plugin.state.clone();
        h.flushed = plugin.params.clone();
        Ok(h)
    }

    /// Unload a track's instrument, keeping its state for a track that gets it back.
    pub(crate) fn unload_instrument(&mut self, track: TrackId) {
        if let Some(mut h) = self.instruments.remove(&track) {
            if let Some(w) = h.window.take() {
                h.main.close_editor();
                // The window goes after the editor closed (on Windows it destroys the
                // native window; elsewhere it is a placeholder with nothing to drop).
                #[cfg_attr(not(windows), allow(clippy::drop_non_drop))]
                drop(w);
            }
            if let Ok(s) = h.main.save_state() {
                self.inst_parked.insert((track, h.key), PluginState::new(s));
            }
        }
    }

    /// The instrument half of [`Self::sync`].
    pub(crate) fn sync_instruments(
        &mut self,
        project: &Project,
        engine: &mut EngineHandle,
        notices: &mut Vec<HostNotice>,
    ) {
        let rate = self.setup.sample_rate;
        let mut seen = HashSet::new();
        for t in &project.tracks {
            let Some(plugin) = t.instrument.external() else {
                continue;
            };
            seen.insert(t.id);
            let key = hosted_key(plugin);
            // The track now hosts another plug-in: the old one goes (and is parked).
            if self.instruments.get(&t.id).is_some_and(|h| h.key != key) {
                self.unload_instrument(t.id);
            }
            if self.inst_missing.get(&t.id).is_some_and(|(k, _)| *k == key) {
                continue;
            }
            self.inst_missing.remove(&t.id);
            if !self.instruments.contains_key(&t.id) {
                match self.load_instrument(t.id, plugin, key) {
                    Ok(h) => {
                        self.instruments.insert(t.id, h);
                    }
                    Err(error) => {
                        log::warn!("instrument {}: {error}", plugin.name);
                        self.inst_missing.insert(t.id, (key, error.clone()));
                        notices.push(HostNotice::InstrumentLoadFailed {
                            track: t.id,
                            name: plugin.name.clone(),
                            error,
                        });
                        continue;
                    }
                }
            }
            let Some(h) = self.instruments.get_mut(&t.id) else {
                continue;
            };
            if plugin.state != h.synced_state {
                if !plugin.state.is_empty() {
                    if let Err(e) = h.main.load_state(plugin.state.bytes()) {
                        log::warn!("{}: {e}", h.info.name);
                    }
                }
                h.synced_state = plugin.state.clone();
                h.generation.fetch_add(1, Ordering::AcqRel);
            }
            if plugin.params != h.flushed {
                let delta: BTreeMap<u32, f64> = plugin
                    .params
                    .iter()
                    .filter(|(id, v)| h.flushed.get(id) != Some(v))
                    .map(|(id, v)| (*id, *v))
                    .collect();
                let mut out = ParamOut::new();
                h.main.flush(&changes(&delta), &mut out);
                h.flushed = plugin.params.clone();
            }
            if let Some(audio) = h.audio.take() {
                let unit = make_unit(h, t.id, audio, rate);
                deliver(engine, unit);
            }
        }
        let gone: Vec<TrackId> = self
            .instruments
            .keys()
            .filter(|id| !seen.contains(id))
            .copied()
            .collect();
        for id in gone {
            self.unload_instrument(id);
        }
        self.inst_missing.retain(|id, _| seen.contains(id));
    }
}

//! A hosted plug-in behind the engine's own contract, `cantus_engine::FxUnit`, so the
//! processor, delay compensation and the sidechain see it exactly as they see a native
//! effect (ADR 0019).
//!
//! **Parameters.** The project stores the plug-in's values by id (`ExternalPlugin::params`).
//! Each snapshot's values are compared, on the audio thread and without allocating, with
//! what the plug-in is known to hold; each difference becomes a `ParamChange` at the start
//! of the next block. What the plug-in's own window does comes back as `FxParamEvent`s for
//! the host to record as commands.
//!
//! **No echo.** A value the plug-in reported is what it holds, so the table takes it and a
//! snapshot that carries it back sends nothing. While the plug-in's own gesture on a
//! parameter is under way, the snapshots (which lag behind it) do not touch that parameter;
//! after the gesture, they wait until one carries the plug-in's final value — or until half
//! a second has passed, in case the host did not record it.

use crate::backend::{AudioThread, ParamChange, ParamEvent, ParamInfo, ParamOut, TransportInfo};
use cantus_core::EffectSpec;
use cantus_engine::fx::{FxParamEvent, FxUnit, SongPosition};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// A parameter as the audio thread tracks it.
#[derive(Copy, Clone, Debug)]
struct Known {
    id: u32,
    min: f64,
    max: f64,
    stepped: bool,
    /// What the plug-in holds, as far as the host knows; NaN when unknown.
    value: f64,
    /// The plug-in's own gesture on it is under way.
    gesture: bool,
    /// After the plug-in's own change: snapshots are held back until one carries this
    /// value, or until `settle_left` frames have passed.
    settle: Option<f64>,
    settle_left: usize,
    /// The last value a snapshot carried (NaN before the first): what the project wants,
    /// sent once nothing holds it back.
    wanted: f64,
}

impl Known {
    /// The project's value, if it differs from what the plug-in holds and nothing holds it
    /// back; the table then takes it.
    fn take_send(&mut self) -> Option<f64> {
        if self.gesture || self.settle.is_some() || !self.wanted.is_finite() {
            return None;
        }
        if self.wanted.to_bits() == self.value.to_bits() {
            return None;
        }
        self.value = self.wanted;
        Some(self.wanted)
    }
}

/// The parameter side of a hosted plug-in on the audio thread, shared by an insert
/// ([`HostedUnit`]) and an instrument (`crate::inst_unit::HostedInstrument`): what the
/// plug-in holds, what the project wants, the changes for the next block, and the plug-in's
/// own changes on their way to the host. Allocates only when made.
pub(crate) struct ParamSync {
    /// Sorted by id; fixed at construction.
    known: Vec<Known>,
    /// Changes waiting for the next block; never more than one per parameter.
    pub(crate) pending: Vec<ParamChange>,
    pub(crate) out: Box<ParamOut>,
    pub(crate) events: Vec<FxParamEvent>,
    /// Bumped by the host after it loads a state into the plug-in: everything the table
    /// knew is stale, so every value of the next snapshot is sent.
    generation: Arc<AtomicU64>,
    seen_generation: u64,
    settle_frames: usize,
}

impl ParamSync {
    /// Built on the main thread. `values` are the plug-in's current values (read from its
    /// main half after its state was loaded).
    pub(crate) fn new(
        params: &[ParamInfo],
        values: impl Fn(u32) -> Option<f64>,
        sample_rate: f64,
        generation: Arc<AtomicU64>,
    ) -> Self {
        let mut known: Vec<Known> = params
            .iter()
            .map(|p| Known {
                id: p.id,
                min: p.min.min(p.max),
                max: p.max.max(p.min),
                stepped: p.stepped,
                value: values(p.id).unwrap_or(f64::NAN),
                gesture: false,
                settle: None,
                settle_left: 0,
                wanted: f64::NAN,
            })
            .collect();
        known.sort_by_key(|k| k.id);
        known.dedup_by_key(|k| k.id);
        let seen_generation = generation.load(Ordering::Acquire);
        ParamSync {
            pending: Vec::with_capacity(known.len()),
            known,
            out: Box::new(ParamOut::new()),
            events: Vec::with_capacity(ParamOut::CAPACITY),
            generation,
            seen_generation,
            settle_frames: (sample_rate * 0.5) as usize,
        }
    }

    pub(crate) fn len(&self) -> usize {
        self.known.len()
    }

    fn find(&self, id: u32) -> Option<usize> {
        self.known.binary_search_by_key(&id, |k| k.id).ok()
    }

    /// A change for the next block; one per parameter, the latest value.
    fn queue(pending: &mut Vec<ParamChange>, id: u32, value: f64) {
        if let Some(p) = pending.iter_mut().find(|p| p.id == id) {
            p.value = value;
        } else if pending.len() < pending.capacity() {
            pending.push(ParamChange {
                offset: 0,
                id,
                value,
            });
        }
    }

    /// A snapshot's values: each one that differs from what the plug-in holds, and that
    /// nothing holds back, becomes a change for the next block.
    pub(crate) fn configure(&mut self, values: &BTreeMap<u32, f64>) {
        let generation = self.generation.load(Ordering::Acquire);
        if generation != self.seen_generation {
            self.seen_generation = generation;
            for k in &mut self.known {
                k.value = f64::NAN;
                k.settle = None;
            }
        }
        for (id, v) in values {
            let Some(i) = self.find(*id) else { continue };
            let k = &mut self.known[i];
            if !v.is_finite() {
                continue;
            }
            let value = v.clamp(k.min, k.max);
            k.wanted = if k.stepped { value.round() } else { value };
            if k.settle
                .is_some_and(|t| !k.gesture && t.to_bits() == k.wanted.to_bits())
            {
                k.settle = None; // the host caught up: from here on, snapshots rule
            }
            if let Some(send) = k.take_send() {
                Self::queue(&mut self.pending, *id, send);
            }
        }
    }

    /// After a block of `frames`: the plug-in's own changes into the table and out to the
    /// host, and the settle timers run down.
    pub(crate) fn after_process(&mut self, frames: usize) {
        self.pending.clear();
        self.take_out();
        for k in &mut self.known {
            if k.settle.is_some() && !k.gesture {
                k.settle_left = k.settle_left.saturating_sub(frames);
                if k.settle_left == 0 {
                    // The host never recorded the plug-in's value: the project's stands.
                    k.settle = None;
                    if let Some(send) = k.take_send() {
                        Self::queue(&mut self.pending, k.id, send);
                    }
                }
            }
        }
    }

    /// The plug-in's own changes: into the table, and out to the host.
    fn take_out(&mut self) {
        let settle = self.settle_frames;
        for e in self.out.events() {
            let (id, event) = match *e {
                ParamEvent::Begin(id) => (id, FxParamEvent::Begin(id)),
                ParamEvent::Value(id, v) => (id, FxParamEvent::Value(id, v)),
                ParamEvent::End(id) => (id, FxParamEvent::End(id)),
            };
            let at = self.known.binary_search_by_key(&id, |k| k.id).ok();
            if let Some(k) = at.map(|i| &mut self.known[i]) {
                match *e {
                    ParamEvent::Begin(_) => k.gesture = true,
                    ParamEvent::Value(_, v) => {
                        k.value = v;
                        k.settle = Some(v);
                        k.settle_left = settle;
                    }
                    ParamEvent::End(_) => {
                        k.gesture = false;
                        k.settle_left = settle;
                    }
                }
                // A snapshot during the gesture may already carry the final value: the
                // host has caught up, and nothing is held back any more.
                if k.settle
                    .is_some_and(|t| !k.gesture && t.to_bits() == k.wanted.to_bits())
                {
                    k.settle = None;
                }
            }
            if self.events.len() < self.events.capacity() {
                self.events.push(event);
            }
        }
        self.out.clear();
    }
}

pub struct HostedUnit {
    audio: Box<dyn AudioThread>,
    sync: ParamSync,
    latency: usize,
    transport: Option<TransportInfo>,
}

impl std::fmt::Debug for HostedUnit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HostedUnit")
            .field("params", &self.sync.len())
            .field("latency", &self.latency)
            .finish()
    }
}

impl HostedUnit {
    /// Built on the main thread. `values` are the plug-in's current values (read from its
    /// main half after its state was loaded), `latency` what it reports now.
    pub fn new(
        audio: Box<dyn AudioThread>,
        params: &[ParamInfo],
        values: impl Fn(u32) -> Option<f64>,
        latency: usize,
        sample_rate: f64,
        generation: Arc<AtomicU64>,
    ) -> Self {
        HostedUnit {
            audio,
            sync: ParamSync::new(params, values, sample_rate, generation),
            latency,
            transport: None,
        }
    }
}

/// The engine's song position as the backend contract carries it.
pub(crate) fn transport_of(p: &SongPosition) -> TransportInfo {
    TransportInfo {
        playing: p.playing,
        recording: false,
        tempo_bpm: p.tempo_bpm,
        position_beats: p.beats,
        position_samples: p.samples,
        bar_start_beats: p.bar_start_beats,
        time_sig_num: p.time_sig_num,
        time_sig_den: p.time_sig_den,
        loop_beats: p.loop_beats,
    }
}

impl FxUnit for HostedUnit {
    fn configure(&mut self, spec: &EffectSpec) {
        if let Some(plugin) = spec.external.as_deref() {
            self.sync.configure(&plugin.params);
        }
    }

    fn process(&mut self, left: &mut [f32], right: &mut [f32], key: Option<(&[f32], &[f32])>) {
        let frames = left.len();
        self.audio.process(
            crate::backend::Audio {
                left,
                right,
                key,
                transport: self.transport.as_ref(),
            },
            &self.sync.pending,
            &[],
            &mut self.sync.out,
        );
        // A plug-in may leave MXCSR changed: the next native unit renders with FTZ/DAZ as
        // the engine set them (ADR 0031 R9; a no-op outside a `DenormalGuard`).
        cantus_rt::fpenv::reassert();
        self.sync.after_process(frames);
    }

    fn reset(&mut self) {
        self.audio.reset();
        cantus_rt::fpenv::reassert();
    }

    fn latency(&self) -> usize {
        self.latency
    }

    fn set_position(&mut self, p: &SongPosition) {
        self.transport = Some(transport_of(p));
    }

    fn param_events(&self) -> &[FxParamEvent] {
        &self.sync.events
    }

    fn clear_param_events(&mut self) {
        self.sync.events.clear();
    }

    fn stop(&mut self) {
        self.audio.stop();
    }
}

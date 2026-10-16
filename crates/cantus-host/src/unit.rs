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
//! **Automation and modulation** (ADR 0057) arrive as timed values in the plug-in's
//! normalised domain, at their sample offsets in the next call: the unit maps each onto the
//! parameter's range, sends it when it differs from what the plug-in holds, and while a
//! parameter is driven it wins over the project's value (FL's override). When the engine
//! plans its automation again (a new snapshot), a parameter it no longer drives goes back
//! to the project's value.
//!
//! **No echo.** A value the plug-in reported is what it holds, so the table takes it and a
//! snapshot that carries it back sends nothing. While the plug-in's own gesture on a
//! parameter is under way, the snapshots (which lag behind it) do not touch that parameter;
//! after the gesture, they wait until one carries the plug-in's final value — or until half
//! a second has passed, in case the host did not record it.

use crate::backend::{AudioThread, ParamChange, ParamEvent, ParamInfo, ParamOut, TransportInfo};
use cantus_core::EffectSpec;
use cantus_engine::fx::{FxParamEvent, FxUnit, HostedValue, SongPosition};
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
    /// Automation or a modulator drives it (ADR 0057): the value they ask for. While set,
    /// the project's value waits.
    auto: Option<f64>,
    /// Driven since the engine last planned its automation.
    auto_seen: bool,
    /// What modulation swings around where no clip plays and the project stores no value
    /// (review 1): what the plug-in held when it was first driven, never what was sent.
    /// Taken once per drive and sent back when the drive lets go (review 2); NaN while
    /// nothing drives it.
    auto_base: f64,
}

impl Known {
    /// The project's value, if it differs from what the plug-in holds and nothing holds it
    /// back; the table then takes it.
    fn take_send(&mut self) -> Option<f64> {
        if self.auto.is_some() || self.gesture || self.settle.is_some() || !self.wanted.is_finite()
        {
            return None;
        }
        if self.wanted.to_bits() == self.value.to_bits() {
            return None;
        }
        self.value = self.wanted;
        Some(self.wanted)
    }

    /// Automation or modulation lets go of it (ADR 0057): the project's value again, or —
    /// where the project stores none — what the plug-in held before it was driven
    /// (review 2: a parameter released at its peak stayed there, and the next drive swung
    /// around the peak). Nothing while the plug-in's own gesture or its settling rules.
    fn release(&mut self) -> Option<f64> {
        self.auto = None;
        let base = std::mem::replace(&mut self.auto_base, f64::NAN);
        if self.wanted.is_finite() {
            return self.take_send();
        }
        if !base.is_finite()
            || self.gesture
            || self.settle.is_some()
            || base.to_bits() == self.value.to_bits()
        {
            return None;
        }
        self.value = base;
        Some(base)
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
    /// Automation's timed changes for the next block, in time order (ADR 0057).
    timed: Vec<ParamChange>,
    /// What the next block is handed: `pending` (all at offset 0), then `timed`.
    pub(crate) merged: Vec<ParamChange>,
    /// The engine planned its automation again: what is not driven by the next block
    /// goes back to the project's value.
    replanned: bool,
    /// Timed changes that did not fit, since the unit was made.
    pub(crate) dropped: u64,
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
                auto: None,
                auto_seen: false,
                auto_base: f64::NAN,
            })
            .collect();
        known.sort_by_key(|k| k.id);
        known.dedup_by_key(|k| k.id);
        let seen_generation = generation.load(Ordering::Acquire);
        ParamSync {
            pending: Vec::with_capacity(known.len()),
            timed: Vec::with_capacity(TIMED_CAPACITY),
            merged: Vec::with_capacity(known.len() + TIMED_CAPACITY),
            replanned: false,
            dropped: 0,
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

    /// Automation's value for parameter `id`, `offset` samples into the next block
    /// (ADR 0057): mapped from the normalised domain onto the parameter's range (the
    /// project's value as the base where nothing automates it), sent when it differs from
    /// what the plug-in holds. A released value hands the parameter back to the project.
    pub(crate) fn automate(&mut self, offset: usize, id: u32, v: HostedValue) {
        let Some(i) = self.find(id) else { return };
        let k = &mut self.known[i];
        k.auto_seen = true;
        let send = if v.is_released() {
            match k.auto {
                Some(_) => k.release(),
                None => None,
            }
        } else {
            let span = k.max - k.min;
            if k.auto.is_none() && !k.auto_base.is_finite() {
                k.auto_base = k.value;
            }
            let base = if v.pos.is_finite() {
                Some(f64::from(v.pos))
            } else {
                [k.wanted, k.auto_base]
                    .into_iter()
                    .find(|x| x.is_finite())
                    .map(|w| if span > 0.0 { (w - k.min) / span } else { 0.0 })
            };
            base.and_then(|b| {
                let norm = (b + f64::from(v.modulation)).clamp(0.0, 1.0);
                let mut plain = k.min + norm * span;
                if k.stepped {
                    plain = plain.round();
                }
                k.auto = Some(plain);
                (plain.to_bits() != k.value.to_bits()).then(|| {
                    k.value = plain;
                    plain
                })
            })
        };
        if let Some(value) = send {
            if self.timed.len() < self.timed.capacity() {
                self.timed.push(ParamChange {
                    offset: offset as u32,
                    id,
                    value,
                });
            } else {
                self.dropped += 1;
            }
        }
    }

    /// The engine planned its automation again (ADR 0057).
    pub(crate) fn replanned(&mut self) {
        self.replanned = true;
        for k in &mut self.known {
            k.auto_seen = false;
        }
    }

    /// What the next block is handed: the project's changes (offset 0), then automation's
    /// in time order.
    pub(crate) fn merge(&mut self) {
        self.merged.clear();
        self.merged.extend_from_slice(&self.pending);
        let room = self.merged.capacity() - self.merged.len();
        let fit = self.timed.len().min(room);
        self.dropped += (self.timed.len() - fit) as u64;
        self.merged.extend_from_slice(&self.timed[..fit]);
    }

    /// After a block of `frames`: the plug-in's own changes into the table and out to the
    /// host, and the settle timers run down.
    pub(crate) fn after_process(&mut self, frames: usize) {
        self.dropped += std::mem::take(&mut self.out.lost_in);
        self.pending.clear();
        self.timed.clear();
        self.merged.clear();
        if std::mem::take(&mut self.replanned) {
            // What the new plan does not drive is the project's again.
            for k in &mut self.known {
                if k.auto.is_some() && !k.auto_seen {
                    if let Some(send) = k.release() {
                        Self::queue(&mut self.pending, k.id, send);
                    }
                }
            }
        }
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

/// Automation's timed changes one block holds at most (the engine's own queue is as long).
const TIMED_CAPACITY: usize = cantus_engine::modulator::HOSTED_QUEUE;

/// What a hosted plug-in costs the thread that calls it (round 29, ADR 0063): the
/// nanoseconds spent in its calls and the frames they covered, summed on the audio thread
/// (relaxed atomics, no allocation) and read by the interface for the plug-in's CPU figure.
/// For an isolated plug-in this is its whole cost to Cantus, the round trip included.
#[derive(Debug, Default)]
pub struct CpuMeter {
    nanos: AtomicU64,
    frames: AtomicU64,
}

impl CpuMeter {
    #[inline]
    pub fn add(&self, nanos: u64, frames: u64) {
        self.nanos.fetch_add(nanos, Ordering::Relaxed);
        self.frames.fetch_add(frames, Ordering::Relaxed);
    }

    /// Nanoseconds and frames since it was made.
    pub fn read(&self) -> (u64, u64) {
        (
            self.nanos.load(Ordering::Relaxed),
            self.frames.load(Ordering::Relaxed),
        )
    }

    /// Times one call of `frames` frames.
    #[inline]
    pub(crate) fn time<R>(meter: Option<&CpuMeter>, frames: usize, f: impl FnOnce() -> R) -> R {
        match meter {
            Some(m) => {
                let t = std::time::Instant::now();
                let r = f();
                m.add(t.elapsed().as_nanos() as u64, frames as u64);
                r
            }
            None => f(),
        }
    }
}

pub struct HostedUnit {
    audio: Box<dyn AudioThread>,
    sync: ParamSync,
    latency: usize,
    transport: Option<TransportInfo>,
    meter: Option<Arc<CpuMeter>>,
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
            meter: None,
        }
    }

    /// Measures what its calls cost into `meter` (round 29).
    pub fn metered(mut self, meter: Arc<CpuMeter>) -> Self {
        self.meter = Some(meter);
        self
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
        self.sync.merge();
        let (audio, sync, transport) = (&mut self.audio, &mut self.sync, self.transport.as_ref());
        CpuMeter::time(self.meter.as_deref(), frames, || {
            audio.process(
                crate::backend::Audio {
                    left,
                    right,
                    key,
                    transport,
                },
                &sync.merged,
                &[],
                &mut sync.out,
            )
        });
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

    fn take_dropped(&mut self) -> u64 {
        std::mem::take(&mut self.sync.dropped)
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

    fn automate(&mut self, offset: usize, id: u32, value: HostedValue) {
        self.sync.automate(offset, id, value);
    }

    fn automation_replanned(&mut self) {
        self.sync.replanned();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(id: u32) -> ParamInfo {
        ParamInfo {
            id,
            name: "p".into(),
            group: String::new(),
            min: 0.0,
            max: 10.0,
            default: 5.0,
            stepped: false,
            automatable: true,
            hidden: false,
            read_only: false,
        }
    }

    #[test]
    fn modulation_of_a_parameter_the_project_never_stored_swings_around_its_value() {
        // Review 1: with no project value the base was the last modulated value, so a
        // constant modulation climbed at every grid point.
        let mut sync = ParamSync::new(&[info(3)], |_| Some(4.0), 48_000.0, Default::default());
        sync.configure(&BTreeMap::new());
        let v = HostedValue {
            pos: f32::NAN,
            modulation: 0.1,
        };
        let mut sent = Vec::new();
        for k in 0..5 {
            sync.automate(k * 16, 3, v);
            sync.merge();
            sent.extend(sync.merged.iter().map(|c| c.value));
            sync.after_process(64);
        }
        assert_eq!(sent.len(), 1, "one change, then still: {sent:?}");
        assert!(
            (sent[0] - 5.0).abs() < 1e-6,
            "4 + 0.1 of the range: {}",
            sent[0]
        );
    }

    /// One block: `drive` (modulation sums) at grid points, then what the block was handed.
    fn block(sync: &mut ParamSync, drive: &[f32]) -> Vec<f64> {
        for (k, m) in drive.iter().enumerate() {
            sync.automate(
                k * 16,
                3,
                HostedValue {
                    pos: f32::NAN,
                    modulation: *m,
                },
            );
        }
        sync.merge();
        let sent = sync.merged.iter().map(|c| c.value).collect();
        sync.after_process(64);
        sent
    }

    #[test]
    fn a_modulation_sum_of_zero_sends_an_unstored_parameter_back_to_its_base() {
        // Review 2: a step at 0 released the parameter, nothing was sent back (the project
        // stores no value), and the next step took the last modulated value as its base —
        // the base climbed every cycle.
        let mut sync = ParamSync::new(&[info(3)], |_| Some(4.0), 48_000.0, Default::default());
        sync.configure(&BTreeMap::new());
        let sent = block(&mut sync, &[0.075, 0.0, -0.025]);
        let want = [4.75, 4.0, 3.75];
        assert_eq!(sent.len(), 3, "{sent:?}");
        for (s, w) in sent.iter().zip(want) {
            assert!((s - w).abs() < 1e-6, "{sent:?} != {want:?}");
        }
        // Many cycles later it still swings around 4.
        for _ in 0..20 {
            block(&mut sync, &[0.075, 0.0, -0.025]);
        }
        let sent = block(&mut sync, &[0.075]);
        assert!((sent[0] - 4.75).abs() < 1e-6, "drifted: {sent:?}");
    }

    #[test]
    fn an_unstored_parameter_let_go_by_a_replan_returns_to_its_base_and_swings_there_again() {
        // Review 2: muting the modulator at its peak left the plug-in there for good, and
        // unmuting it swung around the peak.
        let mut sync = ParamSync::new(&[info(3)], |_| Some(4.0), 48_000.0, Default::default());
        sync.configure(&BTreeMap::new());
        let first = block(&mut sync, &[0.1]);
        assert!(
            first.len() == 1 && (first[0] - 5.0).abs() < 1e-6,
            "{first:?}"
        );
        // The new plan no longer drives it: the next block sends the base back.
        sync.replanned();
        assert_eq!(block(&mut sync, &[]), Vec::<f64>::new());
        let back = block(&mut sync, &[]);
        assert_eq!(back.len(), 1, "{back:?}");
        assert!((back[0] - 4.0).abs() < 1e-9, "{back:?}");
        // Driven again: around 4, not 5.
        let again = block(&mut sync, &[0.1]);
        assert!((again[0] - 5.0).abs() < 1e-6, "{again:?}");
    }
}

//! Audio I/O backend (cpal) and the UI-side [`EngineHandle`].
//!
//! The handle is the only thing the rest of the application touches: it owns the stream,
//! the message producer and the event consumer, caches the latest engine state, and keeps
//! effect instances in sync with the snapshots it sends.

use crate::effects::EffectUnit;
use crate::inst::UnitKind;
use crate::messages::*;
use crate::midi::MidiSender;
use crate::processor::Processor;
use crate::samples::SampleCache;
use cantus_core::{EffectId, Project, Snapshot, Ticks, TrackId};
use cpal::traits::{DeviceTrait, StreamTrait};
use rtrb::{Consumer, Producer, RingBuffer};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use thiserror::Error;

const MESSAGE_QUEUE: usize = 1024;
const EVENT_QUEUE: usize = 4096;
const MIDI_QUEUE: usize = 1024;

#[derive(Debug, Error)]
pub enum EngineError {
    #[error("no output device available")]
    NoOutputDevice,
    #[error("device configuration error: {0}")]
    Config(String),
    #[error("stream error: {0}")]
    Stream(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct DeviceInfo {
    pub name: String,
    pub is_default: bool,
}

/// Enumerate output devices for a settings dialog (the default driver; see
/// [`crate::devices::output_devices_on`] for another).
pub fn output_devices() -> Vec<DeviceInfo> {
    crate::devices::output_devices_on(None)
}

/// Requested stream settings. `None` means "device default".
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AudioConfig {
    /// The driver type by cpal's host name ("WASAPI", "ASIO"); `None` is the default.
    pub driver: Option<String>,
    pub device_name: Option<String>,
    /// The device recorded from (ADR 0025); `None` is the default input, and an empty
    /// string opens no input at all.
    pub input_device_name: Option<String>,
    pub sample_rate: Option<u32>,
    pub buffer_frames: Option<u32>,
    /// Live workers helping the audio callback (ADR 0031 §2.1); `None` is the machine's
    /// default (performance cores minus two), `Some(0)` the callback alone.
    pub workers: Option<usize>,
}

/// The longest a waiting Exemplar zone table is held back, from the first change that
/// waited (ADR 0050 §6, fourth review #2).
pub const HOLD_BOUND: std::time::Duration = std::time::Duration::from_millis(250);

pub struct EngineHandle {
    tx: Producer<EngineMessage>,
    rx: Consumer<EngineEvent>,
    midi: MidiSender,
    _stream: Option<cpal::Stream>,
    pub device_name: String,
    pub sample_rate: f64,
    pub channels: usize,
    /// What is being heard (ADR 0022): what [`Self::position`] returns.
    position: Ticks,
    /// Where the engine is, ahead of what is heard by the output's latency.
    engine_position: Ticks,
    playing: bool,
    meters: Meters,
    /// The modulators' live values (ADR 0057).
    modulation: crate::modulator::ModTelemetry,
    stats: EngineStats,
    send_failures: u64,
    /// Effect ids whose unit currently lives in the engine.
    known_effects: HashSet<EffectId>,
    /// For each track hosting a native or hosted instrument, the kind and serial of the
    /// unit the engine holds (or has been sent) for it.
    known_instruments: HashMap<TrackId, (UnitKind, u64)>,
    /// Every sample any snapshot has referenced, for the life of the session.
    pub samples: SampleCache,
    /// Where frozen renders are and the ones opened (ADR 0064): every snapshot compiled
    /// here plays the renders at hand.
    pub frozen: crate::freeze::FrozenStore,
    /// The newest snapshot, while some Exemplar's new zone table waits for its starts to be
    /// read ([`Self::set_snapshot`]), and since when it has waited (an absolute bound).
    pending: Option<Arc<Snapshot>>,
    pending_since: Option<std::time::Instant>,
    /// Zone tables sent before their starts were read, the bound reached (counted).
    tables_late: u64,
    /// Snapshots compiled (tests: nothing is recompiled every frame while one waits).
    compiles: u64,
    note_inputs: Vec<NoteInput>,
    /// The consumer end of the mix recorder's ring; `None` without an audio device.
    capture: Option<Consumer<f32>>,
    capture_target: CaptureTarget,
    /// The consumer end of the analyser's ring.
    analyser: Option<Consumer<f32>>,
    /// The consumer ends of the scope rings (ADR 0056), which Speculum watches each, and
    /// which were asked for since the last [`Self::release_unwatched_scopes`].
    scopes: Vec<Consumer<crate::fx::speculum::ScopeChunk>>,
    scope_ids: [Option<EffectId>; crate::fx::speculum::MAX_SCOPES],
    scope_seen: [bool; crate::fx::speculum::MAX_SCOPES],
    /// The analysed compressor's meter, the maxima of every block since the last take.
    comp_peak: [f32; 3],
    /// What hosted plug-ins' own windows did since the last take (ADR 0019).
    fx_params: Vec<(EffectId, crate::fx::FxParamEvent)>,
    /// What hosted instruments' own windows did (ADR 0028), oldest first.
    inst_params: Vec<(TrackId, crate::fx::FxParamEvent)>,
    /// Recording's state on this side (`crate::recorder`).
    pub(crate) rec: crate::recorder::HandleRecorder,
    /// The devices' side: the driver, the input stream, latencies, the loopback probe.
    pub(crate) dev: crate::devices::HandleDevices,
}

impl std::fmt::Debug for EngineHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EngineHandle")
            .field("device", &self.device_name)
            .field("sample_rate", &self.sample_rate)
            .field("position", &self.position)
            .finish()
    }
}

impl EngineHandle {
    /// Open the output device and start the audio thread, and the input next to it
    /// (ADR 0025): the driver `config.driver` names (the system's default when it is not
    /// there), the output device, and the input device at the output's rate. An input
    /// that cannot open leaves the output running and says why ([`Self::input_error`]).
    pub fn start(config: &AudioConfig) -> Result<Self, EngineError> {
        let host = crate::devices::host(config.driver.as_deref());
        let device = crate::devices::pick_output(host, config.device_name.as_deref())
            .map_err(EngineError::Config)?;
        let device_name = crate::devices::name_of(&device).unwrap_or_default();
        let default = device
            .default_output_config()
            .map_err(|e| EngineError::Config(crate::devices::describe_error(&e)))?;
        let channels = default.channels().max(1) as usize;
        // The rate asked for when the device can run at it, else its own — read back and
        // shown, never assumed (round 21: an ASIO driver may refuse, or report none).
        let (sample_rate, rate_refused) =
            crate::devices::output_rate(host, &device, &default, config.sample_rate);
        // `f32` when the device plays it at this rate, its own format otherwise (ASIO
        // devices often take only integers).
        let format = if default.sample_format() == cpal::SampleFormat::F32
            || device.supported_output_configs().is_ok_and(|mut it| {
                it.any(|r| {
                    r.sample_format() == cpal::SampleFormat::F32
                        && r.channels() as usize == channels
                        && r.contains_rate(sample_rate)
                })
            }) {
            cpal::SampleFormat::F32
        } else {
            default.sample_format()
        };
        let stream_config = cpal::StreamConfig {
            channels: channels as u16,
            sample_rate,
            // A size the device cannot take is left to the driver (and shown as it runs).
            buffer_size: crate::devices::buffer_request(
                config.buffer_frames,
                default.buffer_size(),
            ),
        };

        let (tx, rx_engine) = RingBuffer::<EngineMessage>::new(MESSAGE_QUEUE);
        let (tx_engine, rx) = RingBuffer::<EngineEvent>::new(EVENT_QUEUE);
        let (midi_tx, midi_rx) = RingBuffer::<MidiEvent>::new(MIDI_QUEUE);
        // The process leaves EcoQoS before any audio runs (ADR 0031 R1): a throttled core
        // under the pool's workers or the callback is a dropout. Process-wide, idempotent.
        static ECO: std::sync::Once = std::sync::Once::new();
        ECO.call_once(|| {
            log::info!(
                "real-time: {}",
                cantus_rt::thread::opt_out_of_power_throttling()
            )
        });
        let mut processor = Processor::new(sample_rate as f64, rx_engine, tx_engine, midi_rx);
        // The pool (ADR 0031 §2): its workers start now and wait for the stream's first
        // block, which tells them the period; each joins once it has promoted itself.
        processor.set_workers(match config.workers {
            None => crate::processor::Workers::auto(),
            Some(n) => crate::processor::Workers::live(n),
        });
        let (capture_tx, capture_rx) = RingBuffer::<f32>::new(CAPTURE_SAMPLES);
        processor.attach_capture(capture_tx);
        let (analyser_tx, analyser_rx) = RingBuffer::<f32>::new(ANALYSER_SAMPLES);
        processor.attach_analyser(analyser_tx);
        let (scope_tx, scope_rx) = scope_rings();
        processor.attach_scopes(scope_tx);
        // The callback's first block raises its own thread (ADR 0031 R1; nothing on ASIO).
        processor.promote_on_first_block(crate::processor::AudioHost::from_driver_name(
            host.id().name(),
        ));

        // The input, at the output's rate, into the processor's ring.
        let telemetry = Arc::new(crate::devices::DeviceTelemetry::default());
        let mut rec = crate::recorder::HandleRecorder::default();
        let (plan, mut input_error) =
            match crate::devices::plan_input(host, config, &device, sample_rate) {
                Ok(p) => (p, None),
                Err(e) => (None, Some(e)),
            };
        let (plan, probe_input) = match plan {
            Some((plan, rings)) => {
                processor.attach_input(rings.ring, plan.channels);
                processor.attach_input_clock(telemetry.input_clock.clone());
                (Some(plan), Some(rings.probe))
            }
            None => (None, None),
        };
        let (probe_tx, probe_rx) = RingBuffer::new(2);
        let (done_tx, done_rx) = RingBuffer::new(2);
        let routing = Arc::new(crate::routing::OutputRouting::default());
        let driver = crate::devices::OutputDriver {
            processor,
            channels,
            rate: sample_rate as f64,
            telemetry: telemetry.clone(),
            probe: crate::loopback::ProbeSlot {
                incoming: probe_rx,
                done: done_tx,
                input: probe_input,
                active: None,
                telemetry: telemetry.clone(),
            },
            scratch: Vec::new(),
            latency: Default::default(),
            routing: routing.clone(),
            tone: Default::default(),
        };
        // Both directions are built first, then started (round 21): an ASIO driver then
        // creates the buffers of both in one `ASIOCreateBuffers` before its first
        // `ASIOStart` (the patched cpal, third_party/README.md) instead of stopping and
        // rebuilding a running driver when the input is added — which stalled Realtek's
        // driver. WASAPI does not mind the order. The output still starts first, and an
        // input that cannot open or start leaves it running and says why.
        let stream = crate::devices::build_output(&device, stream_config, format, driver)
            .map_err(EngineError::Stream)?;
        let input = plan.and_then(|plan| {
            let (name, channels) = (plan.name.clone(), plan.channels);
            match plan.build(&telemetry, sample_rate) {
                Ok(stream) => Some((stream, name, channels)),
                Err(e) => {
                    input_error = Some(e);
                    None
                }
            }
        });
        let opened = std::time::Instant::now();
        stream
            .play()
            .map_err(|e| EngineError::Stream(crate::devices::describe_error(&e)))?;
        // If the input cannot start, the processor keeps a ring nothing fills (silence)
        // and the handle reports no input.
        let input_stream = input.and_then(|(stream, name, channels)| match stream.play() {
            Ok(()) => {
                log::info!("audio input: {name} @ {sample_rate} Hz, {channels} ch");
                rec.input_name = Some(name);
                rec.input_channels = channels;
                Some(stream)
            }
            Err(e) => {
                input_error = Some(format!("{name}: {}", crate::devices::describe_error(&e)));
                None
            }
        });
        if let Some(e) = &input_error {
            log::warn!("audio input: {e}");
        }
        let stream_buffer = stream.buffer_size().ok().filter(|n| *n > 0);
        let driver_name = host.id().name().to_string();
        log::info!(
            "audio started: {driver_name} {device_name} @ {sample_rate} Hz, {channels} ch, {format}, buffer {stream_buffer:?} (asked {:?})",
            config.buffer_frames
        );

        let mut handle = EngineHandle {
            tx,
            rx,
            midi: MidiSender::new(midi_tx),
            _stream: Some(stream),
            device_name,
            sample_rate: sample_rate as f64,
            channels,
            position: Ticks::ZERO,
            engine_position: Ticks::ZERO,
            playing: false,
            meters: Meters::default(),
            modulation: Default::default(),
            stats: EngineStats::default(),
            send_failures: 0,
            known_effects: HashSet::new(),
            known_instruments: HashMap::new(),
            samples: SampleCache::new(),
            frozen: crate::freeze::FrozenStore::default(),
            pending: None,
            pending_since: None,
            tables_late: 0,
            compiles: 0,
            note_inputs: Vec::new(),
            capture: Some(capture_rx),
            capture_target: CaptureTarget::Off,
            analyser: Some(analyser_rx),
            scopes: scope_rx,
            scope_ids: [None; crate::fx::speculum::MAX_SCOPES],
            scope_seen: [false; crate::fx::speculum::MAX_SCOPES],
            comp_peak: [0.0; 3],
            fx_params: Vec::new(),
            inst_params: Vec::new(),
            rec,
            dev: Default::default(),
        };
        handle.dev = crate::devices::HandleDevices {
            driver: driver_name,
            buffer_frames: config.buffer_frames,
            input_error,
            telemetry: Some(telemetry),
            input_stream,
            probe_tx: Some(probe_tx),
            probe_back: Some(done_rx),
            probe_sent: None,
            opened: Some(opened),
            stream_buffer,
            rate_refused,
            routing: Some(routing),
        };
        Ok(handle)
    }

    /// A handle and the processor behind it, with no audio device: the caller drives the
    /// processor (`Processor::process`) wherever it likes — a test standing in for the
    /// audio callback, so everything from the UI's side through the engine runs for real.
    pub fn manual(sample_rate: f64) -> (Self, Processor) {
        let (tx, rx_engine) = RingBuffer::<EngineMessage>::new(MESSAGE_QUEUE);
        let (tx_engine, rx) = RingBuffer::<EngineEvent>::new(EVENT_QUEUE);
        let mut processor = Processor::without_midi(sample_rate, rx_engine, tx_engine);
        let (scope_tx, scope_rx) = scope_rings();
        processor.attach_scopes(scope_tx);
        let mut handle = Self::dummy();
        handle.tx = tx;
        handle.rx = rx;
        handle.sample_rate = sample_rate;
        handle.scopes = scope_rx;
        (handle, processor)
    }

    /// A handle with no audio device (headless / tests / no sound card). Messages are
    /// accepted and silently discarded when the queue fills.
    pub fn dummy() -> Self {
        let (tx, _rx_engine) = RingBuffer::<EngineMessage>::new(MESSAGE_QUEUE);
        let (_tx_engine, rx) = RingBuffer::<EngineEvent>::new(EVENT_QUEUE);
        let (midi_tx, _midi_rx) = RingBuffer::<MidiEvent>::new(MIDI_QUEUE);
        EngineHandle {
            tx,
            rx,
            midi: MidiSender::new(midi_tx),
            _stream: None,
            device_name: "none".into(),
            sample_rate: 48_000.0,
            channels: 2,
            position: Ticks::ZERO,
            engine_position: Ticks::ZERO,
            playing: false,
            meters: Meters::default(),
            modulation: Default::default(),
            stats: EngineStats::default(),
            send_failures: 0,
            known_effects: HashSet::new(),
            known_instruments: HashMap::new(),
            samples: SampleCache::new(),
            frozen: crate::freeze::FrozenStore::default(),
            pending: None,
            pending_since: None,
            tables_late: 0,
            compiles: 0,
            note_inputs: Vec::new(),
            capture: None,
            capture_target: CaptureTarget::Off,
            analyser: None,
            scopes: Vec::new(),
            scope_ids: [None; crate::fx::speculum::MAX_SCOPES],
            scope_seen: [false; crate::fx::speculum::MAX_SCOPES],
            comp_peak: [0.0; 3],
            fx_params: Vec::new(),
            inst_params: Vec::new(),
            rec: Default::default(),
            dev: Default::default(),
        }
    }

    /// Deliver a unit built elsewhere (a hosted plug-in, ADR 0019) for an effect id the
    /// engine does not run yet. Call before [`Self::set_snapshot`] with the snapshot that
    /// holds the slot, which then leaves it alone.
    pub fn add_unit(&mut self, unit: EffectUnit) -> bool {
        let id = unit.id;
        if self
            .tx
            .push(EngineMessage::AddEffect(Box::new(unit)))
            .is_ok()
        {
            self.known_effects.insert(id);
            true
        } else {
            self.send_failures += 1;
            false
        }
    }

    /// Swap a new unit in for the one the engine runs with the same id; the old one comes
    /// back as garbage and the id stays known.
    pub fn replace_unit(&mut self, unit: EffectUnit) -> bool {
        let id = unit.id;
        if self
            .tx
            .push(EngineMessage::ReplaceEffect(Box::new(unit)))
            .is_ok()
        {
            self.known_effects.insert(id);
            true
        } else {
            self.send_failures += 1;
            false
        }
    }

    /// Whether the engine has (or is about to get) a unit for this effect.
    pub fn knows_effect(&self, id: EffectId) -> bool {
        self.known_effects.contains(&id)
    }

    /// What hosted plug-ins' own windows did since the last call, oldest first.
    pub fn take_fx_params(&mut self) -> Vec<(EffectId, crate::fx::FxParamEvent)> {
        std::mem::take(&mut self.fx_params)
    }

    /// What hosted instruments' own windows did since the last call, oldest first.
    pub fn take_inst_params(&mut self) -> Vec<(TrackId, crate::fx::FxParamEvent)> {
        std::mem::take(&mut self.inst_params)
    }

    /// Deliver an instrument unit built elsewhere (a hosted plug-in, ADR 0028) for a track.
    /// Call before [`Self::set_snapshot`] with the snapshot in which the track hosts it,
    /// which then leaves that track alone.
    pub fn add_instrument(&mut self, unit: crate::inst::NativeUnit) -> bool {
        let (track, kind, serial) = (unit.track, unit.kind, unit.serial);
        if self
            .tx
            .push(EngineMessage::AddInstrument(Box::new(unit)))
            .is_ok()
        {
            self.known_instruments.insert(track, (kind, serial));
            true
        } else {
            self.send_failures += 1;
            false
        }
    }

    /// Swap a new unit in for the one the engine runs for the same track and kind (a
    /// hosted instrument restarted); the old one comes back as garbage.
    pub fn replace_instrument(&mut self, unit: crate::inst::NativeUnit) -> bool {
        let (track, kind, serial) = (unit.track, unit.kind, unit.serial);
        if self
            .tx
            .push(EngineMessage::ReplaceInstrument(Box::new(unit)))
            .is_ok()
        {
            self.known_instruments.insert(track, (kind, serial));
            true
        } else {
            self.send_failures += 1;
            false
        }
    }

    /// Whether the engine has (or is about to get) a unit of `kind` for `track`.
    pub fn knows_instrument(&self, track: TrackId, kind: UnitKind) -> bool {
        self.known_instruments.get(&track).map(|k| k.0) == Some(kind)
    }

    /// The analysed compressor's input peak, output peak (linear) and gain reduction (dB),
    /// the largest of every block since the last call. A frame reads one value however
    /// many blocks went by, and a transient in any of them is kept.
    pub fn take_comp_meter(&mut self) -> [f32; 3] {
        std::mem::take(&mut self.comp_peak)
    }

    /// Point the analyser at an effect (the editor that is open), or at nothing.
    pub fn set_analyser(&mut self, effect: Option<cantus_core::EffectId>) {
        if let Some(a) = self.analyser.as_mut() {
            while a.pop().is_ok() {}
        }
        self.send(EngineMessage::SetAnalyser(effect));
    }

    /// Watch a Speculum (ADR 0056): from the next block its signal comes to a scope ring of
    /// its own, every frame of it. Ask every UI frame it is shown;
    /// [`Self::release_unwatched_scopes`] frees what nobody asked for. False when there is no
    /// ring for it: no audio device, or eight already watched.
    pub fn watch_scope(&mut self, effect: EffectId) -> bool {
        use crate::fx::speculum::MAX_SCOPES;
        if self.scopes.len() < MAX_SCOPES {
            return false;
        }
        if let Some(i) = self.scope_ids.iter().position(|s| *s == Some(effect)) {
            self.scope_seen[i] = true;
            return true;
        }
        let Some(i) = self.scope_ids.iter().position(Option::is_none) else {
            return false;
        };
        // What an earlier watcher left in the ring is not this one's.
        while self.scopes[i].pop().is_ok() {}
        self.scope_ids[i] = Some(effect);
        self.scope_seen[i] = true;
        self.send(EngineMessage::WatchScopes(self.scope_ids));
        true
    }

    /// Free the scope rings no Speculum asked for since the last call (once a UI frame).
    pub fn release_unwatched_scopes(&mut self) {
        let mut changed = false;
        for (id, seen) in self.scope_ids.iter_mut().zip(self.scope_seen.iter_mut()) {
            if id.is_some() && !*seen {
                *id = None;
                changed = true;
            }
            *seen = false;
        }
        if changed {
            self.send(EngineMessage::WatchScopes(self.scope_ids));
        }
    }

    /// The Speculums watched now.
    pub fn watched_scopes(&self) -> impl Iterator<Item = EffectId> + '_ {
        self.scope_ids.iter().flatten().copied()
    }

    /// The chunks of `effect`'s signal that came since the last call, oldest first; returns
    /// the frames a full ring lost among them (the editor says its measurements have a gap).
    /// Chunks a ring still held from a Speculum it served before are dropped.
    pub fn drain_scope(
        &mut self,
        effect: EffectId,
        mut f: impl FnMut(&crate::fx::speculum::ScopeChunk),
    ) -> u64 {
        let Some(i) = self.scope_ids.iter().position(|s| *s == Some(effect)) else {
            return 0;
        };
        let ring = &mut self.scopes[i];
        let n = ring.slots();
        let mut lost = 0u64;
        for _ in 0..n {
            match ring.pop() {
                Ok(c) if c.owner == effect => {
                    lost += u64::from(c.lost);
                    f(&c);
                }
                Ok(_) => {}
                Err(_) => break,
            }
        }
        lost
    }

    /// Chunks waiting in scope ring `i` (tests: a ring nobody watches stays empty).
    #[doc(hidden)]
    pub fn scope_ring_len(&self, i: usize) -> usize {
        self.scopes.get(i).map_or(0, |r| r.slots())
    }

    /// Move the analyser's (input, output) pairs into `out`. Called once a frame.
    pub fn drain_analyser(&mut self, out: &mut Vec<f32>) -> usize {
        let Some(a) = self.analyser.as_mut() else {
            return 0;
        };
        let n = a.slots();
        for _ in 0..n {
            match a.pop() {
                Ok(x) => out.push(x),
                Err(_) => break,
            }
        }
        n
    }

    /// Point the mix recorder at the master, a strip, or nothing. Whatever was waiting
    /// in the ring from the last target is thrown away first, so a take never starts
    /// with the tail of another.
    pub fn set_capture(&mut self, target: CaptureTarget) {
        if let Some(c) = self.capture.as_mut() {
            while c.pop().is_ok() {}
        }
        self.capture_target = target;
        self.send(EngineMessage::SetCapture(target));
    }

    pub fn capture_target(&self) -> CaptureTarget {
        self.capture_target
    }

    /// Move every recorded sample (interleaved stereo) into `out`. Called once a frame.
    pub fn drain_capture(&mut self, out: &mut Vec<f32>) -> usize {
        let Some(c) = self.capture.as_mut() else {
            return 0;
        };
        let n = c.slots();
        out.reserve(n);
        for _ in 0..n {
            match c.pop() {
                Ok(x) => out.push(x),
                Err(_) => break,
            }
        }
        n
    }

    /// Whether this handle can record at all (a real audio device is open).
    pub fn can_capture(&self) -> bool {
        self.capture.is_some()
    }

    pub fn is_live(&self) -> bool {
        self._stream.is_some()
    }

    /// Sender for hardware MIDI threads (see [`crate::midi::MidiInputs`]).
    pub fn midi_sender(&self) -> MidiSender {
        self.midi.clone()
    }

    pub fn send(&mut self, msg: EngineMessage) {
        if self.tx.push(msg).is_err() {
            self.send_failures += 1;
        }
    }

    /// Compile the project into a snapshot with its sample files attached.
    ///
    /// The one way the application should make a snapshot it means to *play* — here or
    /// offline — because `Snapshot::compile` on its own leaves every sample pad silent.
    /// Files that cannot be read are reported and the pad stays silent; a project must
    /// never fail to play because a sample moved.
    pub fn compile(&mut self, project: &Project, revision: u64) -> Arc<Snapshot> {
        // A live engine loads audio sources in the background (ADR 0025 §9); a manual
        // one (tests, renders) at once.
        if self.is_live() {
            self.samples.set_background(true);
            // Exemplar streams its long samples from disk (ADR 0050).
            self.samples.set_streaming(true);
        }
        self.compiles += 1;
        // Pitch renders are made on the pitch service's worker, never on this thread —
        // with a device or without (ADR 0054, review 1).
        self.samples.set_pitch_background(true);
        let mut snapshot = Snapshot::compile(project, revision);
        for (path, err) in self.samples.attach(&mut snapshot) {
            log::warn!("sample {}: {err}", path.display());
        }
        // Audio clips' sources, at the device's rate (ADR 0025 §9).
        let rate = self.sample_rate.round() as u32;
        for (path, err) in self.samples.attach_audio(&mut snapshot, rate) {
            log::warn!("audio {}: {err}", path.display());
        }
        // Frozen strips and tracks whose render is at hand play it (ADR 0064).
        if project.any_frozen() {
            self.frozen.attach(project, &mut snapshot, rate);
        }
        Arc::new(snapshot)
    }

    /// [`Self::compile`] for an offline render (export, stems): first waits for every
    /// instrument file still loading in the background, so the render plays what playing
    /// will once they are in — never the silence of a zone still on its way (ADR 0050 §7).
    pub fn compile_for_render(&mut self, project: &Project, revision: u64) -> Arc<Snapshot> {
        let _ = self.compile(project, revision);
        self.samples.wait_assets();
        // And the pitch renders the first compile asked for (ADR 0054): the file holds the
        // edits, never the take as sung.
        self.samples.wait_pitch(std::time::Duration::from_secs(300));
        // And the warp renders (ADR 0061), asked once the pitch renders they read are in:
        // the file holds the full renders, never a bridge.
        let _ = self.compile(project, revision);
        self.samples.wait_warp(std::time::Duration::from_secs(300));
        self.compile(project, revision)
    }

    /// Send a snapshot, first delivering units for any effect the engine does not have. It
    /// goes at once — mute, transport, arming, tempo, every other edit — except the new zone
    /// tables of Exemplars whose new starts are not read yet: those tracks keep the zones
    /// they had until the blocks are in the pool, so a note right after the edit plays as
    /// in memory (third review #6, fourth #2). They wait at most [`HOLD_BOUND`] from the
    /// first change that waited — never restarted by later ones — and then go anyway
    /// (counted: [`Self::tables_late`]); [`Self::flush_pending`] sends them, every frame,
    /// without compiling anything.
    pub fn set_snapshot(&mut self, snapshot: Arc<Snapshot>) {
        let unready = self.samples.unready_tracks();
        if unready.is_empty() {
            self.pending = None;
            self.pending_since = None;
            self.send_snapshot(snapshot);
            return;
        }
        let since = *self
            .pending_since
            .get_or_insert_with(std::time::Instant::now);
        if since.elapsed() >= HOLD_BOUND {
            self.pending = None;
            self.pending_since = None;
            self.tables_late += 1;
            self.send_snapshot(snapshot);
            return;
        }
        let now = self.with_previous_zones(&snapshot, &unready);
        self.pending = Some(snapshot);
        self.send_snapshot(now);
    }

    /// Send the newest snapshot, its waiting zone tables included, once their starts are
    /// read or the bound is reached (every frame; no compile). Returns whether one still
    /// waits.
    pub fn flush_pending(&mut self) -> bool {
        if self.pending.is_none() {
            return false;
        }
        let late = self
            .pending_since
            .is_some_and(|t| t.elapsed() >= HOLD_BOUND);
        if late || self.samples.starts_ready() {
            if late {
                self.tables_late += 1;
            }
            self.pending_since = None;
            if let Some(s) = self.pending.take() {
                self.send_snapshot(s);
            }
        }
        self.pending.is_some()
    }

    /// `snapshot` with the zone tables of `tracks` as the last snapshot sent had them (the
    /// zones' parameters and texts, and their samples); everything else as it is.
    fn with_previous_zones(
        &self,
        snapshot: &Arc<Snapshot>,
        tracks: &[cantus_core::TrackId],
    ) -> Arc<Snapshot> {
        use cantus_core::inst::sampler::zone_key;
        let Some(before) = self.rec.snapshot.as_ref() else {
            return snapshot.clone();
        };
        let mut s = (**snapshot).clone();
        for t in s.tracks.iter_mut().filter(|t| tracks.contains(&t.id)) {
            let Some(old) = before.tracks.iter().find(|o| o.id == t.id) else {
                continue;
            };
            let (
                cantus_core::InstrumentSpec::Native(new),
                cantus_core::InstrumentSpec::Native(was),
            ) = (&mut t.instrument, &old.instrument)
            else {
                continue;
            };
            if new.kind != was.kind {
                continue;
            }
            new.params.retain(|k, _| zone_key(k).is_none());
            new.params.extend(
                was.params
                    .iter()
                    .filter(|(k, _)| zone_key(k).is_some())
                    .map(|(k, v)| (k.clone(), *v)),
            );
            new.text.retain(|k, _| zone_key(k).is_none());
            new.text.extend(
                was.text
                    .iter()
                    .filter(|(k, _)| zone_key(k).is_some())
                    .map(|(k, v)| (k.clone(), v.clone())),
            );
            t.assets.clone_from(&old.assets);
        }
        Arc::new(s)
    }

    /// The last snapshot sent to the engine (tests).
    pub fn sent_snapshot(&self) -> Option<&Arc<Snapshot>> {
        self.rec.snapshot.as_ref()
    }

    /// Zone tables sent before their starts were read, the bound reached.
    pub fn tables_late(&self) -> u64 {
        self.tables_late
    }

    /// Snapshots compiled since the start.
    pub fn compiles(&self) -> u64 {
        self.compiles
    }

    /// Whether a snapshot waits for its zones' starts.
    pub fn snapshot_pending(&self) -> bool {
        self.pending.is_some()
    }

    fn send_snapshot(&mut self, snapshot: Arc<Snapshot>) {
        // An effect this snapshot no longer has is forgotten now, not when the engine hands
        // its unit back: a remove and its undo in quick succession then send a fresh unit
        // with the undo's snapshot (round 22, the soak builder's E1).
        self.known_effects
            .retain(|id| snapshot.effect_slots().any(|s| s.id == *id));
        // The strips' inserts and the master's (ADR 0026).
        for slot in snapshot.effect_slots() {
            if !self.known_effects.contains(&slot.id) {
                let unit = EffectUnit::new(slot, self.sample_rate as f32);
                if self
                    .tx
                    .push(EngineMessage::AddEffect(Box::new(unit)))
                    .is_ok()
                {
                    self.known_effects.insert(slot.id);
                } else {
                    self.send_failures += 1;
                }
            }
        }
        // Native instruments: a track that no longer hosts one (or is gone) forgets its
        // unit — the engine hands it back as the snapshot lands — and a track that needs one
        // it has not been sent gets it first.
        self.known_instruments.retain(|track, (kind, _)| {
            snapshot
                .tracks
                .iter()
                .any(|t| t.id == *track && UnitKind::of(&t.instrument) == Some(*kind))
        });
        let units = crate::inst::units_for(&snapshot, self.sample_rate as f32, |track, kind| {
            self.known_instruments.get(&track).map(|k| k.0) == Some(UnitKind::Native(kind))
        });
        for unit in units {
            let (track, kind, serial) = (unit.track, unit.kind, unit.serial);
            if self.tx.push(EngineMessage::AddInstrument(unit)).is_ok() {
                self.known_instruments.insert(track, (kind, serial));
            } else {
                self.send_failures += 1;
            }
        }
        // The graph's plan (ADR 0031 §1.2) is compiled here, off the audio thread, and only
        // when the routing differs from the last snapshot sent: a knob turn sends none.
        let key = crate::graph::topology_key(&snapshot);
        let routed = self
            .rec
            .snapshot
            .as_ref()
            .is_some_and(|s| crate::graph::topology_key(s) == key);
        self.rec.snapshot = Some(snapshot.clone());
        // The automation's runtime first: the engine pairs it with the snapshot (ADR 0027).
        self.send(EngineMessage::Automation(
            crate::automation::Runtime::build(&snapshot),
        ));
        if routed {
            self.send(EngineMessage::SetSnapshot(snapshot));
        } else {
            let plan = crate::graph::compile(&snapshot);
            self.send(EngineMessage::SetSnapshotPlanned(snapshot, plan));
        }
    }

    /// Ask for one track's instrument telemetry (its editor is open), or for none.
    pub fn set_instrument_probe(&mut self, track: Option<TrackId>) {
        self.send(EngineMessage::SetInstrumentProbe(track));
    }

    pub fn play(&mut self) {
        self.send(EngineMessage::Play);
    }

    pub fn stop(&mut self) {
        self.send(EngineMessage::Stop);
    }

    pub fn seek(&mut self, tick: Ticks) {
        self.position = tick;
        self.engine_position = tick;
        self.send(EngineMessage::Seek(tick));
    }

    /// Audition a decoded sample through the output (the sample browser), or fade the
    /// audition out with `None`.
    pub fn preview(&mut self, sample: Option<(Arc<cantus_core::SampleData>, f32)>) {
        self.send(EngineMessage::Preview(sample));
    }

    pub fn note_on(&mut self, track: TrackId, pitch: u8, velocity: u8) {
        self.send(EngineMessage::NoteOn {
            track,
            pitch,
            velocity,
        });
    }

    pub fn note_off(&mut self, track: TrackId, pitch: u8) {
        self.send(EngineMessage::NoteOff { track, pitch });
    }

    pub fn set_live_track(&mut self, track: Option<TrackId>) {
        self.send(EngineMessage::SetLiveTrack(track));
    }

    /// Tracks that play the live notes too (the armed instrument tracks of a recording
    /// pass); empty for none. At most `MAX_LIVE_ECHO` are taken.
    pub fn set_live_echo(&mut self, tracks: &[TrackId]) {
        let mut a = [None; crate::messages::MAX_LIVE_ECHO];
        for (o, t) in a.iter_mut().zip(tracks) {
            *o = Some(*t);
        }
        self.send(EngineMessage::SetLiveEcho(a));
    }

    pub fn set_recording(&mut self, on: bool) {
        self.send(EngineMessage::SetRecording(on));
    }

    pub fn panic(&mut self) {
        self.send(EngineMessage::Panic);
    }

    /// Drain events from the audio thread. Call once per UI frame. Returns how many
    /// events were processed.
    pub fn poll(&mut self) -> usize {
        self.sync_devices();
        let mut n = 0;
        // The master's true peak is the highest of every block drained, not the last's
        // (ADR 0052, review 1).
        let mut true_peak: Option<f32> = None;
        while let Ok(ev) = self.rx.pop() {
            n += 1;
            match ev {
                EngineEvent::Position {
                    tick,
                    heard,
                    playing,
                } => {
                    self.position = heard;
                    self.engine_position = tick;
                    self.playing = playing;
                }
                EngineEvent::Meters(m) => {
                    for (p, v) in self.comp_peak.iter_mut().zip(m.comp) {
                        *p = p.max(v);
                    }
                    self.rec.meter(&m);
                    let tp = m.master_loudness.true_peak;
                    true_peak = Some(true_peak.map_or(tp, |p| p.max(tp)));
                    self.meters = m;
                }
                EngineEvent::Stats(s) => {
                    self.rec.engine_latency(&s);
                    self.stats = s;
                }
                EngineEvent::NoteInput(note) => self.note_inputs.push(note),
                EngineEvent::Garbage(old) => drop(old),
                EngineEvent::GarbageEffect(unit) => {
                    // A replaced unit's id lives on in its successor; so does one whose id the
                    // last snapshot sent still has (a new unit went with it).
                    let resent = self
                        .rec
                        .snapshot
                        .as_ref()
                        .is_some_and(|s| s.effect_slots().any(|e| e.id == unit.id));
                    if !unit.replaced && !resent {
                        self.known_effects.remove(&unit.id);
                    }
                    drop(unit);
                }
                EngineEvent::FxParam { effect, event } => self.fx_params.push((effect, event)),
                EngineEvent::InstParam { track, event } => self.inst_params.push((track, event)),
                EngineEvent::GarbageInstrument(unit) => {
                    // Only forget it if it is the unit we think the engine holds: a unit
                    // replaced long ago can come back after its successor was sent.
                    if self.known_instruments.get(&unit.track).map(|k| k.1) == Some(unit.serial) {
                        self.known_instruments.remove(&unit.track);
                    }
                    drop(unit);
                }
                EngineEvent::PdcNeed { count, delay } => {
                    let lines = crate::pdc::lines_for(count, delay);
                    self.send(EngineMessage::PdcLines(lines));
                }
                EngineEvent::GarbageLines(lines) => drop(lines),
                EngineEvent::EffectNeed { effect, need } => {
                    let memory = crate::effects::EffectMemory::build(need, self.sample_rate as f32);
                    self.send(EngineMessage::EffectMemory { effect, memory });
                }
                EngineEvent::GarbageMemory(memory) => drop(memory),
                EngineEvent::Record(mark) => self.rec.mark(mark),
                EngineEvent::GarbageRecorder(rings) => self.rec.rings_back(rings),
                EngineEvent::GarbagePreview(sample) => drop(sample),
                EngineEvent::GarbageAutomation(rt) => drop(rt),
                EngineEvent::Modulation(t) => self.modulation = t,
                EngineEvent::GarbagePlan(plan) => drop(plan),
            }
        }
        if let Some(tp) = true_peak {
            self.meters.master_loudness.true_peak = tp;
        }
        n
    }

    /// Live notes recorded since the last call.
    pub fn take_note_inputs(&mut self) -> Vec<NoteInput> {
        std::mem::take(&mut self.note_inputs)
    }

    /// The position being **heard**: the engine's own less the output's latency (ADR
    /// 0022). The transport readout, the playhead and anything placed "at the playhead"
    /// read this, so what is shown is what is heard.
    pub fn position(&self) -> Ticks {
        self.position
    }

    /// Where the engine is rendering, ahead of [`Self::position`] by the latency.
    pub fn engine_position(&self) -> Ticks {
        self.engine_position
    }

    pub fn is_playing(&self) -> bool {
        self.playing
    }

    pub fn meters(&self) -> &Meters {
        &self.meters
    }

    /// Readings as if the engine had sent them, for the dummy engine's tests and screenshots
    /// (it sends none); a running engine's next report replaces them.
    /// Where the engine is heard and whether it plays, as if it had reported it, for the
    /// dummy engine's tests (round 29: a controller's move recorded at its message's time).
    #[doc(hidden)]
    pub fn set_heard_for_tests(&mut self, position: Ticks, playing: bool) {
        self.position = position;
        self.engine_position = position;
        self.playing = playing;
    }

    #[doc(hidden)]
    pub fn set_meters(&mut self, meters: Meters) {
        self.meters = meters;
    }

    /// The modulators' live values, as the engine last reported them (ADR 0057).
    pub fn modulation(&self) -> &crate::modulator::ModTelemetry {
        &self.modulation
    }

    pub fn stats(&self) -> &EngineStats {
        &self.stats
    }

    pub fn send_failures(&self) -> u64 {
        self.send_failures
    }

    pub fn known_effect_count(&self) -> usize {
        self.known_effects.len()
    }

    pub fn known_instrument_count(&self) -> usize {
        self.known_instruments.len()
    }
}

/// The scope rings (ADR 0056), allocated here off the audio thread: producers for the
/// processor, consumers for the handle.
fn scope_rings() -> (
    Vec<Producer<crate::fx::speculum::ScopeChunk>>,
    Vec<Consumer<crate::fx::speculum::ScopeChunk>>,
) {
    use crate::fx::speculum::{MAX_SCOPES, SCOPE_CHUNKS};
    (0..MAX_SCOPES)
        .map(|_| RingBuffer::new(SCOPE_CHUNKS))
        .unzip()
}

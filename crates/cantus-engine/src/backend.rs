//! Audio I/O backend (cpal) and the UI-side [`EngineHandle`].
//!
//! The handle is the only thing the rest of the application touches: it owns the stream,
//! the message producer and the event consumer, caches the latest engine state, and keeps
//! effect instances in sync with the snapshots it sends.

use crate::effects::EffectUnit;
use crate::messages::*;
use crate::midi::MidiSender;
use crate::processor::Processor;
use crate::samples::SampleCache;
use cantus_core::{EffectId, InstrumentKind, Project, Snapshot, Ticks, TrackId};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
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

/// Enumerate output devices for a settings dialog.
pub fn output_devices() -> Vec<DeviceInfo> {
    let host = cpal::default_host();
    let default_name = host
        .default_output_device()
        .and_then(|d| d.description().ok())
        .map(|d| d.name().to_string());
    host.output_devices()
        .map(|it| {
            it.filter_map(|d| d.description().ok().map(|desc| desc.name().to_string()))
                .map(|name| DeviceInfo {
                    is_default: Some(&name) == default_name.as_ref(),
                    name,
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Requested stream settings. `None` means "device default".
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AudioConfig {
    pub device_name: Option<String>,
    pub sample_rate: Option<u32>,
    pub buffer_frames: Option<u32>,
}

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
    stats: EngineStats,
    send_failures: u64,
    /// Effect ids whose unit currently lives in the engine.
    known_effects: HashSet<EffectId>,
    /// For each track hosting a native instrument, the kind and serial of the unit the
    /// engine holds (or has been sent) for it.
    known_instruments: HashMap<TrackId, (InstrumentKind, u64)>,
    /// Every sample any snapshot has referenced, for the life of the session.
    pub samples: SampleCache,
    note_inputs: Vec<NoteInput>,
    /// The consumer end of the mix recorder's ring; `None` without an audio device.
    capture: Option<Consumer<f32>>,
    capture_target: CaptureTarget,
    /// The consumer end of the analyser's ring.
    analyser: Option<Consumer<f32>>,
    /// The analysed compressor's meter, the maxima of every block since the last take.
    comp_peak: [f32; 3],
    /// What hosted plug-ins' own windows did since the last take (ADR 0019).
    fx_params: Vec<(EffectId, crate::fx::FxParamEvent)>,
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
    /// Open the output device and start the audio thread.
    pub fn start(config: &AudioConfig) -> Result<Self, EngineError> {
        let host = cpal::default_host();
        let device = match &config.device_name {
            Some(name) => host
                .output_devices()
                .map_err(|e| EngineError::Config(e.to_string()))?
                .find(|d| d.description().map(|x| x.name() == name).unwrap_or(false))
                .or_else(|| host.default_output_device()),
            None => host.default_output_device(),
        }
        .ok_or(EngineError::NoOutputDevice)?;
        let device_name = device
            .description()
            .map(|d| d.name().to_string())
            .unwrap_or_default();
        let default = device
            .default_output_config()
            .map_err(|e| EngineError::Config(e.to_string()))?;
        let channels = default.channels().max(1) as usize;
        let sample_rate = config.sample_rate.unwrap_or(default.sample_rate());
        let stream_config = cpal::StreamConfig {
            channels: channels as u16,
            sample_rate,
            buffer_size: match config.buffer_frames {
                Some(n) => cpal::BufferSize::Fixed(n),
                None => cpal::BufferSize::Default,
            },
        };

        let (tx, rx_engine) = RingBuffer::<EngineMessage>::new(MESSAGE_QUEUE);
        let (tx_engine, rx) = RingBuffer::<EngineEvent>::new(EVENT_QUEUE);
        let (midi_tx, midi_rx) = RingBuffer::<MidiEvent>::new(MIDI_QUEUE);
        let mut processor = Processor::new(sample_rate as f64, rx_engine, tx_engine, midi_rx);
        let (capture_tx, capture_rx) = RingBuffer::<f32>::new(CAPTURE_SAMPLES);
        processor.attach_capture(capture_tx);
        let (analyser_tx, analyser_rx) = RingBuffer::<f32>::new(ANALYSER_SAMPLES);
        processor.attach_analyser(analyser_tx);

        let stream = device
            .build_output_stream(
                stream_config,
                move |data: &mut [f32], _info: &cpal::OutputCallbackInfo| {
                    processor.process(data, channels);
                },
                |err| log::error!("audio stream error: {err}"),
                None,
            )
            .map_err(|e| EngineError::Stream(e.to_string()))?;
        stream
            .play()
            .map_err(|e| EngineError::Stream(e.to_string()))?;
        log::info!("audio started: {device_name} @ {sample_rate} Hz, {channels} ch");

        Ok(EngineHandle {
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
            stats: EngineStats::default(),
            send_failures: 0,
            known_effects: HashSet::new(),
            known_instruments: HashMap::new(),
            samples: SampleCache::new(),
            note_inputs: Vec::new(),
            capture: Some(capture_rx),
            capture_target: CaptureTarget::Off,
            analyser: Some(analyser_rx),
            comp_peak: [0.0; 3],
            fx_params: Vec::new(),
        })
    }

    /// A handle and the processor behind it, with no audio device: the caller drives the
    /// processor (`Processor::process`) wherever it likes — a test standing in for the
    /// audio callback, so everything from the UI's side through the engine runs for real.
    pub fn manual(sample_rate: f64) -> (Self, Processor) {
        let (tx, rx_engine) = RingBuffer::<EngineMessage>::new(MESSAGE_QUEUE);
        let (tx_engine, rx) = RingBuffer::<EngineEvent>::new(EVENT_QUEUE);
        let processor = Processor::without_midi(sample_rate, rx_engine, tx_engine);
        let mut handle = Self::dummy();
        handle.tx = tx;
        handle.rx = rx;
        handle.sample_rate = sample_rate;
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
            stats: EngineStats::default(),
            send_failures: 0,
            known_effects: HashSet::new(),
            known_instruments: HashMap::new(),
            samples: SampleCache::new(),
            note_inputs: Vec::new(),
            capture: None,
            capture_target: CaptureTarget::Off,
            analyser: None,
            comp_peak: [0.0; 3],
            fx_params: Vec::new(),
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
        let mut snapshot = Snapshot::compile(project, revision);
        for (path, err) in self.samples.attach(&mut snapshot) {
            log::warn!("sample {}: {err}", path.display());
        }
        Arc::new(snapshot)
    }

    /// Send a snapshot, first delivering units for any effect the engine does not have.
    pub fn set_snapshot(&mut self, snapshot: Arc<Snapshot>) {
        for strip in &snapshot.strips {
            for slot in &strip.effects {
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
        }
        // Native instruments: a track that no longer hosts one (or is gone) forgets its
        // unit — the engine hands it back as the snapshot lands — and a track that needs one
        // it has not been sent gets it first.
        self.known_instruments.retain(|track, (kind, _)| {
            snapshot
                .tracks
                .iter()
                .any(|t| t.id == *track && t.instrument.native().map(|n| n.kind) == Some(*kind))
        });
        let units = crate::inst::units_for(&snapshot, self.sample_rate as f32, |track, kind| {
            self.known_instruments.get(&track).map(|k| k.0) == Some(kind)
        });
        for unit in units {
            let (track, kind, serial) = (unit.track, unit.kind, unit.serial);
            if self.tx.push(EngineMessage::AddInstrument(unit)).is_ok() {
                self.known_instruments.insert(track, (kind, serial));
            } else {
                self.send_failures += 1;
            }
        }
        self.send(EngineMessage::SetSnapshot(snapshot));
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

    pub fn set_recording(&mut self, on: bool) {
        self.send(EngineMessage::SetRecording(on));
    }

    pub fn panic(&mut self) {
        self.send(EngineMessage::Panic);
    }

    /// Drain events from the audio thread. Call once per UI frame. Returns how many
    /// events were processed.
    pub fn poll(&mut self) -> usize {
        let mut n = 0;
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
                    self.meters = m;
                }
                EngineEvent::Stats(s) => self.stats = s,
                EngineEvent::NoteInput(note) => self.note_inputs.push(note),
                EngineEvent::Garbage(old) => drop(old),
                EngineEvent::GarbageEffect(unit) => {
                    // A replaced unit's id lives on in its successor.
                    if !unit.replaced {
                        self.known_effects.remove(&unit.id);
                    }
                    drop(unit);
                }
                EngineEvent::FxParam { effect, event } => self.fx_params.push((effect, event)),
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
            }
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

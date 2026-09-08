//! Audio I/O backend (cpal) and the UI-side [`EngineHandle`].
//!
//! The handle is the only thing the rest of the application touches: it owns the stream,
//! the message producer and the event consumer, caches the latest engine state, and keeps
//! effect instances in sync with the snapshots it sends.

use crate::effects::EffectUnit;
use crate::messages::*;
use crate::midi::MidiSender;
use crate::processor::Processor;
use cantus_core::{EffectId, Snapshot, Ticks, TrackId};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use rtrb::{Consumer, Producer, RingBuffer};
use std::collections::HashSet;
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
    position: Ticks,
    playing: bool,
    meters: Meters,
    stats: EngineStats,
    send_failures: u64,
    /// Effect ids whose unit currently lives in the engine.
    known_effects: HashSet<EffectId>,
    note_inputs: Vec<NoteInput>,
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
            playing: false,
            meters: Meters::default(),
            stats: EngineStats::default(),
            send_failures: 0,
            known_effects: HashSet::new(),
            note_inputs: Vec::new(),
        })
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
            playing: false,
            meters: Meters::default(),
            stats: EngineStats::default(),
            send_failures: 0,
            known_effects: HashSet::new(),
            note_inputs: Vec::new(),
        }
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

    /// Send a snapshot, first delivering units for any effect the engine does not have.
    pub fn set_snapshot(&mut self, snapshot: Arc<Snapshot>) {
        for track in &snapshot.tracks {
            for slot in &track.effects {
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
        self.send(EngineMessage::SetSnapshot(snapshot));
    }

    pub fn play(&mut self) {
        self.send(EngineMessage::Play);
    }

    pub fn stop(&mut self) {
        self.send(EngineMessage::Stop);
    }

    pub fn seek(&mut self, tick: Ticks) {
        self.position = tick;
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
                EngineEvent::Position { tick, playing } => {
                    self.position = tick;
                    self.playing = playing;
                }
                EngineEvent::Meters(m) => self.meters = m,
                EngineEvent::Stats(s) => self.stats = s,
                EngineEvent::NoteInput(note) => self.note_inputs.push(note),
                EngineEvent::Garbage(old) => drop(old),
                EngineEvent::GarbageEffect(unit) => {
                    self.known_effects.remove(&unit.id);
                    drop(unit);
                }
            }
        }
        n
    }

    /// Live notes recorded since the last call.
    pub fn take_note_inputs(&mut self) -> Vec<NoteInput> {
        std::mem::take(&mut self.note_inputs)
    }

    pub fn position(&self) -> Ticks {
        self.position
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
}

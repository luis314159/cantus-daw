//! # cantus-engine
//!
//! The real-time audio engine. [`Processor`] runs on the audio thread and follows the
//! rules in ADR 0005; [`EngineHandle`] is what the application talks to; [`render`]
//! drives the same processor offline for bit-identical bounces; [`midi`] feeds hardware
//! MIDI into the engine from its own threads.

#![forbid(unsafe_code)]

pub mod backend;
pub mod effects;
pub mod messages;
pub mod midi;
pub mod processor;
pub mod render;
pub mod samples;
pub mod transport;

pub use backend::{output_devices, AudioConfig, DeviceInfo, EngineError, EngineHandle};
pub use effects::EffectUnit;
pub use messages::{
    EngineEvent, EngineMessage, EngineStats, Meters, MidiEvent, NoteInput, MAX_BLOCK_FRAMES,
    MAX_EFFECTS_PER_TRACK, MAX_TRACKS,
};
pub use midi::{list_ports, MidiInputs, MidiSender};
pub use processor::Processor;
pub use render::{render, render_to_wav, write_wav, RenderError, RenderOptions, Rendered};
pub use samples::{SampleCache, SampleStatus};
pub use transport::Transport;

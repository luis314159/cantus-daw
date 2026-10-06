//! # cantus-engine
//!
//! The real-time audio engine. [`Processor`] runs on the audio thread and follows the
//! rules in ADR 0005; [`EngineHandle`] is what the application talks to; [`render`]
//! drives the same processor offline for bit-identical bounces; [`midi`] feeds hardware
//! MIDI into the engine from its own threads.

#![forbid(unsafe_code)]

pub mod automation;
pub mod backend;
pub mod devices;
pub mod effects;
pub mod fx;
pub mod hotplug;
pub mod inst;
pub mod loopback;
pub mod messages;
pub mod midi;
pub mod pdc;
pub mod preview;
pub mod processor;
pub mod profile;
pub mod pulsus_kit;
pub mod recorder;
pub mod render;
pub mod routing;
pub mod samples;
pub mod stems;
pub mod transport;

pub use automation::Runtime as AutomationRuntime;
pub use backend::{output_devices, AudioConfig, DeviceInfo, EngineError, EngineHandle};
pub use devices::{DeviceLatency, DriverInfo, InputDeviceInfo};
pub use effects::{comp_settings, eq_bands, EffectMemory, EffectUnit, MemoryNeed};
pub use fx::{FxNote, FxParamEvent, FxUnit, SongPosition, Telemetry, Wire, TELEMETRY_LEN};
pub use inst::{
    hosted_key, Controller, InstrumentUnit, NativeUnit, NoteExpression, NoteOn, UnitKind,
};
pub use loopback::{LoopbackError, LoopbackResult};
pub use messages::{
    CaptureTarget, EngineEvent, EngineMessage, EngineStats, Meters, MidiEvent, NoteInput,
    ANALYSER_SAMPLES, CAPTURE_SAMPLES, MAX_BLOCK_FRAMES, MAX_EFFECTS_PER_TRACK, MAX_LIVE_ECHO,
    MAX_SENDS_PER_STRIP, MAX_STRIPS, MAX_TRACKS,
};
pub use midi::{list_ports, MidiInputs, MidiSender};
pub use processor::Processor;
pub use profile::{BlockHistogram, Culprit, Dropout};
pub use recorder::{
    FinishedTake, LivePeaks, RecordLatency, RecordMark, RecordRings, MAX_INPUTS, PEAK_FRAMES,
};
pub use render::{
    render, render_to_wav, render_with_instruments, render_with_units, write_wav, RenderError,
    RenderOptions, Rendered,
};
pub use samples::{SampleCache, SampleStatus};
pub use stems::{
    render_stems, render_stems_to_folder, stem_file_name, Stem, StemOptions, StemSource, StemSpec,
    StemsDone, StemsError,
};
pub use transport::Transport;

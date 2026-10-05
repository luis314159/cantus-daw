//! # cantus-core
//!
//! The heart of Cantus: musical time, the project model, the command bus and playback
//! snapshots. This crate has **no I/O and no threads**; it is pure data and logic so it can
//! be tested exhaustively and embedded anywhere (the UI, the AI layer, scripts, tests).
//!
//! See `docs/adr/0004-command-bus.md`.

#![forbid(unsafe_code)]
#![warn(missing_debug_implementations)]

pub mod audio;
pub mod bus;
pub mod command;
pub mod effect;
pub mod external;
pub mod fx;
pub mod inst;
pub mod instrument;
pub mod model;
pub mod pads;
pub mod snapshot;
pub mod time;

pub use audio::{
    AudioRegion, AudioSource, ClickMode, FadeCurve, MonitorMode, RecordSettings, TakeMode,
    TrackInput,
};
pub use bus::{AuditEntry, AuditKind, CommandBus, HistoryEntry, MergeKey};
pub use command::{Command, CommandError, Origin};
pub use effect::{
    eq_key, EffectKind, EffectParamInfo, EffectSlot, EffectSpec, ExternalPlugin, PluginFormat,
    PluginState, EQ_BANDS, EQ_BAND_FIELDS,
};
pub use inst::{InstParamInfo, InstrumentKind, NativeSpec};
pub use instrument::{
    FirmusParam, FirmusParams, InstrumentParam, InstrumentSpec, ParamInfo, Waveform,
};
pub use model::{
    apply_width, balance_gains, db_to_linear, linear_to_db, pan_gains, pitch_name, pitch_to_hz,
    AudioSourceId, Clip, ClipId, ClipWindow, Color, Dock, EffectId, EqBand, EqField, GroupId,
    LoopRegion, Master, MixerStrip, Note, NoteId, PanLaw, Pitch, Project, Send, StripEq,
    StripGroup, StripId, Track, TrackIcon, TrackId, Velocity, DEFAULT_RIGHT_DOCK, DEFAULT_STRIPS,
    EQ_MAX_GAIN_DB, EQ_MAX_HZ, EQ_MAX_Q, EQ_MIN_HZ, EQ_MIN_Q, MAX_GAIN_DB, MAX_INSERTS, MAX_SENDS,
    MAX_TRACK_HEIGHT, MAX_WIDTH, MIN_GAIN_DB, MIN_TRACK_HEIGHT,
};
pub use pads::{
    DrumPad, KitField, KitSettings, Lanes, PadField, PadId, PadLayer, PadParamInfo, PadVoice,
    PulsusParams, RoundRobin, SampleData, CHOKE_GROUPS, MAX_LAYERS, PULSUS_MODEL,
};
pub use snapshot::{AudioClipSnapshot, NoteEvent, Snapshot, StripSnapshot, TrackSnapshot};
pub use time::{Ticks, TimeSignature, PPQ};

//! # cantus-core
//!
//! The heart of Cantus: musical time, the project model, the command bus and playback
//! snapshots. This crate has **no I/O and no threads**; it is pure data and logic so it can
//! be tested exhaustively and embedded anywhere (the UI, the AI layer, scripts, tests).
//!
//! See `docs/adr/0004-command-bus.md`.

#![forbid(unsafe_code)]
#![warn(missing_debug_implementations)]

pub mod bus;
pub mod command;
pub mod effect;
pub mod instrument;
pub mod model;
pub mod pads;
pub mod snapshot;
pub mod time;

pub use bus::{AuditEntry, AuditKind, CommandBus, HistoryEntry, MergeKey};
pub use command::{Command, CommandError, Origin};
pub use effect::{EffectKind, EffectParamInfo, EffectSlot, EffectSpec};
pub use instrument::{
    FirmusParam, FirmusParams, InstrumentParam, InstrumentSpec, ParamInfo, Waveform,
};
pub use model::{
    apply_width, db_to_linear, linear_to_db, pan_gains, pitch_name, pitch_to_hz, Clip, ClipId,
    ClipWindow, Color, EffectId, LoopRegion, Master, Note, NoteId, PanLaw, Pitch, Project, Track,
    TrackIcon, TrackId, Velocity, MAX_GAIN_DB, MAX_TRACK_HEIGHT, MAX_WIDTH, MIN_GAIN_DB,
    MIN_TRACK_HEIGHT,
};
pub use pads::{DrumPad, Lanes, PadField, PadParamInfo, PadVoice, PulsusParams, SampleData};
pub use snapshot::{NoteEvent, Snapshot, TrackSnapshot};
pub use time::{Ticks, TimeSignature, PPQ};

//! # cantus-core
//!
//! The heart of Cantus: musical time, the project model, the command bus and playback
//! snapshots. This crate has **no I/O and no threads**; it is pure data and logic so it can
//! be tested exhaustively and embedded anywhere (the UI, the AI layer, scripts, tests).
//!
//! See `docs/adr/0004-command-bus.md`.

#![forbid(unsafe_code)]
#![warn(missing_debug_implementations)]

pub mod ara; // r36:ara (ADR 0097)
pub mod arrange; // r39:arrange (ADR 0113): sections, folders, lane modes
pub mod audio;
pub mod audio_edit; // r36:editor — the audio editor's edits as pure functions (ADR 0096)
pub mod audio_time;
pub mod automation;
pub mod bus;
pub mod clip_gain;
pub mod clip_lanes;
pub mod clipboard;
pub mod command;
pub mod comp;
pub mod controller;
pub mod crossfade;
pub mod effect;
pub mod external;
pub mod external_emulation; // r36:external (ADR 0095): virtual MCU and HUI units
pub mod external_hui; // r36:external
pub mod external_mcu; // r36:external
pub mod external_surface; // r36:external
pub mod external_unit; // r36:external (ADR 0094)
mod external_unit_cmd; // r36:external
pub mod freeze;
pub mod fx;
pub mod groove;
pub mod harmony; // r34:tools (ADR 0082): chords, charts, voicings, the music linter
pub mod inst;
pub mod instrument;
pub mod links;
pub mod mix_cmd; // r36:mix (ADR 0093)
pub mod mix_scene; // r36:mix
pub mod mix_snap; // r36:mix
pub mod model;
pub mod modulator;
pub mod notation;
pub mod notefx;
mod notefx_cmd;
pub mod padout; // r37:drums (ADR 0100): instruments with several outputs, split to strips
pub mod pads;
pub mod pattern;
pub mod pitch;
pub mod record_mode; // r36:record (ADR 0092): the recording mode's plan and edit lock
pub mod routing;
pub mod scale;
pub mod sectio; // r37:piano (ADR 0099): Exemplar's slicer
pub mod sectio_ops; // r37:piano
pub mod session; // r33:session (ADR 0076)
pub mod snapshot;
pub mod starting;
pub mod tempo;
pub mod time;
pub mod vca; // r36:mix
pub mod warp;

pub use arrange::{ArrangeCommand, FolderId, LaneMode, Section, SectionId, TrackFolder}; // r39:arrange
pub use audio::{
    AudioRegion,
    AudioSource,
    ClickMode,
    ClickSound,
    FadeCurve,
    MonitorMode,
    Provenance,
    RecordSettings,
    SourceMarker,
    SourceMarks,
    SourceRegion, // r36:editor
    TakeMode,
    TrackInput,
};
pub use automation::{AutoLane, AutoMode, AutoPoint, AutoRange, AutoTarget, Automation, Shape};
pub use bus::{AuditEntry, AuditKind, CommandBus, HistoryEntry, HistoryMark, MergeKey};
pub use clip_lanes::{ClipLane, CtlEvent, LaneKind};
pub use command::{Command, CommandError, Origin};
pub use effect::{
    eq_key, EffectKind, EffectParamInfo, EffectSlot, EffectSpec, ExternalPlugin, PluginFormat,
    PluginState, EQ_BANDS, EQ_BAND_FIELDS,
};
pub use external::PluginIsolation;
pub use external_unit::{ExternalLink, ExternalUnit, ExternalUnitId}; // r36:external
pub use freeze::{FreezeTarget, Frozen};
pub use fx::aula::IrFile; // r39:conv
pub use groove::{ClipGroove, Groove, GrooveId, GrooveSource, SwingGrid};
pub use inst::{InstParamInfo, InstrumentKind, NativeSpec};
pub use instrument::{
    FirmusParam, FirmusParams, InstrumentParam, InstrumentSpec, ParamInfo, Waveform,
};
pub use model::{
    apply_width, balance_gains, db_to_linear, linear_to_db, pan_gains, pitch_name, pitch_to_hz,
    AudioSourceId, Clip, ClipId, ClipWindow, Color, Dock, EffectId, EqBand, EqField, GroupId,
    Lateness, LinkId, LoopRegion, Master, MixerStrip, ModulatorId, Note, NoteId, PanLaw, Pitch,
    Project, Send, SendTap, StripEq, StripGroup, StripId, Track, TrackIcon, TrackId, Velocity,
    DEFAULT_RIGHT_DOCK, DEFAULT_STRIPS, EQ_MAX_GAIN_DB, EQ_MAX_HZ, EQ_MAX_Q, EQ_MIN_HZ, EQ_MIN_Q,
    MAX_GAIN_DB, MAX_INSERTS, MAX_SENDS, MAX_TRACK_HEIGHT, MAX_WIDTH, MIN_GAIN_DB,
    MIN_TRACK_HEIGHT,
};
pub use modulator::{ModHost, ModKind, ModRoute, ModSource, Modulator};
pub use notation::{ClefChoice, ReadingGrid, ScoreSettings, StaffChoice};
pub use notefx::{NoteEffect, NoteFxId, NoteFxKind, NoteFxType};
pub use pads::{
    DrumPad, KitField, KitSettings, Lanes, PadField, PadId, PadLayer, PadParamInfo, PadVoice,
    PulsusParams, RoundRobin, SampleData, StreamRef, CHOKE_GROUPS, MAX_LAYERS, PULSUS_MODEL,
};
pub use scale::{Key, ScaleKind};
pub use snapshot::{AudioClipSnapshot, NoteEvent, Snapshot, StripSnapshot, TrackSnapshot};
pub use tempo::{Marker, MarkerId, MetreChange, TempoMap, TempoPoint};
pub use time::{Ticks, TimeSignature, PPQ};

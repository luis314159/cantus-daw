//! The contract every plug-in format implements (CLAP in `clap/`, VST3 in `vst3/`).
//!
//! A format is a [`Backend`]: it knows where plug-ins live on this system, recognises its
//! files, reads what a file contains ([`Backend::scan`], run inside a separate scanner
//! process so a plug-in that crashes cannot take Cantus with it) and loads one
//! ([`Backend::load`]) into two halves: [`MainThread`] for the UI thread and
//! [`AudioThread`] for the audio thread. The halves share the plug-in instance; the
//! backend keeps it alive until both are dropped, and destroys it on the main thread.
//!
//! Additions are welcome; changing what is here needs a reason written in ADR 0019.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// A plug-in standard.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Format {
    Clap,
    Vst3,
}

/// What a plug-in is for.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PluginKind {
    /// Audio in, audio out: goes in an insert slot.
    Effect,
    /// Notes in, audio out: goes on a track.
    Instrument,
    /// Notes in, notes out.
    NoteEffect,
    Other,
}

/// One plug-in as a scan found it. Stored in the plug-in database and, for a plug-in in
/// use, enough of it in the project to find it again on another machine.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PluginInfo {
    pub format: Format,
    /// The format's stable id: a CLAP plug-in id (`com.vendor.name`), a VST3 class id
    /// (32 hex digits). With `format`, this is what a project stores.
    pub id: String,
    pub name: String,
    pub vendor: String,
    pub version: String,
    /// The file or bundle it was found in.
    pub path: PathBuf,
    pub kind: PluginKind,
    /// Channels of the main input and output (0 input for an instrument).
    pub main_inputs: u32,
    pub main_outputs: u32,
    /// Has a second (sidechain) audio input.
    pub sidechain: bool,
    /// Has its own editor window.
    pub editor: bool,
    /// The format's category words (CLAP features, VST3 subcategories), as given.
    pub features: Vec<String>,
}

/// One parameter as the plug-in describes it. Values are in the plug-in's own range
/// (`min ..= max`), as both standards transmit them to a host that asks for plain values.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParamInfo {
    /// Stable across sessions (CLAP param id, VST3 ParamID).
    pub id: u32,
    pub name: String,
    /// The plug-in's grouping ("Filter/Envelope"), empty if none.
    pub group: String,
    pub min: f64,
    pub max: f64,
    pub default: f64,
    /// Only whole steps between `min` and `max`.
    pub stepped: bool,
    pub automatable: bool,
    pub hidden: bool,
    pub read_only: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum HostError {
    #[error("could not load {0}")]
    Load(String),
    #[error("no plug-in with id {0} in that file")]
    NotFound(String),
    #[error("the plug-in refused to start: {0}")]
    Instantiate(String),
    #[error("the plug-in refused to activate: {0}")]
    Activate(String),
    #[error("not supported: {0}")]
    Unsupported(&'static str),
    #[error("state: {0}")]
    State(String),
    #[error("editor: {0}")]
    Editor(String),
    /// The plug-in's process ended or stopped answering (an isolated plug-in, ADR 0063).
    #[error("the plug-in stopped working: {0}")]
    Crashed(String),
    /// Its helper ended while it loaded, and whose doing that was (round 29, review #40):
    /// a neighbour's crash is no crash of this plug-in's.
    #[error("the plug-in stopped working: {why}")]
    Ended { why: String, blame: Blame },
    /// Its helper lives but has not answered yet (a plug-in's dialog is open, a slow call
    /// is ahead of this one): the call is given up, not the helper (round 29, review #4).
    #[error("the plug-in is busy: {0}")]
    Busy(String),
}

/// How the engine will run the plug-in.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct AudioSetup {
    pub sample_rate: f64,
    /// The largest block `process` will be called with.
    pub max_block: usize,
}

/// A plug-in format.
pub trait Backend: Send + Sync {
    fn format(&self) -> Format;
    /// The standard folders for this format on this system, in the standard's order.
    fn search_paths(&self) -> Vec<PathBuf>;
    /// Whether `path` is a file or bundle of this format (by name and shape, not by
    /// loading it).
    fn is_candidate(&self, path: &Path) -> bool;
    /// Load `path` and describe every plug-in in it. Runs inside the scanner process: it
    /// may crash, hang or print, and Cantus survives all three.
    fn scan(&self, path: &Path) -> Result<Vec<PluginInfo>, HostError>;
    /// Load, instantiate and activate one plug-in. Main thread.
    fn load(&self, info: &PluginInfo, setup: AudioSetup) -> Result<Loaded, HostError>;
}

/// A loaded, activated plug-in, split by thread.
pub struct Loaded {
    pub main: Box<dyn MainThread>,
    pub audio: Box<dyn AudioThread>,
}

/// The size of an editor, in physical pixels.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct EditorSize {
    pub width: u32,
    pub height: u32,
}

/// Something the plug-in asked for that the host must act on (see [`MainThread::idle`]).
#[derive(Clone, Debug, PartialEq)]
pub enum Notice {
    /// Its latency changed: delay compensation must re-plan.
    LatencyChanged(usize),
    /// Its parameter list or their names/ranges changed: rescan them.
    ParamsChanged,
    /// Its state changed without a parameter change (a preset loaded in its own
    /// browser): the project should store the new state.
    StateDirty,
    EditorResized(EditorSize),
    /// The user closed its window (floating editors).
    EditorClosed,
    /// It must be deactivated and activated again (the host does it off the audio thread).
    RestartNeeded,
    /// It stopped working (round 29, ADR 0063): the process it ran in crashed, or it hung
    /// and was ended. Its audio half passes the sound through (an effect) or stays silent
    /// (an instrument) until it is reloaded; its last-good state is what `save_state`
    /// answers from now on. Said once.
    Crashed {
        reason: String,
        /// Whether it was this plug-in that crashed or hung its process (review #49).
        blame: Blame,
    },
}

/// Who an isolated plug-in's helper's end is laid on (round 29, review #49): the plug-in
/// that crashed or hung it, so a neighbour that merely shared its process is neither
/// counted nor remembered for it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Blame {
    /// This instance was running when its helper died.
    This,
    /// Another instance of its helper was.
    Neighbour,
    /// Nothing says which.
    Unknown,
}

/// Everything called on the main (UI) thread.
pub trait MainThread: Send {
    fn info(&self) -> &PluginInfo;
    fn params(&self) -> Vec<ParamInfo>;
    fn param_value(&self, id: u32) -> Option<f64>;
    /// The plug-in's own text for a value ("-6.0 dB", "Hall").
    fn param_text(&self, id: u32, value: f64) -> Option<String>;
    /// The plug-in's whole state, opaque, to store in the project.
    fn save_state(&mut self) -> Result<Vec<u8>, HostError>;
    fn load_state(&mut self, data: &[u8]) -> Result<(), HostError>;
    /// Samples of delay it adds now.
    fn latency(&self) -> usize;
    fn has_editor(&self) -> bool;
    /// Embed the editor in `parent` (a native child window the host owns).
    fn open_editor(
        &mut self,
        parent: raw_window_handle::RawWindowHandle,
    ) -> Result<EditorSize, HostError>;
    fn close_editor(&mut self);
    /// Service the plug-in's requests; call once per UI frame. Returns what the host must
    /// act on.
    fn idle(&mut self) -> Vec<Notice>;

    // ---- Additions (round 17, ADR 0019). Each has a default, so a backend that does not
    // ---- implement one still builds and the host falls back to something that works.

    /// After [`Notice::RestartNeeded`] (or [`Notice::LatencyChanged`]): once the audio half
    /// has been stopped and dropped, deactivate and activate the same instance again and
    /// return its new audio half. `Ok(None)` while the old audio half is still alive: call
    /// again on a later frame. The default is unsupported, and the host then restarts by
    /// loading a new instance and moving the state across.
    fn restart(&mut self) -> Result<Option<Box<dyn AudioThread>>, HostError> {
        Err(HostError::Unsupported("restart"))
    }

    /// Deliver parameter changes now, on this thread, when the audio half is not
    /// processing (no audio device, or before its first block); what the plug-in reports
    /// back goes to `out`. Returns `false` when the audio half is processing: the changes
    /// then travel with its next block, and nothing was done.
    fn flush(&mut self, params: &[ParamChange], out: &mut ParamOut) -> bool {
        let _ = (params, out);
        false
    }

    /// The editor opened by [`Self::open_editor`] is a floating window of the plug-in's
    /// own (the fallback for a plug-in that cannot be embedded), not a child of `parent`.
    fn editor_is_floating(&self) -> bool {
        false
    }

    /// Ask the editor to take a new size (the host window was resized); returns the size
    /// it accepted, which may differ.
    fn resize_editor(&mut self, size: EditorSize) -> Option<EditorSize> {
        let _ = size;
        None
    }

    /// Whether the editor can be resized by the host window.
    fn editor_can_resize(&self) -> bool {
        false
    }

    // ---- Additions (round 29, ADR 0063: plug-ins out of process). Defaults again. ----

    /// Bring the plug-in's own editor window forward, when the window is its own (an
    /// isolated plug-in's lives in its helper process). `false`: the host's window serves.
    fn focus_editor(&mut self) -> bool {
        false
    }

    /// A good moment to remember the plug-in's state (a gesture ended): an isolated
    /// plug-in refreshes the last-good state it keeps for a crash. Nothing in process.
    fn checkpoint(&mut self) {}

    /// The time the plug-in itself spent processing, in nanoseconds, and the frames it
    /// covered, since it was loaded — measured in its helper process. `None` in process
    /// (the host measures the call there).
    fn own_time(&self) -> Option<(u64, u64)> {
        None
    }

    /// The id of the helper process it runs in; `None` within Cantus.
    fn isolated_in(&self) -> Option<u32> {
        None
    }

    /// Blocks that passed without it because it was late (an isolated plug-in past its
    /// deadline), since it was loaded.
    fn late_blocks(&self) -> u64 {
        0
    }

    /// Whether it has stopped working (an isolated plug-in whose helper ended), why, and
    /// whether that was its own doing — asked any time, so a crash during a load or a
    /// state that no `idle` reported is seen too (review #45). `None` while it works.
    fn stopped(&self) -> Option<(String, Blame)> {
        None
    }
}

/// A parameter change at a sample offset in the block.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct ParamChange {
    pub offset: u32,
    pub id: u32,
    pub value: f64,
}

/// What the plug-in's own editor did in a block: gestures and values the host records
/// (into the project, as commands with one undo step per gesture).
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum ParamEvent {
    Begin(u32),
    Value(u32, f64),
    End(u32),
}

/// A note for an instrument or note effect.
///
/// Carries what both standards can say about a note, so a hosted instrument plays
/// expressively (MPE-style) where the plug-in supports it: an id that follows the note,
/// the release velocity, and per-note expressions.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct NoteEvent {
    /// Sample offset in the block.
    pub offset: u32,
    /// Follows one note from on to off (CLAP `note_id`, VST3 `noteId`); −1 when the host
    /// does not track it, and then `channel` + `key` identify the note.
    pub note_id: i32,
    pub channel: u8,
    pub key: u8,
    pub kind: NoteKind,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum NoteKind {
    /// Velocity 0…1.
    On { velocity: f32 },
    /// Release velocity 0…1.
    Off { velocity: f32 },
    /// Stop the voice at once (a drum's choke group).
    Choke,
    /// A per-note expression; `value` in the unit the variant names.
    Expression {
        expression: NoteExpression,
        value: f64,
    },
    /// A channel message of MIDI 1.0 — a controller (`0xB0`), pitch bend (`0xE0`), channel
    /// pressure (`0xD0`) — for the whole channel `NoteEvent::channel` (the status's low
    /// nibble is ignored; `key` and `note_id` mean nothing). Added in round 20 (ADR 0028).
    /// How it reaches the plug-in is its format's business: CLAP sends it as a MIDI event
    /// when the note port speaks MIDI (pitch bend otherwise as a tuning expression on every
    /// note); VST3 sends it to the parameter the controller's `IMidiMapping` names.
    Midi { status: u8, data1: u8, data2: u8 },
}

/// The per-note expressions both standards share.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum NoteExpression {
    /// Semitones from the key (bends, MPE pitch).
    Tuning,
    /// 0…1 (poly aftertouch, MPE pressure).
    Pressure,
    /// 0…1 (MPE timbre / CC 74).
    Brightness,
    /// 0 left … 1 right.
    Pan,
    /// Linear gain, 0…4.
    Volume,
    /// 0…1.
    Vibrato,
    /// 0…1.
    Expression,
}

/// Fixed-capacity outbox for [`ParamEvent`]s, filled on the audio thread without
/// allocating. Events beyond the capacity are counted and dropped.
#[derive(Debug)]
pub struct ParamOut {
    events: [ParamEvent; Self::CAPACITY],
    len: usize,
    pub dropped: usize,
    /// The host's changes the plug-in's input lists had no room for (ADR 0057): the unit
    /// takes them into its count, which the engine reports; [`Self::clear`] keeps it.
    pub lost_in: u64,
}

impl ParamOut {
    pub const CAPACITY: usize = 256;

    pub fn new() -> Self {
        ParamOut {
            events: [ParamEvent::End(0); Self::CAPACITY],
            len: 0,
            dropped: 0,
            lost_in: 0,
        }
    }

    pub fn push(&mut self, e: ParamEvent) {
        if self.len < Self::CAPACITY {
            self.events[self.len] = e;
            self.len += 1;
        } else {
            self.dropped += 1;
        }
    }

    pub fn events(&self) -> &[ParamEvent] {
        &self.events[..self.len]
    }

    pub fn clear(&mut self) {
        self.len = 0;
        self.dropped = 0;
    }
}

impl Default for ParamOut {
    fn default() -> Self {
        Self::new()
    }
}

/// Where the song is, for plug-ins that follow the host's tempo and position.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct TransportInfo {
    pub playing: bool,
    pub recording: bool,
    /// The tempo at the block's first sample (inside a ramp, its step's: ADR 0060).
    pub tempo_bpm: f64,
    /// How the tempo moves inside a ramp, BPM per sample (CLAP's `tempo_inc`); 0 when it
    /// holds.
    pub tempo_inc: f64,
    /// Position at the block's first sample, in quarter notes from the song start.
    pub position_beats: f64,
    /// Position at the block's first sample, in samples from the song start at the tempo
    /// now (the synced-phase clock; see `position_seconds` for real time).
    pub position_samples: i64,
    /// Position at the block's first sample, in seconds from the song start through the
    /// tempo map: real time (CLAP's seconds timeline).
    pub position_seconds: f64,
    /// Position at the block's first sample, in real samples from the song start through
    /// the tempo map, rounded once (VST3's `projectTimeSamples`; at one tempo exactly the
    /// value every project before round 29 told — review 2).
    pub project_samples: i64,
    /// Start of the current bar, in quarter notes.
    pub bar_start_beats: f64,
    /// The current bar's 0-based index, counted through every metre change.
    pub bar_number: i32,
    pub time_sig_num: u16,
    pub time_sig_den: u16,
    /// The loop, in quarter notes, when looping.
    pub loop_beats: Option<(f64, f64)>,
    /// The loop in song seconds, when looping.
    pub loop_seconds: Option<(f64, f64)>,
}

/// The audio buffers of one call: the main stereo pair processed in place, and the
/// sidechain pair when the host routes one.
pub struct Audio<'a> {
    pub left: &'a mut [f32],
    pub right: &'a mut [f32],
    pub key: Option<(&'a [f32], &'a [f32])>,
    /// The song position at the block's start; `None` when the host has none (offline tools).
    pub transport: Option<&'a TransportInfo>,
}

/// Everything called on the audio thread. No allocation, no locks, no panics (ADR 0005).
pub trait AudioThread: Send {
    /// Process one block in place (`audio.left.len()` frames, at most
    /// [`AudioSetup::max_block`]). `params` are the host's changes for this block, sorted
    /// by offset; `notes` likewise (empty for an effect). The plug-in's own changes go to
    /// `out`.
    fn process(
        &mut self,
        audio: Audio<'_>,
        params: &[ParamChange],
        notes: &[NoteEvent],
        out: &mut ParamOut,
    );
    /// Clear tails and voices (transport jump, panic).
    fn reset(&mut self);
    /// Stop processing. Called on the audio thread before the half is handed back to the
    /// UI thread to be dropped; `process` is not called again.
    fn stop(&mut self);
}

//! What `transcribe` is asked and what it answers (ADR 0086): the arguments and results
//! the protocol carries as JSON, shared by the client and the helper. The notes and hits
//! themselves go in a file (`TranscribeArgs::out`), as audio does: a long song's notes and
//! bends would outgrow a protocol line.

use crate::spec::RuntimeSpec;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// What to listen for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    /// Every note, chords included (Basic Pitch).
    #[default]
    Polyphonic,
    /// One line: at most one note at a time (Basic Pitch's notes, the most salient kept).
    Melody,
    /// Kick, snare, toms, hi-hats and cymbals: Onsets and Frames Drums when its model is
    /// given, Cantus's own detector otherwise (and for the hi-hats always).
    Drums,
}

/// The note model and how it is driven.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NoteModel {
    /// Basic Pitch's `nmp.onnx` (or the drum model's graph).
    pub graph: PathBuf,
    pub family: NoteFamily,
    /// The model's files and their SHA-256, checked before it loads (ADR 0085's review).
    #[serde(default)]
    pub check: Vec<crate::spec::FileCheck>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum NoteFamily {
    /// Spotify's Basic Pitch (ICASSP 2022): 22 050 Hz mono in, windows of 43 844 samples,
    /// note, onset and contour posteriorgrams out.
    BasicPitch,
    /// Magenta's Onsets and Frames Drums (E-GMD): 44 100 Hz mono in, onset and velocity
    /// probabilities for eight drum pitches out (drums mode only).
    OafDrums,
    /// A stand-in network in Rust with Basic Pitch's contract, for tests: its posteriors
    /// are a tuned filter bank's (`transcribe_fixture.rs`). Taken only when the helper was
    /// started for tests (`CANTUS_ML_TEST_HOOKS=1`).
    Fixture {
        /// Milliseconds each window takes, so a cancel has something to stop.
        #[serde(default)]
        delay_ms: u64,
        /// Crash (abort) the helper at this window.
        #[serde(default)]
        crash_at: Option<usize>,
    },
}

/// How notes are made from the posteriors: Basic Pitch's own parameters, its defaults.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct NoteSettings {
    pub onset_threshold: f64,
    pub frame_threshold: f64,
    /// The shortest note kept, milliseconds.
    pub min_note_ms: f64,
    pub min_hz: Option<f64>,
    pub max_hz: Option<f64>,
    pub infer_onsets: bool,
    pub melodia_trick: bool,
    /// Each note's pitch bends (thirds of a semitone, one a frame).
    pub bends: bool,
}

impl Default for NoteSettings {
    fn default() -> Self {
        NoteSettings {
            onset_threshold: 0.5,
            frame_threshold: 0.3,
            min_note_ms: 127.70,
            min_hz: None,
            max_hz: None,
            infer_onsets: true,
            melodia_trick: true,
            bends: true,
        }
    }
}

/// How drums are found.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DrumSettings {
    /// 0 finds only the clearest hits, 1 the faintest; 0.5 is the measured default.
    pub sensitivity: f32,
}

impl Default for DrumSettings {
    fn default() -> Self {
        DrumSettings { sensitivity: 0.5 }
    }
}

/// `transcribe`: an audio file into notes or drum hits.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TranscribeArgs {
    pub mode: Mode,
    /// A WAV file, mono or stereo (mixed to mono). The note modes want Basic Pitch's
    /// 22 050 Hz, the drum model 44 100 Hz; drums without it take any rate.
    pub input: PathBuf,
    /// Where the notes or hits go ([`Transcription`], JSON).
    pub out: PathBuf,
    /// The model and its runtime: Basic Pitch for the note modes; for drums, Onsets and
    /// Frames Drums or none.
    #[serde(default)]
    pub runtime: Option<RuntimeSpec>,
    #[serde(default)]
    pub model: Option<NoteModel>,
    #[serde(default)]
    pub notes: NoteSettings,
    #[serde(default)]
    pub drums: DrumSettings,
}

/// What `transcribe` answers (the notes are in `file`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TranscribeResult {
    pub mode: Mode,
    pub file: PathBuf,
    pub notes: usize,
    pub hits: usize,
    /// The input's length, seconds.
    pub duration: f64,
    /// Inferences run (one per window; 0 for drums).
    pub inferences: u32,
    /// Seconds of those inside the network.
    #[serde(default)]
    pub network_seconds: f64,
    /// Seconds the job took, the model's load included.
    pub seconds: f64,
    /// Where it ran: `cpu`, `fixture`, or `dsp` for drums.
    pub device: String,
}

/// A note, seconds from the input's start.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NoteEvent {
    pub start: f64,
    pub end: f64,
    pub pitch: u8,
    /// 1–127, from the note posterior's mean as Basic Pitch makes it.
    pub velocity: u8,
    /// The mean note posterior, 0–1.
    pub amplitude: f32,
    /// Pitch bends, thirds of a semitone from `pitch`, evenly spaced from `start` to `end`
    /// (Basic Pitch's MIDI places them so); empty when not asked or never off the pitch.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bends: Vec<i32>,
    /// A finer bend, where the transcriber has one (Cantus's pitch tracker): points
    /// `(share of the note's length, semitones from `pitch`)`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub curve: Vec<(f32, f32)>,
}

/// The five drum classes Cantus tells apart (MDB Drums' and most drum transcription's).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DrumClass {
    Kick,
    Snare,
    Toms,
    HiHat,
    Cymbals,
}

impl DrumClass {
    pub const ALL: [DrumClass; 5] = [
        DrumClass::Kick,
        DrumClass::Snare,
        DrumClass::Toms,
        DrumClass::HiHat,
        DrumClass::Cymbals,
    ];
}

/// A drum hit, seconds from the input's start, with the General MIDI key it plays.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DrumHit {
    pub time: f64,
    pub class: DrumClass,
    /// GM percussion key: 36 kick, 38 snare, 45/47/50 toms (low, mid, high), 42/46 closed
    /// and open hi-hat, 49 crash, 51 ride.
    pub key: u8,
    pub velocity: u8,
}

/// The result file.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Transcription {
    pub mode: Mode,
    /// The input's length, seconds.
    pub duration: f64,
    #[serde(default)]
    pub notes: Vec<NoteEvent>,
    #[serde(default)]
    pub hits: Vec<DrumHit>,
}

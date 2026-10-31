//! What a job is asked and what it answers: the arguments and results the protocol carries
//! as JSON (`proto::Request::Start::args`, `proto::Event::Done::result`). Both sides use
//! these types, so the shapes cannot drift apart.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// The inference runtime a job runs on (ADR 0085: ONNX Runtime, its library downloaded by
/// the content manager).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RuntimeSpec {
    /// The ONNX Runtime shared library (`onnxruntime.dll`, `libonnxruntime.so.1`,
    /// `libonnxruntime.dylib`). One per helper: a helper asked for another restarts.
    pub library: PathBuf,
    /// Threads for one inference; 0 lets the runtime choose (the physical cores).
    #[serde(default)]
    pub threads: usize,
    /// Where to run.
    #[serde(default)]
    pub device: Device,
    /// The library's SHA-256 as the model manifest gives it: checked before it is loaded,
    /// so a file put in its place is never run (review 1 #5).
    #[serde(default)]
    pub sha256: Option<String>,
}

/// A file a job checks before it trusts it: its path and SHA-256.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FileCheck {
    pub path: PathBuf,
    pub sha256: String,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Device {
    /// The processor: always there, and what every number in ADR 0085 is measured on.
    #[default]
    Cpu,
    /// A graphics processor if the runtime has one for this system — DirectML on Windows,
    /// CoreML on Apple silicon — else the processor: said in the result's `device` (and a
    /// `log` event), never an error.
    Auto,
}

/// A model on disk and what the job must know to drive it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModelSpec {
    /// The graph (`.onnx`); its weights beside it, as the graph names them.
    pub graph: PathBuf,
    pub family: Family,
    /// The sources the model separates, in its output order (`drums`, `bass`, …).
    #[serde(default)]
    pub sources: Vec<String>,
    /// The sample rate the model was trained at; the input must be at it.
    pub sample_rate: u32,
    /// The samples one inference takes (HT-Demucs: 343 980, 7.8 s).
    #[serde(default)]
    pub segment: usize,
    /// The model's files and their hashes (the graph and its weights), checked before the
    /// model is loaded (review 1 #5).
    #[serde(default)]
    pub check: Vec<FileCheck>,
}

/// How a model's graph is driven: what is computed around it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Family {
    /// Hybrid Transformer Demucs (v4): the graph takes a segment and its spectrogram and
    /// gives each source's spectrogram and waveform; the STFT, its inverse and the sum are
    /// Cantus's (`demucs.rs`).
    HtDemucs,
    /// A stand-in network in Rust with HT-Demucs's contract, for tests: source `s` is
    /// `spec[s]` times the spectrogram branch plus `wave[s]` times the time branch. The
    /// helper takes it only when started for tests (`CANTUS_ML_TEST_HOOKS=1`).
    Fixture {
        spec: Vec<f32>,
        wave: Vec<f32>,
        /// Milliseconds each segment takes, so a cancel has something to stop.
        #[serde(default)]
        delay_ms: u64,
        /// Crash (abort) the helper at this segment.
        #[serde(default)]
        crash_at: Option<usize>,
        /// Panic at this segment.
        #[serde(default)]
        panic_at: Option<usize>,
        /// Stop answering at this segment, forever.
        #[serde(default)]
        hang_at: Option<usize>,
    },
}

/// `separate`: a song into its sources.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SeparateArgs {
    pub runtime: RuntimeSpec,
    pub model: ModelSpec,
    /// A WAV file at the model's sample rate, mono or stereo.
    pub input: PathBuf,
    /// Where the stems go: `<out_dir>/<source>.wav`, 32-bit float, the input's length.
    pub out_dir: PathBuf,
    /// How much consecutive segments overlap (demucs's 0.25).
    #[serde(default = "default_overlap")]
    pub overlap: f32,
    /// The "shift trick" (demucs's `--shifts`): 0 is one pass; `n` separates the song `n`
    /// times, each moved by a different fraction of half a second, and averages them —
    /// a little better, `n` times slower. Demucs draws the moves at random; here they are
    /// fixed (`k / n` of the half second), so a song separates the same way every time.
    #[serde(default)]
    pub shifts: u32,
}

pub fn default_overlap() -> f32 {
    0.25
}

/// What `separate` answers.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SeparateResult {
    pub stems: Vec<StemFile>,
    /// The input's length, which every stem has.
    pub frames: u64,
    pub sample_rate: u32,
    /// Inferences run (one per segment).
    pub inferences: u32,
    /// Seconds of those inside the network; the rest of `seconds` is Cantus's own (the
    /// model's load, the STFT, the files).
    #[serde(default)]
    pub network_seconds: f64,
    /// Seconds the job took, the model's load included.
    pub seconds: f64,
    /// Where it ran: `cpu`, or the graphics runtime's name.
    pub device: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StemFile {
    pub name: String,
    pub path: PathBuf,
}

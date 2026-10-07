//! # Export (ADR 0033)
//!
//! What the engine renders, written as WAV, FLAC, MP3 or Ogg Vorbis: the song, the time
//! selection or the stems, at a chosen rate, bit depth or bitrate, in mono or stereo,
//! dithered when cut to integers, optionally brought to a loudness under a true-peak
//! ceiling, with tags and the Cantus credit.
//!
//! **The render is the engine's** (`cantus_engine::render`, `cantus_engine::stems`):
//! this crate takes what they produce and never renders differently. The pipeline after
//! it is fixed: channels → sample rate (the engine's windowed-sinc converter) → loudness
//! (BS.1770-4, Terminus's meter and limiter) → integers with dither (Terminus's
//! ditherer) → the encoder.
//!
//! **Encoders**: FLAC is pure Rust (`flacenc`, Apache-2.0); MP3 is LAME 3.100 and Ogg
//! Vorbis is libvorbis (aoTuV), both C, built from source by their `-sys` crates with
//! the compiler the workspace already needs — no download, no manual step. LAME is LGPL,
//! libvorbis BSD: both fine in a GPL application, which is why this crate is GPL.
//!
//! **Nothing half-written**: a file is written as `name.ext.part` beside its place and
//! renamed when complete; stopped or failed, the part is removed.
//!
//! Everything here runs on a thread of its own (the application's export thread);
//! [`Control`] carries the progress out and the request to stop in.

#![forbid(unsafe_code)]

pub mod encode;
pub mod job;
pub mod process;
pub mod settings;
pub mod stems;
pub mod tags;

pub use job::{
    export_mix, export_rendered, stem_output_name, Control, Done, MixJob, Phase, StemsJob,
};
pub use stems::export_stems;
pub use process::{Audio, Normalised};
pub use settings::{
    Channels, Depth, Dither, ExportSettings, Format, Loudness, Mp3Mode, Preset, Rate, Scope,
    StemChoice, Tags, CEILINGS, LUFS_TARGETS, MP3_BITRATES, MP3_VBR, OGG_QUALITIES, RATES,
};

#[derive(Debug, thiserror::Error)]
pub enum ExportError {
    #[error("stopped")]
    Cancelled,
    #[error("nothing to render: the project has no notes")]
    Empty,
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("render: {0}")]
    Render(String),
    #[error("{0}")]
    Encode(String),
    #[error("the file would pass 4 GB, more than a WAV file can hold")]
    TooLong,
}

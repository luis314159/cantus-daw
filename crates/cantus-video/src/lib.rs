//! # Video export (round 35, ADR 0088)
//!
//! The dancers as a video with the song's audio: an MP4 for social media (H.264 High +
//! AAC-LC, the one format every platform takes), a QuickTime MOV with PNG frames (straight
//! alpha: the transparent layer for a video editor) and a PNG sequence with a WAV (the
//! universal fallback).
//!
//! **Nothing patented is shipped.** H.264 and AAC come from the operating system's
//! encoders, licensed with it: Media Foundation's encoder transforms on Windows, driven
//! directly ([`mf`]) — not its sink writer, which delays video a frame, writes the index at
//! the end and drops the AAC encoder's priming. The containers are ours ([`mp4`], [`mov`]):
//! the index in front (fast start), an audio edit list that trims the encoder's delay, no
//! B-frames and no edit list on the video — so the first decoded sample of each is time 0
//! and a beat's flash lands on its click in every decoder.
//!
//! **Time is counted, never measured**: video frame `n` is at `n / fps` exactly (a
//! 30 000-a-second timescale), the audio is the export's own render cut per frame and timed
//! by its sample index; both start at 0 ([`timing`]).
//!
//! **Frames render on worker threads** ([`pipeline`]): in order, with bounded memory (two
//! frames in flight a worker), each worker drawing its frame and turning it into what the
//! encoder takes (4:2:0 YUV, BT.709 — [`yuv`]; or a PNG).
//!
//! Everything here runs on a thread of the caller's (the application's export thread);
//! [`Control`] carries the progress out and the request to stop in. A file is written as
//! `name.ext.part` beside its place and put there only when whole (as audio exports are).

#[cfg(target_os = "macos")]
pub mod apple;
pub mod audio;
pub mod audio_alt;
pub mod av1;
pub mod bmff;
pub mod dance;
pub mod export;
pub mod h264;
#[cfg(windows)]
pub mod mf;
pub mod mov;
pub mod mp4;
pub mod mux;
pub mod openh264;
pub mod pipeline;
pub mod select;
pub mod seq;
pub mod timing;
pub mod title;
pub mod vorbis;
pub mod webm;
pub mod yuv;

pub use export::{export, Done, Request};
pub use select::{Codecs, Plan, Warning};
pub use timing::Timing;

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

/// The kinds of file a video export writes.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Container {
    /// H.264 High + AAC-LC in MP4: what every platform takes.
    Mp4,
    /// AV1 + Vorbis in WebM: royalty-free (YouTube and TikTok take it); slower to make.
    WebM,
    /// PNG frames (RGBA with straight alpha, or RGB) + 16-bit PCM in a QuickTime MOV.
    MovPng,
    /// A folder of numbered PNG frames and the song as a 16-bit WAV.
    PngSequence,
}

impl Container {
    /// The file's extension (none for a sequence: it is a folder).
    pub fn extension(self) -> &'static str {
        match self {
            Container::Mp4 => "mp4",
            Container::WebM => "webm",
            Container::MovPng => "mov",
            Container::PngSequence => "",
        }
    }

    /// Whether the format keeps a transparent background.
    pub fn has_alpha(self) -> bool {
        !matches!(self, Container::Mp4 | Container::WebM)
    }
}

/// How hard the H.264 encoder works for the picture: its target bitrate, scaled with the
/// picture's size and rate ([`Quality::bitrate`]).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Default)]
pub enum Quality {
    /// Small files, fine for a phone's screen.
    Good,
    /// What platforms recommend for an upload (YouTube: ~8 Mbit/s at 1080p 30).
    #[default]
    High,
    /// The most the encoder is asked for: the cleanest source for a platform's re-encode.
    Best,
}

impl Quality {
    /// Bits a second for `width` × `height` at `fps`: bits per pixel per frame, with the
    /// second 30 frames of a second costing half (motion between close frames is cheap).
    pub fn bitrate(self, width: u32, height: u32, fps: u32) -> u32 {
        let bpp = match self {
            Quality::Good => 0.07,
            Quality::High => 0.13,
            Quality::Best => 0.25,
        };
        let rate = fps.min(30) as f64 + fps.saturating_sub(30) as f64 * 0.5;
        let bits = width as f64 * height as f64 * rate * bpp;
        bits.clamp(500_000.0, 100_000_000.0) as u32
    }
}

/// What can go wrong, in words the dialog shows.
#[derive(Debug, thiserror::Error)]
pub enum VideoError {
    #[error("stopped")]
    Cancelled,
    #[error("{0}")]
    Io(#[from] std::io::Error),
    /// The system has no H.264 or AAC encoder (Windows N without the Media Feature Pack).
    #[error("this system has no {0} encoder")]
    NoEncoder(&'static str),
    #[error("the {codec} encoder: {message}")]
    Encoder {
        codec: &'static str,
        message: String,
    },
    #[error("{0}")]
    Unsupported(String),
}

impl VideoError {
    pub(crate) fn encoder(codec: &'static str, message: impl std::fmt::Display) -> VideoError {
        VideoError::Encoder {
            codec,
            message: message.to_string(),
        }
    }
}

/// Shared between the export thread and the interface: how many frames are done, and
/// whether the export has been asked to stop.
#[derive(Debug, Default)]
pub struct Control {
    cancel: AtomicBool,
    done: AtomicU32,
    total: AtomicU32,
}

impl Control {
    pub fn new() -> std::sync::Arc<Control> {
        std::sync::Arc::new(Control::default())
    }

    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    pub fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }

    /// Frames written so far, and of how many.
    pub fn frames(&self) -> (u32, u32) {
        (
            self.done.load(Ordering::Relaxed),
            self.total.load(Ordering::Relaxed),
        )
    }

    /// 0…1.
    pub fn fraction(&self) -> f32 {
        let (d, t) = self.frames();
        if t == 0 {
            0.0
        } else {
            (d as f32 / t as f32).min(1.0)
        }
    }

    pub(crate) fn start(&self, total: u32) {
        self.done.store(0, Ordering::Relaxed);
        self.total.store(total, Ordering::Relaxed);
    }

    pub(crate) fn frame_done(&self) {
        self.done.fetch_add(1, Ordering::Relaxed);
    }
}

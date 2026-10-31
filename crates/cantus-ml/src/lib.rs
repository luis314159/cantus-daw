//! Cantus's machine-learning helper and its client (round 34, ADR 0085).
//!
//! Models run in their own process, `cantus-ml` ([`helper`]), never in Cantus's: a crash,
//! a hang or a graphics driver's failure ends the helper, and Cantus says so and goes on.
//! Cantus talks to it over its standard pipes in JSON lines ([`proto`]): a job started
//! with its arguments ([`spec`]), its progress, its end, a cancel at any time. The client
//! ([`client`]) starts the helper on a thread of its own when a job first needs it — never
//! on the interface's thread, never near the audio thread — watches it, and tells a crash
//! or a hang from a job's own failure.
//!
//! Inside, jobs come from a table ([`jobs`]); the first is `separate` (stems, HT-Demucs:
//! [`separate`], [`demucs`], [`stft`]). Models run on ONNX Runtime ([`runtime`]), whose
//! library and the models' files the content manager downloads on demand
//! (`cantus_content::models`); none is in the repository or the installer.

pub mod audio;
pub mod client;
pub mod demucs;
pub mod helper;
pub mod jobs;
pub mod platform;
pub mod proto;
pub mod runtime;
pub mod separate;
pub mod spec;
pub mod stft;
pub mod transcribe; // r34:transcribe

pub use client::{Failure, MlClient, Outcome, Update};

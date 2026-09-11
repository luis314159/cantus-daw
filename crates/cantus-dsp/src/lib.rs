//! # cantus-dsp
//!
//! Allocation-free, deterministic DSP building blocks. No internal dependencies, no
//! threads, no I/O — every type here can run on the audio thread (ADR 0005).
//!
//! | Module | What |
//! |---|---|
//! | [`osc`] | PolyBLEP band-limited oscillators |
//! | [`env`] | analog-style ADSR |
//! | [`filter`] | trapezoidal state-variable filter |
//! | [`smoother`] | one-pole parameter smoothing |
//! | [`firmus`] | the built-in subtractive polysynth |
//! | [`pulsus`] | the built-in synthesized drum kit |
//! | [`click`] | metronome click |
//! | [`nulltest`] | render comparison harness |

#![forbid(unsafe_code)]
// `next()` is the conventional name for a per-sample generator in DSP code.
#![allow(clippy::should_implement_trait)]

pub mod click;
pub mod delay;
pub mod env;
pub mod filter;
pub mod firmus;
pub mod nulltest;
pub mod osc;
pub mod pulsus;
pub mod reverb;
pub mod smoother;

pub use click::Click;
pub use delay::{DelaySettings, StereoDelay};
pub use env::{Adsr, Stage};
pub use filter::{FilterMode, Svf};
pub use firmus::{Firmus, FirmusSettings, MAX_VOICES};
pub use osc::{PolyBlepOsc, Wave};
pub use pulsus::{DrumVoice, PadSettings, Pulsus, SampleRef, MAX_DRUM_VOICES, MAX_PADS};
pub use reverb::{Reverb, ReverbSettings};
pub use smoother::Smoother;

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
//! | [`eq`] | biquad sections and the three-band channel EQ |
//! | [`aequalis`] | the parametric EQ: matched filters, cuts, dynamic bands |
//! | [`comprimo`] | the compressor: styles as readable detectors, sidechain key, delta |
//! | [`smoother`] | one-pole parameter smoothing |
//! | [`firmus`] | the built-in subtractive polysynth |
//! | [`pulsus`] | the built-in synthesized drum kit |
//! | [`click`] | metronome click |
//! | [`nulltest`] | render comparison harness |

#![forbid(unsafe_code)]
// `next()` is the conventional name for a per-sample generator in DSP code.
#![allow(clippy::should_implement_trait)]

pub mod aequalis;
pub mod calor;
pub mod click;
pub mod comprimo;
pub mod culmen;
pub mod delay;
pub mod env;
pub mod eq;
pub mod exemplar;
pub mod fervor;
pub mod filter;
pub mod firmus;
pub mod gravis;
pub mod lamina;
pub mod latitudo;
pub mod modmatrix;
pub mod modulus;
pub mod multiplex;
pub mod nulltest;
pub mod organum;
pub mod osc;
pub mod porta;
pub mod pulsus;
pub mod reverb;
pub mod sibilus;
pub mod smoother;
pub mod spatium;
pub mod spectrum;
pub mod terminus;
pub mod unda;
pub mod va;
pub mod voices;

pub use aequalis::{Aequalis, BandSettings, EqShape, Placement};
pub use click::Click;
pub use comprimo::{CompMeter, CompSettings, Comprimo, Listen, Style};
pub use delay::{DelaySettings, StereoDelay};
pub use env::{Adsr, Stage};
pub use eq::{BandShape, Biquad, ChannelEq};
pub use filter::{FilterMode, Svf};
pub use firmus::{Firmus, FirmusSettings, MAX_VOICES};
pub use osc::{PolyBlepOsc, Wave};
pub use pulsus::{DrumVoice, PadSettings, Pulsus, SampleRef, MAX_DRUM_VOICES, MAX_PADS};
pub use reverb::{Reverb, ReverbSettings};
pub use smoother::Smoother;
pub use spectrum::SpectrumAnalyser;

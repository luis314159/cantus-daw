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
//! | [`math`] | the transcendental functions, the same bits on every machine |
//! | [`par`] | an instrument's voices on several threads with the bits of one (ADR 0038) |

#![forbid(unsafe_code)]
// `next()` is the conventional name for a per-sample generator in DSP code.
#![allow(clippy::should_implement_trait)]

pub mod aequalis;
pub mod anulus;
pub mod calor;
pub mod chorda;
pub mod click;
pub mod comprimo;
pub mod concentus;
pub mod culmen;
pub mod delay;
pub mod discrimen;
pub mod dynfx_follow;
pub mod dynfx_meter;
pub mod dynfx_os;
pub mod dynfx_transient;
pub mod env;
pub mod eq;
pub mod exemplar;
pub mod fervor;
pub mod filter;
pub mod firmus;
pub mod flexus;
pub mod fluctus;
pub mod gravis;
pub mod impetus;
pub mod lamina;
pub mod latitudo;
pub mod math;
pub mod modfx_clock;
pub mod modfx_hilbert;
pub mod modfx_lfo;
pub mod modfx_line;
pub mod modmatrix;
pub mod modulus;
pub mod multiplex;
pub mod nulltest;
pub mod organum;
pub mod osc;
pub mod par;
pub mod phasis;
pub mod pitch;
pub mod porta;
pub mod psola;
pub mod pulsus;
pub mod rest;
pub mod reverb;
pub mod sibilus;
pub mod smoother;
pub mod spatium;
pub mod spectrum;
pub mod speculum;
pub mod terminus;
pub mod tonsor;
pub mod tonus;
pub mod translatio;
pub mod tremor;
pub mod unda;
pub mod va;
pub mod vibrans;
pub mod voices;

/// The version of the numbers this crate renders. Two builds with the same `DSP_VERSION`
/// render the same project to the same bits, on any machine; a change that moves any
/// unit's output bits (an algorithm, a coefficient, a [`math`] function) bumps it — once,
/// in the commit that re-blesses every golden, whose message gives the reason and the
/// size of the change. A render cache keys its pages on it.
///
/// | Version | Change |
/// |---|---|
/// | 1 | the version that introduced the constant (round 21): std's transcendentals |
/// | 2 | [`math`]: every transcendental from the pinned pure-Rust `libm` (ADR 0031 R9) |
/// | 3 | round 22: [`math::powf`] in `f32` our own (correctly rounded, ~4× faster) and [`math::db_to_gain`] at the per-sample gains; counter-based TPDF dither (R10), so Terminus and Culmen rest with it on; Aequalis's dynamic release lands on 0 dB |
/// | 4 | round 25: the 0 dB pan law and the balance are exactly unity at centre and on the near side (`cos(π/4)·√2` was 0.99999994 in `f32`): a centred strip under that law is one rounding step louder (≤ 6e-8 of the signal); no golden hash moved |
pub const DSP_VERSION: u32 = 4;

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

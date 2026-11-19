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
// r39:conv — Aula, the convolution reverb (ADR 0112)
pub mod aula;
pub mod aula_conv;
pub mod aula_fft;
pub mod aula_gen;
pub mod aula_kernel;
pub mod aula_shape;
pub mod beat;
pub mod caelum; // fx-a:caelum
pub mod calor;
pub mod chorda;
pub mod click;
pub mod comprimo;
pub mod concentus;
pub mod culmen;
pub mod delay;
pub mod denoise; // r36:editor — noise reduction by profile and the spectral region (ADR 0096)
pub mod discrimen;
pub mod dynfx_follow;
pub mod dynfx_meter;
pub mod dynfx_os;
pub mod dynfx_transient;
pub mod env;
pub mod eq;
pub mod exemplar;
pub mod expr_glide;
pub mod fervor;
pub mod filter;
pub mod firmus;
pub mod flexus;
pub mod fluctus;
pub mod gravis;
pub mod impetus;
pub mod lamina;
pub mod latitudo;
pub mod lenis; // fx-b:lenis
pub mod math;
pub mod modfx_clock;
pub mod modfx_hilbert;
pub mod modfx_lfo;
pub mod modfx_line;
pub mod modmatrix;
pub mod modulus;
pub mod multiplex;
pub mod nulltest;
pub mod onset;
pub mod organum;
pub mod osc;
pub mod outmix; // r40:p3 (ADR 0123): a named output's mix on its lane — tymp-engine
pub mod par;
pub mod phasis;
pub mod pitch;
pub mod porta;
pub mod psola;
pub mod pulsus;
pub mod rest;
pub mod reverb;
pub mod sectio; // r37:piano (ADR 0099): the slicer's analysis
pub mod sibilus;
pub mod smoother;
pub mod spatium;
pub mod spectrum;
pub mod speculum;
pub mod stretch;
pub mod terminus;
pub mod tonsor;
pub mod tonus;
pub mod translatio;
pub mod tremor;
pub mod tympanum; // r40:p3 (ADR 0124): the acoustic drummer's DSP — tymp-engine
pub mod tympanum_residual; // r40:drums-hyb (ADR 0124 amended): the hybrid voice — a recorded residual under the model
pub mod unda;
pub mod va;
pub mod vibrans;
pub mod voices;

// fx:contract — the fx side round's five units (ADRs 0127–0130).
pub mod cribrum; // fx:contract
pub mod fragor; // fx:contract
pub mod lapsus; // fx:contract
pub mod libra; // fx:contract
pub mod vetus; // fx:contract

// fx3:contract — the side round's wave 3 (ADRs 0131–0134).
pub mod arca; // fx3:contract
pub mod rota; // fx3:contract
pub mod saltus; // fx3:contract
pub mod tabula; // fx3:contract
pub mod vinculum; // fx:vinculum — the bus compressor (side round, wave B; no contract yet)

// fx-a:loquax — the side round's wave A: Loquax, the vocoder (DSP first).
pub mod loquax; // fx-a:loquax

// fx-a:fractus — wave A: the glitch sequencer and grain cloud (its DSP).
pub mod fractus; // fx-a:fractus

pub mod cedo; // fx-b:cedo

// fx-e:purus — Purus, the real-time repair unit (de-hum, de-reverb; DSP phase).
pub mod purus; // fx-e:purus

// fx-c:comparatio — the reference-track comparator (DSP phase; no kind yet).
pub mod comparatio; // fx-c:comparatio

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
/// | 5 | round 28: an Aequalis band raised (by its knob, automation or a modulator) from exactly 0 dB while no other band was active glides to its gain instead of staying flat for good (`is_active` sees a gain still gliding); that case moves by the whole of the gain asked, every other render is unchanged — no golden hash moved |
/// | 6 | round 37 (piano, voicing pass two): Chorda's model 2 — new this round, merged once — re-voiced (its body's response, its strings' voicing, its output, the perspective stage); model 1 and every other unit unchanged — no golden hash moved r37:piano |
/// | 7 | round 40 (Chorda toward the Salamander, voicing pass three): Chorda's model 2 ships candidate 16 (`chorda::voicing::c16`; beta.14's voicing is [`chorda::voicing::Voicing::beta14`]); model 1 and every other unit unchanged — no golden hash moved, model 2's own pin re-made |
/// | 8 | fx/final (wave D's finding, ADR 0027 amended): at a jump (a start, a seek, a loop's wrap, a sync start or jump) every unit a lane or modulator drives takes its values at once (`snap`) instead of gliding ~20 ms from its stored ones, and Firmus takes its first settings at once — a render or playback from bar N is the whole render from bar N. Only renders with a lane on a native effect or instrument move, in what their glides were at the start (an EQ band held at +9 dB: 5.6e-2 in the first 20 ms, 1.3e-3 by 50 ms, under 3e-5 after, nothing after 0.5 s; what entered a delay line, a reverb or an LFO's phase in those milliseconds is carried in their tails) — no golden hash moved (v5, the Pulsus kits v1, v14 and v1 from bar 2, the post-fader sends) |
/// | 9 | fx/final (Gravis at the Free phase): an ended voice's oscillators stand still (the synth's main, unison and second phases, the 808's phase) instead of running on to the end of the block it ended in and standing in idle ones — the next note's phase was the block split's, so a render and real time disagreed (8.2e-1 live against the render). Only Gravis at the Free phase moves, from the first note after a voice has ended (the synth engine drawn over its range: up to 1.29 from its second chord, a phase moved; the 808 at its defaults: the same bits) — no golden hash moved |
/// | 10 | round 40, the technician's pass (the owner heard Bb2, B2, C3 out of tune): Chorda's model 2 ships candidate 16 with its unisons centred on their keys, its strings tuned on their bridge, detune room in every loop, sixteen unisons redrawn and a measured table on the keyboard (`chorda::voicing::tune`; candidate 16 is [`chorda::voicing::Voicing::candidate16`], pinned); model 1, beta.14's voicing and every other unit unchanged — no golden hash moved, model 2's own pin re-made (ADR 0098's fourth amendment) |
pub const DSP_VERSION: u32 = 10;

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

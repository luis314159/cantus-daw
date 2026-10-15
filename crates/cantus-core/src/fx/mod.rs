//! One module per plug-in's parameter model, so each plug-in's model is owned by one file.
//!
//! Every module exports the same four things — `NAME_KEY`, `SPARSE`, `TAKES_SIDECHAIN` and
//! `params()` — and [`crate::EffectKind`] dispatches to them. Adding a plug-in is a new
//! module, a new variant and one line in each dispatch.

pub mod anulus;
pub mod compressor;
pub mod concentus;
pub mod deesser;
pub mod discrimen;
pub mod eq;
pub mod flexus;
pub mod fluctus;
pub mod gate;
pub mod impetus;
pub mod limiter;
pub mod maximizer;
pub mod multiband;
pub mod phasis;
pub mod saturator;
pub mod space;
pub mod speculum;
pub mod stereo;
pub mod tonsor;
pub mod tonus;
pub mod translatio;
pub mod tremor;
pub mod vibrans;

/// How any parameter's value is printed and read back (shared by the DAW's knobs and the
/// exported plug-ins, ADR 0020).
pub mod readout;

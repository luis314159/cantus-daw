//! One module per plug-in's parameter model, so each plug-in's model is owned by one file.
//!
//! Every module exports the same four things — `NAME_KEY`, `SPARSE`, `TAKES_SIDECHAIN` and
//! `params()` — and [`crate::EffectKind`] dispatches to them. Adding a plug-in is a new
//! module, a new variant and one line in each dispatch.

pub mod anulus;
pub mod aula; // r39:conv
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

// fx:contract — the fx side round's five units (provisional ADRs 0130–0133).
pub mod cribrum; // fx:contract
pub mod fragor; // fx:contract
pub mod lapsus; // fx:contract
pub mod libra; // fx:contract
pub mod vetus; // fx:contract

// fx3:contract — the side round's wave 3 (provisional ADRs 0134–0137).
pub mod arca; // fx3:contract
pub mod rota; // fx3:contract
pub mod saltus; // fx3:contract
pub mod tabula; // fx3:contract
pub mod tabula_presets; // fx3:tabula — boards as flat maps, the factory boards
pub mod tabula_random; // fx3:tabula — Randomise within musical ranges

// fxa:contract — the side round's wave A (provisional ADRs 0139–0141).
pub mod caelum; // fxa:contract
pub mod fractus; // fxa:contract
pub mod loquax; // fxa:contract

/// How any parameter's value is printed and read back (shared by the DAW's knobs and the
/// exported plug-ins, ADR 0020).
pub mod readout;

//! Fragor: the distortion and fuzz pedals (TS, RAT, DS-1, Fuzz Face, Big Muff), circuit-modelled. The fx side round (ADR 0131, provisional), filled by its builder.

/// The circuit solver (the DK method).
#[path = "fragor_dk.rs"]
pub mod dk;
/// The five pedals' circuits.
#[path = "fragor_circuits.rs"]
pub mod circuits;

pub use circuits::{Model, Pedal};

#[cfg(test)]
#[path = "fragor_tests.rs"]
mod tests;
#[cfg(test)]
#[path = "fragor_spice_tests.rs"]
mod spice_tests;

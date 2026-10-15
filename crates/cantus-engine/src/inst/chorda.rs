//! Chorda — the modelled grand piano: the engine's unit. Owned by this instrument's
//! builder (ADR 0023).
//!
//! **Round 28's contract**: a silent scaffold — it renders nothing and is always at rest —
//! until its builder fills it with `cantus_dsp::chorda`.

use super::{InstrumentUnit, NoteOn};
use cantus_core::NativeSpec;

#[derive(Debug)]
pub struct Unit;

impl Unit {
    pub fn new(sample_rate: f32) -> Self {
        let _ = sample_rate;
        Unit
    }
}

impl InstrumentUnit for Unit {
    fn configure(&mut self, _spec: &NativeSpec, _assets: &super::Assets) {}

    fn note_on(&mut self, _note: NoteOn) {}

    fn note_off(&mut self, _key: u8, _id: u64, _velocity: f32) {}

    fn all_notes_off(&mut self) {}

    fn reset(&mut self) {}

    /// Writes nothing: the scaffold is silent.
    fn process(&mut self, _left: &mut [f32], _right: &mut [f32]) -> bool {
        false
    }

    /// No state at all: always at rest.
    fn at_rest(&self) -> bool {
        true
    }

    fn active_voices(&self) -> usize {
        0
    }
}

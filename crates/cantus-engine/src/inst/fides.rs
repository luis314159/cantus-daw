//! Fides's unit (round 41, ADR 0150): the resonator instrument on the audio thread.
//! Owned by the `fides` builder.
//!
//! **Contract stub**: a unit that makes no sound — `process` returns `false` and touches no
//! sample, and it is at rest from birth — so the kind's dispatch exists from the first day
//! while `InstrumentKind::Resonator` stays out of `InstrumentKind::ALL` and the picker. The
//! `fides` builder replaces it with the real engine: exciters (pluck, pick, a bow by a
//! friction curve, a mallet, noise, a live input) into waveguide strings and modal bars,
//! plates, membranes and tubes with a body, voices split across the pool with the bits of
//! one thread (ADR 0038), its model version read from the spec (`fides::MODEL`).

use super::{InstrumentUnit, NoteOn};
use cantus_core::NativeSpec;

#[derive(Debug)]
pub struct Unit {
    sample_rate: f32,
}

impl Unit {
    pub fn new(sample_rate: f32) -> Self {
        Unit { sample_rate }
    }

    pub fn sample_rate(&self) -> f32 {
        self.sample_rate
    }
}

impl InstrumentUnit for Unit {
    fn configure(&mut self, _spec: &NativeSpec, _assets: &super::Assets) {}

    fn note_on(&mut self, _note: NoteOn) {}

    fn note_off(&mut self, _key: u8, _id: u64, _velocity: f32) {}

    fn all_notes_off(&mut self) {}

    fn reset(&mut self) {}

    fn process(&mut self, _left: &mut [f32], _right: &mut [f32]) -> bool {
        false
    }

    fn at_rest(&self) -> bool {
        true
    }

    fn active_voices(&self) -> usize {
        0
    }
}

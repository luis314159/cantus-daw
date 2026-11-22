//! Arcus's unit (round 42, ADR 0157): the bowed string on the audio thread. Owned by the
//! `arcus` builder.
//!
//! **Contract stub**: a unit that makes no sound — `process` returns `false` and touches no
//! sample, and it is at rest from birth — so the kind's dispatch exists from the first day
//! while `InstrumentKind::BowedString` stays out of `InstrumentKind::ALL` and the picker. The
//! `arcus` builder replaces it with the real engine: a bowed waveguide string on Fides's bow
//! and body, its tuning through `set_tuning` (ADR 0155), each note's articulation and bow
//! through `note_art` (ADR 0158), voices split across the pool with the bits of one thread
//! (ADR 0038), its model version read from the spec (`arcus::MODEL`).

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

//! Fragor's five circuits as board-only pedals of their own (round 41, ADR 0152): the
//! engine's unit. Owned by the `pedals` builder.
//!
//! The builder's unit is Fragor's (`cantus_dsp::fragor::Fragor`) with only this pedal's
//! circuit in its first slot, so a pedal and a one-slot Fragor give the same bits (a test of
//! all five). **The contract's stub** is a wire times `out` (exactly the input at 0 dB).

use super::FxUnit;
use cantus_core::EffectSpec;

#[derive(Clone, Debug)]
pub struct Unit {
    /// The circuit (0 … 4: overdrive, op-amp, silicon, fuzz, muff — Fragor's model order).
    pub circuit: usize,
    gain: f32,
}

impl Unit {
    pub fn new(circuit: usize, sample_rate: f32) -> Unit {
        let _ = sample_rate;
        Unit {
            circuit: circuit.min(4),
            gain: 1.0,
        }
    }
}

impl FxUnit for Unit {
    fn fork(&self) -> Option<Box<dyn FxUnit>> {
        Some(Box::new(self.clone()))
    }
    fn configure(&mut self, spec: &EffectSpec) {
        self.gain = cantus_dsp::math::db_to_gain(spec.get("out"));
    }
    fn process(&mut self, left: &mut [f32], right: &mut [f32], _key: Option<(&[f32], &[f32])>) {
        if self.gain == 1.0 {
            return;
        }
        for x in left.iter_mut().chain(right.iter_mut()) {
            *x *= self.gain;
        }
    }
    fn reset(&mut self) {}
    fn at_rest(&self) -> bool {
        true
    }
}

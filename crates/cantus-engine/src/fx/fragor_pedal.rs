//! Fragor's five circuits as board-only pedals of their own (round 41, ADR 0152): the
//! engine's unit. Owned by the `pedals` builder.
//!
//! Each pedal is Fragor's DSP with only its circuit in the first slot ([`dsp::One`]: Fragor's
//! board built with that one circuit), so a pedal and a one-slot Fragor give the same bits
//! (`tests/fx_fragor_pedal.rs`, all five, at every setting); its keys are Fragor's
//! (`cantus_core::fx::fragor_pedal`). Its telemetry is a one-slot Fragor's.

use super::{FxUnit, Telemetry, TELEMETRY_LEN};
use cantus_core::fx::fragor_pedal::{knobs, switch};
use cantus_core::EffectSpec;
use cantus_dsp::fragor::{Model, FACTORS};

/// One circuit on Fragor's board, built alone.
#[path = "fragor_pedal_dsp.rs"]
pub mod dsp;

/// A pedal's parameters as the DSP wants them (`fragor::settings`'s reading, for one circuit).
pub fn settings(circuit: usize, spec: &EffectSpec) -> dsp::Settings {
    let i = circuit.min(4);
    let k = knobs(i);
    let os = spec.get("os").round().clamp(0.0, 2.0) as usize;
    dsp::Settings {
        on: spec.get(switch(i)) >= 0.5,
        knobs: [0, 1, 2].map(|j| spec.get(k[j]).clamp(0.0, 1.0)),
        gate_db: spec.get("gate"),
        in_db: spec.get("in"),
        out_db: spec.get("out"),
        mix: spec.get("mix"),
        mono: spec.get("mono") >= 0.5,
        oversampling: FACTORS[os],
    }
}

#[derive(Clone, Debug)]
pub struct Unit {
    /// The circuit (0 … 4: overdrive, op-amp, silicon, fuzz, muff — Fragor's model order).
    pub circuit: usize,
    dsp: dsp::One,
    /// Read into on the audio thread (plain data: setting it never allocates).
    scratch: dsp::Settings,
}

impl Unit {
    pub fn new(circuit: usize, sample_rate: f32) -> Unit {
        let circuit = circuit.min(4);
        let model = Model::from_index(circuit);
        Unit {
            circuit,
            dsp: dsp::One::new(model, sample_rate),
            scratch: dsp::Settings::new(model),
        }
    }
}

impl FxUnit for Unit {
    fn fork(&self) -> Option<Box<dyn FxUnit>> {
        Some(Box::new(self.clone()))
    }

    fn configure(&mut self, spec: &EffectSpec) {
        self.scratch = settings(self.circuit, spec);
        self.dsp.set(&self.scratch);
    }

    fn process(&mut self, left: &mut [f32], right: &mut [f32], _key: Option<(&[f32], &[f32])>) {
        self.dsp.process(left, right);
    }

    /// At a start, a seek or a loop's wrap: every glide at its target (Fragor's `snap`).
    fn snap(&mut self) {
        self.dsp.snap();
    }

    fn reset(&mut self) {
        self.dsp.reset();
    }

    fn at_rest(&self) -> bool {
        self.dsp.at_rest()
    }

    fn rest_blocker(&self) -> Option<&'static str> {
        self.dsp.rest_blocker()
    }

    fn skip(&mut self, frames: usize) {
        self.dsp.skip(frames);
    }

    fn latency(&self) -> usize {
        self.dsp.latency()
    }

    fn telemetry(&self) -> Telemetry {
        let mut t = [0.0; TELEMETRY_LEN];
        self.dsp.write_telemetry(&mut t);
        t
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cantus_core::fx::fragor_pedal::{as_fragor, KINDS};

    /// The table's defaults read as the DSP's defaults, and a pedal's reading is the
    /// one-slot Fragor's (`fragor::settings` of [`as_fragor`]).
    #[test]
    fn a_pedal_reads_as_its_one_slot_fragor() {
        for (i, kind) in KINDS.iter().enumerate() {
            let m = Model::from_index(i);
            let d = EffectSpec::new(*kind);
            assert_eq!(settings(i, &d), dsp::Settings::new(m));
            let mut s = EffectSpec::new(*kind);
            s.set(switch(i), 1.0);
            s.set(knobs(i)[1], 0.2);
            s.set("gate", -60.0);
            s.set("mono", 1.0);
            s.set("os", 0.0);
            assert_eq!(
                settings(i, &s).board(m),
                super::super::fragor::settings(&as_fragor(&s)),
                "{kind:?}"
            );
        }
    }

    /// At its defaults the unit touches no sample, to the bit, and adds no latency.
    #[test]
    fn the_defaults_touch_no_sample() {
        for (i, kind) in KINDS.iter().enumerate() {
            let mut u = Unit::new(i, 48_000.0);
            u.configure(&EffectSpec::new(*kind));
            let x: Vec<f32> = (0..64).map(|i| (i as f32 * 0.37).fract() - 0.5).collect();
            let (mut l, mut r) = (x.clone(), x.clone());
            u.process(&mut l, &mut r, None);
            assert!(l
                .iter()
                .chain(&r)
                .zip(x.iter().chain(&x))
                .all(|(a, b)| a.to_bits() == b.to_bits()));
            assert_eq!(u.latency(), 0);
        }
    }
}

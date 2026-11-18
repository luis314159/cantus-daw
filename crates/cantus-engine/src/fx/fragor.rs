//! Fragor (the pedals, ADR 0128) behind the engine's plug-in contract. Owned by Fragor.
//!
//! [`settings`] reads the parameters (`cantus_core::fx::fragor`) into the DSP's terms
//! (`cantus_dsp::fragor`); the editor draws its pedals and computes its curves from the
//! same function, so the picture and the sound come from one reading of the spec.
//!
//! ## Telemetry (64 floats, refreshed after every block)
//!
//! The layout is `cantus_dsp::fragor::telemetry`'s; the constants there name each slot:
//! the block's input and output peaks, the gate's gain and state, latency, the sample rate
//! and the oversampling factor, whether the wet path runs and whether it rests, each
//! pedal's output peak and footswitch share, and the chain as it sounds.

use super::{FxUnit, Telemetry, TELEMETRY_LEN};
use cantus_core::fx::fragor::{KNOBS, SLOTS, SWITCHES};
use cantus_core::EffectSpec;
use cantus_dsp::fragor::{BoardSettings, Fragor, Model, FACTORS};

/// Fragor's parameters as the DSP wants them; also what its editor draws from. A pedal in
/// two slots sounds in the first only.
pub fn settings(spec: &EffectSpec) -> BoardSettings {
    let mut chain = [None; 3];
    for (s, key) in SLOTS.iter().enumerate() {
        let v = spec.get(key).round().clamp(0.0, 5.0) as usize;
        if v > 0 {
            let m = Model::from_index(v - 1);
            if !chain[..s].contains(&Some(m)) {
                chain[s] = Some(m);
            }
        }
    }
    let mut on = [true; 5];
    let mut knobs = [[0.5f32; 3]; 5];
    for m in 0..5 {
        on[m] = spec.get(SWITCHES[m]) >= 0.5;
        for k in 0..3 {
            knobs[m][k] = spec.get(KNOBS[m][k]).clamp(0.0, 1.0);
        }
    }
    let os = spec.get("os").round().clamp(0.0, 2.0) as usize;
    BoardSettings {
        chain,
        on,
        knobs,
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
    dsp: Fragor,
    /// Read into on the audio thread: `BoardSettings` holds no heap data, so setting it
    /// never allocates.
    scratch: BoardSettings,
}

impl Unit {
    pub fn new(sample_rate: f32) -> Self {
        Unit {
            dsp: Fragor::new(sample_rate),
            scratch: BoardSettings::default(),
        }
    }
}

impl FxUnit for Unit {
    fn fork(&self) -> Option<Box<dyn FxUnit>> {
        Some(Box::new(self.clone()))
    }

    fn configure(&mut self, spec: &EffectSpec) {
        self.scratch = settings(spec);
        self.dsp.set(&self.scratch);
    }

    fn process(&mut self, left: &mut [f32], right: &mut [f32], _key: Option<(&[f32], &[f32])>) {
        self.dsp.process(left, right);
    }

    /// fx/final — at a start, a seek or a loop's wrap: every glide at its target
    /// (`Fragor::snap`).
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
    use cantus_core::EffectKind;

    /// The defaults of the table read as the DSP's defaults: the null rule holds through
    /// the mapping.
    #[test]
    fn the_tables_defaults_are_the_dsps_defaults() {
        let spec = EffectSpec::new(EffectKind::Pedal);
        assert_eq!(settings(&spec), BoardSettings::default());
    }

    /// A pedal named in two slots sounds once, in the first.
    #[test]
    fn a_pedal_sounds_in_one_slot_only() {
        let mut spec = EffectSpec::new(EffectKind::Pedal);
        spec.set("s1", 4.0);
        spec.set("s2", 4.0);
        spec.set("s3", 1.0);
        let s = settings(&spec);
        assert_eq!(s.chain, [Some(Model::Fuzz), None, Some(Model::Overdrive)]);
    }

    /// At its defaults the unit touches no sample, to the bit, and adds no latency.
    #[test]
    fn the_defaults_touch_no_sample() {
        let mut u = Unit::new(48_000.0);
        u.configure(&EffectSpec::new(EffectKind::Pedal));
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

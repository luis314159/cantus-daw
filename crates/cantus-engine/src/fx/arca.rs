//! Arca, amp and cabinet: preamp stages, tone stack, power amp, the cabinet by impulse response, behind the engine's plug-in contract.
//! Owned by the arca builder.
//!
//! **The fx side round's wave-3 contract**: [`settings`] reads the placeholder table
//! (`cantus_core::fx::arca`) and the unit passes the audio through with its output gain
//! — at the default, 0 dB, it touches no sample. Its builder replaces this file with the
//! DSP of `cantus_dsp::arca`.

use super::FxUnit;
use cantus_core::EffectSpec;

/// Arca's parameters as the DSP wants them; also what its editor draws from.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Settings {
    pub out_db: f32,
}

/// Arca's parameters as the DSP wants them.
pub fn settings(spec: &EffectSpec) -> Settings {
    Settings {
        out_db: spec.get("out"),
    }
}

#[derive(Clone, Debug)]
pub struct Unit {
    gain: f32,
}

impl Unit {
    pub fn new(sample_rate: f32) -> Self {
        let _ = sample_rate;
        Unit { gain: 1.0 }
    }
}

impl FxUnit for Unit {
    fn fork(&self) -> Option<Box<dyn FxUnit>> {
        Some(Box::new(self.clone()))
    }

    fn configure(&mut self, spec: &EffectSpec) {
        let s = settings(spec);
        self.gain = if s.out_db == 0.0 {
            1.0
        } else {
            cantus_dsp::math::db_to_gain(s.out_db)
        };
    }

    fn process(&mut self, left: &mut [f32], right: &mut [f32], _key: Option<(&[f32], &[f32])>) {
        if self.gain != 1.0 {
            for x in left.iter_mut().chain(right.iter_mut()) {
                *x *= self.gain;
            }
        }
    }

    fn reset(&mut self) {}

    /// No state at all: always at rest.
    fn at_rest(&self) -> bool {
        true
    }

    fn skip(&mut self, _frames: usize) {}

    fn latency(&self) -> usize {
        0
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
        let spec = EffectSpec::new(EffectKind::Amp);
        assert_eq!(settings(&spec), Settings::default());
    }

    /// At its defaults the unit touches no sample, to the bit.
    #[test]
    fn the_defaults_touch_no_sample() {
        let mut u = Unit::new(48_000.0);
        u.configure(&EffectSpec::new(EffectKind::Amp));
        let x: Vec<f32> = (0..64).map(|i| (i as f32 * 0.37).fract() - 0.5).collect();
        let (mut l, mut r) = (x.clone(), x.clone());
        u.process(&mut l, &mut r, None);
        assert!(l
            .iter()
            .chain(&r)
            .zip(x.iter().chain(&x))
            .all(|(a, b)| a.to_bits() == b.to_bits()));
        assert!(u.at_rest());
        assert_eq!(u.latency(), 0);
    }
}

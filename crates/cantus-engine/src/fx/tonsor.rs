//! Tonsor, the clipper with a true-peak ceiling, behind the engine's plug-in contract
//! (ADR 0056). Owned by Tonsor.
//!
//! [`settings`] reads the table (`cantus_core::fx::tonsor`) into the DSP's settings
//! (`cantus_dsp::tonsor`); the editor draws its curve from the same reading. The telemetry
//! is the DSP's (`cantus_dsp::tonsor::tel`).

use super::{FxUnit, Telemetry, TELEMETRY_LEN};
use cantus_core::EffectSpec;
use cantus_dsp::tonsor::{Curve, Tonsor, TonsorSettings};

/// Tonsor's parameters as the DSP wants them; also what its editor draws from.
pub fn settings(spec: &EffectSpec) -> TonsorSettings {
    TonsorSettings {
        drive_db: spec.get("drive"),
        ceiling_db: spec.get("ceil"),
        knee_db: spec.get("knee"),
        curve: Curve::from_index(spec.get("curve")),
        oversampling: cantus_dsp::dynfx_os::factor_of(spec.get("os")),
        true_peak: spec.get("tp") >= 0.5,
        match_gain: spec.get("match") >= 0.5,
        out_db: spec.get("out"),
    }
}

#[derive(Clone, Debug)] // r34:listen: Clone for fork
pub struct Unit {
    dsp: Box<Tonsor>,
}

impl Unit {
    pub fn new(sample_rate: f32) -> Self {
        Unit {
            dsp: Box::new(Tonsor::new(sample_rate)),
        }
    }
}

impl FxUnit for Unit {
    fn fork(&self) -> Option<Box<dyn FxUnit>> {
        Some(Box::new(self.clone()))
    } // r34:listen
    fn configure(&mut self, spec: &EffectSpec) {
        self.dsp.set(&settings(spec));
    }

    fn process(&mut self, left: &mut [f32], right: &mut [f32], _key: Option<(&[f32], &[f32])>) {
        self.dsp.process(left, right);
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
        self.dsp.telemetry(&mut t);
        t
    }

    /// The deepest reduction the curve made lately, dB: the master's reduction meter.
    fn gain_reduction_db(&self) -> Option<f32> {
        let mut t = [0.0; TELEMETRY_LEN];
        self.dsp.telemetry(&mut t);
        Some(t[cantus_dsp::tonsor::tel::REDUCTION].max(t[cantus_dsp::tonsor::tel::GUARD]))
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
        let spec = EffectSpec::new(EffectKind::Clipper);
        assert_eq!(settings(&spec), TonsorSettings::default());
    }

    #[test]
    fn the_settings_read_every_parameter() {
        let mut spec = EffectSpec::new(EffectKind::Clipper);
        for (k, v) in [
            ("drive", 6.0),
            ("ceil", -1.5),
            ("knee", 3.0),
            ("curve", 2.0),
            ("os", 3.0),
            ("tp", 1.0),
            ("match", 1.0),
            ("out", -2.0),
        ] {
            spec.set(k, v);
        }
        let s = settings(&spec);
        assert_eq!(
            s,
            TonsorSettings {
                drive_db: 6.0,
                ceiling_db: -1.5,
                knee_db: 3.0,
                curve: Curve::Analog,
                oversampling: 8,
                true_peak: true,
                match_gain: true,
                out_db: -2.0,
            }
        );
    }

    /// At its defaults the unit touches no sample, to the bit, and is at rest.
    #[test]
    fn the_defaults_touch_no_sample() {
        let mut u = Unit::new(48_000.0);
        u.configure(&EffectSpec::new(EffectKind::Clipper));
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

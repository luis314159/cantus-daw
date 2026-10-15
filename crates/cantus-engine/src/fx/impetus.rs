//! Impetus, the transient processor, behind the engine's plug-in contract (ADR 0056).
//! Owned by Impetus.
//!
//! [`settings`] reads the table (`cantus_core::fx::impetus`) into the DSP's settings
//! (`cantus_dsp::impetus`); the editor draws from the same reading. The key, when a strip
//! is routed to it, is what the detector hears.

use super::{FxUnit, Telemetry, TELEMETRY_LEN};
use cantus_core::EffectSpec;
use cantus_dsp::impetus::{Band, Impetus, ImpetusSettings};

/// Impetus's parameters as the DSP wants them; also what its editor draws from.
pub fn settings(spec: &EffectSpec) -> ImpetusSettings {
    ImpetusSettings {
        attack: spec.get("attack"),
        sustain: spec.get("sustain"),
        speed_ms: spec.get("speed"),
        length_ms: spec.get("length"),
        band: Band::from_index(spec.get("band")),
        xover_hz: spec.get("xover"),
        out_db: spec.get("out"),
    }
}

#[derive(Debug)]
pub struct Unit {
    dsp: Box<Impetus>,
}

impl Unit {
    pub fn new(sample_rate: f32) -> Self {
        Unit {
            dsp: Box::new(Impetus::new(sample_rate)),
        }
    }
}

impl FxUnit for Unit {
    fn configure(&mut self, spec: &EffectSpec) {
        self.dsp.set(&settings(spec));
    }

    fn process(&mut self, left: &mut [f32], right: &mut [f32], key: Option<(&[f32], &[f32])>) {
        self.dsp.process(left, right, key);
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use cantus_core::EffectKind;

    #[test]
    fn the_tables_defaults_are_the_dsps_defaults() {
        let spec = EffectSpec::new(EffectKind::Transient);
        assert_eq!(settings(&spec), ImpetusSettings::default());
    }

    #[test]
    fn the_settings_read_every_parameter() {
        let mut spec = EffectSpec::new(EffectKind::Transient);
        for (k, v) in [
            ("attack", 0.5),
            ("sustain", -0.25),
            ("speed", 4.0),
            ("length", 600.0),
            ("band", 2.0),
            ("xover", 900.0),
            ("out", -3.0),
        ] {
            spec.set(k, v);
        }
        assert_eq!(
            settings(&spec),
            ImpetusSettings {
                attack: 0.5,
                sustain: -0.25,
                speed_ms: 4.0,
                length_ms: 600.0,
                band: Band::Lows,
                xover_hz: 900.0,
                out_db: -3.0,
            }
        );
    }

    #[test]
    fn the_defaults_touch_no_sample() {
        let mut u = Unit::new(48_000.0);
        u.configure(&EffectSpec::new(EffectKind::Transient));
        let x: Vec<f32> = (0..64).map(|i| (i as f32 * 0.37).fract() - 0.5).collect();
        let (mut l, mut r) = (x.clone(), x.clone());
        u.process(&mut l, &mut r, None);
        assert!(l
            .iter()
            .chain(&r)
            .zip(x.iter().chain(&x))
            .all(|(a, b)| a.to_bits() == b.to_bits()));
    }
}

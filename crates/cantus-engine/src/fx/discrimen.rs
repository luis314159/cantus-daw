//! Discrimen, the transient splitter, behind the engine's plug-in contract (ADR 0056).
//! Owned by Discrimen.
//!
//! [`settings`] reads the table (`cantus_core::fx::discrimen`) into the DSP's settings
//! (`cantus_dsp::discrimen`); the editor draws from the same reading.

use super::{FxUnit, Telemetry, TELEMETRY_LEN};
use cantus_core::EffectSpec;
use cantus_dsp::discrimen::{Discrimen, DiscrimenSettings, Listen, Part};

/// Discrimen's parameters as the DSP wants them; also what its editor draws from.
pub fn settings(spec: &EffectSpec) -> DiscrimenSettings {
    // The keys spelled out: this runs on the audio thread, which never formats a string.
    let part = |gain: &str, comp: &str, tilt: &str| Part {
        gain_db: spec.get(gain),
        comp: spec.get(comp),
        tilt_db: spec.get(tilt),
    };
    DiscrimenSettings {
        look_ahead: spec.get("mode") >= 0.5,
        sensitivity: spec.get("sens"),
        speed_ms: spec.get("speed"),
        transient: part("t.gain", "t.comp", "t.tilt"),
        body: part("b.gain", "b.comp", "b.tilt"),
        listen: Listen::from_index(spec.get("listen")),
        out_db: spec.get("out"),
    }
}

#[derive(Debug)]
pub struct Unit {
    dsp: Box<Discrimen>,
}

impl Unit {
    pub fn new(sample_rate: f32) -> Self {
        Unit {
            dsp: Box::new(Discrimen::new(sample_rate)),
        }
    }
}

impl FxUnit for Unit {
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use cantus_core::EffectKind;

    #[test]
    fn the_tables_defaults_are_the_dsps_defaults() {
        let spec = EffectSpec::new(EffectKind::TransientSplit);
        assert_eq!(settings(&spec), DiscrimenSettings::default());
    }

    #[test]
    fn the_settings_read_every_parameter() {
        let mut spec = EffectSpec::new(EffectKind::TransientSplit);
        for (k, v) in [
            ("mode", 1.0),
            ("sens", 0.8),
            ("speed", 6.0),
            ("t.gain", 3.0),
            ("t.comp", 0.2),
            ("t.tilt", -2.0),
            ("b.gain", -4.0),
            ("b.comp", 0.6),
            ("b.tilt", 1.5),
            ("listen", 2.0),
            ("out", -1.0),
        ] {
            spec.set(k, v);
        }
        assert_eq!(
            settings(&spec),
            DiscrimenSettings {
                look_ahead: true,
                sensitivity: 0.8,
                speed_ms: 6.0,
                transient: Part {
                    gain_db: 3.0,
                    comp: 0.2,
                    tilt_db: -2.0
                },
                body: Part {
                    gain_db: -4.0,
                    comp: 0.6,
                    tilt_db: 1.5
                },
                listen: Listen::Body,
                out_db: -1.0,
            }
        );
    }
}

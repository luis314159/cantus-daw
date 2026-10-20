//! Flexus, the through-zero flanger, behind the engine's plug-in contract (ADR 0055).
//! Owned by Flexus.
//!
//! [`settings`] reads the table (`cantus_core::fx::flexus`) into the DSP's settings
//! (`cantus_dsp::flexus`); the editor draws from the same reading. Through zero, the unit
//! reports the dry path's 5 ms as its latency (`FxUnit::latency`); plain, none — an
//! automated switch is re-planned by the engine (`replan_if_latency_moved`, ADR 0056). The
//! telemetry is the
//! family's header and the sweep (`cantus_dsp::flexus::{T_SWEEP, T_DELAY_MS, T_ENV_DB,
//! T_LATENCY}`).

use super::modfx::{lfo_settings, spread_cycles, telemetry_of, transport_of};
use super::{FxUnit, SongPosition, Telemetry};
use cantus_core::EffectSpec;
use cantus_dsp::flexus::{Flexus, FlexusSettings, Mode};

/// Flexus's parameters as the DSP wants them; also what its editor draws from.
pub fn settings(spec: &EffectSpec) -> FlexusSettings {
    FlexusSettings {
        mode: Mode::from_index(spec.get("mode")),
        through_zero: spec.get("tz") >= 0.5,
        mix: spec.get("mix"),
        delay_ms: spec.get("delay"),
        depth_ms: spec.get("depth"),
        feedback: spec.get("fb"),
        invert: spec.get("invert") >= 0.5,
        lfo: lfo_settings(spec, 0),
        spread: spread_cycles(spec.get("spread")),
        manual: spec.get("manual"),
        attack_ms: spec.get("att"),
        release_ms: spec.get("rel"),
        sens_db: spec.get("sens"),
        out_db: spec.get("out"),
    }
}

#[derive(Debug)]
pub struct Unit {
    dsp: Flexus,
}

impl Unit {
    pub fn new(sample_rate: f32) -> Self {
        Unit {
            dsp: Flexus::new(sample_rate),
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
        telemetry_of(|t| self.dsp.telemetry(t))
    }

    fn follows_song(&self) -> bool {
        true
    }

    fn set_position(&mut self, position: &SongPosition) {
        self.dsp.set_transport(&transport_of(position));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cantus_core::EffectKind;

    #[test]
    fn the_tables_defaults_are_the_dsps_defaults() {
        assert_eq!(
            settings(&EffectSpec::new(EffectKind::Flanger)),
            FlexusSettings::default()
        );
    }

    #[test]
    fn the_settings_read_every_parameter() {
        let mut spec = EffectSpec::new(EffectKind::Flanger);
        for (k, v) in [
            ("mode", 1.0),
            ("tz", 1.0),
            ("mix", 0.5),
            ("delay", 1.0),
            ("depth", 6.0),
            ("fb", -0.5),
            ("invert", 1.0),
            ("spread", 90.0),
            ("manual", 0.2),
            ("att", 3.0),
            ("rel", 300.0),
            ("sens", 12.0),
            ("out", 1.0),
        ] {
            spec.set(k, v);
        }
        let s = settings(&spec);
        assert_eq!(s.mode, Mode::Envelope);
        assert!(s.through_zero && s.invert);
        assert_eq!(
            (s.mix, s.delay_ms, s.depth_ms, s.feedback),
            (0.5, 1.0, 6.0, -0.5)
        );
        assert_eq!((s.spread, s.manual), (0.25, 0.2));
        assert_eq!(
            (s.attack_ms, s.release_ms, s.sens_db, s.out_db),
            (3.0, 300.0, 12.0, 1.0)
        );
        let u = {
            let mut u = Unit::new(48_000.0);
            u.configure(&spec);
            u
        };
        assert_eq!(u.latency(), 240, "through zero: 5 ms at 48 kHz");
        spec.set("tz", 0.0);
        let mut u = u;
        u.configure(&spec);
        assert_eq!(u.latency(), 0, "plain: none");
    }
}

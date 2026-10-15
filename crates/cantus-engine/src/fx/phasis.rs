//! Phasis, the phaser, behind the engine's plug-in contract (ADR 0055). Owned by Phasis.
//!
//! [`settings`] reads the table (`cantus_core::fx::phasis`) into the DSP's settings
//! (`cantus_dsp::phasis`); the editor draws from the same reading. The telemetry is the
//! family's header and the stages' frequency (`cantus_dsp::phasis::{T_FC_L, T_FC_R}`).

use super::modfx::{lfo_settings, spread_cycles, telemetry_of, transport_of};
use super::{FxUnit, SongPosition, Telemetry};
use cantus_core::EffectSpec;
use cantus_dsp::phasis::{Phasis, PhasisSettings};

/// The seed of Phasis's random wave.
const SEED: u64 = 0x9a51_0001;

/// Phasis's parameters as the DSP wants them; also what its editor draws from.
pub fn settings(spec: &EffectSpec) -> PhasisSettings {
    PhasisSettings {
        stages: spec.get("stages").round().clamp(2.0, 12.0) as usize,
        mix: spec.get("mix"),
        lo_hz: spec.get("lo"),
        hi_hz: spec.get("hi"),
        feedback: spec.get("fb"),
        lfo: lfo_settings(spec, SEED),
        spread: spread_cycles(spec.get("spread")),
        out_db: spec.get("out"),
    }
}

#[derive(Debug)]
pub struct Unit {
    dsp: Phasis,
}

impl Unit {
    pub fn new(sample_rate: f32) -> Self {
        Unit {
            dsp: Phasis::new(sample_rate),
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

    fn set_position(&mut self, position: &SongPosition) {
        self.dsp.set_transport(&transport_of(position));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cantus_core::EffectKind;
    use cantus_dsp::modfx_lfo::{Rate, Shape};

    #[test]
    fn the_tables_defaults_are_the_dsps_defaults() {
        let s = settings(&EffectSpec::new(EffectKind::Phaser));
        let d = PhasisSettings::default();
        assert_eq!((s.stages, s.mix, s.lo_hz, s.hi_hz), (d.stages, d.mix, d.lo_hz, d.hi_hz));
        assert_eq!((s.feedback, s.spread, s.out_db), (d.feedback, d.spread, d.out_db));
        assert_eq!(s.lfo.rate, d.lfo.rate);
    }

    #[test]
    fn the_settings_read_every_parameter() {
        let mut spec = EffectSpec::new(EffectKind::Phaser);
        for (k, v) in [
            ("stages", 8.0),
            ("mix", 0.5),
            ("lo", 100.0),
            ("hi", 5_000.0),
            ("fb", 0.4),
            ("sync", 1.0),
            ("div", 8.0),
            ("shape", 5.0),
            ("edge", 0.6),
            ("spread", 180.0),
            ("out", -3.0),
        ] {
            spec.set(k, v);
        }
        let s = settings(&spec);
        assert_eq!((s.stages, s.mix, s.lo_hz, s.hi_hz, s.feedback), (8, 0.5, 100.0, 5_000.0, 0.4));
        assert_eq!(s.lfo.rate, Rate::Beats(1.0));
        assert_eq!(s.lfo.shape, Shape::Random);
        assert_eq!((s.lfo.edge, s.spread, s.out_db), (0.6, 0.5, -3.0));
    }
}

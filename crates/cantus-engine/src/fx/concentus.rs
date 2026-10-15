//! Concentus, the chorus (Juno, Dimension), behind the engine's plug-in contract
//! (ADR 0055). Owned by Concentus.
//!
//! [`settings`] reads the table (`cantus_core::fx::concentus`) into the DSP's settings
//! (`cantus_dsp::concentus`); the editor draws from the same reading. The telemetry is the
//! family's header and the mono check (`cantus_dsp::concentus::{T_CORR, T_MONO_DB,
//! T_DELAY_MS}`).

use super::modfx::{lfo_settings, telemetry_of, transport_of};
use super::{FxUnit, SongPosition, Telemetry};
use cantus_core::EffectSpec;
use cantus_dsp::concentus::{Concentus, ConcentusSettings, Mode};

/// Concentus's parameters as the DSP wants them; also what its editor draws from.
pub fn settings(spec: &EffectSpec) -> ConcentusSettings {
    ConcentusSettings {
        mode: Mode::from_index(spec.get("mode")),
        mix: spec.get("mix"),
        voices: spec.get("voices").round().clamp(1.0, 4.0) as u32,
        lfo: lfo_settings(spec, 0),
        delay_ms: spec.get("delay"),
        depth_ms: spec.get("depth"),
        spread: spec.get("spread"),
        tone_hz: spec.get("tone"),
        out_db: spec.get("out"),
    }
}

#[derive(Debug)]
pub struct Unit {
    dsp: Concentus,
}

impl Unit {
    pub fn new(sample_rate: f32) -> Self {
        Unit {
            dsp: Concentus::new(sample_rate),
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
        assert_eq!(
            settings(&EffectSpec::new(EffectKind::Chorus)),
            ConcentusSettings::default()
        );
    }

    #[test]
    fn the_settings_read_every_parameter() {
        let mut spec = EffectSpec::new(EffectKind::Chorus);
        for (k, v) in [
            ("mode", 7.0),
            ("mix", 0.4),
            ("voices", 2.0),
            ("sync", 1.0),
            ("div", 8.0),
            ("shape", 0.0),
            ("delay", 12.0),
            ("depth", 3.0),
            ("spread", 0.5),
            ("tone", 5_000.0),
            ("out", -2.0),
        ] {
            spec.set(k, v);
        }
        let s = settings(&spec);
        assert_eq!(s.mode, Mode::Free);
        assert_eq!(s.mix, 0.4);
        assert_eq!(s.voices, 2);
        assert_eq!(s.lfo.rate, Rate::Beats(1.0));
        assert_eq!(s.lfo.shape, Shape::Sine);
        assert_eq!((s.delay_ms, s.depth_ms, s.spread), (12.0, 3.0, 0.5));
        assert_eq!((s.tone_hz, s.out_db), (5_000.0, -2.0));
    }

    #[test]
    fn the_defaults_touch_no_sample() {
        let mut u = Unit::new(48_000.0);
        u.configure(&EffectSpec::new(EffectKind::Chorus));
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

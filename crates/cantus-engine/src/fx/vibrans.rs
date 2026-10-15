//! Vibrans, the vibrato, behind the engine's plug-in contract (ADR 0055). Owned by Vibrans.
//!
//! [`settings`] reads the table (`cantus_core::fx::vibrans`) into the DSP's settings
//! (`cantus_dsp::vibrans`); the editor draws from the same reading. The telemetry is the
//! family's header (`cantus_dsp::modfx_clock::tel`) and the rise and sweep
//! (`cantus_dsp::vibrans::{T_RISE, T_SWEEP_MS}`).

use super::modfx::{lfo_settings, spread_cycles, telemetry_of, transport_of};
use super::{FxUnit, SongPosition, Telemetry};
use cantus_core::EffectSpec;
use cantus_dsp::vibrans::{Vibrans, VibransSettings};

/// Vibrans's parameters as the DSP wants them; also what its editor draws from.
pub fn settings(spec: &EffectSpec) -> VibransSettings {
    VibransSettings {
        depth_cents: spec.get("depth"),
        lfo: lfo_settings(spec, 0),
        rise_ms: spec.get("rise"),
        spread: spread_cycles(spec.get("spread")),
        mix: spec.get("mix"),
        out_db: spec.get("out"),
    }
}

#[derive(Debug)]
pub struct Unit {
    dsp: Vibrans,
}

impl Unit {
    pub fn new(sample_rate: f32) -> Self {
        Unit {
            dsp: Vibrans::new(sample_rate),
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
        let s = settings(&EffectSpec::new(EffectKind::Vibrato));
        let d = VibransSettings::default();
        assert_eq!(s.depth_cents, d.depth_cents);
        assert_eq!(s.lfo.rate, d.lfo.rate);
        assert_eq!(s.mix, d.mix);
        assert_eq!(s.out_db, d.out_db);
    }

    #[test]
    fn the_settings_read_every_parameter() {
        let mut spec = EffectSpec::new(EffectKind::Vibrato);
        for (k, v) in [
            ("depth", 35.0),
            ("sync", 1.0),
            ("div", 11.0),
            ("shape", 1.0),
            ("rise", 400.0),
            ("spread", 45.0),
            ("mix", 0.5),
            ("out", 2.0),
        ] {
            spec.set(k, v);
        }
        let s = settings(&spec);
        assert_eq!(s.depth_cents, 35.0);
        assert_eq!(s.lfo.rate, Rate::Beats(0.5));
        assert_eq!(s.lfo.shape, Shape::Triangle);
        assert_eq!(s.rise_ms, 400.0);
        assert_eq!(s.spread, 0.125);
        assert_eq!(s.mix, 0.5);
        assert_eq!(s.out_db, 2.0);
    }

    #[test]
    fn the_defaults_touch_no_sample() {
        let mut u = Unit::new(48_000.0);
        u.configure(&EffectSpec::new(EffectKind::Vibrato));
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

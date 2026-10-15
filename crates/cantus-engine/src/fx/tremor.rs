//! Tremor, tremolo and auto-pan, behind the engine's plug-in contract (ADR 0055).
//! Owned by Tremor.
//!
//! [`settings`] reads the table (`cantus_core::fx::tremor`) into the DSP's settings
//! (`cantus_dsp::tremor`); the editor draws from the same reading. The telemetry is the
//! family's header (`cantus_dsp::modfx_clock::tel`).

use super::modfx::{lfo_settings, spread_cycles, telemetry_of, transport_of};
use super::{FxUnit, SongPosition, Telemetry};
use cantus_core::EffectSpec;
use cantus_dsp::tremor::{Mode, Tremor, TremorSettings};

/// The seed of Tremor's random wave.
const SEED: u64 = 0x7e30_0001;

/// Tremor's parameters as the DSP wants them; also what its editor draws from.
pub fn settings(spec: &EffectSpec) -> TremorSettings {
    TremorSettings {
        mode: Mode::from_index(spec.get("mode")),
        depth: spec.get("depth"),
        lfo: lfo_settings(spec, SEED),
        spread: spread_cycles(spec.get("spread")),
        xover_hz: spec.get("xover"),
        out_db: spec.get("out"),
    }
}

#[derive(Debug)]
pub struct Unit {
    dsp: Tremor,
}

impl Unit {
    pub fn new(sample_rate: f32) -> Self {
        Unit {
            dsp: Tremor::new(sample_rate),
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

    /// The defaults of the table read as the DSP's defaults where the null lives: depth 0,
    /// 0 dB out.
    #[test]
    fn the_tables_defaults_are_a_null() {
        let s = settings(&EffectSpec::new(EffectKind::Tremolo));
        assert_eq!(s.depth, 0.0);
        assert_eq!(s.out_db, 0.0);
        assert_eq!(s.mode, Mode::Tremolo);
        assert_eq!(s.lfo.rate, Rate::Beats(0.5));
    }

    #[test]
    fn the_settings_read_every_parameter() {
        let mut spec = EffectSpec::new(EffectKind::Tremolo);
        for (k, v) in [
            ("mode", 2.0),
            ("depth", 0.7),
            ("sync", 0.0),
            ("rate", 6.5),
            ("shape", 4.0),
            ("edge", 0.1),
            ("swing", 0.4),
            ("spread", 90.0),
            ("offset", 180.0),
            ("xover", 1_200.0),
            ("out", -3.0),
        ] {
            spec.set(k, v);
        }
        let s = settings(&spec);
        assert_eq!(s.mode, Mode::Harmonic);
        assert_eq!(s.depth, 0.7);
        assert_eq!(s.lfo.rate, Rate::Hz(6.5));
        assert_eq!(s.lfo.shape, Shape::SawDown);
        assert_eq!(s.lfo.edge, 0.1);
        assert_eq!(s.lfo.swing, 0.4);
        assert_eq!(s.spread, 0.25);
        assert_eq!(s.lfo.offset, 0.5);
        assert_eq!(s.xover_hz, 1_200.0);
        assert_eq!(s.out_db, -3.0);
    }

    /// At its defaults the unit touches no sample, to the bit, and is at rest.
    #[test]
    fn the_defaults_touch_no_sample() {
        let mut u = Unit::new(48_000.0);
        u.configure(&EffectSpec::new(EffectKind::Tremolo));
        let x: Vec<f32> = (0..64).map(|i| (i as f32 * 0.37).fract() - 0.5).collect();
        let (mut l, mut r) = (x.clone(), x.clone());
        u.process(&mut l, &mut r, None);
        assert!(l
            .iter()
            .chain(&r)
            .zip(x.iter().chain(&x))
            .all(|(a, b)| a.to_bits() == b.to_bits()));
        assert!(u.at_rest());
    }
}

//! Anulus, the ring modulator, behind the engine's plug-in contract (ADR 0055). Owned by
//! Anulus.
//!
//! [`settings`] reads the table (`cantus_core::fx::anulus`) into the DSP's settings
//! (`cantus_dsp::anulus`); the editor draws from the same reading. The telemetry is the
//! family's header and the carrier (`cantus_dsp::anulus::{T_FREQ, T_CARRIER}`).

use super::modfx::{lfo_settings, spread_cycles, telemetry_of, transport_of};
use super::{FxUnit, SongPosition, Telemetry};
use cantus_core::EffectSpec;
use cantus_dsp::anulus::{Anulus, AnulusSettings, Wave};

/// The seed of Anulus's random LFO.
const SEED: u64 = 0xa2_0001;

/// Anulus's parameters as the DSP wants them; also what its editor draws from.
pub fn settings(spec: &EffectSpec) -> AnulusSettings {
    AnulusSettings {
        freq_hz: spec.get("freq"),
        wave: Wave::from_index(spec.get("wave")),
        lfo_st: spec.get("lfo"),
        lfo: lfo_settings(spec, SEED),
        spread: spread_cycles(spec.get("spread")),
        mix: spec.get("mix"),
        out_db: spec.get("out"),
    }
}

#[derive(Debug)]
pub struct Unit {
    dsp: Anulus,
}

impl Unit {
    pub fn new(sample_rate: f32) -> Self {
        Unit {
            dsp: Anulus::new(sample_rate),
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
    use cantus_dsp::modfx_lfo::Rate;

    #[test]
    fn the_tables_defaults_are_the_dsps_defaults() {
        let s = settings(&EffectSpec::new(EffectKind::RingMod));
        let d = AnulusSettings::default();
        assert_eq!((s.freq_hz, s.wave, s.lfo_st, s.mix), (d.freq_hz, d.wave, d.lfo_st, d.mix));
        assert_eq!((s.spread, s.out_db), (d.spread, d.out_db));
        assert_eq!(s.lfo.rate, d.lfo.rate);
    }

    #[test]
    fn the_settings_read_every_parameter() {
        let mut spec = EffectSpec::new(EffectKind::RingMod);
        for (k, v) in [
            ("freq", 120.0),
            ("wave", 2.0),
            ("lfo", 12.0),
            ("sync", 1.0),
            ("div", 11.0),
            ("spread", 90.0),
            ("mix", 1.0),
            ("out", -6.0),
        ] {
            spec.set(k, v);
        }
        let s = settings(&spec);
        assert_eq!((s.freq_hz, s.wave, s.lfo_st), (120.0, Wave::Square, 12.0));
        assert_eq!(s.lfo.rate, Rate::Beats(0.5));
        assert_eq!((s.spread, s.mix, s.out_db), (0.25, 1.0, -6.0));
    }
}

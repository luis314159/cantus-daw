//! Translatio, the frequency shifter, behind the engine's plug-in contract (ADR 0055).
//! Owned by Translatio.
//!
//! [`settings`] reads the table (`cantus_core::fx::translatio`) into the DSP's settings
//! (`cantus_dsp::translatio`); the editor draws from the same reading. The unit designs its
//! Hilbert pair for the sample rate when it is built (off the audio thread). The telemetry
//! is the family's header, the shift now and the design's quality
//! (`cantus_dsp::translatio::{T_SHIFT, T_SECTIONS, T_REJECTION}`).

use super::modfx::{lfo_settings, telemetry_of, transport_of};
use super::{FxUnit, SongPosition, Telemetry};
use cantus_core::EffectSpec;
use cantus_dsp::translatio::{Side, Translatio, TranslatioSettings};

/// The seed of Translatio's random LFO.
const SEED: u64 = 0x7a_0001;

/// Translatio's parameters as the DSP wants them; also what its editor draws from.
pub fn settings(spec: &EffectSpec) -> TranslatioSettings {
    TranslatioSettings {
        shift_hz: spec.get("shift"),
        side: Side::from_index(spec.get("side")),
        feedback: spec.get("fb"),
        lfo_hz: spec.get("lfo"),
        lfo: lfo_settings(spec, SEED),
        mix: spec.get("mix"),
        out_db: spec.get("out"),
    }
}

#[derive(Debug)]
pub struct Unit {
    dsp: Translatio,
}

impl Unit {
    pub fn new(sample_rate: f32) -> Self {
        Unit {
            dsp: Translatio::new(sample_rate),
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
        assert_eq!(settings(&EffectSpec::new(EffectKind::FreqShift)), {
            let mut d = TranslatioSettings::default();
            d.lfo.seed = SEED;
            d
        });
    }

    #[test]
    fn the_settings_read_every_parameter() {
        let mut spec = EffectSpec::new(EffectKind::FreqShift);
        for (k, v) in [
            ("shift", -120.0),
            ("side", 2.0),
            ("fb", 0.5),
            ("lfo", 40.0),
            ("mix", 1.0),
            ("out", -2.0),
        ] {
            spec.set(k, v);
        }
        let s = settings(&spec);
        assert_eq!(
            (s.shift_hz, s.side, s.feedback, s.lfo_hz),
            (-120.0, Side::Both, 0.5, 40.0)
        );
        assert_eq!((s.mix, s.out_db), (1.0, -2.0));
    }
}

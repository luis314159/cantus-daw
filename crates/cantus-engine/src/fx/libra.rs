//! Libra, the utility (ADR 0130), behind the engine's plug-in contract. Owned
//! by Libra.
//!
//! [`settings`] reads the table (`cantus_core::fx::libra`) into the DSP's settings
//! (`cantus_dsp::libra`); the balance is ADR 0012's (`cantus_core::balance_gains`, the
//! master's), handed to the DSP as a function. The tuner's reference and the delays' unit
//! are the editor's: nothing here reads them.

use super::modfx::telemetry_of;
use super::{FxUnit, Telemetry};
use cantus_core::EffectSpec;
use cantus_dsp::libra::{Chan, Libra, LibraSettings};

/// Libra's parameters as the DSP wants them; also what its editor draws from.
pub fn settings(spec: &EffectSpec) -> LibraSettings {
    let on = |k: &str| spec.get(k) >= 0.5;
    LibraSettings {
        gain_db: spec.get("out"),
        pan: spec.get("pan"),
        width: spec.get("width"),
        chan: Chan::from_index(spec.get("chan")),
        invert_l: on("pol.l"),
        invert_r: on("pol.r"),
        dc: on("dc"),
        delay_l_ms: spec.get("delay.l"),
        delay_r_ms: spec.get("delay.r"),
        mute: on("mute"),
    }
}

/// ADR 0012's balance.
fn balance(pan: f32) -> (f32, f32) {
    cantus_core::balance_gains(pan)
}

#[derive(Clone, Debug)]
pub struct Unit {
    dsp: Libra,
}

impl Unit {
    pub fn new(sample_rate: f32) -> Self {
        Unit {
            dsp: Libra::new(sample_rate, balance),
        }
    }
}

impl FxUnit for Unit {
    fn fork(&self) -> Option<Box<dyn FxUnit>> {
        Some(Box::new(self.clone()))
    }

    fn configure(&mut self, spec: &EffectSpec) {
        self.dsp.set(&settings(spec));
    }

    fn process(&mut self, left: &mut [f32], right: &mut [f32], _key: Option<(&[f32], &[f32])>) {
        self.dsp.process(left, right);
    }

    /// fx/final — at a start, a seek or a loop's wrap: every glide at its target.
    fn snap(&mut self) {
        self.dsp.snap();
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use cantus_core::EffectKind;

    #[test]
    fn the_tables_defaults_are_the_dsps_defaults() {
        assert_eq!(
            settings(&EffectSpec::new(EffectKind::Utility)),
            LibraSettings::default()
        );
    }

    #[test]
    fn the_settings_read_every_key() {
        let mut spec = EffectSpec::new(EffectKind::Utility);
        for (k, v) in [
            ("out", -6.0),
            ("pan", 0.5),
            ("width", 1.5),
            ("chan", 6.0),
            ("pol.l", 1.0),
            ("pol.r", 1.0),
            ("dc", 1.0),
            ("delay.l", 2.5),
            ("delay.r", 7.0),
            ("mute", 1.0),
        ] {
            spec.set(k, v);
        }
        let s = settings(&spec);
        assert_eq!((s.gain_db, s.pan, s.width), (-6.0, 0.5, 1.5));
        assert_eq!(s.chan, Chan::Side);
        assert!(s.invert_l && s.invert_r && s.dc && s.mute);
        assert_eq!((s.delay_l_ms, s.delay_r_ms), (2.5, 7.0));
    }

    /// At its defaults the unit touches no sample, to the bit; ADR 0012's balance is unity
    /// at the centre.
    #[test]
    fn the_defaults_touch_no_sample() {
        assert_eq!(balance(0.0), (1.0, 1.0));
        let mut u = Unit::new(48_000.0);
        u.configure(&EffectSpec::new(EffectKind::Utility));
        assert!(u.at_rest(), "a new unit rests");
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

//! Latitudo (stereo control and mid/side, ADR 0026) behind the engine's plug-in contract.
//! Owned by Latitudo.
//!
//! [`settings`] reads the parameters (`cantus_core::fx::stereo`) into the DSP's terms
//! (`cantus_dsp::latitudo`); the editor draws its width and tone curves from the same
//! function, so the picture and the sound come from one reading of the spec.
//!
//! ## Telemetry (64 floats, refreshed after every block)
//!
//! The layout is `cantus_dsp::latitudo::view::telemetry`'s, whose constants name each slot:
//! the vectorscope's points (48 of the output, packed), correlation and balance of the
//! output and of the input, correlation in three bands, the side's level against the
//! mid, the levels and the peaks. No latency at any setting.

use super::{FxUnit, Telemetry, TELEMETRY_LEN};
use cantus_core::EffectSpec;
use cantus_dsp::latitudo::{HaasMode, Latitudo, Listen, StereoSettings, MONO_SLOPES, TONE_SLOPES};

/// The index of a stepped parameter, kept inside `0..n`.
fn index(spec: &EffectSpec, key: &str, n: usize) -> usize {
    (spec.get(key).round().max(0.0) as usize).min(n - 1)
}

/// Latitudo's parameters as the DSP wants them; also what its editor draws from.
pub fn settings(spec: &EffectSpec) -> StereoSettings {
    StereoSettings {
        width: spec.get("width"),
        mid_db: spec.get("mid"),
        side_db: spec.get("side"),
        balance: spec.get("bal"),
        rotation_deg: spec.get("rot"),
        swap: spec.get("swap") >= 0.5,
        invert_l: spec.get("inv_l") >= 0.5,
        invert_r: spec.get("inv_r") >= 0.5,
        listen: Listen::from_index(index(spec, "listen", 3)),
        mono_hz: spec.get("mono"),
        mono_slope: MONO_SLOPES[index(spec, "mono_slope", MONO_SLOPES.len())],
        x_lo: spec.get("x_lo"),
        x_hi: spec.get("x_hi"),
        w_lo: spec.get("w_lo"),
        w_mid: spec.get("w_mid"),
        w_hi: spec.get("w_hi"),
        mid_lo_hz: spec.get("m_lo_f"),
        mid_lo_db: spec.get("m_lo"),
        mid_hi_hz: spec.get("m_hi_f"),
        mid_hi_db: spec.get("m_hi"),
        side_lo_hz: spec.get("s_lo_f"),
        side_lo_db: spec.get("s_lo"),
        side_hi_hz: spec.get("s_hi_f"),
        side_hi_db: spec.get("s_hi"),
        tone_slope: TONE_SLOPES[index(spec, "tone_slope", TONE_SLOPES.len())],
        haas_ms: spec.get("haas"),
        haas_mode: HaasMode::from_index(index(spec, "haas_mode", 3)),
        haas_amount: spec.get("haas_amt"),
        out_db: spec.get("out"),
    }
}

#[derive(Debug)]
pub struct Unit {
    dsp: Latitudo,
}

impl Unit {
    pub fn new(sample_rate: f32) -> Self {
        Unit {
            dsp: Latitudo::new(sample_rate),
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

    fn latency(&self) -> usize {
        self.dsp.latency()
    }

    fn telemetry(&self) -> Telemetry {
        let mut t = [0.0; TELEMETRY_LEN];
        self.dsp.write_telemetry(&mut t);
        t
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cantus_core::EffectKind;

    /// The defaults of the table read as the DSP's defaults: the null rule holds through
    /// the mapping, not only inside the DSP.
    #[test]
    fn the_tables_defaults_are_the_dsps_defaults() {
        let spec = EffectSpec::new(EffectKind::Stereo);
        assert_eq!(settings(&spec), StereoSettings::default());
    }

    /// Every key of the table is read (a key the mapping forgot would be a dead knob).
    #[test]
    fn every_key_moves_the_settings() {
        let base = settings(&EffectSpec::new(EffectKind::Stereo));
        for info in EffectKind::Stereo.params() {
            let mut spec = EffectSpec::new(EffectKind::Stereo);
            let other = if info.default == info.max {
                info.min
            } else {
                info.max
            };
            spec.set(info.key, other).unwrap();
            assert_ne!(settings(&spec), base, "{} is not read", info.key);
        }
    }
}

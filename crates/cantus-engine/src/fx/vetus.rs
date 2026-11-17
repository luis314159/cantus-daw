//! Vetus, lo-fi (ADR 0133, provisional), behind the engine's plug-in contract. Owned by
//! Vetus.
//!
//! [`settings`] reads the table (`cantus_core::fx::vetus`) into the DSP's settings
//! (`cantus_dsp::vetus`); the editor draws from the same reading (and from
//! [`VetusSettings::aged`], what Character leaves). The telemetry is the modulation family's
//! header and Vetus's own (`cantus_dsp::vetus::t`).

use super::modfx::{telemetry_of, transport_of};
use super::{FxUnit, SongPosition, Telemetry};
use cantus_core::EffectSpec;
use cantus_dsp::modfx_clock::division_beats;
use cantus_dsp::modfx_lfo::Rate;
use cantus_dsp::vetus::age::AgeParams;
use cantus_dsp::vetus::digital::{BitsMode, RateFilter};
use cantus_dsp::vetus::noise::NoiseParams;
use cantus_dsp::vetus::{Medium, Vetus, VetusSettings};

/// Vetus's parameters as the DSP wants them; also what its editor draws from.
pub fn settings(spec: &EffectSpec) -> VetusSettings {
    let on = |k: &str| spec.get(k) >= 0.5;
    VetusSettings {
        mix: spec.get("mix"),
        out_db: spec.get("out"),
        character: spec.get("char"),
        bits_on: on("bits.on"),
        bits: spec.get("bits.depth"),
        dither: on("bits.dither"),
        bits_mode: BitsMode::from_index(spec.get("bits.mode")),
        rate_on: on("rate.on"),
        rate_hz: spec.get("rate.hz"),
        rate_filter: RateFilter::from_index(spec.get("rate.filter")),
        wob_on: on("wob.on"),
        wow_ct: spec.get("wow"),
        wow_rate: if on("wow.sync") {
            Rate::Beats(division_beats(spec.get("wow.div")))
        } else {
            Rate::Hz(f64::from(spec.get("wow.rate")))
        },
        drift: spec.get("wow.drift"),
        flut_ct: spec.get("flut"),
        flut_hz: spec.get("flut.rate"),
        age_on: on("age.on"),
        age: AgeParams {
            drop: spec.get("age.drop"),
            wear: spec.get("age.wear"),
        },
        noise_on: on("noise.on"),
        noise: NoiseParams {
            crackle: spec.get("crackle"),
            hiss: spec.get("hiss"),
            vinyl: on("hiss.kind"),
            hum: spec.get("hum"),
            hum_60: on("hum.hz"),
        },
        follow: spec.get("noise.follow"),
        tone_on: on("tone.on"),
        medium: Medium::from_index(spec.get("tone.medium")),
        lo: spec.get("tone.lo"),
        hi: spec.get("tone.hi"),
        res: spec.get("tone.res"),
    }
}

#[derive(Clone, Debug)]
pub struct Unit {
    dsp: Vetus,
}

impl Unit {
    pub fn new(sample_rate: f32) -> Self {
        Unit {
            dsp: Vetus::new(sample_rate),
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

    /// The defaults of the table read as the DSP's defaults: the null rule holds through
    /// the mapping.
    #[test]
    fn the_tables_defaults_are_the_dsps_defaults() {
        let spec = EffectSpec::new(EffectKind::LoFi);
        assert_eq!(settings(&spec), VetusSettings::default());
    }

    #[test]
    fn the_settings_read_every_key() {
        let mut spec = EffectSpec::new(EffectKind::LoFi);
        for (k, v) in [
            ("mix", 0.7),
            ("char", 0.4),
            ("bits.on", 1.0),
            ("bits.depth", 5.5),
            ("bits.dither", 1.0),
            ("bits.mode", 2.0),
            ("rate.on", 1.0),
            ("rate.hz", 6_000.0),
            ("rate.filter", 2.0),
            ("wob.on", 1.0),
            ("wow", 30.0),
            ("wow.sync", 1.0),
            ("wow.div", 5.0),
            ("wow.drift", 0.2),
            ("flut", 9.0),
            ("flut.rate", 14.0),
            ("age.on", 1.0),
            ("age.drop", 0.6),
            ("age.wear", 0.1),
            ("noise.on", 1.0),
            ("crackle", 0.9),
            ("hiss", 0.4),
            ("hiss.kind", 1.0),
            ("hum", 0.2),
            ("hum.hz", 1.0),
            ("noise.follow", 0.5),
            ("tone.on", 1.0),
            ("tone.medium", 3.0),
            ("tone.lo", 200.0),
            ("tone.hi", 4_000.0),
            ("tone.res", 0.6),
        ] {
            spec.set(k, v);
        }
        let s = settings(&spec);
        assert!(s.switches().iter().all(|on| *on));
        assert_eq!(
            (s.mix, s.character, s.bits, s.rate_hz),
            (0.7, 0.4, 5.5, 6_000.0)
        );
        assert!(s.dither && s.bits_mode == BitsMode::MuLaw && s.rate_filter == RateFilter::Both);
        assert_eq!(s.wow_rate, Rate::Beats(2.0));
        assert_eq!(
            (s.wow_ct, s.drift, s.flut_ct, s.flut_hz),
            (30.0, 0.2, 9.0, 14.0)
        );
        assert_eq!((s.age.drop, s.age.wear), (0.6, 0.1));
        assert!(s.noise.vinyl && s.noise.hum_60 && s.noise.crackle == 0.9);
        assert_eq!((s.noise.hiss, s.noise.hum, s.follow), (0.4, 0.2, 0.5));
        assert_eq!(s.medium, Medium::Gramophone);
        assert_eq!((s.lo, s.hi, s.res), (200.0, 4_000.0, 0.6));
    }

    /// At its defaults the unit touches no sample, to the bit.
    #[test]
    fn the_defaults_touch_no_sample() {
        let mut u = Unit::new(48_000.0);
        u.configure(&EffectSpec::new(EffectKind::LoFi));
        let x: Vec<f32> = (0..64).map(|i| (i as f32 * 0.37).fract() - 0.5).collect();
        let (mut l, mut r) = (x.clone(), x.clone());
        u.process(&mut l, &mut r, None);
        assert!(l
            .iter()
            .chain(&r)
            .zip(x.iter().chain(&x))
            .all(|(a, b)| a.to_bits() == b.to_bits()));
        assert!(u.at_rest());
        assert_eq!(u.latency(), 0);
    }
}

//! Cribrum, the auto filter (ADR 0130, provisional), behind the engine's plug-in contract.
//! Owned by Cribrum.
//!
//! [`settings`] reads the table (`cantus_core::fx::cribrum`: the globals and the curve's
//! sixteen points) into the DSP's settings (`cantus_dsp::cribrum`); the editor draws from
//! the same reading. The curve's segments take the automation editor's shapes
//! (`fluctus::shape_at`), so a curve drawn here, in Fluctus and in an automation clip bends
//! the same way. The telemetry is the modulation family's header and Cribrum's own
//! (`cantus_dsp::cribrum::t`).

use super::fluctus::shape_at;
use super::modfx::{telemetry_of, transport_of};
use super::{FxUnit, SongPosition, Telemetry};
use cantus_core::fx::cribrum::point_keys;
use cantus_core::EffectSpec;
use cantus_dsp::cribrum::filter::{model_of, Mode};
use cantus_dsp::cribrum::modulation::{division_beats, Wave};
use cantus_dsp::cribrum::Cribrum;
use cantus_dsp::cribrum::{
    CribrumSettings, CurveParams, Depth, EnvParams, FilterParams, LfoParams, OS_FACTORS,
};
use cantus_dsp::fluctus::{CurvePoint, POINTS};

/// One filter's settings under its prefix (`a` or `b`).
fn filter(spec: &EffectSpec, keys: &[&str; 7]) -> FilterParams {
    FilterParams {
        model: model_of(spec.get(keys[0])),
        mode: Mode::from_index(spec.get(keys[1])),
        slope24: spec.get(keys[2]) >= 0.5,
        cut: spec.get(keys[3]),
        res: spec.get(keys[4]),
        drive_db: spec.get(keys[5]),
        vowel: spec.get(keys[6]),
    }
}

fn depth(spec: &EffectSpec, keys: &[&str; 3]) -> Depth {
    Depth {
        cut: spec.get(keys[0]),
        res: spec.get(keys[1]),
        vowel: spec.get(keys[2]),
    }
}

/// Cribrum's parameters as the DSP wants them; also what its editor draws from.
pub fn settings(spec: &EffectSpec) -> CribrumSettings {
    let keys = point_keys();
    let mut points = [None; POINTS];
    for (n, k) in keys.iter().enumerate() {
        if spec.get(k[0]) >= 0.5 {
            points[n] = Some(CurvePoint {
                x: spec.get(k[1]),
                y: spec.get(k[2]),
                tension: spec.get(k[3]),
                shape: spec.get(k[4]).round().clamp(0.0, 5.0) as u8,
            });
        }
    }
    let os = spec
        .get("os")
        .round()
        .clamp(0.0, (OS_FACTORS.len() - 1) as f32) as usize;
    CribrumSettings {
        mix: spec.get("mix"),
        out_db: spec.get("out"),
        parallel: spec.get("route") >= 0.5,
        spread: spec.get("spread"),
        os: OS_FACTORS[os],
        a: filter(
            spec,
            &[
                "a.model", "a.mode", "a.slope", "a.cut", "a.res", "a.drive", "a.vowel",
            ],
        ),
        b: filter(
            spec,
            &[
                "b.model", "b.mode", "b.slope", "b.cut", "b.res", "b.drive", "b.vowel",
            ],
        ),
        b_on: spec.get("b.on") >= 0.5,
        b_track: spec.get("b.track"),
        lfo: LfoParams {
            synced: spec.get("lfo.sync") >= 0.5,
            beats: division_beats(spec.get("lfo.div")),
            rate_hz: spec.get("lfo.rate"),
            wave: Wave::from_index(spec.get("lfo.shape")),
            edge: spec.get("lfo.edge"),
            phase: spec.get("lfo.phase"),
            stereo: spec.get("lfo.stereo"),
        },
        lfo_depth: depth(spec, &["lfo.cut", "lfo.res", "lfo.vowel"]),
        env: EnvParams {
            key: spec.get("env.key") >= 0.5,
            att_ms: spec.get("env.att"),
            rel_ms: spec.get("env.rel"),
            gain_db: spec.get("env.gain"),
            range_db: spec.get("env.range"),
        },
        env_depth: depth(spec, &["env.cut", "env.res", "env.vowel"]),
        curve: CurveParams {
            beats: division_beats(spec.get("cur.div")),
            smooth_ms: spec.get("cur.smooth"),
            points,
        },
        curve_depth: depth(spec, &["cur.cut", "cur.res", "cur.vowel"]),
    }
}

#[derive(Clone, Debug)]
pub struct Unit {
    dsp: Cribrum,
}

impl Unit {
    pub fn new(sample_rate: f32) -> Self {
        Unit {
            dsp: Cribrum::new(sample_rate, shape_at),
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

    fn process(&mut self, left: &mut [f32], right: &mut [f32], key: Option<(&[f32], &[f32])>) {
        self.dsp.process(left, right, key);
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
        let spec = EffectSpec::new(EffectKind::AutoFilter);
        assert_eq!(settings(&spec), CribrumSettings::default());
    }

    #[test]
    fn the_settings_read_every_key() {
        let mut spec = EffectSpec::new(EffectKind::AutoFilter);
        for (k, v) in [
            ("mix", 0.7),
            ("route", 1.0),
            ("os", 2.0),
            ("a.model", 3.0),
            ("a.mode", 2.0),
            ("b.on", 1.0),
            ("b.slope", 1.0),
            ("b.cut", 300.0),
            ("b.track", -0.5),
            ("lfo.sync", 0.0),
            ("lfo.div", 18.0),
            ("lfo.shape", 6.0),
            ("env.key", 1.0),
            ("env.cut", 2.0),
            ("cur.div", 0.0),
            ("cur.vowel", -1.0),
            ("c1.on", 0.0),
            ("c7.on", 1.0),
            ("c7.x", 0.6),
            ("c7.s", 3.0),
        ] {
            spec.set(k, v);
        }
        let s = settings(&spec);
        assert_eq!((s.mix, s.parallel, s.os), (0.7, true, 4));
        assert_eq!(s.a.model, cantus_dsp::va::filter::Model::Diode);
        assert_eq!(s.a.mode, Mode::High);
        assert!(s.b_on && s.b.slope24 && s.b.cut == 300.0 && s.b_track == -0.5);
        assert!(!s.lfo.synced && s.lfo.beats == 0.0625 && s.lfo.wave == Wave::Drift);
        assert!(s.env.key && s.env_depth.cut == 2.0);
        assert_eq!((s.curve.beats, s.curve_depth.vowel), (32.0, -1.0));
        assert!(s.curve.points[1].is_none());
        assert_eq!(s.curve.points[7].map(|p| (p.x, p.shape)), Some((0.6, 3)));
    }

    /// At its defaults the unit touches no sample, to the bit.
    #[test]
    fn the_defaults_touch_no_sample() {
        let mut u = Unit::new(48_000.0);
        u.configure(&EffectSpec::new(EffectKind::AutoFilter));
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

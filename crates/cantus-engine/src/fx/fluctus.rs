//! Fluctus, the volume shaper (LFO Tool / Kickstart), behind the engine's plug-in contract
//! (ADR 0055). Owned by Fluctus.
//!
//! [`settings`] reads the table (`cantus_core::fx::fluctus`, the globals and the sixteen
//! points) into the DSP's settings (`cantus_dsp::fluctus`); the editor draws from the same
//! reading. Each segment takes the automation editor's shape ([`shape_at`]:
//! `cantus_core::automation::Shape::at`), so a curve drawn here and an automation clip bend
//! the same way. The telemetry is the family's header and the level
//! (`cantus_dsp::fluctus::T_LEVEL`).

use super::modfx::{telemetry_of, transport_of};
use super::{FxUnit, SongPosition, Telemetry};
use cantus_core::automation::Shape;
use cantus_core::fx::fluctus::point_keys;
use cantus_core::EffectSpec;
use cantus_dsp::fluctus::{CurvePoint, Fluctus, FluctusSettings, POINTS};
use cantus_dsp::modfx_clock::division_beats;

/// A segment's progress as the automation editor shapes it.
pub fn shape_at(shape: u8, tension: f32, u: f32) -> f32 {
    Shape::ALL[(shape as usize).min(Shape::ALL.len() - 1)].at(tension, u)
}

/// Fluctus's parameters as the DSP wants them; also what its editor draws from.
pub fn settings(spec: &EffectSpec) -> FluctusSettings {
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
    FluctusSettings {
        depth: spec.get("depth"),
        beats: division_beats(spec.get("div")),
        offset: spec.get("offset"),
        smooth_ms: spec.get("smooth"),
        out_db: spec.get("out"),
        points,
    }
}

#[derive(Debug)]
pub struct Unit {
    dsp: Fluctus,
}

impl Unit {
    pub fn new(sample_rate: f32) -> Self {
        Unit {
            dsp: Fluctus::new(sample_rate, shape_at),
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

    #[test]
    fn the_tables_defaults_are_the_dsps_defaults() {
        assert_eq!(
            settings(&EffectSpec::new(EffectKind::VolumeShaper)),
            FluctusSettings::default()
        );
    }

    #[test]
    fn the_settings_read_the_points_and_the_globals() {
        let mut spec = EffectSpec::new(EffectKind::VolumeShaper);
        for (k, v) in [
            ("depth", 0.7),
            ("div", 14.0),
            ("offset", 0.25),
            ("smooth", 5.0),
            ("out", -1.0),
            ("p1.on", 0.0),
            ("p7.on", 1.0),
            ("p7.x", 0.6),
            ("p7.y", 0.3),
            ("p7.t", 0.5),
            ("p7.s", 3.0),
        ] {
            spec.set(k, v);
        }
        let s = settings(&spec);
        assert_eq!((s.depth, s.beats, s.offset, s.smooth_ms, s.out_db), (0.7, 0.25, 0.25, 5.0, -1.0));
        assert!(s.points[1].is_none());
        assert_eq!(
            s.points[7],
            Some(CurvePoint {
                x: 0.6,
                y: 0.3,
                tension: 0.5,
                shape: 3
            })
        );
    }

    #[test]
    fn a_segment_bends_as_an_automation_clip_does() {
        for (i, shape) in Shape::ALL.iter().enumerate() {
            for u in [0.0f32, 0.3, 0.7, 1.0] {
                assert_eq!(shape_at(i as u8, 0.4, u), shape.at(0.4, u));
            }
        }
    }
}

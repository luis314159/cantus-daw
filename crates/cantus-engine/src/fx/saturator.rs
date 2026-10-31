//! Fervor (the saturator) behind the engine's plug-in contract. Owned by Fervor.
//!
//! [`settings`] reads the parameters (`cantus_core::fx::saturator`) into the DSP's terms
//! (`cantus_dsp::fervor`); the editor draws its curve and measures its harmonics from the
//! same function, so the picture and the sound come from one reading of the spec.
//!
//! ## Telemetry (64 floats, refreshed after every block)
//!
//! The layout is `cantus_dsp::fervor::telemetry`'s; the constants there name each slot.
//!
//! | Slot | What |
//! |---|---|
//! | 0 | blocks processed, modulo 2²⁴ |
//! | 1, 2 | the block's input and output peaks, dBFS |
//! | 3, 4 | the block's input and output RMS, dBFS |
//! | 5 | the level match's gain, dB (0 when off) |
//! | 6 | latency, samples |
//! | 7 | the sample rate, Hz |
//! | 8 | the oversampling factor running |
//! | 9 | 1 while the wet path runs, 0 while nothing would colour the signal |
//! | 10 | the drive the curve sees now (gliding, ×100 included), dB |
//! | 11 | the block's input peak, linear (the ball on the editor's curve) |

use super::{FxUnit, Telemetry, TELEMETRY_LEN};
use cantus_core::fx::saturator::{point_keys, POINTS};
use cantus_core::EffectSpec;
use cantus_dsp::fervor::{os, Channels, CurveSpec, Fervor, Model, Point, SatSettings};

/// The drawn curve as the DSP wants it.
pub fn curve(spec: &EffectSpec) -> CurveSpec {
    let keys = point_keys();
    let mut c = CurveSpec {
        symmetric: spec.get("c.sym") >= 0.5,
        lo: spec.get("c.lo"),
        hi: spec.get("c.hi"),
        end_t: spec.get("c.t"),
        points: [None; POINTS],
    };
    for (n, [on, x, y, t]) in keys.iter().enumerate() {
        if spec.get(on) >= 0.5 {
            c.points[n] = Some(Point {
                x: spec.get(x),
                y: spec.get(y),
                t: spec.get(t),
            });
        }
    }
    c
}

/// Fervor's parameters as the DSP wants them; also what its editor draws from.
pub fn settings(spec: &EffectSpec) -> SatSettings {
    let os_index = spec.get("os").round().clamp(0.0, 3.0) as usize;
    SatSettings {
        model: Model::from_index(spec.get("model").round().max(0.0) as usize),
        drive_db: spec.get("drive"),
        boost: spec.get("boost") >= 0.5,
        character: spec.get("char"),
        bias: spec.get("bias"),
        lo_cut_hz: spec.get("lo_cut"),
        hi_cut_hz: spec.get("hi_cut"),
        tone_db: spec.get("tone"),
        oversampling: os::FACTORS[os_index],
        minimum_phase: spec.get("phase") >= 0.5,
        bits: spec.get("bits"),
        rate_hz: spec.get("rate"),
        channels: Channels::from_index(spec.get("chan").round().max(0.0) as usize),
        mix: spec.get("mix"),
        auto_gain: spec.get("auto") >= 0.5,
        out_db: spec.get("out"),
        curve: curve(spec),
    }
}

#[derive(Clone, Debug)] // r34:listen: Clone for fork
pub struct Unit {
    dsp: Fervor,
    /// Read into on the audio thread: setting it never allocates (`SatSettings` holds no
    /// heap data).
    scratch: SatSettings,
}

impl Unit {
    pub fn new(sample_rate: f32) -> Self {
        Unit {
            dsp: Fervor::new(sample_rate),
            scratch: SatSettings::default(),
        }
    }
}

impl FxUnit for Unit {
    fn fork(&self) -> Option<Box<dyn FxUnit>> {
        Some(Box::new(self.clone()))
    } // r34:listen
    fn configure(&mut self, spec: &EffectSpec) {
        self.scratch = settings(spec);
        self.dsp.set(&self.scratch);
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

    fn skip(&mut self, frames: usize) {
        self.dsp.skip(frames);
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

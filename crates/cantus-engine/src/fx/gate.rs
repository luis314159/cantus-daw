//! Porta (the gate) behind the engine's plug-in contract. Owned by the gate.
//!
//! [`settings`] reads the parameters (`cantus_core::fx::gate`) into the DSP's terms
//! (`cantus_dsp::porta`); the editor draws its curves from the same function, so the
//! picture and the sound come from one reading of the spec.
//!
//! ## Telemetry (64 floats, refreshed after every block)
//!
//! The layout is `cantus_dsp::porta::telemetry`'s; the constants there name each slot.
//!
//! | Slot | What |
//! |---|---|
//! | 0 | columns completed since the unit was made, modulo 2²⁴ |
//! | 1 | one column's length, ms (5) |
//! | 2 | the detector's level now, dB |
//! | 3 | the gain now, dB (−120 for silence) |
//! | 4 | the phase now: 0 closed, 1 release, 2 hold, 3 open |
//! | 5 | latency, samples |
//! | 6 | hits measured so far, modulo 2²⁴ |
//! | 7 | the sample rate, Hz |
//! | 8–15 | the last four hits, newest first: samples before the end of the block where the key crossed the threshold (input timeline), and the key's peak in the 10 ms after, dB |
//! | 16–63 | the last 24 columns, oldest first, two packed numbers each (`telemetry::unpack`): input and output peak, key level, gain, phase |
//!
//! The editor keeps its own long history: each frame it takes the columns that arrived
//! since the last one (slot 0 says how many), so a frame that comes late loses nothing
//! as long as it is within 24 columns (120 ms).

use super::{FxUnit, Telemetry, TELEMETRY_LEN};
use cantus_core::EffectSpec;
use cantus_dsp::porta::{GateSettings, Listen, Porta, Style};

/// A gate's parameters as the DSP wants them; also what its editor draws from.
pub fn settings(spec: &EffectSpec) -> GateSettings {
    GateSettings {
        threshold_db: spec.get("thr"),
        hysteresis_db: spec.get("hyst"),
        ratio: spec.get("ratio"),
        range_db: spec.get("range"),
        knee_db: spec.get("knee"),
        attack_ms: spec.get("att"),
        hold_ms: spec.get("hold"),
        release_ms: spec.get("rel"),
        lookahead_ms: spec.get("look"),
        style: Style::from_index(spec.get("style").round().max(0.0) as usize),
        hpf_hz: spec.get("hpf"),
        lpf_hz: spec.get("lpf"),
        key_slope: spec.get("key_slope"),
        listen: Listen::from_index(spec.get("listen").round().max(0.0) as usize),
        midi: spec.get("trig") >= 0.5,
        vel_db: spec.get("vel"),
    }
}

#[derive(Clone, Debug)] // r34:listen: Clone for fork
pub struct Unit {
    dsp: Porta,
}

impl Unit {
    pub fn new(sample_rate: f32) -> Self {
        Unit {
            dsp: Porta::new(sample_rate),
        }
    }
}

impl FxUnit for Unit {
    fn fork(&self) -> Option<Box<dyn FxUnit>> {
        Some(Box::new(self.clone()))
    } // r34:listen
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

    /// The notes of the slot's track (ADR 0022): in MIDI mode (`trig`) they open and
    /// close the gate, at their sample.
    fn note(&mut self, offset: usize, note: super::FxNote) {
        use cantus_dsp::porta::Trigger;
        self.dsp.note(
            offset,
            match note {
                super::FxNote::On { pitch, velocity } => Trigger::On { pitch, velocity },
                super::FxNote::Off { pitch } => Trigger::Off { pitch },
                super::FxNote::AllOff => Trigger::AllOff,
            },
        );
    }
}

//! What the modulation family's units share on the engine side (ADR 0055): the song
//! position as their clock reads it, and the LFO settings their tables spell the same way
//! (`sync`, `div`, `rate`, `shape`, `edge`, `swing`, `offset`).

use super::SongPosition;
use cantus_core::EffectSpec;
use cantus_dsp::modfx_clock::{division_beats, Transport};
use cantus_dsp::modfx_lfo::{LfoSettings, Rate, Shape};

/// The transport as the family's clock reads it.
#[inline]
pub fn transport_of(p: &SongPosition) -> Transport {
    Transport {
        playing: p.playing,
        tempo_bpm: p.tempo_bpm,
        samples: p.samples,
    }
}

/// The LFO a table's family keys describe; a key the kind does not have reads as its
/// table's default, or as nothing (no edge, no swing, no offset).
pub fn lfo_settings(spec: &EffectSpec, seed: u64) -> LfoSettings {
    let has = |k: &str| spec.kind.param(k).is_some();
    let synced = has("sync") && spec.get("sync") >= 0.5;
    LfoSettings {
        rate: if synced {
            Rate::Beats(division_beats(spec.get("div")))
        } else {
            Rate::Hz(f64::from(spec.get("rate")))
        },
        shape: if has("shape") {
            Shape::from_index(spec.get("shape"))
        } else {
            Shape::Sine
        },
        offset: if has("offset") {
            f64::from(spec.get("offset")) / 360.0
        } else {
            0.0
        },
        edge: if has("edge") { spec.get("edge") } else { 0.0 },
        swing: if has("swing") { spec.get("swing") } else { 0.0 },
        seed,
    }
}

/// A stereo spread in degrees as the LFO's cycles (0 … ½).
#[inline]
pub fn spread_cycles(degrees: f32) -> f32 {
    (degrees / 360.0).clamp(0.0, 0.5)
}

/// Copy a DSP's telemetry header into the contract's block.
#[inline]
pub fn telemetry_of(f: impl FnOnce(&mut super::Telemetry)) -> super::Telemetry {
    let mut t = [0.0; super::TELEMETRY_LEN];
    f(&mut t);
    t
}

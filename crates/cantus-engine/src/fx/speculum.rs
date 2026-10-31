//! Speculum, the analyser, behind the engine's plug-in contract (ADR 0056). Owned by
//! Speculum.
//!
//! **A pure pass-through**: the unit touches no sample, holds no state and is always at
//! rest. What Speculum shows is measured on the editor's thread (`cantus_dsp::speculum`)
//! from its slot's own stereo signal, which the engine copies out — for every Speculum
//! being looked at, up to [`MAX_SCOPES`] at once — into a lock-free ring of
//! [`ScopeChunk`]s that carry the song's position with them (`graph/strip_run.rs`,
//! `graph/master_run.rs`, `EngineHandle::watch_scope`). Nothing is dropped while the
//! editor drains its ring once a frame: a ring holds 1.4 s at 48 kHz.

use super::FxUnit;
use cantus_core::fx::speculum::{FLOORS, SCOPE_BEATS};
use cantus_core::{EffectId, EffectSpec};
use cantus_dsp::speculum::{Channel, SpeculumSettings, Sync, FFT_SIZES};

/// How many Speculums can be watched at once (each a ring of its own).
pub const MAX_SCOPES: usize = 8;
/// Frames a chunk carries.
pub const CHUNK: usize = 64;
/// Chunks a ring holds: 65 536 frames (1.4 s at 48 kHz, more than any editor frame waits) —
/// a chunk is filled across passes, however short they are, so the capacity is in frames.
pub const SCOPE_CHUNKS: usize = 1_024;

/// Frames of a watched Speculum's signal, with where the song was at the first of them.
#[derive(Copy, Clone, Debug)]
pub struct ScopeChunk {
    /// The Speculum whose frames these are: a ring handed to another drops what is not its
    /// own (review 1).
    pub owner: EffectId,
    /// Frames lost just before these, the ring having been full (the editor says so).
    pub lost: u32,
    /// Frames used, 1 … [`CHUNK`].
    pub frames: u16,
    /// The song's position at the first frame, quarter notes, while it plays.
    pub beat: Option<f64>,
    pub tempo_bpm: f64,
    pub l: [f32; CHUNK],
    pub r: [f32; CHUNK],
}

impl ScopeChunk {
    pub const EMPTY: ScopeChunk = ScopeChunk {
        owner: EffectId(0),
        lost: 0,
        frames: 0,
        beat: None,
        tempo_bpm: 0.0,
        l: [0.0; CHUNK],
        r: [0.0; CHUNK],
    };
}

/// Speculum's view settings as the measurements want them; also what its editor draws
/// from.
pub fn settings(spec: &EffectSpec) -> SpeculumSettings {
    let pick = |i: f32, n: usize| (i.round().max(0.0) as usize).min(n - 1);
    SpeculumSettings {
        fft_size: FFT_SIZES[pick(spec.get("fft"), FFT_SIZES.len())],
        avg_ms: spec.get("avg"),
        slope_db: spec.get("slope"),
        hold: spec.get("hold") >= 0.5,
        channel: Channel::from_index(spec.get("chan")),
        sync: Sync::from_index(spec.get("sync")),
        scope_beats: SCOPE_BEATS[pick(spec.get("beats"), SCOPE_BEATS.len())],
        scope_ms: spec.get("ms"),
        target_lufs: spec.get("target"),
        floor_db: FLOORS[pick(spec.get("floor"), FLOORS.len())],
    }
}

/// The view a spec shows, 0 … 5.
pub fn view_of(spec: &EffectSpec) -> usize {
    (spec.get("view").round().max(0.0) as usize).min(5)
}

/// The pass-through: nothing to configure, nothing to remember.
#[derive(Clone, Debug, Default)] // r34:listen: Clone for fork
pub struct Unit;

impl Unit {
    pub fn new(sample_rate: f32) -> Self {
        let _ = sample_rate;
        Unit
    }
}

impl FxUnit for Unit {
    fn fork(&self) -> Option<Box<dyn FxUnit>> {
        Some(Box::new(self.clone()))
    } // r34:listen
    fn configure(&mut self, _spec: &EffectSpec) {}

    fn process(&mut self, _left: &mut [f32], _right: &mut [f32], _key: Option<(&[f32], &[f32])>) {}

    fn reset(&mut self) {}

    /// No state at all: always at rest.
    fn at_rest(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cantus_core::EffectKind;

    /// The table's defaults are the measurements' defaults.
    #[test]
    fn the_tables_defaults_are_the_analysis_defaults() {
        let spec = EffectSpec::new(EffectKind::Analyzer);
        assert_eq!(settings(&spec), SpeculumSettings::default());
        assert_eq!(view_of(&spec), 0);
    }

    /// Whatever is set, the unit touches no sample and stays at rest.
    #[test]
    fn it_touches_no_sample_whatever_is_set() {
        let mut spec = EffectSpec::new(EffectKind::Analyzer);
        for info in EffectKind::Analyzer.params() {
            spec.set(info.key, info.max);
        }
        let mut u = Unit::new(48_000.0);
        u.configure(&spec);
        let x: Vec<f32> = (0..64).map(|i| (i as f32 * 0.37).fract() - 0.5).collect();
        let (mut l, mut r) = (x.clone(), x.clone());
        u.process(&mut l, &mut r, None);
        assert!(l
            .iter()
            .chain(&r)
            .zip(x.iter().chain(&x))
            .all(|(a, b)| a.to_bits() == b.to_bits()));
        assert!(u.at_rest());
    }
}

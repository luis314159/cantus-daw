//! The plug-in contract on the engine side: one [`FxUnit`] per plug-in, in a module of its
//! own, so a plug-in is owned by its files and the processor never learns its name.
//!
//! `EffectUnit` (`crate::effects`) holds a `Box<dyn FxUnit>` for every kind made here. The
//! box is built on the UI thread and handed to the audio thread, which only calls these
//! methods; none of them may allocate, lock or block (ADR 0005).

use cantus_core::{EffectKind, EffectSpec};

pub mod deesser;
pub mod gate;
pub mod limiter;
pub mod maximizer;
pub mod multiband;
pub mod saturator;
pub mod space;
pub mod stereo;

/// Numbers a plug-in reports to its editor each block: meters, gain reduction per band,
/// loudness — whatever its display needs. Their meaning is the plug-in's own; they travel
/// in `Meters::fx` for the effect whose editor is open.
pub const TELEMETRY_LEN: usize = 64;
pub type Telemetry = [f32; TELEMETRY_LEN];

/// A note an effect hears from the track its slot listens to
/// (`cantus_core::EffectSlot::midi_from`, ADR 0022).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum FxNote {
    On {
        pitch: u8,
        velocity: u8,
    },
    Off {
        pitch: u8,
    },
    /// Every note stops: the transport stopped, sought or wrapped a loop.
    AllOff,
}

pub trait FxUnit: Send + std::fmt::Debug {
    /// New parameter values. Called on the audio thread with each snapshot: read the spec,
    /// set the DSP, never allocate. Changes should glide where a jump would click.
    fn configure(&mut self, spec: &EffectSpec);
    /// Process a stereo block in place; `key` is the sidechain when one is routed.
    fn process(&mut self, left: &mut [f32], right: &mut [f32], key: Option<(&[f32], &[f32])>);
    fn reset(&mut self);
    /// True when the unit is **at rest** (ADR 0031 §5.1): its state equals, for everything
    /// that can reach its output, a freshly reset unit's with the same configuration — once
    /// that unit has been [`skip`](Self::skip)ped by as many samples as this one has
    /// processed since it was made or reset (so the counters agree too). Then, with silent
    /// input (silent key, no note), `process` outputs exact zeros and leaves the unit at
    /// rest, and `skip` is exactly that — so the engine may stop calling `process` until
    /// something arrives, and may reset the unit, without changing a sample.
    ///
    /// **Exact, never a guess**: a unit that cannot prove it — a free-running LFO, a
    /// smoother still gliding, noise that advances on silence — says `false`. Cheap: the
    /// engine may ask once per block, so a unit keeps counters rather than scanning long
    /// lines. Default `false`: nothing is ever skipped until a unit opts in. Checked for
    /// every native kind by the twin tests (`tests/fx_contract.rs`).
    fn at_rest(&self) -> bool {
        false
    }
    /// The engine did not call `process` for `frames` samples of silence (silent key, no
    /// note) because the unit was [at rest](Self::at_rest): advance whatever counts samples
    /// — a ring's write position, the grid a unit updates its coefficients on — exactly as
    /// processing silence would have, so that skipping is processing silence, also when a
    /// parameter moves afterwards. Cheap (no audio is rendered), no allocation. Called only
    /// while at rest, and on a fresh unit; the unit stays at rest. Default: nothing counts.
    fn skip(&mut self, frames: usize) {
        let _ = frames;
    }
    /// Samples of delay the unit adds, for delay compensation.
    fn latency(&self) -> usize {
        0
    }
    /// What the editor draws, refreshed each block.
    fn telemetry(&self) -> Telemetry {
        [0.0; TELEMETRY_LEN]
    }
    /// The gain reduction a dynamics unit applies now, dB (positive): what the master's
    /// reduction meter (`Meters::master_gr`, ADR 0026) shows. Default: none.
    fn gain_reduction_db(&self) -> Option<f32> {
        None
    }

    // ---- For units the engine did not build: hosted plug-ins (ADR 0019). Every one has
    // ---- a default, so a native unit ignores them.

    /// Where the song is, before each `process`: for a unit that follows tempo.
    fn set_position(&mut self, position: &SongPosition) {
        let _ = position;
    }
    /// What the unit's own editor did in the last `process` calls (a hosted plug-in's
    /// window moving a parameter), oldest first. The processor forwards each as
    /// `EngineEvent::FxParam` and then calls [`Self::clear_param_events`].
    fn param_events(&self) -> &[FxParamEvent] {
        &[]
    }
    fn clear_param_events(&mut self) {}
    /// Called on the audio thread just before the unit is handed back to be dropped:
    /// `process` is not called again. A hosted plug-in stops processing here.
    fn stop(&mut self) {}

    /// A note from the slot's MIDI source, `offset` samples into the next `process` call
    /// (always inside it): sample-accurate whatever the block size, already delayed by
    /// the host to line up with the audio arriving at this slot. Called on the audio
    /// thread before that `process`; must not allocate. Default: ignored.
    fn note(&mut self, offset: usize, note: FxNote) {
        let _ = (offset, note);
    }
}

/// A parameter change made by a unit's own editor, by the unit's parameter id: a gesture's
/// start, a value, a gesture's end. The host records them as commands, one undo step per
/// gesture.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum FxParamEvent {
    Begin(u32),
    Value(u32, f64),
    End(u32),
}

/// The song position at the start of a block, for units that follow the host's tempo.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct SongPosition {
    pub playing: bool,
    pub tempo_bpm: f64,
    /// Quarter notes from the song start.
    pub beats: f64,
    /// Samples from the song start at the current tempo.
    pub samples: i64,
    /// Start of the current bar, in quarter notes.
    pub bar_start_beats: f64,
    pub time_sig_num: u16,
    pub time_sig_den: u16,
    /// The loop, in quarter notes, when looping.
    pub loop_beats: Option<(f64, f64)>,
}

/// A unit that passes audio through untouched: a hosted plug-in that is missing on this
/// machine, or one the engine was given no unit for.
#[derive(Debug, Default)]
pub struct Wire;

impl FxUnit for Wire {
    fn configure(&mut self, _spec: &EffectSpec) {}
    fn process(&mut self, _l: &mut [f32], _r: &mut [f32], _key: Option<(&[f32], &[f32])>) {}
    fn reset(&mut self) {}
}

/// The unit for a kind whose DSP lives behind this contract; `None` for the kinds the
/// engine handles directly (delay, reverb, EQ, compressor).
pub fn make(kind: EffectKind, sample_rate: f32) -> Option<Box<dyn FxUnit>> {
    Some(match kind {
        EffectKind::Limiter => Box::new(limiter::Unit::new(sample_rate)),
        EffectKind::Space => Box::new(space::Unit::new(sample_rate)),
        EffectKind::Multiband => Box::new(multiband::Unit::new(sample_rate)),
        EffectKind::DeEsser => Box::new(deesser::Unit::new(sample_rate)),
        EffectKind::Gate => Box::new(gate::Unit::new(sample_rate)),
        EffectKind::Saturator => Box::new(saturator::Unit::new(sample_rate)),
        EffectKind::Stereo => Box::new(stereo::Unit::new(sample_rate)),
        EffectKind::Maximizer => Box::new(maximizer::Unit::new(sample_rate)),
        // Hosted: the host delivers the real unit (`EngineHandle::add_unit`); without one
        // the slot is a wire.
        EffectKind::External => Box::new(Wire),
        _ => return None,
    })
}

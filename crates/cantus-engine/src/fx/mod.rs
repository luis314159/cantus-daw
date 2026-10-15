//! The plug-in contract on the engine side: one [`FxUnit`] per plug-in, in a module of its
//! own, so a plug-in is owned by its files and the processor never learns its name.
//!
//! `EffectUnit` (`crate::effects`) holds a `Box<dyn FxUnit>` for every kind made here. The
//! box is built on the UI thread and handed to the audio thread, which only calls these
//! methods; none of them may allocate, lock or block (ADR 0005).

use cantus_core::{EffectKind, EffectSpec};

pub mod anulus;
pub mod concentus;
pub mod deesser;
pub mod discrimen;
pub mod flexus;
pub mod fluctus;
pub mod gate;
pub mod impetus;
pub mod limiter;
pub mod maximizer;
pub mod modfx;
pub mod multiband;
pub mod phasis;
pub mod saturator;
pub mod space;
pub mod speculum;
pub mod stereo;
pub mod tonsor;
pub mod tonus;
pub mod translatio;
pub mod tremor;
pub mod vibrans;

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
    /// True when the unit is **at rest** (ADR 0031 §5.1). Exactly:
    ///
    /// 1. silent input (silent key, no note) gives exact zeros out and leaves it at rest;
    /// 2. **suspension is exact**: skipping those calls — telling the unit with
    ///    [`skip`](Self::skip) — changes no sample of what follows, whatever input and
    ///    whatever parameter changes come next;
    /// 3. **it is as good as new**: it renders exactly what a freshly reset unit with the same
    ///    configuration renders (that unit [`skip`](Self::skip)ped as far as this one has run,
    ///    so their counters agree) for any input, as long as the parameters stay as they
    ///    are. A unit whose memory outlives silence (a loudness match that holds its level)
    ///    may differ from a new one only after a parameter moves.
    ///
    /// **Exact, never a guess**: a unit that cannot prove it — a free-running LFO, a
    /// smoother still gliding, noise that advances on silence — says `false`. Cheap: the
    /// engine may ask once per block, so a unit keeps counters rather than scanning long
    /// lines. Default `false`: nothing is ever skipped until a unit opts in. Checked for
    /// every native kind by the twin tests (`tests/fx_contract.rs`).
    fn at_rest(&self) -> bool {
        false
    }
    /// Why the unit is not at rest, when it can say: the first condition that fails, by
    /// name — for the tests and a profiler that shows why a node keeps running. `None`
    /// exactly when [`Self::at_rest`] is true.
    fn rest_blocker(&self) -> Option<&'static str> {
        (!self.at_rest()).then_some("not at rest")
    }
    /// The engine did not call `process` for `frames` samples of silence (silent key, no
    /// note) because the unit was [at rest](Self::at_rest): advance whatever counts samples
    /// — a ring's write position, the grid a unit updates its coefficients on, a meter's
    /// columns — exactly as processing silence would have. Cheap (no audio is rendered), no
    /// allocation. Called only while at rest (a new unit is, when its kind can rest); it
    /// stays at rest. Default: nothing counts.
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

    /// A hosted plug-in's parameter `id` driven by automation or modulators (ADR 0057),
    /// `offset` samples into the next `process` call (always inside it), in time order.
    /// Called on the audio thread before that `process`; must not allocate. A native unit
    /// is configured through its parameter set instead. Default: ignored.
    fn automate(&mut self, offset: usize, id: u32, value: HostedValue) {
        let _ = (offset, id, value);
    }

    /// The engine's automation was planned again (a new snapshot): from the next control
    /// point on, a parameter this unit is not sent [`Self::automate`] for is the project's
    /// own again. Default: ignored.
    fn automation_replanned(&mut self) {}
}

/// What automation and modulation ask of a hosted parameter (ADR 0057), in the plug-in's
/// **normalised** domain (0…1 of its range): the automated position (`NaN`: nothing
/// automates it here, the project's own value is the base) and the modulators' sum, which
/// the unit adds to the base and clamps.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct HostedValue {
    pub pos: f32,
    pub modulation: f32,
}

impl HostedValue {
    /// Nothing drives the parameter here: the project's own value.
    pub const RELEASED: HostedValue = HostedValue {
        pos: f32::NAN,
        modulation: 0.0,
    };

    /// Whether it leaves the parameter to the project.
    pub fn is_released(&self) -> bool {
        !self.pos.is_finite() && self.modulation == 0.0
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
        EffectKind::Chorus => Box::new(concentus::Unit::new(sample_rate)),
        EffectKind::Flanger => Box::new(flexus::Unit::new(sample_rate)),
        EffectKind::Phaser => Box::new(phasis::Unit::new(sample_rate)),
        EffectKind::Tremolo => Box::new(tremor::Unit::new(sample_rate)),
        EffectKind::Vibrato => Box::new(vibrans::Unit::new(sample_rate)),
        EffectKind::RingMod => Box::new(anulus::Unit::new(sample_rate)),
        EffectKind::FreqShift => Box::new(translatio::Unit::new(sample_rate)),
        EffectKind::VolumeShaper => Box::new(fluctus::Unit::new(sample_rate)),
        EffectKind::Clipper => Box::new(tonsor::Unit::new(sample_rate)),
        EffectKind::Transient => Box::new(impetus::Unit::new(sample_rate)),
        EffectKind::TransientSplit => Box::new(discrimen::Unit::new(sample_rate)),
        EffectKind::Analyzer => Box::new(speculum::Unit::new(sample_rate)),
        EffectKind::AutoTune => Box::new(tonus::Unit::new(sample_rate)),
        EffectKind::External => Box::new(Wire),
        _ => return None,
    })
}

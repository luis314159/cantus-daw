//! Messages crossing the UI ⇄ audio-thread boundary. Everything here is `Send`, fixed-size
//! (no heap allocation in the audio thread) and travels over `rtrb` SPSC ring buffers.

use crate::effects::EffectUnit;
use cantus_core::{EffectId, Snapshot, StripId, Ticks, TrackId};
use std::sync::Arc;

/// Hard limits that let the audio thread pre-allocate everything (ADR 0005).
pub const MAX_TRACKS: usize = 64;
/// Mixer strips the engine can hold, buffers preallocated for each.
pub const MAX_STRIPS: usize = 64;
pub const MAX_BLOCK_FRAMES: usize = 8192;
/// Most tracks that play the live notes besides the live track (multitrack MIDI
/// recording: every armed instrument track sounds).
pub const MAX_LIVE_ECHO: usize = 8;
/// Insert effects per track.
pub const MAX_EFFECTS_PER_TRACK: usize = cantus_core::MAX_INSERTS;
/// Sends per strip, preallocated on the audio thread like everything else.
pub const MAX_SENDS_PER_STRIP: usize = cantus_core::MAX_SENDS;
/// Effect units the engine can hold before a snapshot claims them.
pub const MAX_PENDING_EFFECTS: usize = 256;
/// Messages the engine drains per block at most; the rest wait for the next block.
pub const MAX_MESSAGES_PER_BLOCK: usize = 256;
/// The recorder's ring: interleaved stereo samples, four seconds at 48 kHz. The UI drains
/// it every frame, so this is the slack for a UI that stalls, not a buffer that fills.
pub const CAPTURE_SAMPLES: usize = 48_000 * 2 * 4;
/// The analyser's ring: pairs of (input, output) mono samples of the effect being looked
/// at. Half a second of slack at 48 kHz.
pub const ANALYSER_SAMPLES: usize = 48_000;
/// Bands an EQ reports its dynamic gain for.
pub const METERED_EQ_BANDS: usize = cantus_core::EQ_BANDS;

/// What the mix recorder is taking: nothing, the master (after its fader and polarity,
/// before the metronome), or one strip after its fader.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum CaptureTarget {
    #[default]
    Off,
    Master,
    Strip(StripId),
}

/// A raw MIDI channel message (channel nibble kept in `status`).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct MidiEvent {
    pub status: u8,
    pub data1: u8,
    pub data2: u8,
}

impl MidiEvent {
    pub fn note_on(pitch: u8, velocity: u8) -> Self {
        MidiEvent {
            status: 0x90,
            data1: pitch,
            data2: velocity,
        }
    }

    pub fn note_off(pitch: u8) -> Self {
        MidiEvent {
            status: 0x80,
            data1: pitch,
            data2: 0,
        }
    }
}

/// UI → engine.
#[derive(Debug)]
pub enum EngineMessage {
    /// A freshly compiled project snapshot. The engine keeps it until the next one.
    SetSnapshot(Arc<Snapshot>),
    /// An effect instance for an id the engine has not seen yet. Must precede the
    /// snapshot that references it.
    AddEffect(Box<EffectUnit>),
    /// A new instance for an id the engine already runs (a hosted plug-in restarted, or
    /// taken out while it restarts): swapped in where the old one was, the old one stopped
    /// and handed back, and delay compensation planned again for the new latency.
    ReplaceEffect(Box<EffectUnit>),
    /// A native instrument's unit for one track (ADR 0023). Must precede the snapshot in
    /// which that track hosts that kind.
    AddInstrument(Box<crate::inst::NativeUnit>),
    /// A unit in place of the one the engine runs for the same track and kind (a hosted
    /// instrument restarted with another latency, ADR 0028): swapped in at once, the old
    /// one stopped and handed back, compensation planned again. Waits like
    /// [`Self::AddInstrument`] when no snapshot holds that track and kind yet.
    ReplaceInstrument(Box<crate::inst::NativeUnit>),
    /// Report this track's instrument telemetry in `Meters::inst` (its editor is open).
    SetInstrumentProbe(Option<TrackId>),
    Play,
    Stop,
    /// Jump to an absolute position (works while playing or stopped).
    Seek(Ticks),
    /// Live note from the computer keyboard, played on the given track.
    NoteOn {
        track: TrackId,
        pitch: u8,
        velocity: u8,
    },
    NoteOff {
        track: TrackId,
        pitch: u8,
    },
    /// Raw MIDI (hardware inputs use a dedicated queue; this variant serves tests).
    Midi(MidiEvent),
    /// Track that receives hardware MIDI and is recorded into.
    SetLiveTrack(Option<TrackId>),
    /// Tracks that also play every live note (and controller) the live track gets: the
    /// armed instrument tracks of a recording pass, so each is heard as it is recorded
    /// into (multitrack MIDI recording, as Cubase and Logic do it). Only the live track's
    /// notes are reported back; an entry equal to the note's own track is skipped.
    SetLiveEcho([Option<TrackId>; MAX_LIVE_ECHO]),
    /// While recording, live notes are reported back as [`EngineEvent::NoteInput`].
    SetRecording(bool),
    /// Start, switch or stop the mix recorder. Samples go into the capture ring the
    /// processor was built with; without one this does nothing.
    SetCapture(CaptureTarget),
    /// Feed the analyser from this effect's input and output (the editor that is open), or
    /// stop feeding it.
    SetAnalyser(Option<EffectId>),
    /// Release every voice on every track immediately.
    Panic,
    /// Longer delay-compensation lines, answering [`EngineEvent::PdcNeed`]. The engine
    /// swaps them in for free short ones and sends those back as
    /// [`EngineEvent::GarbageLines`].
    PdcLines(Vec<crate::pdc::DelayLine>),
    /// Memory for one effect, answering [`EngineEvent::EffectNeed`]. The effect switches to
    /// the setting that needed it when it arrives; what it replaces comes back as
    /// [`EngineEvent::GarbageMemory`].
    EffectMemory {
        effect: EffectId,
        memory: crate::effects::EffectMemory,
    },
    /// Rings for the armed tracks of the next recording pass (ADR 0025). The old set, if
    /// any, comes back as [`EngineEvent::GarbageRecorder`].
    AttachRecorder(Box<crate::recorder::RecordRings>),
    /// Stop writing into the rings and hand them back.
    DetachRecorder,
    /// Where the take begins when recording starts with a count-in (`ClickMode::CountInOnly`
    /// clicks only before it); `None` when not counting in.
    SetRecordStart(Option<Ticks>),
}

/// Peak meters for one block. Track order matches the snapshot's track order.
#[derive(Clone, Debug)]
pub struct Meters {
    pub master: [f32; 2],
    pub track_count: usize,
    /// Each track's own output, before its strip: what the arrangement header shows.
    pub tracks: [f32; MAX_TRACKS],
    pub strip_count: usize,
    /// Each strip after its fader: what the mixer shows.
    pub strips: [f32; MAX_STRIPS],
    /// Each strip's compressor gain reduction, dB (positive): the deepest of its working
    /// compressors in the block, 0 without one.
    pub strip_gr: [f32; MAX_STRIPS],
    /// The analysed EQ's dynamic gain per band, dB.
    pub eq_deltas: [f32; METERED_EQ_BANDS],
    /// The analysed compressor's last block: input peak, output peak (linear), and gain
    /// reduction (dB).
    pub comp: [f32; 3],
    /// The analysed plug-in's telemetry (`crate::fx::FxUnit::telemetry`).
    pub fx: crate::fx::Telemetry,
    /// The probed track's instrument telemetry (`crate::inst::InstrumentUnit::telemetry`).
    pub inst: crate::inst::Telemetry,
    /// Each input channel's peak this block (ADR 0025); the first `input_count` are live.
    pub input_count: usize,
    pub inputs: [f32; crate::recorder::MAX_INPUTS],
    /// The master chain's gain reduction, dB (positive), the deepest of its dynamics.
    pub master_gr: f32,
}

impl Default for Meters {
    fn default() -> Self {
        Meters {
            master: [0.0; 2],
            track_count: 0,
            tracks: [0.0; MAX_TRACKS],
            strip_count: 0,
            strips: [0.0; MAX_STRIPS],
            strip_gr: [0.0; MAX_STRIPS],
            eq_deltas: [0.0; METERED_EQ_BANDS],
            comp: [0.0; 3],
            fx: [0.0; crate::fx::TELEMETRY_LEN],
            inst: [0.0; crate::inst::TELEMETRY_LEN],
            input_count: 0,
            inputs: [0.0; crate::recorder::MAX_INPUTS],
            master_gr: 0.0,
        }
    }
}

/// Real-time instrumentation (idea.md §12). Updated every block.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct EngineStats {
    pub sample_rate: f64,
    pub block_frames: usize,
    /// Wall time of the last callback in microseconds.
    pub last_block_us: f64,
    /// Worst callback since the stream started.
    pub max_block_us: f64,
    /// last_block_us / block budget. > 1.0 means the block overran.
    pub load: f32,
    pub overruns: u64,
    /// Messages the UI sent that could not be delivered (queue full).
    pub dropped_events: u64,
    pub active_voices: usize,
    pub blocks_processed: u64,
    /// Recorder samples dropped because the capture ring was full.
    pub capture_overruns: u64,
    /// Delay compensation: how far the output lags the engine's position, in samples —
    /// the latency of the slowest path to the master. The position the engine reports as
    /// heard (`EngineEvent::Position::heard`), the click and recorded notes all allow for
    /// it (ADR 0022).
    pub latency_samples: u64,
    /// Edges that needed compensation and found no free delay line.
    pub pdc_unserved: u64,
    /// The input ring held more than its target (the input device's clock runs fast
    /// against the output's): frames thrown away to catch up (ADR 0025 §10).
    pub input_dropped: u64,
    /// The input ring ran short (the input's clock runs slow, or it stalled): frames the
    /// processor made up by holding the last one.
    pub input_repeated: u64,
    /// Recorded frames lost because a track's recording ring was full (the disk fell
    /// behind); each loss is also reported as `RecordMark::Overflow`.
    pub record_overflow: u64,
    /// How long the input frames the processor pops have waited since their buffer
    /// arrived, in frames, smoothed (`crate::devices::queue_wait`); 0 without an input.
    /// Part of the latency a recorded take is moved by (ADR 0025 §7).
    pub input_queue: f64,
}

/// A live note the engine saw while recording, stamped with the position being **heard**
/// when it came in — where the player put it (ADR 0022).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct NoteInput {
    pub track: TrackId,
    pub tick: Ticks,
    pub pitch: u8,
    /// 0 = note off.
    pub velocity: u8,
}

/// Engine → UI.
#[derive(Debug)]
// `Meters` grew a strip array; boxing it would put an allocation on the audio thread
// every block, which is the one thing this enum must never do. The ring buffer's slots
// are sized for the largest variant once, at construction.
#[allow(clippy::large_enum_variant)]
pub enum EngineEvent {
    Position {
        /// Where the engine is: what it is rendering now.
        tick: Ticks,
        /// What is being **heard** now: `tick` less the output's latency
        /// (`EngineStats::latency_samples`), held at the start point until the first sample
        /// of a new start has come out (ADR 0022). What the user is shown.
        heard: Ticks,
        playing: bool,
    },
    Meters(Meters),
    Stats(EngineStats),
    NoteInput(NoteInput),
    /// A snapshot the engine no longer uses; the UI drops it (never the audio thread).
    Garbage(Arc<Snapshot>),
    /// An effect instance no snapshot references any more.
    GarbageEffect(Box<EffectUnit>),
    /// A native instrument's unit the engine no longer uses.
    GarbageInstrument(Box<crate::inst::NativeUnit>),
    /// Delay compensation needs `count` lines able to delay `delay` samples; answer with
    /// [`EngineMessage::PdcLines`] (`crate::pdc::lines_for`).
    PdcNeed {
        count: usize,
        delay: usize,
    },
    /// Delay lines swapped out of the pool; the UI drops them.
    GarbageLines(Vec<crate::pdc::DelayLine>),
    /// A unit's own editor moved a parameter (a hosted plug-in's window): the host turns
    /// these into commands, one undo step per gesture.
    FxParam {
        effect: EffectId,
        event: crate::fx::FxParamEvent,
    },
    /// A track's instrument's own editor moved a parameter (a hosted instrument's window,
    /// ADR 0028), as [`Self::FxParam`] is for an insert.
    InstParam {
        track: TrackId,
        event: crate::fx::FxParamEvent,
    },
    /// An effect's settings need memory it has not got (a linear-phase EQ's convolver);
    /// answer with [`EngineMessage::EffectMemory`] (`crate::effects::EffectMemory::build`).
    /// Until it arrives the effect keeps running its previous path.
    EffectNeed {
        effect: EffectId,
        need: crate::effects::MemoryNeed,
    },
    /// Effect memory handed back; the UI drops it.
    GarbageMemory(crate::effects::EffectMemory),
    /// Where a recording pass is (`crate::recorder`).
    Record(crate::recorder::RecordMark),
    /// Recording rings handed back; the UI drops them.
    GarbageRecorder(Box<crate::recorder::RecordRings>),
}

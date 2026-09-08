//! Messages crossing the UI ⇄ audio-thread boundary. Everything here is `Send`, fixed-size
//! (no heap allocation in the audio thread) and travels over `rtrb` SPSC ring buffers.

use crate::effects::EffectUnit;
use cantus_core::{Snapshot, Ticks, TrackId};
use std::sync::Arc;

/// Hard limits that let the audio thread pre-allocate everything (ADR 0005).
pub const MAX_TRACKS: usize = 64;
pub const MAX_BLOCK_FRAMES: usize = 8192;
/// Insert effects per track.
pub const MAX_EFFECTS_PER_TRACK: usize = 8;
/// Effect units the engine can hold before a snapshot claims them.
pub const MAX_PENDING_EFFECTS: usize = 256;
/// Messages the engine drains per block at most; the rest wait for the next block.
pub const MAX_MESSAGES_PER_BLOCK: usize = 256;

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
    /// While recording, live notes are reported back as [`EngineEvent::NoteInput`].
    SetRecording(bool),
    /// Release every voice on every track immediately.
    Panic,
}

/// Peak meters for one block. Track order matches the snapshot's track order.
#[derive(Clone, Debug)]
pub struct Meters {
    pub master: [f32; 2],
    pub track_count: usize,
    pub tracks: [f32; MAX_TRACKS],
}

impl Default for Meters {
    fn default() -> Self {
        Meters {
            master: [0.0; 2],
            track_count: 0,
            tracks: [0.0; MAX_TRACKS],
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
}

/// A live note the engine saw while recording, stamped with the transport position.
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
pub enum EngineEvent {
    Position {
        tick: Ticks,
        playing: bool,
    },
    Meters(Meters),
    Stats(EngineStats),
    NoteInput(NoteInput),
    /// A snapshot the engine no longer uses; the UI drops it (never the audio thread).
    Garbage(Arc<Snapshot>),
    /// An effect instance no snapshot references any more.
    GarbageEffect(Box<EffectUnit>),
}

//! Messages crossing the UI ⇄ audio-thread boundary. Everything here is `Send`, fixed-size
//! (no heap allocation in the audio thread) and travels over `rtrb` SPSC ring buffers.

use cantus_core::{Snapshot, Ticks, TrackId};
use std::sync::Arc;

/// Hard limits that let the audio thread pre-allocate everything (ADR 0005).
pub const MAX_TRACKS: usize = 64;
pub const MAX_BLOCK_FRAMES: usize = 8192;
/// Messages the engine drains per block at most; the rest wait for the next block.
pub const MAX_MESSAGES_PER_BLOCK: usize = 256;

/// UI → engine.
#[derive(Debug)]
pub enum EngineMessage {
    /// A freshly compiled project snapshot. The engine keeps it until the next one.
    SetSnapshot(Arc<Snapshot>),
    Play,
    Stop,
    /// Jump to an absolute position (works while playing or stopped).
    Seek(Ticks),
    /// Live note from the computer keyboard / MIDI, played on the given track.
    NoteOn {
        track: TrackId,
        pitch: u8,
        velocity: u8,
    },
    NoteOff {
        track: TrackId,
        pitch: u8,
    },
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

/// Engine → UI.
#[derive(Debug)]
pub enum EngineEvent {
    Position {
        tick: Ticks,
        playing: bool,
    },
    Meters(Meters),
    Stats(EngineStats),
    /// A snapshot the engine no longer uses; the UI drops it (never the audio thread).
    Garbage(Arc<Snapshot>),
}

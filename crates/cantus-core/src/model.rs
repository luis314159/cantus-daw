//! The project model: the single source of truth that the UI edits (through commands),
//! the project file stores, and the engine snapshots.
//!
//! This module is deliberately plain data. All behaviour lives in [`crate::command`].

use crate::effect::EffectSlot;
use crate::instrument::InstrumentSpec;
use crate::time::{Ticks, TimeSignature};
use serde::{Deserialize, Serialize};
use std::fmt;

macro_rules! id_type {
    ($name:ident, $prefix:literal) => {
        #[derive(
            Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
        )]
        #[serde(transparent)]
        pub struct $name(pub u64);

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}{}", $prefix, self.0)
            }
        }
    };
}

id_type!(TrackId, "t");
id_type!(ClipId, "c");
id_type!(NoteId, "n");
id_type!(EffectId, "fx");

/// MIDI-style note (0..=127). Middle C is 60.
pub type Pitch = u8;
/// 1..=127. Zero is never stored; a velocity-0 note-on is a note-off in MIDI.
pub type Velocity = u8;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Note {
    pub id: NoteId,
    /// Start relative to the clip start.
    pub start: Ticks,
    pub length: Ticks,
    pub pitch: Pitch,
    pub velocity: Velocity,
}

impl Note {
    pub fn end(&self) -> Ticks {
        self.start + self.length
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Clip {
    pub id: ClipId,
    pub name: String,
    /// Absolute start on the timeline.
    pub start: Ticks,
    pub length: Ticks,
    /// Notes relative to `start`. Notes whose start lies outside `[0, length)` are not
    /// played (they are kept so shrinking then growing a clip is lossless).
    pub notes: Vec<Note>,
}

impl Clip {
    pub fn end(&self) -> Ticks {
        self.start + self.length
    }

    pub fn note(&self, id: NoteId) -> Option<&Note> {
        self.notes.iter().find(|n| n.id == id)
    }

    pub fn note_mut(&mut self, id: NoteId) -> Option<&mut Note> {
        self.notes.iter_mut().find(|n| n.id == id)
    }
}

/// A track's colour, stored as sRGB so themes can derive contrast from it.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Color { r, g, b }
    }

    /// A pleasant, distinguishable palette for new tracks.
    pub const PALETTE: [Color; 8] = [
        Color::rgb(0xE0, 0x8E, 0x3A),
        Color::rgb(0x4F, 0xA3, 0xC7),
        Color::rgb(0xB3, 0x6B, 0xD6),
        Color::rgb(0x5C, 0xB8, 0x6E),
        Color::rgb(0xD9, 0x5D, 0x5D),
        Color::rgb(0xD8, 0xC0, 0x4A),
        Color::rgb(0x4A, 0x8F, 0xD8),
        Color::rgb(0xC9, 0x6F, 0x9E),
    ];
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Track {
    pub id: TrackId,
    pub name: String,
    pub color: Color,
    pub instrument: InstrumentSpec,
    /// Insert effects, processed in order after the instrument.
    #[serde(default)]
    pub effects: Vec<EffectSlot>,
    pub clips: Vec<Clip>,
    /// Fader gain in decibels. 0 dB = unity, `-inf` is represented by [`MIN_GAIN_DB`].
    pub gain_db: f32,
    /// -1.0 (hard left) ..= 1.0 (hard right).
    pub pan: f32,
    pub muted: bool,
    pub soloed: bool,
}

pub const MIN_GAIN_DB: f32 = -96.0;
pub const MAX_GAIN_DB: f32 = 12.0;

impl Track {
    pub fn clip(&self, id: ClipId) -> Option<&Clip> {
        self.clips.iter().find(|c| c.id == id)
    }

    pub fn clip_mut(&mut self, id: ClipId) -> Option<&mut Clip> {
        self.clips.iter_mut().find(|c| c.id == id)
    }

    pub fn effect(&self, id: EffectId) -> Option<&EffectSlot> {
        self.effects.iter().find(|e| e.id == id)
    }

    pub fn effect_mut(&mut self, id: EffectId) -> Option<&mut EffectSlot> {
        self.effects.iter_mut().find(|e| e.id == id)
    }

    pub fn effect_index(&self, id: EffectId) -> Option<usize> {
        self.effects.iter().position(|e| e.id == id)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LoopRegion {
    pub start: Ticks,
    pub end: Ticks,
}

impl LoopRegion {
    pub fn length(&self) -> Ticks {
        self.end - self.start
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Master {
    pub gain_db: f32,
}

impl Default for Master {
    fn default() -> Self {
        Master { gain_db: 0.0 }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Project {
    pub name: String,
    pub tempo_bpm: f64,
    pub time_signature: TimeSignature,
    pub tracks: Vec<Track>,
    pub master: Master,
    pub loop_region: Option<LoopRegion>,
    pub loop_enabled: bool,
    pub metronome: bool,
    /// Monotonic id source for tracks, clips and notes. Never reused, never decremented.
    pub next_id: u64,
}

impl Default for Project {
    fn default() -> Self {
        Project {
            name: "Untitled".to_string(),
            tempo_bpm: 120.0,
            time_signature: TimeSignature::default(),
            tracks: Vec::new(),
            master: Master::default(),
            loop_region: Some(LoopRegion {
                start: Ticks::ZERO,
                end: Ticks::beats(16),
            }),
            loop_enabled: false,
            metronome: false,
            next_id: 1,
        }
    }
}

impl Project {
    pub fn new(name: impl Into<String>) -> Self {
        Project {
            name: name.into(),
            ..Default::default()
        }
    }

    /// Allocate a fresh id. Ids are shared across all entity types so a `u64` from any
    /// source can never collide with another.
    pub fn alloc_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub fn track(&self, id: TrackId) -> Option<&Track> {
        self.tracks.iter().find(|t| t.id == id)
    }

    pub fn track_mut(&mut self, id: TrackId) -> Option<&mut Track> {
        self.tracks.iter_mut().find(|t| t.id == id)
    }

    pub fn track_index(&self, id: TrackId) -> Option<usize> {
        self.tracks.iter().position(|t| t.id == id)
    }

    /// Timeline end: the end of the last clip, or zero if the project is empty.
    pub fn content_end(&self) -> Ticks {
        self.tracks
            .iter()
            .flat_map(|t| t.clips.iter().map(Clip::end))
            .max()
            .unwrap_or(Ticks::ZERO)
    }

    pub fn any_solo(&self) -> bool {
        self.tracks.iter().any(|t| t.soloed)
    }

    /// Whether the track is audible given mute/solo state.
    pub fn track_audible(&self, track: &Track) -> bool {
        if track.muted {
            return false;
        }
        if self.any_solo() {
            track.soloed
        } else {
            true
        }
    }

    /// Build a default track with a fresh id and the next palette colour.
    pub fn make_track(&mut self, name: impl Into<String>) -> Track {
        let id = TrackId(self.alloc_id());
        let color = Color::PALETTE[self.tracks.len() % Color::PALETTE.len()];
        Track {
            id,
            name: name.into(),
            color,
            instrument: InstrumentSpec::default(),
            effects: Vec::new(),
            clips: Vec::new(),
            gain_db: 0.0,
            pan: 0.0,
            muted: false,
            soloed: false,
        }
    }

    pub fn make_clip(&mut self, name: impl Into<String>, start: Ticks, length: Ticks) -> Clip {
        Clip {
            id: ClipId(self.alloc_id()),
            name: name.into(),
            start,
            length,
            notes: Vec::new(),
        }
    }

    pub fn make_effect(&mut self, kind: crate::effect::EffectKind) -> EffectSlot {
        EffectSlot {
            id: EffectId(self.alloc_id()),
            bypass: false,
            spec: crate::effect::EffectSpec::new(kind),
        }
    }

    pub fn make_note(
        &mut self,
        start: Ticks,
        length: Ticks,
        pitch: Pitch,
        velocity: Velocity,
    ) -> Note {
        Note {
            id: NoteId(self.alloc_id()),
            start,
            length,
            pitch: pitch.min(127),
            velocity: velocity.clamp(1, 127),
        }
    }
}

/// Convert a fader value in dB to a linear gain. `MIN_GAIN_DB` and below is silence.
pub fn db_to_linear(db: f32) -> f32 {
    if db <= MIN_GAIN_DB {
        0.0
    } else {
        10f32.powf(db / 20.0)
    }
}

pub fn linear_to_db(lin: f32) -> f32 {
    if lin <= 0.0 {
        MIN_GAIN_DB
    } else {
        (20.0 * lin.log10()).max(MIN_GAIN_DB)
    }
}

/// Equal-power pan law (−3 dB at centre). Returns `(left, right)` multipliers.
pub fn pan_gains(pan: f32) -> (f32, f32) {
    let p = pan.clamp(-1.0, 1.0);
    let angle = (p + 1.0) * 0.25 * std::f32::consts::PI;
    (angle.cos(), angle.sin())
}

/// MIDI pitch → frequency in Hz with A4 = 440 Hz.
pub fn pitch_to_hz(pitch: f32) -> f32 {
    440.0 * 2f32.powf((pitch - 69.0) / 12.0)
}

pub fn pitch_name(pitch: Pitch) -> String {
    const NAMES: [&str; 12] = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    let octave = (pitch / 12) as i32 - 1;
    format!("{}{}", NAMES[(pitch % 12) as usize], octave)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gains() {
        assert!((db_to_linear(0.0) - 1.0).abs() < 1e-6);
        assert!((db_to_linear(-6.0206) - 0.5).abs() < 1e-4);
        assert_eq!(db_to_linear(-200.0), 0.0);
        assert!((linear_to_db(0.5) + 6.0206).abs() < 1e-3);
        let (l, r) = pan_gains(0.0);
        assert!((l - r).abs() < 1e-6);
        assert!((l * l + r * r - 1.0).abs() < 1e-6);
        assert!((pan_gains(-1.0).1).abs() < 1e-6);
    }

    #[test]
    fn pitches() {
        assert!((pitch_to_hz(69.0) - 440.0).abs() < 1e-4);
        assert!((pitch_to_hz(57.0) - 220.0).abs() < 1e-4);
        assert_eq!(pitch_name(60), "C4");
        assert_eq!(pitch_name(61), "C#4");
        assert_eq!(pitch_name(0), "C-1");
    }

    #[test]
    fn ids_never_collide() {
        let mut p = Project::default();
        let t = p.make_track("a");
        let c = p.make_clip("c", Ticks::ZERO, Ticks::beats(4));
        let n = p.make_note(Ticks::ZERO, Ticks::QUARTER, 60, 100);
        assert_ne!(t.id.0, c.id.0);
        assert_ne!(c.id.0, n.id.0);
        assert_eq!(p.next_id, 4);
    }
}

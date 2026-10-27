//! A note clip's **controller lanes** (ADR 0068): pitch bend, channel pressure and any
//! MIDI controller, drawn under the notes in the piano roll — FL's event editor, Ableton's
//! clip envelopes, Cubase's controller lanes.
//!
//! A lane is a curve of [`AutoPoint`]s in the clip's **content** ticks, exactly as its notes
//! are (and as an automation clip's curve is): a split, a trim, a slip, a loop, a linked copy
//! and the clipboard need no special case, and the curve tools, shapes and tensions of
//! automation (ADR 0027 §3) draw it. Values are positions **0…1**: a CC's 0…127, pressure's
//! 0…127, and the bend's −8192…8191 with **0.5 the centre**.
//!
//! What the engine is handed is a stream of [`CtlEvent`]s per track (`Snapshot`), compiled
//! here: every point, the curve between points sampled every [`STEP`] ticks where it
//! moves, and the lane's **rest value** where its clip (or a loop's repetition) ends — so a
//! bend never outlives the clip that drew it, and a render from any start point chases the
//! same values (`TrackSnapshot::controls`).
//!
//! The **release velocity** is the note's own (`Note::release`); its lane in the piano roll
//! is a view of the notes, not a curve.

use crate::automation::AutoPoint;
use crate::model::Clip;
use crate::time::Ticks;
use serde::{Deserialize, Serialize};

/// What a lane drives.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum LaneKind {
    /// The channel's pitch bend (14 bits; 0.5 is the centre).
    PitchBend,
    /// Channel pressure (aftertouch for the whole channel).
    Pressure,
    /// A control change, 0…119 (120…127 are the channel mode messages — all notes off,
    /// reset all controllers — not controllers, MIDI 1.0 §4: no lane sends them).
    Cc(u8),
}

/// The highest controller number a lane may drive.
pub const MAX_CC: u8 = 119;

/// How finely a moving curve is sampled into events, in ticks (1/96 of a beat: 5.2 ms at
/// 120 BPM — the grid `modulator::bake` traces on). A point is always sent at its own tick.
pub const STEP: i64 = crate::time::PPQ / 96;

/// The most lanes one clip holds (every controller, the bend and pressure).
pub const MAX_LANES: usize = MAX_CC as usize + 3;

/// The controllers the add menu names (FL's and every keyboard's): mod wheel, breath, …
pub const NAMED: [u8; 17] = [1, 2, 4, 5, 7, 8, 10, 11, 64, 65, 66, 67, 71, 72, 73, 74, 91];

impl LaneKind {
    /// Whether a lane may drive it.
    pub fn is_valid(self) -> bool {
        match self {
            LaneKind::Cc(n) => n <= MAX_CC,
            _ => true,
        }
    }

    /// Where the controller rests (0…1): the bend's centre, no pressure, a pedal up, and
    /// the General MIDI reset values of volume (100), balance and pan (64) and expression
    /// (127) — what a lane returns to where its clip ends, and what a stop leaves.
    pub fn rest(self) -> f32 {
        match self {
            LaneKind::PitchBend => 0.5,
            LaneKind::Pressure => 0.0,
            LaneKind::Cc(7) => 100.0 / 127.0,
            LaneKind::Cc(8) | LaneKind::Cc(10) => 64.0 / 127.0,
            LaneKind::Cc(11) => 1.0,
            LaneKind::Cc(_) => 0.0,
        }
    }

    /// Whether the lane is a switch (a pedal): drawn and sent as on or off by default.
    pub fn is_switch(self) -> bool {
        matches!(self, LaneKind::Cc(64..=69))
    }

    /// The locale key naming it: `automodes.lane.bend`, `automodes.cc.1`, … — a number
    /// with no name of its own is `automodes.cc.n` with `{n}`.
    pub fn name_key(self) -> &'static str {
        match self {
            LaneKind::PitchBend => "automodes.lane.bend",
            LaneKind::Pressure => "automodes.lane.pressure",
            LaneKind::Cc(n) => match n {
                0 => "automodes.cc.0",
                1 => "automodes.cc.1",
                2 => "automodes.cc.2",
                4 => "automodes.cc.4",
                5 => "automodes.cc.5",
                7 => "automodes.cc.7",
                8 => "automodes.cc.8",
                10 => "automodes.cc.10",
                11 => "automodes.cc.11",
                64 => "automodes.cc.64",
                65 => "automodes.cc.65",
                66 => "automodes.cc.66",
                67 => "automodes.cc.67",
                71 => "automodes.cc.71",
                72 => "automodes.cc.72",
                73 => "automodes.cc.73",
                74 => "automodes.cc.74",
                91 => "automodes.cc.91",
                93 => "automodes.cc.93",
                _ => "automodes.cc.n",
            },
        }
    }

    /// The MIDI value a position sends: 0…127, or the bend's 0…16383.
    pub fn midi(self, pos: f32) -> u16 {
        let p = pos.clamp(0.0, 1.0);
        match self {
            // 0.5 is exactly 8192, both ends exact.
            LaneKind::PitchBend => (p * 16384.0).round().min(16383.0) as u16,
            _ => (p * 127.0).round() as u16,
        }
    }

    /// The position a MIDI value is (the inverse of [`Self::midi`] on its values).
    pub fn pos_of_midi(self, v: u16) -> f32 {
        match self {
            LaneKind::PitchBend => {
                if v >= 16383 {
                    1.0
                } else {
                    f32::from(v) / 16384.0
                }
            }
            _ => f32::from(v.min(127)) / 127.0,
        }
    }

    /// Sort order in a clip and in the piano roll: bend, pressure, then controllers by
    /// number.
    fn order(self) -> u16 {
        match self {
            LaneKind::PitchBend => 0,
            LaneKind::Pressure => 1,
            LaneKind::Cc(n) => 2 + u16::from(n),
        }
    }
}

/// One lane of a note clip: what it drives and its curve, in content ticks.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ClipLane {
    pub kind: LaneKind,
    /// Sorted by tick (two at one tick are a jump), values 0…1.
    #[serde(default)]
    pub points: Vec<AutoPoint>,
}

impl ClipLane {
    pub fn new(kind: LaneKind, mut points: Vec<AutoPoint>) -> Self {
        points.sort_by_key(|p| p.tick);
        ClipLane { kind, points }
    }

    /// What a command accepts: a kind a lane may drive, finite points in range and order,
    /// none before the content's start.
    pub fn is_valid(&self) -> bool {
        self.kind.is_valid()
            && self.points.windows(2).all(|w| w[0].tick <= w[1].tick)
            && self.points.iter().all(|p| {
                p.tick >= Ticks::ZERO
                    && p.value.is_finite()
                    && (0.0..=1.0).contains(&p.value)
                    && p.tension.is_finite()
                    && (-1.0..=1.0).contains(&p.tension)
            })
    }

    /// The curve at a content tick (0…1); `None` for a lane with no points. Before the
    /// first point it holds the first, after the last the last.
    pub fn value_at(&self, content: f64) -> Option<f32> {
        let pts = &self.points;
        let first = pts.first()?;
        let i = pts.partition_point(|p| p.tick.0 as f64 <= content);
        if i == 0 {
            return Some(first.value);
        }
        let p = pts[i - 1];
        match pts.get(i) {
            Some(q) => Some(crate::automation::segment(p, *q, content)),
            None => Some(p.value),
        }
    }
}

/// Whether a clip's lanes are what a command may set: each valid, one per kind, at most
/// [`MAX_LANES`].
pub fn lanes_valid(lanes: &[ClipLane]) -> bool {
    lanes.len() <= MAX_LANES
        && lanes.iter().all(ClipLane::is_valid)
        && lanes
            .iter()
            .enumerate()
            .all(|(i, l)| lanes[..i].iter().all(|m| m.kind != l.kind))
}

/// Sort lanes into the order the piano roll lists them.
pub fn sort_lanes(lanes: &mut [ClipLane]) {
    lanes.sort_by_key(|l| l.kind.order());
}

/// One controller value at a timeline tick, as the engine plays it.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct CtlEvent {
    pub tick: Ticks,
    pub kind: LaneKind,
    /// 0…1 (the bend: 0.5 the centre).
    pub value: f32,
}

#[path = "clip_lanes_compile.rs"]
mod compile;
pub use compile::{compile_track, controls_of_clip, kinds};

#[path = "clip_lanes_edit.rs"]
pub mod edit;

#[cfg(test)]
#[path = "clip_lanes_tests.rs"]
mod tests;

/// The lanes of `clip` as a slice (none for an audio or automation clip).
pub fn of(clip: &Clip) -> &[ClipLane] {
    &clip.lanes
}

//! The session's settings types (ADR 0076): launch modes and quantisation, follow actions,
//! the global settings. Every one has a stable serde form (a key is forever once shipped)
//! and an `ALL` list and `label_key` for the view.

use super::{SceneId, QUARTER};
use crate::time::Ticks;
use serde::{Deserialize, Serialize};

/// The longest fixed recording length (bars) and the most times a follow action's time
/// may pass before it is taken.
pub const MAX_FIXED_BARS: u16 = 512;
pub const MAX_FOLLOW_COUNT: u16 = 999;

/// What pressing (and releasing) a clip's launcher does (Live's launch modes).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LaunchMode {
    /// Press starts it (again, from its start marker); release does nothing.
    #[default]
    Trigger,
    /// Plays while held: release stops it (at the quantisation).
    Gate,
    /// Press starts it; pressing it again while it plays stops it.
    Toggle,
    /// Retriggers at every quantisation boundary while held; release stops it.
    Repeat,
}

impl LaunchMode {
    pub const ALL: [LaunchMode; 4] = [
        LaunchMode::Trigger,
        LaunchMode::Gate,
        LaunchMode::Toggle,
        LaunchMode::Repeat,
    ];

    pub fn label_key(self) -> &'static str {
        match self {
            LaunchMode::Trigger => "session.mode.trigger",
            LaunchMode::Gate => "session.mode.gate",
            LaunchMode::Toggle => "session.mode.toggle",
            LaunchMode::Repeat => "session.mode.repeat",
        }
    }
}

/// When a launch (or a stop) takes effect: on the next boundary of a grid. `Global` —
/// a clip's default — follows the session's own setting.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Quantize {
    #[default]
    Global,
    /// At once (the next block's first sample the message reaches).
    None,
    Bars8,
    Bars4,
    Bars2,
    Bar,
    Half,
    HalfTriplet,
    Quarter,
    QuarterTriplet,
    Eighth,
    EighthTriplet,
    Sixteenth,
    SixteenthTriplet,
    ThirtySecond,
}

/// A grid a launch waits for: none, every `n` bars, or every so many ticks from each bar
/// line.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum QuantGrid {
    None,
    Bars(u8),
    Ticks(i64),
}

impl Quantize {
    pub const ALL: [Quantize; 15] = [
        Quantize::Global,
        Quantize::None,
        Quantize::Bars8,
        Quantize::Bars4,
        Quantize::Bars2,
        Quantize::Bar,
        Quantize::Half,
        Quantize::HalfTriplet,
        Quantize::Quarter,
        Quantize::QuarterTriplet,
        Quantize::Eighth,
        Quantize::EighthTriplet,
        Quantize::Sixteenth,
        Quantize::SixteenthTriplet,
        Quantize::ThirtySecond,
    ];

    /// The grid this resolves to, `Global` taking the session's `global` (which is never
    /// `Global` itself: a session setting of `Global` reads as a bar).
    pub fn grid(self, global: Quantize) -> QuantGrid {
        let q = if self == Quantize::Global {
            if global == Quantize::Global {
                Quantize::Bar
            } else {
                global
            }
        } else {
            self
        };
        let t = |num: i64, den: i64| QuantGrid::Ticks(QUARTER * num / den);
        match q {
            Quantize::Global | Quantize::Bar => QuantGrid::Bars(1),
            Quantize::None => QuantGrid::None,
            Quantize::Bars8 => QuantGrid::Bars(8),
            Quantize::Bars4 => QuantGrid::Bars(4),
            Quantize::Bars2 => QuantGrid::Bars(2),
            Quantize::Half => t(2, 1),
            Quantize::HalfTriplet => t(4, 3),
            Quantize::Quarter => t(1, 1),
            Quantize::QuarterTriplet => t(2, 3),
            Quantize::Eighth => t(1, 2),
            Quantize::EighthTriplet => t(1, 3),
            Quantize::Sixteenth => t(1, 4),
            Quantize::SixteenthTriplet => t(1, 6),
            Quantize::ThirtySecond => t(1, 8),
        }
    }

    pub fn label_key(self) -> &'static str {
        match self {
            Quantize::Global => "session.q.global",
            Quantize::None => "session.q.none",
            Quantize::Bars8 => "session.q.bars8",
            Quantize::Bars4 => "session.q.bars4",
            Quantize::Bars2 => "session.q.bars2",
            Quantize::Bar => "session.q.bar",
            Quantize::Half => "session.q.half",
            Quantize::HalfTriplet => "session.q.half_t",
            Quantize::Quarter => "session.q.quarter",
            Quantize::QuarterTriplet => "session.q.quarter_t",
            Quantize::Eighth => "session.q.eighth",
            Quantize::EighthTriplet => "session.q.eighth_t",
            Quantize::Sixteenth => "session.q.sixteenth",
            Quantize::SixteenthTriplet => "session.q.sixteenth_t",
            Quantize::ThirtySecond => "session.q.thirty_second",
        }
    }
}

/// How notes recorded into a slot are moved to the grid as they come in (Live's record
/// quantisation): a grid, or two grids at once (straight and triplet).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RecordQuantize {
    #[default]
    None,
    Quarter,
    Eighth,
    EighthTriplet,
    EighthAndTriplet,
    Sixteenth,
    SixteenthTriplet,
    SixteenthAndTriplet,
    ThirtySecond,
}

impl RecordQuantize {
    pub const ALL: [RecordQuantize; 9] = [
        RecordQuantize::None,
        RecordQuantize::Quarter,
        RecordQuantize::Eighth,
        RecordQuantize::EighthTriplet,
        RecordQuantize::EighthAndTriplet,
        RecordQuantize::Sixteenth,
        RecordQuantize::SixteenthTriplet,
        RecordQuantize::SixteenthAndTriplet,
        RecordQuantize::ThirtySecond,
    ];

    /// The grids a note start snaps to, the nearest winning (none: as played).
    pub fn grids(self) -> &'static [i64] {
        const Q: i64 = QUARTER;
        match self {
            RecordQuantize::None => &[],
            RecordQuantize::Quarter => &[Q],
            RecordQuantize::Eighth => &[Q / 2],
            RecordQuantize::EighthTriplet => &[Q / 3],
            RecordQuantize::EighthAndTriplet => &[Q / 2, Q / 3],
            RecordQuantize::Sixteenth => &[Q / 4],
            RecordQuantize::SixteenthTriplet => &[Q / 6],
            RecordQuantize::SixteenthAndTriplet => &[Q / 4, Q / 6],
            RecordQuantize::ThirtySecond => &[Q / 8],
        }
    }

    /// `tick` (relative to where the recording's grid starts) moved to the nearest line of
    /// any of the grids; ties go to the earlier line.
    pub fn snap(self, tick: Ticks) -> Ticks {
        let mut best = tick;
        let mut dist = i64::MAX;
        for &g in self.grids() {
            let down = tick.0.div_euclid(g) * g;
            for line in [down, down + g] {
                let d = (line - tick.0).abs();
                if d < dist || (d == dist && line < best.0) {
                    dist = d;
                    best = Ticks(line);
                }
            }
        }
        best
    }

    pub fn label_key(self) -> &'static str {
        match self {
            RecordQuantize::None => "session.rq.none",
            RecordQuantize::Quarter => "session.rq.quarter",
            RecordQuantize::Eighth => "session.rq.eighth",
            RecordQuantize::EighthTriplet => "session.rq.eighth_t",
            RecordQuantize::EighthAndTriplet => "session.rq.eighth_and_t",
            RecordQuantize::Sixteenth => "session.rq.sixteenth",
            RecordQuantize::SixteenthTriplet => "session.rq.sixteenth_t",
            RecordQuantize::SixteenthAndTriplet => "session.rq.sixteenth_and_t",
            RecordQuantize::ThirtySecond => "session.rq.thirty_second",
        }
    }
}

/// How a session clip launches.
#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LaunchSettings {
    #[serde(default)]
    pub mode: LaunchMode,
    #[serde(default)]
    pub quantize: Quantize,
    /// Launched while another clip of its track plays, it takes over that clip's position
    /// instead of starting from its own start marker (Live's *Legato*).
    #[serde(default)]
    pub legato: bool,
    /// How much the launch's velocity (a pad's) sets the clip's level, 0…1: 0 — the
    /// default — plays it at its level whatever the velocity (Live's *Velocity* amount).
    #[serde(default)]
    pub velocity: f32,
}

impl Default for LaunchSettings {
    fn default() -> Self {
        LaunchSettings {
            mode: LaunchMode::Trigger,
            quantize: Quantize::Global,
            legato: false,
            velocity: 0.0,
        }
    }
}

impl LaunchSettings {
    pub fn is_default(&self) -> bool {
        *self == LaunchSettings::default()
    }

    pub fn is_valid(&self) -> bool {
        self.velocity.is_finite() && (0.0..=1.0).contains(&self.velocity)
    }

    /// The gain a launch at `velocity` (0…1) plays the clip with.
    pub fn gain(&self, velocity: f32) -> f32 {
        1.0 - self.velocity * (1.0 - velocity.clamp(0.0, 1.0))
    }
}

/// What a follow action does when its time comes (Live's follow actions). Relative ones
/// count in the column, top to bottom, over the slots that hold clips.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FollowAction {
    /// Nothing: the clip (or scene) goes on playing.
    #[default]
    None,
    /// Stops it.
    Stop,
    /// Launches it again from its start marker.
    Again,
    /// Launches the clip above it (the last after the first).
    Previous,
    /// Launches the clip below it (the first after the last).
    Next,
    /// Launches the column's first clip.
    First,
    /// Launches the column's last clip.
    Last,
    /// Launches a clip of the column drawn at random — itself included.
    Any,
    /// Launches another clip of the column drawn at random (itself when it is alone).
    Other,
    /// Launches the clip in this scene's row (or, for a scene, this scene): one that is
    /// gone (or an empty slot) does nothing.
    Jump(SceneId),
}

impl FollowAction {
    /// Every kind, `Jump` with a placeholder scene (the view picks the row).
    pub const KINDS: [FollowAction; 10] = [
        FollowAction::None,
        FollowAction::Stop,
        FollowAction::Again,
        FollowAction::Previous,
        FollowAction::Next,
        FollowAction::First,
        FollowAction::Last,
        FollowAction::Any,
        FollowAction::Other,
        FollowAction::Jump(SceneId(0)),
    ];

    pub fn label_key(self) -> &'static str {
        match self {
            FollowAction::None => "session.follow.none",
            FollowAction::Stop => "session.follow.stop",
            FollowAction::Again => "session.follow.again",
            FollowAction::Previous => "session.follow.previous",
            FollowAction::Next => "session.follow.next",
            FollowAction::First => "session.follow.first",
            FollowAction::Last => "session.follow.last",
            FollowAction::Any => "session.follow.any",
            FollowAction::Other => "session.follow.other",
            FollowAction::Jump(_) => "session.follow.jump",
        }
    }
}

/// When the follow action is taken: after a time, or after the clip's own length (Live's
/// *Linked*: a pass of the clip from its start marker, then whole loops) — in both cases
/// `count` times over.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FollowAfter {
    /// The clip's length: its first pass, then its loop for each further count. For a
    /// scene, the longest of its clips' first passes.
    Linked,
    /// A time, from the launch (bars and beats as ticks; never zero).
    Time(Ticks),
}

/// The follow actions of a clip (or a scene): two actions and the chance of each, and when.
#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FollowActions {
    /// Live 11's switch: off, nothing follows (the actions are kept).
    pub on: bool,
    pub a: FollowAction,
    pub b: FollowAction,
    /// The chance of A, 0…100 (B has the rest): drawn from the session's seed, so a
    /// performance replays and renders the same.
    pub chance_a: u8,
    pub after: FollowAfter,
    /// How many times `after` passes before the action: a clip linked with a count of 4
    /// plays four times, then follows (a repeat count). At least 1.
    pub count: u16,
}

impl Default for FollowActions {
    fn default() -> Self {
        FollowActions::off()
    }
}

impl FollowActions {
    /// Nothing follows: what every clip and scene starts with.
    pub const fn off() -> Self {
        FollowActions {
            on: false,
            a: FollowAction::None,
            b: FollowAction::None,
            chance_a: 100,
            after: FollowAfter::Linked,
            count: 1,
        }
    }

    pub fn is_off(&self) -> bool {
        *self == FollowActions::off()
    }

    /// Whether anything can follow: on, with an action that does something.
    pub fn is_active(&self) -> bool {
        self.on
            && ((self.chance_a > 0 && self.a != FollowAction::None)
                || (self.chance_a < 100 && self.b != FollowAction::None))
    }

    pub fn is_valid(&self) -> bool {
        self.chance_a <= 100
            && (1..=MAX_FOLLOW_COUNT).contains(&self.count)
            && match self.after {
                FollowAfter::Linked => true,
                FollowAfter::Time(t) => t > Ticks::ZERO,
            }
    }

    /// The action a draw `roll` (0…99) picks: A below its chance, B from there.
    pub fn pick(&self, roll: u8) -> FollowAction {
        if roll < self.chance_a {
            self.a
        } else {
            self.b
        }
    }
}

/// The session's own settings: the global launch quantisation, the record quantisation,
/// the follow actions' seed and a fixed length for recordings into a slot.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionSettings {
    /// What a clip's `Quantize::Global` means (never `Global` itself). A bar by default.
    #[serde(default = "bar")]
    pub launch_quantize: Quantize,
    #[serde(default)]
    pub record_quantize: RecordQuantize,
    /// The seed follow actions draw their chances from: the same seed and the same
    /// launches play the same performance, live and rendered.
    #[serde(default)]
    pub seed: u64,
    /// A recording into a slot stops by itself after this many bars, and is launched
    /// looping (Live's *Fixed Length*). `None`: it ends at the bar line after it is stopped.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fixed_bars: Option<u16>,
}

fn bar() -> Quantize {
    Quantize::Bar
}

impl Default for SessionSettings {
    fn default() -> Self {
        SessionSettings {
            launch_quantize: Quantize::Bar,
            record_quantize: RecordQuantize::None,
            seed: 0,
            fixed_bars: None,
        }
    }
}

impl SessionSettings {
    pub fn is_default(&self) -> bool {
        *self == SessionSettings::default()
    }

    pub fn is_valid(&self) -> bool {
        self.launch_quantize != Quantize::Global
            && self
                .fixed_bars
                .is_none_or(|b| (1..=MAX_FIXED_BARS).contains(&b))
    }
}

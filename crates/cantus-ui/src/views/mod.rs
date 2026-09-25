//! The views. Each is a plain function `show(app, ui)`; state that must survive between
//! frames lives in a small struct owned by `CantusApp`.

pub mod arrangement;
pub mod channel_rack;
pub mod comp_editor;
pub mod eq_editor;
pub mod fx;
pub mod inspector;
pub mod interop;
pub mod mixer;
pub mod piano_roll;
pub mod plugins;
pub mod transport;

use cantus_core::{Ticks, PPQ};

pub fn ticks_to_beats(t: Ticks) -> f32 {
    t.0 as f32 / PPQ as f32
}

pub fn beats_to_ticks(b: f32) -> Ticks {
    Ticks((b * PPQ as f32).round() as i64)
}

/// Grid choices for snapping (label key, ticks).
pub const GRIDS: [(&str, i64); 8] = [
    ("grid.bar", PPQ * 4),
    ("grid.half", PPQ * 2),
    ("grid.quarter", PPQ),
    ("grid.eighth", PPQ / 2),
    ("grid.sixteenth", PPQ / 4),
    ("grid.thirtysecond", PPQ / 8),
    ("grid.triplet8", PPQ / 3),
    ("grid.triplet16", PPQ / 6),
];
